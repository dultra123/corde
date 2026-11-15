#!/bin/sh
# Point d'entrée de l'image de Corde serveur (docs/SYNOLOGY.md, ADR 0017).
#
# Sans argument : démarre Corde serveur.
#   1. vérifie CORDE_ALLOW_HOST (noms séparés par des virgules → un --allow-host chacun ; jamais
#      évalué) et CORDE_FIRST_PAIRING_CODE (1 par défaut dans l'image → --first-pairing-code) ;
#   2. lancé en root (le cas normal) : donne au compte « corde » ce qui, dans le dossier de données
#      /var/lib/corde, ne lui appartient pas encore (un dossier partagé du NAS appartient en
#      général à un utilisateur du NAS) — rien d'autre n'est modifié ;
#   3. quitte root pour de bon (setpriv : uid et gid de « corde », groupes, capacités héritables
#      vidées, no_new_privs) et remplace ce script par le serveur (exec) : le serveur est le
#      processus 1 du conteneur et reçoit directement SIGTERM (docker stop) pour s'arrêter
#      proprement.
#
# Avec des arguments, ou appelé sous le nom « corde-server » (/usr/local/bin/corde-server, la
# commande à taper dans le terminal du conteneur) : lance corde-server avec ces arguments sous le
# compte « corde », sans rien préparer. Exemple : corde-server pair --admin
#
# Appelé sous le nom « corde-sauvegarde » : écrit une archive du dossier de données dans ce dossier.
set -eu

BIN=/opt/corde/corde-server
DATA=/var/lib/corde
UI=/opt/corde/ui
ACCOUNT=corde
PORT=8471

die() {
  printf 'corde : %s\n' "$*" >&2
  exit 1
}

# Lance "$@" sous le compte « corde » (déjà ce compte : tel quel).
as_corde() {
  if [ "$(id -u)" -ne 0 ]; then
    exec "$@"
  fi
  HOME="$DATA"
  export HOME
  exec setpriv --reuid="$ACCOUNT" --regid="$ACCOUNT" --init-groups \
    --inh-caps=-all --no-new-privs -- "$@"
}

[ -x "$BIN" ] || die "programme introuvable : $BIN"

# « corde-sauvegarde » (terminal du conteneur) : une archive du dossier de données, au même format
# que la sauvegarde d'un Raspberry Pi (docs/SERVEUR.md §8), écrite dans ce dossier.
if [ "$(basename "$0")" = corde-sauvegarde ]; then
  file="corde-sauvegarde-$(date -u +%Y-%m-%d-%H%M).tar.gz"
  # Lisible par les comptes du NAS (pour la récupérer avec File Station) : à effacer ensuite.
  umask 022
  # L'archive est écrite dans le dossier qu'elle sauvegarde (exclue) : le dossier « change » pendant la
  # lecture, ce que tar signale par le code 1 ; 2 et plus sont de vraies erreurs.
  set -- tar -czf "$DATA/$file" --warning=no-file-changed --exclude="corde/corde-sauvegarde-*" \
    -C "${DATA%/*}" "${DATA##*/}"
  if [ "$(id -u)" -eq 0 ]; then
    set -- setpriv --reuid="$ACCOUNT" --regid="$ACCOUNT" --init-groups --no-new-privs -- "$@"
  fi
  rc=0
  "$@" || rc=$?
  [ "$rc" -le 1 ] || die "sauvegarde impossible (tar : code $rc)"
  printf 'Sauvegarde écrite : %s\n' "$file"
  printf 'Elle est dans le dossier des données (« data ») du projet. Récupère-la, puis efface-la du NAS :\n'
  printf 'elle contient tes sessions et la liste de tes appareils (docs/SYNOLOGY.md, « Sauvegarder »).\n'
  exit 0
fi

if [ "$(basename "$0")" = corde-server ] || [ "$#" -gt 0 ]; then
  as_corde "$BIN" "$@"
fi

# --- Démarrage du serveur -------------------------------------------------------------------

set -- --role hub --data-dir "$DATA" --ui-dir "$UI" --port "$PORT"

# Premier code d'appairage dans le journal (docs/SECURITY-MODEL.md §7).
case "${CORDE_FIRST_PAIRING_CODE:-1}" in
  1 | true | oui | yes | on) set -- "$@" --first-pairing-code ;;
  0 | false | non | no | off | "") ;;
  *) die "CORDE_FIRST_PAIRING_CODE doit valoir 1 ou 0 (lu : « ${CORDE_FIRST_PAIRING_CODE} »)" ;;
esac

# Noms d'hôte supplémentaires : lettres, chiffres, points et tirets seulement.
if [ -n "${CORDE_ALLOW_HOST:-}" ]; then
  set -f
  old_ifs=$IFS
  IFS=','
  # shellcheck disable=SC2086 # découpage voulu sur les virgules (globbing coupé par set -f)
  for name in $CORDE_ALLOW_HOST; do
    IFS=$old_ifs
    # Espaces autour du nom retirés.
    name=$(printf '%s' "$name" | sed 's/^[[:space:]]*//; s/[[:space:]]*$//')
    [ -n "$name" ] || continue
    case "$name" in
      *[!A-Za-z0-9.-]* | -* | .* | *- | *..*)
        die "CORDE_ALLOW_HOST : nom refusé « $name » (lettres, chiffres, points et tirets seulement, séparés par des virgules)"
        ;;
    esac
    [ "${#name}" -le 253 ] || die "CORDE_ALLOW_HOST : nom trop long « $name »"
    set -- "$@" --allow-host "$name"
    IFS=','
  done
  IFS=$old_ifs
  set +f
fi

if [ "$(id -u)" -eq 0 ]; then
  id "$ACCOUNT" >/dev/null 2>&1 || die "compte « $ACCOUNT » absent de l'image"
  [ -d "$DATA" ] || mkdir -p "$DATA" || die "impossible de créer $DATA"
  # Ce qui n'appartient pas encore à « corde » lui est donné (premier démarrage, données copiées
  # depuis un Raspberry Pi ou une sauvegarde). Liens symboliques non suivis, un seul système de
  # fichiers.
  if ! find "$DATA" -xdev \( ! -user "$ACCOUNT" -o ! -group "$ACCOUNT" \) \
    -exec chown -h "$ACCOUNT:$ACCOUNT" {} + 2>/dev/null; then
    die "impossible de donner $DATA au compte « $ACCOUNT » (droits du dossier partagé : docs/SYNOLOGY.md, Dépannage)"
  fi
  # Le compte doit pouvoir lire et écrire tout ce qui est à lui (fait sous son nom, sans droit
  # particulier).
  setpriv --reuid="$ACCOUNT" --regid="$ACCOUNT" --init-groups --no-new-privs -- \
    find "$DATA" -xdev ! -type l \
    \( \( -type d ! -perm -u=rwx \) -o \( ! -type d ! -perm -u=rw \) \) \
    -exec chmod u+rwX {} + ||
    die "impossible de rendre $DATA inscriptible pour « $ACCOUNT »"
fi

# Fichiers créés par le serveur : lisibles par lui seul (comme le service systemd, UMask=0077).
umask 077
as_corde "$BIN" "$@"
