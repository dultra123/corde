# Architecture de Corde

> Version 0.1 — novembre 2026. Document de référence pour toute personne (ou agent) qui touche au code.
> Le cahier des charges fonctionnel est dans [SPEC.md](SPEC.md) ; les décisions datées dans [adr/](adr/).

## 1. Vue d'ensemble

```
                 PC de jeu (Windows ou Linux)                         Réseau local
┌───────────────────────────────────────────────────────────┐
│  Le Mans Ultimate ──► mémoire partagée LMU_Data (100 Hz)   │
│                   ──► API REST 127.0.0.1:6397              │
│                   ──► UserData\player\Settings\*.svm       │
│                   ──► UserData\Telemetry\*.duckdb          │
│                   ──► UserData\Log\Results\*.xml           │
│                                                           │
│  corde-server (Rust, 1 exécutable)                        │
│   ├─ corde-lmu : adaptateur LMU ─► TelemetryFrame         │
│   ├─ corde-core : LapTracker, métriques dérivées          │
│   ├─ enregistreur : SQLite (sessions/tours) + Parquet     │   ┌──────────────┐
│   ├─ import des résultats XML → profil pilote             │   │              │
│   ├─ corde-engineer : analyse + 3 orientations de setup   │◄──┤ Dashboard     │
│   └─ axum : HTTP (UI statique, /api) + WebSocket /ws ─────┼──►│ desktop/Tauri │
│                                                           │   │ mobile/Tauri  │
│  corde-sim : faux LMU pour développer sans le jeu         │   │ OBS browser   │
└───────────────────────────────────────────────────────────┘   └──────────────┘
```

Principes :

1. **Le serveur local est le seul composant qui connaît le jeu.** Les interfaces (desktop, mobile, source navigateur OBS) ne sont que des clients WebSocket qui reçoivent des `TelemetryFrame` normalisées.
2. **Un modèle de données unique, multi-jeux** (`crates/corde-protocol`), exporté en TypeScript. Ajouter un jeu = écrire un adaptateur qui remplit ce modèle.
3. **Local-first.** Aucune donnée ne quitte le PC sans action de l'utilisateur. Une synchronisation cloud restera optionnelle.
4. **Rust pour tout ce qui touche à la donnée** (acquisition, stockage, analyse) ; **web (React/TypeScript) pour tout ce qui s'affiche** ; **Tauri 2** pour emballer le tout en applications natives.

## 2. Dépôt (monorepo)

```
corde/
├─ Cargo.toml                 # workspace Rust
├─ crates/
│  ├─ corde-protocol/         # types partagés (serde + ts-rs) — LE contrat
│  ├─ corde-core/             # trait TelemetrySource, LapTracker, derive::*
│  ├─ corde-lmu/              # adaptateur Le Mans Ultimate (+ parseur des résultats XML)
│  ├─ corde-sim/              # simulateur LMU_Data pour dev/tests
│  ├─ corde-store/            # enregistrement : SQLite + Parquet, résultats importés, lecture, CLI corde-store
│  ├─ corde-engineer/         # ingénieur : analyse d'une session → rapport + 3 orientations de réglage
│  └─ corde-server/           # exécutable serveur (axum)
├─ packages/
│  ├─ protocol/               # @corde/protocol : types TS générés + constantes
│  └─ ui/                     # @corde/ui : dashboard React (widgets, grille)
├─ apps/
│  ├─ mobile/                 # application iPhone (Capacitor 8, projet Xcode SPM) — ADR 0011, docs/IOS.md
│  └─ desktop/                # application de bureau Windows (coquille Tauri 2) — voir apps/desktop/README.md
│     ├─ dist/                #   page locale « Démarrage de Corde… » / erreur (frontendDist, sans build)
│     ├─ assets/, scripts/    #   logo source (SVG) et génération du jeu d'icônes
│     └─ src-tauri/           #   crate corde-desktop, WORKSPACE CARGO À PART : serveur embarqué, fenêtre,
│                             #   zone de notification, veilleur, commandes IPC, configuration de l'installeur NSIS
├─ docs/                      # SPEC, ARCHITECTURE, ADR, guides, research/
├─ knowledge/                 # base de connaissances ingénieur (YAML)
├─ reference/                 # dump de layout mémoire, échantillons réels (anonymisés)
└─ .github/workflows/         # CI Linux + Windows : tests, binaires, installeur, pré-release « continuous »
```

Outils : Rust stable (≥ 1.85 ; ≥ 1.90 pour `apps/desktop`), `pnpm` 10, Node 22, Biome pour le TS, `cargo fmt` +
`clippy` pour le Rust.

