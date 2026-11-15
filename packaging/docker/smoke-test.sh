#!/usr/bin/env bash
# Essai réel de l'image de Corde serveur (job CI « server-docker », ou à la main) :
#
#   packaging/docker/smoke-test.sh <version>
#
# L'image corde-server:<version> doit être chargée (docker load < corde-server-…-docker-x64.tar.gz).
# Démarre le conteneur avec packaging/docker/compose.yaml tel quel (lecture seule, capacités,
# no-new-privileges, mémoire), sur un dossier de données vide appartenant à un autre uid (comme un
# dossier partagé de NAS), puis vérifie : état « healthy », processus non root sans capacité,
# GET /api/hub/pairing, premier code dans « docker logs » qui appaire un appareil par la vraie API
# (POST /api/hub/pair), commandes du terminal (devices, pair --admin), rien au second démarrage,
# données conservées, arrêt propre et rapide (SIGTERM), Host refusé / accepté avec
# CORDE_ALLOW_HOST, valeur invalide refusée, CORDE_FIRST_PAIRING_CODE=0.
#
# Utilise le port 8471 de la machine. Nettoie derrière lui.
set -euo pipefail

[ "$#" -eq 1 ] || {
  printf 'Usage : %s <version>\n' "$0" >&2
  exit 2
}
version="$1"
image="corde-server:${version}"
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
port=8471
base="http://127.0.0.1:${port}"
project=corde-smoke
other_uid=4242

as_root() {
  if [ "$(id -u)" -eq 0 ]; then "$@"; else sudo "$@"; fi
}
fail() {
  printf '::error::%s\n' "$*" >&2
  docker compose -p "$project" logs --no-color 2>/dev/null | tail -n 80 >&2 || true
  exit 1
}
step() { printf '\n=== %s\n' "$*"; }

docker image inspect "$image" >/dev/null 2>&1 || fail "image $image absente (docker load d'abord)"

work="$(mktemp -d)"
cleanup() {
  (cd "$work" && docker compose -p "$project" down --timeout 5 >/dev/null 2>&1) || true
  as_root rm -rf "$work"
}
trap cleanup EXIT

# Le fichier compose publié, tel quel, sauf l'image (déjà la bonne si les versions concordent).
grep -q "image: ${image}\$" "$here/compose.yaml" ||
  fail "compose.yaml ne référence pas ${image} : mets à jour la ligne « image: » de packaging/docker/compose.yaml"
cp "$here/compose.yaml" "$work/compose.yaml"
mkdir "$work/data"
as_root chown "${other_uid}:${other_uid}" "$work/data"
as_root chmod 0755 "$work/data"

compose() { (cd "$work" && docker compose -p "$project" "$@"); }
cid() { compose ps -q corde; }

wait_healthy() {
  local c status
  for _ in $(seq 1 60); do
    c="$(cid)"
    if [ -n "$c" ]; then
      status="$(docker inspect --format '{{if .State.Health}}{{.State.Health.Status}}{{end}}' "$c")"
      [ "$status" = healthy ] && return 0
      [ "$(docker inspect --format '{{.State.Running}}' "$c")" = true ] || fail "le conteneur s'est arrêté"
    fi
    sleep 2
  done
  fail "pas « healthy » après 120 s"
}

http() {
  # http <méthode> <chemin> [en-têtes curl…] → « code corps » dans $reply_code / $reply_body
  local method="$1" path="$2"
  shift 2
  local out
  out="$(curl -sS -o - -w '\n%{http_code}' -X "$method" "$@" "${base}${path}")" || fail "curl ${path}"
  reply_code="${out##*$'\n'}"
  reply_body="${out%$'\n'*}"
}

step "Premier démarrage (dossier vide appartenant à l'uid ${other_uid})"
compose up -d
wait_healthy
c="$(cid)"

