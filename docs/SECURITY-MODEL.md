# Modèle de sécurité de Corde

> Ce document décrit ce que le serveur local de Corde expose, à qui, et ce contre quoi il protège.
> Code : `crates/corde-server/src/guard.rs` (politique), `crates/corde-server/src/net.rs`
> (origines locales, garde « depuis ce PC »), tests : `crates/corde-server/tests/access_policy.rs`.
> Signaler une faille : [SECURITY.md](../SECURITY.md). Vie privée : [PRIVACY.md](../PRIVACY.md).
> Le **serveur Corde de la maison** (`--role hub`, 0.7, appairage obligatoire) a ses propres règles :
> [§7](#7-le-serveur-corde-mode-hub). L'**application iPhone** (0.8) : [§8](#8-lapplication-iphone).
> L'**application Android** (1.0) : [§9](#9-lapplication-android).

## 1. Ce qui tourne

Corde est un serveur HTTP + WebSocket (axum) qui écoute sur le **port 8471** (repli 8472…8480 dans
l'application de bureau) et sert à la fois l'interface web et l'API `/api/*`. Il n'y a **pas de compte ni de
mot de passe** : c'est voulu, Corde vise un PC de jeu et le réseau de la maison. L'exposition est donc bornée
autrement :

- **Par défaut, le serveur n'écoute que sur `127.0.0.1`** : seul le PC lui-même peut l'ouvrir.
- L'accès depuis le réseau local (téléphone, tablette, OBS sur un autre PC) s'active par le bouton « Activer
  l'accès depuis mon téléphone » de l'écran d'accueil (réglage `lan_access` de `settings.json`, appliqué au
  redémarrage), ou par `--lan` / `--bind` en mode console. Le serveur écoute alors sur toutes les interfaces.
- L'interface est chargée de six façons, toutes prises en compte par la politique ci-dessous :

| Cas | Adresse de la page | Origine envoyée à l'API |
|---|---|---|
| Application de bureau (fenêtre Tauri) | `http://127.0.0.1:<port>/#/…` (la coquille fait naviguer sa webview vers le serveur embarqué : `apps/desktop/src-tauri/src/shell.rs`, `ui_url` → `server::local_url`) | `http://127.0.0.1:<port>` = même origine que l'hôte |
| Navigateur du PC | `http://127.0.0.1:8471` | même origine |
| Téléphone du réseau local | `http://<ip-du-pc>:8471` | même origine (`http://<ip-du-pc>:8471`) |
| OBS (source navigateur, mode overlay de 0.5) | `http://127.0.0.1:8471/#/overlay/…` ; second PC de stream : `http://<ip-du-pc>:8471/#/overlay/…` (accès réseau activé) | même origine |
| Développement, scripts de capture | `http://127.0.0.1:<autre port>/?server=127.0.0.1:8471`, `vite dev` sur `localhost:5173` | origine locale (autre port de la boucle locale) |
| Application iPhone (0.8, `apps/mobile`, Capacitor) | `capacitor://localhost/index.html` (l'interface est **dans** l'application) ; elle appelle `http://<ip-du-pc>:8471` ou le serveur Corde | `capacitor://localhost` (§8) |
| Application Android (1.0, `apps/mobile/android`, Capacitor) | `http://localhost/index.html` (servie par l'application elle-même, pas par le réseau) ; elle appelle `http://<ip-du-pc>:8471` ou le serveur Corde | `http://localhost` : une origine locale, sans droit « ce PC » depuis un téléphone (§9) |

Les pages propres à la coquille (écran de démarrage, `apps/desktop/dist` : `tauri://localhost` sous
macOS/Linux, `http://tauri.localhost` sous Windows) n'appellent pas l'API HTTP — elles passent par l'IPC de
Tauri — mais leurs origines sont acceptées quand même. Le code Rust de la coquille interroge le serveur par
`ureq` sur `http://127.0.0.1:<port>` sans en-tête `Origin` : accepté. Les commandes IPC de la coquille
(démarrage avec Windows, redémarrage, version, mises à jour) et l'écoute des événements ne sont accordées
qu'aux pages `http://127.0.0.1:*` et `http://localhost:*`
(`apps/desktop/src-tauri/capabilities/embedded-ui.json`) ; aucune commande de plugin (ni celles de
`tauri-plugin-updater`) ne leur est ouverte. Un téléphone ou un navigateur n'a pas d'IPC : il ne peut ni
vérifier ni installer une mise à jour (§5 bis).

## 2. Modèle de menace

**Corde protège contre :**

1. **Un site web malveillant ouvert dans le navigateur du pilote** (même PC). Sans protection, une page
   quelconque pouvait lire l'API locale (sessions, tours, noms des autres pilotes dans les résultats) et
   appeler les routes d'écriture. Désormais toute requête vers `/api/*` ou `/ws` qui porte un en-tête
   `Origin` étranger reçoit **403, lecture comprise**, et les en-têtes CORS ne sont jamais accordés à une
   autre origine que celles du §3.
2. **Le « DNS rebinding »** : un site qui fait pointer son propre nom de domaine vers `127.0.0.1` pour que le
   navigateur le considère comme « même origine ». Le serveur refuse tout en-tête `Host` qui n'est pas une
   adresse attendue (§3) : le nom de domaine de l'attaquant ne passe pas.
3. **Les autres machines quand l'accès réseau est coupé** : le serveur n'écoute que sur la boucle locale, et
   refuse en plus les en-têtes `Host` qui ne sont pas locaux.
4. **Un appareil du réseau local, pour les actions lourdes de conséquences** : même accès réseau activé, les
   suppressions, les traitements lourds, l'écriture dans les dossiers du jeu et le changement des réglages ne
   sont possibles que depuis le PC lui-même (§4).

**Corde ne protège pas contre :**

1. **Une machine hostile sur le même réseau quand l'accès réseau est activé.** Tout appareil du Wi-Fi peut
   alors lire tout ce que l'interface montre (sessions, télémétrie en direct, résultats avec les noms des
   autres pilotes, chemins de dossiers du PC, adresses IP), modifier les dispositions, envoyer un ressenti
   (qui part vers l'API d'Anthropic si l'extracteur LLM optionnel est activé, donc à tes frais), démarrer
   ou arrêter l'enregistrement, choisir ou retirer des courses de « Mes courses » (≤ 50), changer les
   réglages de la page Courses et, si le planning est activé, faire relire les deux pages publiques du
   planning (au plus une fois par minute, cache compris). Un programme (pas un navigateur) peut aussi forger `Host` et `Origin` : ces
   contrôles visent les navigateurs. Le trafic est en HTTP simple, non chiffré. **N'active l'accès réseau
   que sur un réseau de confiance** (la maison, pas un Wi-Fi public), et coupe-le quand tu n'en as pas besoin.
2. **Un programme malveillant qui tourne déjà sur le PC** : il a les mêmes droits que l'utilisateur, il
   peut lire `%LOCALAPPDATA%\Corde` directement.
3. **Les extensions du navigateur** qui ont accès à toutes les pages.

## 3. Règles appliquées à chaque requête

Ordre d'application (`guard::guard`, après le routage) :

1. **En-tête `Host`** (toute requête, interface comprise) :
   - accès réseau coupé : `localhost` et les adresses de boucle locale (`127.0.0.1`, `[::1]`…), avec ou sans
     port ;
   - accès réseau activé : en plus, **toute adresse IP littérale**, le **nom de la machine** (variable
     `COMPUTERNAME` sous Windows, nom d'hôte du noyau sous Linux) et `<nom>.local` ;
   - sinon **403** « hôte non autorisé ». Une requête sans `Host` (HTTP/1.0, outil) est acceptée.
2. **En-tête `Origin`** (`/api/*` et ouverture du WebSocket `/ws` — les navigateurs n'appliquent pas CORS aux
   WebSockets) : une requête qui en porte un n'est acceptée que si l'origine est
   - la **même que l'hôte demandé** : schéma `http`, même hôte et même port que `Host` (cas du téléphone et
     de la fenêtre de bureau) ;
   - une **origine locale** : `http://127.0.0.1:*`, `http://localhost:*`, `http://[::1]:*` (autres ports :
     serveur de développement, scripts de capture ; sans port, c'est aussi l'origine de l'application
     Android, §9) ;
   - une **origine de la coquille Tauri** : `tauri://localhost`, `http://tauri.localhost`,
     `https://tauri.localhost` ;
   - l'**origine de l'application iPhone** : `capacitor://localhost` exactement (0.8, §8) — acceptée
     pour l'`Origin` et CORS, **jamais** comme une page de ce PC (règle 4) ;
   - sinon **403** « origine non autorisée », **y compris pour un GET**. Une requête **sans `Origin`**
     (navigation directe, `curl`) est acceptée.
3. **CORS** : `Access-Control-Allow-Origin` ne reflète que les origines acceptées ci-dessus, pré-requêtes
   `OPTIONS` comprises (méthodes GET, POST, PUT, PATCH, DELETE ; en-têtes `Content-Type`, `Accept` et,
   depuis 0.8, `Authorization` : le jeton d'appareil de l'application iPhone, §8). Une pré-requête d'une
   autre origine reçoit 403. Aucune réponse n'autorise les cookies d'une autre origine
   (`Access-Control-Allow-Credentials` absent).
4. **Routes réservées au PC** (tableau du §4) : 403 « cette action n'est possible que depuis le PC qui fait
   tourner Corde » si la connexion ne vient pas de la boucle locale, ou si la page qui l'envoie n'est pas une
   origine locale (`net::may_change_settings`, la garde qui protégeait déjà les réglages). Un navigateur du
   PC qui ouvre Corde par l'adresse réseau du PC (`http://192.168.x.x:8471`) est vu comme un autre appareil :
   sur le PC, ouvrir `http://127.0.0.1:8471`.
5. **Taille** : corps de requête limité à **1 Mio** (413 au-delà), message WebSocket du client à **16 Kio**.
6. **Erreurs** : une erreur 5xx renvoyée à un autre appareil du réseau perd son détail (qui peut nommer un
   dossier du PC) : « erreur interne du serveur (détails dans le journal du PC) ». Le détail reste dans le
   journal. Le PC lui-même voit le message complet.

Limites connues de ces règles : en `vite dev` ouvert depuis un téléphone (`http://<ip>:5173`), le proxy de
développement réécrit `Host` en `127.0.0.1:8471` mais garde l'`Origin` du téléphone : l'API répond 403. Ce cas
ne concerne que le développement ; ouvrir l'interface servie par Corde (`http://<ip>:8471`) fonctionne.

## 4. Les routes

Effet : **L** lecture · **D** écrit dans le dossier de données (`%LOCALAPPDATA%\Corde`) · **S** supprime ·
**H** écrit hors du dossier de données · **X** appelle l'extérieur. « Avant » = garde avant ce lot (CORS
permissif pour toutes). « Maintenant » : **ouverte** = tout appareil autorisé à joindre le serveur (le PC ;
le réseau local quand l'accès est activé), après les contrôles `Host` / `Origin` du §3 ; **PC** = depuis ce
PC seulement.

| Méthode | Chemin | Effet | Avant | Maintenant | Pourquoi |
|---|---|---|---|---|---|
| GET | `/api/health` | L | aucune | ouverte | version, état ; la coquille s'en sert pour reconnaître Corde |
| GET | `/api/status` | L | aucune | ouverte | état du jeu et de la source, tableau de bord |
| GET | `/api/frame` | L | aucune | ouverte | dernière image de télémétrie (classement en direct : noms des autres pilotes) |
| GET | `/api/laps` | L | aucune | ouverte | tours de la session en cours |
| GET | `/api/layouts` | L | aucune | ouverte | dispositions du tableau de bord ; une page overlay (OBS) relit la liste toutes les 10 s pour suivre la disposition qu'elle montre |
| PUT | `/api/layouts` | D | aucune | ouverte | le tableau de bord du téléphone y enregistre ses dispositions (remplacement complet par tableau) ; données petites et recréables |
| DELETE | `/api/layouts` | S | aucune | **PC** | supprime toutes les dispositions ; l'interface ne l'utilise pas |
| GET | `/api/layouts/{id}` | L | aucune | ouverte | |
| PUT | `/api/layouts/{id}` | D | aucune | ouverte | édition d'une disposition depuis le téléphone |
| DELETE | `/api/layouts/{id}` | S | aucune | **PC** | suppression ; l'interface ne l'utilise pas |
| GET | `/api/sessions` | L | aucune | ouverte | liste des sessions |
| GET | `/api/sessions/{id}` | L | aucune | ouverte | détail d'une session |
| GET | `/api/sessions/{id}/laps/{lap}/samples` | L | aucune | ouverte | courbes d'un tour (Parquet) |
| DELETE | `/api/sessions/{id}` | S | aucune | **PC** | efface la session (base et fichiers Parquet), irréversible |
| GET | `/api/sessions/{id}/analysis` | L, D (cache), lit les `.svm` du jeu | aucune | ouverte | page Ingénieur, aussi sur le téléphone ; calculée une fois puis servie depuis le cache |
| POST | `/api/sessions/{id}/analysis/recompute` | D, traitement lourd | aucune | **PC** | relance l'analyse complète en ignorant le cache |
| GET | `/api/sessions/{id}/feedback` | L | aucune | ouverte | liste des ressentis |
| POST | `/api/sessions/{id}/feedback` | D ; X si l'extracteur LLM est activé | aucune | ouverte | saisie du ressenti, prévue au téléphone ; texte ≤ 2000 caractères |
| GET | `/api/sessions/{id}/iterations` | L (+ `.svm` du jeu) | aucune | ouverte | itérations de réglage |
| GET | `/api/setups?session=` | L hors dossier (dossier `Settings` du jeu) | aucune | ouverte | bibliothèque de réglages affichée par la page Ingénieur |
| GET | `/api/sessions/{id}/setup-preview` | L hors dossier | aucune | ouverte | aperçu d'un fichier, rien n'est écrit |
| POST | `/api/sessions/{id}/setup-files` | **H** (`UserData/player/Settings/<circuit>/Corde_*.svm`) | PC | **PC** | écrit dans les dossiers du jeu (jamais d'écrasement) |
| GET | `/api/recording` | L | aucune | ouverte | état de l'enregistreur |
| POST | `/api/recording/start`, `/stop` | D | aucune | ouverte | commande d'enregistrement, utile au volant depuis le téléphone ; ne supprime rien |
| GET | `/api/storage` | L | aucune | ouverte | tailles et chemin du dossier de données |
| GET | `/api/channels` | L | aucune | ouverte | liste des canaux |
| GET | `/api/profile` | L | aucune | ouverte | profil pilote, état de l'import (chemin du dossier de résultats) |
| POST | `/api/profile/import` | lit le dossier de résultats du jeu, D, traitement lourd | aucune | **PC** | relit tous les fichiers de résultats, y compris ceux ignorés |
| GET | `/api/results`, `/api/results/{id}` | L | aucune | ouverte | résultats importés (classement complet : noms des autres pilotes) |
| GET | `/api/settings` | L | aucune | ouverte | `{lan_access, online_schedule}` |
| PUT | `/api/settings` | D ; ouvre ou ferme le serveur au réseau ; autorise ou non la lecture du planning en ligne (`online_schedule`) | PC | **PC** | inchangé ; le réglage `online_schedule` (0.4) suit la même règle |
| GET | `/api/races/sources` | **X** si `online_schedule` : `GET` des deux pages de la liste fermée (`races::sources`), cache 10 / 60 min, relecture forcée ≤ 1/min | — (0.4) | ouverte | la page Courses, aussi sur le téléphone ; aucune adresse ne vient de la requête ; rien n'est lu si le réglage est faux |
| POST | `/api/races/sources/{source}/unreadable` | D (`diagnostics/races/<source>.html`, copie déjà détenue par le serveur, une par source) | — (0.4) | ouverte | l'interface signale une page sans course ; `source` ∈ {`racecontrol`, `lmutools`}, le contenu ne vient pas du client |
| GET | `/api/races/picks` | L | — (0.4) | ouverte | « Mes courses », partagées PC / téléphone |
| PUT | `/api/races/picks/{id}` | D (`races/picks.json`) | — (0.4) | ouverte | choix validé (identifiant `<course>@<départ>`, champs bornés, image `https://` seulement, champs inconnus refusés), ≤ 50 choix (409 au-delà) |
| DELETE | `/api/races/picks/{id}` | S (un choix, recréable) | — (0.4) | ouverte | retirer une course depuis le téléphone |
| GET | `/api/races/settings` | L | — (0.4) | ouverte | délais de rappel, affichage, correction d'heure, DR / SR, exigences |
| PUT | `/api/races/settings` | D (`races/settings.json`) | — (0.4) | ouverte | clés connues seulement, valeurs dans des listes fermées |
| GET | `/api/network` | L | aucune | ouverte | adresses à ouvrir sur le téléphone |
| GET | `/api/hub-link` | L | — (0.7) | ouverte | état du lien avec le serveur Corde (adresse, état, dernière synchronisation, en attente) ; **jamais le jeton** |
| PUT | `/api/hub-link` | D (`hub-link.json`) | — (0.7) | **PC** | adresse du serveur |
| POST | `/api/hub-link/pair` | D (jeton dans `hub-link.json`, 0600) ; **X** vers l'adresse donnée (le serveur de la maison) | — (0.7) | **PC** | appairage avec un code lu sur le serveur |
| POST | `/api/hub-link/sync`, `/resend` | **X** vers le serveur appairé | — (0.7) | **PC** | « Synchroniser maintenant », « Tout renvoyer » |
| DELETE | `/api/hub-link` | S (`hub-link.json`) ; **X** (demande au serveur de révoquer ce PC) | — (0.7) | **PC** | « Dissocier » |
| GET | `/ws` | L (télémétrie en direct ; le client n'envoie que `SetRate` et `Ping`) | aucune | ouverte, avec contrôle d'`Origin` | tableaux de bord, OBS |
| GET | `/*` | L (fichiers de l'interface, repli sur `index.html`) | aucune | ouverte, contrôle `Host` seulement | |

Tâches de fond (pas de route) : lecture de la mémoire partagée du jeu ; `GET` vers l'API locale du jeu
(`http://127.0.0.1:6397`, liste fermée de chemins en lecture seule, `lmu_api.rs`, `lmu_garage.rs`) avec
copie dans `diagnostics/lmu-api/` du dossier de données ; lecture des fichiers de résultats toutes les 60 s ;
rappels des courses choisies toutes les 5 s (`races/mod.rs` : message `race_reminder` diffusé sur `/ws`,
rappels envoyés notés dans `races/reminders.json`, aucun appel extérieur) ; **seulement si le PC est appairé
à un serveur Corde** (0.7) : envoi au serveur de la maison au démarrage, à la fin de chaque enregistrement
et toutes les 5 minutes (`hub_link.rs`, routes `/api/sync/*` du §7, en HTTP vers l'adresse enregistrée,
jamais par un proxy).

**Appels vers Internet (planning des courses, 0.4)** : seulement `https://racecontrol.gg/` et
`https://lmu.tools/schedule` (constante `SOURCES` de `crates/corde-server/src/races/sources.rs`), seulement
quand `online_schedule` est vrai et qu'un client demande `/api/races/sources`. `GET`, délai 20 s, réponse
≤ 3 Mio, au plus 3 redirections et seulement en `https://` sur le même site, sans cookie (`ureq` sans la
fonctionnalité `cookies`), en-têtes `User-Agent: Corde/<version> (+https://github.com/dultra123/corde)`,
`Accept: text/html`, `Accept-Language: en-GB,en;q=0.8`. La page est rendue à l'interface **comme texte** ;
l'interface l'analyse (`packages/ui/src/races/parse.ts`) et ne la met jamais dans le DOM ; les images de
course ne sont affichées que pour des adresses `https://`, en `<img loading="lazy"
referrerpolicy="no-referrer">`, et le nom des courses n'est inséré que comme texte (React). Jamais d'appel
vers `raceos.gg` ni vers l'API du jeu pour l'inscription.

### Ce qui change pour l'utilisateur

Depuis un téléphone ou un autre appareil du réseau, ces actions **ne marchent plus** et affichent « cette
action n'est possible que depuis le PC qui fait tourner Corde » ; elles restent disponibles sur le PC :

- supprimer une session ;
- « Recalculer » l'analyse de l'ingénieur (l'analyse elle-même s'affiche toujours) ;
- « Réimporter » les résultats (l'import automatique toutes les 60 s continue) ;
- supprimer une disposition par l'API (l'interface ne le fait pas).

Créer les fichiers de réglage et changer l'accès réseau étaient déjà réservés au PC. Le tableau de bord, la
saisie du ressenti, la commande d'enregistrement et toutes les lectures restent disponibles au téléphone.

### Le mode overlay (OBS, 0.5)

Aucune route ajoutée : une page overlay (`#/overlay/widget/<id>`, `#/overlay/layout/<id>`) est l'interface
habituelle (`GET /*`, le fragment `#…` n'atteint jamais le serveur), qui lit `GET /api/layouts` et le
WebSocket `/ws` à 30 Hz, **sans jamais écrire** : pas de synchronisation des dispositions, pas de démo
automatique. Elle n'appelle ni `/api/network`, ni `/api/storage`, ni `/api/profile`, ni les réglages : elle
ne peut afficher ni adresse IP, ni chemin du PC, ni réglage de voiture. Les dispositions overlay
(`device: "overlay"`) s'enregistrent par les routes de dispositions existantes. La page « OBS » de Corde
(`#/obs`) propose `http://127.0.0.1:<port>` ; l'adresse réseau (`GET /api/network`) n'y est lue et montrée
que sur demande.

## 5. Autres vérifications (novembre 2026)

| Sujet | Constat |
|---|---|
| Traversée de chemin, fichiers statiques | `tower-http::ServeDir` refuse les composants `..` ; testé avec `/../`, `%2e%2e`, `..%2f`, `..\` (`static_files_stay_inside_the_ui_folder`). |
| Identifiants dans les chemins | sessions : `corde_store::valid_id` (lettres, chiffres, `-`, `_`, ≤ longueur max, pas de point initial) avant tout accès disque ou suppression ; dispositions : `layouts::valid_id` ; résultats : entier ; tour : entier ; canaux : liste fermée. |
| Paramètre `base` (`Dossier/Fichier.svm`) | `svm_library::resolve_relative` : exactement deux composants, ni `..`, ni `:`, ni caractère de contrôle, extension `.svm` ; reste dans `Settings`. Noms de fichiers écrits nettoyés (`file_name_for`), jamais d'écrasement. |
| Taille des corps | 1 Mio par requête (avant : 2 Mio, défaut d'axum) ; 16 Kio par message WebSocket (avant : 64 Mio). Texte du ressenti ≤ 2000 caractères, ≤ 8 plans par demande de fichiers. |
| Clé de l'extracteur LLM | lue dans la variable d'environnement `CORDE_ANTHROPIC_API_KEY`, envoyée seulement dans l'en-tête `x-api-key` vers `api.anthropic.com` ; jamais journalisée (le `Debug` de l'extracteur ne l'affiche pas, les erreurs ne citent que le statut HTTP) ; `/api/health` dit seulement `anthropic` ou `dictionary`. |
| Chemins dans les erreurs | 5xx : détail masqué aux autres appareils (§3). 4xx : messages sans chemin absolu (noms relatifs `Dossier/Fichier.svm`, identifiants). **Reste visible au réseau quand l'accès est activé** : le chemin du dossier de données (`/api/storage`), du dossier de résultats (`/api/profile`) et du dossier `Settings` du jeu (`/api/setups`), affichés par l'interface ; ils peuvent contenir le nom du compte Windows. Accepté : réseau de confiance (§2). |

## 5 bis. Mises à jour de l'application de bureau (0.6)

Décision : [ADR 0009](adr/0009-mises-a-jour-releases-github-signees.md). Code : `apps/desktop/src-tauri/src/updater.rs`
(liaison avec `tauri-plugin-updater` 2.14), `updates.rs` (règles, testées), `.github/scripts/updater_manifest.py` (CI).

**Chaîne de confiance**

1. La CI construit l'installeur `Corde_<version>_x64-setup.exe` (job `desktop`, windows-latest) et la CLI
   Tauri le **signe avec la clé privée de mise à jour** (minisign, Ed25519), secret
   `TAURI_SIGNING_PRIVATE_KEY` du dépôt, donné à la seule étape de construction, jamais sur une PR ni un
   fork. La signature (`.sig`) couvre l'installeur final et un commentaire signé
   `timestamp:…\tfile:Corde_<version>_x64-setup.exe\tversion:<version>`.
2. Le job `version` vérifie que la signature vient bien de la clé dont la **clé publique** est dans
   `tauri.conf.json` (identifiant de clé), qu'elle est faite pour cette version et ce fichier, puis publie
   la release `v<version>` avec `latest.json` (version, notes, date, adresse de l'installeur, signature).
3. L'application lit `latest.json` en **HTTPS** sur une adresse fixe compilée dans l'exécutable
   (`plugins.updater.endpoints`, http refusé en build de production), télécharge l'installeur, puis
   **vérifie la signature avec la clé publique embarquée** (`plugins.updater.pubkey`, dans l'exécutable,
   copie de `apps/desktop/updater-public-key.txt`). Signature absente ou invalide : rien n'est installé.
4. `requireSignedVersion` est activé : la version **signée** doit être celle qu'annonce `latest.json`. Un
   `latest.json` falsifié ne peut donc pas présenter un ancien installeur (authentique, bien signé) comme
   une nouvelle version ; et l'application n'installe qu'une version **strictement plus récente**
   (`updates::is_newer`) : pas de retour en arrière.
5. L'installation ne part que sur un **clic** du pilote, jamais pendant que le jeu est détecté ou qu'un
   enregistrement est en cours ; l'installeur NSIS (mode passif, par utilisateur, sans droits
   administrateur) remplace les fichiers du programme, pas les données (`%LOCALAPPDATA%\Corde`).

**Ce que la signature de mise à jour garantit** : l'installeur téléchargé est, octet pour octet, celui
qu'a signé le détenteur de la clé privée de Corde, pour la version annoncée. Ni GitHub, ni un proxy, ni un
réseau compromis, ni un `latest.json` modifié ne peuvent faire installer autre chose.

**Ce qu'elle ne garantit pas** :

- **Ce n'est pas une signature de code Windows (Authenticode)**. Windows ne la connaît pas : SmartScreen
  et Smart App Control voient un installeur non signé, comme avant (ADR 0007). Elle ne protège que le
  chemin « mise à jour depuis l'application » ; un installeur téléchargé à la main n'est vérifié par
  personne (comparer avec la release).
- Elle ne dit rien de la qualité du code : elle prouve **qui** a publié, pas **ce que** fait la version.
  Tout ce qui est poussé sur `main` du dépôt d'origine avec un nouveau numéro est signé et publié
  automatiquement : la protection de la branche `main` et du compte GitHub (double authentification) fait
  partie de la chaîne.
- **Perte ou fuite de la clé privée** : fuite → quelqu'un qui contrôle aussi la page des versions (compte
  GitHub) peut pousser une mise à jour ; perte → les versions installées ne peuvent plus être mises à jour
  depuis l'application (il faut réinstaller à la main une version embarquant une nouvelle clé publique).
  La clé est hors du dépôt, chez le propriétaire, et dans les secrets du dépôt.
- `latest.json` n'est pas signé lui-même : il peut être bloqué ou retardé (on reste alors sur la version
  installée, « Impossible de vérifier pour l'instant »), ses notes peuvent être modifiées, mais pas
  l'installeur ni la version installée.
- La première version dotée du bouton (0.6.0) s'installe encore à la main : c'est elle qui apporte la clé
  publique.

**Réseau** : seule l'application de bureau contacte GitHub (`github.com`, puis le stockage de fichiers,
un domaine en `*.githubusercontent.com`), en `GET`, sans cookie ni identifiant ; le serveur local de Corde n'a
aucune route de mise à jour. Vie privée : [PRIVACY.md](../PRIVACY.md#mises-à-jour-de-lapplication-de-bureau-activé-par-défaut-désactivable).

## 6. Tests

- `crates/corde-server/src/guard.rs` (unitaires) : `Host` réseau coupé / activé, origines acceptées et
  refusées, classes des routes, intergiciel sur un petit routeur (403 GET compris, pré-requêtes, WebSocket,
  routes PC depuis un « téléphone » simulé, masquage des 5xx).
- `crates/corde-server/tests/access_policy.rs` (serveur réel, requêtes HTTP brutes et WebSocket) : `Host`,
  `Origin`, CORS, 413, traversée de chemin, et — si la machine a une adresse réseau — les routes réservées au
  PC appelées depuis cette adresse.
- `crates/corde-server/tests/settings_api.rs`, `setups_api.rs` : gardes existantes ; `online_schedule`
  refusé depuis un autre appareil.
- `crates/corde-server/src/races/` (unitaires) et `tests/races_api.rs` : liste fermée et `https`, règles de
  cache, aucune lecture tant que le réglage est faux, redirections limitées au site, en-têtes réels envoyés
  (serveur HTTP local), pas de cookie, limite de taille, validation des choix et des réglages, copie de
  diagnostic, rappel reçu sur le WebSocket.
- `tests/access_policy.rs` → `an_obs_browser_source_loads_the_overlay_from_the_server` (0.5) : la page
  overlay, ses fichiers, `GET /api/layouts` et le WebSocket chargés comme le fait une source OBS, depuis
  `127.0.0.1`, `localhost` et, si la machine a une adresse réseau, depuis cette adresse avec l'accès réseau
  activé.
- Mises à jour (0.6) : `apps/desktop/src-tauri/src/updates.rs` et `settings.rs` (banc des modules sans
  Tauri, hors dépôt) — refus pendant le jeu / un enregistrement / état inconnu, comparaison de versions,
  calendrier, une notification par version, chemins personnels masqués dans le journal, réglage
  `update_auto_check` vrai par défaut ; `packages/ui/src/lib/updates.test.ts`, `desktop.test.ts` (contrat
  des commandes) ; `.github/scripts/updater_manifest.py` essayé hors dépôt contre une signature produite
  comme la CLI Tauri et vérifiée comme le plugin (mauvaise clé, mauvaise version, signature absente,
  section du CHANGELOG absente : refus).
- Serveur Corde (0.7) : `crates/corde-server/src/hub/auth.rs` (unitaires : code à usage unique, trois
  erreurs, expiration, limite par adresse et totale, jeton haché, fichier 0600, révocation, cookie),
  `guard.rs::hub_route_classes`, `tests/hub_access.rs` (serveur réel : routes fermées sans jeton, cookie
  `HttpOnly; SameSite=Strict; Path=/` sans `Secure`, jeton faux / révoqué sur HTTP et WebSocket, WebSocket
  fermé à la révocation, administrateur, routes refusées, routes locales et commandes `pair | devices |
  revoke`, garde `Host` / `Origin` toujours devant), `tests/hub_sync.rs` (deux serveurs réels : coupure,
  reprise, aucun doublon, analyse identique, ressentis dans les deux sens, version incompatible refusée,
  envoi au démarrage et à la fin d'un enregistrement), `hub_link.rs` (jeton jamais montré, fichier 0600).
- Les scripts de capture (`packages/ui/scripts/screenshot-home.mjs`, `screenshot-sessions.mjs --spawn`)
  échouent sur toute erreur de console : ils vérifient que l'interface fonctionne depuis une autre origine
  locale.
- Application Android (1.0) : `guard.rs` (`http://localhost` accepté sur toute adresse, jamais « ce PC »
  depuis une adresse du réseau), `tests/mobile_app.rs` (appairage et jeton sur un vrai serveur Corde avec
  cette origine), `packages/ui/src/mobile/android.test.tsx` (rien de `CordeNative` sur Android, rappels sur
  le canal, alarmes exactes seulement autorisées), et la CI (`android.yml` : autorisations exactes,
  composants exportés, sauvegardes coupées, aucune bibliothèque Google, essai sur un émulateur sans
  services Google).
- Application iPhone (0.8) : `guard.rs` (unitaires : `capacitor://localhost` accepté sur toute adresse,
  imitations refusées, jamais « ce PC », pré-requête avec `Authorization`), `hub/auth.rs` (jeton dans le
  sous-protocole, seulement sur une demande de WebSocket), `tests/mobile_app.rs` (serveurs réels : PC avec
  l'accès réseau ouvert — API, CORS, WebSocket avec et sans `corde.v1`, routes du PC refusées même depuis la
  boucle locale ; accès réseau fermé → 403 ; serveur Corde — appairage `kind: app` sans cookie, `Bearer`,
  jeton en sous-protocole accepté et jamais renvoyé, faux jeton et jeton dans l'adresse refusés, révocation),
  `packages/ui/src/mobile/*.test.ts` (adresses acceptées, jeton jamais dans une entrée de la liste, un
  journal ni un message d'erreur, rappels, planning), `scripts/screenshot-app.mjs` (aperçu avec un faux
  pont : rien ne sort de la boucle locale sauf par le HTTP natif, vers les deux adresses du planning).

## 7. Le serveur Corde (mode hub)

Décision : [ADR 0010](adr/0010-serveur-domestique.md). Guide : [SERVEUR.md](SERVEUR.md), accès extérieur :
[ACCES-EXTERIEUR.md](ACCES-EXTERIEUR.md). Code : `crates/corde-server/src/hub/` (`auth.rs`, `routes.rs`,
`receive.rs`, `cli.rs`), `guard.rs` (`hub_route_access`), `hub_link.rs` (côté PC), `sync_wire.rs`.

`corde-server --role hub` tourne sur une machine toujours allumée de la maison (Raspberry Pi 5). Il ne lit
aucun jeu, n'enregistre rien, n'écrit que dans son dossier de données (`/var/lib/corde`), et écoute sur
toutes les interfaces. Contrairement au PC, **il exige un appareil appairé pour tout** : il est fait pour
rester allumé sans surveillance, et pour être joint à travers un VPN.

### 7.1 Modèle de menace

**Ce qui protège :**

1. **Le VPN** (choisi et monté par le pilote, [ACCES-EXTERIEUR.md](ACCES-EXTERIEUR.md)) : le serveur n'est
   jamais exposé à Internet ; dehors, seul un appareil qui a une clé VPN entre dans la maison, et le trafic
   est chiffré. Le pare-feu du Pi empêche le tunnel d'atteindre le reste du réseau.
2. **L'appairage** : même sur le réseau de la maison ou dans le VPN, un appareil ne voit rien sans un jeton
   d'appareil, obtenu seulement avec un code lu **sur le serveur** (`sudo corde-server pair`).
3. **La garde du §3** (`Host`, `Origin`, tailles) reste devant tout : une page web malveillante ouverte sur
   un appareil appairé ne peut ni lire l'API (origine refusée, cookie `SameSite=Strict` non envoyé), ni
   s'en servir par un nom de domaine piégé (DNS rebinding).
4. **Le service systemd durci** (`packaging/linux/corde.service`) : compte `corde` sans droits ni
   connexion, `NoNewPrivileges`, `ProtectSystem=strict`, `ProtectHome`, `PrivateTmp`, `PrivateDevices`,
   `ReadWritePaths=/var/lib/corde`, aucune capacité, appels système filtrés.

**Ce qui ne protège pas :**

1. **HTTP en clair sur le réseau local.** À la maison (sans VPN), le trafic entre le téléphone et le Pi
   n'est pas chiffré : une machine hostile sur le même réseau, ou quelqu'un qui contrôle le Wi-Fi, peut lire
   ce qui passe **et voler le cookie** d'un appareil, donc agir en son nom jusqu'à sa révocation. Le cookie
   n'a pas l'attribut `Secure` (il ne serait pas envoyé en HTTP). Protège ton Wi-Fi (WPA2/WPA3, mot de passe
   solide, pas d'invités sur le même réseau que le Pi).
2. **Un appareil appairé volé** garde l'accès **tant qu'il n'est pas révoqué** (page Appareils ou
   `sudo corde-server revoke`), et un téléphone volé avec son profil VPN entre dans la maison tant que sa clé
   VPN n'est pas révoquée : il faut faire les deux.
3. **Qui a un accès administrateur au Pi** (compte `sudo`, carte SD en main) a tout : les données sont en
   clair sur le disque, et il peut fabriquer des codes d'appairage.
4. **Un appareil appairé** voit tout ce que montre l'interface (sessions, résultats avec les noms des autres
   pilotes) et peut écrire des ressentis, choisir des courses, envoyer des sessions (y compris de fausses :
   tout appareil appairé peut parler le protocole du PC). Les actions destructrices demandent un appareil
   administrateur.
5. **Un PC compromis** qui est appairé peut envoyer n'importe quoi au serveur (dans les limites de taille et
   de forme du §7.4).

### 7.2 Appairage et jetons

| Élément | Règle |
|---|---|
| Code d'appairage | 8 chiffres tirés du générateur aléatoire du système (sans biais), **un seul à la fois**, usage unique, **10 minutes** ; **3 erreurs l'annulent** ; jamais écrit dans le journal ni dans une réponse d'erreur |
| Obtenir un code | uniquement sur le serveur : `sudo corde-server pair` appelle `POST /api/hub/local/pairing-code` sur `127.0.0.1`, **sans** en-tête `Origin`, avec la clé de `hub/control.json` (nouvelle à chaque démarrage, fichier 0600 du compte `corde`, lisible par root) |
| Limite d'essais | 5 par adresse et 30 au total par tranche de 10 minutes (429 + `Retry-After`), comptés avant de regarder le code ; une adresse IPv4 vue en IPv6 (`::ffff:`) compte pour la même |
| Jeton d'appareil | 256 bits aléatoires (64 caractères hexadécimaux), remis **une fois** : cookie `corde_device` (`HttpOnly; SameSite=Strict; Path=/; Max-Age` ≈ 400 jours, pas `Secure`) pour un navigateur, corps de la réponse pour le PC et l'application iPhone (`"kind": "app"`). Le PC le garde dans `hub-link.json` (0600 sous Unix ; `%LOCALAPPDATA%` est propre à l'utilisateur sous Windows), l'application iPhone dans son stockage (§8) ; les deux l'envoient en `Authorization: Bearer`, et l'application, sur le WebSocket, en entrée `corde.token.<jeton>` de l'en-tête `Sec-WebSocket-Protocol` (lue seulement sur une demande de passage en WebSocket ; le serveur répond `corde.v1`, jamais cette entrée) |
| Stockage côté serveur | `hub/devices.json` (0600, dossier `hub/` en 0700) : identifiant, nom, **SHA-256 du jeton**, dates, administrateur ; le jeton lui-même n'est jamais écrit. Un fichier illisible est mis de côté et le serveur démarre **sans aucun appareil** (fermé, jamais ouvert) |
| Vérification | SHA-256 du jeton présenté comparé à **chaque** empreinte en temps constant (`subtle`), sans sortie anticipée |
| Révocation | immédiate : l'appareil disparaît de la liste, ses requêtes suivantes reçoivent 401, ses WebSockets ouverts sont fermés (code 4401) |
| Administrateur | le premier appareil appairé ; ensuite, un administrateur en désigne d'autres ; le dernier administrateur ne peut pas retirer ses droits (409) ; s'il n'en reste aucun, le prochain appairé l'est (`sudo corde-server pair --admin` force ce rôle) |
| Journal | appairages (adresse, identifiant, nom), refus (adresse, motif), révocations ; jamais un code, un jeton ni une empreinte |

### 7.3 Routes du serveur

Toutes les règles du §3 s'appliquent d'abord (`Host` : adresses IP, nom de la machine, `<nom>.local`, et les
noms donnés par `--allow-host` ; `Origin` ; tailles ; 5xx sans détail). Puis :

| Accès | Routes |
|---|---|
| **Public** (sans jeton) | `GET /api/hub/pairing` (rôle, version, cet appareil est-il appairé), `POST /api/hub/pair` (`{code, name?, kind?}`) ; et les fichiers de l'interface (`GET /*`), qui affichent l'écran de code |
| **Commandes locales** (boucle locale + pas d'`Origin` + clé de contrôle) | `POST /api/hub/local/pairing-code`, `GET /api/hub/local/devices`, `DELETE /api/hub/local/devices/{id}` |
| **Appareil appairé** | toutes les autres routes `/api/*` du §4 (lectures, ressentis, dispositions, courses…) et `/ws` ; `GET /api/hub/status` ; `GET /api/hub/devices` ; `PATCH` / `DELETE /api/hub/devices/{id}` (soi-même ; un autre : administrateur ; changer `admin` : administrateur) ; `/api/sync/*` (§7.4) ; **toute route inconnue** (401 sans jeton, puis 404) |
| **Administrateur** | `DELETE /api/sessions/{id}` (+ pierre tombale : le PC ne la renvoie plus), `POST /api/sessions/{id}/analysis/recompute`, `DELETE /api/layouts`, `DELETE /api/layouts/{id}`, `PUT /api/settings` (seulement `online_schedule` ; `lan_access` → 400) |
| **Refusées** (403 `not_on_server`) | `POST /api/sessions/{id}/setup-files` (écrirait hors du dossier de données ; le jeu est sur le PC), `POST /api/profile/import` (pas de fichiers du jeu), `POST /api/recording/start`, `/stop` (pas d'enregistreur) |

Sur le PC, aucune de ces routes n'existe (404) : c'est ainsi que l'interface reconnaît un PC.

### 7.4 Réception des données d'un PC (`/api/sync/*`)

| Méthode | Chemin | Effet | Bornes |
|---|---|---|---|
| POST | `/api/sync/hello` | L ; note le dernier PC (nom, version, adresses de son direct) | adresses `http://hôte[:port]` seulement, ≤ 8, ≤ 100 caractères |
| POST | `/api/sync/results` | D (`results`, `result_laps`) | corps zstd ≤ 16 Mio, ≤ 256 Mio décompressé ; nom de fichier sans `/`, `\`, NUL ni point initial |
| PUT | `/api/sync/files/{id}/{name}` | D (`sync-incoming/<id>/<name>.part`) | morceau ≤ 1 Mio, fichier ≤ 2 Gio ; `id` validé (`valid_id`), `name` = `[A-Za-z0-9._-]+.parquet` sans `..` ; écrit seulement à la position attendue (sinon 409 + position) |
| POST | `/api/sync/files/{id}/{name}/commit` | D (déplace dans `sessions/<id>/`) | taille et SHA-256 vérifiés, sinon fichier partiel supprimé (422) |
| PUT | `/api/sync/sessions/{id}` | D (remplace les lignes de la session ; garde ses ressentis) | corps zstd borné comme plus haut ; session fermée ; tous ses fichiers présents (sinon 409) ; pierre tombale → 410 |
| GET / POST | `/api/sync/sessions/{id}/feedback` | L / D (ajoute les ressentis et itérations manquants, ne supprime rien) | identifiants hexadécimaux, texte ≤ 4000 caractères |

Chaque requête porte `corde-sync-version` et `corde-schema-version` : un PC plus récent que le serveur est
refusé (409 `server_outdated`, « mets à jour le serveur »), et le PC refuse de même un serveur plus ancien.

### 7.5 Côté PC

- `hub-link.json` (dossier de données du PC) garde l'adresse et le jeton ; `GET /api/hub-link` ne renvoie
  jamais le jeton ; aucune ligne du journal ne le contient.
- Le PC ne parle qu'à l'adresse enregistrée, en HTTP, **sans proxy**, avec des délais (5 s pour se
  connecter, 120 s par requête) ; il n'envoie que des sessions **fermées**.
- Toutes les routes `/api/hub-link` qui changent quelque chose sont réservées au PC (§4).

## 8. L'application iPhone

Décision : [ADR 0011](adr/0011-application-mobile-capacitor.md). Guide : [IOS.md](IOS.md). Code :
`packages/ui/src/mobile/` (application), `crates/corde-server/src/net.rs` (`MOBILE_APP_ORIGIN`),
`guard.rs` (`origin_allowed`, CORS), `hub/auth.rs` (`token_of`, `WS_PROTOCOL`), `ws.rs`.

### 8.1 Ce que le serveur accepte

| Élément | Règle |
|---|---|
| Origine | `capacitor://localhost` exactement (schéma `capacitor`, hôte `localhost`, sans port) : l'origine par défaut de Capacitor sur iOS (`server.iosScheme` = `capacitor`, `server.hostname` = `localhost` ; vérifié dans `@capacitor/ios` 8.6.0, `CAPInstanceDescriptor.swift` / `.m`, et `@capacitor/cli`, `declarations.d.ts`), laissée telle quelle dans `apps/mobile/capacitor.config.json`. `capacitor://autre-chose`, `ionic://localhost`, un port ou des identifiants : refusés |
| « Ce PC » | jamais : `net::origin_is_local` ne la connaît pas, donc `may_change_settings` la refuse. Les routes réservées au PC (§4) répondent 403 à l'application, même si la connexion venait de la boucle locale |
| `Host` | inchangé : l'application tape l'adresse IP du PC (ou son nom `.local`), qui n'est acceptée que si l'accès réseau est activé (§3, règle 1). Accès réseau coupé : 403 |
| CORS | l'origine est reflétée, `Authorization` est un en-tête autorisé ; pas de cookie entre origines |
| WebSocket | origine acceptée ; si l'application demande le sous-protocole `corde.v1`, le serveur le choisit (`WebSocketUpgrade::protocols`), sinon il n'en choisit aucun (navigateur : inchangé) |
| Serveur Corde | appairage `kind: "app"` : le jeton arrive dans la réponse, aucun cookie. Ensuite `Authorization: Bearer` sur HTTP ; sur le WebSocket, l'entrée `corde.token.<jeton>` de `Sec-WebSocket-Protocol`, lue **seulement** sur une demande de passage en WebSocket (`Upgrade: websocket`), jamais renvoyée. Un jeton en paramètre d'adresse (`?token=`) n'est **pas** lu : l'adresse complète finit dans le journal des requêtes (`TraceLayer`), les en-têtes non |

**Pourquoi accepter cette origine ne retire rien** : le contrôle `Origin` protège contre une **page web**
ouverte dans un navigateur (§2). Une page web ne peut pas avoir l'origine `capacitor://localhost` : elle
n'existe que dans une application qui embarque Capacitor et sert ses propres fichiers. Une application, elle,
n'est pas un navigateur : elle peut déjà envoyer n'importe quel `Origin`, ou aucun (accepté, comme `curl`).
L'accès réseau reste désactivé par défaut, et le serveur Corde exige toujours un appareil appairé.

### 8.2 Ce que fait l'application

- **Serveurs** : n'accepte une adresse en `http://` que vers le réseau privé ou local (IPv4 10/8, 172.16/12,
  192.168/16, 127/8, 169.254/16 ; IPv6 de boucle locale, fc00::/7, fe80::/10 ; `localhost`, `.local`, nom
  sans point) ; ailleurs `https://` est exigé (`mobile/addresses.ts`). Au plus 10 serveurs.
- **Jeton** : gardé sous sa propre clé dans le stockage de l'application (Capacitor Preferences =
  `UserDefaults`, bac à sable de l'application), jamais dans la liste des serveurs, jamais affiché, jamais
  dans un journal ni un message d'erreur (testé). Envoyé seulement au serveur auquel il appartient. Un 401
  du serveur (révoqué) l'efface ; « Oublier l'appairage » demande d'abord au serveur de révoquer cet
  appareil. Il n'est pas dans le trousseau iOS (pas de module officiel) : il est inclus dans les sauvegardes
  de l'iPhone.
- **Requêtes vers un serveur Corde** : `fetch` et WebSocket depuis la webview, `credentials: "omit"`.
- **Planning des courses** : HTTP natif (`CapacitorHttp`, `URLSession`), deux adresses en dur
  (`mobile/schedule.ts`), redirections suivies par l'application (au plus 3, `https://` sur le même site),
  20 s, 3 Mio, en-têtes du serveur (§4). `URLSession` garde les cookies qu'un site envoie : l'application
  les efface après chaque page (`CapacitorCookies.clearCookies`).
- **App Transport Security** (`Info.plist`) : `NSAllowsLocalNetworking` (noms sans point et `.local` en
  HTTP) et des exceptions `NSExceptionAllowsInsecureHTTPLoads` pour 10.0.0.0/8, 172.16.0.0/12 et
  192.168.0.0/16. D'après Apple (Quinn, DTS, forum « ATS and local IP addresses ») les adresses IP littérales
  échappent à ATS depuis iOS 10 ; la documentation d'iOS 17 parle d'exceptions en notation CIDR : les
  exceptions sont là pour le cas où ATS s'appliquerait aux adresses IP. Une adresse publique en HTTP reste
  bloquée par ATS (et refusée par l'application avant).
- **Liens `corde://`** : ouvrent une page d'une liste fermée, sans paramètre ; ne déclenchent aucune action.
  Les widgets et l'activité en direct ouvrent `corde://races`.

### 8.2 bis Widgets et activité en direct (0.9, photos des courses en 0.10)

Décision : [ADR 0012](adr/0012-widgets-activite-en-direct.md). Code : `packages/ui/src/mobile/widgets.ts`,
`widget-sync.ts` (ce qui est écrit), `apps/mobile/ios/App/Shared/SharedStore.swift` (où),
`CordeWidgets/` (l'extension), `App/CordeNativePlugin.swift` (le pont), `App/WidgetImages.swift` (les
photos du widget « Courses en images », 0.10).

| Question | Réponse |
|---|---|
| **Ce qui est partagé avec l'extension** | un bloc JSON (format v1, ≤ 48 Kio) : le planning public déjà lu par le téléphone (noms des courses, circuits, classes, durées, heures de départ, à travers les filtres du pilote), ses départs choisis, la langue de l'interface. Depuis la 0.10, pour chaque course et chaque choix : **l'exigence SR / DR** de sa catégorie (réglages de la page Courses) et **ce qui manque au pilote** pour elle (`miss` : « sr », « dr » — qu'il est en dessous, pas ses notes), et la **clé de sa photo** (condensé FNV-1a de l'adresse, pas l'adresse). À part, dans le même groupe : au plus **8 photos réduites** des courses (JPEG ≤ 48 Kio chacune, sans métadonnées), une par élément. Les mêmes informations que la page Courses et que les notifications de rappel |
| **Ce qui ne l'est jamais** | adresse d'un PC ou d'un serveur Corde, jeton d'appairage, liste des serveurs, données de télémétrie, sessions, **notes SR / DR du profil** (seulement « en dessous de l'exigence » ou non), adresses des photos. Le bloc est construit champ par champ (`buildPayload`), pas copié du stockage (essais : aucune adresse ni le profil dans le bloc) |
| **Où** | des éléments du trousseau iOS (`kSecClassGenericPassword`) : le bloc (service `io.github.dultra123.corde.widgets`) et, depuis la 0.10, les photos (service `io.github.dultra123.corde.widgets.img`, compte = la clé, au plus 8), dans le groupe partagé `<équipe>.io.github.dultra123.corde.shared`, déclaré par le droit `keychain-access-groups` des deux cibles (dans l'application, en second, après son propre groupe : iOS range dans le premier groupe tout élément écrit sans groupe nommé, donc rien d'autre que ce bloc et ces photos n'arrive par défaut dans le groupe que lit l'extension) et par la clé `CordeKeychainGroup` de leur `Info.plist`. Accessible après le premier déverrouillage (`kSecAttrAccessibleAfterFirstUnlock`, pour qu'un widget se redessine téléphone verrouillé), jamais synchronisé avec iCloud (`kSecAttrSynchronizable` faux), pas dans les sauvegardes d'un autre appareil (règle du trousseau pour cette classe d'accès : migré seulement par une sauvegarde chiffrée). Les éléments du trousseau **survivent à la suppression de l'application** : rien de secret, et ils sont remplacés ou effacés à la réinstallation |
| **Qui le lit** | l'application et son extension seulement : le groupe commence par l'identifiant d'équipe, iOS refuse tout autre application. Le diagnostic de la page Courses dit seulement si le groupe est trouvé, jamais son nom (il contient l'identifiant d'équipe) |
| **Réseau** | l'extension : **aucune requête** (contrôlé par `xcodeproj.test.mjs` : aucun appel réseau dans le code qu'elle compile). L'application : le planning de la page Courses (mêmes deux pages, mêmes règles §8.2) et, depuis la 0.10, les **photos des courses que ces pages citent** (au plus 8, seulement planning activé et application ouverte) : adresse passée par `safeImage` côté TypeScript, puis `https` seulement, aucun hôte en adresse IP littérale, ni `localhost` ni `.local` (redirections comprises), `URLSession` éphémère sans cookies, cache ni `Referer`, 15 s, 4 Mo au plus, décodage par ImageIO réduit (≤ 60 mégapixels annoncés, vignette ≤ 720 px), réencodage en JPEG. L'hébergeur d'une photo voit l'adresse IP du téléphone (comme quand l'interface l'affiche) |
| **Activité en direct** | démarrée et terminée par l'application, au premier plan seulement (règle d'iOS) ; contenu : nom, circuit, classes, catégorie, heure de départ. **Aucune notification push** (pas d'APNs, `pushType: nil`) : aucun jeton d'activité n'est créé ni envoyé où que ce soit ; le décompte tourne seul et iOS la marque périmée à l'heure du départ |
| **Pourquoi pas un App Group** | il faudrait le créer à la main sur le portail d'Apple (l'API ne le fait pas) ; le trousseau partagé ne demande rien. Effet pour la sécurité : équivalent (stockage dans le bac à sable de l'équipe, illisible par une autre application). Pour les photos, le trousseau n'est pas l'endroit naturel (pas de taille maximale documentée ; 8 × 48 Kio au plus) : repli possible vers un App Group, qui ne toucherait que `SharedStore.swift` |
| **Boutons du widget en images** | ‹ › sont des intentions (`AppIntent`, iOS 17) exécutées **par l'extension** : elles changent un décalage de page dans ses propres `UserDefaults`, rien d'autre (pas d'ouverture de l'application, pas de réseau) |

Ce qui reste visible : un widget ou l'activité affichent le nom des courses choisies et leur heure **sur
l'écran verrouillé** (comme les notifications de rappel). Le pilote les retire comme tout widget, coupe
l'activité dans les réglages de la page Courses, ou dans Réglages de l'iPhone → Corde → Activités en direct.

### 8.3 Ce qui ne protège pas

- Le trafic vers le PC ou le serveur Corde reste en **HTTP non chiffré** sur le réseau de la maison (§2,
  §7.1) : quelqu'un qui contrôle le Wi-Fi peut lire les données et voler le jeton d'appairage de
  l'application. Dehors : le VPN.
- Un **iPhone volé et déverrouillé** ouvre l'application et donc le serveur Corde jusqu'à la révocation de
  cet appareil (page Appareils, `sudo corde-server revoke`).

### 8.4 Signature et envoi vers TestFlight (CI)

Workflow `.github/workflows/ios.yml`, scripts `apps/mobile/scripts/` (`asc-key.sh`, `asc-api.mjs`,
`ios-signing.sh`, `xcode-signing.mjs`).

- **Secrets** : `APPSTORE_API_KEY` (clé d'API App Store Connect, accès Admin), son ID, l'ID d'émetteur et
  le Team ID. Ils ne sont lus que par les jobs `cle-api` et `testflight`, qui ne tournent que sur
  `dultra123/corde`, sur une poussée vers `main` ou un lancement manuel — jamais sur une demande de fusion
  ni sur un fork. La clé n'est écrite que dans un dossier temporaire du runner (0600), effacée à la fin.
- **Rien n'est affiché** : un secret mal collé n'est décrit que par sa longueur et sa forme ; le nom du
  certificat, qui contient le nom du titulaire du compte, est masqué dans le journal.
- **Certificat de distribution**, deux modes :
  - **sans** le secret facultatif `APPLE_DIST_KEY_BASE64` : sa clé privée est créée sur le runner, rangée
    dans un trousseau temporaire ouvert au seul outil de signature, et disparaît avec la machine. Elle n'est
    ni dans le dépôt, ni dans les secrets, ni dans un cache. Un certificat ne sert donc qu'à une exécution
    (et Apple envoie un e-mail de révocation à chaque build) ;
  - **avec** ce secret (clé privée RSA 2048, PEM en base64) : la clé est donnée à la seule étape
    « Certificat et profils… » du job `testflight`, vérifiée (base64, clé privée lisible sans mot de
    passe, RSA, 2048 bits, cohérente) sans jamais être affichée — un secret mal collé n'est décrit que par
    sa longueur et sa forme —, écrite en 0600 dans le dossier temporaire, puis dans le trousseau
    temporaire, effacés à la fin. Le certificat du compte qui porte la même clé publique (comparée en
    SPKI) et expire dans plus de 30 jours est réutilisé ; sinon un certificat est créé avec cette clé.
    **Conséquence assumée** : la clé privée vit maintenant aussi dans les secrets du dépôt (et chez le
    propriétaire) ; qui a la main sur ces secrets peut signer une application au nom du compte pendant la
    validité du certificat — c'était déjà vrai avec la clé d'API Admin, qui peut créer des certificats. En
    cas de doute : changer de clé (un nouveau secret ; le certificat de l'ancienne est révoqué au build
    suivant) ou supprimer le secret.
  - Le journal ne montre ni le contenu ni le sujet d'un certificat (son nom contient le nom du titulaire du
    compte, masqué s'il revient dans une réponse) : seulement l'empreinte SHA-256 (16 premiers caractères)
    de la clé publique stable et des dates d'expiration.
- **Ce que le script peut révoquer** : uniquement un certificat de type « Distribution » attaché à un profil
  dont le nom commence par « Corde CI » — c'est-à-dire ceux qu'il a créés. Jamais un certificat géré par
  Apple, de développement, ou attaché à un autre profil. Avec la clé stable, il ne révoque pas non plus le
  certificat de cette clé tant qu'il reste plus de 30 jours avant son expiration ; il peut le réutiliser
  même s'il n'est attaché à aucun profil « Corde CI », sans jamais le révoquer dans ce cas.
- **Ce que le script peut créer** (0.9) : un certificat par exécution (sans la clé stable ; avec, un par
  an), un profil App Store par identifiant
  (« Corde CI <n> » pour l'application, « Corde CI <n> widgets » pour l'extension), et l'identifiant de
  l'extension `io.github.dultra123.corde.widgets` s'il manque (nom « Corde Widgets », aucune capacité).
  Jamais l'identifiant de l'application : s'il manque, arrêt avant tout ménage.
- **Contrôle avant l'envoi** : l'application, ses frameworks et l'extension doivent porter une signature
  « Apple Distribution », un profil embarqué, et le droit `keychain-access-groups` avec le groupe partagé ;
  seul « oui / non » est affiché (la valeur commence par l'identifiant d'équipe).
- **Le jeton de l'API** (ES256, 10 minutes) n'est envoyé qu'à `api.appstoreconnect.apple.com` ; une adresse
  de « page suivante » qui pointerait ailleurs est refusée.
- **Limite assumée** : une clé Admin peut tout faire dans App Store Connect, sauf ce qui est réservé au
  titulaire du compte. C'est l'accès avec lequel l'envoi a été vérifié ; qui a la main sur les secrets du
  dépôt a donc la main sur le compte App Store Connect.

## 9. L'application Android

Décision : [ADR 0013](adr/0013-application-android.md). Guide : [ANDROID.md](ANDROID.md). Code : le même
que l'iPhone (`packages/ui/src/mobile/`, choix par plateforme dans `device.ts`, `android.ts`), projet
`apps/mobile/android/` (manifeste, `res/xml/network_security_config.xml`,
`res/xml/data_extraction_rules.xml`, `app/build.gradle`), CI `.github/workflows/android.yml`.

### 9.1 Origine et serveur

| Élément | Règle |
|---|---|
| Origine des pages | `http://localhost` (sans port) : `server.androidScheme` = `http` dans `apps/mobile/capacitor.config.json` ; la webview sert l'interface embarquée à cette adresse, sans passer par le réseau. Choisie parce que la valeur par défaut de Capacitor (`https://localhost`) ferait de chaque appel au PC (`http://192.168.…`) du « contenu mixte », bloqué par la webview sauf à l'autoriser pour tout (`allowMixedContent`, laissé à `false`) |
| Côté serveur | **rien à changer** : `http://localhost` est une origine locale (`net::origin_is_local`, §3 règle 2), déjà acceptée pour l'`Origin`, CORS (avec `Authorization`) et le WebSocket, sur le PC comme sur le serveur Corde. Fonctionne avec un Corde 0.8 ou plus |
| « Ce PC » | jamais depuis un téléphone : les routes réservées (§4) exigent **aussi** que la connexion TCP vienne de la boucle locale (`may_change_settings` : `peer_is_local` **et** origine locale). Un téléphone se connecte depuis son adresse du Wi-Fi ou du VPN, jamais `127.0.0.1` (essai `guard.rs`, `the_android_application_is_allowed_but_never_counts_as_this_pc`) |
| Serveur Corde | comme l'iPhone (§8.1) : appairage `kind: "app"`, `Authorization: Bearer`, jeton en sous-protocole WebSocket ; sans jeton, 401 (essai `tests/mobile_app.rs`, `the_android_application_pairs_with_the_home_server_from_its_origin`) |

**Pourquoi c'est sans risque nouveau** : une page web ouverte dans un navigateur n'a l'origine
`http://localhost` que si elle est servie **par la machine même du navigateur**, sur le port 80. Sur le PC,
c'était déjà le cas avant l'application Android (§3), et une telle page n'a de toute façon pas plus de droits
qu'un programme du PC ; sur le téléphone, seule l'application Corde sert des pages à cette adresse dans sa
webview. Différence avec l'iPhone, à connaître : `capacitor://localhost` n'est **jamais** une page du PC,
`http://localhost` **l'est** pour le contrôle d'origine ; c'est la règle de la boucle locale qui garde les
routes du PC.

### 9.2 Ce que fait l'application

- **Serveurs, jeton, requêtes, planning, liens** : les mêmes règles que sur iPhone (§8.2) — `http://`
  seulement vers le réseau privé ou local (`mobile/addresses.ts`), jeton à part dans les préférences de
  l'application (Capacitor Preferences = `SharedPreferences`, dans son bac à sable), planning par le HTTP
  natif (`CapacitorHttp` = `HttpURLConnection`) avec les mêmes limites, cookies effacés après chaque page,
  liens `corde://` limités à une liste de pages.
- **Trafic en clair** (`network_security_config.xml`) : **autorisé pour toutes les adresses**. Android ne
  sait pas le limiter à des plages d'adresses privées (un `domain-config` prend des **noms** d'hôtes, pas
  des plages comme `192.168.0.0/16`). La limite est donc dans le code de l'application, pas dans le système :
  `addresses.ts` refuse un serveur `http://` dont l'adresse est publique. Le fichier refuse en plus le clair
  vers `racecontrol.gg` et `lmu.tools` (que le code lit déjà en `https` seulement). Seules les autorités de
  certification du système sont acceptées (aucune ajoutée par l'utilisateur). À retenir : une faille qui
  ferait appeler une adresse publique en `http://` par l'application ne serait pas arrêtée par Android.
- **Sauvegardes** : `android:allowBackup="false"` et `data_extraction_rules.xml` excluent **tout** (tous
  les domaines, sauvegarde dans le nuage et transfert d'appareil à appareil). Le jeton d'appairage, les
  adresses des serveurs et les choix de courses ne partent ni chez Google ni sur un autre téléphone.
  Différence avec l'iPhone, où ces données suivent les sauvegardes de l'appareil (§8.2).
- **Autorisations** (liste exacte vérifiée par la CI sur le manifeste fusionné) : `INTERNET` ;
  `POST_NOTIFICATIONS`, `SCHEDULE_EXACT_ALARM`, `RECEIVE_BOOT_COMPLETED`, `WAKE_LOCK` (rappels) ; `CAMERA`
  (demandée au toucher de « Scanner le QR code ») ; la permission « signature »
  `io.github.dultra123.corde.DYNAMIC_RECEIVER_NOT_EXPORTED_PERMISSION` d'androidx.core (protège ses propres
  récepteurs, accordée à Corde seule). Aucune localisation, aucun accès aux fichiers.
- **Aucune bibliothèque Google** : ML Kit (que le module du lecteur de QR code tire, et qui envoie des
  rapports d'usage à Google), Play services, Firebase et `datatransport` sont exclus de l'APK ; la CI vérifie
  les dépendances et les classes définies dans l'APK. L'application fonctionne sur un téléphone sans
  services Google (essayée sur une image AOSP).
- **Composants exportés** : l'activité principale seulement (lanceur et liens `corde://`), plus ceux des
  bibliothèques qu'une autorisation du système réserve au système ou à l'outil de débogage (liste affichée et
  vérifiée par la CI sur le manifeste fusionné). Les récepteurs du module de notifications ne sont pas
  exportés.
- **Débogage** : la webview n'est inspectable que dans le build Debug ; les **entrées de l'essai de fumée**
  (`DebugHooks.java` : programmer un rappel de test, appeler une adresse `http://`, ouvrir le lecteur) n'existent
  que dans le build Debug (`src/debug`) ; le build Release en a une version vide. Aucune ne lit ni ne change
  les données du pilote.

### 9.3 Ce qui ne protège pas

- Comme sur iPhone (§8.3) : le trafic vers le PC ou le serveur Corde reste en **HTTP non chiffré** sur le
  réseau de la maison ; un **téléphone volé et déverrouillé** ouvre le serveur Corde jusqu'à la révocation.
- L'APK s'installe hors du Play Store : le pilote doit autoriser « Installer des applis inconnues » pour son
  navigateur le temps de l'installation (il peut la retirer ensuite, [ANDROID.md §2](ANDROID.md#2-installer-lapplication-pour-le-pilote)).
  Une mise à jour n'est acceptée par Android que signée par **la même clé** que l'application installée.

### 9.4 Signature et publication (CI)

Workflow `.github/workflows/android.yml`, script `apps/mobile/scripts/android-signing.sh` (essais :
`android-signing.test.sh`).

- **Secrets** : `ANDROID_KEYSTORE_BASE64` (magasin PKCS12 encodé en base64, clé `corde`) et
  `ANDROID_KEYSTORE_PASSWORD` (le même pour le magasin et la clé). Lus seulement par le job `release`, qui
  ne tourne que sur `dultra123/corde`, sur `main`, jamais sur une demande de fusion ni un fork. Sans eux, le
  job le dit et réussit.
- **Le magasin** n'est écrit que dans un dossier temporaire du runner (0700, fichier 0600) et effacé à la
  fin du job, même en cas d'échec. Le mot de passe n'est passé qu'à Gradle, par l'environnement de l'étape ;
  il n'est écrit dans aucun fichier.
- **Rien n'est affiché** que des longueurs, des formes et l'**empreinte SHA-256** du certificat : ni le mot
  de passe, ni le magasin, ni le nom du certificat (`Owner: CN=…`, que le propriétaire est invité à ne pas
  remplir avec son vrai nom : le certificat est lisible par tous dans l'APK).
- **Contrôle** : `apksigner verify` (schéma v2 ou v3, un seul signataire, empreinte égale à celle du
  magasin) et `jarsigner` / `keytool` pour le bundle (même certificat).
- **Branches** : le même chemin est essayé avec une **clé jetable** créée par le job (`throwaway`), masquée
  dans le journal comme un secret ; rien n'est publié.
- **Publication** : `contents: write` seulement pour ce job ; il remplace l'APK de la pré-release
  `continuous` et n'ajoute un APK à une release numérotée que si elle n'en a pas (jamais remplacé).
