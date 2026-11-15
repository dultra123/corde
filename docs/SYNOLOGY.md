# Corde serveur sur un NAS Synology, sans taper de commande

> Pour qui : le pilote qui a déjà un **NAS Synology** allumé en permanence et qui veut y mettre Corde serveur
> au lieu d'acheter un Raspberry Pi. Tout se fait dans l'interface du NAS (DSM), à la souris. Compte
> 45 minutes la première fois.
> Ce que fait Corde serveur, et le Raspberry Pi : [SERVEUR.md](SERVEUR.md). Accès depuis l'extérieur :
> [ACCES-EXTERIEUR.md](ACCES-EXTERIEUR.md) (à lire **après** ce guide). Sécurité :
> [SECURITY-MODEL.md §7](SECURITY-MODEL.md#7-le-serveur-corde-mode-hub). Décision :
> [ADR 0017](adr/0017-corde-serveur-en-conteneur.md).
>
> **Les noms des menus** sont ceux de DSM 7.2 en français tels qu'ils apparaissent dans des guides
> publiés ; Synology les change parfois d'une version à l'autre. Si un nom diffère un peu chez toi, cherche
> le plus proche : l'étape reste la même. Quand un nom n'a pas pu être vérifié, l'étape décrit ce qu'il faut
> faire sans le citer.

## En bref

```text
  PC de jeu (Corde)  ── chaque session terminée ──▶  NAS Synology (Corde serveur, dans Container Manager)
        │                                                     │
   le direct (en roulant)                       sessions, ingénieur, profil, courses, rappels
        ▼                                                     ▼
  téléphone (même Wi-Fi)                        téléphone (à la maison, ou dehors par ton VPN)
```

Corde serveur est exactement le même programme que sur un Raspberry Pi ([SERVEUR.md](SERVEUR.md)) : seule
l'installation change. Sur le NAS, il tourne dans un **conteneur** (une boîte isolée gérée par l'application
**Container Manager** de Synology) :

