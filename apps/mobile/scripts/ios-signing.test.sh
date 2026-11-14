#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Essais de `ios-signing.sh check` (contrôle de l'archive avant l'envoi à Apple) avec un faux
# `codesign` : application, frameworks et extension de widgets fabriqués ici, signature et droits
# simulés. Vérifie aussi que l'identifiant d'équipe n'est jamais affiché.
# Puis essais de `ios-signing.sh key` (la clé du certificat : neuve sans le secret
# APPLE_DIST_KEY_BASE64, celle du secret sinon) avec des clés jetables créées par openssl ici.
# Ces essais-là tournent aussi sous Linux.
#   bash apps/mobile/scripts/ios-signing.test.sh

set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
TEAM="ZYXWV98765"
failures=0

mkdir -p "$work/bin"
cat > "$work/bin/codesign" << 'EOF'
#!/usr/bin/env bash
# Faux codesign : le dernier argument est le chemin ; des fichiers témoins décident de la réponse.
path="${*: -1}"
case " $* " in
  *" --verify "*) [ ! -e "$path/.badsig" ] ;;
  *" --entitlements "*) [ -f "$path/.ents" ] && cat "$path/.ents" ;;
  *)
    if [ -e "$path/.adhoc" ]; then
      echo "Signature=adhoc" >&2
    else
      echo "Authority=Apple Distribution: Camille Exemple (ZYXWV98765)" >&2
    fi
    ;;
esac
EOF
chmod +x "$work/bin/codesign"

ents() {
  cat << EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>application-identifier</key>
	<string>${TEAM}.$2</string>
	<key>keychain-access-groups</key>
	<array>
$(for group in $1; do printf '\t\t<string>%s</string>\n' "$group"; done)
	</array>
</dict>
</plist>
EOF
}

# make <cas> : une application complète et correcte dans $work/<cas>/App.app.
make() {
  local app="$work/$1/App.app"
  mkdir -p "$app/Frameworks/Capacitor.framework" "$app/PlugIns/CordeWidgets.appex"
  touch "$app/embedded.mobileprovision" "$app/PlugIns/CordeWidgets.appex/embedded.mobileprovision"
  # L'application : son propre groupe d'abord, le groupe partagé ensuite (App.entitlements).
  ents "${TEAM}.io.github.dultra123.corde ${TEAM}.io.github.dultra123.corde.shared" io.github.dultra123.corde > "$app/.ents"
  ents "${TEAM}.io.github.dultra123.corde.shared" io.github.dultra123.corde.widgets > "$app/PlugIns/CordeWidgets.appex/.ents"
  echo "$app"
}

# run <attendu ok|ko> <nom> <app> [motif attendu dans la sortie]
run() {
  local want="$1" name="$2" app="$3" pattern="${4:-}" code=0
  PATH="$work/bin:$PATH" "${BASH:-bash}" "$here/ios-signing.sh" check "$app" > "$work/out.txt" 2>&1 || code=$?
  local got=ok
  [ "$code" -eq 0 ] || got=ko
  if [ "$got" != "$want" ]; then
    echo "FAIL $name (attendu $want, obtenu $got)"
    sed 's/^/     | /' "$work/out.txt"
    failures=$((failures + 1))
  elif [ -n "$pattern" ] && ! grep -q -- "$pattern" "$work/out.txt"; then
    echo "FAIL $name (« $pattern » absent)"
    sed 's/^/     | /' "$work/out.txt"
    failures=$((failures + 1))
  elif grep -q "$TEAM" "$work/out.txt"; then
    echo "FAIL $name (identifiant d'équipe affiché)"
    failures=$((failures + 1))
  else
    echo "ok   $name"
  fi
}

app="$(make good)"
run ok "application, frameworks et widgets signés, trousseau partagé" "$app" "CordeWidgets.appex : trousseau partagé"

app="$(make noappex)"
rm -rf "$app/PlugIns"
run ko "extension absente" "$app" "extension de widgets absente"

