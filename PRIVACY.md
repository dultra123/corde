# Vie privée

Ce document décrit ce que Corde fait **aujourd'hui** (version 0.7) avec tes données. Il est vérifié dans le
code ; s'il devient faux, c'est un bogue à signaler.

## En bref

- **Tout reste sur ton PC.** Corde n'a pas de serveur en ligne, pas de compte, pas de télémétrie, pas de
  statistiques d'usage, pas de publicité.
- **Rien n'est envoyé sur Internet**, sauf si tu actives toi-même l'extracteur LLM optionnel (voir plus bas).
- **L'application de bureau vérifie les mises à jour sur GitHub** (activé par défaut, désactivable) : elle
  lit un petit fichier public ; GitHub voit l'adresse IP de ta connexion, rien d'autre n'est envoyé (voir
  plus bas).
- **Deux pages publiques peuvent être lues, si tu l'actives** : le planning des courses en ligne
  (racecontrol.gg et lmu.tools), désactivé par défaut. Rien de toi n'y est envoyé (voir plus bas).
- **L'accès depuis le réseau local est désactivé par défaut** : seul ton PC peut ouvrir Corde tant que tu
  n'as pas activé l'accès depuis le téléphone.
- **Serveur Corde de la maison (optionnel, 0.7)** : si tu installes Corde serveur sur ta propre machine
  (Raspberry Pi) et que tu y appaires ton PC, le PC lui envoie tes sessions **sur ton réseau** ; rien ne passe
  par Internet ni par un service à nous (voir plus bas).

## Ce que Corde lit (en lecture seule)

| Source | Quoi |
|---|---|
| Mémoire partagée du jeu (`LMU_Data`) | la télémétrie en direct : ta voiture, la session, la météo, et le classement en direct (avec les noms des autres pilotes) |
| API locale du jeu (`http://127.0.0.1:6397`, sur ton PC) | uniquement des requêtes de lecture (`GET`) d'une liste fermée : description de l'API, données du garage (limites des réglages de ta voiture). Jamais ton profil, jamais de ticket d'authentification, jamais une écriture |
| Fichiers de résultats du jeu (`UserData\Log\Results\*.xml`) | tes résultats de course, de qualification et d'essais. **Ils contiennent les noms et les temps des autres pilotes de la session** |
| Fichiers de réglages du jeu (`UserData\player\Settings\…\*.svm`) et `UserData\player\Settings.JSON` | tes réglages de voiture (pour proposer des modifications) et le port de l'API locale du jeu |

## Ce que Corde écrit

Dans son **dossier de données**, `%LOCALAPPDATA%\Corde` (sous Linux : `~/.local/share/corde`) :

- `corde.db` : tes sessions, tours, événements, les résultats importés (dont le classement complet, donc les
  noms des autres pilotes), tes ressentis écrits dans la page Ingénieur, les analyses ;
