# ADR 0002 — Un protocole normalisé, multi-jeux, généré de Rust vers TypeScript

- **Statut** : acceptée — 2026-11-02
- **Source** : `crates/corde-protocol/src/lib.rs`, `docs/ARCHITECTURE.md` §1–§4,
  `docs/research/03-stack-technique.md` §2 et §6

## Contexte

Les interfaces (bureau, mobile, OBS) ne doivent rien savoir du jeu. Chaque simulateur expose des structures
différentes (`TelemInfoV01` rF2/LMU, `acpmf_*` AC/ACC, irsdk iRacing), des unités différentes (Kelvin,
radians, fractions) et des sentinelles différentes (`-1`, `0`). Il faut un contrat unique entre le cœur Rust
et l'UI, stable dans le temps et typé des deux côtés.

## Décision

1. **Un seul modèle de données** dans `crates/corde-protocol` : `TelemetryFrame { session, player,
   standings }`, `LapRecord`, `GameEvent`, `SourceStatus`, `DashboardLayout`. C'est *le* contrat ; ajouter un
   jeu = écrire un adaptateur qui remplit ce modèle.
2. **Génération TypeScript** par `ts-rs` : `cargo test -p corde-protocol` écrit
   `packages/protocol/src/generated/*.ts` ; le paquet `@corde/protocol` les réexporte avec les constantes
   partagées (`PROTOCOL_VERSION`, `DEFAULT_PORT = 8471`, `WS_PATH = "/ws"`, ordre des roues).
3. **Conventions** (ARCHITECTURE §4) : unités humaines (°C, km/h et m/s, degrés, kPa, litres, secondes) ;
   roues toujours FL, FR, RL, RR ; `temp_c[3]` = gauche/centre/droite de la bande ; un temps absent est
   `None`, jamais `-1` ou `0` ; un champ qu'un jeu ne fournit pas est `None`, `0` ou `Unknown`, jamais deviné.
4. **Transport** : messages `ServerMessage` / `ClientMessage` tagués `type`, JSON en v0 ; MessagePack prévu
   avec le même schéma. Cadence négociée par client (`SetRate` 1–60 Hz, défaut 20). Les classements
   (`standings`) sont décimés à ≈ 5 Hz à la source.
5. **Versionnage** : `PROTOCOL_VERSION` incrémenté à tout changement incompatible, annoncé dans `Hello`.

## Conséquences

- Les conversions (Kelvin → °C, rad → °, fraction → %) vivent dans l'adaptateur (`corde_core::derive`), une
  seule fois.
- Les fichiers générés sont ignorés par git ; la CI et chaque développeur les régénèrent
  (`pnpm protocol:gen`). Le job web de la CI doit donc disposer d'une toolchain Rust.
- Les grandeurs dérivées utiles à l'analyse (slip ratio, slip angle, g) sont dans le protocole pour que
  l'UI et l'analyse partagent la même définition.
- Manques connus à ce jour (à traiter par un futur bump) : accélération de lacet (`mLocalRotAccel`) absente
  de `Motion` (nécessaire au diagnostic D-BAL-02) ; unité de `Energy.virtual_energy` à confirmer sur PC.
- Les widgets et la base de connaissances (`knowledge/`) référencent les champs par leur chemin dans ce
  protocole, ce qui rend la cohérence vérifiable par script (voir `knowledge/README.md` §4).

## Références

- ts-rs : https://docs.rs/ts-rs — serde : https://serde.rs
- Comparatif JSON / MessagePack / Protobuf sur WebSocket :
  https://dev.to/nate10/performance-analysis-of-json-buffer-custom-binary-protocol-protobuf-and-messagepack-for-websockets-2apn
- Champs disponibles dans LMU : `docs/research/01-telemetrie-lmu.md` §1.2
