# ADR 0020 — Plusieurs jeux, détection automatique ; Assetto Corsa Competizione en premier

- **Statut** : **décidée** — 2026-11-17 (demande du propriétaire, 16 nov. : que la télémétrie soit récupérée
  « sur tous les jeux de course de manière automatisée » — on se connecte à LMU, ça récupère ; à une course
  en ACC, ça récupère).
- **Source** : `crates/corde-core/src/auto.rs`, `crates/corde-acc/`, `crates/corde-server/src/runner.rs`,
  `packages/ui/src/lib/games.ts`, `docs/JEUX.md`.

## Contexte

Corde a été écrite pour Le Mans Ultimate, mais deux choix du début la préparaient à d'autres jeux : une
interface commune de source (`TelemetrySource` : `connect`, `poll`) et une trame normalisée
(`TelemetryFrame`) dont le reste de l'application — chronométrage des tours, événements, enregistrement,
interface — se sert sans connaître le jeu. `GameId` nommait déjà cinq autres jeux. Restaient à faire : le
choix du jeu sans le demander au pilote, et un premier adaptateur.

« Tous les jeux » ne se fait pas d'un coup : chaque jeu a son interface (mémoire partagée propre, mémoire
partagée d'un plugin, paquets UDP), ses unités, ses trous. Et aucun adaptateur ne peut être tenu pour juste
tant qu'il n'a pas tourné face au jeu.

## Décision

### A. Corde ne demande pas le jeu : `AutoSource`

`corde_core::AutoSource` contient un adaptateur par jeu, dans un ordre fixe. `connect` les essaie l'un après
l'autre ; le premier qui se branche devient le jeu actif. `poll` lit le jeu actif ; quand il se dit parti, la
source revient à l'examen de tous. La boucle du serveur réessaie comme avant (1 s, puis 2 s).

- **Un adaptateur refuse quand son jeu n'est pas en piste**, pas seulement quand il n'est pas lancé : sinon
  un jeu ouvert dans ses menus masquerait le suivant. Refuser coûte quelques appels système.
- **L'ordre ne compte que si deux jeux tournent ensemble** : le premier de la liste gagne. Le Mans Ultimate
  est premier.
- `--source auto` (défaut, et celui de l'application Windows) cherche tous les jeux ; `--source lmu` et
  `--source acc` en imposent un. Sans jeu, l'état annonce `game: unknown`, `source: auto` : l'interface dit
  « En attente d'un jeu… ».

### B. Un module par jeu, en lecture seule

Chaque jeu a son module (`corde-lmu`, `corde-acc`) qui ne fait que **lire** : mémoire partagée ouverte en
lecture seule, rien d'écrit dans le jeu ni dans ses fichiers, pas de réseau. Chaque module porte un miroir
exact du format, des essais de taille et de position des champs, un faux jeu pour les essais, et un outil de
diagnostic (`lmu-probe`, `acc-probe`) livré dans l'archive Windows.

### C. Ce que le jeu ne publie pas est reconstitué dans l'adaptateur, ou laissé vide

Le reste de Corde ne doit pas apprendre les particularités d'un jeu. Pour Assetto Corsa Competizione :

- **Horloge de session** : absente des pages. L'adaptateur compte les secondes passées session en cours, sur
  une horloge monotone (un saut de plus de 0,5 s est ignoré). Le chronométrage mesure les tours dessus et
  les recoupe avec les temps officiels du jeu.
- **Distance au tour** : le jeu donne une fraction du tour. Longueur prise dans une table des circuits du
  jeu ; pour un circuit inconnu, mesurée sur le premier tour complet (vitesse intégrée entre deux passages
  de ligne).
- **Décompte officiel des tours** : le compteur du jeu peut bouger avant que le temps du tour soit publié.
  `lap_number` suit le compteur ; `laps_completed` attend que le dernier temps ait changé (1 s au plus).
- **Laissé vide plutôt qu'inventé** : temps par secteur, usure des pneus, hauteurs de caisse, détail des
  dégâts, classement des autres voitures (les pages ne donnent que leurs coordonnées : le classement
  complet passe par une autre interface du jeu, pas encore branchée).
- Le premier Assetto Corsa publie des pages du même nom, dans un format plus court : l'adaptateur les
  reconnaît à leur numéro d'interface (« 1.7 » au plus) et les refuse.

### C bis. Le premier Assetto Corsa (1.8.0) : le même module, deux dialectes

Ses pages « physics » et « static » sont le **début** de celles de Competizione ; sa page « graphics » est la
même jusqu'à la position sur le tour, puis **différente** (les coordonnées du joueur seul, pas de liste de
voitures) et bien plus courte. Source retenue : `sim_info.py`, le fichier que le jeu fournit à ses
applications Python — un lecteur libre multi-jeux la décrit à tort comme celle de Competizione, ce qui
rappelle qu'un recoupement n'est pas une vérification.

