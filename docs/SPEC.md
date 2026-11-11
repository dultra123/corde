# Corde — Cahier des charges

> Version 0.1 — 3 novembre 2026. Document fonctionnel de référence : ce que Corde doit faire, pour qui, et
> comment on saura que c'est fait. L'architecture technique est dans [ARCHITECTURE.md](ARCHITECTURE.md),
> les décisions datées dans [adr/](adr/), les connaissances « ingénieur » dans [`knowledge/`](../knowledge/README.md).
>
> Les sections 1 à 3, 7 et 9 à 11 s'adressent d'abord au client (le pilote propriétaire du projet) ; les sections 4 à 6 et 8
> précisent ce que les développeurs doivent livrer et vérifier. Les termes techniques sont expliqués au premier emploi.

---

## 1. Vision et promesse

**Corde est l'ingénieur de course personnel du pilote de simulation.** Elle tourne sur le PC de jeu, lit toute la
télémétrie de Le Mans Ultimate (LMU) — puis d'autres simulateurs —, l'affiche en direct sur un tableau de bord
composé de widgets que l'on déplace, ajoute ou enlève (sur PC, sur téléphone, dans OBS), enregistre chaque
session, et, après la course, analyse les données pour proposer un réglage de voiture adapté **à ce circuit et à
cette façon de conduire**.

La promesse, dans les mots du client :

- récupérer **toute** la télémétrie du jeu ;
- un **dashboard** (carte, températures de pneus, temps au tour…) en widgets libres, identique sur mobile ;
- **enregistrer** les données et, en fin de course, les **compiler et analyser** ;
- proposer un réglage « optimal » qui **évolue avec les progrès du pilote**, décliné en **trois versions**
  (pluie, plus agressif, plus soft) ;
- le pilote en teste un, **donne son ressenti**, l'application **ajuste finement** en analysant aussi la
  nouvelle course ; tout cela **par circuit** ;
- un **profil** qui rassemble toutes les données de toutes les courses.

Ce que Corde n'est pas : un coach de pilotage en temps réel, un service cloud, une boutique de setups.

Principes non négociables (voir ARCHITECTURE.md §1) : local-first (rien ne quitte le PC sans action
explicite), un seul modèle de données pour tous les jeux, Rust pour la donnée et web pour l'affichage.

---

## 2. Personas

### 2.1 Le streamer-pilote (persona principal)

Streame ses courses LMU sur Twitch, ne code pas, délègue la technique. Il veut :
un tableau de bord lisible sur un second écran ou un téléphone posé sur le cockpit, sans configuration ; une
source navigateur OBS pour montrer certaines données à ses spectateurs ; après la course, comprendre « pourquoi
la voiture glisse » et obtenir trois setups prêts à essayer ; retrouver toutes ses courses au même endroit.
Contrainte forte : **l'installation et l'usage doivent tenir en quelques clics** ; toute erreur doit être
expliquée en français clair.

### 2.2 Le spectateur

Regarde le stream (ou un écran partagé en LAN). Il ne manipule rien. Il voit, via l'overlay OBS, des widgets
choisis par le streamer : écarts, pneus, carburant/énergie, position. Contrainte : lisibilité à petite taille,
fond transparent, aucune information que le streamer ne veut pas montrer (adresse IP, réglages).

### 2.3 Le pilote de ligue

Court en championnat (multi-classes, relais longs, pluie). Il veut comparer ses relais, surveiller
dégradation et consommation, préparer un setup par circuit avant la course, et affiner avec ses propres
retours. Il accepte une interface plus dense et sait ce qu'est un « cran de barre anti-roulis ». C'est lui qui
exigera des recommandations explicables et prudentes (jamais un setup qui casse la voiture).

---

## 3. Parcours utilisateur

Chaque parcours décrit l'expérience cible ; la phase où il devient possible est indiquée entre crochets
(voir §4).

### 3.1 Installer [phase 0 → 1]

1. Dans LMU : `Settings > Gameplay > Enable Plugins` activé, puis redémarrage complet du jeu (c'est le jeu
   lui-même qui publie alors sa télémétrie ; aucun fichier à copier dans le jeu — brief 01 §1.2).