**Application de bureau.** `apps/desktop/src-tauri` est exclu du workspace Cargo racine (`exclude`) : Tauri
tire WebView2 / GTK-WebKit et ne se compile pas sur toutes les machines, donc `cargo check --workspace` et
`cargo test --workspace` l'ignorent. Il dépend de `crates/corde-server` par chemin et démarre ce serveur
**dans son propre processus** (un thread, un runtime tokio) : la fenêtre native affiche l'UI web servie sur
`http://127.0.0.1:<port>`, exactement celle qu'un navigateur ou un téléphone reçoit. L'UI parle à la coquille
par quatre commandes IPC (`get_autostart`, `set_autostart`, `restart_app`, `app_version`), accordées à
l'origine locale seulement. Le job CI `desktop` (windows-latest) est le seul endroit où ce crate est compilé ;
il produit l'installeur `Corde_<version>_x64-setup.exe`.

## 3. Flux de données

1. **Acquisition** — `corde-lmu::LmuSource` implémente `corde_core::TelemetrySource` :
   - Windows : `OpenFileMappingW("LMU_Data")`, copie sous le verrou coopératif `LMU_SharedMemoryLockData` (ou après attente sur `LMU_Data_HoldEvent` puis `LMU_Data_DataEvent`), puis conversion.
   - Linux : `mmap` en lecture de `/dev/shm/LMU_Data` (alimenté par un pont dans le préfixe Proton, ex. winecarte), polling 8–16 ms, double lecture témoin de `mElapsedTime`.
   - Layout binaire : voir `reference/lmu-shared-memory-layout.txt` (offsets calculés depuis le header officiel ; les structs Rust ont des tests `size_of`/`offset_of`).
2. **Normalisation** — un `TelemetryFrame` par snapshot : `session` (ScoringInfo + météo), `player` (TelemInfo du joueur + sa VehicleScoringInfo), `standings` (toutes les voitures, décimé ~5 Hz).
3. **Diffusion** — `corde-server` pousse les frames dans un `tokio::sync::broadcast` ; chaque client WebSocket reçoit à la cadence qu'il demande (`ClientMessage::SetRate`, 1–60 Hz, défaut 20). Format JSON en v0 (`ServerMessage` tagué `type`), MessagePack prévu (même schéma).
4. **Suivi des tours** — `corde_core::LapTracker` émet un `LapRecord` à chaque passage de ligne ; le serveur le diffuse (`ServerMessage::Lap`) et l'enregistre.
5. **Enregistrement** — `corde_store::Recorder`, alimenté par le thread source (après les événements et le tour clos par la frame, avant la frame elle-même). Il démarre quand `in_realtime && player` (hors `SessionOver`), décime à `--record-rate` (50 Hz par défaut, sur le temps de session) et s'arrête quand la session change, que la source se perd, que le pilote reste > 60 s hors piste ou à l'arrêt du serveur (flush garanti) ; `POST /api/recording/{start|stop}` force. Sur disque : `<data-dir>/corde.db` (SQLite WAL, migrations versionnées : `sessions`, `laps` avec la plage de lignes de chaque tour, `events`, `setups`, `sample_files`) et `<data-dir>/sessions/<AAAAMMJJ-HHMMSS-circuit-type>/samples-NNN.parquet` (163 colonnes typées définies dans `crates/corde-store/src/channels.rs`, zstd + BYTE_STREAM_SPLIT, row groups de 4096 lignes, un fichier par ~10 min / 50 000 lignes). Chaque tour = un commit SQLite + un point de reprise Parquet ; un point de reprise au moins toutes les 30 s écrit un pied de fichier provisoire, donc un fichier en cours reste lisible après un crash. Lecture : `GET /api/sessions`, `/api/sessions/{id}`, `/api/sessions/{id}/laps/{n}/samples?channels=&points=` (décimation min/max par canal), `/api/storage`, `/api/recording` ; événements WS `recording_started` / `recording_stopped` ; CLI `corde-store` (list / show / export-csv). Volumétrie mesurée sur le simulateur : ≈ 335 o/ligne → ≈ 60 Mo/h à 50 Hz (données bruitées ; à remesurer sur LMU).
6. **Import des résultats** — `corde_server::results_import` scanne au démarrage puis toutes les 60 s (`POST /api/profile/import` force) le dossier `<UserData>/Log/Results/*.xml` (`--lmu-results-dir`, sinon le `UserData` annoncé par le jeu dans `LMU_Data.paths`, sinon les bibliothèques Steam). `corde_lmu::results` parse chaque nouveau fichier (nom + mtime) — format rFactorXML : en-tête, une section par session, `Driver` avec ses `Lap` et le `Stream` d'incidents ; attention, `isPlayer=1` marque tous les humains en multijoueur, le pilote local est résolu par `--player-name`, le nom de la session live, les imports précédents ou « le nom présent dans tous les fichiers ». `corde_store::results` range l'en-tête, les colonnes du joueur et le fichier parsé (JSON zstd) dans `results`, ses tours dans `result_laps` (migration v2), et agrège `GET /api/profile` (totaux, par circuit, par voiture, derniers résultats) ; `GET /api/results`, `/api/results/{id}` (classement complet + `linked_session_id` : session enregistrée du même circuit à ±10 min). UI : onglet « Profil » (`#/profile`, `#/results/<id>`).
7. **Analyse & ingénieur** (phase 2) — `corde_engineer::analyse_session` lit les tours propres d'une session (`Store`), segmente les virages (`|g_lat|`), calcule les métriques (équilibre par angles de dérive et gradient volant − lacet, blocages, pneus I/M/O, freins, plateforme, histogrammes d'amortisseurs, style, tours), produit des `Finding`s alignés sur `knowledge/diagnostics.yaml` et trois `SetupPlan`s (agressif / sûr / pluie) par le moteur de règles sur `knowledge/setup-rules.yaml` (embarqué, surchargeable par dossier). Le serveur sert `GET /api/sessions/{id}/analysis` (cache dans la table `analyses`, migration v3 ; `POST …/analysis/recompute`). Détails : `docs/ENGINEER.md`. À venir : UI « Ingénieur », écriture `.svm`, boucle de ressenti.

