# Brief technique — Extraction de données depuis Le Mans Ultimate (LMU)

Rédigé le 2026-11-02. Cible : appli de télémétrie bureau Windows + Linux, puis mobile connecté au PC.
Niveaux de confiance utilisés : **[VÉRIFIÉ]** = lu dans le code source / doc officielle / plusieurs sources concordantes ; **[PROBABLE]** = une seule source ou source indirecte ; **[À CONFIRMER PC]** = à tester sur la machine du joueur.

Méthode : clonage et lecture directe des dépôts `TinyPedal/pyLMUSharedMemory`, `TinyPedal/TinyPedal` (+ wiki), `TheIronWolfModding/rF2SharedMemoryMapPlugin`, `schlegp/rF2SharedMemoryMapPlugin_Wine`, `Kalanur/lmu-pitwall`, `Alexander-Gro/race-engineer`, `Pagh/LmuOverlay`, `srounce/winecarte`, `mzluzifer/lmu-telemetry-analyzer`, `seralaci/Le-Mans-Ultimate-Setups` ; lecture des changelogs LMU (guide.lemansultimate.com, bsimracing, traxion), docs SimHub, Crew Chief, Track Titan, RaceLab, DR Sim Manager, Coach Dave, Wine bugzilla, ProtonDB/GamingOnLinux.

---

## 0. Résumé exécutif