2. Télécharger Corde (archive `.zip` en phase 0, installeur Windows en phase 1), le lancer.
3. Au premier lancement, Corde affiche : « En attente de Le Mans Ultimate » puis « Jeu détecté ».
4. Le pare-feu Windows demande une autorisation (réseau privé) : nécessaire pour le téléphone.

Guide pas à pas pour le client : [TESTING-ON-PC.md](TESTING-ON-PC.md).

### 3.2 Première session [phase 0]

Le pilote lance une session d'essais. Corde ouvre le dashboard par défaut dans le navigateur (phase 0) ou dans
sa fenêtre (phase 1). Dès l'entrée en piste, les widgets se remplissent. Aucun réglage n'est demandé.

### 3.3 Utiliser le dashboard [phase 0]

- Bouton **Modifier** : la grille devient éditable, on déplace et redimensionne les widgets, on en ajoute
  depuis un catalogue, on en retire. **Terminer** enregistre la disposition côté serveur (elle est donc
  identique depuis n'importe quel navigateur du réseau).
- Plusieurs **dispositions** nommées (ex. « Course », « Essais », « OBS »), par type d'appareil (bureau 12
  colonnes, téléphone 4 colonnes, overlay).
- Réglages : adresse du serveur (téléphone), cadence d'affichage, langue, mode démo (course fictive pour
  tester sans le jeu).

### 3.4 Après la course [phase 1 → 2]

À la fin de la session, Corde a enregistré tours, secteurs, événements (arrêts, drapeaux, impacts) et
échantillons bruts. L'écran « Session » montre : tableau des tours (temps, secteurs, validité, carburant et
énergie consommés, pneus), courbes par tour (vitesse, pédales, températures), carte avec trace. En phase 2,
l'onglet « Ingénieur » affiche les diagnostics (« blocages avant répétés au freinage de T1 », « pneus arrière
hors fenêtre en fin de relais ») avec leur niveau de confiance.

### 3.5 Cycle setup → ressenti → ajustement [phase 2]

1. Corde lit le setup réellement utilisé (fichier `.svm` ou API du jeu) et l'associe à la session.
2. Il propose **trois setups** dérivés : *Pluie*, *Agressif*, *Soft/sûr*, chacun sous forme de **liste de
   changements relatifs** (« barre arrière −1 cran, pression avant −3 kPa ») avec la raison et la confiance.
3. Le pilote choisit « Générer le fichier » : Corde écrit un `.svm` complet dans le dossier du circuit ; dans
   le jeu, bouton **Refresh** de l'écran setups, puis chargement (brief 01 §3).
4. Il roule, puis saisit son **ressenti** en texte libre (« ça pousse à l'entrée des rapides et l'arrière part
   à la remise des gaz ») ou via un formulaire (essieu × phase × vitesse de virage). Corde pose au besoin une
   ou deux questions de clarification.
5. Corde croise le ressenti et la télémétrie de la nouvelle session, pondère (voir `knowledge/methodology.md`
   §4) et propose une **itération** : au plus 3 paramètres, avec les effets secondaires attendus.
6. Chaque itération est historisée (setup parent → enfant) ; on peut revenir en arrière.

### 3.6 Nouveau circuit [phase 2]

Sans historique sur ce circuit, Corde part du setup de base (par défaut du jeu ou dernier setup validé du
pilote sur un circuit de même typologie — rapide, mixte, lent, bosselé), applique le « paquet » de la
typologie (`knowledge/methodology.md` §5) et les préférences déjà apprises du pilote (ex. « préfère toujours
−0,5 % de répartition »), puis demande 3 à 5 tours de contrôle avant toute recommandation d'équilibre.

### 3.7 Profil [phase 2]

Toutes les sessions, par jeu / circuit / voiture : meilleurs tours, régularité, dégradation, consommation,
indicateurs de style (agressivité au freinage, trail braking, lissage du volant, usage des vibreurs),
historique des setups et des ressentis. Voir §7.

---

