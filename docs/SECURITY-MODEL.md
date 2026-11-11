# Modèle de sécurité de Corde

> Ce document décrit ce que le serveur local de Corde expose, à qui, et ce contre quoi il protège.
> Code : `crates/corde-server/src/guard.rs` (politique), `crates/corde-server/src/net.rs`
> (origines locales, garde « depuis ce PC »), tests : `crates/corde-server/tests/access_policy.rs`.
> Signaler une faille : [SECURITY.md](../SECURITY.md). Vie privée : [PRIVACY.md](../PRIVACY.md).

## 1. Ce qui tourne

Corde est un serveur HTTP + WebSocket (axum) qui écoute sur le **port 8471** (repli 8472…8480 dans
l'application de bureau) et sert à la fois l'interface web et l'API `/api/*`. Il n'y a **pas de compte ni de
mot de passe** : c'est voulu, Corde vise un PC de jeu et le réseau de la maison. L'exposition est donc bornée
autrement :

- **Par défaut, le serveur n'écoute que sur `127.0.0.1`** : seul le PC lui-même peut l'ouvrir.
- L'accès depuis le réseau local (téléphone, tablette, OBS sur un autre PC) s'active par le bouton « Activer
  l'accès depuis mon téléphone » de l'écran d'accueil (réglage `lan_access` de `settings.json`, appliqué au
  redémarrage), ou par `--lan` / `--bind` en mode console. Le serveur écoute alors sur toutes les interfaces.
- L'interface est chargée de cinq façons, toutes prises en compte par la politique ci-dessous :

| Cas | Adresse de la page | Origine envoyée à l'API |
|---|---|---|
| Application de bureau (fenêtre Tauri) | `http://127.0.0.1:<port>/#/…` (la coquille fait naviguer sa webview vers le serveur embarqué : `apps/desktop/src-tauri/src/shell.rs`, `ui_url` → `server::local_url`) | `http://127.0.0.1:<port>` = même origine que l'hôte |
| Navigateur du PC | `http://127.0.0.1:8471` | même origine |
| Téléphone du réseau local | `http://<ip-du-pc>:8471` | même origine (`http://<ip-du-pc>:8471`) |
| OBS (source navigateur) | `http://127.0.0.1:8471/…` | même origine |
| Développement, scripts de capture | `http://127.0.0.1:<autre port>/?server=127.0.0.1:8471`, `vite dev` sur `localhost:5173` | origine locale (autre port de la boucle locale) |

Les pages propres à la coquille (écran de démarrage, `apps/desktop/dist` : `tauri://localhost` sous
macOS/Linux, `http://tauri.localhost` sous Windows) n'appellent pas l'API HTTP — elles passent par l'IPC de
Tauri — mais leurs origines sont acceptées quand même. Le code Rust de la coquille interroge le serveur par
`ureq` sur `http://127.0.0.1:<port>` sans en-tête `Origin` : accepté. Les quatre commandes IPC de la coquille
ne sont accordées qu'aux pages `http://127.0.0.1:*` et `http://localhost:*`
(`apps/desktop/src-tauri/capabilities/embedded-ui.json`).

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
   (qui part vers l'API d'Anthropic si l'extracteur LLM optionnel est activé, donc à tes frais) et démarrer
   ou arrêter l'enregistrement. Un programme (pas un navigateur) peut aussi forger `Host` et `Origin` : ces
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
     serveur de développement, scripts de capture) ;
   - une **origine de la coquille Tauri** : `tauri://localhost`, `http://tauri.localhost`,
     `https://tauri.localhost` ;
   - sinon **403** « origine non autorisée », **y compris pour un GET**. Une requête **sans `Origin`**
     (navigation directe, `curl`) est acceptée.
