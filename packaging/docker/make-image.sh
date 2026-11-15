#!/usr/bin/env bash
# Construit l'image de Corde serveur et l'enregistre dans un fichier importable par Container
# Manager (Synology) ou « docker load » (utilisé par la CI, job « server-docker », et à la main) :
#
#   packaging/docker/make-image.sh <version> <x64|arm64> <binaire corde-server> <dossier ui> <dossier de sortie>
#
# → image locale corde-server:<version> (plateforme linux/amd64 ou linux/arm64)
# → <sortie>/corde-server-<version>-docker-<arch>.tar.gz   (docker save | gzip)
#
# Ne compile rien : le binaire et l'interface sont ceux de l'archive Linux (make-archive.sh).
# Variable facultative : BASE_IMAGE (sinon celle épinglée dans le Dockerfile).
set -euo pipefail

[ "$#" -eq 5 ] || {
  printf 'Usage : %s <version> <x64|arm64> <binaire> <ui> <sortie>\n' "$0" >&2
  exit 2
}
version="$1"
arch="$2"
binary="$3"
ui="$4"
out="$5"

case "$arch" in
  x64) platform=linux/amd64 ;;
  arm64) platform=linux/arm64 ;;
  *)
    printf 'architecture inconnue : %s (x64 ou arm64)\n' "$arch" >&2
    exit 2
    ;;
esac
case "$version" in
  *[!0-9A-Za-z.+-]* | "")
    printf 'version invalide : %s\n' "$version" >&2
    exit 2
    ;;
esac
[ -x "$binary" ] || {
  printf 'binaire introuvable ou non exécutable : %s\n' "$binary" >&2
  exit 1
}
[ -f "$ui/index.html" ] || {
  printf 'interface introuvable : %s/index.html\n' "$ui" >&2
  exit 1
}

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

ctx="$work/context"
mkdir -p "$ctx/docs"
install -m 0755 "$binary" "$ctx/corde-server"
cp -R "$ui" "$ctx/ui"
chmod -R u=rwX,go=rX "$ctx/ui"
install -m 0644 "$here/Dockerfile" "$ctx/Dockerfile"
install -m 0755 "$here/entrypoint.sh" "$ctx/entrypoint.sh"
install -m 0644 "$repo/LICENSE" "$ctx/LICENSE"
for doc in SERVEUR.md SYNOLOGY.md ACCES-EXTERIEUR.md; do
  install -m 0644 "$repo/docs/$doc" "$ctx/docs/$doc"
done

tag="corde-server:${version}"
build_args=(--build-arg "VERSION=${version}")
if [ -n "${BASE_IMAGE:-}" ]; then
  build_args+=(--build-arg "BASE_IMAGE=${BASE_IMAGE}")
fi
# --provenance=false : une image simple (pas de liste d'attestations), que « docker save » et
# l'import de Container Manager prennent telle quelle.
docker buildx build --platform "$platform" --provenance=false --load \
  "${build_args[@]}" --tag "$tag" "$ctx" >&2

mkdir -p "$out"
file="$out/corde-server-${version}-docker-${arch}.tar.gz"
docker save "$tag" | gzip -9 >"$file"
printf '%s\n' "$file"
