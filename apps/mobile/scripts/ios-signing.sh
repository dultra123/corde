#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Signature de distribution pour la compilation TestFlight (sur le Mac de la CI).
#
#   ios-signing.sh setup <dossier de travail>      avant `xcodebuild archive`
#   ios-signing.sh check <App.app>                 après l'archive : la signature est-elle la bonne ?
#   ios-signing.sh teardown <dossier de travail>   toujours, à la fin
#   ios-signing.sh key <dossier de travail>        seulement l'étape 1 (pour les essais)
#
# setup, avec ASC_KEY_PATH, ASC_KEY_ID, ASC_ISSUER_ID, ASC_TEAM_ID (donnés par asc-key.sh),
# BUNDLE_ID (l'application), WIDGETS_BUNDLE_ID (son extension de widgets) et, facultatif,
# APPLE_DIST_KEY_BASE64 (la clé stable du certificat) :
#   1. la clé privée du certificat et sa demande (CSR), sur cette machine : sans
#      APPLE_DIST_KEY_BASE64, une clé neuve pour cette exécution ; avec, la clé du secret
#      (PEM RSA 2048 en base64), vérifiée sans jamais être affichée ;
#   2. obtient d'Apple un certificat « Apple Distribution » et un profil App Store par
#      identifiant (asc-api.mjs, qui crée l'identifiant de l'extension s'il manque et fait le
#      ménage de l'exécution précédente). Avec la clé stable, le certificat qui la porte est
#      réutilisé tant qu'il reste plus de 30 jours avant son expiration : plus de révocation, donc
#      plus d'e-mail d'Apple, à chaque build. Sans elle, le certificat de l'exécution précédente
#      est révoqué et un neuf est créé, comme avant ;
#   3. range clé et certificat dans un trousseau temporaire, ouvert au seul outil de signature ;
#   4. installe les profils et passe les deux cibles en signature manuelle (xcode-signing.mjs) ;
#      donne SIGN_PROFILE_NAME et SIGN_PROFILE_NAME_WIDGETS à l'étape d'export.
#
# check contrôle l'application, ses frameworks et son extension (PlugIns/*.appex) : signature
# « Apple Distribution », profil embarqué, et droit `keychain-access-groups` (trousseau partagé
# avec les widgets) dans l'application et l'extension — sans afficher sa valeur, qui contient
# l'identifiant d'équipe.
#
# La clé privée n'est écrite que dans le dossier de travail (0600), puis dans le trousseau
# temporaire, et disparaît avec eux. Sans le secret, chaque exécution a son propre certificat ;
# avec, la clé vient du secret et n'est jamais affichée. Rien de personnel n'est affiché : le nom
# du certificat contient le nom du titulaire du compte, on ne montre donc que des comptes et des
# « oui / non ».
# Compatible avec le bash 3.2 de macOS.

set -euo pipefail
umask 077

here="$(cd "$(dirname "$0")" && pwd)"
PROFILE_PREFIX="Corde CI "
key_mode=""

fail() {
  echo "::error::$*"
  exit 1
}

pick_ssl() {
  if [ -x /usr/bin/openssl ]; then echo /usr/bin/openssl; else echo openssl; fi
}

# Décrit une valeur sans l'afficher (comme asc-key.sh).
shape() {
  local v="$1" lines
  if [ -z "$v" ]; then
    printf 'vide'
    return
  fi
  lines=$(printf '%s\n' "$v" | wc -l | tr -d '[:space:]')
  printf '%s caractères sur %s ligne(s)' "${#v}" "$lines"
}