app="$(make noents)"
rm "$app/PlugIns/CordeWidgets.appex/.ents"
run ko "droits absents de l'extension" "$app" "CordeWidgets.appex : droit keychain-access-groups absent"

app="$(make noprefix)"
ents "io.github.dultra123.corde.shared" io.github.dultra123.corde > "$app/.ents"
run ko "groupe sans préfixe d'équipe" "$app" "App.app : droit keychain-access-groups absent"

app="$(make adhoc)"
touch "$app/PlugIns/CordeWidgets.appex/.adhoc"
run ko "extension signée ad hoc" "$app" "CordeWidgets.appex : signature locale"

app="$(make badsig)"
touch "$app/Frameworks/Capacitor.framework/.badsig"
run ko "framework à la signature invalide" "$app" "Capacitor.framework : signature invalide"

app="$(make noprofile)"
rm "$app/PlugIns/CordeWidgets.appex/embedded.mobileprovision"
run ko "profil absent de l'extension" "$app" "CordeWidgets.appex : profil de provisionnement absent"

# --- ios-signing.sh key : la clé du certificat (secret facultatif APPLE_DIST_KEY_BASE64) ---------
# Clés jetables, créées ici et effacées à la fin : jamais une vraie clé.

openssl genrsa -out "$work/stable.pem" 2048 2> /dev/null
openssl pkcs8 -topk8 -nocrypt -in "$work/stable.pem" -out "$work/stable-pkcs8.pem"
openssl genrsa -out "$work/small.pem" 1024 2> /dev/null
openssl genpkey -algorithm EC -pkeyopt ec_paramgen_curve:P-256 -out "$work/ec.pem" 2> /dev/null
pub() { openssl pkey -in "$1" -pubout 2> /dev/null; }
want_pub="$(pub "$work/stable.pem")"
b64() { base64 < "$1" | tr -d '\n'; }
secret="$(b64 "$work/stable.pem")"
k=0

# key_run <valeur du secret, ou « - » pour un secret absent> : sortie dans $work/out.txt,
# dossier $kdir.
key_run() {
  k=$((k + 1))
  kdir="$work/key$k"
  if [ "$1" = "-" ]; then
    env -u APPLE_DIST_KEY_BASE64 "${BASH:-bash}" "$here/ios-signing.sh" key "$kdir" > "$work/out.txt" 2>&1
  else
    APPLE_DIST_KEY_BASE64="$1" "${BASH:-bash}" "$here/ios-signing.sh" key "$kdir" > "$work/out.txt" 2>&1
  fi
}

key_ok() { echo "ok   key : $1"; }
key_ko() {
  echo "FAIL key : $1"
  sed 's/^/     | /' "$work/out.txt"
  failures=$((failures + 1))
}

# La demande (CSR) porte-t-elle la clé publique attendue ?
csr_pub() { openssl req -in "$kdir/dist.csr" -noout -pubkey 2> /dev/null; }

# key_accepts <nom> <valeur> : clé stable acceptée, la même clé publique dans dist.key et la CSR.
key_accepts() {
  if key_run "$2" && [ "$(cat "$kdir/key-mode.txt")" = stable ] &&
    [ "$(pub "$kdir/dist.key")" = "$want_pub" ] && [ "$(csr_pub)" = "$want_pub" ] &&
    grep -q "Clé du certificat : stable" "$work/out.txt"; then
    key_ok "$1"
  else
    key_ko "$1"
  fi
}

# key_rejects <nom> <valeur> <texte attendu> : arrêt, message clair, ni clé ni CSR, aucune fuite.
key_rejects() {
  local leak
  if key_run "$2"; then
    key_ko "$1 (accepté à tort)"
    return
  fi
  leak="$(printf '%s' "$2" | tr -d '[:space:]' | cut -c 20-60)"
  if ! grep -q "::error::APPLE_DIST_KEY_BASE64" "$work/out.txt" || ! grep -q -- "$3" "$work/out.txt"; then
    key_ko "$1 (message inattendu)"
  elif [ -e "$kdir/dist.key" ] || [ -e "$kdir/dist.csr" ]; then
    key_ko "$1 (une clé ou une demande est restée)"
  elif [ -n "$leak" ] && grep -q -F -- "$leak" "$work/out.txt"; then
    key_ko "$1 (valeur affichée)"
  elif grep -q -e "PRIVATE KEY" -e "MII" "$work/out.txt"; then
    key_ko "$1 (morceau de clé affiché)"
  else
    key_ok "$1"
  fi
}

