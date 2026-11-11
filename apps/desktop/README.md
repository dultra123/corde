# Corde — application de bureau (coquille Tauri 2)

Une icône, une fenêtre, pas de console : la coquille démarre le **serveur Corde embarqué** (le même code que
`corde-server.exe`), affiche dans une fenêtre native l'UI web qu'il sert sur la boucle locale, reste dans la
zone de notification quand on ferme la fenêtre et la ressort quand le pilote entre en piste, sans prendre le premier plan au jeu.

Guide d'installation pour le pilote : [`docs/TESTING-ON-PC.md`](../../docs/TESTING-ON-PC.md).
Pare-feu, SmartScreen et signature : [ADR 0007](../../docs/adr/0007-windows-smartscreen-signature.md).

## Structure

```
apps/desktop/
├─ README.md
├─ assets/
│  ├─ corde-icon.svg             # logo source : trajectoire en « C » et point de corde, ambre #ffb454 sur fond sombre
│  └─ corde-icon-1024.png        # rendu 1024 px (entrée de `tauri icon`)
├─ scripts/make-icons.mjs        # régénère assets/*.png et src-tauri/icons/* (+ pack-icons.py : .ico, .icns)
├─ dist/                         # frontendDist : page locale « Démarrage de Corde… » / erreur
│  ├─ index.html                 #   (écrite à la main, pas de build, embarquée dans l'exe)
│  ├─ splash.css
│  └─ splash.js
└─ src-tauri/                    # crate `corde-desktop` — workspace Cargo À PART (voir plus bas)
   ├─ Cargo.toml, Cargo.lock
   ├─ build.rs                   # tauri-build + liste des commandes IPC (permissions générées)
   ├─ tauri.conf.json            # fenêtre, CSP, bundle NSIS, ressources (ui/)
   ├─ capabilities/
   │  ├─ local-pages.json        # page locale : boot_status, restart_app, app_version
   │  └─ embedded-ui.json        # UI servie par le serveur (http://127.0.0.1:*, http://localhost:*) : 4 commandes
   ├─ windows/hooks.nsh          # installeur : programme dans %LOCALAPPDATA%\Programs\Corde
   ├─ icons/                     # générées (png, ico, icns, logos Store)
   └─ src/
      ├─ main.rs                 # point d'entrée : journal, plugins, fenêtre, démarrage du serveur
      ├─ logging.rs              # <données>/logs/corde-desktop.log, rotation, paniques
      ├─ migration.rs            # reprise des dossiers de l'ancien nom (données, profil WebView)
      ├─ settings.rs             # lecture de settings.json (lan_access) et de desktop.json
      ├─ server.rs               # serveur embarqué : thread + runtime tokio, ports, arrêt propre
      ├─ shell.rs                # état, fenêtre principale, démarrage, quitter, redémarrer
      ├─ tray.rs                 # icône et menu de la zone de notification
      ├─ watcher.rs, edges.rs    # veilleur 1 Hz sur /api/status, détection des fronts
      └─ commands.rs             # commandes IPC (contrat avec l'UI)
```

Versions retenues (novembre 2026, Tauri **2.x stable**, pas la 3.0 alpha) : `tauri` 2.12.5, `tauri-build`
2.7.4, `tauri-plugin-single-instance` 2.5.5, `tauri-plugin-autostart` 2.7.0, `tauri-plugin-window-state`
2.5.0, `tauri-plugin-notification` 2.8.1, CLI `@tauri-apps/cli` 2.12.4 (épinglée dans la CI). Rust ≥ 1.90.

## Comportement

