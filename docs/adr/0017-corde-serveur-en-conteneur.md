# ADR 0017 — Corde serveur en image de conteneur (NAS Synology), premier code dans le journal

- **Statut** : **décidée** — 2026-11-15 (demande du propriétaire : héberger Corde serveur sur son NAS
  Synology DS220+, Intel, 2 Go, plutôt que d'acheter un Raspberry Pi ; il ne programme pas et n'a pas de
  terminal commode sur le NAS).
- **Source** : `docs/SYNOLOGY.md`, `docs/SECURITY-MODEL.md` §7.6, `packaging/docker/`, ADR 0010 (serveur
  domestique), ADR 0009 (releases GitHub).

## Contexte

Corde serveur (`corde-server --role hub`, ADR 0010) n'existe qu'en archive Linux installée par `install.sh`
comme service systemd. Un NAS Synology n'offre ni systemd accessible ni gestionnaire de paquets Debian ; il
offre **Container Manager** (Docker), avec une interface pour importer une image et créer un « projet »
compose. Le premier appairage demande aujourd'hui `sudo corde-server pair` sur le serveur, donc un terminal.

## Décision

1. **Une image de conteneur construite à partir de l'archive Linux**, sans rien recompiler : même binaire
   (glibc 2.28), même interface, même licence, sur `debian:bookworm-slim` épinglé par empreinte, sans paquet
   ajouté. Données dans un volume `/var/lib/corde` (les commandes `pair`, `devices`, `revoke` le trouvent sans
   option). Images `linux/amd64` et `linux/arm64` (l'étape qui ajoute le compte tourne sur la machine de
   construction : pas d'émulation).
2. **Compte sans droits** : un point d'entrée POSIX sh démarre en root, donne au compte `corde` (8471) ce qui
   ne lui appartient pas dans le dossier de données (dossier partagé du NAS), puis quitte root pour de bon
   (`setpriv`, `no_new_privs`) et `exec` le serveur, qui reçoit SIGTERM. Le texte compose fourni ajoute la
   lecture seule, `cap_drop: ALL` sauf les quatre capacités du point d'entrée, `no-new-privileges`, une
   limite de mémoire.
3. **Premier code d'appairage dans le journal** : option `--first-pairing-code` du serveur, désactivée par
   défaut, activée dans l'image par `CORDE_FIRST_PAIRING_CODE=1` : au démarrage, seulement si aucun appareil
   n'est appairé, un code administrateur est émis par le mécanisme habituel et écrit sur la sortie standard.
   Les appareils suivants passent par le terminal du conteneur (`corde-server pair`). Analyse :
   SECURITY-MODEL §7.6.
4. **Distribution par fichier** : `docker save | gzip` → `corde-server-<version>-docker-<x64|arm64>.tar.gz`,
   publié au même endroit que les archives Linux (release `v<version>`, pré-release `continuous`), importé
   par Container Manager. **Aucun registre** pour l'instant (révisé le 21 novembre 2026 : voir l'amendement
   en fin de document).
5. **Vérifié par la CI** à chaque poussée : construction des deux images, architecture, licence, étiquettes,
   puis essai réel de l'image amd64 avec le texte compose tel que publié (`smoke-test.sh`).

## Alternatives écartées

- **Paquet DSM natif (`.spk`)** : format et outils propres à Synology, signature par Synology pour une
  installation sans avertissement, une variante par architecture et par version majeure de DSM, et un
  environnement d'exécution différent du Pi (pas de systemd, scripts de démarrage de DSM). Beaucoup de
  maintenance pour une seule marque ; l'image marche aussi sur les autres NAS et toute machine Docker.