step "Durcissement effectif"
[ "$(docker inspect --format '{{.HostConfig.ReadonlyRootfs}}' "$c")" = true ] || fail "pas en lecture seule"
docker inspect --format '{{.HostConfig.CapDrop}} {{.HostConfig.CapAdd}} {{.HostConfig.SecurityOpt}} {{.HostConfig.Memory}}' "$c"
status="$(docker exec "$c" cat /proc/1/status)"
printf '%s\n' "$status" | grep -E '^(Name|Uid|Gid|Groups|CapInh|CapPrm|CapEff|CapAmb|NoNewPrivs)'
printf '%s\n' "$status" | grep -Eq '^Name:[[:space:]]+corde-server$' || fail "le processus 1 n'est pas corde-server (exec manquant ?)"
printf '%s\n' "$status" | grep -Eq '^Uid:[[:space:]]+8471[[:space:]]+8471[[:space:]]+8471[[:space:]]+8471$' || fail "le serveur ne tourne pas sous l'uid 8471"
printf '%s\n' "$status" | grep -Eq '^Gid:[[:space:]]+8471[[:space:]]+8471[[:space:]]+8471[[:space:]]+8471$' || fail "le serveur ne tourne pas sous le gid 8471"
for cap in CapInh CapPrm CapEff CapAmb; do
  printf '%s\n' "$status" | grep -Eq "^${cap}:[[:space:]]+0+\$" || fail "${cap} non vide"
done
printf '%s\n' "$status" | grep -Eq '^NoNewPrivs:[[:space:]]+1$' || fail "no_new_privs absent"
if docker exec "$c" sh -c ': > /opt/corde/essai' 2>/dev/null; then fail "système de fichiers inscriptible"; fi
owner="$(as_root stat -c '%u:%g' "$work/data")"
[ "$owner" = "8471:8471" ] || fail "dossier de données à ${owner}, attendu 8471:8471"
as_root find "$work/data" -maxdepth 2 -exec stat -c '%A %u:%g %n' {} +

step "Route publique et journal"
http GET /api/hub/pairing
[ "$reply_code" = 200 ] || fail "GET /api/hub/pairing → ${reply_code}"
printf '%s\n' "$reply_body" | grep -q '"role":"hub"' || fail "rôle hub attendu : ${reply_body}"
logs="$(docker logs "$c" 2>&1)"
printf '%s\n' "$logs" | grep -q 'PREMIER APPAIRAGE' || fail "pas de premier code dans le journal"
code="$(printf '%s\n' "$logs" | sed -n "s/.*Code d'appairage : \([0-9]\{4\}\) \([0-9]\{4\}\).*/\1\2/p" | head -n 1)"
[ "${#code}" -eq 8 ] || fail "code introuvable dans le journal"
# Le code n'est écrit nulle part ailleurs.
if as_root grep -rqF "$code" "$work/data"; then fail "le code est écrit dans le dossier de données"; fi
printf '%s\n' "$logs" | grep -q 'adresse interne' || fail "la bannière ne signale pas le conteneur"

step "Appairage par la vraie API"
http POST /api/hub/pair -H 'Content-Type: application/json' \
  --data "{\"code\":\"${code}\",\"name\":\"PC essai\",\"kind\":\"app\"}"
[ "$reply_code" = 201 ] || fail "POST /api/hub/pair → ${reply_code} ${reply_body}"
token="$(printf '%s\n' "$reply_body" | sed -n 's/.*"token":"\([0-9a-f]\{64\}\)".*/\1/p')"
[ "${#token}" -eq 64 ] || fail "pas de jeton dans la réponse"
printf '%s\n' "$reply_body" | grep -q '"admin":true' || fail "le premier appareil doit être administrateur"
http POST /api/hub/pair -H 'Content-Type: application/json' --data "{\"code\":\"${code}\",\"kind\":\"app\"}"
[ "$reply_code" = 400 ] || fail "le code a servi deux fois (${reply_code})"
http GET /api/hub/status -H "Authorization: Bearer ${token}"
[ "$reply_code" = 200 ] || fail "GET /api/hub/status avec le jeton → ${reply_code}"
http GET /api/hub/status
[ "$reply_code" = 401 ] || fail "GET /api/hub/status sans jeton → ${reply_code}"

