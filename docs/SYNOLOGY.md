# Corde serveur sur un NAS Synology, sans taper de commande

> Pour qui : le pilote qui a déjà un **NAS Synology** allumé en permanence et qui veut y mettre Corde serveur
> au lieu d'acheter un Raspberry Pi. Tout se fait dans l'interface du NAS (DSM), à la souris. Compte
> 45 minutes la première fois.
> Ce que fait Corde serveur, et le Raspberry Pi : [SERVEUR.md](SERVEUR.md). Accès depuis l'extérieur :
> [ACCES-EXTERIEUR.md](ACCES-EXTERIEUR.md) (à lire **après** ce guide). Sécurité :
> [SECURITY-MODEL.md §7](SECURITY-MODEL.md#7-le-serveur-corde-mode-hub). Décision :
> [ADR 0017](adr/0017-corde-serveur-en-conteneur.md).
>
> **Les noms des menus** des §2 à 7 et 11 ont été relevés sur un vrai NAS (DS220+, DSM 7.4.1, Container
> Manager 24.0.2, novembre 2026), en français. Synology les change parfois d'une version à l'autre : si un
> nom diffère un peu chez toi, cherche le plus proche, l'étape reste la même. Là où un nom n'a pas été
> vérifié (terminal, suppression, sauvegarde, DSM 7.0 / 7.1), l'étape décrit ce qu'il faut faire sans le
> citer.

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
4. **Configurer le réseau bridge** : garde le sous-réseau proposé (`172.17.0.1/16`, « Par défaut »), puis
   **Suivant**. Ne le change que si ton réseau à la maison utilise déjà des adresses en `172.17.…`.
5. **Confirmer les paramètres** : laisse « Lancer après l'installation » coché, puis **Effectué**.

L'installation crée un **dossier partagé `docker`** : c'est là qu'on range Corde. S'il n'existe pas, crée-le :
**Panneau de configuration** → **Dossier partagé** → **Créer**, nom `docker`, sur le volume 1, sans rien
changer d'autre.

**NAS tout neuf (ou réinstallé) ?** Le Centre de paquets répond « Vous n'avez créé aucun groupe de stockage
ni volume » : il faut d'abord préparer le disque. Clique sur **Lancer** (ou ouvre le **Gestionnaire de
stockage** → **Créer maintenant**), puis suis l'assistant en gardant ce qu'il propose : **Démarrer** → type
de RAID **SHR** → coche ton ou tes disques → contrôle du disque (au choix) → taille : **Max** → **Btrfs
(recommandé)** → chiffrement : laisse décoché → **Appliquer** → **OK**. **Cette étape efface tout ce qui se
trouve sur les disques cochés.** Attends que le groupe de stockage et le volume soient « Sain », puis que DSM
finisse d'installer ses propres paquets (sinon : « Un autre package est en cours de configuration,
d'installation ou de restauration », réessaie une minute plus tard). Avec un seul disque, rien ne protège tes
données d'une panne de ce disque : SHR permet d'en ajouter un second plus tard, en miroir.

## 3. Créer le dossier de Corde

1. Ouvre **File Station**.
2. Ouvre le dossier partagé **`docker`**.
3. **Créer** → **Créer dossier**, nom **`corde`**, **OK**.
4. Ouvre `corde` (double-clic) et crée dedans, de la même façon, un dossier nommé **`data`**.

Résultat : `docker/corde/data`. Le dossier `data` gardera tout (sessions, appareils appairés, courses) ;
Corde serveur se l'attribue tout seul au premier démarrage (il appartient ensuite à son compte « corde »,
numéro 8471). Ne mets rien d'autre dedans.

## 4. Télécharger l'image de Corde serveur

