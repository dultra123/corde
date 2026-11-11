# ADR 0005 — Licences : pas de code GPL, structures réécrites à partir du header S397

- **Statut** : acceptée — 2026-11-02
- **Source** : `docs/research/01-telemetrie-lmu.md` §6 (licences) et §8.9, `docs/ARCHITECTURE.md` §4

## Contexte

Les briques existantes autour de la télémétrie LMU ont des licences incompatibles entre elles :

| Brique | Licence | Usage possible pour Corde |
|---|---|---|
| Header S397 `SharedMemoryInterface.hpp` / `InternalsPlugin.hpp` (livré dans `Le Mans Ultimate\Support\SharedMemoryInterface\`) | **aucune licence publique** ; non publié sur GitHub par Studio 397 | lecture comme spécification d'interface ; pas de redistribution |
| TinyPedal, `rF2SharedMemoryMapPlugin` (The Iron Wolf, dont `rF2State.h`), winecarte | **GPL-3** | documentation uniquement ; **aucune copie de code** |
| `pyLMUSharedMemory`, `goLMUSharedMemory`, lmu-pitwall, lmu-telemetry-analyzer, `pyRfactor2SharedMemory`, DuckDB | MIT | réutilisables (avec mention), utiles pour un prototype ou pour croiser les offsets |

La destination finale de Corde (usage personnel, open source, commercial) n'est pas tranchée (question
ouverte au client, `docs/SPEC.md` §9). Une dépendance GPL figerait ce choix ; une copie du header S397
exposerait à un retrait.

## Décision

1. **Aucune copie de code GPL** dans le dépôt, ni de la DLL du plugin rF2. TinyPedal, le plugin et winecarte
   sont lus comme documentation ; winecarte est, sous Linux, un prérequis externe lancé par l'utilisateur
   (ADR 0004), jamais embarqué.
2. **Le header S397 n'est pas versionné dans le dépôt.** Nos structures Rust (`crates/corde-lmu/src/layout.rs`)
   sont **réécrites** à partir de sa lecture ; ce qui est versionné est `reference/lmu-shared-memory-layout.txt`
   (tailles et offsets calculés) et des tests `size_of` / `offset_of` qui échouent si le layout dérive
   (`TelemInfoV01` 1888, `VehicleScoringInfoV01` 584, `ScoringInfoV01` 548, `TelemWheelV01` 260, total 324 820).
   Les structures sont des faits d'interface, comme pour les ports MIT existants.
3. Les **échantillons réels** (`reference/samples/`) sont anonymisés avant dépôt (noms de pilotes, SteamID).
4. Les dépendances Rust/npm restent sous licences permissives (MIT, Apache-2.0, BSD, ISC, MPL-2.0 toléré pour
   les bibliothèques non modifiées) ; `cargo-deny` sera ajouté en CI (phase 1) pour l'imposer.
5. La licence de Corde lui-même est **« à définir — tous droits réservés pour l'instant »** (README) jusqu'à
   la réponse du client ; le code est écrit de façon à pouvoir passer en open source sans réécriture.
   **Mise à jour du 2026-11-11** : le client a choisi l'open source, sous licence **GPL-3.0-or-later** ; la
   suite (historique, publication) est dans l'[ADR 0008](0008-open-source-publication.md).

## Conséquences

- Un peu plus de travail au démarrage (réécriture + tests de layout) contre une liberté totale sur la
  licence finale.
- À chaque mise à jour de LMU, le client nous fournit le header de sa version (guide
  `docs/TESTING-ON-PC.md` §5) ; on compare et on met à jour `layout.rs` et le fichier de référence.
- Les contributeurs doivent savoir d'où ils lisent : une PR qui colle du code TinyPedal ou du plugin est
  refusée (`docs/CONTRIBUTING.md`).
- Les images et icônes suivent la même règle (TinyPedal : CC BY-SA 4.0, non réutilisées).

## Références

- Plugin rF2 (GPL-3, clause de redistribution de la DLL dans le README) :
  https://github.com/TheIronWolfModding/rF2SharedMemoryMapPlugin
- TinyPedal (GPL-3) : https://github.com/TinyPedal/TinyPedal — winecarte (GPL-3) :
  https://github.com/srounce/winecarte
- Ports MIT : https://github.com/TinyPedal/pyLMUSharedMemory , https://github.com/Kalanur/lmu-pitwall ,
  https://github.com/LuisGasparGH/goLMUSharedMemory , https://github.com/mzluzifer/lmu-telemetry-analyzer
- Notes de version LMU V1.2 (livraison du header dans `Support`) :
  https://guide.lemansultimate.com/hc/en-gb/articles/14556121957775-V1-2-Update-2
- cargo-deny : https://github.com/EmbarkStudios/cargo-deny