- `sessions\…\*.parquet` : les échantillons de télémétrie enregistrés ;
- `layouts\` : tes dispositions du tableau de bord ; `settings.json` : tes réglages (accès réseau, planning
  des courses…) ;
- `races\` : les courses que tu as choisies (nom, circuit, heure de départ), les réglages de la page Courses
  (délais de rappel, ton DR / SR tels que tu les as saisis, exigences d'accès, correction d'heure) et la
  liste des rappels déjà envoyés ;
- `diagnostics\lmu-api\` : une copie de la description de l'API locale du jeu et des données du garage ;
- `diagnostics\races\` : seulement si une page de planning n'a pas pu être lue, la copie de cette page
  publique (une par site, remplacée à chaque fois) ;
- `logs\` (application de bureau) : le journal. Il peut contenir des chemins de ton PC (donc le nom de ton
  compte Windows) et ton nom de pilote.
- seulement si tu as appairé un serveur Corde : `hub-link.json` (adresse du serveur et clé de ce PC auprès de
  lui) et `sync-hashes.json` (sommes de contrôle des fichiers d'échantillons, pour ne pas les renvoyer).

**Ces données ne quittent pas ton PC**, y compris les fichiers de résultats et les noms des autres pilotes
qu'ils contiennent.

Hors de ce dossier, Corde n'écrit que :

- les fichiers de réglage `Corde_*.svm`, dans le dossier `Settings` du jeu, **uniquement quand tu cliques
  sur « Créer les fichiers de réglage »** (jamais en écrasant un fichier existant) ;
- pour l'application de bureau : son profil d'affichage (WebView2) et la position de la fenêtre, dans
  `%LOCALAPPDATA%\app.corde.desktop` et `%APPDATA%\app.corde.desktop`, et l'entrée « démarrer avec Windows »
  si tu l'actives.

L'interface garde aussi quelques préférences dans le stockage local de ton navigateur (langue, adresse du serveur, dispositions,
navigation automatique, son des rappels de course), sur chaque appareil.

## Accès depuis le réseau local

Désactivé par défaut. Quand tu l'actives (bouton de l'écran d'accueil), **tout appareil connecté au même
réseau** peut ouvrir Corde et voir ce que l'interface montre : sessions, télémétrie, résultats avec les noms
des autres pilotes, chemins de dossiers. Certaines actions restent réservées au PC (suppressions, création
des fichiers de réglage, réglages). Active-le seulement sur un réseau de confiance, et coupe-le quand tu n'en
as plus besoin. Détails : [docs/SECURITY-MODEL.md](docs/SECURITY-MODEL.md).

## Serveur Corde de la maison (optionnel, 0.7)

Tu peux installer **Corde serveur** sur une machine à toi, toujours allumée, chez toi (un Raspberry Pi :
[docs/SERVEUR.md](docs/SERVEUR.md)). Rien ne se passe tant que tu n'y as pas **appairé** ton PC.

- **Ce que le PC lui envoie**, après chaque session terminée, au démarrage et toutes les 5 minutes : tes
  sessions (tours, événements, réglages capturés, fichiers d'échantillons), tes **résultats importés** (dont
  le classement complet : **les noms des autres pilotes**), tes ressentis et les itérations de l'ingénieur.
  Pas les dispositions, ni les réglages, ni tes choix de courses.
- **Ce que le serveur renvoie au PC** : les ressentis que tu as saisis sur le serveur.
- **Par où** : directement du PC à l'adresse que tu as donnée (ton réseau local, ou ton propre VPN), en HTTP,
  jamais par un proxy, jamais par un service en ligne. Corde n'a pas de serveur à nous.
- **Qui peut lire le serveur** : seulement les appareils que tu as appairés avec un code obtenu sur le serveur
  lui-même. Sur le réseau de la maison, ce trafic n'est **pas chiffré** (comme l'accès depuis le téléphone) :
  dehors, passe par un VPN ([docs/ACCES-EXTERIEUR.md](docs/ACCES-EXTERIEUR.md)).
- **Ce que le serveur garde**, dans `/var/lib/corde` : les mêmes données que le PC (`corde.db`, `sessions/`),
  la liste des appareils appairés (`hub/devices.json` : nom, dates, empreinte de la clé, jamais la clé
  elle-même), le dernier PC qui a envoyé quelque chose (nom, version, adresse de son direct), ses propres
  choix et réglages de courses (`races/`), et son journal système (`journalctl -u corde` : adresses IP des
  appareils qui s'appairent ou sont refusés ; jamais un code ni une clé).
- **Le serveur lui-même ne contacte Internet** que si un appareil administrateur active le planning des
  courses en ligne sur lui (mêmes deux pages, mêmes règles que plus bas).
- **Effacer** : supprimer une session sur le PC ne la supprime pas du serveur ; supprime-la aussi sur le
  serveur (appareil administrateur), ou efface tout le serveur (`sudo /opt/corde/uninstall.sh --purge`).
  Pour que le PC arrête d'envoyer : Réglages → Serveur Corde → **Dissocier**.

## Planning des courses en ligne (optionnel, désactivé par défaut)

La page **Courses** affiche le planning en ligne de Le Mans Ultimate. Tant que tu ne l'actives pas (bouton
de la page, sur le PC ; réglage `online_schedule` de `settings.json`), Corde ne contacte **aucun** site.
Une fois activé :

- le serveur de Corde, **sur ton PC**, lit deux pages publiques et seulement elles (liste fermée dans le
  code) : `https://racecontrol.gg/` et `https://lmu.tools/schedule` ;