## 4. Fonctionnalités par phase et critères d'acceptation

Les phases reprennent la feuille de route de ARCHITECTURE.md §6. Un critère d'acceptation est une
vérification que n'importe qui peut refaire ; « CI » désigne les tests automatiques exécutés sur GitHub à
chaque modification du code.

### Phase 0 — Lire, diffuser, afficher

| Fonction | Critères d'acceptation |
|---|---|
| **Protocole** `corde-protocol` : `TelemetryFrame` normalisée, types TypeScript générés | `cargo test -p corde-protocol` passe et régénère `packages/protocol/src/generated/*.ts` ; `pnpm -r typecheck` passe sur les types générés |
| **Adaptateur LMU** `corde-lmu` : lecture de la mémoire partagée native `LMU_Data` (Windows), de `/dev/shm/LMU_Data` (Linux) | Tests de taille/offset des structures contre `reference/lmu-shared-memory-layout.txt` (bloc de 324 820 octets, brief 01 §1.2) ; `lmu-probe` sur le PC du client affiche piste, voiture, vitesse et écrit une trame complète avec `--once --dump frame.json` |
| **Simulateur** `corde-sim` : faux `LMU_Data` réaliste | `corde-server --source sim` produit des frames continues (tours qui s'enchaînent, 20+ voitures) sans le jeu |
| **Serveur** `corde-server` : HTTP `127.0.0.1:8471`, WebSocket `/ws`, cadence par client (1–60 Hz), statut source | Un navigateur reçoit `hello` puis des `frame` à la cadence demandée ; déconnexion du jeu → `status.connected=false` sans crash ; deux clients simultanés avec cadences différentes |
| **Dashboard v0** : grille éditable, catalogue de widgets, dispositions persistées côté serveur | Les 10 widgets v0 (§5) affichent des données du simulateur ; une disposition modifiée est retrouvée après rechargement et depuis un autre navigateur ; utilisable à 390 px de large (téléphone) |
| **Mode démo** | `?demo=1` ou réglage : course fictive sans serveur |
| **Langue** | UI en français par défaut, anglais disponible, aucune chaîne codée en dur hors `i18n/` |
| **Build Windows** | L'artefact `corde-windows` de la CI contient `corde-server.exe`, `lmu-probe.exe`, `corde-sim.exe` et `ui/` et se lance sur le PC du client |

### Phase 1 — Application de bureau et enregistrement

| Fonction | Critères d'acceptation |
|---|---|
| **CI Windows + Linux** complète, Release GitHub sur tag | Un tag `v0.x` produit un zip Windows attaché à une Release |
| **Coquille Tauri desktop** (Windows, Linux) | Installeur Windows (NSIS/MSI) ; au lancement, le serveur démarre et le dashboard s'ouvre ; icône de zone de notification avec état du jeu |
| **Enregistrement** : SQLite (`sessions`, `laps`, `events`, `setups`) + Parquet zstd (échantillons) dans `%LOCALAPPDATA%\Corde\` / `~/.local/share/corde/` | Une session de 30 min est enregistrée sans perte de tours ; redémarrage de Corde en cours de session → nouvelle session, pas de corruption ; taille sur disque mesurée et affichée |
| **Écran Session** : tours, secteurs, courbes par tour, carte avec trace | Deux tours superposables par distance ; export CSV d'un tour |
| **Import des résultats XML** `UserData\Log\Results\*.xml` | Une course terminée hors Corde apparaît dans l'historique avec positions et tours (brief 01 §5) |
| **Lecture du setup courant** (`.svm` + REST `getPlayerGarageData`) | Le setup est attaché à chaque session ; diff lisible entre deux setups au cran près |
| **Source OBS** (fait en 0.5) | Une disposition « overlay » (grille libre 48 × 27 sur 1920 × 1080, fond transparent) s'affiche dans une source navigateur OBS à `http://127.0.0.1:8471/#/overlay/layout/<id>`, un widget seul à `#/overlay/widget/<id>` ; ni barre, ni bandeau, ni curseur, ni défilement ; options `bg`, `opacity`, `scale`, `hide=offtrack`, `lang` dans l'adresse ; page « OBS » de Corde avec aperçus, adresses à copier et tailles conseillées ; reconnexion silencieuse ; jamais d'adresse IP, de chemin ni de réglage de voiture à l'écran |

### Phase 2 — L'ingénieur

| Fonction | Critères d'acceptation |
|---|---|
| **Analyse post-session** : prétraitement (tours propres, virages, phases), diagnostics de `knowledge/diagnostics.yaml` marqués phase ≤ 2 | Sur une session enregistrée, chaque diagnostic produit ses `outputs` avec un niveau de confiance ; rejouable (même session → même résultat) |
| **Trois setups proposés** : Pluie, Agressif, Soft/sûr | Pour une session + un setup de référence, Corde génère trois listes de changements relatifs, chacune avec règle, raison, confiance, effets secondaires ; aucun changement hors des bornes `minValue`/`maxValue` lues dans le jeu |
| **Écriture `.svm`** | Le fichier généré apparaît dans le jeu après Refresh et se charge (vérifié sur le PC du client) ; le fichier source n'est jamais modifié |
| **Ressenti** : formulaire structuré + texte libre (dictionnaire `feedback-dictionary.yaml`, LLM optionnel) | « ça pousse à l'entrée des lents » → `FEEL_FRONT_WONT_TURN_IN` → question de clarification → symptôme ; fonctionne sans réseau via le formulaire ; le LLM n'émet jamais de valeur de réglage |
| **Itération** | Au plus 3 paramètres par itération (règle ARB-03) ; conflits détectés et expliqués ; historique parent/enfant des setups |
| **Profil pilote** (§7) | Indicateurs de style S-01…S-08 calculés par session et agrégés ; préférences apprises affichées et modifiables |

### Phase 3 — Mobile

| Fonction | Critères d'acceptation |
|---|---|
| **Application iOS / Android** (Tauri 2) avec les mêmes widgets | Le dashboard s'affiche à 20 Hz sur un téléphone en Wi-Fi ; même catalogue de widgets ; dispositions « mobile » 4 colonnes |
| **Découverte du PC** (mDNS) + **appairage** (code à 6 chiffres, QR) | Sans saisir d'adresse IP sur un réseau domestique ; repli par QR / saisie manuelle |
| **Lecture seule par défaut** | Un téléphone non appairé ne peut pas modifier les dispositions ni lancer un enregistrement |

### Phase 4 — Autres simulateurs

| Fonction | Critères d'acceptation |
|---|---|
| Adaptateurs ACC, AC EVO, iRacing (Windows seulement), rFactor 2, AMS2 | Chaque adaptateur implémente `TelemetrySource`, livre ses fixtures et ses tests de layout ; les widgets fonctionnent sans modification ; les champs non fournis sont `None`/`Unknown`, jamais inventés |

---

## 5. Widgets

Un widget est une tuile du dashboard. Chaque widget a un `kind` (identifiant), des réglages propres et
s'adapte à sa taille (petit = chiffres clés, grand = graphiques).

### 5.1 Widgets v0 (phase 0)

| `kind` | Nom | Contenu | Champs du protocole |
|---|---|---|---|
| `tyres` | Pneus | 4 pneus : températures gauche/centre/droite (couleur selon fenêtre), pression, usure, composé, carcasse | `player.wheels[].temp_c`, `pressure_kpa`, `wear_remaining`, `carcass_c`, `optimal_temp_c`, `player.tyres` |
| `brakes` | Freins | Température par roue, pression, répartition, migration ; activité ABS | `wheels[].brake_temp_c`, `brake_pressure`, `electronics.brake_bias_rear`, `brake_migration`, `abs_active` |
| `inputs` | Commandes | Traces défilantes accélérateur / frein / embrayage / volant, filtré vs brut, rapport, régime | `player.inputs.*`, `engine.gear`, `engine.rpm` |
| `laps` | Tours | Tour courant, dernier, meilleur, secteurs, delta au meilleur, tour invalidé | `player.lap.*`, `ServerMessage::Lap` |
| `fuel` | Carburant / énergie | Litres, énergie virtuelle, état de charge batterie, régénération, consommation par tour et tours restants estimés, arrêts prévus | `player.energy.*`, `LapRecord.fuel_used_l`, `virtual_energy_used` |
| `map` | Carte | Tracé appris du premier tour complet, position du joueur et des autres voitures (couleur par classe), secteurs | `player.motion.pos`, `lap.lap_fraction`, `standings[].pos`, `lap_fraction`, `vehicle_class` |
| `session` | Session / météo | Type et phase de session, temps restant, drapeaux, températures air/piste, pluie, humidité de la trajectoire, nuages, niveau de gomme, heure | `session.*`, `session.weather.*` |
| `standings` | Classement | Positions générales et par classe, pilote, voiture, dernier/meilleur tour, écart au suivant, arrêts, pénalités, drapeau bleu, aux stands | `standings[]` |
| `gaps` | Écarts | Voiture devant / derrière (sur la piste et au classement), tendance sur les derniers tours | `player.gaps.*`, `standings[].time_behind_next_s` |
| `electronics` | Électronique | TC (3 valeurs), ABS, map moteur, migration, barres anti-roulis réglables, limiteur, essuie-glace, phares, lift-and-coast | `player.electronics.*`, `engine.speed_limiter` |

Règles communes : unités humaines (°C, km/h, kPa, L) ; les valeurs absentes (`None`) s'affichent « — » et
jamais 0 ; roues toujours dans l'ordre AVG, AVD, ARG, ARD ; couleurs de fenêtre thermique paramétrables (les
seuils par défaut viennent de `knowledge/setup-parameters.yaml` → `thermal_windows`, confiance basse, donc
modifiables).

