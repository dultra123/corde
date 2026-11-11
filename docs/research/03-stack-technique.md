# 03 — Stack technique : évaluation et décisions

*Application de télémétrie pour simulateurs de course (LMU / rFactor 2 d'abord). État des technologies vérifié le 2 novembre 2026 (versions crates.io / npm interrogées ce jour, documentation officielle consultée). Un seul développeur principal assisté d'agents IA : chaque choix privilégie une base de code unique, outillée et testable.*

---

## Résumé exécutif

**Stack recommandée : un cœur Rust unique + une UI web unique, emballés par Tauri 2 (desktop Win/Linux et mobile iOS/Android).**

1. **Cœur Rust (Cargo workspace)** : lecture mémoire partagée (`memmap2` + `windows` sur Windows, fichiers `/dev/shm/*` sous Linux via pont Proton), adaptateurs de jeu derrière un trait `GameAdapter`, enregistreur, analyse et moteur de règles. Ce cœur est un **serveur local autonome** (axum 0.8, WebSocket binaire MessagePack, découverte mDNS) : l'UI, l'app mobile et même une source navigateur OBS ne sont que des clients.
2. **UI React 19 + TypeScript 7 + Vite 8**, un seul paquet de widgets partagé desktop/mobile ; grille **gridstack.js 14** (tactile natif, sauvegarde de layouts), graphes haute fréquence **uPlot**, carte de circuit en **Canvas 2D maison**.
3. **Shell Tauri 2.12** (desktop stable, mobile utilisable mais à prototyper en priorité : signature, WebSocket côté Rust, Bonjour via plugin natif iOS). Overlay in-game : fenêtre Tauri transparente/always-on-top/click-through (OK Windows et X11, limité sous Wayland).
4. **Stockage** : SQLite (`rusqlite`, bundled) pour sessions/tours/événements/setups/profil, **Parquet zstd** (crate `parquet` 60) pour les échantillons ; DuckDB différé. Export CSV natif, MoTeC `.ld` via writer dédié.
5. **Analyse et recommandation** : tout en Rust (réutilisable en WASM côté UI), règles en **tables de décision JDM (zen-engine, MIT)**, interprétation du ressenti par **Claude Haiku 5.5** en sorties structurées (JSON Schema, GA), mode hors-ligne = règles seules.
6. **Monorepo** pnpm 12 + Cargo workspace, Biome, Vitest/Playwright, cargo-nextest, GitHub Actions (Win/Linux/Android/iOS) avec `tauri-action`.
7. **Plan B** : si Tauri mobile bloque, garder le cœur Rust et la UI web intacts, remplacer le shell par **Electron 45 (desktop) + Capacitor 8 (mobile)** — aucun widget à réécrire.

Positionnement : seul outil **local-first, Linux natif, open stack** couvrant à la fois tableau de bord temps réel, mobile, analyse post-course et **recommandation de setup pilotée par le ressenti** ; les concurrents LMU (Delta, Trophi, GO Fast, Neuralic) sont Windows-only et/ou cloud/abonnement.

---

## 0. Contexte vérifié : d'où vient la donnée LMU ?

Avant de trancher la stack, l'état réel des sources de données (novembre 2026) :

| Source | Mécanisme | Cadence | Notes |
|---|---|---|---|
| **rF2SharedMemoryMapPlugin** (The Iron Wolf) | DLL dans `Plugins/`, mappings nommés `$rFactor2SMMP_Telemetry$`, `$rFactor2SMMP_Scoring$`, `$rFactor2SMMP_Extended$`, `…Rules$`, `…PitInfo$`, `…Weather$`, `…ForceFeedback$` | Telemetry 50 Hz, Scoring 5 Hz, PitInfo 100 Hz, FFB 400 Hz, Weather 1 Hz | Synchronisation « faible » par compteurs de version (seqlock) depuis v3.0 ; dernière release v3.7.15.1 (mars 2023). Utilisé par SimHub, Second Monitor, lmu-pitwall, crate Rust `kerb`. Buffers ~500 Ko chacun (128 véhicules max). |
| **Interface mémoire partagée native LMU** | Activée par `Settings > Gameplay > Enable Plugins` ; headers `SharedMemoryInterface` de Studio 397 ; mapping `LMU_Data` | non documentée publiquement | Lue par TinyPedal (`pyLMUSharedMemory`, MIT) et HyperTrace. La v1.3 (30 mars 2026) a « ajouté de nouveaux paramètres de télémétrie au fichier de mémoire partagée ». Avantage : aucune DLL à installer. |
| **API REST locale LMU** | `http://localhost:6397` (`WebUI port` dans `UserData/player/Settings.JSON`), activée par défaut | requête | Chat, replays, stints, tracé de circuit, setup garage (`enable_garage_setup_info`). rF2 équivalent sur 5397. |
| Logs MoTeC du jeu | `.ld`/`.csv` dans `Documents/Le Mans Ultimate/Telemetry` | post-session | Utile comme référence de format d'export. |

**Sous Linux/Proton** : Wine implémente `CreateFileMapping` nommé par des mappings anonymes gérés par wineserver, **pas** dans `/dev/shm` (bug Wine 54015, toujours non résolu). Trois ponts existent : (a) le fork **rF2SharedMemoryMapPlugin_Wine** (schlegp) qui écrit directement dans `/dev/shm/$rFactor2SMMP_*$` — c'est ce qu'utilise TinyPedal ; (b) **winecarte** (`wine2linux.exe` lancé dans le préfixe Proton via `winecarte-run %command%`, copie périodique vers `/dev/shm/<nom>`), qui couvre AC/ACC/AC Evo (`acpmf_*`), LMU (`LMU_Data` + SMMP), rF2, PC2/AMS2 (`$pcars2$`) ; (c) Datalink / simshmbridge (compilation manuelle). Conséquence : côté Linux, notre lecteur ouvre simplement des fichiers `/dev/shm/...` avec `memmap2` ; la différence avec Windows tient dans l'ouverture du handle, pas dans le parsing.

Sources : [Game Setup ModernTinyPedals](https://github.com/Keenny38/ModernTinyPedals/wiki/Game-Setup) · [rF2SharedMemoryMapPlugin](https://github.com/TheIronWolfModding/rF2SharedMemoryMapPlugin) · [pyLMUSharedMemory (fork, basé sur TinyPedal)](https://github.com/blortle-dev/pyLMUSharedMemory) · [LMU v1.3](https://news.racecontrol.gg/?p=1511) · [wiki LMU Telemetry](https://lemansultimate.wiki.gg/wiki/Telemetry) · [kerb/lmu/connection.rs](https://docs.rs/crate/kerb/latest/source/src/lmu/connection.rs) · [lmu-pitwall](https://github.com/Swizzjack/lmu-pitwall) · [HyperTrace](https://github.com/PoKPoK68/hypertrace) · [Wine bug 54015](https://list.winehq.org/archives/list/wine-bugs@list.winehq.org/thread/J47UZL64XJ27JOOD3M3R7NKXVS3REXLX) · [winecarte](https://github.com/srounce/winecarte) · [Fil Reiza « AMS2 shared memory on Linux » (Datalink, simshmbridge)](https://forum.reizastudios.com/threads/ams-2-shared-memory-on-linux.36396/) · [wine-linux-shm-adapter](https://github.com/Spacefreak18/wine-linux-shm-adapter) · [TinyPedal README (miroir, section Linux)](https://github.com/reloadggg/TinyPedal_CN)

---

## 1. Framework applicatif

### État des candidats (novembre 2026)

| Critère | **Tauri 2** | Electron + RN/Capacitor | Flutter | .NET (Avalonia / MAUI) | Python PySide6 (modèle TinyPedal) |
|---|---|---|---|---|---|
| Version actuelle | 2.12.5 (crates.io, 21 oct. 2026) ; 2.12.0 (26 sept.) : MSRV Rust 1.90, édition 2024, abandon Windows 7 | Electron 45.2.1 (31 oct. 2026, Chromium 156) ; Capacitor 8.6 ; RN 0.88 / Expo 58 | Desktop confié à **Canonical** (Google I/O, mai 2026) | Avalonia 12.x (Wayland opt-in depuis 12.1) ; MAUI 10, Linux seulement via pont Avalonia | PySide6 à jour ; TinyPedal tourne encore PySide2/6 |
| Accès mémoire partagée Windows | Natif : `windows` 0.62 (`OpenFileMappingW` + `MapViewOfFile`) ou `winmmf` 0.6 ; `memmap2` 0.9.12 pour les fichiers. Crate `shared_memory` 0.12.4 **figé depuis 2022** → à éviter. Référence : crate `kerb` 0.4 (LMU/iRacing/AC Evo, Windows-only, MIT) | Addon natif N-API ou sidecar (friction) | FFI Dart, plugin natif à écrire | P/Invoke trivial, écosystème SimHub en C# | `mmap`/`ctypes` : prouvé par TinyPedal |
| Lecture `/dev/shm` Linux | `memmap2::Mmap` sur le fichier, zéro dépendance | Node `fs` + parsing manuel | FFI | .NET `MemoryMappedFile` | `mmap` |
| Perf UI 60 Hz | Rendu par webview système (WebView2 / WKWebView / WebKitGTK / Android WebView). Canvas 60 Hz OK ; **risque WebKitGTK + NVIDIA** (renderer DMA-BUF → fenêtre blanche/lenteur, contournement `WEBKIT_DISABLE_DMABUF_RENDERER=1`) ; **macOS 13-15 bride WKWebView à 60 fps**, levé sur macOS 26 (non bloquant pour nous) | Chromium embarqué : rendu identique partout, le plus prévisible | Skia/Impeller, excellent | Skia, excellent | Qt, excellent mais widgets Qt à réécrire pour le mobile |
| Taille binaires | 3-15 Mo desktop (webview système) | 80-150 Mo par plateforme | 20-40 Mo | 30-70 Mo (trim) | 30-80 Mo (PyInstaller, hors numpy/pandas : « bien au-delà ») |
| Auto-update | `tauri-plugin-updater` 2.14 : signature Ed25519 **obligatoire**, JSON statique (GitHub Releases) ou serveur dynamique ; NSIS/MSI, `.app.tar.gz`, **AppImage uniquement** côté Linux ; desktop seulement (mobile = stores) | electron-updater / Squirrel, mûrs | Pas d'updater intégré | Velopack/Squirrel | Rien d'intégré |
| Packaging | NSIS + MSI (pas de MSIX natif), deb/rpm/AppImage/Flatpak/Snap/AUR ; APK/AAB, IPA | NSIS/MSI/AppImage/deb/rpm/snap ; mobile via stores | MSIX/deb/snap | MSIX/deb/rpm | PyInstaller/Nuitka, AppImage |
| Mobile iOS/Android | Supporté depuis 2.0 (oct. 2024) ; retours terrain mitigés : juillet 2025 « inadapté au mobile » (débogage iOS pénible, plugins Swift/Kotlin à écrire, docs faibles) ; évaluation mai 2026 (Tauri 2.11) : « plus proche de l'alpha » pour caméra/push/BLE — **aucun de ces besoins ne nous concerne** (nous : WebSocket + Bonjour + rendu) | Mature (deux bases de code UI ou Capacitor = même web) | Mature | Avalonia iOS/Android Tier 1-2 ; MAUI mature | **Aucun** |
| Overlay in-game (toujours au-dessus, transparent, click-through) | API `WebviewWindow` : `transparent`, `always_on_top`, `decorations:false`, `set_ignore_cursor_events`. Fonctionne sur Windows (WS_EX_TRANSPARENT) et X11 ; **Wayland** : pas de position curseur globale → toggle manuel (limite partagée par toutes les techs) ; exclusif-fullscreen jamais recouvrable (fenêtré sans bordure requis, comme SimHub/TinyPedal) | Idem (`setIgnoreMouseEvents`) | Multi-fenêtre tout juste arrivé | Avalonia OK | Qt OK (TinyPedal le fait) |
| Écosystème sim racing | Jeune (`kerb`, `motec-i2`) mais tout le parsing est du `#[repr(C)]` trivial | JS : peu | Dart : rien | **Le plus riche** (SimHub, iRacing SDK C#) | Python : TinyPedal, pyRfactor2SharedMemory |
| Coût pour 1 dev + agents IA | 1 langage backend (Rust) + 1 UI web pour 4 OS ; agents IA très efficaces sur React/TS, bons sur Rust ; mobile = même projet | 2 runtimes (Node + natif), 2 pipelines | Réécriture complète des widgets en Dart, pas de réutilisation web (OBS, navigateur) | Pas de réutilisation web ; Linux/Wayland récent ; licences Avalonia Accelerate pour certains outils | Pas de mobile : disqualifiant pour le besoin (3) |

### Recommandation : **Tauri 2.12**, avec une architecture qui rend le shell remplaçable

- Le **cœur est un binaire Rust headless** (`sim-core`) : adaptateurs de jeu, enregistreur, serveur axum (WS + REST + fichiers statiques de l'UI). Tauri l'embarque en desktop ; sur mobile, le même crate compile en « mode client » (WebSocket vers le PC) et pousse les frames à la webview via les **Channels Tauri** (prévus pour le streaming, fiabilisés en 2.12.3). L'UI ne parle jamais au réseau directement : cela évite les restrictions cleartext des webviews mobiles et garde un seul chemin de données.
- Bonus gratuit : n'importe quel navigateur (second écran, tablette sans app, **source navigateur OBS**) peut afficher les widgets servis par `sim-core`.
- Overlay : deuxième `WebviewWindow` transparente, à activer uniquement si le jeu est en fenêtré sans bordure ; sous Wayland, raccourci global pour basculer l'interactivité.

**Plan B** : Electron 45 + Capacitor 8 (même UI React, même `sim-core` lancé en sidecar). Déclencheur : si après 2 semaines de spike mobile Tauri la boucle « build signé iOS → WS vers le PC → 60 Hz » n'est pas stable. **Plan C** (si l'équipe devient C#) : Avalonia 12 — mais perte de la réutilisation web.

### Risques et mitigations
| Risque | Mitigation |
|---|---|
| WebKitGTK + NVIDIA (fenêtre blanche, canvas lent) | Détection au premier lancement, variable d'env posée avant création de la webview, fallback non-WebGL ; AppImage + Flatpak |
| Debug mobile Tauri (crash iOS sans stack) | Garder la logique mobile en Rust (testable sur desktop), webview = rendu pur ; Safari Web Inspector |
| Updater : pas de deb/rpm, clé privée irremplaçable | Distribuer AppImage comme canal auto-update, deb/Flatpak sans updater ; clé dans un coffre, sauvegarde hors ligne |
| Pas de MSIX (Microsoft Store) | Hors périmètre v1 ; GitHub Releases + site |
| Rust = courbe d'apprentissage | Parsing `#[repr(C)]` + tokio/axum sont bien couverts par les agents IA ; éviter l'unsafe hors des crates d'adaptateurs |

Sources : [Releases Tauri](https://tauri.app/release/tauri/) · [Capawesome : Electron vs Tauri (juil. 2026)](https://capawesome.io/docs/blog/electron-vs-tauri-for-capacitor-apps/) · [PkgPulse Electron vs Tauri 2026](https://www.pkgpulse.com/guides/electron-vs-tauri-2026) · [Rapport Tauri vs Electron (mars 2026 : MSIX, packaging, updater)](https://moklabs-research.pages.dev/reports/technology/2026-03-20-tauri-vs-electron-desktop-framework-decision) · [r/tauri « unsuitable for mobile » (juil. 2025)](https://reddit.zackpollard.pro/r/tauri/comments/1maezph/tauri_is_unsuitable_for_mobile) · [Décision Tauri vs Capacitor (mai 2026)](https://range-day-docs.pages.dev/decisions/Tauri-vs-Capacitor) · [endoflife.date Electron](https://endoflife.date/electron) · [Flutter desktop → Canonical](https://www.omgubuntu.co.uk/2026/05/flutter-desktop-canonical-maintained) · [Avalonia plateformes](https://docs.avaloniaui.net/docs/supported-platforms) · [Avalonia + MAUI Linux (The Register)](https://www.theregister.com/2025/11/13/dotnet_maui_linux_avalonia/) · [Tauri Linux graphics](https://v2.tauri.app/develop/debug/linux-graphics/) · [tauri-plugin-macos-fps (cap 60 fps WKWebView)](https://docs.rs/crate/tauri-plugin-macos-fps/latest/source/README.md) · [Updater Tauri](https://v2.tauri.app/plugin/updater/) · [Issue 11461 click-through](https://github.com/tauri-apps/tauri/issues/11461) · [Issue 9250 curseur + ignore events](https://github.com/tauri-apps/tauri/issues/9250) · [egui_overlay #37 (Wayland)](https://github.com/coderedart/egui_overlay/issues/37) · [tauri-runtime-servo (expérimental, desktop X11 seulement)](https://docs.rs/crate/tauri-runtime-servo/0.1.0/source/README.md) · [kerb](https://docs.rs/crate/kerb/latest) · [winmmf](https://docs.rs/crate/winmmf/latest/source/src/mmf.rs) · [Tailles PyInstaller](https://www.pythonguis.com/faq/packaged-installer-file-sizes/)

---

## 2. Communication PC ↔ UI ↔ mobile

### Transport et encodage

| Option | Version | Taille (bench WS, « big » payload) | Décodage navigateur | Évolution de schéma | Verdict |
|---|---|---|---|---|---|
| JSON texte | — | 329 o | natif, très rapide | libre | Garder comme **mode debug** (`?format=json`) |
| **MessagePack** | `rmp-serde` 1.3.1 / `msgpackr` 2.1.3 | 277 o (-16 %) ; bien plus si on envoie des tableaux typés | `msgpackr` très rapide, gère les tableaux typés (extension) | libre (self-describing) | **Recommandé** |
| Protobuf | `prost` 0.14.4 / protobuf-es | 229 o (-30 %) | rapide | codegen, champs numérotés | Bon, mais codegen double (Rust+TS) pour un gain marginal |
| FlatBuffers | 25.12 / 25.9 | ~ proto | zéro-copie | schéma rigide | Sur-dimensionné pour quelques Ko à 60 Hz |
| Binaire maison | — | 220 o | le plus rapide | fragile | Non (maintenance) |

Dimensionnement : une frame « dashboard » (voiture joueur + classement relatif de 20 voitures) ≈ 2-6 Ko MessagePack ; à 60 Hz ≈ 120-360 Ko/s, négligeable en LAN/Wi-Fi. Astuce qui donne la compacité d'un binaire maison sans en payer le prix : **dictionnaire de canaux** envoyé une fois à la connexion (`{id, name, unit, dtype}`), puis frames `[timestamp, Float32Array]`. Les widgets s'abonnent par groupes (`telemetry@60`, `scoring@5`, `weather@1`) pour que le mobile ne reçoive que l'utile.

Serveur : **axum 0.8.9** (`axum::extract::ws`, s'appuie sur `tokio-tungstenite` 0.30) ; un `tokio::sync::broadcast` par groupe, un writer par client avec coalescence (si le client est lent, on saute des frames, jamais on ne bufferise). Même protocole consommé par la webview desktop (via Channel Tauri) et par le client mobile (via `tokio-tungstenite` côté Rust).

### Découverte sur le LAN

| Plateforme | Mécanisme | Contraintes |
|---|---|---|
| PC (Windows/Linux) | **`mdns-sd` 0.21.6** (pur Rust, pas de runtime async imposé, IPv4/IPv6, Win/Linux/macOS) : annonce `_simtelem._tcp.local` avec TXT `{ver, name, pair}` | Auto-déclaré « beta » ; pas de réponses unicast (§5.4). Suffisant pour un LAN domestique. Ouvrir UDP 5353 dans le pare-feu Windows à l'installation |
| Android | `mdns-sd` dans le cœur Rust **+ `WifiManager.MulticastLock`** acquis par un mini-plugin Kotlin (sinon les paquets multicast sont filtrés), ou `NsdManager` | Plugin Tauri de ~50 lignes |
| iOS | **Pas de multicast brut** : l'entitlement `com.apple.developer.networking.multicast` est restreint (formulaire Apple). Utiliser Bonjour via **`NWBrowser`** dans un plugin Swift ; déclarer `NSBonjourServices` (`_simtelem._tcp`) et `NSLocalNetworkUsageDescription` ; déclencher la recherche sur action utilisateur (prompt « réseau local ») | Plugin Tauri Swift (~80 lignes). Sans ces clés, la découverte échoue silencieusement |
| Repli universel | **QR code** affiché par le PC (`ws://ip:port/?pair=…`) + saisie manuelle de l'IP | À implémenter dès la v1, c'est aussi le chemin le plus robuste sur réseaux invités/isolés |

### Sécurité minimale
- **Appairage** : code à 6 chiffres affiché sur le PC (TTL 2 min) → échange d'un jeton long (32 o, stocké côté mobile dans le keystore/keychain via `keyring` 4.2 ou `tauri-plugin-stronghold` 2.4) ; toutes les connexions WS suivantes présentent ce jeton dans le premier message.
- Écoute **uniquement** sur les interfaces LAN (jamais 0.0.0.0 exposé via UPnP) ; vérification de l'en-tête `Origin` ; limite de clients ; lecture seule par défaut (les commandes « changer de layout », « démarrer l'enregistrement » exigent un jeton « contrôleur »).
- TLS optionnel plus tard (certificat auto-signé épinglé lors de l'appairage) — prérequis si une synchro cloud arrive.

**Risques** : Wi-Fi invité avec isolation client (mDNS bloqué → QR/IP) ; iOS révoque la permission réseau local (message explicite + lien vers Réglages) ; pare-feu Windows (règle ajoutée par l'installeur NSIS).

Sources : [mdns-sd README](https://docs.rs/crate/mdns-sd/latest/source/README.md) · [WWDC20 Local Network Privacy](https://developer.apple.com/wwdc20/10110) · [Apple : multicast networking](https://developer.apple.com/news/?id=0oi77447) · [Forum Apple : quand l'entitlement multicast est requis](https://developer.apple.com/forums/thread/655920) · [Bench JSON/MsgPack/Protobuf/binaire sur WS](https://dev.to/nate10/performance-analysis-of-json-buffer-custom-binary-protocol-protobuf-and-messagepack-for-websockets-2apn) · versions : crates.io (`axum` 0.8.9, `tokio-tungstenite` 0.30.0, `rmp-serde` 1.3.1, `prost` 0.14.4, `flatbuffers` 25.12.19, `keyring` 4.2.0, `tauri-plugin-stronghold` 2.4.0), npm (`msgpackr` 2.1.3, `@msgpack/msgpack` 3.1.3, `flatbuffers` 25.9.23)

---

## 3. Grille de widgets et graphes

### Grille

| Lib | Version (date) | Licence | Maintenance | Tactile | Points clés |
|---|---|---|---|---|---|
| **gridstack.js** | **14.0.0** (21 sept. 2026) | MIT | Très active, cadence majeure annuelle | **Natif depuis v6** ; `alwaysShowResizeHandle: 'mobile'` | Zéro dépendance, wrappers React/Vue/Angular officiels, `save()/load()` JSON, grilles imbriquées, colonnes responsives (`columnOpts`), drag-in depuis une palette |
| react-grid-layout | 2.3.0 (5 oct. 2026) | MIT | Réécriture TS 2.0 (déc. 2025), hooks ; série 2.1-2.2 riche en correctifs (boucles infinies) | Partiel : drop-from-outside tactile seulement depuis 2.3 ; `allowMobileScroll` | Le standard React (Grafana) ; exige React 18+, enfants mémoïsés |
| dockview | 8.8.3 (30 oct. 2026) | MIT (cœur) + fonctions « Enterprise » (ex. navigation clavier) | Active | Non documenté | Docking IDE (onglets, flottant, popout) : excellent pour l'**espace d'analyse**, pas pour un dashboard libre |
| @dnd-kit | core 6.3.1 (déc. 2024) ; `@dnd-kit/react` 0.5.3 (oct. 2026, pré-1.0) | MIT | Nouvelle génération en cours | Oui | Primitive DnD, pas une grille : tout le placement/compaction serait à écrire |
| Muuri | 0.9.5 (**juil. 2021**) | MIT | **Abandonnée** | Oui | À écarter |

**Recommandation : gridstack.js 14** via son wrapper React, un seul composant `WidgetGrid` partagé desktop/mobile ; layouts stockés en JSON (`{id, type, x, y, w, h, props}`) dans SQLite, avec breakpoints (12 colonnes desktop, 4 colonnes téléphone) et layouts par contexte (voiture/circuit/session), comme RaceLab. **Plan B** : react-grid-layout 2.x si le wrapper React de gridstack pose problème (API très proche, migration mécanique). **dockview** en option pour l'écran d'analyse post-course (panneaux comparables, popout sur second écran).

### Graphes haute fréquence

| Lib | Version | Licence | Taille | Streaming 60 Hz | Remarques |
|---|---|---|---|---|---|
| **uPlot** | 1.6.32 (mars 2025) | MIT | ~45 Ko | Le plus léger CPU/mémoire ; auteur actif (Grafana) | Axe X numérique libre (distance ou temps) : indispensable pour superposer des tours par distance ; pas de légende riche intégrée |
| ECharts | 6.1.0 (mai 2026) | Apache-2.0 | ~1 Mo | OK mais plus lourd ; retours de dégradation sur longues sessions (source vendeur concurrent, à prendre avec recul) | Riche (légendes, heatmaps) : utile pour les graphiques statistiques statiques |
| Lightweight Charts | 5.2.1 (août 2026) | Apache-2.0 | ~45 Ko | Excellent | Orienté finance : axe horizontal temporel par défaut, personnalisable mais détourné |
| Canvas maison | — | — | — | Total contrôle | Justifié uniquement pour la **carte de circuit** et les jauges |

**Recommandation** : **uPlot** pour toutes les traces (inputs live, comparaison de tours, deltas), alimenté par des ring buffers `Float32Array` et un seul `requestAnimationFrame` global qui pousse les données à tous les widgets (pas un timer par widget). ECharts réservé aux pages d'analyse (distribution, heatmaps pneus). **Carte de circuit** : Canvas 2D maison — tracé construit en apprenant la ligne centrale à partir des positions XYZ du premier tour complet (ré-échantillonnage par `lapDist`), stocké par circuit dans SQLite, puis voitures dessinées par `lapDist`/position à chaque frame ; pixi.js 8 (WebGL) seulement si >60 voitures + effets.

**Risques** : WebKitGTK sans accélération → prévoir un mode « basse fréquence » (30 Hz) ; mobile : limiter le nombre de séries visibles ; uPlot a une cadence de releases lente (stable, pas abandonné).

Sources : [CHANGELOG react-grid-layout](https://raw.githubusercontent.com/STRML/react-grid-layout/master//CHANGELOG.md) · [README gridstack](https://github.com/gridstack/gridstack.js/blob/master/README.md) · [dockview intro](https://dockview.dev/docs/overview/introduction) · [Comparatif live charts Node-RED (uPlot vs ECharts, CPU)](https://discourse.nodered.org/t/fast-live-time-line-charts-comparison/61342) · [LightningChart vs ECharts 2026 (biaisé vendeur)](https://lightningchart.com/blog/best-apache-echarts-alternative-in-2026/) · [uPlot npm](https://npmjs.com/package/uplot) · versions npm interrogées le 2 nov. 2026

---

## 4. Stockage des sessions

### Comparatif

| Option | Version | Écritures 50-100 Hz | Analyse | Taille | Export | Complexité |
|---|---|---|---|---|---|---|
| **SQLite** (`rusqlite` bundled) | 0.41.1 (28 oct. 2026) ; `sqlx` 0.9.0 ; `tauri-plugin-sql` 2.6 | Excellentes en transactions groupées (1 commit/s) ; 1 écrivain | Correcte pour métadonnées, faible pour balayages de millions de lignes | Ligne par échantillon ≈ brut ×1,5 | CSV facile | Minimale, 1 fichier, ACID |
| **Parquet** (crate `parquet`/`arrow` 60.1) | 60.1.0 (28 oct. 2026) | Écriture par row-groups (buffer 1-5 s) ; fichier par session | Colonnaire, compression delta+zstd, lecture vectorisée | **la plus petite** | Lisible par DuckDB, pandas, Polars, MoTeC via conversion | Faible (schéma Arrow) |
| DuckDB embarqué | crate `duckdb` 1.20000.1 = DuckDB 2.0 LTS, `bundled` compile depuis les sources | Appender OK mais « lent pour les petites écritures » | Le meilleur SQL analytique | Colonnaire | Natif Parquet/CSV | Temps de compilation et binaire lourds ; 1 process |
| Binaire maison | — | Trivial | Tout à écrire | Petite | Tout à écrire | Dette certaine |
| InfluxDB embarqué | — | — | — | — | — | Pas de version embarquable sérieuse en Rust/Tauri ; hors sujet local-first |

### Volumétrie : course de 6 h

Hypothèses : échantillon = `t: f64` + N canaux `f32` ; 100 Hz ⇒ 2 160 000 échantillons (à 50 Hz, diviser par 2 — rappel : le plugin rF2 produit la télémétrie à 50 Hz). Ratios Parquet (delta + zstd) **estimés 4-8×** sur signaux lisses, à confirmer par benchmark sur données réelles.

| Canaux | Octets/éch. | Brut | SQLite ligne/éch. (≈×1,5) | Parquet zstd (÷4 … ÷8) |
|---|---|---|---|---|
| 40 (dashboard) | 168 | 363 Mo | ~540 Mo | 45-90 Mo |
| 80 (analyse standard) | 328 | 708 Mo | ~1,06 Go | 90-180 Mo |
| 150 (4 roues détaillées, suspension, aéro) | 608 | 1,31 Go | ~2 Go | 165-330 Mo |

Conclusion : stocker la **structure brute du plugin** (~500 Ko/buffer) est exclu ; on normalise en canaux. Une saison de 50 sessions de 1-2 h tient en quelques Go en Parquet.

### Recommandation : **SQLite + Parquet** (« métadonnées relationnelles, échantillons colonnaires »)

```
~/SimTelemetry/
  library.sqlite                   # sessions, tours, secteurs, événements, setups, profil, layouts
  sessions/2026/11/<session_id>/
    samples.parquet                # 1 row-group par ~5 s, zstd, colonnes = canaux
    events.parquet (optionnel)     # si > 100k événements
    meta.json                      # copie dénormalisée pour résilience / synchro future
```

Schéma SQLite (extraits) :
- `session(id ULID, game, game_version, track_id, layout, car_id, class, session_type, started_at, ended_at, weather_summary, sample_rate_hz, samples_path, schema_version)`
- `lap(id, session_id, number, start_t, end_t, time_ms, valid, invalid_reason, fuel_start, fuel_end, tyre_compound, pit_in/out, best_flag)` + `sector(lap_id, idx, time_ms)`
- `channel(session_id, name, unit, dtype, min, max, mean)` (catalogue par session : permet d'ajouter des canaux par jeu sans migration)
- `event(id, session_id, t, lap_id, kind ENUM[pit_entry, pit_exit, flag, incident, offtrack, overtake, setup_change, note], payload JSON)`
- `setup(id, session_id?, car_id, track_id, created_at, source ENUM[game_rest, user, recommender], values JSON, parent_id)` — versionné, lien vers la recommandation qui l'a produit
- `driver_profile(id, name, created_at)` ; `session.driver_id` ; vues matérialisées `profile_track_car_stats`
- `recommendation(id, session_id, created_at, inputs JSON, rules_fired JSON, llm_interpretation JSON, proposed_changes JSON, accepted BOOL)`

Échantillons Parquet : colonne `t` (f64 s depuis début de session), `lap`, `lap_dist`, puis canaux normalisés en **SI** (m/s, K ou °C fixé une fois, bar/kPa, rad) ; le mapping jeu→canonique vit dans l'adaptateur. Lecture d'analyse en Rust via `arrow` (ou Polars 0.55 si besoin de dataframes). **DuckDB** devient un ajout optionnel (feature flag) le jour où l'on veut du SQL ad hoc multi-sessions — ses fichiers d'entrée (Parquet) sont déjà là.

### Export
- **CSV** : écriture directe depuis Arrow (choix des canaux, décimation).
- **MoTeC i2** : format `.ld`/`.ldx` propriétaire mais rétro-ingéniéré ; le crate **`motec-i2` 0.2.0 (MIT, juil. 2024)** lit/écrit `.ld` et `.ldx` ; le projet Python `sim-to-motec` (AMS2/GT7) sert de référence pour les beacons de tour. Prévoir une fixture testée dans MoTeC i2 Pro (le jeu lui-même émet des `.ld`, utiles comme oracle).

**Risques** : évolution du schéma (versionner `schema_version` + migrations `rusqlite_migration`) ; corruption en cas de crash en course (row-groups Parquet déjà flushés restent lisibles ; `meta.json` écrit en continu) ; SSD saturé (quota + rotation configurable).

Sources : [DuckDB Rust client 2.0](https://duckdb.org/docs/2.0/clients/rust/overview.html) · [DuckDB vs SQLite (DataCamp)](https://www.datacamp.com/blog/duckdb-vs-sqlite-complete-database-comparison) · [motec-i2 (rust-digger)](https://rust-digger.code-maven.com/crates/motec-i2) · [afonso360/motec-i2](https://github.com/afonso360/motec-i2) · [sim-to-motec](https://github.com/GeekyDeaks/sim-to-motec) · cadences plugin : [rF2SharedMemoryMapPlugin](https://github.com/TheIronWolfModding/rF2SharedMemoryMapPlugin) · versions crates.io (`rusqlite` 0.41.1, `sqlx` 0.9.0, `duckdb` 1.20000.1, `parquet`/`arrow` 60.1.0, `polars` 0.55.2, `zstd` 0.14.3, `tauri-plugin-sql` 2.6.0)

---

## 5. Analyse et recommandation de setup

### Où exécuter l'analyse ?

| Option | Avantages | Inconvénients | Verdict |
|---|---|---|---|
| **Rust dans le cœur** (crate `analysis`) | Même code en live (diagnostics en cours de session), en batch (post-course) et **en WASM dans l'UI** (mobile hors connexion au PC) ; typé, testé, rapide ; aucune dépendance de packaging | Moins de bibliothèques « notebook » que Python ; écrire soi-même les stats (percentiles, régressions simples, filtres) — volume modeste | **Recommandé** |
| TypeScript côté UI | Itération rapide | Dupliquerait la logique pour l'enregistreur/serveur, perf limitée sur mobile | Seulement pour l'agrégation d'affichage |
| Sidecar Python (numpy/pandas) | Écosystème | +100-300 Mo par OS (PyInstaller), signature/notarisation, pas de mobile, démarrage lent, surface de bugs | Non en production ; **oui en R&D** (notebooks qui lisent nos Parquet pour prototyper les règles) |

### Moteur de règles

| Option | Version | Licence | Format | Éditeur | Remarques |
|---|---|---|---|---|---|
| **zen-engine (GoRules)** | crate 2.1.6 / npm 2.1.9 (oct. 2026) | MIT | **JDM** (JSON) : tables de décision avec politique de hit `first`/`collect`, nœuds Switch/Expression/Function (QuickJS) | `@gorules/jdm-editor` 1.52 (React, MIT) | Cœur Rust, bindings Node/Python/Go ; les tables « symptôme × phase de virage × contexte → ajustement » s'y expriment naturellement ; contributions externes non acceptées (vendor-driven) |
| json-rules-engine | 7.3.1 (fév. 2025) | ISC | JSON conditions/événements | — | JS uniquement → imposerait l'analyse côté UI |
| Rhai | 1.27.0 (oct. 2026) | MIT/Apache | script | — | Bon complément pour des règles « expertes » scriptées par les utilisateurs avancés |
| Format maison (TOML/JSON évalué en Rust) | — | — | libre | — | Simple, mais réinvente tables + hit policies + traçabilité |

**Recommandation** : `zen-engine` en Rust, décisions JDM versionnées dans le dépôt (`rules/<game>/<class>/*.json`), **ne pas utiliser les nœuds Function (QuickJS)** et vérifier si le crate permet de les exclure à la compilation pour réduire la surface et la taille du binaire ; chaque règle produit `{change, direction, magnitude, confidence, rationale, evidence[]}`. Les entrées sont des **indicateurs** calculés par `analysis` (sur/sous-virage par phase via ratio vitesse de lacet / angle de volant, écart de températures I/M/O, blocage, patinage, utilisation de la course de suspension, delta par secteur) plus le **ressenti** structuré.

### LLM pour le ressenti en texte libre (API Claude)

- Rôle strict : traduire « ça pousse à l'entrée des rapides et ça décroche à la réaccélération » en un objet **structuré** `{phase, corner_type, axis, severity, confidence}` ; la **recommandation reste produite par les règles** (déterministe, explicable, rejouable). Option : deuxième appel pour rédiger l'explication en français à partir des règles déclenchées.
- API : **sorties structurées** via `output_config.format = {type: "json_schema"}` — **GA sur tous les modèles**, plus de header bêta ; grammaire compilée mise en cache 24 h ; schéma sans contraintes numériques (`minimum`, `maxLength`… non supportés → les mettre en description et valider côté client avec `zod` 4 / `serde`). Alternative : outil `strict: true`.
- Modèle : **Claude Haiku 5.5** (`claude-haiku-5-5`, à partir de 0,10 $/MTok entrée, 0,50 $/MTok sortie) pour l'extraction ; Sonnet 5.5 (2 $/10 $) si l'on veut une explication plus fine. Une interprétation ≈ 1-2 k tokens → **< 0,002 $**. Lecture de cache à 10 % du prix d'entrée pour le prompt système.
- Intégration : client HTTP depuis le cœur Rust (`reqwest`) ou SDK TS `@anthropic-ai/sdk` 1.3 côté UI — préférer le **cœur Rust** (une seule implémentation, clé stockée via `keyring`/Stronghold, jamais dans la webview). Clé API fournie par l'utilisateur en v1 (pas de serveur) ; un proxy avec quotas viendra avec le cloud.
- **Hors-ligne dégradé** : (1) formulaire de ressenti structuré (boutons sous/sur-virage × entrée/milieu/sortie × lent/rapide) qui alimente directement les règles — c'est aussi le chemin le moins ambigu ; (2) cache local des interprétations ; (3) option « modèle local » (Ollama/llama.cpp) derrière le même trait `FeelInterpreter`, sans promesse de qualité. Toujours afficher la provenance (règle / LLM) et le niveau de confiance.

**Risques** : hallucination de causes → sortie contrainte au vocabulaire du schéma (`enum`), jamais de valeur numérique produite par le LLM ; dépendance réseau → tout fonctionne sans ; règles « généralistes » fausses pour une voiture → règles par classe (Hypercar/LMP2/LMGT3) avec bornes de setup lues via l'API REST garage.

Sources : [Structured outputs (plateforme Claude)](https://platform.claude.com/docs/en/build-with-claude/structured-outputs) · [Modèles et tarifs Claude](https://platform.claude.com/docs/en/about-claude/models/overview) · [zen-engine / JDM (description)](https://pypi.org/project/zen-engine/0.40.1/) · versions npm/crates (`@gorules/zen-engine` 2.1.9, `@gorules/jdm-editor` 1.52.0, `zen-engine` 2.1.6, `json-rules-engine` 7.3.1, `rhai` 1.27.0, `@anthropic-ai/sdk` 1.3.0, `zod` 4.6.7) · [Tailles PyInstaller](https://www.pythonguis.com/faq/packaged-installer-file-sizes/)

---

## 6. Monorepo, outillage, CI, adaptateurs de jeu

### Structure

```
sim-telemetry/
├─ Cargo.toml                 # workspace Rust
├─ crates/
│  ├─ telemetry-types/        # Frame canonique, Channel, unités, enums (+ specta → TS)
│  ├─ game-adapters/          # trait GameAdapter + crates par jeu (features)
│  │   ├─ rf2-smmp/           # $rFactor2SMMP_*$ (rF2 + LMU via plugin)
│  │   ├─ lmu-native/         # LMU_Data + REST 6397
│  │   ├─ ac-shm/  acc-shm/   # acpmf_* (plus tard)
│  │   ├─ iracing/            # irsdk mmap + .ibt (Windows only)
│  │   └─ ams2-shm/           # $pcars2$
│  ├─ shm/                    # ouverture mappings : windows (OpenFileMapping) | linux (/dev/shm)
│  ├─ recorder/               # Parquet writer, SQLite (rusqlite + migrations)
│  ├─ analysis/               # indicateurs, stats, comparaisons (no_std-friendly, cible wasm32)
│  ├─ rules/                  # zen-engine + décisions JDM embarquées
│  ├─ server/                 # axum : WS, REST, statiques, mDNS, appairage
│  └─ sim-core/               # binaire headless (CLI) = desktop daemon
├─ apps/
│  ├─ tauri/                  # un seul projet Tauri : desktop + android + ios (cfg(mobile) = mode client)
│  └─ web/                    # UI React (Vite) servie par Tauri ET par sim-core (OBS/navigateur)
├─ packages/
│  ├─ widgets/                # composants widgets (gridstack, uPlot, canvas) — partagés
│  ├─ protocol/               # types TS générés (tauri-specta 1.0 / specta 1.0), client WS, décodeur msgpackr
│  └─ ui-kit/                 # thème, primitives
├─ rules/                     # JDM par jeu/classe (données, pas code)
├─ fixtures/                  # dumps mémoire partagée anonymisés (.bin) + sessions Parquet de test
├─ pnpm-workspace.yaml, turbo.json, biome.json
└─ .github/workflows/
```

### Outillage (versions vérifiées)
- **JS** : pnpm 12.16, Turborepo 2.11 (cache des tâches), Vite 8.3, **TypeScript 7.0** (compilateur natif), React 19.3, Biome 2.6 (lint+format, remplace ESLint/Prettier ; oxlint 1.90 en alternative), Vitest 5.0, Playwright 1.64 (E2E de la web app ; les widgets se testent dans un navigateur contre un `sim-core --replay fixtures/…`).
- **Rust** : stable ≥ 1.90 (MSRV Tauri 2.12), clippy `-D warnings`, rustfmt, cargo-nextest, cargo-deny (licences), `insta` pour les snapshots des parseurs, `criterion` pour le décodage de frames.
- **Typage bout en bout** : `specta`/`tauri-specta` 1.0 génèrent les types et les commandes TS depuis Rust ; le protocole WS est défini **une fois** en Rust.
- **Replay** : `sim-core --replay session.parquet --speed 1.0` rejoue une session comme si le jeu tournait : c'est l'outil n°1 pour développer sans PC de jeu et pour les agents IA (tests déterministes).

### CI GitHub Actions
| Job | Runner | Contenu |
|---|---|---|
| `check` | ubuntu | biome, tsc, vitest, cargo fmt/clippy/nextest, cargo-deny |
| `desktop` | windows-latest, ubuntu-22.04 (+ `ubuntu-22.04-arm`) | `tauri-action@v1` → NSIS+MSI, deb/rpm/AppImage (+ Flatpak manifest), artefacts updater signés (`TAURI_SIGNING_PRIVATE_KEY`) ; deps Linux `libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf xdg-utils` |
| `android` | ubuntu-latest | JDK 17, SDK/NDK (r26d), 4 cibles Rust, `tauri android build`, keystore base64 en secret |
| `ios` | macos-latest | cible `aarch64-apple-ios`, Xcode stable, clé API App Store Connect, `tauri ios build --export-method app-store-connect` |
| `e2e` | ubuntu | Playwright contre `sim-core --replay` |
| `release` | — | tags `app-v*`, draft GitHub Release, `latest.json` pour l'updater |

Le workflow mobile n'est pas couvert par la doc officielle `tauri-action` (desktop seulement) ; le modèle de référence est le workflow CrabNebula Cloud (desktop + Android + iOS). Prévoir un compte Apple Developer (99 $/an) et Google Play (25 $) dès le spike mobile.

### Abstraction « adaptateur de jeu »

```rust
pub trait GameAdapter: Send {
    fn id(&self) -> GameId;
    fn probe(&self) -> Probe;                 // process détecté ? mapping ouvrable ? version struct ?
    fn open(&mut self) -> Result<()>;
    fn poll(&mut self, out: &mut FrameSet) -> Result<Tick>;  // lecture seqlock, 0-copy vers FrameSet
    fn capabilities(&self) -> Capabilities;   // canaux dispo, cadences, scoring, météo, setup RW
    fn aux(&self) -> Option<&dyn AuxApi>;     // REST LMU, commandes iRacing…
}
```

- **Frame canonique** (`telemetry-types`) : identifiants de canaux stables (`speed_mps`, `tyre_temp_fl_core_k`…), unités SI, `lap_dist_m`, métadonnées session ; chaque adaptateur fournit une **table de mapping** + conversions, et un bloc `raw_ext` (MessagePack) pour les canaux spécifiques, exposé tel quel aux widgets avancés.
- **Lecture** : `shm` cache les différences OS — Windows `OpenFileMappingW` + `MapViewOfFile` (crate `windows`), Linux `memmap2` sur `/dev/shm/<nom>` (noms issus de winecarte / plugin Wine) ; lecture **seqlock** (relire si `version_begin != version_end`), copie dans un buffer possédé par l'adaptateur (les mappings ~500 Ko ne sont jamais relus par la UI).
- **Horloge** : chaque adaptateur fournit un `source_time` (temps jeu) + `host_time` monotone ; l'enregistreur ré-échantillonne à la cadence native (50 Hz rF2/LMU, 60 Hz iRacing, ACC jusqu'à 333 Hz physique) sans interpoler.
- **Feuille de route jeux** : LMU natif + rF2 SMMP (v1) → AC/ACC (`acpmf_physics/graphics/static`, Proton OK) → AMS2 (`$pcars2$` + UDP) → iRacing (`Local\IRSDKMemMapFileName` + `.ibt`, **Windows uniquement**, pas de Linux) → AC Evo. Chaque adaptateur livre ses fixtures (dumps binaires) et ses tests de layout `#[repr(C)]` (`size_of`, offsets) : c'est ce qui casse à chaque mise à jour de jeu.

**Risques** : changement de struct par une mise à jour LMU (tests d'offsets + `schema_version` du plugin + détection de taille de mapping) ; iRacing SDK derrière login (fixtures locales, pas en CI publique) ; temps de build Rust en CI (cache `Swatinem/rust-cache`, `sccache`).

Sources : [Pipeline GitHub officielle Tauri](https://v2.tauri.app/distribute/pipelines/github/) · [Workflow CrabNebula desktop + Android + iOS](https://docs.crabnebula.dev/cloud/ci/tauri-v2-workflow/) · [tauri-action](https://github.com/tauri-apps/tauri-action) · [winecarte (noms de mappings par jeu)](https://github.com/srounce/winecarte) · [kerb (exemple de design d'adaptateurs en Rust)](https://docs.rs/crate/kerb/latest) · versions npm/crates du 2 nov. 2026 (`pnpm` 12.16.1, `turbo` 2.11.10, `vite` 8.3.5, `typescript` 7.0.2, `@biomejs/biome` 2.6.4, `oxlint` 1.90.0, `vitest` 5.0.5, `playwright` 1.64.1, `specta` 1.0.5, `tauri-specta` 1.0.2, `ts-rs` 12.0.1)

---

## 7. Paysage concurrentiel (positionnement)

| Produit | Couvre LMU ? | Plateformes | Mobile | Analyse post-course | Setup assisté (IA ?) | Prix | Licence |
|---|---|---|---|---|---|---|---|
| **SimHub** | Oui (80+ jeux) | Windows | Dashboards via navigateur/tablette | Faible | Non | Gratuit ; licence « prix libre » 8-30 € (sorties matériel 60 fps au lieu de 10, auto-switch) | Propriétaire |
| **RaceLab** | Oui (iRacing, AC, ACC, rF2, LMU, AMS2, F1, RaceRoom) | Windows (.exe) | Non | Non | Non | Basic gratuit ; Pro 49,90 €/an ; Commercial 299 €/siège/an | Propriétaire |
| **TinyPedal** | Oui (rF2 + LMU) | **Windows + Linux** | Non | Non | Non | Gratuit | **GPL-3.0** (ne pas copier de code sans accepter la GPL) |
| **Garage 61** | **Non** (iRacing uniquement) | Web | Web | Oui (navigateur, CSV, ghost laps, live timing) | Non | Gratuit ; Pro 5 €/mois (annuel) | SaaS |
| **Coach Dave Delta** | Oui (iRacing, ACC, LMU, AC Evo, AC, AMS2, GT7) | Windows (enregistrement) + cloud | — | Oui (comparaison ligne/freinage vs référence, « Auto Insights » IA) | **Setup IQ** (LMU/ACC) : analyse tes tours, diagnostique l'équilibre, « Auto-Tune » un setup pro ; pas de ressenti en texte libre documenté | 11,99 £/mois ou 109 £/an, pas d'essai | SaaS |
| **Trophi.ai** | Oui (iRacing, ACC, F1 23-25, LMU) | PC | — | Rapports tour/stint | Coaching IA temps réel (voix, points de freinage) ; setups = packs Grid & Go, pas générés | Premium mensuel/annuel (prix non affichés sans compte ; -50 % annuel), Team, Professional (coach humain) ; levée 3,3 M$ | SaaS |
| **Track Titan** | Oui (plugin requis) + iRacing, AMS2, AC, ACC, Forza, F1, AC Evo bêta | PC | App mobile (pas pour F1 24/25) | Oui, coaching IA | Non (coaching pilotage) ; levée 5 M$ | Abonnement | SaaS |
| **GO Fast (GO Setups)** | Oui (Race Engineer, Setup Optimiser, télémétrie, auto-installeur) | Windows (Microsoft Store) | — | Oui | **Setup Optimiser** (LMU/ACC/iRacing) | Abonnement (Store) | Propriétaire |
| **Neuralic Race Engineer** | Oui (Hypercar/LMDh/LMP2/LMGT3) + ACC | Windows (Steam) | — | — | **Oui** : setup complet calculé à partir de la télémétrie (hauteurs, aéro, diff, carrossage, pression), corrections tour par tour | 14,99 $ (achat unique) | Propriétaire |
| **Popometer** | Non (ACC) | Web | — | Oui (vs données Coach Dave) | Non | 5 €/mois / 50 €/an | SaaS |

**Lecture** : (1) le temps réel « overlay » est un marché saturé et gratuit (SimHub, RaceLab Basic, TinyPedal) ; (2) l'analyse et le setup assisté sont **payants, cloud et Windows-only** ; (3) personne ne combine **local-first + Linux natif + mobile avec les mêmes widgets + recommandation de setup expliquée à partir du ressenti** ; (4) les acteurs IA (Delta Setup IQ, Neuralic, GO Setups) partent de la télémétrie seule — notre différenciateur est la boucle *télémétrie × ressenti pilote × règles explicables*, hors ligne, avec le LLM en simple interprète. Modèle de prix cohérent avec le marché : cœur gratuit/open (adaptateurs, dashboard, enregistrement) + fonctions premium (recommandation, historique multi-saisons, synchro future) autour de 5-10 €/mois ou achat unique à la Neuralic.

Sources : [Delta vs Garage 61 (fév. 2026)](https://coachdaveacademy.com/tutorials/coach-dave-delta-vs-garage-61-which-is-best-for-you/) · [Setup IQ](https://coachdaveacademy.com/delta/setup-iq/) · [Trophi pricing](https://www.trophi.ai/pricing) · [Track Titan — jeux supportés](https://intercom.help/track-titan/en/articles/6227990-what-games-are-supported-by-track-titan) · [RaceLab](https://racelab.app/) · [SimHub — licence](https://www.simhubdash.com/get-a-license/) · [GO Fast — sims supportés](https://support.gosetups.gg/hc/en-gb/articles/23224129387154-What-Sims-Are-Supported-in-GO-Fast) · [Neuralic Race Engineer](https://gg.deals/application/neuralic-race-engineer/) · [Popometer × Coach Dave](https://coachdaveacademy.com/announcements/everything-you-need-to-know-popometer-cda-subscription/) · [TinyPedal (GPL-3.0, Win+Linux)](https://github.com/reloadggg/TinyPedal_CN) · [Second Monitor — support LMU via SMMP](https://gitlab.com/winzarten/SecondMonitor/-/merge_requests/201)

---

## Annexe — Ordre des spikes recommandé (2-3 semaines)

1. **Lecteur LMU** (Windows puis Linux/winecarte) : ouvrir `LMU_Data` et `$rFactor2SMMP_Telemetry$`, valider offsets contre `pyLMUSharedMemory`, dump de fixtures. Critère : 50 Hz stables, 0 lecture déchirée.
2. **sim-core + UI replay** : serveur axum + msgpackr + 3 widgets gridstack/uPlot à 60 Hz dans Chrome et dans Tauri (Windows, Linux NVIDIA/Wayland).
3. **Spike mobile Tauri** : build Android signé + iOS TestFlight, client WS en Rust → Channel → mêmes widgets ; Bonjour via `NWBrowser` ; QR d'appairage. **Décision Tauri vs plan B à l'issue de ce spike.**
4. **Enregistreur Parquet + SQLite** sur une session de 2 h, mesure réelle du ratio de compression, export CSV + `.ld` ouvert dans MoTeC i2.
5. **Première décision JDM** (sous-virage entrée lente → ARB avant / différentiel) + extraction de ressenti Haiku 5.5 en sortie structurée, avec mode hors-ligne par formulaire.