- **Publication dans un registre maintenant** (GitHub Container Registry, Docker Hub) : la mise à jour
  serait plus simple (« télécharger » depuis Container Manager), mais il faut des droits d'écriture de paquets
  dans la CI, une politique de nettoyage et de visibilité des étiquettes, et une nouvelle surface (image
  publique que d'autres peuvent tirer et à laquelle se fier). Le fichier dans la release suffit au
  propriétaire ; à reconsidérer si d'autres utilisateurs viennent.
- **Lancer le binaire par le Planificateur de tâches de DSM** (script au démarrage) : tournerait en root
  sans isolation (ou sous un compte à créer à la main), sans redémarrage automatique en cas d'arrêt, sans
  journal lisible facilement, et dépend de la glibc et des chemins de DSM, qui changent d'une version à
  l'autre. Rien de ce que systemd fait sur le Pi.
- **Premier code sans journal** (fichier dans le dossier de données, page web « premier appareil » ouverte
  sans code) : un fichier est lisible par plus de monde (sauvegardes, partage réseau du dossier) et survit au
  processus ; une page ouverte sans code donnerait le serveur au premier appareil du réseau qui la trouve.
  Le journal n'est lisible que par qui peut déjà lancer `corde-server pair`.

## Conséquences

- Le propriétaire installe Corde serveur sur son NAS à la souris (docs/SYNOLOGY.md) ; la mise à jour est un
  import plus un numéro à changer dans le projet (depuis la 1.12.0 : arrêter puis recréer le projet, voir
  l'amendement). Pas de mise à jour automatique.
- La ligne `image:` de `packaging/docker/compose.yaml` suit le numéro de version (vérifié par la CI ; plus
  depuis la 1.12.0, voir l'amendement) ; le
  texte du guide doit rester identique à ce fichier (vérifié par `check-guide.py`).
- L'empreinte de l'image de base est à rafraîchir à la main (le job `server-docker` l'affiche) ; les
  correctifs de Debian n'arrivent qu'à ce moment-là.
- L'image arm64 est construite mais jamais essayée sur un vrai NAS ARM ; seule l'amd64 est essayée par la CI.
- Le nom d'hôte du NAS n'est pas reconnu tout seul (le conteneur a le sien) : adresse IP, ou
  `CORDE_ALLOW_HOST`.
- Le serveur garde les mêmes protections qu'ADR 0010 : jamais exposé à Internet, accès par VPN.

## Amendement du 21 novembre 2026 (version 1.12.0) : l'image est aussi publiée dans un registre

Le point 4 (« Aucun registre pour l'instant ») est révisé. Raison : la première mise à jour faite par le
propriétaire seul a échoué. La procédure demandait de télécharger un fichier, de l'importer, d'arrêter le
projet, de changer un numéro dans un texte, puis de reconstruire ; il a importé l'image et l'a lancée par le
bouton « Exécuter » de la section Image, ce qui a créé un second conteneur sans données ni port, pendant
que le vrai projet restait arrêté. Une mise à jour en cinq gestes dont un texte à modifier est trop fragile
pour quelqu'un qui n'est pas informaticien.

Décision :

1. Le job `version` de la CI pousse les deux images de la release dans le registre de GitHub :
   `ghcr.io/dultra123/corde-server:<version>` et `:latest`, chacune pour x64 et arm64 sous la même étiquette
   (liste de manifestes). Les images poussées sont **les fichiers de la release**, chargés puis poussés,
   pas une reconstruction. `packages: write` n'est accordé qu'à ce job.
2. `packaging/docker/compose.yaml` (et le texte du guide) prend `image: ghcr.io/dultra123/corde-server:latest`
   avec `pull_policy: always`. La ligne `image:` ne suit donc plus le numéro de version : le texte du projet
   ne change plus d'une version à l'autre.
3. Mettre à jour sur Synology : projet `corde` → **Arrêt**, puis **Action → Créer**. Rien à télécharger, à
   importer ou à écrire.
4. Le fichier `…-docker-<x64|arm64>.tar.gz` reste dans chaque release, pour un NAS sans Internet, DSM 7.0 /
   7.1 et qui veut figer une version.

Ce qui a été pesé :

- **Un bouton « Mettre à jour » dans Corde serveur lui-même** (le serveur se remplace tout seul) : écarté
  pour l'instant. Le conteneur est en lecture seule et sans privilèges ; se mettre à jour demanderait
  d'exécuter un programme téléchargé depuis le dossier de données, avec vérification de signature, retour
  arrière et protection de la page qui déclenche l'opération. C'est une surface sensible pour un gain de un
  clic.
- **Mise à jour automatique** (Watchtower, tâche planifiée) : écartée. Elle demande l'accès au démon Docker
  (équivalent root sur le NAS) et changerait le serveur sans que le propriétaire le décide.
- **Confiance** : l'image n'est pas signée ; le NAS se fie au compte GitHub et à TLS (voir
  SECURITY-MODEL §7.6). Même niveau que le fichier téléchargé à la main.
- **`latest`** : étiquette mobile, voulue ici (c'est elle qui rend la mise à jour sans texte à changer).
  Chaque version garde aussi son étiquette fixe.

Restent à vérifier sur le NAS du propriétaire : que « Créer » sur un projet arrêté télécharge bien la
nouvelle image avec `pull_policy: always` (version de Compose de Container Manager), et que le paquet est
public (un paquet créé par la CI est privé par défaut : réglage à faire une fois sur GitHub).