step "Commandes du terminal du conteneur (root → compte corde)"
docker exec "$c" corde-server devices | tee "$work/devices.txt"
grep -q 'PC essai' "$work/devices.txt" || fail "corde-server devices ne liste pas l'appareil"
docker exec "$c" corde-server pair --admin | tee "$work/pair.txt"
grep -q "Code d'appairage" "$work/pair.txt" || fail "corde-server pair --admin sans code"
docker exec "$c" corde-sauvegarde | tee "$work/backup.txt"
backup="$(sed -n 's/^Sauvegarde écrite : \(corde-sauvegarde-[0-9-]*\.tar\.gz\)$/\1/p' "$work/backup.txt")"
[ -n "$backup" ] || fail "corde-sauvegarde : pas de fichier annoncé"
as_root tar -tzf "$work/data/$backup" > "$work/backup-list.txt"
grep -qx 'corde/hub/devices.json' "$work/backup-list.txt" || fail "sauvegarde sans corde/hub/devices.json"
grep -qx 'corde/corde.db' "$work/backup-list.txt" || fail "sauvegarde sans corde/corde.db"
if grep -q 'corde-sauvegarde-' "$work/backup-list.txt"; then fail "la sauvegarde se contient elle-même"; fi
as_root rm -f "$work/data/$backup"

step "Arrêt propre (SIGTERM) et rapide"
start=$(date +%s)
compose stop --timeout 20
elapsed=$(($(date +%s) - start))
exit_code="$(docker inspect --format '{{.State.ExitCode}}' "$c")"
echo "arrêt en ${elapsed} s, code de sortie ${exit_code}"
[ "$elapsed" -le 5 ] || fail "arrêt en ${elapsed} s (SIGTERM mal reçu ?)"
[ "$exit_code" = 0 ] || fail "code de sortie ${exit_code} après SIGTERM"

step "Second démarrage : données conservées, aucun code"
compose start
wait_healthy
c="$(cid)"
since_restart="$(docker logs --since 1m "$c" 2>&1 | tail -n 40)"
printf '%s\n' "$since_restart"
# Le journal d'un conteneur garde les démarrages précédents : seul le dernier compte.
last_run="$(docker logs "$c" 2>&1 | awk '/serveur Corde \(rôle hub\)/ { buf = "" } { buf = buf $0 "\n" } END { printf "%s", buf }')"
if printf '%s\n' "$last_run" | grep -q 'PREMIER APPAIRAGE'; then fail "un code est affiché alors qu'un appareil est appairé"; fi
http GET /api/hub/status -H "Authorization: Bearer ${token}"
[ "$reply_code" = 200 ] || fail "jeton perdu après redémarrage (${reply_code})"

step "Nom d'hôte : refusé, puis accepté avec CORDE_ALLOW_HOST"
http GET /api/hub/pairing -H 'Host: nas-maison.example:8471'
[ "$reply_code" = 403 ] || fail "Host inconnu accepté (${reply_code})"
sed -i 's/CORDE_ALLOW_HOST: ""/CORDE_ALLOW_HOST: " nas-maison.example , corde.vpn.example "/' "$work/compose.yaml"
grep -q 'nas-maison.example' "$work/compose.yaml" || fail "sed CORDE_ALLOW_HOST"
compose up -d
wait_healthy
for h in nas-maison.example:8471 corde.vpn.example; do
  http GET /api/hub/pairing -H "Host: ${h}"
  [ "$reply_code" = 200 ] || fail "Host ${h} refusé malgré CORDE_ALLOW_HOST (${reply_code})"
done
http GET /api/hub/pairing -H 'Host: autre.example'
[ "$reply_code" = 403 ] || fail "Host non listé accepté (${reply_code})"

step "Valeur de CORDE_ALLOW_HOST invalide : le conteneur refuse de démarrer"
out="$(docker run --rm -e 'CORDE_ALLOW_HOST=ok.example,$(id);x' "$image" 2>&1 || true)"
printf '%s\n' "$out"
printf '%s\n' "$out" | grep -q 'nom refusé' || fail "valeur invalide non refusée"

step "CORDE_FIRST_PAIRING_CODE=0 : aucun code, même sans appareil"
compose down --timeout 5
as_root rm -rf "$work/data"
mkdir "$work/data"
sed -i 's/CORDE_FIRST_PAIRING_CODE: "1"/CORDE_FIRST_PAIRING_CODE: "0"/' "$work/compose.yaml"
compose up -d
wait_healthy
if docker logs "$(cid)" 2>&1 | grep -q 'PREMIER APPAIRAGE'; then fail "code affiché avec CORDE_FIRST_PAIRING_CODE=0"; fi

step "Mémoire utilisée"
docker stats --no-stream --format '{{.Name}} {{.MemUsage}}' "$(cid)"

printf '\nImage %s : tous les essais sont passés.\n' "$image"
