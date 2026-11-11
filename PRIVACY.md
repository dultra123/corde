# Vie privée

Ce document décrit ce que Corde fait **aujourd'hui** (version 0.4) avec tes données. Il est vérifié dans le
code ; s'il devient faux, c'est un bogue à signaler.

## En bref

- **Tout reste sur ton PC.** Corde n'a pas de serveur en ligne, pas de compte, pas de télémétrie, pas de
  statistiques d'usage, pas de publicité. Il ne vérifie pas les mises à jour en ligne.
- **Rien n'est envoyé sur Internet**, sauf si tu actives toi-même l'extracteur LLM optionnel (voir plus bas).
- **Deux pages publiques peuvent être lues, si tu l'actives** : le planning des courses en ligne
  (racecontrol.gg et lmu.tools), désactivé par défaut. Rien de toi n'y est envoyé (voir plus bas).
- **L'accès depuis le réseau local est désactivé par défaut** : seul ton PC peut ouvrir Corde tant que tu
  n'as pas activé l'accès depuis le téléphone.

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
(il est déjà présent sur Windows 10 et 11 à jour). Corde lui-même ne télécharge rien.

## Tout effacer

Désinstalle Corde (Paramètres Windows → Applications → Applications installées → Corde → Désinstaller), puis
supprime le dossier `%LOCALAPPDATA%\Corde`. Voir aussi le [README](README.md#désinstaller).

---

## English summary

Corde keeps **everything on your PC**: no online service, no account, no telemetry or usage statistics, no
online update check. The **online race schedule is off by default**: once you turn it on, the server
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
delete `%LOCALAPPDATA%\Corde`.
