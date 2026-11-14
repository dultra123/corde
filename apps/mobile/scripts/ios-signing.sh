#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Signature de distribution pour la compilation TestFlight (sur le Mac de la CI).
#
#   ios-signing.sh setup <dossier de travail>      avant `xcodebuild archive`
#   ios-signing.sh check <App.app>                 après l'archive : la signature est-elle la bonne ?
#   ios-signing.sh teardown <dossier de travail>   toujours, à la fin
#
# setup, avec ASC_KEY_PATH, ASC_KEY_ID, ASC_ISSUER_ID, ASC_TEAM_ID (donnés par asc-key.sh) et
# BUNDLE_ID :
#   1. crée une clé privée et une demande de certificat, sur cette machine ;
#   2. obtient d'Apple un certificat « Apple Distribution » et un profil App Store (asc-api.mjs,
#      qui révoque d'abord le certificat de l'exécution précédente) ;
#   3. range clé et certificat dans un trousseau temporaire, ouvert au seul outil de signature ;
#   4. installe le profil et passe la cible de l'application en signature manuelle
#      (xcode-signing.mjs) ; donne SIGN_PROFILE_NAME à l'étape d'export.
#
# La clé privée ne quitte jamais la machine et disparaît avec elle : chaque exécution a son propre
# certificat. Rien de personnel n'est affiché : le nom du certificat contient le nom du titulaire
# du compte, on ne montre donc que des comptes et des « oui / non ».
# Compatible avec le bash 3.2 de macOS.

set -euo pipefail
umask 077

here="$(cd "$(dirname "$0")" && pwd)"
PROFILE_PREFIX="Corde CI "

fail() {
  echo "::error::$*"
  exit 1
}

# Nombre d'identités de signature « Apple Distribution » valides dans un trousseau.
count_identities() {
  security find-identity -v -p codesigning "$1" | awk '/Apple Distribution/ { n++ } END { print n + 0 }'
}

setup() {
  local work="$1" kc ssl pw profile_name profile_uuid project line dir ca
  local -a legacy keychains
  : "${ASC_KEY_PATH:?}" "${ASC_KEY_ID:?}" "${ASC_ISSUER_ID:?}" "${ASC_TEAM_ID:?}" "${BUNDLE_ID:?}"
  project="${XCODE_PROJECT:-$here/../ios/App/App.xcodeproj/project.pbxproj}"
  kc="$work/corde-signing.keychain-db"
  mkdir -p "$work"
  chmod 700 "$work"

  # Le LibreSSL de macOS écrit un .p12 que `security import` sait lire ; un OpenSSL 3 a besoin
  # de -legacy pour cela.
  ssl=/usr/bin/openssl
  [ -x "$ssl" ] || ssl=openssl
  legacy=()
  case "$("$ssl" version)" in
    OpenSSL\ 3*) legacy=(-legacy) ;;
  esac

  "$ssl" genrsa -out "$work/dist.key" 2048 2> /dev/null
  "$ssl" req -new -key "$work/dist.key" -subj "/CN=Corde CI" -out "$work/dist.csr"

  node "$here/asc-api.mjs" prepare \
    --bundle-id "$BUNDLE_ID" \
    --csr "$work/dist.csr" \
    --out "$work" \
    --profile-name "${PROFILE_PREFIX}${GITHUB_RUN_NUMBER:-0}.${GITHUB_RUN_ATTEMPT:-1}" \
    --profile-prefix "$PROFILE_PREFIX"

  profile_name="$(cat "$work/profile-name.txt")"
  profile_uuid="$(cat "$work/profile-uuid.txt")"
  [[ "$profile_uuid" =~ ^[0-9A-Fa-f-]{36}$ ]] || fail "identifiant de profil inattendu"

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

  for dir in "$HOME/Library/MobileDevice/Provisioning Profiles" "$HOME/Library/Developer/Xcode/UserData/Provisioning Profiles"; do
    mkdir -p "$dir"
    cp "$work/profile.mobileprovision" "$dir/$profile_uuid.mobileprovision"
  done

  node "$here/xcode-signing.mjs" "$project" --bundle-id "$BUNDLE_ID" --team "$ASC_TEAM_ID" --profile "$profile_name"
  if command -v plutil > /dev/null 2>&1; then
    plutil -lint "$project" > /dev/null || fail "le projet Xcode modifié n'est plus lisible"
  fi

  if [ -n "${GITHUB_ENV:-}" ]; then
    echo "SIGN_PROFILE_NAME=$profile_name" >> "$GITHUB_ENV"
  fi
}

# Chaque binaire de l'application doit porter une signature « Apple Distribution » intacte.
check() {
  local app="$1" item info bad=0 name
  [ -d "$app" ] || fail "application introuvable dans l'archive"
  for item in "$app" "$app"/Frameworks/*.framework; do
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
  if [ -f "$app/embedded.mobileprovision" ]; then
    echo "✓ profil de provisionnement présent"
  else
    echo "✗ profil de provisionnement absent"
    bad=1
  fi
  [ "$bad" -eq 0 ] || fail "l'application n'est pas signée pour la distribution : inutile de l'envoyer à Apple"
}

teardown() {
  local work="$1" uuid dir
  security delete-keychain "$work/corde-signing.keychain-db" > /dev/null 2>&1 || true
  if [ -f "$work/profile-uuid.txt" ]; then
    uuid="$(cat "$work/profile-uuid.txt")"
    for dir in "$HOME/Library/MobileDevice/Provisioning Profiles" "$HOME/Library/Developer/Xcode/UserData/Provisioning Profiles"; do
      rm -f "$dir/$uuid.mobileprovision"
    done
  fi
  rm -rf "$work"
}

case "${1:-}" in
  setup) setup "${2:?usage : ios-signing.sh setup <dossier de travail>}" ;;
  check) check "${2:?usage : ios-signing.sh check <App.app>}" ;;
  teardown) teardown "${2:?usage : ios-signing.sh teardown <dossier de travail>}" ;;
  *)
    echo "usage : ios-signing.sh setup|check|teardown <chemin>" >&2
    exit 2
    ;;
esac
