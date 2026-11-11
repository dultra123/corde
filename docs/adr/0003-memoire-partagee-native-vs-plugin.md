# ADR 0003 — Source primaire : mémoire partagée native `LMU_Data`, pas le plugin rF2

- **Statut** : acceptée — 2026-11-02
- **Source** : `docs/research/01-telemetrie-lmu.md` §1 et §8

## Contexte

Deux interfaces de télémétrie coexistent dans Le Mans Ultimate :

| | Mémoire partagée **native** S397 (`LMU_Data`) | Plugin **rF2 Shared Memory Map** (The Iron Wolf) |
|---|---|---|
| Disponible depuis | LMU V1.2 (9 déc. 2025) ; nouveaux champs en V1.3 (31 mars 2026) | 2024 (early access), DLL `rFactor2SharedMemoryMapPlugin64.dll` v3.7.15.1 (2023) |
| Installation | `Settings > Gameplay > Enable Plugins` + redémarrage ; **aucun fichier** | copier la DLL dans `Plugins\`, entrée dans `CustomPluginVariables.JSON` |
| Contenu | un bloc de 324 820 octets : `generic` (version, chemins), `scoring` (104 véhicules), `telemetry` (104 × `TelemInfoV01`) | 9 buffers `$rFactor2SMMP_*$` (128 véhicules), compteurs de version begin/end |
| Champs LMU ≥ 1.3 (TC/ABS/map live, énergie virtuelle, SoC, écarts, track limits) | **oui** | **non** |
| Synchronisation | pas de compteur de version : verrou coopératif (`LMU_SharedMemoryLockData`) ou copie + contrôle de cohérence / relecture témoin | seqlock `mVersionUpdateBegin/End` |
| Cadence | télémétrie ≈ 100 Hz, scoring ≈ 5 Hz | télémétrie 50 Hz, scoring 5 Hz |
| Avenir | référence de TinyPedal, SimHub (migration en cours), lmu-pitwall, irdashies | déprécié par TinyPedal (2.45.0), retiré de lmu-pitwall, « sera retiré » de SimHub |
| Licence | header S397 livré avec le jeu, sans licence publique (voir ADR 0005) | GPL-3 |

Le client a déjà la DLL du plugin dans son dossier `plugins\` (posée par un autre outil).

## Décision

1. **Source primaire = `LMU_Data`**, ouverte en lecture seule (`OpenFileMappingW` + `MapViewOfFile` avec
   `FILE_MAP_READ` sur Windows ; `/dev/shm/LMU_Data` sur Linux, ADR 0004).
2. Lecture dans un **thread dédié** : attente de `LMU_Data_Event` (Windows) ou polling 8–16 ms (Linux) ; copie
   du strict nécessaire sous le verrou coopératif, jamais plus de quelques microsecondes, **ou** copie puis
   contrôle de cohérence (`scoringInfo.mNumVehicles == telemetry.activeVehicles`, relecture témoin de
   `mElapsedTime` / `mCurrentET`). Une lecture déchirée est rejetée, pas corrigée.
3. **Couche versionnée** : lire `generic.gameVersion` et dégrader proprement (les champs électronique/énergie
   n'existent qu'à partir de 1.3) ; tests de taille/offset (`TelemInfoV01` = 1888, `VehicleScoringInfoV01`
   = 584, `ScoringInfoV01` = 548, `TelemWheelV01` = 260, total 324 820) relancés à chaque mise à jour du jeu.
4. **Le plugin rF2 n'est pas utilisé.** Il reste tolérable sur le PC de l'utilisateur (inoffensif) ; un mode
   « repli / comparaison » pourra être ajouté plus tard s'il devient nécessaire, jamais comme chemin
   principal.
5. L'API REST locale (`127.0.0.1:6397`) complète la mémoire partagée pour ce qu'elle n'expose pas (setup,
   température optimale des pneus, usure freins, prévisions) : **GET uniquement**, cadence ≥ 200 ms avec
   backoff, `RepairAndRefuel` ≥ 500 ms.

## Conséquences

- Aucun fichier à installer dans le jeu ; un seul prérequis utilisateur (`Enable Plugins` + redémarrage).
- Nos structures sont réécrites à partir du header (ADR 0005) et vérifiées par `reference/lmu-shared-memory-layout.txt`.
- Détection de la sortie du jeu par `appInfo.mAppWindow` / présence du processus : les handles ouverts
  gardent le mapping vivant.
- Coexistence avec SimHub, Crew Chief, TinyPedal : tous lisent la même mémoire ; Corde ne modifie aucun
  fichier partagé (`CustomPluginVariables.JSON`, `/dev/shm/*`).
- À confirmer sur le PC du client : noms exacts des objets de verrou dans le header livré, fréquences réelles,
  comportement d'EAC en ligne avec un lecteur externe, unité de `mVirtualEnergy`.

## Références

- Notes de version LMU V1.2 : https://guide.lemansultimate.com/hc/en-gb/articles/14556121957775-V1-2-Update-2 ;
  V1.3 : https://www.bsimracing.com/le-mans-ultimate-v1-3-and-new-hotfix-released/
- Ports MIT de l'interface native : https://github.com/TinyPedal/pyLMUSharedMemory ,
  https://github.com/Kalanur/lmu-pitwall , https://github.com/LuisGasparGH/goLMUSharedMemory
- Observations sur le verrou et les cadences : https://github.com/Pagh/LmuOverlay ,
  https://github.com/tariknz/irdashies/pull/778
- Plugin historique (GPL-3) : https://github.com/TheIronWolfModding/rF2SharedMemoryMapPlugin
- Dépréciation : changelog TinyPedal https://github.com/TinyPedal/TinyPedal ; SimHub
  https://www.simhubdash.com/download-2/