### 5.2 À venir

| Phase | Widget | Contenu |
|---|---|---|
| 1 | `damage` | Dégâts (8 zones), dernier impact, pièces détachées |
| 1 | `aero` | Hauteurs de caisse, 3e élément, appuis, traînée, volets |
| 1 | `suspension` | Course et vitesse d'amortisseur par roue, histogramme en direct |
| 1 | `g-meter` | Accéléromètre latéral/longitudinal |
| 1 | `track-limits` | Points track-limits, avertissements |
| 1 | `relative` | Classement relatif (voitures autour de soi, comme un spotter) |
| 1 | `stint` | Relais : tours effectués, dégradation, prévision d'arrêt |
| 1 | `rest-pit` | Données REST : usure freins, dégâts, estimation d'arrêt (sous réserve du bug MFD, brief 01 §2) |
| 2 | `engineer` | Résumé des diagnostics en direct (ex. « pneus froids », « freins hors fenêtre ») |
| 2 | `delta-trace` | Delta au meilleur tour tracé par distance |
| 2 | `weather-forecast` | Prévisions météo de la session (REST) |

---

## 6. Fonctionnement de « l'ingénieur » (phase 2)

### 6.1 Sources de données

| Source | Ce qu'elle apporte | Référence |
|---|---|---|
| Mémoire partagée native `LMU_Data` (≈ 100 Hz) enregistrée par Corde | Tout l'état dynamique : roues, pédales, aéro, électronique, énergie, position, classement | brief 01 §1.2 |
| API REST locale `http://127.0.0.1:6397` (lecture seule) | Setup courant avec bornes (`getPlayerGarageData`), température optimale des pneus, pneus autorisés, usure freins/dégâts, énergie par tour, prévisions météo | brief 01 §2 |
| Fichiers `.svm` (`UserData\player\Settings\<Circuit>\`) | Setup complet lisible/écrivable ; c'est le support des propositions | brief 01 §3 |
| Fichiers DuckDB natifs (`UserData\Telemetry\*.duckdb`, si « Automatically Record Telemetry » est activé) | Tour complet + setup + météo dans un même fichier : source hors ligne idéale pour relier un tour à un setup | brief 01 §5 |

Le modèle de setup interne est `{clé, index, libellé affiché, min, max}` alimenté par ces trois sources
concordantes (brief 01 §8.5) ; les recommandations sont exprimées en crans relatifs.

### 6.2 Diagnostics

`knowledge/diagnostics.yaml` décrit 45 diagnostics (pneus, freins, équilibre par phase de virage et vitesse de
virage, plateforme/talonnage, amortisseurs, motricité, stratégie, style de pilotage, conditions). Chacun
nomme ses champs d'entrée dans le protocole, sa méthode, ses seuils indicatifs et sa confiance (méthode et
seuils séparés). Préalable à tout : ne travailler que sur des **tours propres** (pas in/out-lap, pas invalidé,
pas sous drapeau jaune, pas de sortie de piste) et sur **plusieurs tours** (jamais une recommandation sur un
seul tour — `methodology.md` §1).

### 6.3 Trois philosophies

Les trois setups proposés dérivent d'une même base (setup courant du pilote, ou setup par défaut/CDA du jeu,
ou dernier setup validé sur un circuit de même typologie) :

- **Pluie** = base + paquet `WET-PACK-01` (+ volet « sûr » allégé : TC, répartition) ;
- **Agressif** = base + `PHI-AGGRESSIVE-01` (vif, orienté temps au tour, usure acceptée) ;
- **Soft / sûr** = base + `PHI-SAFE-01` (stable, prévisible, relais longs).

Puis ajustement par le profil de style s'il existe (`methodology.md` §6), et vérification des invariants
(aileron ≥ minimum BoP, pressions ≥ minimum, hauteurs ≥ minimum, bornes lues dans le jeu).

### 6.4 Ressenti en langage naturel

Deux chemins, toujours disponibles :

1. **Formulaire** (hors ligne, sans ambiguïté) : essieu (avant/arrière) × phase (freinage, entrée, milieu,
   sortie) × vitesse de virage (lent/moyen/rapide) × gravité.
2. **Texte libre** : normalisation puis recherche dans `feedback-dictionary.yaml` (56 expressions FR/EN avec
   questions de clarification). Un modèle de langage (API Claude, clé fournie par l'utilisateur) peut
   **traduire** le texte en identifiants `FEEL_*` et rédiger l'explication finale — il ne choisit jamais un
   paramètre ni une valeur (brief 03 §5). Seul le texte du ressenti quitte le PC, avec accord explicite.

Puis pondération ressenti/données (`methodology.md` §4) : les faits physiques (talonnage, températures,
blocages, batterie) viennent des données ; la pilotabilité vient d'abord du ressenti si le pilote est loin de
la limite ou irrégulier.

### 6.5 Adaptation par circuit et par pilote

- Un setup par couple (circuit, typologie météo), un journal des deltas appliqués et acceptés.
- Typologies de circuits et transformations associées : `methodology.md` §5 (rapide/aéro, mixte, lent,
  bosselé, chaud/froid).
- Apprentissage des « coefficients » du pilote : préférences récurrentes détectées sur plusieurs circuits
  (ex. répartition toujours plus arrière que la base) et réappliquées sur un nouveau circuit, affichées dans le
  profil et désactivables.

### 6.6 Ce que l'ingénieur ne fera pas (V1)

- **Écrire un setup dans le jeu via l'API REST.** Les endpoints d'écriture existent mais sont non officiels,
  réputés fragiles et non utilisés par les projets open source lus (brief 01 §2). Corde **génère un fichier
  `.svm`** que le pilote charge lui-même après Refresh. Toute écriture REST (setup, menu de stand) reste
  derrière un drapeau expérimental, hors ligne, après validation sur le PC du client.
- **Promettre un gain chiffré** (« −0,3 s au tour »). Les seuils de la base de connaissances sont indicatifs
  (confiance moyenne ou basse) ; Corde annonce une direction, une raison et une confiance, puis mesure.
- Modifier le fichier de setup d'origine, écrire pendant que le jeu écrit, ou changer la configuration du jeu
  (`CustomPluginVariables.JSON`, `Settings.JSON`) sans sauvegarde.
- Remplacer le jugement du pilote : une recommandation se refuse en un clic et l'historique le garde.

---

## 7. Profil pilote

Un profil local par pilote (plusieurs profils possibles sur un même PC). Contenu :

| Rubrique | Détail | Phase |
|---|---|---|
| Sessions | Liste de toutes les sessions (jeu, circuit, voiture, type, date, durée, météo), import XML inclus | 1 |
| Performance par circuit / voiture | Meilleur tour et secteurs, médiane des 3 meilleurs, régularité (écart-type), vitesse de pointe | 1 |
| Relais et stratégie | Dégradation (s/tour corrigée du carburant), consommation carburant et énergie virtuelle par tour, tours par relais | 2 |
| Style de pilotage | S-01 agressivité au freinage, S-02 trail braking, S-03 lissage du volant, S-04 usage des vibreurs, S-05 application des gaz, S-06 rétrogradages, S-07 marge au grip, S-08 vitesse minimale (voir `diagnostics.yaml`) | 2 |
| Setups | Tous les setups lus et générés, arbre parent/enfant, ressenti associé, accepté/refusé | 2 |
| Préférences apprises | Coefficients par paramètre (ex. répartition −0,5 % vs base), avec nombre d'observations et bouton « oublier » | 2 |
| Export | CSV / JSON de l'ensemble ; suppression complète en un clic | 1 |

Vie privée : tout est stocké localement (ADR 0006). Le nom de pilote et d'éventuels identifiants Steam ne
sont jamais envoyés.

---

## 8. Contraintes

| Contrainte | Conséquence pour Corde |
|---|---|
| **Easy Anti-Cheat** (LMU ≥ 1.2) | Corde est un processus externe en **lecture seule** (mémoire partagée publiée par le jeu, API REST du jeu, fichiers). Aucune DLL injectée, aucune écriture dans le processus du jeu. Le comportement d'EAC en session en ligne avec un lecteur externe reste **à confirmer sur le PC du client** (brief 01 §7.13) ; on teste d'abord hors ligne. |
| **Licences** : TinyPedal, plugin rF2 Shared Memory Map (The Iron Wolf), winecarte sont GPL-3 ; le header S397 `SharedMemoryInterface.hpp` n'a pas de licence publique | Aucune copie de code GPL. Nos structures sont **réécrites** à partir de la lecture du header livré avec le jeu, avec tests de taille/offset (`reference/lmu-shared-memory-layout.txt`). Les ports MIT (`pyLMUSharedMemory`, lmu-pitwall) servent de documentation. Voir ADR 0005. |
| **Linux** : le jeu tourne sous Proton (non supporté officiellement) et Wine ne rend pas les mappings nommés visibles aux processus natifs | Un **pont dans le préfixe Proton** est nécessaire (winecarte, ou un mini-exécutable Corde plus tard) pour obtenir `/dev/shm/LMU_Data` ; Corde le lit par `mmap` avec relecture témoin. Voir ADR 0004. Linux est une cible de développement dès la phase 0, une cible utilisateur « avancé » ensuite. |
| **Pas de cloud en V1** | Aucun compte, aucun serveur Corde. Le seul appel réseau sortant possible est l'API du modèle de langage pour le ressenti, désactivé par défaut, avec clé fournie par l'utilisateur. |
| **Mises à jour de LMU** | Les structures mémoire et les routes REST changent avec les versions (1.2 → 1.3 → 1.4). Corde lit `gameVersion`, dégrade proprement et dispose de tests de layout à relancer à chaque mise à jour. |
| **Coexistence** avec SimHub, Crew Chief, TinyPedal | Tous lisent la même mémoire et le même REST : Corde ne modifie rien de partagé et limite son polling REST (≥ 200 ms ; `RepairAndRefuel` ≥ 500 ms) pour ne pas aggraver le clignotement connu du menu de stand. |
| **Plugin `rFactor2SharedMemoryMapPlugin64.dll`** déjà présent chez le client | Inoffensif et non nécessaire : Corde ne l'utilise pas (ADR 0003). Il peut rester. |

---

## 9. Questions ouvertes pour le client

1. ~~**Nom définitif.**~~ **Tranché le 11 novembre 2026** : l'application s'appelle **Corde**, d'après le
   point de corde (l'endroit où la trajectoire touche l'intérieur du virage).
2. ~~**Destination.**~~ **Tranché le 11 novembre 2026** : projet **open source** sous licence
   GPL-3.0-or-later, dépôt public `dultra123/corde` ([ADR 0008](adr/0008-open-source-publication.md)). Restent à
   décider : la signature des exécutables (ADR 0007) et les mises à jour automatiques.
3. **Langue de l'interface.** Français par défaut ; faut-il l'anglais dès le début (spectateurs, ligue) ?
4. ~~**OBS.**~~ **Tranché le 12 novembre 2026 (0.5)** : **fond transparent**, n'importe quel widget seul ou
   une disposition overlay entière, choisis par le streamer dans la page « OBS » ; trois fonds au choix
   (transparent avec contour, verre fumé par défaut, opaque), taille des textes réglable, taille de source
   conseillée par widget (1920 × 1080 pour une disposition). Un habillage aux couleurs d'un overlay existant
   reste possible plus tard (CSS personnalisé de la source).
5. **Ressenti par IA.** Acceptes-tu de fournir une clé API (coût de l'ordre du centime par ressenti, brief 03
   §5) ou le formulaire suffit-il au départ ?
6. **Linux.** Priorité réelle pour toi (tu joues sous Windows) ou uniquement pour l'équipe ?
7. **Mobile.** iOS implique un compte développeur Apple (99 $/an, brief 03 §6) et des tests sur appareil :
   Android d'abord ?
8. **Classes et circuits.** Quelles voitures (Hypercar, LMP2, LMGT3) et quels circuits roules-tu le plus ?
   On calibre les seuils d'abord sur ceux-là.
9. **Données de test.** Acceptes-tu de nous transmettre quelques sessions enregistrées (anonymisées) et le
   header `SharedMemoryInterface.hpp` de ta version du jeu à chaque mise à jour ?
10. **Overlay en jeu** (fenêtre transparente par-dessus le jeu, comme SimHub) : souhaité, ou le second
    écran / téléphone / OBS suffisent ?

---

## 10. Hors de portée

- Coaching de pilotage en temps réel (voix, points de freinage), comparaison avec des tours de pilotes
  professionnels, marketplace de setups.
- Écriture dans le jeu via l'API REST (setup, menu de stand) en V1 — voir §6.6.
- Replays `.Vcr` (format propriétaire sans parseur public), export MoTeC `.ld` (possible plus tard via un
  crate dédié, brief 03 §4).
- Steam Deck, VR (overlay dans le casque), serveur dédié, mode multi-utilisateurs / équipe, synchronisation
  cloud, compte en ligne.
- Autres simulateurs avant la phase 4 ; iRacing sous Linux (SDK Windows uniquement).
- Télémétrie complète des adversaires en ligne au-delà de ce que le jeu expose (usure des autres voitures
  « probablement non envoyée », brief 01 §1.2).
- Pilotage automatique des réglages en roulant (TC, map) : Corde conseille, le pilote agit au volant.

---

## 11. Glossaire rapide

- **Télémétrie** : l'ensemble des mesures que le jeu publie 100 fois par seconde (vitesse, pédales, pneus…).
- **Mémoire partagée** : zone de mémoire où le jeu écrit ces mesures et que d'autres programmes peuvent lire.
- **Widget** : tuile du tableau de bord ; **disposition** : arrangement de widgets enregistré.
- **Setup** / **`.svm`** : réglages de la voiture ; fichier texte où le jeu les enregistre.
- **Cran** : une position d'un réglage (le jeu stocke un index, pas une valeur physique).
- **Diagnostic** : indicateur calculé à partir des données (ex. « blocage avant »).
- **Symptôme** : comportement de la voiture (ex. sous-virage en entrée de virage lent) qui déclenche des règles.
- **Philosophie** : orientation d'un setup (pluie, agressif, soft/sûr).
- **Local-first** : tout fonctionne et reste sur ton PC.