# Secret absent : comme avant, une clé neuve à chaque exécution.
if key_run - && [ "$(cat "$kdir/key-mode.txt")" = run ] && openssl rsa -in "$kdir/dist.key" -noout -check > /dev/null 2>&1 &&
  [ "$(csr_pub)" = "$(pub "$kdir/dist.key")" ] && grep -q "neuve, pour cette exécution" "$work/out.txt"; then
  first="$(pub "$kdir/dist.key")"
  if key_run - && [ "$(pub "$kdir/dist.key")" != "$first" ]; then
    key_ok "secret absent : clé neuve, différente à chaque exécution"
  else
    key_ko "secret absent : clé neuve, différente à chaque exécution"
  fi
else
  key_ko "secret absent : clé neuve, différente à chaque exécution"
fi
if key_run "" && [ "$(cat "$kdir/key-mode.txt")" = run ]; then key_ok "secret vide = absent"; else key_ko "secret vide = absent"; fi

key_accepts "PEM RSA 2048 (genrsa) en base64, une ligne" "$secret"
key_accepts "PEM PKCS#8 en base64" "$(b64 "$work/stable-pkcs8.pem")"
key_accepts "base64 coupé en lignes de 64" "$(printf '%s' "$secret" | fold -w 64)"
key_accepts "espaces, retours Windows et lignes vides autour" "$(printf '\n  %s  \r\n\n' "$secret")"
key_accepts "espaces au milieu" "$(printf '%s' "$secret" | sed 's/.\{100\}/& /g')"

perm="$(stat -c '%a' "$kdir/dist.key" 2> /dev/null || stat -f '%Lp' "$kdir/dist.key")"
if [ "$perm" = "600" ]; then key_ok "fichier de clé en 0600"; else key_ko "fichier de clé en 0600 (trouvé : $perm)"; fi

key_rejects "pas du base64" "ceci n'est pas du base64, c'est une phrase collée par erreur !" "ce n'est pas du base64"
key_rejects "base64 tronqué" "${secret%?????}" "ce n'est pas du base64"
key_rejects "clé en clair au lieu du base64" "$(cat "$work/stable.pem")" "on dirait la clé en clair"
key_rejects "base64 d'un texte qui n'est pas une clé" "$(printf 'bonjour, ceci est un texte et pas une clé privée\n' | base64 | tr -d '\n')" "n'est pas une clé privée PEM"
key_rejects "base64 d'une clé coupée" "$(sed '5,$d' "$work/stable.pem" | base64 | tr -d '\n')" "n'est pas une clé privée PEM"
key_rejects "base64 d'une clé abîmée (lisible, mais fausse)" "$(sed '3s/./A/g' "$work/stable.pem" | base64 | tr -d '\n')" "la clé RSA est abîmée"
key_rejects "clé EC au lieu de RSA" "$(b64 "$work/ec.pem")" "n'est pas une clé RSA"
key_rejects "clé RSA 1024" "$(b64 "$work/small.pem")" "clé RSA de 1024 bits, 2048 attendus"

# Aucune ligne de la clé dans la sortie, même acceptée.
key_run "$secret"
if grep -q -F -- "$(sed -n 2p "$work/stable.pem")" "$work/out.txt" || grep -q -F -- "${secret:20:40}" "$work/out.txt"; then
  key_ko "aucune fuite (clé acceptée)"
else
  key_ok "aucune fuite (clé acceptée)"
fi

echo
if [ "$failures" -ne 0 ]; then
  echo "ios-signing.sh (check, key) : ${failures} essai(s) en échec"
  exit 1
fi
echo "ios-signing.sh (check, key) : tous les essais passent"
