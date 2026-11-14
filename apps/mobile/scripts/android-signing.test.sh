#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Essais de `android-signing.sh setup / throwaway / teardown` avec des magasins de clés fabriqués
# ici (keytool) : bon magasin (aussi collé avec des retours à la ligne), mauvais mot de passe,
# mauvais nom de clé, base64 abîmé, magasin vide. Vérifie aussi que ni le mot de passe ni le nom
# du certificat ne sont jamais affichés. (`check` est essayé par la CI sur un vrai APK.)
#   bash apps/mobile/scripts/android-signing.test.sh

set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
failures=0
PASS="essai-$(od -An -N8 -tx1 /dev/urandom | tr -d ' \n')"
NAME="Camille Exemple"

store() {
  # store <fichier> <alias> : un PKCS12 avec une clé RSA nommée <alias>, au nom de $NAME.
  ANDROID_KEYSTORE_PASSWORD="$PASS" keytool -genkeypair -storetype PKCS12 -keystore "$1" \
    -storepass:env ANDROID_KEYSTORE_PASSWORD -keypass:env ANDROID_KEYSTORE_PASSWORD \
    -alias "$2" -keyalg RSA -keysize 2048 -validity 2 -dname "CN=${NAME}" > /dev/null 2>&1
}

# run <nom> <attendu : ok|fail> <motif attendu dans la sortie> -- commande…
run() {
  local name="$1" want="$2" pattern="$3"
  shift 4
  local out rc=0
  out="$("$@" 2>&1)" || rc=$?
  local got=ok
  [ "$rc" -eq 0 ] || got=fail
  if [ "$got" != "$want" ]; then
    echo "✗ ${name} : attendu ${want}, obtenu ${got}"
    printf '%s\n' "$out" | sed 's/^/    /'
    failures=$((failures + 1))
    return
  fi
  if [ -n "$pattern" ] && ! printf '%s\n' "$out" | grep -E -q -- "$pattern"; then
    echo "✗ ${name} : « ${pattern} » absent de la sortie"
    printf '%s\n' "$out" | sed 's/^/    /'
    failures=$((failures + 1))
    return
  fi
  case "$out" in
    *"$PASS"* | *"$NAME"*)
      echo "✗ ${name} : la sortie contient le mot de passe ou le nom du certificat"
      failures=$((failures + 1))
      return
      ;;
  esac
  echo "✓ ${name}"
}

store "$work/good.p12" corde
store "$work/other.p12" release
good="$(base64 < "$work/good.p12" | tr -d '\n')"
wrapped="$(base64 < "$work/good.p12" | fold -w 64 | sed 's/$/\r/')"
other="$(base64 < "$work/other.p12" | tr -d '\n')"

export GITHUB_ENV="$work/env"
: > "$GITHUB_ENV"

run "bon magasin" ok "PKCS12, [0-9]+ octets, clé « corde », certificat SHA-256 [0-9a-f]{64}" -- \
  env ANDROID_KEYSTORE_BASE64="$good" ANDROID_KEYSTORE_PASSWORD="$PASS" \
  bash "$here/android-signing.sh" setup "$work/s1"
if [ "$(stat -c %a "$work/s1/corde.p12" 2> /dev/null || stat -f %Lp "$work/s1/corde.p12")" = "600" ] &&
  [ "$(stat -c %a "$work/s1" 2> /dev/null || stat -f %Lp "$work/s1")" = "700" ]; then
  echo "✓ magasin en 0600 dans un dossier 0700"
else
  echo "✗ droits du magasin ou du dossier"
  failures=$((failures + 1))
fi
if grep -q "^CORDE_KEYSTORE_FILE=$work/s1/corde.p12$" "$GITHUB_ENV" &&
  grep -E -q "^CORDE_CERT_SHA256=[0-9a-f]{64}$" "$GITHUB_ENV" &&
  ! grep -q "$PASS" "$GITHUB_ENV"; then
  echo "✓ variables pour les étapes suivantes (sans le mot de passe)"
else
  echo "✗ variables pour les étapes suivantes"
  failures=$((failures + 1))
fi

run "magasin collé avec des retours à la ligne Windows" ok "clé « corde »" -- \
  env ANDROID_KEYSTORE_BASE64="$wrapped" ANDROID_KEYSTORE_PASSWORD="$PASS" \
  bash "$here/android-signing.sh" setup "$work/s2"
run "mauvais mot de passe" fail "ne s'ouvre pas avec ANDROID_KEYSTORE_PASSWORD" -- \
  env ANDROID_KEYSTORE_BASE64="$good" ANDROID_KEYSTORE_PASSWORD="faux-${PASS}x" \
  bash "$here/android-signing.sh" setup "$work/s3"
run "clé qui ne s'appelle pas corde" fail "pas de clé nommée « corde »" -- \
  env ANDROID_KEYSTORE_BASE64="$other" ANDROID_KEYSTORE_PASSWORD="$PASS" \
  bash "$here/android-signing.sh" setup "$work/s4"
run "pas du base64" fail "n'est pas du base64" -- \
  env ANDROID_KEYSTORE_BASE64="pas du base64 !" ANDROID_KEYSTORE_PASSWORD="$PASS" \
  bash "$here/android-signing.sh" setup "$work/s5"
run "un autre fichier en base64" fail "trop petit|ne s'ouvre pas" -- \
  env ANDROID_KEYSTORE_BASE64="$(printf 'bonjour' | base64)" ANDROID_KEYSTORE_PASSWORD="$PASS" \
  bash "$here/android-signing.sh" setup "$work/s6"
run "secret vide" fail "ANDROID_KEYSTORE_BASE64 est vide" -- \
  env ANDROID_KEYSTORE_BASE64="" ANDROID_KEYSTORE_PASSWORD="$PASS" \
  bash "$here/android-signing.sh" setup "$work/s7"

# Clé jetable : elle passe ensuite par `setup` comme les vrais secrets.
: > "$GITHUB_ENV"
run "clé jetable" ok "Clé jetable créée" -- bash "$here/android-signing.sh" throwaway "$work/t"
tb64="$(sed -n 's/^ANDROID_KEYSTORE_BASE64=//p' "$GITHUB_ENV")"
tpass="$(sed -n 's/^ANDROID_KEYSTORE_PASSWORD=//p' "$GITHUB_ENV")"
if [ -n "$tb64" ] && [ "${#tpass}" -ge 32 ] && [ ! -e "$work/t/throwaway.p12" ]; then
  echo "✓ clé jetable donnée sous les noms des secrets, fichier effacé"
else
  echo "✗ clé jetable : variables ou fichier"
  failures=$((failures + 1))
fi
PASS="$tpass"
run "clé jetable ouverte par setup" ok "clé « corde »" -- \
  env ANDROID_KEYSTORE_BASE64="$tb64" ANDROID_KEYSTORE_PASSWORD="$tpass" \
  bash "$here/android-signing.sh" setup "$work/t2"

bash "$here/android-signing.sh" teardown "$work/s1"
if [ ! -e "$work/s1" ]; then
  echo "✓ dossier effacé"
else
  echo "✗ dossier non effacé"
  failures=$((failures + 1))
fi

if [ "$failures" -gt 0 ]; then
  echo "${failures} essai(s) en échec"
  exit 1
fi
echo "android-signing.sh : tous les essais passent"