- tu télécharges **un fichier** (l'« image » de Corde serveur) sur la page des versions de Corde ;
- tu l'**importes** dans Container Manager, puis tu crées un **projet** en collant un petit texte tout prêt ;
- le **premier code d'appairage** s'affiche dans le **journal** du conteneur : pas besoin de terminal ;
- tes données restent dans un dossier du NAS, que les mises à jour ne touchent pas.

Comme sur le Pi : le serveur n'est **jamais** ouvert sur Internet ; depuis l'extérieur, on passe par un VPN.

## 1. Ce qu'il faut

| Élément | Détail |
|---|---|
| **Un NAS Synology à processeur Intel ou AMD** | c'est le cas des modèles « plus » (DS220+, DS224+, DS920+…) et de la plupart des modèles pour petites entreprises. Fichier à prendre : **`…-docker-x64.tar.gz`**. Pour connaître ton processeur : la fiche technique de ton modèle sur le site de Synology (ligne « Processeur »), ou le centre d'informations de DSM (Panneau de configuration). |
| **Le paquet Container Manager** | DSM 7.2 ou plus récent. Sous DSM 7.0 / 7.1, il s'appelait **Docker** : voir [§ 15](#15-dsm-70-ou-71--le-paquet-docker). |
| **Une adresse fixe pour le NAS** | dans ta box, réserve une adresse au NAS (« bail statique » ou « réservation DHCP »), sinon le PC ne le retrouvera plus. Dans ce guide : `192.168.1.50` (exemple, remplace par la tienne). |
| **De la mémoire** | Corde serveur utilise quelques dizaines de Mo ; le projet le limite à 512 Mo (largement assez sur un NAS de 2 Go). |
| **Corde sur le PC** | la même version que l'image, ou plus ancienne (le serveur doit être au moins aussi récent que le PC). |

**Les NAS qui ne peuvent pas** :

- ceux qui n'ont **pas Container Manager** (ni Docker) dans le Centre de paquets : la plupart des modèles
  d'entrée de gamme anciens et tous les modèles à processeur ARM 32 bits. Prends un Raspberry Pi
  ([SERVEUR.md](SERVEUR.md)) ;
- les modèles **à processeur ARM 64 bits** auxquels Synology a ouvert Container Manager (certains modèles
  récents des séries « j » ou « value ») : le fichier **`…-docker-arm64.tar.gz`** est fait pour eux, mais il
  n'a **jamais été essayé** sur un tel NAS. Le guide s'applique tel quel ; si ça ne marche pas, signale-le
  (et le Raspberry Pi reste la voie sûre).

Exemple : un DS220+ a un processeur Intel (Celeron J4025) : fichier **x64**.

## 2. Installer Container Manager

1. Ouvre DSM dans le navigateur du PC (l'adresse du NAS) et connecte-toi avec un compte **administrateur**.
2. Ouvre le **Centre de paquets**.
3. Cherche **Container Manager** (champ de recherche en haut), puis clique sur **Installer**.
4. S'il demande sur quel volume l'installer, garde **Volume 1** (ou ton seul volume).

L'installation crée en général un **dossier partagé `docker`** : c'est là qu'on range Corde. S'il n'existe
pas, crée-le : **Panneau de configuration** → **Dossier partagé** → **Créer**, nom `docker`, sur le
volume 1, sans rien changer d'autre.

## 3. Créer le dossier de Corde

1. Ouvre **File Station**.
2. Ouvre le dossier partagé **`docker`**.
3. Crée un dossier (bouton de création de dossier, ou clic droit dans la liste) nommé **`corde`**.
4. Ouvre `corde` et crée dedans un dossier nommé **`data`**.

Résultat : `docker/corde/data`. Le dossier `data` gardera tout (sessions, appareils appairés, courses) ;
Corde serveur se l'attribue tout seul au premier démarrage (il appartient ensuite à son compte « corde »,
numéro 8471). Ne mets rien d'autre dedans.

## 4. Télécharger l'image de Corde serveur

1. Sur le PC, ouvre la [dernière version de Corde](https://github.com/dultra123/corde/releases/latest)
   (section *Assets* en bas de la page).
2. Télécharge **`corde-server-<version>-docker-x64.tar.gz`** (par exemple
   `corde-server-1.4.0-docker-x64.tar.gz`, environ 40 Mo ; le numéro change à chaque version). **Ne le
   décompresse pas** : le NAS le lit tel quel.

Les images apparaissent à partir de la première version publiée après l'arrivée de ce guide ; la pré-release
« Corde — dernière version » les a aussi.

## 5. Importer l'image dans Container Manager

1. Ouvre **Container Manager**, section **Image** (menu de gauche).
2. Ouvre le menu **Action**, puis **Importer**, et choisis l'import **à partir d'un fichier** venant de
   **ton ordinateur** (le PC sur lequel tu as téléchargé le fichier ; l'autre choix sert pour un fichier
   déjà posé sur le NAS).
3. Sélectionne `corde-server-<version>-docker-x64.tar.gz` et valide. L'envoi prend une minute environ.
4. L'image **`corde-server`**, étiquette **`<version>`** (ex. `1.4.0`), apparaît dans la liste.

Si l'import refuse le fichier compressé : décompresse-le une fois sur le PC (clic droit → Extraire, ou
7-Zip) pour obtenir `corde-server-<version>-docker-x64.tar`, puis importe ce `.tar`.

## 6. Créer le projet Corde

Un **projet** est un conteneur décrit par un petit texte (format « compose ») : on le colle une fois, et
Container Manager s'occupe du reste.

1. Dans Container Manager, section **Projet**, clique sur **Créer**.
2. **Nom du projet** : `corde`.
3. **Chemin** : choisis le dossier **`docker/corde`** créé au § 3.
4. **Source** : l'option qui **crée** le fichier compose (son nom varie selon la version :
   « Créer docker-compose.yml », « Créer compose.yml »…). Une zone de texte s'ouvre.