- **quand** : seulement quand une page Courses est ouverte (sur le PC ou le téléphone), au plus toutes les
  10 minutes pour racecontrol.gg et toutes les 60 minutes pour lmu.tools ; le bouton « Actualiser » relit au
  plus une fois par minute ;
- **ce qui part** : une simple requête `GET`, avec les en-têtes `User-Agent: Corde/<version>
  (+https://github.com/dultra123/corde)`, `Accept: text/html` et `Accept-Language: en-GB,en;q=0.8`.
  **Aucun cookie, aucun compte, aucun identifiant, aucune donnée de pilote** (ni ton nom, ni tes rangs, ni
  tes courses choisies). Comme pour toute page web, ces sites voient l'adresse IP de ta connexion et la
  version de Corde ;
- **les images** des courses, quand les pages en citent, sont chargées directement par l'appareil qui
  affiche la page (PC ou téléphone), depuis l'adresse `https://` donnée par le site, sans en-tête
  `Referer` : le serveur de ces images voit l'adresse IP de cet appareil ;
- les pages lues ne sont pas enregistrées, sauf la copie de diagnostic d'une page illisible (plus haut).

Corde n'appelle jamais l'API du jeu pour t'inscrire, ni aucun autre service : l'inscription se fait dans
le jeu. Les rappels (bandeau et son) sont produits par Corde elle-même, sur ton PC et les appareils de ton
réseau où elle est ouverte ; rien ne passe par un service de notifications. Pour couper : **Affichage et
heure → Ne plus lire le planning sur ces sites** (ou `"online_schedule": false` dans `settings.json`).

## Mises à jour de l'application de bureau (activé par défaut, désactivable)

L'application Windows peut se mettre à jour depuis **Réglages → Mises à jour**. Pour savoir s'il existe
une version plus récente, elle lit **un seul fichier public** :
`https://github.com/dultra123/corde/releases/latest/download/latest.json` (GitHub le sert depuis son
stockage de fichiers, un domaine en `*.githubusercontent.com`, vers lequel `github.com` redirige).

- **Quand** : 30 secondes après le lancement, puis toutes les 6 heures, tant que la case « Rechercher
  automatiquement » est cochée ; et chaque fois que tu cliques sur « Rechercher une mise à jour ». C'est
  l'application de bureau qui le fait (pas le serveur, ni le navigateur du téléphone).
- **Ce qui part** : une simple requête `GET`, avec les en-têtes `User-Agent: tauri-plugin-updater/<version
  du module>` et `Accept: application/json`. **Aucun cookie, aucun compte, aucun identifiant, aucune donnée
  de pilote ni de session, pas même la version de Corde installée** (la comparaison se fait sur ton PC).
  Comme pour toute page web, **GitHub voit l'adresse IP de ta connexion** et l'heure de la requête.
- **Quand tu cliques sur « Installer et redémarrer »** : l'installeur de la nouvelle version est téléchargé
  depuis la même page des versions (même chose : GitHub voit ton adresse IP), sa signature est vérifiée sur
  ton PC, puis il s'installe. Rien n'est installé sans ce clic.
- **Pour couper** : décoche **Réglages → Mises à jour → Rechercher automatiquement** (réglage
  `update_auto_check` de `%LOCALAPPDATA%\Corde\desktop.json`). Corde ne contacte alors plus GitHub, sauf
  quand tu cliques toi-même sur « Rechercher une mise à jour ».
- `desktop.json` garde aussi la dernière version annoncée par une notification (pour ne pas la répéter) et,
  le temps d'une installation, la version d'avant et la version installée.

Les politiques de GitHub s'appliquent à ces requêtes : https://docs.github.com/site-policy/privacy-policies.