- `GraphicsAc` décrit cette page ; `GraphicsAc::upgrade` en fait une page de Competizione (la voiture du
  joueur devient la seule de la liste, ce que le jeu ne publie pas reste à zéro, tout tour est valide). Une
  seule conversion sert donc les deux jeux, avec un drapeau pour ce que seul le premier Assetto Corsa
  publie : longueur du circuit, tracé, trois températures par pneu (remises dans l'ordre gauche / centre /
  droite de la trame), usure, charge, carrossage, hauteurs de caisse, nom de la gomme.
- Une zone de mémoire partagée fait au moins une page mémoire remplie de zéros après ce que le jeu écrit :
  la lire avec les structures plus longues de Competizione donne des zéros dans les champs absents.
- Deux sources (`AccSource::new`, `AccSource::assetto_corsa`), chacune ne répondant qu'à son jeu : la
  détection automatique les essaie l'une après l'autre (`lmu`, `acc`, `ac`).
- `GameId::Ac` (« ac ») est ajouté au protocole.

### D. Un jeu non vérifié est dit expérimental, partout

L'adaptateur ACC est écrit d'après la documentation de la mémoire partagée du jeu, recoupé avec deux
lecteurs libres indépendants, et essayé de bout en bout sur des pages fabriquées. Il n'a pas tourné face au
jeu. Tant que c'est le cas : l'accueil le dit quand ce jeu est détecté, `docs/JEUX.md` le dit, les notes de
version le disent, et les hypothèses à confirmer sont marquées `TODO(verify-in-game)` dans le code.

### E. Ce qui reste propre à Le Mans Ultimate

L'ingénieur de réglages (connaissance des paramètres, fichiers de setup), l'import des résultats officiels,
l'API locale du jeu et le planning des courses en ligne ne concernent que Le Mans Ultimate. Ils étaient déjà
conditionnés au jeu (`status.game == Lmu`) ; pour une session d'un autre jeu, la page Ingénieur prévient que
l'analyse est indicative et que les réglages proposés ne s'appliquent pas.

## Ordre des jeux suivants

Le propriétaire a dit (17 nov.) posséder tous les jeux de la liste : chacun pourra être vérifié chez lui.
Fait : le premier Assetto Corsa (1.8.0), iRacing (1.9.0, § D ci-dessous), Assetto Corsa EVO (1.11.0, § G),
Assetto Corsa Rally (1.12.0, § H). Ensuite, par coût croissant :
rFactor 2 (plugin tiers, structures proches de celles de Le Mans Ultimate),
Automobilista 2, RaceRoom, puis les jeux à télémétrie UDP (une
option à activer dans le jeu, et un port à écouter : à étudier avec le modèle de sécurité). Les consoles ne
sont pas prévues.

## D. iRacing (1.9.0)

Module `crates/corde-iracing`. Le jeu publie une seule zone de mémoire, `Local\IRSDKMemMapFileName`, qui
se décrit elle-même : un en-tête, un texte de session (circuit, sessions, voitures, pilotes), une table des
mesures que publie la voiture conduite (nom, type, position), et jusqu'à quatre lignes de valeurs écrites
à tour de rôle 60 fois par seconde.