5. Colle ce texte, puis **remplace `1.4.0` par le numéro de l'image que tu as importée** (§ 5) :

   ```yaml
   version: "2.4"

   services:
     corde:
       image: corde-server:1.4.0
       container_name: corde
       restart: unless-stopped
       ports:
         - "8471:8471"
       volumes:
         - ./data:/var/lib/corde
       environment:
         CORDE_FIRST_PAIRING_CODE: "1"
         CORDE_ALLOW_HOST: ""
       read_only: true
       tmpfs:
         - /tmp:size=16m,mode=1777
       cap_drop:
         - ALL
       cap_add:
         - CHOWN
         - DAC_READ_SEARCH
         - SETUID
         - SETGID
       security_opt:
         - "no-new-privileges:true"
       mem_limit: 512m
       pids_limit: 256
   ```

   Ce texte est le fichier [`packaging/docker/compose.yaml`](../packaging/docker/compose.yaml) du dépôt
   (avec des commentaires qui expliquent chaque ligne). En résumé : le port **8471** du NAS mène à Corde ;
   les données vont dans `docker/corde/data` ; le conteneur **redémarre tout seul** (après une coupure ou
   un redémarrage du NAS) sauf si tu l'as arrêté toi-même ; le reste l'enferme (lecture seule, aucun droit
   particulier, mémoire limitée).
6. **Suivant**. S'il propose des **paramètres du portail web** (Web Station) : **ne coche rien**, Corde n'en
   a pas besoin (et ce portail servirait à publier le service).
7. **Suivant**, puis **Effectué** (ou **Terminé**) : le projet se construit et démarre. Au bout de quelques
   secondes, le conteneur `corde` est **vert** (en cours d'exécution).

## 7. Lire le premier code d'appairage

Tant qu'**aucun appareil** n'est appairé, Corde serveur écrit à chaque démarrage un code d'appairage dans
son journal.

1. Container Manager → section **Conteneur** → ouvre le conteneur **`corde`** (double-clic).
2. Ouvre l'onglet du **journal** du conteneur. Tu y trouves un cadre comme celui-ci :

   ```text
     ==========================================================================
      PREMIER APPAIRAGE : aucun appareil n'est encore appairé à ce serveur.

      Code d'appairage : 4815 1623   (l'appareil sera administrateur)

      Valable 10 minutes, une seule fois : jusqu'à 14:32 UTC le 15/11/2026
      (heure UTC : en France et en Suisse, ajoute 1 h en hiver, 2 h en été).
      Trois codes faux l'annulent.
     ...
   ```

3. Tu as **10 minutes** pour le taper (§ 8). Trop tard, ou trois codes faux ? **Redémarre le conteneur**
   (section Conteneur → `corde` → menu Action → redémarrer, ou arrêter puis démarrer) : un nouveau code
   s'affiche, tant qu'aucun appareil n'est appairé.

Une fois un appareil appairé, plus aucun code ne s'affiche (c'est voulu). Les adresses en `172.…` en haut du
journal sont internes au conteneur : ne t'en sers pas, utilise celle du NAS.

## 8. Ouvrir Corde serveur et appairer le PC