1. Sur le PC, ouvre la [dernière version de Corde](https://github.com/dultra123/corde/releases/latest)
   (section *Assets* en bas de la page).
2. Télécharge **`corde-server-<version>-docker-x64.tar.gz`** (par exemple
   `corde-server-1.5.0-docker-x64.tar.gz`, environ 40 Mo ; le numéro change à chaque version). **Ne le
   décompresse pas** : le NAS le lit tel quel.

Les images apparaissent à partir de la première version publiée après l'arrivée de ce guide ; la pré-release
« Corde — dernière version » les a aussi.

## 5. Importer l'image dans Container Manager

1. Ouvre **Container Manager**, section **Image** (menu de gauche).
2. Clique sur **Importer** (s'il y a déjà des images dans la liste : menu **Action** → **Importer**), puis
   **Ajouter à partir d'un fichier** → **À partir d'un périphérique local** (le PC sur lequel tu as
   téléchargé le fichier ; « À partir de ce DSM » sert pour un fichier déjà posé sur le NAS).
3. Sélectionne `corde-server-<version>-docker-x64.tar.gz` et valide. L'envoi prend une minute environ.
4. L'image **`corde-server`** apparaît dans la liste, avec son numéro dans la colonne **Identification**
   (ex. `1.5.0`).

Si l'import refuse le fichier compressé : décompresse-le une fois sur le PC (clic droit → Extraire, ou
7-Zip) pour obtenir `corde-server-<version>-docker-x64.tar`, puis importe ce `.tar`.

## 6. Créer le projet Corde

Un **projet** est un conteneur décrit par un petit texte (format « compose ») : on le colle une fois, et
Container Manager s'occupe du reste.

1. Dans Container Manager, section **Projet**, clique sur **Créer** : l'écran **Paramètres généraux**
   s'ouvre.
2. **Nom du projet** : `corde`.
3. **Chemin** : clique sur **Définir le chemin**, déplie `docker`, choisis **`corde`** (créé au § 3), puis
   **Sélectionnez**. Le champ affiche `/docker/corde`.
4. **Source** : choisis **Créer un fichier docker-compose.yml**. Une zone de texte s'ouvre en dessous.
5. Colle ce texte (Ctrl + V), puis **remplace `1.5.0` par le numéro de l'image que tu as importée** (§ 5) :

   ```yaml
   version: "2.4"

   services:
     corde:
       image: corde-server:1.5.0
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
6. **Suivant**. Écran **Paramètres du portail Web** : laisse « Configurer le portail Web via Web Station »
   **décoché**, Corde n'en a pas besoin (et ce portail servirait à publier le service).
7. **Suivant**. Écran **Résumé** : laisse « Démarrer le projet dès sa création » coché, puis **Effectué**.
8. Une fenêtre **Terminal : Créer - corde** montre la création. Elle doit finir par `Container corde
   Started` et `Exit Code: 0` ; clique sur **Fermer**. La ligne « Your kernel does not support PIDs limit
   capabilities… PIDs limit discarded » est normale sur un NAS Synology (une des limites du texte n'y est
   pas prise en charge, les autres le sont).

Dans la section **Conteneur**, `corde` a maintenant une pastille **verte** ; en l'ouvrant (double-clic), tu
lis « En cours d'exécution », puis « Sain » au bout de quelques secondes.

Si la fenêtre affiche `Top-level object must be a mapping` : le texte n'a pas été enregistré (zone restée
vide). Ouvre le projet (double-clic) → onglet **Configurations YAML**, colle le texte, **Sauvegarder** →
« Créer et démarrer le projet (reconstruire l'image) » → **OK**.

## 7. Lire le premier code d'appairage

Tant qu'**aucun appareil** n'est appairé, Corde serveur écrit à chaque démarrage un code d'appairage dans
son journal.

1. Container Manager → section **Conteneur** → ouvre le conteneur **`corde`** (double-clic).
2. Ouvre l'onglet **Journal** (à côté de **Général** et **Statistiques**). DSM y affiche **la ligne la plus
   récente en haut** : le cadre se lit donc de bas en haut. Cherche simplement la ligne **« Code
   d'appairage : … »** (fais défiler un peu, ou tape `Code` dans le champ **Rechercher** du journal). Dans
   l'ordre où Corde l'écrit, le cadre est celui-ci :

   ```text
     ==========================================================================
      PREMIER APPAIRAGE : aucun appareil n'est encore appairé à ce serveur.

      Code d'appairage : 4815 1623   (l'appareil sera administrateur)

      Valable 10 minutes, une seule fois : jusqu'à 14:32 UTC le 15/11/2026
      (heure UTC : en France et en Suisse, ajoute 1 h en hiver, 2 h en été).
      Trois codes faux l'annulent.
     ...
   ```

3. Tu as **10 minutes** pour le taper (§ 8). Trop tard, ou trois codes faux ? **Redémarre le conteneur** :
   dans le détail du conteneur, **Action** → **Redémarrer**, puis l'icône d'actualisation en bas à droite du
   journal. Un nouveau code s'affiche, tant qu'aucun appareil n'est appairé.

Une fois un appareil appairé, plus aucun code ne s'affiche (c'est voulu). Les adresses en `172.…` en haut du
journal sont internes au conteneur : ne t'en sers pas, utilise celle du NAS.

## 8. Ouvrir Corde serveur et appairer le PC

Comme sur un Raspberry Pi ([SERVEUR.md §4](SERVEUR.md#4-appairer-le-pc)), en remplaçant l'adresse du Pi par
celle du NAS :

1. Sur le PC, dans Corde : **Réglages** (roue dentée) → section **Serveur Corde** → **Adresse du serveur**
   (`192.168.1.50`, le port 8471 est ajouté tout seul) → **Code d'appairage** (celui du journal) →
   **Appairer**. Cela se fait **dans Corde sur le PC**, pas dans l'application du téléphone (le téléphone
   s'appaire autrement, § 9).
2. L'état passe à **Envoi en cours…** : le PC envoie tout ce qu'il a déjà enregistré (la première fois, cela
   peut prendre plusieurs minutes), puis à **Connecté**. Ensuite il envoie chaque session dès qu'elle se
   termine.

Le premier appareil appairé devient **administrateur** du serveur (il peut révoquer les autres, page
**Appareils**). Si tu préfères que ce soit ton téléphone, appaire-le en premier avec ce code (§ 9).

Pour vérifier depuis le PC : ouvre `http://192.168.1.50:8471` dans le navigateur. Tant que l'appareil n'est
pas appairé, la page **Serveur Corde** demande un code : c'est normal.

