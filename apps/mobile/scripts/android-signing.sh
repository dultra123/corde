#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Signature de l'application Android dans la CI (.github/workflows/android.yml, docs/ANDROID.md).
#
#   ANDROID_KEYSTORE_BASE64=… ANDROID_KEYSTORE_PASSWORD=… android-signing.sh setup <dossier>
#       Écrit le magasin de clés (PKCS12, clé « corde ») dans <dossier>/corde.p12 (dossier 0700,
#       fichier 0600), vérifie qu'il s'ouvre avec le mot de passe et qu'il contient la clé, et
#       donne aux étapes suivantes (GITHUB_ENV) CORDE_KEYSTORE_FILE (lu par app/build.gradle) et
#       CORDE_CERT_SHA256 (l'empreinte attendue). Le mot de passe n'est jamais écrit ailleurs :
#       l'étape de compilation le reçoit des secrets.
#   android-signing.sh throwaway <dossier>
#       Fabrique une clé jetable (PKCS12, clé « corde », valable 2 jours, mot de passe tiré au
#       sort) et la donne aux étapes suivantes sous les mêmes noms que les secrets
#       (ANDROID_KEYSTORE_BASE64, ANDROID_KEYSTORE_PASSWORD, masqués dans le journal) : le chemin
#       de signature s'essaie sur une branche sans aucun secret.
#   android-signing.sh check <apk> [<aab>]
#       Vérifie la signature de l'APK (apksigner : schémas v2 / v3, un seul signataire) et, s'il
#       est donné, du bundle (jarsigner), avec la même clé, celle de CORDE_CERT_SHA256 si elle est
#       connue. N'affiche que l'empreinte SHA-256 du certificat.
#   android-signing.sh teardown <dossier>
#       Efface le dossier (toujours, même après un échec).
#
# Le magasin est accepté encodé en base64 avec des retours à la ligne ou des espaces (copier-coller).
# Rien d'autre que des longueurs, des formes et l'empreinte n'est affiché : le certificat porte un
# nom (« Owner: CN=… ») qui ne doit pas finir dans le journal public.

set -euo pipefail
umask 077

die() {
  echo "::error::$*" >&2
  exit 1
}

# Empreinte « AB:CD:… » ou « abcd… » → « abcd… » (minuscules, sans deux-points).
norm() {
  tr -d ': \r\n' | tr 'A-F' 'a-f'
}

# keytool, sans la ligne « Picked up JAVA_TOOL_OPTIONS » ni rien d'autre que ce qu'on extrait.
cert_sha256_of_store() {
  local store="$1" out
  out="$(keytool -list -v -storetype PKCS12 -keystore "$store" \
    -storepass:env ANDROID_KEYSTORE_PASSWORD -alias corde 2> /dev/null)" || return 1
  case "$out" in
    *"PrivateKeyEntry"*) ;;
    *) return 2 ;;
  esac
  printf '%s\n' "$out" | sed -n 's/^[[:space:]]*SHA256:[[:space:]]*//p' | head -n 1 | norm
}

env_out() {
  if [ -n "${GITHUB_ENV:-}" ]; then
    printf '%s\n' "$1" >> "$GITHUB_ENV"
  fi
}

setup() {
  local dir="$1" store b64 size fp
  [ -n "${ANDROID_KEYSTORE_BASE64:-}" ] || die "ANDROID_KEYSTORE_BASE64 est vide"
  [ -n "${ANDROID_KEYSTORE_PASSWORD:-}" ] || die "ANDROID_KEYSTORE_PASSWORD est vide"
  mkdir -p "$dir"
  chmod 700 "$dir"
  store="$dir/corde.p12"
  b64="$(printf '%s' "$ANDROID_KEYSTORE_BASE64" | tr -d ' \t\r\n')"
  case "$b64" in
    *[!A-Za-z0-9+/=]*) die "ANDROID_KEYSTORE_BASE64 n'est pas du base64 (${#b64} caractères) : encoder le fichier .p12 entier (docs/ANDROID.md)" ;;
  esac
  printf '%s' "$b64" | base64 -d > "$store" 2> /dev/null ||
    die "ANDROID_KEYSTORE_BASE64 ne se décode pas (${#b64} caractères)"
  chmod 600 "$store"
  size="$(wc -c < "$store" | tr -d ' ')"
  [ "$size" -gt 100 ] || die "magasin de clés trop petit après décodage (${size} octets)"
  set +e
  fp="$(cert_sha256_of_store "$store")"
  local rc=$?
  set -e
  case "$rc" in
    0) ;;
    2) die "le magasin s'ouvre, mais l'entrée « corde » n'est pas une clé privée" ;;
    *) die "le magasin (${size} octets) ne s'ouvre pas avec ANDROID_KEYSTORE_PASSWORD, n'est pas un PKCS12, ou n'a pas de clé nommée « corde » (docs/ANDROID.md)" ;;
  esac
  [ -n "$fp" ] || die "empreinte du certificat introuvable"
  echo "Magasin de clés : PKCS12, ${size} octets, clé « corde », certificat SHA-256 ${fp}"
  env_out "CORDE_KEYSTORE_FILE=${store}"
  env_out "CORDE_CERT_SHA256=${fp}"
}