| Moment | Ce que fait la coquille |
|---|---|
| **Lancement** | journal ouvert ; second lancement → réveille la première instance et s'arrête (`tauri-plugin-single-instance`). Sinon : icône de zone de notification, serveur embarqué lancé dans son thread, fenêtre `main` affichée sur la page locale « Démarrage de Corde… » |
| **Serveur prêt** | dès que `GET /api/health` répond (20 s max), la fenêtre navigue vers `http://127.0.0.1:<port>/#/home` |
| **Serveur en échec** | la page locale affiche l'erreur lisible, le chemin du journal et « Réessayer » ; fermer la fenêtre quitte alors l'application |
| **`--hidden`** (démarrage avec Windows) | pas de fenêtre ; l'UI n'est chargée qu'au premier affichage (pas de tableau de bord qui tourne dans une fenêtre invisible) |
| **Fermer la fenêtre** | elle est cachée, Corde continue (attente du jeu, enregistrement). Notification « Corde reste active… » la première fois, mémorisée dans `<données>/desktop.json` |
| **Zone de notification** | clic gauche / double-clic : ouvrir. Menu : « Ouvrir Corde », « Dashboard » (route `#/`), « Quitter ». Info-bulle : démarrage, en attente du jeu, jeu détecté, en piste, erreur |
| **Le pilote entre en piste** | front montant de `in_realtime` (veilleur, 1 Hz) → si la fenêtre est cachée ou réduite : `show` + `unminimize`, **sans `set_focus`** (le jeu garde le clavier et reste devant ; sur un seul écran la fenêtre réapparaît derrière lui) ; si elle est déjà à l'écran : rien. L'UI passe seule au tableau de bord |
| **Jeu détecté** (`connected`) | seule l'info-bulle change ; une application démarrée cachée reste cachée jusqu'à l'entrée en piste |
| **« Quitter »** | fenêtre cachée, puis arrêt propre du serveur (`Server::run` rejoint le thread source, qui clôt l'enregistrement), 10 s max, puis fin du processus |
| **`restart_app`** | même arrêt propre, puis relance de l'exécutable **sans** `--hidden` |

Le serveur embarqué a la configuration par défaut de `corde-server` (source `auto` = LMU, enregistrement à
50 Hz, import des résultats). Seules différences : `ui_dir` = ressource `ui/` de l'installeur, port 8471
avec repli sur 8472…8480, adresse d'écoute `127.0.0.1` sauf `lan_access`, et refus de démarrer si un autre
Corde répond déjà sur 8471.

## Contrat avec l'UI web

L'UI (`packages/ui`) est servie par le serveur embarqué : pour Tauri c'est une origine **distante**, qui n'a
accès qu'à ce que `capabilities/embedded-ui.json` lui donne. `withGlobalTauri` est activé :

```ts
const invoke = window.__TAURI__?.core?.invoke; // undefined dans un navigateur ordinaire
await invoke("get_autostart");                 // → boolean
await invoke("set_autostart", { enabled: true });
await invoke("restart_app");                   // l'application s'arrête et se relance
await invoke("app_version");                   // → "0.5.0"
```

| Commande | Arguments | Retour | Effet |
|---|---|---|---|
| `get_autostart` | — | `boolean` | Corde est-elle inscrite au démarrage de Windows ? |
| `set_autostart` | `{ enabled: boolean }` | — | inscrit / retire la clé `HKCU\…\Run\Corde` (`"…\Corde.exe" --hidden`) |
| `restart_app` | — | — | arrêt propre puis relance, fenêtre visible |
| `app_version` | — | `string` | version de `tauri.conf.json` |

Les erreurs rejettent la promesse avec un texte. Ajouter une commande = `src/commands.rs` +
`generate_handler!` (`main.rs`) + la liste `COMMANDS` de `build.rs` + la capacité concernée.

Réglages : la coquille **lit** `<données>/settings.json`, clé booléenne `lan_access` (défaut `false`), une
seule fois au démarrage, dans `settings::resolve_bind_ip` — c'est le **point d'intégration** à remplacer par
`corde_server::resolve_bind` après le merge de `main`. Elle n'écrit jamais dans ce fichier ; sa propre
mémoire est dans `desktop.json`. Dossier de données : `corde_store::default_data_dir()`
(`%LOCALAPPDATA%\Corde`), préparé au lancement par `corde_store::prepare_default_data_dir()` **avant**
l'ouverture du journal : le dossier de l'ancien nom (avant 0.3.0) est renommé, ou utilisé tel quel pour
cette exécution si le renommage échoue (ancienne application encore ouverte) ; `src/migration.rs` reprend
de même les dossiers de l'ancien identifiant Tauri (profil WebView2, état de la fenêtre). Règles et tests :
`crates/corde-store/src/migrate.rs`.

## Développer

Ce crate est **hors du workspace Cargo racine** (`exclude` dans le `Cargo.toml` racine, `[workspace]` vide
ici) : Tauri tire WebView2 / GTK-WebKit et ne se compile pas partout. `cargo check --workspace` à la racine
ne le voit pas ; **le job CI `desktop` (windows-latest) est son compilateur de référence** et produit
l'installeur (artefact `corde-setup`).

Sous Windows (Rust stable, Node 22, pnpm ; WebView2 est déjà là sur Windows 11) :

```powershell
pnpm install
cargo test -p corde-protocol          # types TS du protocole
pnpm --filter @corde/ui build         # packages/ui/dist : c'est la ressource ui/ (obligatoire pour compiler)
cd apps/desktop
$env:CORDE_SOURCE = "sim"             # facultatif : simulateur à la place du jeu
pnpm dlx "@tauri-apps/cli@2.12.4" dev     # lance l'application en debug (console visible, journal aussi sur stderr)
pnpm dlx "@tauri-apps/cli@2.12.4" build   # src-tauri/target/release/bundle/nsis/Corde_<version>_x64-setup.exe
```

Variables d'environnement utiles : `CORDE_SOURCE=sim|lmu|auto` (source de télémétrie), `CORDE_LOG=debug`
(filtre du journal, syntaxe `tracing`). Les outils de développement du WebView (F12) sont actifs en debug ;
pour un installeur qui les garde : `… build --features devtools`.

Sous Linux, le crate se compile si les paquets de développement GTK / WebKitGTK 4.1 sont installés
(prérequis Tauri) ; ce n'est pas le cas des conteneurs de développement du projet. Les modules qui ne
dépendent pas de Tauri (`server`, `settings`, `edges`, `logging`) ont des tests unitaires (`cargo test` dans
`src-tauri`, là où le crate se compile).

### Déboguer chez le pilote

Tout est dans `%LOCALAPPDATA%\Corde\logs\corde-desktop.log` : lancement, port choisi, bannière du serveur,
jeu détecté, entrée en piste, affichage de la fenêtre, arrêt, paniques de tous les threads.

### Icônes

`node apps/desktop/scripts/make-icons.mjs` régénère tout depuis `assets/corde-icon.svg` : chaque PNG est
rendu à sa taille par Chromium (Playwright, celui de `packages/ui`), puis `scripts/pack-icons.py` (Python +
Pillow) assemble `icon.ico` (16 à 256 px) et `icon.icns` (16 à 1024 px). Le signe est le même que celui de la
barre de l'UI (`packages/ui/public/favicon.svg`, icônes web : `packages/ui/scripts/make-icons.mjs`). Pas de
jeux Android / iOS (pas encore de coquille mobile).

### Mettre à jour Tauri

Faire suivre ensemble `Cargo.toml` / `Cargo.lock` et `TAURI_CLI_VERSION` dans `.github/workflows/ci.yml`.
**Relire le modèle NSIS de la nouvelle version** (`tauri-bundler/src/bundle/windows/nsis/installer.nsi`) :
`windows/hooks.nsh` s'appuie sur `MUI_CUSTOMFUNCTION_GUIINIT` et sur le dossier par défaut
`$LOCALAPPDATA\<produit>`, et l'on compte sur le fait que le désinstalleur ne supprime que les fichiers
qu'il a installés (les données du pilote sont dans `%LOCALAPPDATA%\Corde`).

## Limites connues

- Jamais exécuté sous Windows par ses auteurs : compilé par la CI, logique sans Tauri testée sous Linux.
  Voir `docs/STATUS.md` pour la liste des points à vérifier sur le PC du pilote.
- Pas de mise à jour automatique (réinstaller par-dessus), pas de signature de code (ADR 0007).
- Fenêtre cachée après un premier affichage : l'UI reste chargée et continue de recevoir la télémétrie
  (un peu de CPU) ; la mettre en veille serait une optimisation possible.
- Arrêt de Windows / fermeture de session sans « Quitter » : le processus est tué par le système ;
  l'enregistrement en cours garde son dernier point de reprise (30 s au plus).
