# ADR 0001 — Stack technique : cœur Rust + UI web + Tauri 2

- **Statut** : acceptée — 2026-11-02
- **Source** : `docs/research/03-stack-technique.md` (versions vérifiées le 2 novembre 2026)

## Contexte

Corde doit tourner sur PC (Windows et Linux) puis sur mobile (iOS/Android), lire de la mémoire partagée à
≈ 100 Hz, enregistrer des sessions de plusieurs heures, analyser hors ligne et afficher un tableau de bord
de widgets à 20–60 Hz, y compris dans une source navigateur OBS. L'équipe est réduite (un développeur
principal assisté d'agents IA, trois développeurs en parallèle au démarrage) : il faut **une seule base de
code par couche**, outillée et testable sans le jeu.

Candidats évalués : Tauri 2, Electron + Capacitor, Flutter, .NET (Avalonia/MAUI), Python PySide6 (modèle
TinyPedal). Python n'a pas de mobile ; Flutter et .NET imposent de réécrire les widgets sans réutilisation
navigateur (OBS) ; Electron double le runtime et le poids des binaires.

## Décision

1. **Cœur Rust** (workspace Cargo) : acquisition (`memmap2` + `windows-sys` sur Windows, fichiers `/dev/shm`
   sous Linux), adaptateurs de jeu derrière le trait `corde_core::TelemetrySource`, enregistreur, analyse,
   moteur de règles. Le cœur est un **serveur local autonome** (`corde-server`, axum 0.8, WebSocket `/ws`,
   HTTP `/api`, fichiers statiques de l'UI).
2. **UI web unique** : React 19 + TypeScript 7 + Vite 8, grille gridstack.js 14 (tactile natif, layouts
   JSON), graphes uPlot, carte de circuit en Canvas 2D maison. Même paquet de widgets pour bureau, mobile et
   OBS.
3. **Tauri 2** pour emballer en applications natives (desktop en phase 1, mobile en phase 3). Sur mobile, le
   même crate compile en « mode client » (WebSocket vers le PC).
4. **Outillage** : pnpm 10 + Cargo workspace, Biome, Vitest, `cargo fmt`/`clippy`, GitHub Actions
   (Linux + Windows).

Plan B explicite : si le spike mobile Tauri échoue, Electron (desktop) + Capacitor (mobile) avec la même UI
et le même cœur en sidecar — aucun widget à réécrire.

## Conséquences

- Un seul langage côté données (Rust), un seul côté affichage (TypeScript) ; les types du protocole sont
  générés de Rust vers TypeScript (ADR 0002).
- N'importe quel navigateur du réseau local peut afficher les widgets : c'est ce qui donne « gratuitement »
  l'overlay OBS et le téléphone en phase 0.
- Risques à surveiller : WebKitGTK + NVIDIA sous Linux (fenêtre blanche, contournement
  `WEBKIT_DISABLE_DMABUF_RENDERER=1`), débogage mobile Tauri, MSRV Rust que Tauri 2.12 porte à 1.90 (le
  workspace est à 1.85 en phase 0).
- `unsafe` cantonné aux crates d'adaptateurs.

## Références

- Tauri 2 : https://tauri.app/release/tauri/ — updater : https://v2.tauri.app/plugin/updater/ — graphismes
  Linux : https://v2.tauri.app/develop/debug/linux-graphics/
- gridstack.js : https://github.com/gridstack/gridstack.js/blob/master/README.md — uPlot :
  https://npmjs.com/package/uplot
- Comparatifs : https://capawesome.io/docs/blog/electron-vs-tauri-for-capacitor-apps/ ,
  https://www.pkgpulse.com/guides/electron-vs-tauri-2026
- Exemple d'adaptateurs Rust pour simulateurs : https://docs.rs/crate/kerb/latest
