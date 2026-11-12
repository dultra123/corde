#!/usr/bin/env bash
# Désinstalle Corde serveur (docs/SERVEUR.md).
#
#   sudo ./uninstall.sh            retire le programme et le service, GARDE tes données
#   sudo ./uninstall.sh --purge    retire aussi /var/lib/corde (sessions, appareils…) et le compte « corde »
#
# Le script se trouve aussi dans /opt/corde/uninstall.sh après l'installation.
set -euo pipefail

PREFIX=/opt/corde
DATA=/var/lib/corde
UNIT=/etc/systemd/system/corde.service
LINK=/usr/local/bin/corde-server
ACCOUNT=corde

purge=no
case "${1:-}" in
  "") ;;
  --purge) purge=yes ;;
  *)
    printf 'Usage : sudo %s [--purge]\n' "$0" >&2
    exit 2
    ;;
esac

[ "$(id -u)" -eq 0 ] || {
  printf 'Erreur : lance ce script avec sudo.\n' >&2
  exit 1
}

if command -v systemctl >/dev/null 2>&1; then
  systemctl disable --now corde.service >/dev/null 2>&1 || true
fi
rm -f "$UNIT"
if command -v systemctl >/dev/null 2>&1; then
  systemctl daemon-reload || true
fi
if [ -L "$LINK" ] && [ "$(readlink "$LINK")" = "$PREFIX/corde-server" ]; then
  rm -f "$LINK"
fi
rm -rf "$PREFIX"

if [ "$purge" = yes ]; then
  rm -rf "$DATA"
  if getent passwd "$ACCOUNT" >/dev/null; then
    userdel "$ACCOUNT" || true
  fi
  if getent group "$ACCOUNT" >/dev/null; then
    groupdel "$ACCOUNT" || true
  fi
  printf 'Corde serveur est désinstallé, données comprises.\n'
elif [ -d "$DATA" ]; then
  printf 'Corde serveur est désinstallé. Tes données restent dans %s\n' "$DATA"
  printf '(pour les effacer aussi : sudo rm -rf %s puis sudo userdel %s).\n' "$DATA" "$ACCOUNT"
else
  printf 'Corde serveur est désinstallé.\n'
fi
