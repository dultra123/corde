#!/usr/bin/env bash
# Assemble l'archive de Corde serveur (utilisé par la CI, job « server-linux », et à la main) :
#
#   packaging/linux/make-archive.sh <version> <arm64|x64> <binaire corde-server> <dossier ui> <dossier de sortie>
#
# → <sortie>/corde-server-<version>-linux-<arch>.tar.gz, qui contient un dossier
#   corde-server-<version>-linux-<arch>/ (binaire, ui/, service, scripts, docs, licence).
set -euo pipefail

[ "$#" -eq 5 ] || {
  printf 'Usage : %s <version> <arm64|x64> <binaire> <ui> <sortie>\n' "$0" >&2
  exit 2
}
version="$1"
arch="$2"
binary="$3"
ui="$4"
out="$5"

case "$arch" in
  arm64 | x64) ;;
  *)
    printf 'architecture inconnue : %s (arm64 ou x64)\n' "$arch" >&2
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
name="corde-server-${version}-linux-${arch}"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

dir="$work/$name"
mkdir -p "$dir/docs"
install -m 0755 "$binary" "$dir/corde-server"
cp -R "$ui" "$dir/ui"
install -m 0644 "$here/corde.service" "$dir/corde.service"
install -m 0755 "$here/install.sh" "$dir/install.sh"
install -m 0755 "$here/uninstall.sh" "$dir/uninstall.sh"
install -m 0644 "$here/LISEZMOI.txt" "$dir/LISEZMOI.txt"
install -m 0644 "$repo/LICENSE" "$dir/LICENSE"
install -m 0644 "$repo/docs/SERVEUR.md" "$dir/docs/SERVEUR.md"
install -m 0644 "$repo/docs/ACCES-EXTERIEUR.md" "$dir/docs/ACCES-EXTERIEUR.md"
printf '%s\n' "$arch" >"$dir/ARCH"

mkdir -p "$out"
tar -C "$work" --owner=0 --group=0 --numeric-owner -czf "$out/$name.tar.gz" "$name"
printf '%s\n' "$out/$name.tar.gz"