throwaway() {
  local dir="$1" pass store b64
  mkdir -p "$dir"
  chmod 700 "$dir"
  pass="$(od -An -N24 -tx1 /dev/urandom | tr -d ' \n')"
  store="$dir/throwaway.p12"
  if [ -n "${GITHUB_ENV:-}" ]; then echo "::add-mask::${pass}"; fi
  ANDROID_KEYSTORE_PASSWORD="$pass" keytool -genkeypair -storetype PKCS12 -keystore "$store" \
    -storepass:env ANDROID_KEYSTORE_PASSWORD -keypass:env ANDROID_KEYSTORE_PASSWORD \
    -alias corde -keyalg RSA -keysize 3072 -validity 2 \
    -dname "CN=Corde CI throwaway" > /dev/null 2>&1 || die "keytool n'a pas créé la clé jetable"
  b64="$(base64 < "$store" | tr -d '\n')"
  # Masqué lui aussi, comme le serait le vrai secret (le journal affiche l'environnement des étapes).
  if [ -n "${GITHUB_ENV:-}" ]; then echo "::add-mask::${b64}"; fi
  env_out "ANDROID_KEYSTORE_BASE64=${b64}"
  env_out "ANDROID_KEYSTORE_PASSWORD=${pass}"
  rm -f "$store"
  echo "Clé jetable créée (RSA 3072, 2 jours) : elle ne signe que l'essai de cette exécution."
}

apksigner_bin() {
  local sdk="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-}}" bin
  [ -n "$sdk" ] || die "ANDROID_HOME n'est pas défini"
  bin="$(ls -d "$sdk"/build-tools/*/apksigner 2> /dev/null | sort -V | tail -n 1)"
  [ -n "$bin" ] || die "apksigner introuvable dans ${sdk}/build-tools"
  printf '%s' "$bin"
}

check() {
  local apk="$1" aab="${2:-}" out fp signers aab_fp
  [ -f "$apk" ] || die "APK introuvable : $apk"
  out="$("$(apksigner_bin)" verify --verbose --print-certs "$apk" 2>&1)" ||
    die "apksigner refuse la signature de $(basename "$apk")"
  # Les schémas utilisés, puis l'empreinte seulement (jamais le nom du certificat).
  printf '%s\n' "$out" | grep -E '^Verified using v[0-9]' || true
  case "$out" in
    *"Verified using v2 scheme (APK Signature Scheme v2): true"* | *"Verified using v3 scheme (APK Signature Scheme v3): true"*) ;;
    *) die "ni le schéma v2 ni le v3 : $(basename "$apk")" ;;
  esac
  signers="$(printf '%s\n' "$out" | sed -n 's/^Number of signers: //p')"
  [ "$signers" = "1" ] || die "nombre de signataires inattendu : ${signers:-?}"
  # « Signer #1 certificate SHA-256 digest: … » (la forme varie selon la version d'apksigner).
  fp="$(printf '%s\n' "$out" | grep -E -i 'certificate SHA-256 digest:' | head -n 1 |
    sed 's/.*[Dd]igest:[[:space:]]*//' | norm)"
  if [ -z "$fp" ]; then
    # Pour comprendre, les lignes des signataires sans le nom du certificat (« DN »).
    printf '%s\n' "$out" | grep -i -E 'signer|digest' | grep -v -i 'DN:' | head -n 20 || true
    die "empreinte du certificat de l'APK introuvable"
  fi
  if [ -n "${CORDE_CERT_SHA256:-}" ] && [ "$fp" != "$CORDE_CERT_SHA256" ]; then
    die "l'APK n'est pas signé par la clé du magasin (empreinte ${fp})"
  fi
  echo "APK signé : $(basename "$apk"), certificat SHA-256 ${fp}"
  if [ -n "$aab" ]; then
    [ -f "$aab" ] || die "bundle introuvable : $aab"
    # jarsigner -verify affiche le nom du signataire : seule la conclusion est gardée.
    out="$(jarsigner -verify "$aab" 2>&1)" || die "jarsigner refuse la signature de $(basename "$aab")"
    case "$out" in
      *"jar verified."*) ;;
      *) die "bundle non signé : $(basename "$aab")" ;;
    esac
    aab_fp="$(keytool -printcert -jarfile "$aab" 2> /dev/null |
      sed -n 's/^[[:space:]]*SHA256:[[:space:]]*//p' | head -n 1 | norm)"
    [ "$aab_fp" = "$fp" ] || die "le bundle n'est pas signé par la même clé que l'APK"
    echo "Bundle signé : $(basename "$aab"), même certificat"
  fi
}

cmd="${1:-}"
case "$cmd" in
  setup) setup "${2:?usage : android-signing.sh setup <dossier>}" ;;
  throwaway) throwaway "${2:?usage : android-signing.sh throwaway <dossier>}" ;;
  check) check "${2:?usage : android-signing.sh check <apk> [<aab>]}" "${3:-}" ;;
  teardown) rm -rf "${2:?usage : android-signing.sh teardown <dossier>}" ;;
  *) die "usage : android-signing.sh setup|throwaway|check|teardown …" ;;
esac