1. **Depuis LMU V1.2 (9 déc. 2025) le jeu publie lui-même une mémoire partagée native** : mapping Win32 `LMU_Data` (324 820 octets, un seul bloc), événement `LMU_Data_Event`, verrou `LMU_SharedMemoryLockData`/`LMU_SharedMemoryLockEvent`. Header officiel S397 livré dans `Le Mans Ultimate\Support\SharedMemoryInterface\SharedMemoryInterface.hpp`. Il suffit d'activer `Settings > Gameplay > Enable Plugins` (redémarrage requis). Aucune DLL à installer. [VÉRIFIÉ]
2. **Le plugin historique `rFactor2SharedMemoryMapPlugin64.dll` (The Iron Wolf, v3.7.15.1, GPL-3) fonctionne encore avec LMU** (buffers `$rFactor2SMMP_Telemetry$`, `_Scoring$`, `_Rules$`, `_MultiRules$`, `_ForceFeedback$`, `_Graphics$`, `_Extended$`, `_PitInfo$`, `_Weather$` + 4 buffers d'entrée), mais il est **déprécié** par TinyPedal, abandonné par lmu-pitwall, en cours d'abandon par SimHub, et **n'expose pas** les nouveaux champs LMU 1.3 (TC/ABS/map moteur live, énergie virtuelle, SoC, gaps). Il reste pertinent uniquement comme repli et pour Linux (fork Wine). [VÉRIFIÉ]
3. La structure native réutilise les structs rF2 (`TelemInfoV01` 1888 o, `VehicleScoringInfoV01` 584 o, `ScoringInfoV01` 548 o, pack 4), 104 véhicules max (vs 128), **pas de compteur `mVersionUpdateBegin/End`** : il faut lire sous verrou ou faire une double lecture témoin. Télémétrie ~100 Hz, scoring ~5 Hz. [VÉRIFIÉ]
4. **API REST locale `http://localhost:6397`** (port = `WebUI port` dans `UserData\player\Settings.JSON`), Swagger sur `/swagger/index.html` + `/swagger-schema.json` (OpenAPI 2.0, 178 chemins dont 107 non-GET). Lecture du setup courant via `/rest/garage/getPlayerGarageData` (objets `VM_*`), inventaire pneus, météo/prévisions, énergie virtuelle, usure freins/dégâts, estimation d'arrêt. **Des endpoints d'écriture (setup POST/PUT/DELETE, PitMenu) existent** mais sont non officiels et réputés fragiles (bug clignotement MFD). [VÉRIFIÉ / écriture À CONFIRMER PC]
5. **Setups = fichiers texte `.svm` (format rF2)** dans `UserData\player\Settings\<Circuit>\*.svm`, sections INI `[GENERAL]`, `[SUSPENSION]`, `[CONTROLS]`, `[ENGINE]`, `[DRIVELINE]`, `[FRONTLEFT]`…, valeurs = **index** + commentaire `//valeur affichée`. Une appli tierce peut écrire un `.svm` : la communauté le fait massivement, le jeu rescanne le dossier via le bouton Refresh du garage. [VÉRIFIÉ]
6. **Enregistrement natif de télémétrie en DuckDB** (`UserData\Telemetry\*.duckdb`, LMU ≥ 1.2) : une table par canal, tables `metadata` (dont le setup complet en JSON, clé `CarSetup`), `channelsList`, `eventsList`. C'est la meilleure source hors-ligne « tour + setup » pour la recommandation de setup. [VÉRIFIÉ]
7. **Linux** : LMU tourne sous Proton (GE-Proton conseillé, EAC OK en ligne selon témoignages 2026) mais est officiellement « unsupported ». La mémoire partagée Win32 reste invisible aux process natifs ; il faut un pont : **winecarte** (`wine2linux.exe` dans le préfixe → `/dev/shm/LMU_Data`, GPL-3) ou le fork Wine du plugin rF2 (`/dev/shm/$rFactor2SMMP_*$`). TinyPedal lit `/dev/shm/<nom>` tel quel. [VÉRIFIÉ]
8. **Pas d'UDP natif**, pas d'export MoTeC natif (le `.ld` vient de DAMPlugin ou d'outils tiers), replays `.Vcr` propriétaires, résultats XML `UserData\Log\Results\*.xml` (format `rFactorXML`). [VÉRIFIÉ/PROBABLE]
9. Concurrence « recommandation de setup LMU » : **SteadyState(LMU)** (payant, knowledge packs), **Coach Dave Delta/Setup IQ**, **lmu-telemetry-analyzer** (MIT, hints), **MyLMU** (SaaS, DuckDB). Track Titan et RaceLab utilisent leur propre DLL plugin. Garage 61 = iRacing only. [VÉRIFIÉ]
10. Licences : TinyPedal et plugin IronWolf = **GPL-3 (ne pas copier le code)** ; `pyLMUSharedMemory`, `goLMUSharedMemory`, lmu-pitwall, lmu-telemetry-analyzer = **MIT** ; les **définitions de structures sont celles du header S397** (pas de licence publique connue → ré-écrire nos structs à partir du header livré avec le jeu). [VÉRIFIÉ]

---

## 1. Mémoire partagée

### 1.1 Deux interfaces coexistent (chronologie)

| Date | Événement | Source |
|---|---|---|
| 2024 (early access) → 2025 | Seule voie : plugin rF2 Shared Memory Map (The Iron Wolf) installé par SimHub/Crew Chief/l'utilisateur. | TinyPedal changelog, Crew Chief beta avr. 2024 |
| **9 déc. 2025 — LMU V1.2** | « Added SharedMemory Header file to support folder for new shared memory implementation » ; ajout d'EAC ; enregistrement télémétrie DuckDB. | https://guide.lemansultimate.com/hc/en-gb/articles/14556121957775-V1-2-Update-2 ; https://www.bsimracing.com/le-mans-ultimate-update-v1-2-full-changelog/ |
| 11 déc. 2025 | Première version de `pyLMUSharedMemory` (TinyPedal). TinyPedal 2.36.0 (13 déc. 2025) : « supports LMU's new built-in API, no plugin required… not accessible on Linux ». | git log pyLMUSharedMemory ; docs/changelog.txt TinyPedal |
| **31 mars 2026 — LMU V1.3** | « Added various new values to plugin system » → nouveaux champs (TC/ABS/map, énergie virtuelle, SoC, gaps, track limits…). pyLMUSharedMemory « updated to 1.3 spec » le même jour ; SimHub 9.11.6 migre « most data » vers la SHM native. TinyPedal déclare l'API legacy dépréciée (v2.45.0). | https://www.bsimracing.com/le-mans-ultimate-v1-3-and-new-hotfix-released/ ; https://www.simhubdash.com/download-2/ |
| juin 2026 — LMU 1.3.3 | Changement de résolution `localhost` côté REST (TinyPedal 2.47.0 ajoute une résolution d'hôte). | changelog TinyPedal |
| 21 juil. 2026 — LMU V1.4 ; 1.4.2 (sept. 2026) ; 1.4.3 | Pas de changement SHM annoncé. goLMUSharedMemory « supports LMU v1.4.2 ». lmu-probe (lmu-pitwall) validé sur LMU 1.4. | https://guide.lemansultimate.com/hc/en-gb/articles/16987981296015-V1-4-release-notes ; https://github.com/LuisGasparGH/goLMUSharedMemory |

Version LMU courante en nov. 2026 : branche **1.4.x** [PROBABLE]. Le champ `generic.gameVersion` (int) de `LMU_Data` permet de la lire à chaud [VÉRIFIÉ].

### 1.2 Interface native S397 (`LMU_Data`) — recommandée

**Activation** [VÉRIFIÉ] : `Settings > Gameplay > Enable Plugins = ON`, puis **redémarrage complet du jeu**. lmu-pitwall a vérifié qu'avec l'option OFF aucun mapping `LMU_Data` n'est créé, même sans aucune DLL dans `Plugins\`. Aucun fichier à installer. (https://github.com/Kalanur/lmu-pitwall README ; https://github.com/TinyPedal/TinyPedal README « Setup for Le Mans Ultimate »)

**Objets Win32** [VÉRIFIÉ, lus dans winecarte et LmuOverlay qui compilent contre le header S397] :
- Mapping : `LMU_Data` (sans `$`, sans préfixe `Local\` dans le code Python ; `OpenFileMappingA(FILE_MAP_READ, FALSE, "LMU_Data")` côté C++).
- Événement de mise à jour : `LMU_Data_Event` (signalé à chaque frame, ~100/s — LmuOverlay : « about 100 frames signalled per second », « LMU signals every frame (144+ Hz) »).
- Verrou coopératif : le header fournit `SharedMemoryLock::MakeSharedMemoryLock()` et `SharedMemoryUpdateWaiter::MakeSharedMemoryUpdateWaiter()` (LmuOverlay les appelle ; il tient le verrou uniquement le temps de quelques `memcpy` et mesure le temps de verrou dans son widget perf). Les noms d'objets sous-jacents observés dans winecarte (`--lmu-lock` recrée « LMU's lock objects ») sont le mapping `LMU_SharedMemoryLockData` = `struct { int32 waiters; int32 busy; }` et l'événement `LMU_SharedMemoryLockEvent` [PROBABLE — à confirmer dans le header livré avec le jeu].
- **Pas de bloc `mVersionUpdateBegin/End`** (contrairement au plugin rF2). Stratégies observées : (a) verrou S397 (LmuOverlay, winecarte `--lmu-lock`) ; (b) copie puis contrôle de cohérence `events.SME_UPDATE_SCORING/SME_UPDATE_TELEMETRY` + `scoringInfo.mNumVehicles == telemetry.activeVehicles` (pyLMUSharedMemory, mode « copy access ») ; (c) relecture témoin de `mCurrentET` / `mElapsedTime` avant-après (lmu-pitwall `reader.rs`). (https://github.com/Pagh/LmuOverlay ; https://github.com/srounce/winecarte ; https://github.com/TinyPedal/pyLMUSharedMemory)

**Header de référence** [VÉRIFIÉ] : `<install LMU>\Support\SharedMemoryInterface\SharedMemoryInterface.hpp` (+ `InternalsPlugin.hpp`). Non publié sur GitHub par S397 ; les ports publics sont `pyLMUSharedMemory/lmu_data.py` (Python ctypes, MIT, à jour du 2026-11-01), `lmu-pitwall/bridge/src/shared_memory/lmu_data.rs` (Rust, MIT), `goLMUSharedMemory` (Go, MIT), `irdashies/src/app/lmu/native/lmu_struct.h` (C++).

**Layout global `SharedMemoryLayout { SharedMemoryObjectOut data; }`** — tailles et offsets recalculés avec ctypes à partir de `lmu_data.py` [VÉRIFIÉ] :

| Bloc | Type | Offset | Taille |
|---|---|---|---|
| `generic` | `SharedMemoryGeneric` | 0 | 332 |
| `generic.events` | 16 × `uint32` compteurs (`SME_ENTER, SME_EXIT, SME_STARTUP, SME_SHUTDOWN, SME_LOAD, SME_UNLOAD, SME_START_SESSION, SME_END_SESSION, SME_ENTER_REALTIME, SME_EXIT_REALTIME, SME_UPDATE_SCORING, SME_UPDATE_TELEMETRY, SME_INIT_APPLICATION, SME_UNINIT_APPLICATION, SME_SET_ENVIRONMENT, SME_FFB`) | 0 | 64 |
| `generic.gameVersion` | `int32` | 64 | 4 |
| `generic.FFBTorque` | `float` | 68 | 4 |
| `generic.appInfo` | `ApplicationStateV01` (`mAppWindow` HWND, `mWidth`, `mHeight`, `mRefreshRate`, `mWindowed`, `mOptionsLocation` 0=main UI/1=loading/2=monitor/3=on track, `mOptionsPage[31]`) | 72 | 260 |
| `paths` | `SharedMemoryPathData` : 5 × `char[260]` (`userData`, `customVariables`, `stewardResults`, `playerProfile`, `pluginsFolder`) | 332 | 1300 |
| `scoring` | `SharedMemoryScoringData` | 1632 | 126 832 |
| `scoring.scoringInfo` | `ScoringInfoV01` | 1632 | 548 |
| `scoring.scoringStreamSize` | 12 octets | 2180 | 12 |
| `scoring.vehScoringInfo[104]` | `VehicleScoringInfoV01` × 104 | 2192 | 60 736 |
| `scoring.scoringStream` | `char[65536]` (flux résultats, lignes `\n`) | 62 928 | 65 536 |
| `telemetry` | `SharedMemoryTelemetryData` | 128 464 | 196 356 |
| `telemetry.activeVehicles` / `playerVehicleIdx` / `playerHasVehicle` | `uint8`/`uint8`/`bool` | 128 464 / 128 465 / 128 466 | |
| `telemetry.telemInfo[104]` | `TelemInfoV01` × 104 (1888 o chacun) | 128 468 | 196 352 |
| **Total** | | | **324 820** (irdashies confirme « a snapshot is 324,820 bytes ») |

Les chemins `paths.*` donnent directement `UserData`, `CustomPluginVariables.JSON`, le profil joueur et le dossier plugins : **utile pour localiser setups/telemetry sans deviner le chemin Steam** [VÉRIFIÉ, usage À CONFIRMER PC].

**Fréquences** [VÉRIFIÉ par mesures tierces] : télémétrie ~100 Hz (irdashies « LMU publishes shared-memory data at 100 Hz », lmu-pitwall « 100 Hz telemetry »), scoring ~5–6 Hz (`mLapDist` 5 Hz ; LmuOverlay « ~6 Hz »). irdashies tourne sa boucle à 8 ms car la granularité timer Windows (15,6 ms) fait perdre des frames à 16 ms.

**Champs les plus utiles — `TelemInfoV01` (par véhicule, pack 4, 1888 o)** [VÉRIFIÉ, docstrings = commentaires du header]. Unités : m, m/s, rad, N, kPa, Kelvin pour pneus/freins, °C pour eau/huile, litres.

| Catégorie | Champs (offset dans la struct) |
|---|---|
| Identité/temps | `mID` (0, slot réutilisable en MP), `mDeltaTime`, `mElapsedTime` (12), `mLapNumber`, `mLapStartET`, `mVehicleName[64]`, `mTrackName[64]`, `mVehicleModel[30]` (796, marque+modèle), `mVehicleClass` uint8 (826 : 0=Hypercar, 2=LMP2_ELMS, 3=LMP2, 4=LMP3, 5=GTE, 6=GT3, 8=PaceCar, 0xFF=Unknown), `mVehicleChampionship` uint8 (0..3 = WEC 2023..2026, 0x10/0x11 = ELMS 2025/2026) |
| Cinématique | `mPos` (xyz, m), `mLocalVel` (184) → vitesse = norme, `mLocalAccel`, `mOri[3]`, `mLocalRot`, `mLocalRotAccel`. Repère véhicule vérifié par LmuOverlay : x = gauche, y = haut, z = arrière (monde main gauche) |
| Moteur/transmission | `mGear` (352, -1/0/1+), `mEngineRPM` (356), `mEngineMaxRPM`, `mEngineWaterTemp`, `mEngineOilTemp` (°C), `mClutchRPM`, `mEngineTorque`, `mTurboBoostPressure`, `mMaxGears`, `mSpeedLimiter`, `mIgnitionStarter` |
| Entrées | `mUnfilteredThrottle/Brake/Steering/Clutch`, `mFilteredThrottle/Brake/Steering/Clutch`, `mSteeringShaftTorque`, `mVisualSteeringWheelRange`, `mPhysicalSteeringWheelRange` |
| Aéro/châssis | `mFront3rdDeflection`, `mRear3rdDeflection`, `mFrontWingHeight`, `mFrontRideHeight`, `mRearRideHeight`, `mDrag`, `mFrontDownforce`, `mRearDownforce`, `mFrontFlapActivated`, `mRearFlapActivated`, `mRearFlapLegalStatus` |
| Carburant/énergie | `mFuel` (524, L), `mFuelCapacity`, `mVirtualEnergy` float (776, fraction — unité À CONFIRMER PC : LmuOverlay gère 0–1 ou %), `mScheduledStops` |
| Hybride | `mBatteryChargeFraction` (704), `mElectricBoostMotorTorque` (négatif en régén), `mElectricBoostMotorRPM`, `mElectricBoostMotorTemperature`, `mElectricBoostWaterTemperature`, `mElectricBoostMotorState` (744 : 0=indispo,1=inactif,2=propulsion,3=régén), `mRegen` float kW (768), `mStateOfCharge` float % (772), `mMotorMap`/`mMotorMapMax` |
| Aides (LMU ≥ 1.3) | `mABSActive`, `mTCActive`, `mSpeedLimiterActive`, `mTC`/`mTCMax` (750), `mTCSlip`/`mTCSlipMax`, `mTCCut`/`mTCCutMax`, `mABS`/`mABSMax` (756), `mMigration`/`mMigrationMax` (760, brake migration), `mFrontAntiSway`/`Max`, `mRearAntiSway`/`Max` (barres anti-roulis réglables en roulant), `mRearBrakeBias` (664, fraction arrière), `mWiperState`, `mLiftAndCoastProgress` (0–255) |
| Course | `mDeltaBest` (696), `mLapInvalidated` (745), `mTrackLimitsSteps` (points track-limits normalisés = points × `mTrackLimitsStepsPerPoint`), `mTimeGapCarAhead/Behind`, `mTimeGapPlaceAhead/Behind` (780…), `mCurrentSector` (bit de signe = pitlane) |
| Dégâts | `mDentSeverity[8]`, `mLastImpactET`, `mLastImpactMagnitude`, `mLastImpactPos`, `mDetached`, `mOverheating`, `mHeadlights` |
| Pneus (noms) | `mFrontTireCompoundIndex`, `mRearTireCompoundIndex`, `mFrontTireCompoundName[18]`, `mRearTireCompoundName[18]` |
| `mWheels[4]` (848, ordre FL, FR, RL, RR, 260 o chacun) | voir ci-dessous |

**`TelemWheelV01` (par roue)** [VÉRIFIÉ] : `mSuspensionDeflection` (m), `mRideHeight` (m), `mSuspForce` (N, charge pushrod), `mBrakeTemp` (K), `mBrakePressure` (0–1 pour l'instant), `mRotation` (rad/s), `mLateralPatchVel`, `mLongitudinalPatchVel`, `mLateralGroundVel`, `mLongitudinalGroundVel`, `mCamber` (rad), `mLateralForce`, `mLongitudinalForce`, `mTireLoad` (N), **`mGripFract`** (fraction de patch en glissement), `mPressure` (kPa), **`mTemperature[3]`** (K, **gauche/centre/droite**, pas intérieur/extérieur), `mWear` (0–1, **1.0 = neuf**, non proportionnel à la perte de grip), `mTerrainName[16]`, `mSurfaceType` (0 dry,1 wet,2 grass,3 dirt,4 gravel,5 rumble,6 special), `mFlat`, `mDetached`, `mStaticUndeflectedRadius` (cm, uint8), `mVerticalTireDeflection`, `mWheelYLocation`, `mToe`, `mTireCarcassTemperature` (K), `mTireInnerLayerTemperature[3]` (K), `mOptimalTemp` (°C, float), `mCompoundIndex`, `mCompoundType` (0 soft,1 medium,2 hard,3 wet).
Dérivables : slip ratio/angle à partir des vitesses de patch vs sol et `mRotation × rayon` ; vitesses d'amortisseur par dérivation de `mSuspensionDeflection` à 100 Hz.

**`ScoringInfoV01` (session)** [VÉRIFIÉ] : `mTrackName`, `mSession` (0 test, 1–4 practice, 5–8 qual, 9 warmup, 10–13 race), `mCurrentET`, `mEndET`, `mMaxLaps`, `mLapDist` (longueur piste), `mNumVehicles`, `mGamePhase` (0..9, 6 = FCY/safety car), `mYellowFlagState`, `mSectorFlag[3]` (LmuOverlay : 11 = vert, 1 = jaune), `mStartLight`, `mNumRedLights`, `mInRealtime`, `mPlayerName`, `mPlrFileName`, `mDarkCloud`, `mRaining`, `mAmbientTemp`, `mTrackTemp`, `mWind` (toujours 0 dans LMU/rF2 selon TinyPedal), `mMinPathWetness`, `mMaxPathWetness`, `mAvgPathWetness`, `mGameMode`, `mServerName`, `mServerPort`, `mServerPublicIP`, `mMaxPlayers`, `mStartET`, `mSessionTimeRemaining`, `mTimeOfDay`, `mIsFixedSetup`, **`mTrackGripLevel`** (0 green…4 saturated), **`mCloudCoverage`** (0..10), `mTrackLimitsStepsPerPenalty`, `mTrackLimitsStepsPerPoint`.

**`VehicleScoringInfoV01` (par véhicule, 584 o)** [VÉRIFIÉ] : `mID`, `mDriverName[32]`, `mVehicleName[64]`, `mTotalLaps`, `mSector` (0 = S3, 1 = S1, 2 = S2 !), `mFinishStatus`, `mLapDist`, `mPathLateral`, `mTrackEdge`, `mBestSector1/2`, `mBestLapTime`, `mLastSector1/2`, `mLastLapTime`, `mCurSector1/2`, `mNumPitstops`, `mNumPenalties`, `mIsPlayer`, `mControl`, `mInPits`, `mPlace`, **`mVehicleClass[32]` (chaîne : « Hyper », « LMP2 », « GT3 » observés en live avec le plugin legacy — À CONFIRMER PC sur l'API native)**, `mTimeBehindNext`, `mLapsBehindNext`, `mTimeBehindLeader`, `mLapsBehindLeader`, `mLapStartET`, `mPos/mLocalVel/mLocalAccel/mOri/mLocalRot/mLocalRotAccel`, `mHeadlights`, `mPitState` (0 none,1 request,2 entering,3 stopped,4 exiting), `mServerScored`, `mIndividualPhase`, `mQualification`, `mTimeIntoLap`, `mEstimatedLapTime`, `mPitGroup`, `mFlag` (0 vert, 6 bleu), `mUnderYellow`, `mCountLapFlag`, `mInGarageStall`, `mUpgradePack[16]`, `mPitLapDist`, `mBestLapSector1/2`, `mSteamID` (uint64), `mVehFilename[32]`, `mAttackMode`, `mFuelFraction` (uint8 0x00–0xFF), `mDRSState`. Sentinelles temps : `-1.0`/`0.0` = pas de temps.

**Ce qui N'EST PAS exposé par la SHM native** [VÉRIFIÉ sauf mention] :
- Valeurs de setup (ressorts, amortisseurs, ailerons, géométrie, rapports) → `.svm`, REST garage ou `metadata.CarSetup` du DuckDB.
- Usure des freins, dégâts aéro/suspension chiffrés, inventaire des trains de pneus, prévisions météo, temps d'arrêt estimé, énergie virtuelle max/consommation par tour → **REST uniquement**.
- Existence/état du safety car (`mSafetyCarExists`/`mSafetyCarActive` du buffer `Rules` du plugin rF2) : pas d'équivalent ; se rabattre sur `mGamePhase == 6` + `mYellowFlagState` (lmu-probe).
- Pression de frein réelle (kPa) : `mBrakePressure` reste 0–1 « for now ».
- Pressions/températures par zone au-delà de 3 zones ; pas de température de disque par face.
- Tracé de piste/centerline : rien ; TinyPedal/irdashies enregistrent la trace du joueur.
- Télémétrie complète des adversaires en ligne : la structure réserve un `TelemInfoV01` par voiture (LmuOverlay lit `mWear`, `mVirtualEnergy`, composés des autres voitures) mais l'usure des autres en multijoueur est « assumed not sent » [À CONFIRMER PC].
- Avec le **plugin legacy** en plus : aucun niveau TC/ABS/map live (race-engineer l'a constaté en juin 2026 : « the live TC/ABS/mixture level the driver dials on track is NOT externally readable » — vrai pour le legacy, **faux pour la native ≥ 1.3** qui a `mTC/mABS/mMotorMap` : à valider en roulant), pas de `mVirtualEnergy`, pas de gaps, 128 slots.

Sources : https://github.com/TinyPedal/pyLMUSharedMemory (lmu_data.py, lmu_enum.py, lmu_mmap.py) ; https://github.com/Kalanur/lmu-pitwall (README, tools/lmu-probe) ; https://github.com/Pagh/LmuOverlay (README « Verified / still to verify ») ; https://github.com/tariknz/irdashies/pull/778 ; https://github.com/Alexander-Gro/race-engineer/blob/main/docs/03-LMU-INTEGRATION.md.

### 1.3 Plugin historique `rF2 Shared Memory Map` (The Iron Wolf)

- **Livré avec LMU ?** Non d'après toutes les docs d'intégration (TinyPedal, DR Sim Manager, lmu-pitwall, SteadyState) qui demandent de le télécharger/copier ; SimHub et Crew Chief l'installent eux-mêmes (« Crew Chief will always ship with the latest version of the .dll »). Un utilisateur l'a trouvé « already present » parce qu'un autre outil l'avait posé. [PROBABLE — À CONFIRMER PC : vérifier `Plugins\` sur une install vierge]
- **DLL** : `rFactor2SharedMemoryMapPlugin64.dll`. Dépôt de référence : https://github.com/TheIronWolfModding/rF2SharedMemoryMapPlugin (dernier commit 31 déc. 2024, version **3.7.15.1** du 25 mars 2023 ; download sur mediafire `rf2_sm_tools_3.7.15.1.zip`). **Licence GPL-3** (fichier LICENSE) + clause README « You are allowed to include this .dll with your distribution, as long as it is: Free; This Readme is included ». Pas de fork « LMU » officiel ; variantes : `schlegp/rF2SharedMemoryMapPlugin_Wine` (Linux, build `rFactor2SharedMemoryMapPlugin64_Wine.dll`, 2023), `TonyWhitley/rF2SharedMemoryMapPlugin` (variante Crew Chief avec préfixe de nom différent, cf. option TinyPedal `memory_map_prefix`), `SeriousOldMan/…`, `ianv1/BSTSharedMemoryMapPlugin`. [VÉRIFIÉ]
- **Emplacement LMU** : `…\steamapps\common\Le Mans Ultimate\Plugins\` (racine du jeu, pas `Bin64\Plugins` comme rF2) — concordant DR Sim Manager, RaceLab, Track Titan, lmu-pitwall, gist TinyPedal/OpenSUSE. [VÉRIFIÉ] (un README isolé dit `Bin64\Plugins` : À CONFIRMER PC si un `Bin64` existe.)
- **Activation** : (1) `Settings > Gameplay > Enable Plugins` ; (2) `UserData\player\CustomPluginVariables.JSON`, entrée `"rFactor2SharedMemoryMapPlugin64.dll": { " Enabled": 1, "DebugOutputLevel": …, "DebugOutputSource": …, "DebugISIInternals": 0, "DedicatedServerMapGlobally": 0, "EnableDirectMemoryAccess": 0, "EnableHWControlInput": 1, "EnableWeatherControlInput": 0, "EnableRulesControlInput": 0, "UnsubscribedBuffersMask": 160 }` — **espace initial obligatoire dans `" Enabled"`** ; entrée créée automatiquement au premier lancement/fermeture du jeu avec la DLL présente ; le jeu doit être fermé pour éditer. Le menu in-game peut afficher le plugin « enabled » alors que le JSON dit 0 (SteadyState). Logs : `UserData\Log\RF2SMMP_DebugOutput.txt`. [VÉRIFIÉ] (https://docs.departedreality.com/dr-sim-manager/general/sources/le-mans-ultimate/ ; https://www.overtake.gg/threads/steadystate-lmu.296916/page-4)
- **Buffers** (`Source/rFactor2SharedMemoryMap.cpp` l.173–186) [VÉRIFIÉ] : sortie `$rFactor2SMMP_Telemetry$`, `$rFactor2SMMP_Scoring$`, `$rFactor2SMMP_Rules$`, `$rFactor2SMMP_MultiRules$`, `$rFactor2SMMP_ForceFeedback$`, `$rFactor2SMMP_Graphics$`, `$rFactor2SMMP_Extended$`, `$rFactor2SMMP_PitInfo$`, `$rFactor2SMMP_Weather$` ; entrée `$rFactor2SMMP_HWControl$`, `$rFactor2SMMP_WeatherControl$`, `$rFactor2SMMP_RulesControl$`, `$rFactor2SMMP_PluginControl$`. Serveur dédié : suffixe PID, ou `Global\` si `DedicatedServerMapGlobally`.
- **Fréquences** (README) [VÉRIFIÉ] : Telemetry 50 FPS (delta ET ≤ 2 ms), Scoring 5, Rules 3, MultiRules sur callback, ForceFeedback 400, Graphics 400, PitInfo 100, Weather 1, Extended 5 + callbacks. `Graphics` et `Weather` désabonnés par défaut (`UnsubscribedBuffersMask = 160 = 32 + 128`).
- **Versioning** [VÉRIFIÉ] : chaque buffer commence par `rF2MappedBufferVersionBlock { unsigned long mVersionUpdateBegin; unsigned long mVersionUpdateEnd; }` (`Include/rF2State.h` l.860). Lecture sûre : copier, comparer begin/end, réessayer si différents (voir `rF2SMMonitor.MappedBuffer<>.GetMappedData`). `ForceFeedback` = un seul double, pas de synchro.
- **Définitions de structures** : `Include/rF2State.h` (C++, `#pragma pack(push, 4)`, `MAX_MAPPED_VEHICLES = 128`, `MAX_MAPPED_IDS = 512`), `Monitor/rF2SMMonitor/rF2SMMonitor/rF2Data.cs` (C#), `TonyWhitley/pyRfactor2SharedMemory/rF2data.py` (Python, MIT, 2022), fork `s-victor/pyRfactor2SharedMemory` (`rf2_data.py`, MIT, Linux `/dev/shm`). [VÉRIFIÉ]
- **Compatibilité LMU** : fonctionne toujours (tests live race-engineer 14–16 juin 2026 sur LMU 1.3.x, 53 voitures ; SteadyState sept. 2026 ; kerb 0.4.0 oct. 2026) mais **TinyPedal 2.45.0 (avr. 2026) : « Le Mans Ultimate (legacy) API is now deprecated and no longer maintained… will be removed »** ; lmu-pitwall l'a retiré ; SimHub 9.11.6 : « plugin dependency still required for now, but will be removed very soon ». [VÉRIFIÉ]
- **Spécifique LMU dans le legacy** : champs hybrides déjà présents (`mBatteryChargeFraction`, `mElectricBoostMotor*`) ; classes lues `Hyper`/`LMP2`/`GT3` ; pas d'énergie virtuelle ni d'aides live. `rF2Extended` ajoute dégâts accumulés, transitions de session, `mSafetyCarExists/Active`, messages (avec `EnableDirectMemoryAccess`, fragile).

---

## 2. API REST locale

- **Base** : `http://localhost:6397` ; port = clé « WebUI port » de `UserData\player\Settings.JSON` (TinyPedal FAQ) ; serveur = backend de l'UI web du jeu, **actif dès que le jeu tourne, sans plugin**. Depuis LMU **1.3.3** `localhost` peut résoudre en IPv6 alors que le serveur n'écoute qu'en IPv4 : utiliser `127.0.0.1` (race-engineer : `[::1]` refusé) et/ou résoudre l'hôte (TinyPedal `resolve_hostname`). [VÉRIFIÉ] (https://github.com/TinyPedal/TinyPedal/wiki/Frequently-Asked-Questions ; TinyPedal `tinypedal/setting/default_api.py` : `url_host: "localhost", url_port: 6397`)
- **Découverte** : `GET /swagger/index.html` (UI) et `GET /swagger-schema.json` (OpenAPI 2.0, titre `LMU`, version `1.0`) — **la seule source faisant foi pour un build donné** (178 chemins dont 107 POST/PUT/DELETE en juin 2026). [VÉRIFIÉ par capture live race-engineer 2026-06-16]
- Pas d'authentification ni de CORS documentés [À CONFIRMER PC] ; les réponses sont du JSON, parfois volumineuses.

**Endpoints GET documentés par la communauté** (tous observés en live par au moins un outil) :

| Endpoint | Contenu | Utilisé par |
|---|---|---|
| `/rest/sessions` | infos session (`SESSSET_race_timescale.currentValue`, `SESSSET_private_qual`…) | TinyPedal |
| `/rest/sessions/weather` | météo + prévisions par session (`PRACTICE`/`QUALIFY`/`RACE`, 5 nœuds 0–100 %) | TinyPedal, lmu-pitwall, LmuOverlay |
| `/rest/sessions/GetGameState` | enums texte : `gamePhase:"GPHASE_GREEN"`, `PitState`, `MultiStintState`, `PitEntryDist`, `playerVehicleLoaded`, `raceFinished` | race-engineer |
| `/rest/sessions/getAllVehicles` | véhicules installés | communauté |
| `/rest/watch/standings` | classement riche par voiture : `carClass`, `carPosition{x,y,z}`, temps/secteurs, `lapDistance`, `pitState`, `timeBehind{Leader,Next,ClassLeader}`, `lapsBehind…`, **`fuelFraction`, `veFraction`**, `penalties`, `flag`, `underYellow`, `driverName`, `player` | race-engineer |
| `/rest/watch/sessionInfo` | `ambientTemp`, `trackTemp`, `gamePhase`, `maxTime`, `maximumLaps`, `raceCompletion`, `sectorFlag`, `yellowFlagState`, `timeRemainingInGamePhase`, `raining`, `windSpeed` | race-engineer |
| `/rest/strategy/usage` | historique d'énergie virtuelle par tour, par pilote (clé = nom) | lmu-pitwall, race-engineer |
| `/rest/strategy/pitstop-estimate` | `{fuel, tires, brakes, brakeDucts, damage, driverSwap, penalties, ve, total}` en secondes (0.0 tant qu'aucun arrêt n'est configuré) | TinyPedal, LmuOverlay |
| **`/rest/garage/getPlayerGarageData`** | **setup courant** : objets `VM_*` `{value (index), stringValue, lastSavedStringValue, minValue, maxValue}` : `VM_STEER_LOCK`, `VM_BRAKE_BALANCE` (`"49.5:50.5"`), `VM_ENGINE_MIXTURE`, `VM_VIRTUAL_ENERGY` (`"100% (11.1 laps)"`), `VM_FUEL_LEVEL`, `VM_FRONT_TIRE_COMPOUND`, `VM_TRACTIONCONTROLMAP`, `VM_ANTILOCKBRAKESYSTEMMAP`, `VM_TRACTION_CONTROL`/`VM_ANTILOCK_BRAKES` (flags « Available »)… **Snapshot figé au garage : ne bouge pas en roulant** (vérifié sur 3,5 tours). | TinyPedal (`export_lmu_car_setup`, sauvegarde auto de setup), LmuOverlay, race-engineer |
| `/rest/garage/getVehicleCondition` | `fuel`, `fuelCapacity`, `tireCondition[4]`, `brakeCondition[4]`, `suspensionDamage[4]`, `vehicleDamage` | race-engineer |
| **`/rest/garage/UIScreen/RepairAndRefuel`** | `fuelInfo{currentVirtualEnergy,maxVirtualEnergy…}`, `pitMenu`, `pitRecommendations`, `pitStopLength`, `pitStopTimes`, `wearables{body.aero, brakes[4], suspension[4]}`, `weatherForecast`. **Seule source** d'usure freins/dégâts/VE max. **Poller trop vite fait clignoter le menu MFD Pitstop** (bug reconnu par S397, non corrigé) → TinyPedal 100–200 ms avec backoff jusqu'à 5 s, irdashies hash FNV + backoff ×1,5. | TinyPedal, lmu-pitwall, LmuOverlay, irdashies |
| `/rest/garage/UIScreen/TireManagement` | `tireInventory`, `tireInvGarageOptions`, `expectedUsage`, `optimalCompoundConditions`, `wheelInfo` (trains disponibles) | race-engineer |
| `/rest/garage/UIScreen/CarSetupOverview` | `carSetup`, `carPresetSetups`, `weatherForecast` | race-engineer |
| `/rest/hud` | visibilité des composants HUD (70 o) | race-engineer |

Historique utile : TinyPedal 2.30.1 (juin 2025) : « Updated URL addresses… due to changes from June update » ; certaines routes (`garage/Pitstop/getPitstopTimes`) **faisaient planter le jeu** dans certaines conditions (TinyPedal 2.2x) ; TinyPedal 2.51.0 (oct. 2026) a supprimé l'import de marques via REST (plus nécessaire). **Les chemins et schémas changent avec les mises à jour LMU** : prévoir une couche d'adaptation par version. [VÉRIFIÉ]

**Lire le setup courant ?** Oui (`getPlayerGarageData`, `CarSetupOverview`) — valeurs + libellés affichés, bornes min/max, « dernière valeur sauvegardée ». [VÉRIFIÉ]
**Écrire/appliquer un setup ?** Le Swagger liste des opérations **POST/PUT/DELETE sur `garage … setup`**, `PitMenu/loadPitMenu`, `SetCurrentVehicle`, `sessions/*` save/load/drive, `hud/toggle` ; Crew Chief utiliserait l'API pour régler le menu de stand (option désactivée par défaut, qualifiée de « problematic »). Aucun projet open source lu n'écrit de setup via REST ; TinyPedal et race-engineer s'y refusent explicitement. **Faisabilité technique : probable ; stabilité, effets de bord (MFD) et acceptabilité (EAC/ToS) : À CONFIRMER PC et à éviter en V1.** [PROBABLE]

Repos exploitant l'API : https://github.com/TinyPedal/TinyPedal (`tinypedal/adapter/lmu_restapi.py`, `_restapi.py`), https://github.com/Kalanur/lmu-pitwall (`bridge/src/garage_api.rs`), https://github.com/Pagh/LmuOverlay (`src/LmuRest.cpp`, WinHTTP vers `127.0.0.1:6397`), https://github.com/tariknz/irdashies/pull/778, https://github.com/Alexander-Gro/race-engineer (`packages/adapters/lmu/src/rest`, `apps/desktop/src/lmu-rest.ts`). Aucun dépôt nommé « lmu-rest » trouvé.

---

## 3. Fichiers de setup LMU

- **Emplacement** [VÉRIFIÉ] : `<install>\UserData\player\Settings\<DossierCircuit>\<nom>.svm` (ex. `Lemans`, `Spa`, `Portimao`, `Qatar`, `Monza`, `Sebring`…). Le **dossier est par circuit, la voiture est encodée dans le fichier** (pas dans le nom). GO Setups : pas de redémarrage nécessaire, bouton **Refresh** dans l'écran setups ; un setup d'une autre voiture n'apparaît pas même dans la même classe. (https://support.gosetups.gg/hc/en-gb/articles/34402916284178-Setups-Not-Appearing-in-LMU ; race-engineer S4 live 2026-06-16 ; dépôt https://github.com/seralaci/Le-Mans-Ultimate-Setups, 91 fichiers)
- **Format** [VÉRIFIÉ, fichier réel Hypercar BMW M Hybrid V8 lu] : texte ASCII, héritage rF2.
  ```
  VehicleClassSetting="BMW_M_Hybrid Hypercar WEC2025"
  UpgradeSetting=(103,0,0,0)
  //VEH=C:\...\Installed\Vehicles\BMW_M_Hybrid_V8_2023\1.19\15_25_WRT_A1285A64.VEH
  //UpgradeClass=
  //Tyre Restrictions=1
  //AI Tweaks=3
  //Atmos Conditions=3
  //Note: settings commented out if using the default

  [GENERAL]
  Notes=""
  Symmetric=1
  FuelSetting=89//0.94
  FuelCapacitySetting=0//47.0L (13.3 laps)
  VirtualEnergySetting=50//50% (12.5 laps)
  ...
  [BASIC]
  Downforce=0.500000
  Balance=0.500000
  Ride=0.500000
  Gearing=0.500000
  Custom=1
  ```
  Autres en-têtes vus : `VehicleClassSetting="BMW_M4_LMGT3 GT3 WEC2025"`, `"GT3 Porsche_911_GT3_R_LMGT3 WEC2025"`, `"ELMS2025 LMP3 Ligier_JS_P325"` (ordre des mots variable).
- **Sections et clés** [VÉRIFIÉ] : `[GENERAL]` (`CGHeight/CGRight/CGRearSetting`, `WedgeSetting`, `FuelSetting`, `FuelCapacitySetting`, `VirtualEnergySetting`, `NumPitstopsSetting`, `Pitstop1..3Setting`), `[LEFTFENDER]/[RIGHTFENDER]` (`FenderFlareSetting`), `[FRONTWING]` (`FWSetting`), `[REARWING]` (`RWSetting` — Hypercar en crans `P1…P20`, GT3 en degrés), `[BODYAERO]` (`WaterRadiatorSetting`, `OilRadiatorSetting`, `BrakeDuctSetting`, `BrakeDuctRearSetting`), `[SUSPENSION]` (`Front/RearWheelTrackSetting`, `Front/RearAntiSwaySetting`, `Front/RearToeInSetting`, `Front/RearToeOffsetSetting`, `Left/RightCasterSetting`, `Left/RightTrackBarSetting`, `Front3rd*`/`Rear3rd*` : `Packer`, `Spring`, `TenderSpring`, `TenderTravel`, `SlowBump`, `FastBump`, `SlowRebound`, `FastRebound`, `ChassisAdj00..11Setting`), `[CONTROLS]` (`SteerLockSetting`, `RearBrakeSetting` `//55.0:45.0`, `BrakeMigrationSetting`, `BrakePressureSetting`, `HandfrontbrakePressSetting`, `HandbrakePressSetting`, `TCSetting`, `ABSSetting`, `TractionControlMapSetting`, `TCPowerCutMapSetting`, `TCSlipAngleMapSetting`, `AntilockBrakeSystemMapSetting`), `[ENGINE]` (`RevLimitSetting`, `EngineBoostSetting`, `RegenerationMapSetting` `//170kW`, `ElectricMotorMapSetting` `//30kW`, `EngineMixtureSetting`, `EngineBrakingMapSetting`), `[DRIVELINE]` (`FinalDriveSetting`, `ReverseSetting`, `Gear1..7Setting`, `RatioSetSetting`, `DiffPump/Power/Coast/PreloadSetting`, `FrontDiff*`, `RearSplitSetting`, `GearAutoUp/DownShiftSetting`), `[FRONTLEFT]/[FRONTRIGHT]/[REARLEFT]/[REARRIGHT]` (`CamberSetting`, `PressureSetting` `//130 kPa`, `PackerSetting`, `SpringSetting`, `TenderSpringSetting`, `TenderTravelSetting`, `SpringRubberSetting`, `RideHeightSetting`, `SlowBumpSetting`, `FastBumpSetting`, `SlowReboundSetting`, `FastReboundSetting`, `BrakeDiscSetting`, `BrakePadSetting`, `CompoundSetting`), `[BASIC]` (curseurs 0–1 + `Custom`).
- **Différences par classe** [VÉRIFIÉ sur 3 fichiers] : le jeu de clés est identique (seul `Gear7Setting` disparaît sur GT3) ; ce qui change, ce sont les libellés `//` (« N/A », « Non-adjustable », « Detached ») et les plages. Hypercar : régén/moteur électrique actifs, aileron en crans ; LMGT3 : ABS map « 6 (Balanced) », TC 3 niveaux, régén 0 % ; LMP3 : pas d'ABS, `TCSlipAngleMapSetting=4//Linked`, barres en mm. LMP2 : pas de TC/ABS (0). (https://configuracion.patagoniasimracing.cl/en/setups/lmu)
- **Sémantique** [VÉRIFIÉ] : la valeur stockée est un **index** (ou un delta vs défaut, cf. commentaire « settings commented out if using the default ») ; la valeur physique n'est reconstructible qu'avec les tables du `.VEH/.HDV` chiffré. Mais LMU **écrit la valeur affichée après `//`**, donc un parseur garde `{index, display}` et un diff de setups est fiable au cran près.
- **Écriture par une appli tierce** [VÉRIFIÉ pour le dépôt de fichiers ; chargement À CONFIRMER PC] : déposer un `.svm` complet dans `Settings\<Circuit>\` suffit (c'est ce que font GO Setups, Coach Dave Delta « setups installed automatically when you enter a session », simracingsetup.com). Points de vigilance : conserver la première ligne `VehicleClassSetting` et `UpgradeSetting` cohérentes avec la voiture (sinon le setup n'apparaît pas), respecter les index valides par voiture (bornes `minValue/maxValue` lisibles via `getPlayerGarageData`), encoder en ASCII/CRLF comme le jeu, ne pas écrire pendant que le jeu sauvegarde. TinyPedal implémente déjà une « sauvegarde auto de setup » à la sortie des stands à partir du REST.
- Les mêmes réglages apparaissent aussi en JSON dans `metadata.CarSetup` des fichiers DuckDB (`{stringValue, value, minValue, maxValue, lastSavedStringValue}`) — format identique à `getPlayerGarageData`. [VÉRIFIÉ]

---

## 4. Linux

**État du jeu sous Proton (nov. 2026)** :
- Officiel : la page « Known Issues and Advice » de S397 dit « Mac, Linux and Steamdeck are unsupported » ; GamingOnLinux classe LMU « Broken » (EAC). [VÉRIFIÉ] (https://www.gamingonlinux.com/anticheat/status/broken/)
- Terrain : Sim Racing Info (test avr. 2026 sur LMU 1.2, mis à jour août 2026 sur 1.4, Nobara) : fonctionne avec **GE-Proton** (build `GE-Proton10-25-LMU-hid_fixes` puis versions plus récentes obligatoires depuis 1.3), « no issues with the anti-cheat », sessions en ligne OK ; overlays et Crew Chief ne marchaient pas. Guide Steam « How to Play LMU on a Linux System » (id 3536726840) existe. ProtonDB : page non lisible depuis ici. [PROBABLE — À CONFIRMER PC : EAC en ligne avec la version LMU courante]
- Steam Deck : aucune source 2026 de bon fonctionnement trouvée ; considérer non cible. [À CONFIRMER]

**Pourquoi un process natif ne voit pas la SHM** [VÉRIFIÉ] : Wine implémente `CreateFileMapping` nommé (pagefile-backed) par des mappings anonymes/fichiers temporaires gérés par wineserver, **pas** par `/dev/shm/<nom>` (Wine bug 54015, réponses de Z. Figura et E. Pouech ; problèmes de nommage NT vs POSIX). `LMU_Data` est créé ainsi → invisible. D'où TinyPedal : « LMU's built-in API can be selected on Linux, but may require third-party plugin to access » (discussion #9).

**Solutions existantes** :
1. **winecarte** (https://github.com/srounce/winecarte, GPL-3, Rust, dernier commit 10 oct. 2026) [VÉRIFIÉ] : `wine2linux.exe` tourne dans le préfixe Proton du jeu (lancé par `winecarte-run %command%` via `steam-runtime-launch-client`, ou par le démon `winehub` qui rejoint les namespaces du conteneur), ouvre les mappings Win32 et les recopie dans `/dev/shm/<nom>` toutes les `--interval-ms` (défaut 16 ms), avec attente sur `LMU_Data_Event` et option `--lmu-lock` (recrée `LMU_SharedMemoryLockData`/`Event`). Pour LMU il mirrore `LMU_Data` + tous les `$rFactor2SMMP_*$`. Sens inverse possible (`--from-linux`, pour SimHub dans un autre préfixe). **Ne pas combiner avec le fork Wine du plugin** (mêmes fichiers).
2. **Fork Wine du plugin rF2** (https://github.com/schlegp/rF2SharedMemoryMapPlugin_Wine, `build/rFactor2SharedMemoryMapPlugin64_Wine.dll`, base 3.7.15.1, avr. 2023) [VÉRIFIÉ code] : dans `MappedBuffer.h` il remplace le mapping pagefile par un **fichier réel** `CreateFileA("/dev/shm/<nom>")` puis `CreateFileMappingA(hFile…)`, donc le buffer apparaît sous `/dev/shm/$rFactor2SMMP_Telemetry$` etc. Témoignage fonctionnel avec LMU + TinyPedal (gist OpenSUSE, 8 avr. 2026 : `$rFactor2SMMP_Scoring$`, `_Telemetry$`, `_Extended$`, `_ForceFeedback$` actifs). Limites : layout legacy (pas de champs 1.3), projet non maintenu.
3. **Relais UDP depuis le préfixe** (modèle `logi-tf-relay` du driver Logitech TrueForce Linux : exe Windows dans le préfixe → UDP localhost) [VÉRIFIÉ pour ACC, « decoder written, unconfirmed live » pour LMU].

**Comment TinyPedal lit sous Linux** [VÉRIFIÉ, `pyLMUSharedMemory/_common.py`] : `if sys.platform == "win32": mmap.mmap(-1, size, name)` sinon `open(f"/dev/shm/{name}", "a+b")` + `mmap.mmap(fd, size)` (le fichier est créé/retaillé à `sizeof(struct)` s'il manque — une version intermédiaire d'oct. 2026 refusait de le créer pour ne pas polluer les autres lecteurs ; comportement en cours de stabilisation). Même logique pour le legacy (`svictor_pyrf2/_common.py`, avec option `memory_map_prefix` pour les variantes de nom). Le nom de fichier est **identique au nom Win32**, `$` compris. Config/données TinyPedal : `~/.config/TinyPedal/`, `~/.local/share/TinyPedal/`. Les overlays nécessitent X11 (`enable_x11_platform_plugin_override`) ou « Bypass Window Manager » sous KDE ; pas de hotkeys globaux.

**Contraintes pour notre appli native Linux** : (a) dépendre d'un pont dans le préfixe (packager notre propre `*.exe` minimal plutôt que d'exiger winecarte GPL — ou le recommander comme prérequis) ; (b) lire `/dev/shm/LMU_Data` par `mmap` en lecture seule, taille 324 820 o, avec les mêmes contrôles de cohérence qu'en « copy access » ; (c) **pas d'événement/verrou Win32 côté Linux** → polling 8–16 ms + relecture témoin ; (d) l'API REST `127.0.0.1:6397` **est** accessible depuis Linux (le serveur écoute sur la pile réseau de l'hôte via Wine) [PROBABLE — À CONFIRMER PC] ; (e) chemins des fichiers : `~/.steam/steam/steamapps/common/Le Mans Ultimate/UserData/...` ou `~/.local/share/Steam/...` (lire `libraryfolders.vdf`) ; `paths.*` de `LMU_Data` donne des chemins Windows `C:\…` à convertir via `<prefix>/drive_c` ou le `Z:`.

---

## 5. Autres sources de données

- **UDP natif** : aucun. Aucune doc S397, aucun outil (TinyPedal, SimHub, lmu-pitwall, irdashies) n'en mentionne ; le seul « UDP » des changelogs est le netcode « Reliable UDP » de V1.4. SimHub peut rediffuser en UDP/WebSocket. [VÉRIFIÉ par absence]
- **Enregistrement natif DuckDB (LMU ≥ 1.2)** [VÉRIFIÉ] : activer `Settings > Gameplay > Automatic Telemetry Recording` (clé `"Automatically Record Telemetry": true` dans `UserData\player\Settings.JSON`) ou binder la touche « Telemetry Recording » ; icône dans le tachymètre. Fichiers `UserData\Telemetry\*.duckdb` (~20 Mo / 15 min). Schéma : **une table par canal** (`value` ou `value1..4` = FL, FR, RL, RR, colonne `ts` pour les événements), tables `metadata` (clé/valeur, dont `CarSetup` = JSON complet du setup), `channelsList(channelName, frequency, unit)`, `eventsList(eventName, unit)`. Canaux vus : « Lap Dist », « Ground Speed », « Throttle Pos », « Brake Pos », « Steering Pos », « Engine RPM », « GPS Latitude/Longitude », « G Force Lat/Long », « Fuel Level », « Virtual Energy », « SoC », « Wheel Speed », « Steering Shaft Torque », « Path Lateral », « Ambient/Track Temperature », « Wind Speed/Heading », « Tyres Wear », « TyresPressure », « TyresTempCentre/Left/Right », « TyresRubberTemp », « Brakes Temp » ; événements « Gear », « Lap », « Lap Time », « Last Sector1/2 », « Current Sector », « In Pits », « TC », « ABS », « TCLevel », « ABSLevel », « Best LapTime », « Minimum Path Wetness », « CloudDarkness », « Yellow Flag State ». Fréquences par canal dans `channelsList` (À CONFIRMER PC, non documentées). Lecteurs : DuckDB CLI/lib (MIT). (https://github.com/mzluzifer/lmu-telemetry-analyzer ; https://mylmu.app/docs/telemetry/getting-started/recording-in-lmu ; https://guide.lemansultimate.com/hc/en-gb/articles/14524956311695-Telemetry-Recording)
- **MoTeC `.ld`** : pas d'export natif confirmé ; le wiki communautaire parle de `.ld/.csv` dans `Documents\Le Mans Ultimate\Telemetry` mais c'est contredit par les outils (DuckDB dans `UserData\Telemetry`). Les `.ld` LMU proviennent de **DAMPlugin** (plugin rF2 `DAMPlugin.dll` + `DataFiles`, v2.00, téléchargement forum S397 instable en 2025) ou d'outils tiers. [PROBABLE] (https://lemansultimate.wiki.gg/wiki/Telemetry ; https://steamcommunity.com/app/2399420/discussions/0/594028005822747332/)
- **Plugins DLL rF2-style** : LMU charge des plugins `InternalsPlugin` 64-bit depuis `Plugins\` (Track Titan `TrackTitanRf2Plugin.dll`, RaceLab `RacelabLMUPlugin.dll`, DAMPlugin, ChatTransceiver, TrackIR) via `CustomPluginVariables.JSON` ; cela reste possible avec EAC (docs Track Titan 21 juin 2026, RaceLab 2026). Écrire notre propre DLL donnerait les callbacks à la fréquence physique et la main sur le format de sortie (y compris `/dev/shm` sous Wine), au prix d'un composant natif à maintenir par version et d'un risque EAC. [VÉRIFIÉ existence / risque À CONFIRMER]
- **Replays** : `.Vcr` propriétaires (héritage rF2), navigables par « LmuReplayBrowser » ; emplacement attendu `UserData\Replays\` [À CONFIRMER PC] ; pas de parseur public : inexploitable.
- **Logs** : `UserData\Log\` (traces, `RF2SMMP_*.txt` du plugin) ; `paths.stewardResults` dans `LMU_Data` pointe vers les résultats commissaires. [PROBABLE]
- **Résultats XML** [VÉRIFIÉ] : `UserData\Log\Results\*.xml`, format `rFactorXML > RaceResults > Setting, TrackVenue, Race/Qualify/Practice > Driver{Name, isPlayer, Position, Laps, Lap num= p= fuel= fuelUsed= twfl= twfr= twrl= twrr= FL="0,Medium" …}` (par tour : temps, position, carburant, usure des 4 pneus, composé). lmu-pitwall les importe en SQLite (`bridge/src/post_race/importer.rs`).

---

## 6. Écosystème et positionnement

| Outil | LMU | Source de données | Reco. de setup ? | Licence / modèle |
|---|---|---|---|---|
| **TinyPedal** (s-victor) v2.51.0 (3 oct. 2026) | oui, Windows + Linux | SHM native `LMU_Data` (legacy rF2 dépréciée) + REST 6397 | non (sauvegarde auto de setup, planificateur pneus, énergie) | **GPL-3** → ne pas copier ; `pyLMUSharedMemory` **MIT** |
| **SimHub** 9.11.x | oui | « most data » via SHM native depuis 9.11.6 (31 mars 2026), plugin rF2 encore requis « for now » ; données adversaires via NCalc/JS 9.11.12 | non | freeware/licence payante, fermé |
| **Crew Chief** | oui (bêta mars 2024) | plugin rF2 SMMP (il l'installe) ; REST pour le menu de stand (option off, [PROBABLE]) | non | open source sur GitLab (licence à vérifier), spotter |
| **Second Monitor** | oui (MR 201, mars 2024, v9.21.2) | plugin rF2 SMMP auto-installé | non | open source |
| **lmu-pitwall** (Swizzjack/Kalanur) | oui | `LMU_Data` + REST | non (stratégie, VE) | **MIT**, Rust + React |
| **LmuOverlay** (Pagh, oct. 2026) | oui | `LMU_Data` + REST | non | voir repo (C++) |
| **irdashies** (PR #778, oct. 2026) | oui | `LMU_Data` (addon natif) + REST | non | open source |
| **SteadyState(LMU)** (Mike Nice84, juil. 2026) | oui | plugin rF2 SMMP + dernier setup sauvegardé | **oui** (onglets Setup/Recommendations/History, « knowledge packs » payants par voiture) | propriétaire |
| **Coach Dave Delta / Setup IQ** | oui (7 sims) | app desktop fermée ; installe les setups automatiquement à l'entrée en session | **oui** (Setup IQ « tune to your driving style », setups pros) | abonnement 11,99 £/mois |
| **MyLMU** | oui | fichiers DuckDB natifs uniquement (upload / Sync Agent Pro) | partiel (coach, comparaisons) | SaaS freemium |
| **lmu-telemetry-analyzer** (mzluzifer) | oui | DuckDB natif + `.ld` de référence | **oui** (« Setup & Pace », hints pression/carrossage/balance) | **MIT**, Node + HTML |
| **Track Titan** | oui (plugin `TrackTitanRf2Plugin.dll`, 21 juin 2026) | plugin DLL maison | coaching IA, pas de reco setup documentée | SaaS |
| **RaceLab** | oui (`RacelabLMUPlugin.dll`) | plugin DLL maison | non | freemium |
| **Garage 61** | **non** (« supports iRacing only ») | — | — | — |
| **Trophi.ai** | pas de support LMU trouvé (ACC/iRacing) | — | — | — |
| **Popometer** | pas de support LMU trouvé | — | — | — |
| **DR Sim Manager** (motion) | oui | plugin rF2 SMMP (installeur intégré) | non | — |

Licences des briques réutilisables [VÉRIFIÉ] :
- Header S397 `SharedMemoryInterface.hpp`/`InternalsPlugin.hpp` : livré avec le jeu, **aucune licence explicite publiée** ; les structs sont des faits d'interface (ports MIT existants : `pyLMUSharedMemory` © 2021 Tony Whitley / 2025 Xiang, `goLMUSharedMemory`, `lmu-pitwall`). Recommandation : écrire nos propres définitions à partir du header + tests de taille (`TelemInfoV01` = 1888, `VehicleScoringInfoV01` = 584, `ScoringInfoV01` = 548, `TelemWheelV01` = 260, total 324 820).
- `rF2SharedMemoryMapPlugin` (IronWolf) : **GPL-3** ; `rF2State.h` est GPL, le README autorise la redistribution gratuite de la DLL avec le README. `pyRfactor2SharedMemory` (Whitley) : MIT.
- TinyPedal : GPL-3 (images CC BY-SA 4.0) ; winecarte : GPL-3 ; lmu-pitwall, lmu-telemetry-analyzer, NikMusy/le-mans-ultimate-telemetry : MIT ; DuckDB : MIT.

---

## 7. À vérifier à la main sur le PC du joueur

1. Version LMU exacte (`generic.gameVersion` + écran titre) et présence de `Support\SharedMemoryInterface\SharedMemoryInterface.hpp` ; copier le header (c'est notre spec) et vérifier `sizeof` des structs contre 1888/584/548/260/324 820.
2. `Settings > Gameplay > Enable Plugins` OFF → ON : confirmer que `LMU_Data`, `LMU_Data_Event`, `LMU_SharedMemoryLockData/Event` apparaissent (Sysinternals WinObj / handle) et qu'aucune DLL n'est nécessaire.
3. Mesurer les fréquences réelles (telemetry/scoring/event) au garage, en piste, en replay, en spectateur, en multijoueur ; vérifier la cohérence `mNumVehicles` vs `activeVehicles` et le taux de lectures déchirées sans verrou.
4. Valider en roulant que `mTC`, `mABS`, `mMotorMap`, `mMigration`, `mFront/RearAntiSway`, `mRegen`, `mStateOfCharge`, `mVirtualEnergy` (unité 0–1 ou %), `mTimeGap*`, `mTrackLimitsSteps` bougent live ; relever les chaînes `mVehicleClass` du scoring sur une grille multi-classes et l'enum uint8 côté télémétrie.
5. Vérifier si les `TelemInfoV01` des adversaires sont remplis en ligne (usure, VE, composés) ou seulement en solo.
6. Dossier `Plugins\` sur install vierge : la DLL IronWolf est-elle présente ? Existe-t-il un `Bin64\` ? Contenu initial de `CustomPluginVariables.JSON`.
7. REST : ouvrir `http://127.0.0.1:6397/swagger-schema.json`, archiver le spec et un échantillon JSON de chaque endpoint GET (garage, standings, weather, TireManagement, pitstop-estimate avec un arrêt configuré) ; tester `localhost` vs `127.0.0.1` vs `[::1]` ; mesurer l'effet du polling de `RepairAndRefuel` sur le MFD ; lire la valeur « WebUI port » de `Settings.JSON`.
8. REST écriture : lister les verbes non-GET du Swagger ; tester **hors ligne uniquement** un `PUT/POST` de setup minimal et observer effets (garage, MFD, stabilité). Décider ensuite.
9. Setups : confirmer le chemin `UserData\player\Settings\<Circuit>\`, le nom exact des dossiers circuits, déposer un `.svm` généré par nous et vérifier qu'il apparaît après Refresh et se charge ; vérifier l'encodage/CRLF des fichiers écrits par le jeu ; vérifier la correspondance index ↔ `min/maxValue` REST.
10. DuckDB : enregistrer un stint, dumper `channelsList`/`eventsList`/`metadata`, noter les fréquences et unités réelles, vérifier la présence de `CarSetup`, et la convention de nommage des fichiers (changée en 1.2).
11. Résultats XML : vérifier `UserData\Log\Results\` et le contenu en solo/en ligne ; replays : localiser `UserData\Replays\*.Vcr`.
12. Linux (machine de test) : LMU + GE-Proton récent, EAC en ligne ; winecarte → `/dev/shm/LMU_Data` (taille, fraîcheur, CPU) ; REST joignable depuis l'hôte ; chemins Steam.
13. EAC : confirmer qu'un process externe lisant la SHM/REST et qu'une DLL tierce dans `Plugins\` n'entraînent ni kick ni avertissement en session en ligne avec la version courante.

---

## 8. Décisions d'architecture que cela impose

1. **Source primaire = mémoire partagée native `LMU_Data`** (lecture seule, mapping `FILE_MAP_READ`), avec nos propres structs générées depuis le header S397 et des `static_assert` de taille ; **plugin IronWolf uniquement en mode repli/diagnostic** (et pour comparer), jamais comme chemin principal.
2. **Lecteur SHM dans un thread temps réel** : attendre `LMU_Data_Event` (Windows) ou poller à 8 ms (Linux) ; copier sous verrou S397 le strict nécessaire (joueur + scoring) ou faire copie + contrôle `mNumVehicles == activeVehicles` + relecture de `mElapsedTime`/`mCurrentET` ; ne jamais tenir le verrou plus de quelques µs ; détecter la sortie du jeu via `appInfo.mAppWindow`/process car les handles ouverts gardent le mapping vivant.
3. **Couche d'abstraction versionnée** (« LMU 1.2 / 1.3 / 1.4+ ») : les champs aides/énergie n'existent qu'à partir de 1.3 ; lire `gameVersion` et dégrader proprement ; prévoir un test automatique de layout à chaque mise à jour LMU.
4. **Client REST GET-only en V1**, hôte forcé `127.0.0.1` avec repli résolution d'hôte, port lu dans `Settings.JSON`, découverte par `/swagger-schema.json`, cadence ≥ 200 ms avec backoff sur hash inchangé, `RepairAndRefuel` ≥ 500 ms ; toute écriture (setup, pit menu) derrière un flag expérimental, hors ligne, après validation point 8.
5. **Modèle de setup = `{clé, index, libellé affiché, min, max}`**, alimenté par trois sources concordantes (`.svm`, `getPlayerGarageData`, `metadata.CarSetup` DuckDB) ; recommandations exprimées en **crans relatifs** ; écriture de setup = génération d'un `.svm` complet (copie du setup courant + deltas) déposé dans `Settings\<Circuit>\`, jamais d'édition partielle ni d'écriture pendant que le jeu écrit.
6. **Pipeline hors-ligne basé sur DuckDB natif** (tour complet + setup + météo dans un même fichier) pour l'analyse et l'entraînement des recommandations ; l'enregistrement live SHM 100 Hz sert au temps réel et au mobile, pas à remplacer DuckDB.
7. **Linux** : l'appli native lit `/dev/shm/LMU_Data` (même format, même taille) ; fournir/recommander un pont dans le préfixe (winecarte GPL-3 comme prérequis externe, ou notre propre mini-exe Win32 sous licence compatible) ; pas de dépendance aux événements/verrous Win32 ; détection du jeu par `/proc` ; mobile = même protocole que Windows via le PC.
8. **Mobile** : le PC fait tout l'accès données ; exposer un flux WebSocket LAN déjà décimé (ex. 20–50 Hz télémétrie joueur, 5 Hz classement, REST agrégé 1 Hz), modèle lmu-pitwall/irdashies ; pas d'accès direct SHM/REST depuis le téléphone.
9. **Licences** : code original uniquement pour les structs et parseurs ; ne lire TinyPedal/IronWolf que comme documentation ; réutiliser librement les ports MIT (`pyLMUSharedMemory`, lmu-pitwall) si besoin d'un prototype.
10. **Robustesse multi-outils** : coexistence avec SimHub/Crew Chief/TinyPedal (tous lecteurs de la même SHM et du même REST) → aucune écriture dans `CustomPluginVariables.JSON` sans sauvegarde, aucune modification des fichiers `/dev/shm` partagés, limitation du polling REST pour ne pas aggraver le clignotement MFD.
