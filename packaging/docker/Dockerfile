# Image de Corde serveur (rôle hub) : docs/SYNOLOGY.md, ADR 0017.
#
# Ne compile rien : elle reprend le binaire et l'interface déjà construits, les mêmes entrées que
# packaging/linux/make-archive.sh. Le contexte de construction est assemblé par make-image.sh :
#
#   corde-server     binaire Linux (glibc 2.28 ou plus ancienne, job CI « server-linux »)
#   ui/              interface construite (packages/ui/dist)
#   LICENSE          licence (GPL-3.0-or-later), comme dans l'archive Linux
#   docs/            SERVEUR.md, SYNOLOGY.md, ACCES-EXTERIEUR.md
#   entrypoint.sh    point d'entrée (démarrage, compte « corde »)
#
# Image de base épinglée par son empreinte (index multi-architecture). Pour la rafraîchir :
#
#   docker buildx imagetools inspect debian:bookworm-slim
#
# puis remplacer la valeur « Digest: sha256:… » ci-dessous (ligne BASE_IMAGE), et noter le
# changement dans CHANGELOG.md. Le job CI « server-docker » affiche l'empreinte actuelle à côté de
# l'empreinte épinglée.
ARG BASE_IMAGE=debian:bookworm-slim@sha256:734a72b3d6142a71993b4bc580ca01d8ca687f77a2c422d96a961d3376b179bc

# Le compte « corde » (uid et gid 8471, le port de Corde), ajouté aux fichiers de comptes de
# l'image de base. Cette étape tourne sur la machine qui construit ($BUILDPLATFORM) : pas
# d'émulation pour l'image arm64 ; les fichiers /etc/passwd et /etc/group de Debian sont les mêmes
# pour toutes les architectures d'une même empreinte.
FROM --platform=$BUILDPLATFORM ${BASE_IMAGE} AS accounts
RUN printf '%s\n' 'corde:x:8471:8471:Corde serveur:/var/lib/corde:/usr/sbin/nologin' >> /etc/passwd \
 && printf '%s\n' 'corde:x:8471:' >> /etc/group

FROM ${BASE_IMAGE}

ARG VERSION=0.0.0
LABEL org.opencontainers.image.title="Corde serveur" \
      org.opencontainers.image.description="Serveur Corde de la maison (rôle hub) : sessions, analyses, profil et courses pour tes appareils appairés. Jamais exposé sur Internet." \
      org.opencontainers.image.version="${VERSION}" \
      org.opencontainers.image.source="https://github.com/dultra123/corde" \
      org.opencontainers.image.url="https://github.com/dultra123/corde/blob/main/docs/SYNOLOGY.md" \
      org.opencontainers.image.documentation="https://github.com/dultra123/corde/blob/main/docs/SYNOLOGY.md" \
      org.opencontainers.image.licenses="GPL-3.0-or-later"

COPY --from=accounts /etc/passwd /etc/group /etc/
COPY --chmod=0755 corde-server /opt/corde/corde-server
COPY --chmod=0755 entrypoint.sh /usr/local/bin/corde-entrypoint
# La commande « corde-server » du terminal du conteneur : le même script, qui passe sous le compte
# « corde » avant de lancer le binaire (pair, devices, revoke).
COPY --chmod=0755 entrypoint.sh /usr/local/bin/corde-server
# « corde-sauvegarde » : une archive du dossier de données, au format de la sauvegarde d'un Pi.
COPY --chmod=0755 entrypoint.sh /usr/local/bin/corde-sauvegarde
COPY ui /opt/corde/ui
COPY LICENSE /opt/corde/LICENSE
COPY docs /opt/corde/docs

# 1 : premier code d'appairage dans le journal tant qu'aucun appareil n'est appairé (0 : jamais).
# CORDE_ALLOW_HOST : noms d'hôte supplémentaires, séparés par des virgules (vide : aucun).
ENV CORDE_FIRST_PAIRING_CODE=1 \
    CORDE_ALLOW_HOST=""

EXPOSE 8471
VOLUME ["/var/lib/corde"]
STOPSIGNAL SIGTERM
HEALTHCHECK --interval=30s --timeout=5s --start-period=30s --start-interval=2s --retries=3 \
  CMD ["/opt/corde/corde-server", "healthcheck", "--port", "8471"]
ENTRYPOINT ["/usr/local/bin/corde-entrypoint"]