Comme sur un Raspberry Pi ([SERVEUR.md §4](SERVEUR.md#4-appairer-le-pc)), en remplaçant l'adresse du Pi par
celle du NAS :

1. Sur le PC, dans Corde : **Réglages** (roue dentée) → section **Serveur Corde** → **Adresse du serveur**
   (`192.168.1.50`, le port 8471 est ajouté tout seul) → **Code d'appairage** (celui du journal) →
   **Appairer**. Cette section n'apparaît que sur le PC lui-même (pas depuis le téléphone).
2. L'état passe à **Connecté** et le PC envoie aussitôt tout ce qu'il a déjà enregistré (la première fois,
   cela peut prendre quelques minutes : « En attente d'envoi » décompte). Ensuite il envoie chaque session
   dès qu'elle se termine.

Le premier appareil appairé devient **administrateur** du serveur (il peut révoquer les autres, page
**Appareils**). Si tu préfères que ce soit ton téléphone, appaire-le en premier avec ce code (§ 9).

Pour vérifier depuis le PC : ouvre `http://192.168.1.50:8471` dans le navigateur. Tant que l'appareil n'est
pas appairé, la page **Serveur Corde** demande un code : c'est normal.

## 9. Appairer le téléphone

Il faut un **nouveau code** : le premier a servi au PC, et le journal n'en donne plus une fois un appareil
appairé. Un appareil appairé ne peut pas en fabriquer (c'est voulu : un code ne s'obtient que sur le serveur
lui-même). Obtiens-le avec **une commande dans le terminal du conteneur** (§ 10), depuis le navigateur du PC.

Puis, comme sur le Pi ([SERVEUR.md §5](SERVEUR.md#5-appairer-le-téléphone)) : le téléphone sur le Wi-Fi de la
maison, ouvre `http://192.168.1.50:8471`, la page **Serveur Corde** demande un code : tape-le avec un nom
(« iPhone », « Téléphone »…) → **Appairer cet appareil**. Ajoute la page à l'écran d'accueil pour l'ouvrir
comme une application. L'application Corde pour iPhone ou Android s'appaire de la même façon (« Mes
serveurs »).

> **L'appareil est appairé pour une adresse.** Ouvre toujours le serveur par la **même adresse**
> (`http://192.168.1.50:8471`), y compris dehors à travers le VPN ([ACCES-EXTERIEUR.md](ACCES-EXTERIEUR.md)).

## 10. Appairer d'autres appareils (terminal du conteneur)

Container Manager ouvre un **terminal** dans le conteneur, depuis le navigateur (pas besoin de SSH) :

1. Container Manager → **Conteneur** → ouvre **`corde`** → onglet du **terminal**.
2. Crée une session de terminal (bouton de création ; si on te demande une commande, prends `bash`).
3. Tape, puis Entrée :

   ```text
   corde-server pair
   ```

   Le code s'affiche (valable 10 minutes, une seule fois). `corde-server pair --admin` fait de l'appareil un
   administrateur.
4. Ferme la session de terminal.

Les autres commandes de [SERVEUR.md §6](SERVEUR.md#6-les-appareils) marchent aussi, **sans `sudo`** :
`corde-server devices` (liste) et `corde-server revoke 3f9a1c2e` (révoque l'appareil `3f9a1c2e` tout de
suite). La révocation se fait aussi sans terminal : page **Appareils** du serveur, depuis un appareil
administrateur.

**Téléphone perdu ou volé** : révoque-le, **et** retire sa clé du VPN
([ACCES-EXTERIEUR.md](ACCES-EXTERIEUR.md#téléphone-perdu-ou-volé)).

## 11. Mettre à jour

Le serveur doit avoir **la même version que Corde sur le PC, ou plus récente** (sinon le PC affiche
« Serveur à mettre à jour » et attend, sans rien perdre).

1. Télécharge le nouveau `corde-server-<version>-docker-x64.tar.gz` (§ 4) et **importe-le** (§ 5) : une
   seconde étiquette (le nouveau numéro) apparaît à côté de l'ancienne.
2. Container Manager → **Projet** → `corde` : **arrête** le projet.
3. Modifie son texte (l'onglet qui montre le fichier YAML du projet) : remplace l'ancien numéro par le
   nouveau sur la ligne `image: corde-server:…`, et enregistre.
4. **Reconstruis** puis démarre le projet (selon la version de Container Manager, l'enregistrement le
   propose directement, ou c'est une action du projet : construire / démarrer).
5. Vérifie dans le journal (§ 7) la ligne `corde-server <nouvelle version> — serveur Corde (rôle hub)`.

Les données (`docker/corde/data`) ne bougent pas : les appareils restent appairés. Quand tout marche, tu peux
supprimer l'**ancienne** image (section **Image**, l'ancienne étiquette) pour libérer de la place. Il n'y a
pas de mise à jour automatique.

## 12. Sauvegarder, déménager

Tout est dans **`docker/corde/data`** : la base `corde.db`, les fichiers d'échantillons `sessions/`, les
appareils `hub/`, les courses `races/`. Ces fichiers appartiennent au compte du conteneur et ne sont lisibles
que par lui : c'est voulu.

**Sauvegarder** — deux façons :

- **avec Hyper Backup** (l'application de sauvegarde de Synology) : ajoute le dossier `docker/corde` à une
  tâche de sauvegarde. Pour une copie parfaitement cohérente, arrête le projet le temps de la sauvegarde ;
- **une archive à emporter**, au même format que celle d'un Raspberry Pi : dans le terminal du conteneur
  (§ 10), PC Corde fermé et sans te servir du téléphone pendant ce temps, tape

  ```text
  corde-sauvegarde
  ```

  Il écrit `corde-sauvegarde-AAAA-MM-JJ-HHMM.tar.gz` dans `docker/corde/data`. Télécharge-la avec File
  Station, puis **efface-la du NAS**.

Garde une sauvegarde **privée** : elle contient tes sessions, les noms des autres pilotes des résultats
importés et la liste des appareils (clés sous forme hachée, inutilisables telles quelles).

**Du NAS vers un Raspberry Pi (ou un autre serveur Linux)** : fais l'archive `corde-sauvegarde` ci-dessus,
installe Corde serveur sur le Pi ([SERVEUR.md §3](SERVEUR.md#3-installer-corde-serveur)), puis restaure-la
comme une sauvegarde du Pi ([SERVEUR.md §8](SERVEUR.md#8-sauvegarder)). Arrête ensuite le projet du NAS
(deux serveurs avec les mêmes appareils, c'est un de trop).

**D'un Raspberry Pi vers le NAS** :

1. Sur le Pi, fais la sauvegarde de [SERVEUR.md §8](SERVEUR.md#8-sauvegarder) et récupère-la sur le PC.
2. Sur le NAS, **arrête** le projet `corde` (s'il existe déjà).
3. Avec File Station, envoie l'archive dans `docker/corde`, puis extrais-la au même endroit (clic droit sur
   l'archive → extraire) : un dossier `docker/corde/corde` apparaît.
4. Renomme l'ancien `data` (par exemple `data-ancien`), puis renomme `corde` en **`data`**.
5. Démarre le projet : Corde serveur s'attribue les fichiers copiés et retrouve tes sessions et tes
   appareils appairés.
6. L'adresse a changé (celle du NAS au lieu de celle du Pi) : sur le PC, **Réglages → Serveur Corde**, mets
   l'adresse du NAS (si Corde redemande un code, appaire de nouveau : § 10). Un navigateur de téléphone
   garde sa clé par adresse : ouvre `http://192.168.1.50:8471` et appaire-le de nouveau (§ 9).
7. Quand tout marche, supprime `data-ancien` et l'archive, et éteins Corde serveur sur le Pi
   (`sudo systemctl disable --now corde`).

**Le plus simple, sans copier** : le PC garde tout. Sur un serveur neuf, appairer le PC puis **Tout renvoyer**
(Réglages → Serveur Corde) recrée tout, sauf les ressentis saisis seulement sur le serveur, les choix de
courses du serveur et les sessions supprimées du PC.

## 13. Arrêter, désinstaller

- **Arrêter** : Container Manager → **Projet** → `corde` → arrêter. Il ne redémarre plus tout seul (même
  après un redémarrage du NAS) tant que tu ne le démarres pas.
- **Désinstaller** : arrête le projet, supprime-le (Container Manager propose aussi de le **nettoyer**, ce
  qui retire le conteneur), puis supprime l'image `corde-server` (section **Image**). Le dossier
  `docker/corde/data` reste : supprime-le avec File Station seulement si tu ne veux plus tes données.
- Sur le PC : Réglages → Serveur Corde → **Dissocier**.

## 14. Dépannage

| Ce que tu vois | Quoi faire |
|---|---|
| Le projet ne démarre pas : **port déjà utilisé** (« port is already allocated », « address already in use ») | Une autre application du NAS prend 8471. Dans le texte du projet, remplace `"8471:8471"` par `"8472:8471"` (seul le nombre de **gauche** change), reconstruis, et utilise `http://192.168.1.50:8472` partout (sur le PC : adresse `192.168.1.50:8472`). |
| Le conteneur redémarre en boucle ; le journal dit **« impossible de donner /var/lib/corde au compte « corde » »** ou **« impossible de rendre … inscriptible »** | Le dossier de données n'est pas utilisable. Vérifie que le projet est bien dans `docker/corde` et que le dossier `data` existe. Sinon : arrête le projet, renomme `data` en `data-ancien`, crée un dossier `data` **vide** avec File Station, redémarre. Ne retire pas `CHOWN`, `DAC_READ_SEARCH`, `SETUID` ni `SETGID` de la liste `cap_add` : ils servent au démarrage. |
| Le journal dit **« compte « corde » absent de l'image »** ou **« programme introuvable »** | Le fichier importé est abîmé : supprime l'image, retélécharge-la, réimporte-la. |
| Le navigateur affiche **« hôte non autorisé »** | Tu as ouvert le serveur par un **nom** (le nom du NAS, un nom donné par le VPN…) et non par son adresse. Ouvre-le par l'adresse (`http://192.168.1.50:8471`), ou ajoute ce nom dans le texte du projet : `CORDE_ALLOW_HOST: "mon-nas.local"` (plusieurs noms : séparés par des virgules, ex. `"mon-nas,mon-nas.local"`), puis reconstruis. Lettres, chiffres, points et tirets seulement : un autre caractère et le conteneur refuse de démarrer (le journal le dit). |
| Le PC (ou le téléphone) **n'atteint pas** le serveur alors que le conteneur est vert | Le **pare-feu de DSM** est peut-être activé (Panneau de configuration, sécurité, pare-feu) : ajoute une règle qui **autorise le port TCP 8471** (ou celui que tu as choisi) **depuis ton réseau local seulement** (par exemple `192.168.1.0/24`). Jamais « toutes les adresses » si le NAS est joignable depuis Internet. |
| Pas de code dans le journal | Un appareil est déjà appairé (c'est voulu), ou `CORDE_FIRST_PAIRING_CODE` vaut `"0"`. Utilise le terminal (§ 10). |
| « Code incorrect » / « Ce code a expiré » | Redémarre le conteneur (§ 7) s'il n'y a encore aucun appareil, sinon `corde-server pair` (§ 10). |
| Le PC dit **« Serveur à mettre à jour »** | Importe l'image de la même version que Corde sur le PC (§ 11). |

## 15. DSM 7.0 ou 7.1 : le paquet Docker

Avant DSM 7.2, Container Manager s'appelait **Docker** (même Centre de paquets). Différences :

- **Pas de section « Projet »** : on crée le conteneur avec l'assistant. Section **Image** → importe le
  fichier (menu **Ajouter**, ajout depuis un fichier) → sélectionne l'image → lance-la ; dans les
  **paramètres avancés** : redémarrage automatique activé ; dossier `docker/corde/data` monté sur
  **`/var/lib/corde`** ; port local **8471** vers le port du conteneur **8471** (TCP) ; laisse les variables
  d'environnement telles quelles (ajoute `CORDE_ALLOW_HOST` seulement si tu en as besoin).
- Les options d'enfermement du texte compose (lecture seule, liste des capacités, mémoire) ne se règlent pas
  dans cet assistant. Corde serveur tourne quand même sous son compte « corde », sans droits ; c'est un peu
  moins enfermé qu'avec un projet. Passer à DSM 7.2 est conseillé.
- Le journal et le terminal sont dans les détails du conteneur, comme plus haut.

Ces écrans n'ont pas pu être vérifiés : suis l'idée de chaque réglage plutôt que les noms exacts.

## 16. Un autre NAS ou une autre machine avec Docker

L'image n'a rien de propre à Synology. Sur tout système avec Docker et Docker Compose (autre marque de NAS,
mini-PC Linux…) : importe l'image (`docker load < corde-server-<version>-docker-x64.tar.gz`), copie
[`packaging/docker/compose.yaml`](../packaging/docker/compose.yaml) dans un dossier, mets le bon numéro sur la
ligne `image:`, crée le sous-dossier `data`, puis `docker compose up -d`. Journal : `docker logs corde` ;
commandes : `docker exec -it corde corde-server pair` ; sauvegarde : `docker exec corde corde-sauvegarde`.
Une interface graphique de NAS qui accepte un fichier compose (souvent appelé « stack » ou « projet ») prend
le même texte.

## 17. Depuis l'extérieur de la maison

Les règles de [ACCES-EXTERIEUR.md](ACCES-EXTERIEUR.md) valent pour le NAS comme pour le Pi :

- **Ne redirige jamais le port 8471** (ni celui que tu as choisi) depuis ta box. Corde serveur parle en HTTP
  simple et n'est pas fait pour Internet.
- **N'expose pas Corde par les services d'accès à distance du NAS** : ni **QuickConnect**, ni le **proxy
  inversé** de DSM, ni le **portail web** de Container Manager, ni un nom **DDNS** qui mènerait au port de
  Corde. Ils publieraient Corde sur Internet.
- **On entre chez soi par un VPN**, puis on ouvre `http://192.168.1.50:8471` comme à la maison. Les voies
  A, B et C de [ACCES-EXTERIEUR.md](ACCES-EXTERIEUR.md) marchent telles quelles (dans la voie A, le
  Raspberry Pi qui fait serveur WireGuard peut être n'importe quelle machine Linux de la maison, pas le NAS).
  Le paquet **VPN Server** de Synology fait du NAS lui-même le serveur VPN de la maison : c'est l'équivalent
  de la **voie B** (la box qui fait serveur VPN), avec le NAS à la place de la box. Il propose OpenVPN,
  L2TP/IPsec et PPTP (pas WireGuard) : prends **OpenVPN**, jamais PPTP (chiffrement dépassé). Seul le port
  du VPN est alors redirigé par la box, jamais celui de Corde ; suis la documentation de Synology pour le
  régler, et applique la liste de vérification finale d'[ACCES-EXTERIEUR.md](ACCES-EXTERIEUR.md).

## 18. Ce qui protège tes données (résumé)

- Le serveur ne répond qu'aux **appareils appairés** ; seuls l'écran de code et l'envoi du code sont
  ouverts. Un code sert une fois, expire en 10 minutes, meurt après 3 erreurs, et les essais sont limités.
- **Le premier code est écrit dans le journal du conteneur**, et seulement tant qu'aucun appareil n'est
  appairé. Qui peut lire ce journal (les **administrateurs du NAS**) peut donc appairer un appareil pendant
  ces 10 minutes : c'est le même pouvoir que lancer `corde-server pair` sur le serveur. Ne donne pas de
  compte administrateur du NAS à quelqu'un à qui tu ne donnerais pas tes sessions. Pour ne jamais écrire de
  code dans le journal : `CORDE_FIRST_PAIRING_CODE: "0"` (et terminal, § 10).
- Dans le conteneur, Corde serveur tourne sous un compte sans droits (`corde`, 8471), système de fichiers en
  lecture seule sauf ses données, sans aucune capacité, mémoire limitée.
- **Limites** : trafic en HTTP simple sur le réseau local (pas chiffré) ; un appareil appairé volé garde
  l'accès jusqu'à sa révocation ; un administrateur du NAS a tout (il lit les données sur le disque). Dehors,
  le VPN chiffre tout. Détails : [SECURITY-MODEL.md §7](SECURITY-MODEL.md#7-le-serveur-corde-mode-hub).