## 4. Conventions

- **Unités humaines dans le protocole** : °C, km/h et m/s, degrés, kPa, litres, secondes. Les conversions Kelvin/radians se font dans l'adaptateur (`corde_core::derive`).
- **Roues** : toujours l'ordre FL, FR, RL, RR.
- **Températures pneus** : `temp_c[0..3]` = gauche / centre / droite du pneu (convention rFactor), **pas** intérieur/extérieur. Intérieur = côté gauche pour les roues droites, côté droit pour les roues gauches.
- **Sentinelles** : un temps absent est `None`, jamais `-1` ou `0`.
- **Identifiants** en anglais dans le code ; documentation utilisateur en français.
- **Pas de copie de code GPL** (TinyPedal, plugin Iron Wolf, winecarte) ni du header S397 (licence restrictive) : nos structures sont réécrites à partir de la lecture du header, avec tests de taille/offset.

## 5. Ports et chemins

| Quoi | Valeur |
|---|---|
| HTTP + WebSocket corde-server | `http://127.0.0.1:8471` (LAN : `http://<ip-du-pc>:8471`), WS sur `/ws` |
| API REST de LMU | `http://127.0.0.1:6397` (clé `WebUI port` de `Settings.JSON`) |
| Données Corde (Windows) | `%LOCALAPPDATA%\Corde\` |
| Données Corde (Linux) | `~/.local/share/corde/` |
| Application de bureau (Windows) | programme : `%LOCALAPPDATA%\Programs\Corde\` ; journal : `%LOCALAPPDATA%\Corde\logs\corde-desktop.log` ; écoute sur `127.0.0.1` sauf `lan_access` dans `settings.json` ; port 8471, repli 8472…8480 |
| Dossier LMU (par défaut) | `C:\Program Files (x86)\Steam\steamapps\common\Le Mans Ultimate\` — à lire aussi dans `LMU_Data.paths.userData` |

## 6. Feuille de route technique

| Phase | Contenu | Livrable |
|---|---|---|
| 0 | protocole, adaptateur LMU, simulateur, serveur, dashboard v0 (6–8 widgets), layouts | `corde-server` + UI web (Linux build ici, Windows via CI) |
| 1 | CI Windows, coquille Tauri desktop, enregistrement SQLite/Parquet, import résultats XML | installeur Windows, premières sessions enregistrées |
| 2 | analyse post-session, diagnostics, 3 setups proposés, boucle de ressenti (LLM + règles), profil pilote | « l'ingénieur » |
| 3 | application mobile : iPhone par Capacitor (ADR 0011, 0.8 : courses sur le téléphone, serveurs, appairage ; widgets ADR 0012) ; Android par le même projet (ADR 0013, 1.0, sans widgets) ; découverte mDNS à venir | app mobile |
| 4 | adaptateurs ACC, AC EVO, iRacing, rF2, AMS2 | multi-jeux |