3. **CORS** : `Access-Control-Allow-Origin` ne reflète que les origines acceptées ci-dessus, pré-requêtes
   `OPTIONS` comprises (méthodes GET, POST, PUT, DELETE ; en-têtes `Content-Type`, `Accept`). Une pré-requête
   d'une autre origine reçoit 403.
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
| GET | `/api/layouts` | L | aucune | ouverte | dispositions du tableau de bord |
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
| GET | `/api/settings` | L | aucune | ouverte | `{lan_access}` |
| PUT | `/api/settings` | D ; ouvre ou ferme le serveur au réseau | PC | **PC** | inchangé |
| GET | `/api/network` | L | aucune | ouverte | adresses à ouvrir sur le téléphone |
| GET | `/ws` | L (télémétrie en direct ; le client n'envoie que `SetRate` et `Ping`) | aucune | ouverte, avec contrôle d'`Origin` | tableaux de bord, OBS |
| GET | `/*` | L (fichiers de l'interface, repli sur `index.html`) | aucune | ouverte, contrôle `Host` seulement | |

Tâches de fond (pas de route) : lecture de la mémoire partagée du jeu ; `GET` vers l'API locale du jeu
(`http://127.0.0.1:6397`, liste fermée de chemins en lecture seule, `lmu_api.rs`, `lmu_garage.rs`) avec
copie dans `diagnostics/lmu-api/` du dossier de données ; lecture des fichiers de résultats toutes les 60 s.

### Ce qui change pour l'utilisateur

Depuis un téléphone ou un autre appareil du réseau, ces actions **ne marchent plus** et affichent « cette
action n'est possible que depuis le PC qui fait tourner Corde » ; elles restent disponibles sur le PC :

- supprimer une session ;
- « Recalculer » l'analyse de l'ingénieur (l'analyse elle-même s'affiche toujours) ;
- « Réimporter » les résultats (l'import automatique toutes les 60 s continue) ;
- supprimer une disposition par l'API (l'interface ne le fait pas).

Créer les fichiers de réglage et changer l'accès réseau étaient déjà réservés au PC. Le tableau de bord, la
saisie du ressenti, la commande d'enregistrement et toutes les lectures restent disponibles au téléphone.

## 5. Autres vérifications (novembre 2026)

| Sujet | Constat |
|---|---|
| Traversée de chemin, fichiers statiques | `tower-http::ServeDir` refuse les composants `..` ; testé avec `/../`, `%2e%2e`, `..%2f`, `..\` (`static_files_stay_inside_the_ui_folder`). |
| Identifiants dans les chemins | sessions : `corde_store::valid_id` (lettres, chiffres, `-`, `_`, ≤ longueur max, pas de point initial) avant tout accès disque ou suppression ; dispositions : `layouts::valid_id` ; résultats : entier ; tour : entier ; canaux : liste fermée. |
| Paramètre `base` (`Dossier/Fichier.svm`) | `svm_library::resolve_relative` : exactement deux composants, ni `..`, ni `:`, ni caractère de contrôle, extension `.svm` ; reste dans `Settings`. Noms de fichiers écrits nettoyés (`file_name_for`), jamais d'écrasement. |
| Taille des corps | 1 Mio par requête (avant : 2 Mio, défaut d'axum) ; 16 Kio par message WebSocket (avant : 64 Mio). Texte du ressenti ≤ 2000 caractères, ≤ 8 plans par demande de fichiers. |
| Clé de l'extracteur LLM | lue dans la variable d'environnement `CORDE_ANTHROPIC_API_KEY`, envoyée seulement dans l'en-tête `x-api-key` vers `api.anthropic.com` ; jamais journalisée (le `Debug` de l'extracteur ne l'affiche pas, les erreurs ne citent que le statut HTTP) ; `/api/health` dit seulement `anthropic` ou `dictionary`. |
| Chemins dans les erreurs | 5xx : détail masqué aux autres appareils (§3). 4xx : messages sans chemin absolu (noms relatifs `Dossier/Fichier.svm`, identifiants). **Reste visible au réseau quand l'accès est activé** : le chemin du dossier de données (`/api/storage`), du dossier de résultats (`/api/profile`) et du dossier `Settings` du jeu (`/api/setups`), affichés par l'interface ; ils peuvent contenir le nom du compte Windows. Accepté : réseau de confiance (§2). |

## 6. Tests

- `crates/corde-server/src/guard.rs` (unitaires) : `Host` réseau coupé / activé, origines acceptées et
  refusées, classes des routes, intergiciel sur un petit routeur (403 GET compris, pré-requêtes, WebSocket,
  routes PC depuis un « téléphone » simulé, masquage des 5xx).
- `crates/corde-server/tests/access_policy.rs` (serveur réel, requêtes HTTP brutes et WebSocket) : `Host`,
  `Origin`, CORS, 413, traversée de chemin, et — si la machine a une adresse réseau — les routes réservées au
  PC appelées depuis cette adresse.
- `crates/corde-server/tests/settings_api.rs`, `setups_api.rs` : gardes existantes.
- Les scripts de capture (`packages/ui/scripts/screenshot-home.mjs`, `screenshot-sessions.mjs --spawn`)
  échouent sur toute erreur de console : ils vérifient que l'interface fonctionne depuis une autre origine
  locale.
