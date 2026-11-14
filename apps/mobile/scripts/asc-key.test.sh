#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Essais de asc-key.sh avec une clé jetable (créée ici, jamais une vraie clé d'Apple).
#   bash apps/mobile/scripts/asc-key.test.sh
# Le script est lancé avec le même bash que ces essais : sous macOS, /bin/bash (3.2).

set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

openssl genpkey -algorithm EC -pkeyopt ec_paramgen_curve:P-256 -out "$work/ref.p8" 2> /dev/null
ref="$(cat "$work/ref.p8")"
body="$(grep -v -- "-----" "$work/ref.p8" | tr -d '\n')"
want="$(openssl pkey -in "$work/ref.p8" -outform DER | openssl dgst -sha256)"

KEY_ID="ABCDE12345"
ISSUER="01234567-89ab-cdef-0123-456789abcdef"
TEAM="ZYXWV98765"
failures=0
n=0

# run <clé> [ID de clé] : lance le script, sortie dans $work/out.txt, dossier $work/k$n.
run() {
  n=$((n + 1))
  dir="$work/k$n"
  : > "$work/env"
  APPSTORE_API_KEY="$1" APPSTORE_API_KEY_ID="${2-$KEY_ID}" APPSTORE_API_ISSUER_ID="$ISSUER" \
    APPLE_TEAM_ID="$TEAM" GITHUB_ENV="$work/env" "${BASH:-bash}" "$here/asc-key.sh" "$dir" > "$work/out.txt" 2>&1
}

ok() { echo "ok   $1"; }
ko() {
  echo "FAIL $1"
  sed 's/^/     | /' "$work/out.txt"
  failures=$((failures + 1))
}

# accepts <nom> <clé> : la clé écrite doit être exactement la clé de référence.
accepts() {
  if run "$2" && [ "$(openssl pkey -in "$dir/AuthKey_$KEY_ID.p8" -outform DER | openssl dgst -sha256)" = "$want" ]; then
    ok "$1"
  else
    ko "$1"
  fi
}

# rejects <nom> <clé> <texte attendu dans l'erreur> : refus, sans fichier de clé, sans fuite.
rejects() {
  if run "$2"; then
    ko "$1 (accepté à tort)"
  elif ! grep -q "::error::" "$work/out.txt" || ! grep -q -- "$3" "$work/out.txt"; then
    ko "$1 (message inattendu)"
  elif ls "$dir"/AuthKey_* > /dev/null 2>&1; then
    ko "$1 (un fichier de clé est resté)"
  else
    ok "$1"
  fi
}

accepts "fichier .p8 tel quel" "$ref"
accepts "fins de ligne Windows" "$(printf '%s' "$ref" | sed $'s/$/\r/')"
accepts "lignes vides et espaces autour" "$(printf '\n\n  %s  \n\n' "$ref")"
accepts "tout sur une seule ligne, espaces à la place des retours" "$(printf '%s' "$ref" | tr '\n' ' ')"
accepts "tout sur une seule ligne, sans séparateur" "$(printf '%s' "$ref" | tr -d '\n')"
accepts "seulement le texte entre BEGIN et END" "$body"
accepts "fichier .p8 entier en base64" "$(base64 < "$work/ref.p8" | tr -d '\n')"
accepts "fichier .p8 en base64, coupé en lignes" "$(base64 < "$work/ref.p8" | tr -d '\n' | fold -w 40)"

rejects "secret vide" "" "vide"
rejects "ID de clé à la place de la clé" "$KEY_ID" "on dirait un identifiant"
rejects "ID d'émetteur à la place de la clé" "$ISSUER" "on dirait un identifiant"
rejects "nom du fichier à la place du contenu" "C:\\Users\\quelquun\\Downloads\\AuthKey_$KEY_ID.p8 et un peu de texte autour" "nom du fichier"
rejects "clé coupée (pas de ligne END)" "$(printf '%s' "$ref" | sed '$d' | sed '$d')" "pas de ligne END"
rejects "texte abîmé entre BEGIN et END" "$(printf -- '-----BEGIN PRIVATE KEY-----\nMIGTAgEAMBMGByqGSM49AgEGCCqGSM49\n-----END PRIVATE KEY-----')" "n'est pas une clé"
rejects "texte quelconque" "ceci n'est pas une clé, c'est une phrase assez longue pour ne pas être un identifiant" "pas le contenu"

# Aucune valeur de clé dans la sortie, que la clé soit acceptée ou refusée.
run "$ref"
if grep -q -- "$body" "$work/out.txt" || grep -q "MIG" "$work/out.txt"; then ko "aucune fuite (clé acceptée)"; else ok "aucune fuite (clé acceptée)"; fi
run "$(printf '%s' "$ref" | sed '$d' | sed '$d')" || true
first_line="$(sed -n 2p "$work/ref.p8")"
if grep -q -- "$first_line" "$work/out.txt"; then ko "aucune fuite (clé refusée)"; else ok "aucune fuite (clé refusée)"; fi

# Les identifiants : espaces et retours à la ligne retirés, variables pour les étapes suivantes.
if run "$ref" "  $KEY_ID
" && grep -qx "ASC_KEY_ID=$KEY_ID" "$work/env" && grep -qx "ASC_ISSUER_ID=$ISSUER" "$work/env" &&
  grep -qx "ASC_TEAM_ID=$TEAM" "$work/env" && grep -qx "ASC_KEY_PATH=$dir/AuthKey_$KEY_ID.p8" "$work/env" &&
  ! grep -q "::warning::" "$work/out.txt"; then
  ok "identifiants nettoyés et transmis"
else
  ko "identifiants nettoyés et transmis"
fi

# Un identifiant de forme inattendue avertit sans bloquer, et sans afficher la valeur (la ligne
# « ::add-mask:: » est une commande pour GitHub, qui la masque : elle ne compte pas).
if run "$ref" "pas-un-id" && grep -q "::warning::APPSTORE_API_KEY_ID" "$work/out.txt" &&
  grep -v "^::add-mask::" "$work/out.txt" > "$work/shown.txt" && ! grep -q "pas-un-id" "$work/shown.txt"; then
  ok "identifiant de forme inattendue : avertissement sans la valeur"
else
  ko "identifiant de forme inattendue : avertissement sans la valeur"
fi

# Droits du fichier de clé : lisible par le seul propriétaire.
run "$ref"
perm="$(stat -c '%a' "$dir/AuthKey_$KEY_ID.p8" 2> /dev/null || stat -f '%Lp' "$dir/AuthKey_$KEY_ID.p8")"
if [ "$perm" = "600" ]; then ok "fichier de clé en 0600"; else ko "fichier de clé en 0600 (trouvé : $perm)"; fi

echo
if [ "$failures" -eq 0 ]; then
  echo "asc-key.sh : tous les essais passent"
else
  echo "asc-key.sh : $failures essai(s) en échec"
  exit 1
fi
