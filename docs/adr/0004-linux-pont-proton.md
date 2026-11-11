# ADR 0004 — Linux : lecture de `/dev/shm/LMU_Data` via un pont dans le préfixe Proton

- **Statut** : acceptée — 2026-11-02
- **Source** : `docs/research/01-telemetrie-lmu.md` §4, `docs/research/03-stack-technique.md` §0

## Contexte

Le Mans Ultimate tourne sous Linux via Proton (GE-Proton conseillé ; le jeu est officiellement « unsupported »
et GamingOnLinux le classe « Broken » à cause d'EAC, alors que des témoignages 2026 rapportent des sessions
en ligne fonctionnelles). Problème structurel : Wine implémente les mappings mémoire nommés
(`CreateFileMapping` pagefile-backed) par des objets gérés par wineserver, **pas** par des fichiers
`/dev/shm/<nom>` (bug Wine 54015). `LMU_Data` est donc invisible à un processus Linux natif.

Trois ponts existent : **winecarte** (`wine2linux.exe` lancé dans le préfixe, recopie périodique des mappings
vers `/dev/shm/<nom>`, GPL-3, maintenu en 2026, gère `LMU_Data` et les buffers rF2) ; le **fork Wine du plugin
rF2** (écrit directement dans `/dev/shm/$rFactor2SMMP_*$`, layout legacy sans les champs 1.3, non maintenu) ;
un relais UDP maison dans le préfixe. TinyPedal lit simplement `/dev/shm/<nom Win32>` par `mmap`.

Linux est la plateforme de développement de l'équipe (le CI Windows produit les binaires du client) et une
cible « utilisateur avancé » ; ce n'est pas la plateforme du client.

## Décision

1. Sous Linux, `corde-lmu` ouvre **`/dev/shm/LMU_Data`** en lecture seule par `mmap` (`memmap2`), même
   format et même taille (324 820 octets) que sous Windows : la différence entre OS tient à l'ouverture du
   handle, pas au parsing.
2. **Pas de dépendance aux événements ni aux verrous Win32** : polling 8–16 ms + copie + contrôle de cohérence
   (`mNumVehicles == activeVehicles`) + relecture témoin de `mElapsedTime`.
3. Le pont est un **prérequis externe recommandé : winecarte** (`winecarte-run %command%` dans les options de
   lancement Steam). Corde ne l'embarque pas (GPL-3, ADR 0005) et ne copie pas son code. Un mini-exécutable
   Win32 Corde sous licence compatible pourra le remplacer plus tard si le besoin est confirmé.
4. L'API REST `127.0.0.1:6397` est supposée joignable depuis l'hôte (le serveur du jeu écoute sur la pile
   réseau de l'hôte via Wine) — **à confirmer** sur une machine de test.
5. Chemins de fichiers : lire `libraryfolders.vdf` de Steam ; les chemins `C:\…` exposés dans
   `LMU_Data.paths.*` sont convertis via `<prefix>/drive_c`.
6. Le simulateur `corde-sim` écrit lui aussi un fichier `/dev/shm/LMU_Data` (ou un mapping nommé sous
   Windows) pour développer sans le jeu ni le pont.

## Conséquences

- Développement et tests possibles entièrement sous Linux (simulateur + fixtures), CI Linux + Windows.
- Un utilisateur Linux doit installer et lancer winecarte : à documenter, avec détection « fichier absent »
  et message explicite dans `SourceStatus.last_error`.
- Ne jamais combiner winecarte et le fork Wine du plugin (mêmes fichiers `/dev/shm`).
- Steam Deck : non ciblé (aucune source de bon fonctionnement en 2026).
- Les mêmes contrôles de cohérence servent sous Windows en mode « copie » : un seul chemin de validation.

## Références

- Wine bug 54015 (mappings nommés et `/dev/shm`) :
  https://list.winehq.org/archives/list/wine-bugs@list.winehq.org/thread/J47UZL64XJ27JOOD3M3R7NKXVS3REXLX
- winecarte : https://github.com/srounce/winecarte
- Fork Wine du plugin rF2 : https://github.com/schlegp/rF2SharedMemoryMapPlugin_Wine
- Lecture `/dev/shm` dans TinyPedal : https://github.com/TinyPedal/pyLMUSharedMemory (`_common.py`)
- Statut EAC : https://www.gamingonlinux.com/anticheat/status/broken/ — guide Steam « How to Play LMU on a
  Linux System » (id 3536726840)
- Autres ponts : https://github.com/Spacefreak18/wine-linux-shm-adapter ,
  https://forum.reizastudios.com/threads/ams-2-shared-memory-on-linux.36396/