- **Lecture seule, et rien d'autre.** La zone est ouverte en lecture. Le jeu accepte aussi des commandes par
  messages Windows (caméras, arrêts aux stands, discussion, télémétrie sur disque) : le module n'en contient
  aucune et n'en contiendra pas sans décision écrite ici.
- **Rien n'est cru sur parole.** Chaque position et chaque taille de l'en-tête et de la table sont comparées
  à la taille réelle de la zone avant toute lecture ; une ligne de table incohérente est ignorée ; une mesure
  absente se lit « absente ». La ligne la plus récente est recopiée, puis l'en-tête relu : si le jeu est
  revenu dessus entre-temps, la copie est refaite.
- **Le texte de session n'est pas toujours du YAML valide** (un nom de pilote y est écrit tel que tapé) : il
  est lu ligne à ligne par un lecteur qui ne refuse rien, plutôt que par une bibliothèque YAML qui
  rejetterait toute la session pour un nom.
- **Les chronos sont ceux du jeu.** Le compteur de tours bouge à la ligne ; le temps du tour arrive jusqu'à
  une seconde plus tard. `lap_number` suit le compteur ; `laps_completed` suit quand le temps a changé, ou
  après 2 s — le tour est alors sans temps officiel, et le temps encore affiché (celui d'un tour plus
  ancien) est retiré plutôt qu'attribué. Hors de la piste (garage), les compteurs du jeu lisent 0 : ils
  sont tenus, pas pris pour un redémarrage.
- **Détection.** Le jeu laisse sa zone derrière lui en se fermant, avec le bit « en session » à zéro :
  l'adaptateur se dit alors indisponible, et Corde regarde les autres jeux. Un jeu qui a planté peut
  laisser ce bit à un, et un autre programme qui lit la même zone la garde en vie : avant de se brancher,
  l'adaptateur attend donc jusqu'à 100 ms de voir le jeu écrire une nouvelle ligne. Ordre : `lmu`, `acc`,
  `ac`, `iracing`.
- **Carte du circuit.** Le jeu ne publie aucune coordonnée en roulant (celles de ses fichiers de télémétrie
  ne sont pas dans la mémoire partagée : constaté le 21 novembre 2026, 1.9.0). Depuis la 1.9.1, la position
  du pilote est reconstruite d'après sa vitesse et son cap (`odometer`), remise à sa place à chaque passage
  de ligne ; les autres voitures arrivent sans position et le widget les place d'après leur avancée sur le
  tour, une fois le tracé appris. Mesuré sur une vraie session : un tour se referme à 5–7 m près sur
  6,9 km.
- **Copie de diagnostic** (`iracing-probe --dump`) : le texte de session n'y garde que les clés d'une liste
  blanche (circuit, session, voitures) ; tout le reste — noms, équipes, identifiants, licence, nom du
  réglage — est vidé, et les mesures de type texte sont effacées.
- **Essai en jeu du 21 novembre 2026** (sessions du propriétaire, comparées à Le Mans Ultimate au même
  circuit) : détection, compte des tours et chronos officiels justes ; signes recalés en 1.9.1 (lacet,
  accélération verticale, amplitude du volant). Les températures, pressions et usure des pneus ne changent
  qu'à l'arrêt aux stands : c'est le jeu. Reste à voir : le classement avec d'autres voitures.
- **ACC, même essai** : la carte et tout ce qui est latéral étaient en miroir (un circuit qui tourne à
  gauche était dessiné tournant à droite) : l'axe z, l'accélération latérale, le lacet et le cap sont
  retournés depuis la 1.9.1, pour ACC comme pour le premier Assetto Corsa.

## F. Un profil par jeu, une interface de départ par jeu (1.10.0)

Demande du propriétaire après son premier essai de deux jeux : que Corde ouvre, pour chaque jeu, la
disposition et le tableau de bord qu'il a choisis pour lui, et que l'interface proposée au départ ne montre
pas ce que le jeu ne publie pas.

- **Où** : dans l'interface seulement (`store/game-profiles.ts`). Le serveur et le protocole ne changent
  pas : le jeu détecté est déjà dans l'état de la source (`status.game`).