## 9. Appairer le téléphone

Il faut un **nouveau code** : le premier a servi au PC, et le journal n'en donne plus une fois un appareil
appairé. Un appareil appairé ne peut pas en fabriquer (c'est voulu : un code ne s'obtient que sur le serveur
lui-même). Obtiens-le avec **une commande dans le terminal du conteneur** (§ 10), depuis le navigateur du PC.

Puis, le téléphone sur le Wi-Fi de la maison :

- **avec l'application Corde** (iPhone ou Android) : **Accueil** → **Mes serveurs** → **Ajouter un serveur** →
  **Adresse** : `192.168.1.50:8471` → **Ajouter** → **Appairer ce téléphone** → **Code d'appairage** et
  **Nom de ce téléphone** → **Appairer** → **Utiliser**. Le PC de jeu reste dans la liste : c'est lui qui
  donne le direct quand tu roules, le NAS donne tes sessions quand le PC est éteint ;
- **ou dans le navigateur**, comme sur le Pi ([SERVEUR.md §5](SERVEUR.md#5-appairer-le-téléphone)) : ouvre
  `http://192.168.1.50:8471`, la page **Serveur Corde** demande un code : tape-le avec un nom (« iPhone »,
  « Téléphone »…) → **Appairer cet appareil**. Ajoute la page à l'écran d'accueil pour l'ouvrir comme une
  application.

> **L'appareil est appairé pour une adresse.** À la maison, ouvre toujours le serveur par la **même
> adresse** (`http://192.168.1.50:8471`). Dehors, tout dépend du VPN (§ 17) : avec un VPN qui te remet dans
> le réseau de la maison, c'est la même adresse ; avec un VPN maillé, le NAS a une **seconde adresse**
> (`100.x.y.z`) et il faut un **second appairage** pour celle-ci.

## 10. Appairer d'autres appareils (terminal du conteneur)

Container Manager ouvre un **terminal** dans le conteneur, depuis le navigateur (pas besoin de SSH) :

1. Container Manager → **Conteneur** → ouvre **`corde`** → **Action** → **Ouvrir le terminal**. Une fenêtre
   **Terminal** s'ouvre ; sa liste de gauche contient déjà une ligne **`corde`** (ce que le serveur écrit,
   comme le journal).
2. Dans cette fenêtre : petite flèche à droite de **Créer** → **Lancer avec une commande**.
3. Sous **« Veuillez saisir une commande : »**, tape la commande ci-dessous, puis **OK** :

   ```text
   corde-server pair
   ```

4. Une ligne **`corde-server`** apparaît dans la liste de gauche : **clique dessus**. Le code s'affiche à
   droite (« Code d'appairage : … »), valable 10 minutes, une seule fois. `corde-server pair --admin` fait
   de l'appareil un administrateur.
5. Ferme la fenêtre Terminal (la croix en haut à droite).

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
2. Container Manager → **Projet** → ouvre `corde` (double-clic) → bouton **Arrêt** (tant que le projet
   tourne, son texte est en lecture seule).
3. Onglet **Configurations YAML** : remplace l'ancien numéro par le nouveau sur la ligne
   `image: corde-server:…`, puis **Sauvegarder**.
4. DSM demande quoi faire ensuite : garde **Créer et démarrer le projet (reconstruire l'image)**, puis
   **OK**. La fenêtre de création doit finir par `Exit Code: 0`.
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
- **On entre chez soi par un VPN**, puis on ouvre Corde serveur comme à la maison.

### 17.1 Le plus simple sur un Synology : un VPN maillé (vérifié sur un DS220+, DSM 7.4.1)

C'est la **voie C** d'[ACCES-EXTERIEUR.md](ACCES-EXTERIEUR.md#c-pas-dadresse-publique-cg-nat--un-réseau-maillé) :
une application sur le téléphone, un paquet sur le NAS, le même compte des deux côtés. **Aucun port n'est
ouvert sur la box et aucun réglage de DSM n'est touché.** À savoir avant de choisir : une société tierce
met tes appareils en relation (elle connaît tes appareils et quand ils se connectent ; ce qui circule reste
chiffré), et **qui contrôle ton compte peut ajouter un appareil à ton réseau** : protège ce compte par une
double authentification.

L'exemple ci-dessous est celui de **Tailscale**, dont le paquet se trouve dans le Centre de paquets de DSM.
Son interface est en anglais et ses libellés peuvent changer.

1. **Sur le téléphone** : installe l'application **Tailscale**, ouvre-la, accepte l'ajout d'une
   configuration VPN, connecte-toi (un compte est créé à la première connexion). L'écran affiche un
   interrupteur allumé et **Connected**.
2. **Sur le NAS** : **Centre de paquets** → cherche **Tailscale** → **Installer**. Quand le paquet affiche
   **En cours d'exécution** → **Ouvrir** : une page s'ouvre dans un nouvel onglet.
3. Sur cette page, clique sur le bouton bleu (**Reauthenticate** ou **Log in** : la première fois, la page
   annonce « Your device's key has expired », c'est normal) → connecte-toi avec **le même compte** que sur
   le téléphone → **Connect**.
4. Rouvre la page du paquet (**Ouvrir**) : sous **This device**, le nom du NAS, **Connected** et son adresse
   dans le VPN, du type **`100.x.y.z`**. Note-la.
5. **Essaie depuis l'extérieur** : téléphone en 4G/5G (Wi-Fi coupé), VPN allumé, ouvre
   `http://100.x.y.z:8471` dans le navigateur : la page **Serveur Corde** doit s'afficher et demander un
   code. Si elle ne s'affiche pas : vérifie l'interrupteur du VPN sur le téléphone, puis l'état
   **Connected** sur la page du paquet.
6. **Dans l'application Corde** (version **1.4.2** ou suivante : avant, elle refusait ces adresses) :
   **Accueil** → **Mes serveurs** → **Ajouter un serveur** → **Adresse** : `100.x.y.z:8471` → **Nom** :
   « NAS dehors » → **Ajouter** → **Appairer ce téléphone**, avec un **nouveau code** (§ 10) → **Utiliser**.
   Sans l'application, la page ouverte à l'étape 5 s'appaire de la même façon.

Ensuite :

- **Deux lignes dans « Mes serveurs »**, une par adresse : le NAS à la maison (`192.168.…`) et le NAS dehors
  (`100.…`). En principe, l'adresse du VPN marche aussi à la maison tant que le VPN est allumé (non
  vérifié sur ce NAS) : tu pourrais alors n'utiliser qu'elle.
- **Le paquet du Centre de paquets est ancien** (1.58.2 au moment de l'essai ; son éditeur indique ne plus
  maintenir ce canal). Il fonctionne, mais mets-le à jour avec la tâche planifiée que recommande l'éditeur
  (vérifié : 1.58.2 → 1.104.7, le NAS garde son adresse et reste **Connected**) : **Panneau de
  configuration** → **Planificateur de tâches** → **Créer** → **Tâche planifiée** → **Script défini par
  l'utilisateur** ; onglet **Général** : un nom (« Mise a jour Tailscale »), **Utilisateur** : `root`,
  **Activé** coché ; onglet **Paramètres de tâche**, zone du script : `tailscale update --yes` ; **OK**
  (DSM avertit qu'une tâche `root` a tous les droits, et peut demander ton mot de passe). Puis sélectionne
  la tâche → **Exécuter** pour la première mise à jour ; ensuite elle tourne chaque jour. C'est un réglage
  système du NAS : fais-le toi-même, en connaissance de cause.
- **Par défaut, chaque appareil doit se reconnecter au VPN tous les 180 jours.** Pour un serveur, l'éditeur
  permet de désactiver cette expiration pour l'appareil, dans la console d'administration du compte. Sinon,
  le NAS sortira du VPN au bout de six mois et il faudra refaire l'étape 3.
- **Limite ce que le téléphone atteint** avec les règles d'accès du service, pour qu'il ne joigne que le
  port 8471 du NAS.
- **Téléphone perdu ou volé** : retire-le de ton réseau dans la console d'administration du VPN, **et**
  révoque-le dans Corde (§ 10).

Non vérifié sur ce NAS : Android (même principe), les règles d'accès.

### 17.2 Les autres voies

Les voies A et B d'[ACCES-EXTERIEUR.md](ACCES-EXTERIEUR.md) marchent telles quelles (dans la voie A, le
Raspberry Pi qui fait serveur WireGuard peut être n'importe quelle machine Linux de la maison, pas le NAS).
Le paquet **VPN Server** de Synology fait du NAS lui-même le serveur VPN de la maison : c'est l'équivalent
de la **voie B** (la box qui fait serveur VPN), avec le NAS à la place de la box. Il propose OpenVPN,
L2TP/IPsec et PPTP (pas WireGuard) : prends **OpenVPN**, jamais PPTP (chiffrement dépassé). Seul le port
du VPN est alors redirigé par la box, jamais celui de Corde ; suis la documentation de Synology pour le
régler, et applique la liste de vérification finale d'[ACCES-EXTERIEUR.md](ACCES-EXTERIEUR.md). Avec ces
voies, le téléphone se retrouve dans le réseau de la maison : il ouvre `http://192.168.1.50:8471`, la même
adresse qu'à la maison, sans second appairage.

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