## Extracteur LLM (optionnel, désactivé par défaut)

Par défaut, ton ressenti (« ça sous-vire en entrée ») est interprété **sur ton PC**, par un dictionnaire.

Si tu définis la variable d'environnement `CORDE_ANTHROPIC_API_KEY` avec **ta propre clé** d'API Anthropic
(ou lances le serveur avec `--llm anthropic` et cette clé), Corde envoie **le texte de chaque ressenti** que tu
saisis à l'API d'Anthropic (`api.anthropic.com`), avec des consignes fixes et la liste des symptômes connus,
pour l'interpréter. Rien d'autre n'est envoyé : ni télémétrie, ni sessions, ni résultats, ni noms. La clé
n'est envoyée qu'à Anthropic, et n'est jamais écrite dans le journal. Les requêtes sont facturées sur ton
compte Anthropic et soumises à ses conditions. Sans la variable, rien ne part.

## Installation

L'installeur Windows peut télécharger le moteur d'affichage WebView2 depuis Microsoft s'il manque sur le PC
(il est déjà présent sur Windows 10 et 11 à jour). En dehors de ses propres mises à jour (plus haut), Corde
ne télécharge rien.

## Tout effacer

Désinstalle Corde (Paramètres Windows → Applications → Applications installées → Corde → Désinstaller), puis
supprime le dossier `%LOCALAPPDATA%\Corde`. Voir aussi le [README](README.md#désinstaller). Si tu as un
serveur Corde : `sudo /opt/corde/uninstall.sh --purge` sur le serveur ([docs/SERVEUR.md](docs/SERVEUR.md#10-désinstaller)).

---

## English summary

Corde keeps **everything on your PC**: no online service, no account, no telemetry or usage statistics.
The **desktop app checks for updates on GitHub** (on by default, can be turned off in Settings → Updates):
30 s after start-up, then every 6 hours, and when you click "Check for updates", it downloads one public
file (`https://github.com/dultra123/corde/releases/latest/download/latest.json`) with a plain `GET`; GitHub
sees your IP address, nothing else is sent (no cookie, no identifier, not even the installed version). The
installer of a new version is downloaded only when you click "Install and restart", and its signature is
checked on your PC. The **online race schedule is off by default**: once you turn it on, the server
reads two public pages only (`https://racecontrol.gg/`, `https://lmu.tools/schedule`), when the Races page
is open and at most every 10 / 60 minutes, with a plain `GET` (User-Agent `Corde/<version>`, no cookie, no
account, no driver data); race pictures are loaded by the displaying device straight from the `https://`
address the sites give, without referrer. It reads the game locally and read-only (shared memory, `GET` requests to the game's
local API on `127.0.0.1`, results files, setup files) and stores sessions, imported results, feedback and
logs, plus your picked races and the Races page settings (`races\`), in `%LOCALAPPDATA%\Corde`. Game results files contain **other drivers' names**; they never leave the
PC. Outside its data folder Corde only writes `Corde_*.svm` setup files when you ask for them, plus the
desktop app's WebView2 profile, window state and the optional "start with Windows" entry. **LAN access is
off by default**; when you turn it on, any device on the same network can see what the UI shows. The
optional LLM extractor is **off by default**: only when you set `CORDE_ANTHROPIC_API_KEY` with your own key
does Corde send the text of each feedback you type to Anthropic's API (nothing else). The Windows installer
may download Microsoft's WebView2 runtime if it is missing. To erase everything: uninstall Corde, then
delete `%LOCALAPPDATA%\Corde`. **Home server (optional, 0.7)**: if you install Corde server on your own
always-on machine (a Raspberry Pi) and pair your PC with it, the PC sends it your sessions, imported results
(with other drivers' names), feedback and plan iterations, straight over your own network or VPN in plain HTTP
(no proxy, no online service), and gets back the feedback typed on the server. Only devices paired with a code
read on the server can open it; it keeps the same data in `/var/lib/corde`, plus the paired devices (a hash of
each key, never the key). Erase it with `sudo /opt/corde/uninstall.sh --purge`.