- **Quoi** : par jeu, la disposition PC, la disposition téléphone, le tableau de bord de course et
  l'affichage du Direct. **Par appareil** (stockage du navigateur), comme la disposition courante et le
  tableau de bord utilisé l'étaient déjà : un téléphone et un PC n'affichent pas la même chose.
- **Quand** : à la détection d'un jeu, son profil est remis. Ce qui est choisi ensuite est gardé pour le
  dernier jeu vu, jeu fermé compris — sinon un choix fait hors piste serait défait au lancement suivant.
- **Interface de départ** : une table `GameTraits` dit ce que chaque jeu publie en roulant. Une
  disposition « Course » sans les widgets sans données est créée à la première détection, avec un
  identifiant fixe (`game-<jeu>-desktop|mobile`) pour n'exister qu'une fois entre les appareils, et une
  date nulle pour qu'une version déjà modifiée ailleurs l'emporte à la synchronisation. Le tableau de bord
  « Corde » saute la page « Pneus & freins » quand le jeu ne publie ni les uns ni les autres.
- **Rien n'est verrouillé** : ce sont des dispositions ordinaires. Les widgets restent tous dans la
  palette ; un tableau de bord fait dans le Studio est montré tel quel, quel que soit le jeu.
- **Le Mans Ultimate** garde ce qui était utilisé avant les profils : aucune surprise à la mise à jour.
- **Les overlays (OBS)** ne sont pas par jeu : leur adresse désigne déjà une disposition précise.
- **Écarté** : stocker les profils sur le serveur (un profil est un choix d'écran, pas une donnée
  partagée) ; masquer d'office un widget sans données (le pilote ne pourrait plus le remettre).

## G. Assetto Corsa EVO (1.11.0)

- **Dans le même module** que Competizione et le premier Assetto Corsa (`corde-acc`, module `evo`) : même
  famille, même lecture de pages nommées, et la page physique a la même disposition. Les pages graphique et
  statique sont nouvelles ; leurs noms aussi (`Local\acevo_pmf_*`), ce qui suffit à reconnaître le jeu.
- **Source** : la documentation de la mémoire partagée publiée par le studio, lue à travers deux lecteurs
  libres indépendants. Les structures sont réécrites d'après la liste des champs, et leurs tailles et
  positions comparées à un calcul fait à part.
- **Accès anticipé** : le format peut changer. La version de l'interface est journalisée à la connexion ;
  sous Windows la copie de diagnostic contient toute la zone mappée, pour voir ce qu'une mise à jour du jeu
  aurait ajouté. Un champ absent d'un ancien build se lit à zéro (la table des identifiants des voitures,
  ajoutée en avril 2026 : sans elle, la première case est prise pour celle du joueur).
- **Ce qui n'est pas repris** : les sous-structures dont le sens n'est pas établi (dégâts en unités
  inconnues, états des stands) ; les textes de chronos déjà mis en forme (les mêmes temps existent en
  millisecondes).
- **Copie de diagnostic** : prénom et nom effacés ; les identifiants des voitures remplacés par de petits
  numéros qui gardent les égalités (la case du joueur reste retrouvable).
- **Hypothèses à confirmer en jeu** : marquées `TODO(verify-in-game)` dans `evo/convert.rs`.

## H. Assetto Corsa Rally (1.12.0)