# La clé privée du certificat et sa demande (CSR) : <dossier>/dist.key et <dossier>/dist.csr.
#   - APPLE_DIST_KEY_BASE64 absent ou vide : une clé RSA 2048 neuve, pour cette exécution
#     (key_mode=run, comportement d'avant) ;
#   - sinon : la clé du secret (key_mode=stable). Attendu : le fichier PEM d'une clé privée RSA
#     2048, encodé en base64 ; les espaces et retours à la ligne collés par erreur sont retirés.
#     Une valeur qui ne convient pas arrête tout, avec sa longueur et sa forme seulement.
make_key() {
  local work="$1" ssl="$2" raw b64 what modulus bits check
  local hint="Attendu : le fichier .pem d'une clé privée RSA 2048, encodé en base64 sur une ligne (docs/IOS.md, « Le cinquième secret »)."
  raw="${APPLE_DIST_KEY_BASE64:-}"
  rm -f "$work/dist.key" "$work/dist.csr"
  if [ -z "$raw" ]; then
    key_mode=run
    "$ssl" genrsa -out "$work/dist.key" 2048 2> /dev/null
    echo "Clé du certificat : neuve, pour cette exécution (secret APPLE_DIST_KEY_BASE64 absent)."
  else
    key_mode=stable
    b64="$(printf '%s' "$raw" | tr -d '[:space:]')"
    if ! [[ "$b64" =~ ^[A-Za-z0-9+/]+={0,2}$ ]] || [ $((${#b64} % 4)) -ne 0 ] ||
      ! printf '%s' "$b64" | base64 --decode > "$work/dist.key" 2> /dev/null; then
      rm -f "$work/dist.key"
      case "$raw" in
        *-----BEGIN*) what="on dirait la clé en clair (lignes BEGIN / END) : il faut le fichier encodé en base64" ;;
        *) what="ce n'est pas du base64" ;;
      esac
      fail "APPLE_DIST_KEY_BASE64 : ${what} (reçu : $(shape "$raw")). ${hint}"
    fi
    if ! grep -q -- "-----BEGIN" "$work/dist.key" ||
      ! "$ssl" pkey -in "$work/dist.key" -passin pass: -noout > /dev/null 2>&1; then
      rm -f "$work/dist.key"
      fail "APPLE_DIST_KEY_BASE64 : le texte décodé n'est pas une clé privée PEM lisible (sans mot de passe) (reçu : $(shape "$raw")). ${hint}"
    fi
    if ! modulus="$("$ssl" rsa -in "$work/dist.key" -passin pass: -noout -modulus 2> /dev/null)"; then
      rm -f "$work/dist.key"
      fail "APPLE_DIST_KEY_BASE64 : la clé n'est pas une clé RSA. ${hint}"
    fi
    modulus="${modulus#*=}"
    bits=$((${#modulus} * 4))
    if [ "$bits" -ne 2048 ]; then
      rm -f "$work/dist.key"
      fail "APPLE_DIST_KEY_BASE64 : clé RSA de ${bits} bits, 2048 attendus. ${hint}"
    fi
    # (OpenSSL 3 répond 0 même quand la vérification échoue : seul son texte fait foi.)
    check="$("$ssl" rsa -in "$work/dist.key" -passin pass: -noout -check 2> /dev/null || true)"
    if [ "$check" != "RSA key ok" ]; then
      rm -f "$work/dist.key"
      fail "APPLE_DIST_KEY_BASE64 : la clé RSA est abîmée (sa vérification échoue : texte modifié ?). ${hint}"
    fi
    chmod 600 "$work/dist.key"
    echo "Clé du certificat : stable (secret APPLE_DIST_KEY_BASE64, RSA 2048) ; le certificat qui la porte est réutilisé s'il est encore valable."
  fi
  "$ssl" req -new -key "$work/dist.key" -subj "/CN=Corde CI" -out "$work/dist.csr"
}

# Nombre d'identités de signature « Apple Distribution » valides dans un trousseau.
count_identities() {
  security find-identity -v -p codesigning "$1" | awk '/Apple Distribution/ { n++ } END { print n + 0 }'
}

setup() {
  local work="$1" kc ssl pw profile_name profile_uuid project line dir ca id
  local -a legacy keychains sign_args stable_args
  : "${ASC_KEY_PATH:?}" "${ASC_KEY_ID:?}" "${ASC_ISSUER_ID:?}" "${ASC_TEAM_ID:?}" "${BUNDLE_ID:?}" "${WIDGETS_BUNDLE_ID:?}"
  project="${XCODE_PROJECT:-$here/../ios/App/App.xcodeproj/project.pbxproj}"
  kc="$work/corde-signing.keychain-db"
  mkdir -p "$work"
  chmod 700 "$work"

  # Le LibreSSL de macOS écrit un .p12 que `security import` sait lire ; un OpenSSL 3 a besoin
  # de -legacy pour cela.
  ssl="$(pick_ssl)"
  legacy=()
  case "$("$ssl" version)" in
    OpenSSL\ 3*) legacy=(-legacy) ;;
  esac

  make_key "$work" "$ssl"
  stable_args=()
  [ "$key_mode" != stable ] || stable_args=(--stable-key "$work/dist.key")

  node "$here/asc-api.mjs" prepare \
    --bundle-id "$BUNDLE_ID" \
    --bundle-id "$WIDGETS_BUNDLE_ID" \
    --csr "$work/dist.csr" \
    --out "$work" \
    --profile-name "${PROFILE_PREFIX}${GITHUB_RUN_NUMBER:-0}.${GITHUB_RUN_ATTEMPT:-1}" \
    --profile-prefix "$PROFILE_PREFIX" \
    ${stable_args[@]+"${stable_args[@]}"}

  for id in "$BUNDLE_ID" "$WIDGETS_BUNDLE_ID"; do
    profile_uuid="$(cat "$work/$id/profile-uuid.txt")"
    [[ "$profile_uuid" =~ ^[0-9A-Fa-f-]{36}$ ]] || fail "identifiant de profil inattendu ($id)"
  done

  # Trousseau temporaire : clé + certificat, utilisables par codesign sans fenêtre de confirmation.
  "$ssl" x509 -inform DER -in "$work/dist.cer" -out "$work/dist.pem"
  pw="$("$ssl" rand -hex 24)"
  P12_PASSWORD="$pw" "$ssl" pkcs12 -export ${legacy[@]+"${legacy[@]}"} \
    -inkey "$work/dist.key" -in "$work/dist.pem" -name "Corde CI" \
    -out "$work/dist.p12" -passout env:P12_PASSWORD
  security create-keychain -p "$pw" "$kc"
  security set-keychain-settings -lut 21600 "$kc"
  security unlock-keychain -p "$pw" "$kc"
  security import "$work/dist.p12" -k "$kc" -f pkcs12 -P "$pw" -T /usr/bin/codesign -T /usr/bin/security > /dev/null
  security set-key-partition-list -S apple-tool:,apple:,codesign: -s -k "$pw" "$kc" > /dev/null
  rm -f "$work/dist.key" "$work/dist.p12" "$work/dist.pem" "$work/dist.csr"

  keychains=("$kc")
  while IFS= read -r line; do
    line="${line#"${line%%[![:space:]]*}"}"
    line="${line#\"}"
    line="${line%\"}"
    [ -z "$line" ] || keychains+=("$line")
  done < <(security list-keychains -d user)
  security list-keychains -d user -s "${keychains[@]}"

  if [ "$(count_identities "$kc")" -eq 0 ]; then
    # L'autorité intermédiaire d'Apple manque peut-être sur cette machine : l'ajouter au trousseau.
    for ca in AppleWWDRCAG3 AppleWWDRCAG6 AppleWWDRCAG2 AppleWWDRCAG5 AppleWWDRCAG4; do
      if curl -fsSL --retry 2 --max-time 30 "https://www.apple.com/certificateauthority/$ca.cer" -o "$work/$ca.cer"; then
        security import "$work/$ca.cer" -k "$kc" > /dev/null 2>&1 || true
      fi
    done
  fi
  [ "$(count_identities "$kc")" -ge 1 ] ||
    fail "le certificat de distribution reçu d'Apple ne donne aucune identité de signature valide dans le trousseau"
  echo "Trousseau temporaire : 1 identité « Apple Distribution » prête."

  sign_args=(--team "$ASC_TEAM_ID")
  for id in "$BUNDLE_ID" "$WIDGETS_BUNDLE_ID"; do
    profile_name="$(cat "$work/$id/profile-name.txt")"
    profile_uuid="$(cat "$work/$id/profile-uuid.txt")"
    for dir in "$HOME/Library/MobileDevice/Provisioning Profiles" "$HOME/Library/Developer/Xcode/UserData/Provisioning Profiles"; do
      mkdir -p "$dir"
      cp "$work/$id/profile.mobileprovision" "$dir/$profile_uuid.mobileprovision"
    done
    sign_args+=(--bundle-id "$id" --profile "$profile_name")
  done
  echo "Profils installés : 2 (application et widgets)."

  node "$here/xcode-signing.mjs" "$project" "${sign_args[@]}"
  if command -v plutil > /dev/null 2>&1; then
    plutil -lint "$project" > /dev/null || fail "le projet Xcode modifié n'est plus lisible"
  fi

  if [ -n "${GITHUB_ENV:-}" ]; then
    echo "SIGN_PROFILE_NAME=$(cat "$work/$BUNDLE_ID/profile-name.txt")" >> "$GITHUB_ENV"
    echo "SIGN_PROFILE_NAME_WIDGETS=$(cat "$work/$WIDGETS_BUNDLE_ID/profile-name.txt")" >> "$GITHUB_ENV"
  fi
}

# Le droit `keychain-access-groups` d'un binaire signé contient-il le groupe partagé ?
# (Seulement oui / non : la valeur commence par l'identifiant d'équipe.)
has_shared_keychain() {
  local ents re
  ents="$(codesign -d --entitlements - --xml "$1" 2> /dev/null | tr -d '\n\t ' || true)"
  re='<key>keychain-access-groups</key><array>(<string>[^<]*</string>)*<string>[A-Z0-9]{10}\.io\.github\.dultra123\.corde\.shared</string>'
  [[ "$ents" =~ $re ]]
}

# Chaque binaire de l'application doit porter une signature « Apple Distribution » intacte.
check() {
  local app="$1" item info bad=0 name appex found=0
  [ -d "$app" ] || fail "application introuvable dans l'archive"
  for item in "$app" "$app"/Frameworks/*.framework "$app"/PlugIns/*.appex "$app"/PlugIns/*.appex/Frameworks/*.framework; do
    [ -e "$item" ] || continue
    name="$(basename "$item")"
    info="$(codesign -dvv "$item" 2>&1 || true)"
    if ! codesign --verify --strict "$item" > /dev/null 2>&1; then
      echo "✗ $name : signature invalide"
      bad=1
      continue
    fi
    case "$info" in
      *"Authority=Apple Distribution"*) echo "✓ $name : signature de distribution" ;;
      *"Signature=adhoc"*)
        echo "✗ $name : signature locale (ad hoc), pas de distribution"
        bad=1
        ;;
      *)
        echo "✗ $name : signée, mais pas par un certificat « Apple Distribution »"
        bad=1
        ;;
    esac
  done
  for appex in "$app"/PlugIns/*.appex; do
    [ -d "$appex" ] || continue
    found=$((found + 1))
  done
  if [ "$found" -ge 1 ]; then
    echo "✓ extension de widgets présente ($found)"
  else
    echo "✗ extension de widgets absente (PlugIns/*.appex)"
    bad=1
  fi
  for item in "$app" "$app"/PlugIns/*.appex; do
    [ -e "$item" ] || continue
    name="$(basename "$item")"
    if [ -f "$item/embedded.mobileprovision" ]; then
      echo "✓ $name : profil de provisionnement présent"
    else
      echo "✗ $name : profil de provisionnement absent"
      bad=1
    fi
    if has_shared_keychain "$item"; then
      echo "✓ $name : trousseau partagé (keychain-access-groups) dans les droits"
    else
      echo "✗ $name : droit keychain-access-groups absent ou sans le groupe partagé"
      bad=1
    fi
  done
  [ "$bad" -eq 0 ] || fail "l'application n'est pas signée pour la distribution : inutile de l'envoyer à Apple"
}

teardown() {
  local work="$1" uuid dir file
  security delete-keychain "$work/corde-signing.keychain-db" > /dev/null 2>&1 || true
  for file in "$work"/*/profile-uuid.txt; do
    [ -f "$file" ] || continue
    uuid="$(cat "$file")"
    [[ "$uuid" =~ ^[0-9A-Fa-f-]{36}$ ]] || continue
    for dir in "$HOME/Library/MobileDevice/Provisioning Profiles" "$HOME/Library/Developer/Xcode/UserData/Provisioning Profiles"; do
      rm -f "$dir/$uuid.mobileprovision"
    done
  done
  rm -rf "$work"
}

# Seulement la clé et la demande (CSR), sans Apple ni trousseau : sert aux essais
# (ios-signing.test.sh), sous Linux comme sous macOS.
key_only() {
  local work="$1"
  mkdir -p "$work"
  chmod 700 "$work"
  make_key "$work" "$(pick_ssl)"
  echo "$key_mode" > "$work/key-mode.txt"
}

case "${1:-}" in
  setup) setup "${2:?usage : ios-signing.sh setup <dossier de travail>}" ;;
  key) key_only "${2:?usage : ios-signing.sh key <dossier de travail>}" ;;
  check) check "${2:?usage : ios-signing.sh check <App.app>}" ;;
  teardown) teardown "${2:?usage : ios-signing.sh teardown <dossier de travail>}" ;;
  *)
    echo "usage : ios-signing.sh setup|key|check|teardown <chemin>" >&2
    exit 2
    ;;
esac
