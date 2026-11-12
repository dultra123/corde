#!/usr/bin/env bash
# Installe ou met à jour Corde serveur (docs/SERVEUR.md). À lancer depuis le dossier de
# l'archive décompressée :
#
#   sudo ./install.sh
#
# - programme dans /opt/corde (remplacé à chaque installation),
# - données dans /var/lib/corde (jamais effacées par ce script : une mise à jour les garde),
# - compte système « corde » sans mot de passe ni connexion,
# - service systemd « corde » activé au démarrage, redémarré s'il s'arrête,
# - commande « corde-server » disponible pour l'appairage (sudo corde-server pair).
#
# Relancer ce script avec une nouvelle archive met Corde serveur à jour.
set -euo pipefail

PREFIX=/opt/corde
DATA=/var/lib/corde
UNIT=/etc/systemd/system/corde.service
LINK=/usr/local/bin/corde-server
ACCOUNT=corde
PORT=8471

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

say() { printf '%s\n' "$*"; }
die() {
  printf 'Erreur : %s\n' "$*" >&2
  exit 1
}

[ "$(id -u)" -eq 0 ] || die "lance ce script avec sudo : sudo ./install.sh"
command -v systemctl >/dev/null 2>&1 || die "systemd est introuvable (Raspberry Pi OS ou Debian attendu)."
for f in corde-server corde.service ui/index.html; do
  [ -e "$here/$f" ] || die "fichier manquant dans l'archive : $f (décompresse toute l'archive, puis relance)."
done

# Le bon processeur : l'archive arm64 sur un Raspberry Pi 64 bits, x64 sur un PC.
machine="$(uname -m)"
if [ -f "$here/ARCH" ]; then
  want="$(cat "$here/ARCH")"
  case "$want:$machine" in
    arm64:aarch64 | arm64:arm64 | x64:x86_64 | x64:amd64) ;;
    arm64:armv7l | arm64:armv6l)
      die "ce système est en 32 bits ($machine) : installe Raspberry Pi OS 64 bits (docs/SERVEUR.md)."
      ;;
    *) die "cette archive est pour $want, ce système est $machine : télécharge l'autre archive." ;;
  esac
fi
"$here/corde-server" --version >/dev/null 2>&1 || die "le programme ne démarre pas sur ce système ($machine)."
version="$("$here/corde-server" --version | awk '{ print $2 }')"

updating=no
[ -f "$UNIT" ] && updating=yes

# Compte système.
if ! getent group "$ACCOUNT" >/dev/null; then
  groupadd --system "$ACCOUNT"
fi
if ! getent passwd "$ACCOUNT" >/dev/null; then
  useradd --system --gid "$ACCOUNT" --home-dir "$DATA" --no-create-home \
    --shell /usr/sbin/nologin --comment "Corde serveur" "$ACCOUNT"
fi

# Dossier de données : créé s'il manque, jamais vidé.
install -d -m 0750 -o "$ACCOUNT" -g "$ACCOUNT" "$DATA"

# Arrêt pendant le remplacement des fichiers (mise à jour).
if [ "$updating" = yes ]; then
  say "Mise à jour : arrêt du service…"
  systemctl stop corde.service || true
fi

# Programme et interface : copiés à côté puis échangés.
install -d -m 0755 "$PREFIX"
install -m 0755 "$here/corde-server" "$PREFIX/corde-server.new"
mv -f "$PREFIX/corde-server.new" "$PREFIX/corde-server"
rm -rf "$PREFIX/ui.new"
cp -R "$here/ui" "$PREFIX/ui.new"
chmod -R u=rwX,go=rX "$PREFIX/ui.new"
rm -rf "$PREFIX/ui"
mv "$PREFIX/ui.new" "$PREFIX/ui"
for f in LICENSE LISEZMOI.txt uninstall.sh; do
  if [ -f "$here/$f" ]; then
    install -m 0644 "$here/$f" "$PREFIX/$f"
  fi
done
[ -f "$PREFIX/uninstall.sh" ] && chmod 0755 "$PREFIX/uninstall.sh"
ln -sfn "$PREFIX/corde-server" "$LINK"

# Service. Le fichier de contrôle de l'ancien processus est retiré : le nouveau en écrit un
# (nouvelle clé) quand il est prêt, et c'est lui qu'on attend.
install -m 0644 "$here/corde.service" "$UNIT"
systemctl daemon-reload
systemctl enable corde.service >/dev/null
rm -f "$DATA/hub/control.json"
systemctl restart corde.service

# Le service écrit son fichier de contrôle au démarrage.
ready=no
for _ in $(seq 1 40); do
  if systemctl is-active --quiet corde.service && [ -s "$DATA/hub/control.json" ]; then
    ready=yes
    break
  fi
  sleep 0.5
done
if [ "$ready" != yes ]; then
  say ""
  say "Le service ne semble pas démarré. Regarde son journal :"
  say "  sudo journalctl -u corde -n 50 --no-pager"
  exit 1
fi

say ""
if [ "$updating" = yes ]; then
  say "Corde serveur $version est installé (mise à jour : tes données sont conservées)."
else
  say "Corde serveur $version est installé et démarré."
fi
say ""
say "Adresse à ouvrir sur ton téléphone ou à indiquer à Corde sur le PC :"
found=no
for ip in $(hostname -I 2>/dev/null || true); do
  case "$ip" in
    *:*) continue ;;
    127.*) continue ;;
  esac
  say "  http://$ip:$PORT"
  found=yes
done
[ "$found" = yes ] || say "  (aucune adresse réseau trouvée : le Raspberry Pi est-il branché au réseau ?)"
say "  ou http://$(hostname).local:$PORT si ton réseau connaît les noms en .local"
say ""
say "Pour appairer un appareil (le PC, puis le téléphone), lance ici :"
say "  sudo corde-server pair"
say "et tape le code affiché (valable 10 minutes, une seule fois)."
say ""
say "Ne redirige jamais le port $PORT depuis ta box : pour l'accès depuis l'extérieur,"
say "lis docs/ACCES-EXTERIEUR.md (VPN)."