- **Dans le même module** (`corde-acc`, module `rally`) : le jeu publie les pages de Competizione, sous les
  mêmes noms et dans la même disposition. Il se reconnaît à ses températures, écrites en kelvins (un air à
  plus de 200 ne peut pas être en °C) ; les sources de Competizione et du premier Assetto Corsa refusent
  désormais de telles pages, et une page physique vide (pause, zone d'arrêt, ralenti) ne dit rien : la
  détection attend.
- **Source** : deux lecteurs libres indépendants qui ont observé le jeu ; aucun document du studio.
  Rien n'est vérifié face au jeu.
- **Une spéciale n'a pas de tours ; le modèle de données, lui, est fait de tours.** Plutôt que d'ajouter une
  notion d'« épreuve » partout (enregistrement, analyse, comparaison, interface), **chaque passage est un
  tour** :
  - départ : le chrono du jeu quitte zéro ;
  - arrivée : le chrono se fige alors que la voiture roule encore d'au moins 2 m (une pause fige les deux) ;
    le temps est le chrono figé — celui du jeu, jamais un temps compté par Corde ;
  - passage abandonné (chrono revenu à zéro avant l'arrivée) : un tour sans temps ;
  - hors passage (ligne de départ, zone d'arrêt), la voiture est rapportée « aux stands », pour que
    l'attente ne devienne pas un tour.
- **La source dit elle-même si le tour compte** (`count_lap_flag` : 2 pendant un passage, 0 en dehors) :
  le suivi des tours ne chronomètre alors jamais à sa place.
- **Longueur de la spéciale** : celle de la page statique quand elle arrive (le jeu l'écrit en retard) ;
  sinon celle mesurée sur le premier passage terminé ; d'ici là, une longueur provisoire (deux fois la
  distance parcourue), parce que le suivi des tours a besoin d'une longueur pour distinguer une arrivée
  d'un abandon.
- **Suivi des tours (`corde-core`), pour tous les jeux** : un « tour » passé entièrement aux stands n'en est
  pas un ; un tour abandonné n'est rapporté qu'après 3 s en piste ; la règle du « compteur qui rattrape »
  ne joue que sans passage aux stands. Les jeux de circuit n'y perdent rien (tests inchangés).
- **Carte** : un tracé ouvert (du départ à l'arrivée), que l'interface ne referme pas.
- **Profil du jeu** : sans les widgets Classement, Écarts et Électronique.
- **Hypothèses à confirmer en jeu** : marquées `TODO(verify-in-game)` dans `rally/convert.rs` — d'abord le
  départ et l'arrivée.

## Alternatives écartées

- **Demander le jeu dans les réglages** : c'est exactement ce que le propriétaire ne veut pas.
- **Dépendre d'une bibliothèque tierce multi-jeux** : celles qui existent sont jeunes, tirent un moteur
  asynchrone ou ne couvrent pas les mêmes jeux ; et un format qu'on ne maîtrise pas ne se corrige pas vite.
  Elles ont servi de recoupement.
- **Repérer le jeu par le nom de son processus** : demande d'énumérer les processus, pour un résultat moins
  sûr que « ses données sont là et vivantes ».
- **Publier l'adaptateur ACC sans le dire non vérifié** : un chrono faux en silence coûte plus cher qu'une
  mention.

## Vérification

- `corde-acc` : tailles des trois pages (800, 1588, 820 octets) et positions de champs calculées à la main ;
  trame complète depuis un faux jeu ; tours chronométrés par le vrai `LapTracker` (temps officiel publié à
  l'instant du passage, ou 0,3 s plus tard) ; horloge arrêtée en pause ; longueur d'un circuit inconnu
  mesurée ; pages du premier Assetto Corsa refusées ; menus → session → sortie ; copie de diagnostic sans le
  nom du joueur.
- `corde-core` : `AutoSource` suit un jeu, le perd, suit l'autre ; deux jeux à la fois.
- `corde-server/tests/games_auto.rs` : un serveur en `--source auto` sans jeu attend ; une session ACC
  (fausse) est détectée, enregistrée sous `game: acc`, ses tours chronométrés ; la session quittée,
  l'enregistrement est clos ; elle est retrouvée à la suivante.
- Premier Assetto Corsa : taille de sa page « graphics » (296 octets) et fin de ses pages dans les nôtres
  (580 et 684 octets) calculées à la main ; page convertie champ par champ ; trame, pneus et tours depuis
  un faux jeu qui écrit son format ; chaque source ne répond qu'à son jeu ; dans `games_auto.rs`, après
  Competizione, une session du premier Assetto Corsa est reconnue et enregistrée sous `game: ac`.
- **Non vérifié** : face aux jeux réels (le lecteur Windows compile et ses essais passent dans
  l'intégration continue, sans jeu) ; les hypothèses listées dans `docs/JEUX.md`.
