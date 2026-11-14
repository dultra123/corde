#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Prépare la clé d'API App Store Connect pour xcodebuild, à partir des secrets du dépôt.
#
#   APPSTORE_API_KEY=… APPSTORE_API_KEY_ID=… APPSTORE_API_ISSUER_ID=… APPLE_TEAM_ID=… \
#     asc-key.sh <dossier de sortie>
#
# Écrit <dossier>/AuthKey_<ID de clé>.p8 (lisible par ce seul compte) et, sous GitHub Actions,
# les variables ASC_KEY_PATH, ASC_KEY_ID, ASC_ISSUER_ID et ASC_TEAM_ID pour les étapes suivantes.
#
# Un secret collé à la main arrive rarement tel quel : le script accepte donc
#   - le contenu du fichier .p8 (lignes BEGIN et END comprises), avec des fins de ligne Windows,
#     des espaces en trop, ou tout sur une seule ligne ;
#   - seulement le texte entre les deux lignes ;
#   - le fichier .p8 entier encodé en base64 ;
# et il retire les espaces et retours à la ligne autour des trois identifiants.
#
# Il n'affiche JAMAIS une valeur : en cas de problème, seulement sa longueur et sa forme.
# Compatible avec le bash 3.2 de macOS. Aucun `… | grep -q` : avec `pipefail`, un lecteur qui
# s'arrête au premier résultat peut faire échouer l'écrivain.

set -euo pipefail
umask 077

out_dir="${1:?usage : asc-key.sh <dossier de sortie>}"
mkdir -p "$out_dir"

BEGIN="-----BEGIN PRIVATE KEY-----"
END="-----END PRIVATE KEY-----"

trim() { printf '%s' "$1" | tr -d '[:space:]'; }

# Décrit une valeur sans l'afficher.
shape() {
  local v="$1" lines
  if [ -z "$v" ]; then
    printf 'vide'
    return
  fi
  lines=$(printf '%s\n' "$v" | wc -l | tr -d '[:space:]')
  printf '%s caractères sur %s ligne(s)' "${#v}" "$lines"
}

# Le texte entre les lignes BEGIN et END, sans espace ni retour à la ligne.
body_of() {
  local t="$1"
  t="${t#*"$BEGIN"}"
  t="${t%%"$END"*}"
  trim "$t"
}

notice() { echo "::notice::$*"; }
warn() { echo "::warning::$*"; }
fail() {
  echo "::error::$*"
  exit 1
}

key_id="$(trim "${APPSTORE_API_KEY_ID:-}")"
issuer_id="$(trim "${APPSTORE_API_ISSUER_ID:-}")"
team_id="$(trim "${APPLE_TEAM_ID:-}")"
raw="${APPSTORE_API_KEY:-}"
raw="${raw//$'\r'/}"

# --- Les trois identifiants : avertir si la forme surprend (sans bloquer) ------------------------

if ! [[ "$key_id" =~ ^[A-Z0-9]{10}$ ]]; then
  warn "APPSTORE_API_KEY_ID : forme inattendue ($(shape "$key_id")). Attendu : l'« ID de clé », 10 lettres majuscules et chiffres."
fi
if ! [[ "$issuer_id" =~ ^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$ ]]; then
  warn "APPSTORE_API_ISSUER_ID : forme inattendue ($(shape "$issuer_id")). Attendu : l'« ID d'émetteur », 36 caractères de la forme xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx."
fi
if ! [[ "$team_id" =~ ^[A-Z0-9]{10}$ ]]; then
  warn "APPLE_TEAM_ID : forme inattendue ($(shape "$team_id")). Attendu : le Team ID, 10 lettres majuscules et chiffres."
fi
[ -n "$key_id" ] || fail "APPSTORE_API_KEY_ID est vide."

# --- La clé : retrouver le texte entre BEGIN et END, quelle que soit la façon de la coller -------

case "$raw" in
  *"$BEGIN"*) body="$(body_of "$raw")" ;;
  *)
    body="$(trim "$raw")"
    # Le secret peut aussi être le fichier .p8 entier en base64.
    tmp="$out_dir/decoded.tmp"
    if printf '%s' "$body" | base64 --decode > "$tmp" 2> /dev/null && grep -q -- "$BEGIN" "$tmp"; then
      inner="$(tr -d '\r' < "$tmp")"
      body="$(body_of "$inner")"
    fi
    rm -f "$tmp"
    ;;
esac

key="$out_dir/AuthKey_${key_id}.p8"
{
  printf '%s\n' "$BEGIN"
  printf '%s\n' "$body" | fold -w 64
  printf '%s\n' "$END"
} > "$key"

if ! command -v openssl > /dev/null 2>&1; then
  warn "openssl absent : la clé n'a pas pu être vérifiée avant xcodebuild."
elif ! openssl pkey -in "$key" -noout > /dev/null 2>&1; then
  rm -f "$key"
  hint="Coller tout le contenu du fichier AuthKey_….p8 (environ 260 caractères sur 6 lignes), de la ligne « $BEGIN » à la ligne « $END » comprises."
  case "$raw" in
    "") what="le secret est vide" ;;
    *"$BEGIN"*"$END"*) what="les lignes BEGIN et END sont là, mais le texte entre les deux n'est pas une clé (texte coupé ou modifié ?)" ;;
    *"$BEGIN"*) what="la fin de la clé manque (pas de ligne END)" ;;
    *AuthKey_*) what="on dirait le nom du fichier, pas son contenu" ;;
    *PRIVATE\ KEY*) what="ce n'est pas une clé au format d'Apple (ligne BEGIN différente)" ;;
    *)
      if [ "${#raw}" -le 40 ]; then
        what="trop court pour une clé : on dirait un identifiant (ID de clé ou ID d'émetteur)"
      else
        what="ce n'est pas le contenu d'un fichier .p8"
      fi
      ;;
  esac
  fail "APPSTORE_API_KEY n'est pas une clé .p8 lisible : ${what} (reçu : $(shape "$raw")). ${hint}"
fi

if [ -n "${GITHUB_ENV:-}" ]; then
  for v in "$key_id" "$issuer_id" "$team_id"; do
    [ -z "$v" ] || echo "::add-mask::$v"
  done
  {
    echo "ASC_KEY_PATH=$key"
    echo "ASC_KEY_ID=$key_id"
    echo "ASC_ISSUER_ID=$issuer_id"
    echo "ASC_TEAM_ID=$team_id"
  } >> "$GITHUB_ENV"
fi

notice "Clé d'API App Store Connect : clé privée lisible, prête pour xcodebuild."
