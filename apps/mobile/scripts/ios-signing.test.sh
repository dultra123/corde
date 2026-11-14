#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Essais de `ios-signing.sh check` (contrôle de l'archive avant l'envoi à Apple) avec un faux
# `codesign` : application, frameworks et extension de widgets fabriqués ici, signature et droits
# simulés. Vérifie aussi que l'identifiant d'équipe n'est jamais affiché.
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
		<string>$1</string>
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
  ents "${TEAM}.io.github.dultra123.corde.shared" io.github.dultra123.corde > "$app/.ents"
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

if [ "$failures" -ne 0 ]; then
  echo "ios-signing.sh check : ${failures} essai(s) en échec"
  exit 1
fi
echo "ios-signing.sh check : tous les essais passent"
