# Les jeux que Corde sait lire

Corde ne te demande pas à quel jeu tu joues. Elle regarde, toutes les deux secondes, si l'un des jeux qu'elle
connaît est en piste, s'y branche toute seule, et recommence à chercher quand tu le quittes. Tu peux passer
d'un jeu à l'autre sans rien toucher : chaque session est enregistrée sous le nom de son jeu.

Corde ne fait que **lire** ce que le jeu publie. Elle n'écrit rien dans le jeu, ne modifie aucun fichier du
jeu, et n'utilise aucun compte.

## Où en est chaque jeu

| Jeu | État | À faire dans le jeu | Comment Corde le lit |
|---|---|---|---|
| **Le Mans Ultimate** | **Vérifié en jeu** | Settings → Gameplay → **Enable Plugins** sur ON, une fois | mémoire partagée du jeu |
| **Assetto Corsa Competizione** | **Expérimental, essayé en jeu** le 21 novembre 2026 (une session de 7 tours) : détection, direct, enregistrement, chronos officiels, pneus, freins et carte fonctionnent ; les signes ont été recalés en 1.9.1 | rien | mémoire partagée du jeu |
| **Assetto Corsa** (le premier) | **Expérimental** depuis la 1.8.0 : écrit d'après le fichier de référence fourni avec le jeu, **pas encore vérifié face au jeu** | rien | mémoire partagée du jeu (les mêmes pages qu'ACC, dans une version plus ancienne) |
| **iRacing** | **Expérimental, essayé en jeu** le 21 novembre 2026 (sessions d'essai, seul en piste) : détection, direct, enregistrement, chronos officiels et carburant fonctionnent ; carte du circuit et signes corrigés en 1.9.1 ; classement avec d'autres voitures pas encore vu | rien | mémoire partagée du jeu |
| **Assetto Corsa EVO** | **Expérimental** depuis la 1.11.0 : écrit d'après la documentation de la mémoire partagée publiée par le studio, **pas encore vérifié face au jeu**. Le jeu est en accès anticipé : son format peut encore changer | rien | mémoire partagée du jeu (trois pages à son nom) |
| **Assetto Corsa Rally** | **Expérimental** depuis la 1.12.0. **Corrigé en 1.14.1 d'après des pages relevées dans le jeu** (27 novembre 2026) : avant, Corde ne voyait pas le jeu. Le direct est vérifié sur ces pages ; le comptage des passages (départ, arrivée) reste à voir en roulant. Chaque passage d'une spéciale est compté comme un tour. Le jeu est en accès anticipé : son format peut encore changer | rien | mémoire partagée du jeu (les pages d'ACC, températures en kelvins) |
| **rFactor 2** | **Expérimental** depuis la 1.13.0 : même moteur que Le Mans Ultimate, lu par le même convertisseur (déjà vérifié en jeu pour Le Mans Ultimate) ; la lecture du jeu lui-même n'est **pas encore vérifiée** | activer un plugin livré avec le jeu, une fois (voir plus bas) | mémoire partagée du plugin `rFactor2SharedMemoryMapPlugin64.dll` |
| Automobilista 2, Project CARS 2 | Prévu, les prochains (une même interface pour les deux) | activer la mémoire partagée dans les options | mémoire partagée du jeu |
| RaceRoom Racing Experience | Prévu, ensuite | rien | mémoire partagée du jeu |
| F1 24, F1 2021 | À étudier | activer la télémétrie réseau (UDP) dans les options | paquets UDP envoyés par le jeu |
| DiRT Rally 2.0, DiRT Rally, DiRT 4 | À étudier | activer la télémétrie réseau (UDP) dans un fichier de réglages du jeu | paquets UDP envoyés par le jeu |
| Forza Motorsport, Forza Horizon 4 et 5 | À étudier | activer « Data Out » dans les options | paquets UDP envoyés par le jeu |
| BeamNG.drive | À étudier (peu de mesures : vitesse, régime, rapport, pédales) | activer un protocole de sortie dans les options | paquets UDP envoyés par le jeu |
| Automobilista (le premier), rFactor (le premier), RENNSPORT, WRC 10 | À étudier : pas d'interface connue sans outil tiers | — | — |
| Euro Truck Simulator 2, American Truck Simulator | À part, sur demande (pas de tours ni de chronos : ce serait un tableau de bord, pas un ingénieur de course) | installer un plugin | mémoire partagée du plugin |
| Jeux sur console (Gran Turismo 7…) | Pas prévu | — | pas d'interface officielle |

« Prévu » veut dire : le jeu publie de quoi le lire et le modèle de données de Corde l'accueille ; rien n'est
encore écrit. « À étudier » : le jeu envoie ses mesures par le réseau ; il faut d'abord décider comment
Corde les écoute sans rien ouvrir d'autre sur le PC. L'ordre suit la bibliothèque du propriétaire du
projet : les jeux qu'il peut essayer d'abord.

## Ce qui marche avec chaque jeu

| | Le Mans Ultimate | rFactor 2 | Assetto Corsa Competizione | Assetto Corsa | Assetto Corsa EVO | Assetto Corsa Rally | iRacing |
|---|---|---|---|---|---|---|---|
| Direct : widgets et tableau de bord de course (PC, téléphone, OBS) | oui | oui | oui | oui | oui | oui | oui |
| Enregistrement des sessions, chronos des tours | oui | oui (chronos officiels du jeu) | oui (chronos officiels du jeu) | oui (chronos officiels du jeu) | oui (chronos officiels du jeu) | oui : chaque passage d'une spéciale est un tour, avec le temps du jeu ; un passage abandonné est un tour sans temps | oui (chronos officiels du jeu) |
| Temps par secteur | oui | oui | pas encore | pas encore | non : le jeu ne les publie pas (le secteur affiché est un tiers du tour) | non : le jeu ne les publie pas | non : le jeu ne les publie pas. Le secteur en cours est connu (quand le jeu découpe le circuit en plus de trois parties, Corde les regroupe en trois) |
| Comparaison de tours, graphes | oui | oui | oui | oui | oui | oui, entre deux passages de la même spéciale | oui |
| Classement, écarts avec les autres voitures | oui | oui | écart avec la voiture devant et derrière seulement : le jeu ne donne les autres pilotes que par une autre interface, pas encore branchée | non : le jeu ne publie que ta voiture | écart avec la voiture devant et derrière seulement : le jeu ne donne des autres voitures que leur position sur la carte | non : tu es seul sur la spéciale | oui : places, tours, meilleur et dernier tour de chacun, écart sur la piste ; écart au premier en course |
| Températures des pneus intérieur / milieu / extérieur, usure | oui | oui | non : le jeu publie une seule température par pneu, et pas l'usure | oui | trois températures par pneu et la pression ; pas l'usure (le jeu la laisse à zéro) | une seule température par pneu, pas l'usure | **pas en roulant** : le jeu ne publie températures, pressions et usure des pneus qu'à l'arrêt aux stands. Entre deux arrêts, Corde montre celles du dernier arrêt |
| Températures des freins | oui | oui | oui | oui | oui | oui | non : le jeu ne les publie pas |
| Niveaux d'antipatinage et d'ABS, tour annulé | oui | non : le jeu ne publie pas le niveau des aides | oui | non : le jeu ne les publie pas | oui, chacun avec son maximum ; un tour est marqué quand le jeu l'invalide | non | les réglages au volant que la voiture possède ; un tour est marqué quand le jeu y compte un incident |
| Carte du circuit | oui | oui | oui | oui | oui | oui, après un premier passage : la spéciale est une ligne du départ à l'arrivée, pas une boucle | oui, après un premier tour : le jeu ne donne aucune position, Corde reconstruit le tracé d'après ta vitesse et ton cap, et place les autres voitures d'après leur avancée sur le tour |
| Ingénieur de course : analyse du pilotage | oui | indicative | indicative (sens de certaines mesures à vérifier) | indicative | indicative (sens de certaines mesures à vérifier) | indicative | indicative |
| Ingénieur de course : réglages proposés, fichiers de setup | oui | non | non : il connaît les réglages de Le Mans Ultimate | non | non | non | non |
| Import des résultats officiels, page Profil | oui | non | non | non | non | non | non |
| Planning des courses en ligne, rappels | oui | non | non (c'est le planning de Le Mans Ultimate) | non | non | non | non |

## Un profil par jeu (depuis la 1.10.0)

Corde garde, pour chaque jeu, ce que tu utilises avec lui :

- la **disposition des widgets** (une pour le PC, une pour le téléphone) ;
- le **tableau de bord de course** ;
- l'**affichage du Direct** (widgets ou tableau de bord).

Dès qu'elle détecte un jeu, elle remet son profil. Ce que tu choisis ensuite — une autre disposition, un
autre tableau de bord — reste attaché à ce jeu, même une fois le jeu fermé : ce qui est affiché est toujours
le profil du dernier jeu vu.

**La première fois qu'un jeu est vu**, Corde lui prépare une disposition à son nom (« iRacing », « Assetto
Corsa Competizione »…) : c'est « Course », sans les widgets pour lesquels ce jeu ne publie rien, les autres
se partageant la place. Le Mans Ultimate garde simplement ce que tu utilisais.

| Jeu | Ce que Corde laisse de côté au départ |
|---|---|
| Le Mans Ultimate | rien |
| rFactor 2 | le widget Électronique (le jeu ne publie pas le niveau des aides) |
| Assetto Corsa Competizione | le widget Classement (le jeu ne nomme pas les autres pilotes) |
| Assetto Corsa | les widgets Classement, Écarts et Électronique (le jeu ne publie que ta voiture, et pas le niveau des aides) |
| Assetto Corsa EVO | le widget Classement (le jeu ne donne des autres voitures que leur position) |
| Assetto Corsa Rally | les widgets Classement, Écarts et Électronique (tu es seul sur la spéciale, et le jeu ne publie pas le niveau des aides) |
| iRacing | les widgets Pneus et Freins ; la page « Pneus & freins » et la case « Pneus » du tableau de bord « Corde » (le jeu ne publie les pneus qu'à l'arrêt aux stands, et jamais les freins) |

Rien n'est verrouillé : une disposition préparée par Corde se modifie comme les autres (**Modifier** →
**Ajouter un widget**), et un widget remis reste.

Tout se règle dans **Réglages** → **Profils par jeu** :

- **Un profil par jeu** : l'interrupteur. Coupé, Corde ne change plus rien quand le jeu change.
- Pour chaque jeu, **Disposition des widgets** et **Tableau de bord** : ce que Corde ouvrira avec ce jeu.
  Le jeu marqué « en place » est celui dont le profil est affiché.
- **Réinitialiser** : remet ce jeu comme Corde le propose (sa disposition est refaite ; Corde demande
  confirmation).

Les profils sont retenus **par appareil** : le PC et le téléphone ont chacun leurs choix. Les dispositions,
elles, sont partagées entre les appareils comme avant.

## rFactor 2 : ce qu'il faut savoir

- **Un plugin à activer, une fois.** rFactor 2 ne publie rien de lui-même : c'est le plugin
  `rFactor2SharedMemoryMapPlugin64.dll` qui le fait.
  1. Vérifie qu'il est dans `…\steamapps\common\rFactor 2\Bin64\Plugins\`. Il est normalement livré
     avec le jeu ; s'il n'y est pas, il se télécharge sur la
     [page de son auteur](https://github.com/TheIronWolfModding/rF2SharedMemoryMapPlugin) (logiciel libre,
     indépendant de Corde ; Corde ne l'installe pas à ta place).
  2. Dans le jeu : réglages (la roue dentée du menu principal) → onglet **GAMEPLAY** → section **PLUGINS** →
     `rFactor2SharedMemoryMapPlugin64.dll` sur **On**.
  3. Quitte complètement le jeu et relance-le.
- Ensuite : lance Corde, lance le jeu, entre dans une session. L'accueil passe à « Jeu détecté —
  rFactor 2 » en quelques secondes (Corde attend de voir les données bouger avant de s'y fier).
- **Le même moteur que Le Mans Ultimate** : le plugin publie les mêmes mesures, dans les mêmes unités.
  Corde les lit avec le convertisseur de Le Mans Ultimate, celui qui a été vérifié en jeu : chronos
  officiels, secteurs, classement et écarts, pneus (trois températures, pression, usure, carcasse), freins,
  carburant, batterie et moteur électrique, carte.
- **Ce que rFactor 2 ne publie pas** (Le Mans Ultimate l'a ajouté depuis) : le niveau des aides
  (antipatinage, ABS, cartographie), l'énergie virtuelle, l'écart au meilleur tour calculé par le jeu,
  le type de gomme, la température idéale des pneus, le nom du modèle de voiture, l'état de la piste.
- **Corde ne fait que lire.** Le plugin accepte aussi des commandes venues d'autres programmes (météo,
  règles de course, boutons) : Corde n'ouvre aucune de ces entrées.
- **Expérimental, pas encore vérifié face au jeu** : écrit d'après la description publique du plugin et
  testé contre un faux jeu. À confirmer : que le jeu est reconnu, que tes tours ont le bon chrono, et que
  rien n'est décalé.
- **Si Le Mans Ultimate charge ce même plugin** (d'autres outils l'y installent) : rien ne change, Corde
  reconnaît Le Mans Ultimate par son interface à lui.

### Aider à vérifier rFactor 2

Dans l'archive `corde-<version>-portable-windows.zip` de la page des versions, l'outil **`rf2-probe.exe`**
montre ce que Corde lit du jeu :

1. Lance rFactor 2 (plugin activé) et entre en piste.
2. Ouvre une invite de commandes dans le dossier de l'archive et lance `rf2-probe.exe --dump copie-rf2`.
3. Compare les lignes affichées (circuit, voiture, place, tour, vitesse, rapport, régime, carburant,
   chronos) avec ce que montre le jeu.

Si l'outil répond « rFactor 2 introuvable », le plugin n'est pas activé (ou le jeu n'est pas dans une
session). Le dossier `copie-rf2` contient une copie de ce que le jeu publiait à cet instant, **sans aucun
nom de pilote** (ni le tien, ni ceux des autres). C'est le plus utile à joindre à un signalement sur
[GitHub](https://github.com/dultra123/corde/issues) quand une valeur est fausse.

## Assetto Corsa Competizione : ce qu'il faut savoir

- **Rien à installer ni à activer** : lance Corde, lance le jeu, entre en piste. L'accueil passe à « Jeu
  détecté — Assetto Corsa Competizione ».
- Corde ne voit le jeu **que pendant une session** (dans les menus, le jeu ne publie rien : l'accueil reste
  sur « En attente d'un jeu… »).
- **Essayé en jeu le 21 novembre 2026** (Ferrari 296 GT3 à Imola, 7 tours) : le jeu est reconnu, les sept
  tours ont le chrono officiel du jeu, pneus, freins, secteurs et carte sont vivants. Cette session a
  permis de corriger en 1.9.1 ce qui était à l'envers : le sens des accélérations latérales et du lacet,
  la carte (dessinée en miroir), l'embrayage (le jeu publie son engagement, pas la pédale), et le nom du
  circuit (« Imola » et non « Imola — Track Config »).
- **Reste à confirmer** : la répartition de freinage (le jeu la publie avant un décalage propre à chaque
  voiture : Corde affiche la valeur brute, proche de celle de la voiture mais pas égale), et le carburant
  (resté constant pendant cette session : consommation désactivée dans le jeu, ou mesure à revoir).
- La longueur des circuits livrés avec le jeu est connue ; pour un autre circuit, elle est mesurée sur le
  premier tour complet.

## Assetto Corsa (le premier) : ce qu'il faut savoir

- **Rien à installer ni à activer**, comme pour Competizione. L'accueil passe à « Jeu détecté — Assetto
  Corsa ». Corde ne voit le jeu que pendant une session.
- Les deux jeux publient des pages du même nom ; Corde les distingue par le numéro de leur interface
  (« 1.7 » au plus pour le premier Assetto Corsa). Sa page de session est plus ancienne et plus courte :
  elle est décrite d'après `sim_info.py`, le fichier que le jeu fournit à ses applications Python.
- **Pas encore vérifié face au jeu.** Les corrections faites en 1.9.1 d'après Competizione (sens des
  accélérations, carte, embrayage) lui sont appliquées aussi : les deux jeux écrivent ces mesures de la
  même façon. À confirmer en priorité : l'échelle de l'usure des pneus (supposée de 100 à 0), l'ordre
  intérieur / extérieur des températures.
- Le jeu donne la longueur du circuit : rien à mesurer. Les voitures et circuits ajoutés (mods) sont lus
  comme les autres ; leur nom est celui de leur dossier.
- `acc-probe.exe` (ci-dessous) reconnaît les deux jeux.

## Assetto Corsa EVO : ce qu'il faut savoir

- **Rien à installer ni à activer** : le jeu publie de lui-même trois pages de mémoire partagée, sous des
  noms à lui. Corde les ouvre en lecture seule.
- **Expérimental, pas encore vérifié face au jeu** : le module est écrit d'après la documentation que le
  studio a publiée, et testé contre un faux jeu. Le premier essai en piste dira ce qui est juste.
- **Le jeu est en accès anticipé** : le format de ses pages peut changer d'une mise à jour à l'autre. Si
  Corde affiche soudain des valeurs absurdes après une mise à jour du jeu, c'est sans doute cela : une copie
  de diagnostic (ci-dessous) permet de corriger.
- **Ce que le jeu publie** : chronos officiels, position sur le tour, longueur du circuit, carburant et
  réservoir, régime maximal, trois températures et la pression de chaque pneu, températures des freins,
  niveaux d'antipatinage, d'ABS et de cartographie moteur avec leur maximum, répartition de freinage,
  écarts avec la voiture de devant et de derrière, phares et essuie-glaces.
- **Ce qu'il ne publie pas** : les autres pilotes (seulement la position de leur voiture : pas de
  classement), les temps par secteur, l'usure des pneus (le champ existe et reste à zéro), la pluie.
- **À confirmer en piste** : le sens de ce qui est latéral (la carte pourrait être dessinée en miroir,
  comme elle l'était pour Assetto Corsa Competizione avant la 1.9.1), l'ordre des accélérations, l'embrayage.

### Aider à vérifier Assetto Corsa EVO

Dans l'archive `corde-<version>-portable-windows.zip` de la page des versions, l'outil **`acc-probe.exe`**
reconnaît aussi ce jeu :

1. Lance Assetto Corsa EVO et entre en piste.
2. Ouvre une invite de commandes dans le dossier de l'archive et lance `acc-probe.exe --dump copie-evo`.
3. Compare les lignes affichées (version de l'interface, circuit, voiture, tour, vitesse, rapport, carburant,
   chronos) avec ce que montre le jeu. La ligne « brut » donne les valeurs telles que le jeu les écrit.

Le dossier `copie-evo` contient une copie de ce que le jeu publiait à cet instant, **sans ton prénom ni ton
nom**, et les identifiants des voitures y sont remplacés par de simples numéros. C'est le plus utile à
joindre à un signalement sur [GitHub](https://github.com/dultra123/corde/issues) quand une valeur est fausse.

## Assetto Corsa Rally : ce qu'il faut savoir

- **Rien à installer ni à activer** : le jeu publie les mêmes pages de mémoire partagée qu'Assetto Corsa
  Competizione. Corde les ouvre en lecture seule, et reconnaît le jeu à ses températures (il les écrit en
  kelvins).
- **Expérimental.** Le module a d'abord été écrit d'après ce que des outils libres avaient relevé du jeu.
  Le premier essai (27 novembre 2026) a montré que **Corde ne voyait pas le jeu** : il ne remplit pas tout
  ce que ses pages prévoient. Corrigé en **1.14.1**, d'après des pages copiées dans le jeu en roulant et en
  pause (Peugeot 206 WRC, Alsace Steigenbach) :
  - la case « état » reste à zéro, en roulant comme à l'arrêt : Corde regarde si la voiture est simulée ;
  - le chrono n'est donné qu'en texte (« 00:05.996 ») : Corde lit ce texte ;
  - la page fixe ne donne que la voiture, la spéciale et sa longueur : le régime maximal vient de la page
    de la voiture ; la capacité du réservoir n'est donnée nulle part ;
  - en pause, la page de la voiture est vide et le reste est figé (comme attendu).
  Sur ces pages, Corde lit juste : spéciale et longueur, voiture, vitesse, rapport, régime, températures
  (air, piste, eau, pneus, freins), pressions, carburant.
- **Une spéciale n'a pas de tours** : Corde compte **chaque passage comme un tour**.
  - Le passage commence quand le chrono du jeu démarre (au franchissement de la ligne de départ).
  - Il finit à l'arrivée, quand le chrono du jeu s'arrête alors que la voiture roule encore. Le temps
    enregistré est **celui du jeu**, pas un temps compté par Corde.
  - Si tu relances la spéciale avant l'arrivée, le passage est gardé comme un tour **sans temps**.
  - L'attente sur la ligne de départ et la zone d'arrêt après l'arrivée ne sont pas comptées : Corde les
    montre comme un passage « aux stands ».
- **Comparer deux passages** de la même spéciale marche comme pour deux tours d'un circuit (page Sessions).
- **La carte** se dessine pendant ton premier passage et reste : une ligne du départ à l'arrivée.
- **La longueur de la spéciale** : le jeu la donne parfois avec retard. En attendant, Corde la mesure sur ton
  premier passage terminé ; pendant ce tout premier passage, l'avancée affichée sur la spéciale n'est qu'une
  estimation.
- **Ce que le jeu ne publie pas** : les autres concurrents, les temps intermédiaires, le niveau des aides,
  l'usure des pneus.
- **Le jeu est en accès anticipé** : le format de ses pages peut changer d'une mise à jour à l'autre.
- **Quand tu quittes une spéciale**, rien dans les pages ne le dit : Corde ferme la session quand la voiture
  n'est plus simulée depuis une minute (ou tout de suite si le jeu est fermé). Une pause de plus d'une
  minute coupe donc la session en deux.
- **Si Corde est lancée au milieu d'un passage**, ce passage n'est pas chronométré (elle n'a pas vu le
  départ) : le suivant l'est.
- **À confirmer en jeu** : le départ et l'arrivée (un passage par passage, avec le bon temps), ce que le jeu
  publie dans ses menus, le sens de ce qui est latéral (carte en miroir ?).

### Aider à vérifier Assetto Corsa Rally

Dans l'archive `corde-<version>-portable-windows.zip` de la page des versions, l'outil **`acc-probe.exe`**
reconnaît aussi ce jeu :

1. Lance Assetto Corsa Rally et pars sur une spéciale (le chrono doit tourner).
2. Ouvre une invite de commandes dans le dossier de l'archive et lance `acc-probe.exe --dump copie-rally`.
3. La première ligne doit dire « Jeu reconnu : Assetto Corsa Rally ». Compare les suivantes (épreuve,
   voiture, chrono du jeu, vitesse, rapport, numéro du passage) avec ce que montre le jeu.

Le dossier `copie-rally` contient une copie de ce que le jeu publiait à cet instant, **sans ton prénom, ton
nom ni ton pseudo**. Et le plus parlant reste une session enregistrée : fais deux passages de la même
spéciale, puis regarde dans **Sessions** si Corde a bien compté deux tours, avec les temps du jeu.

## iRacing : ce qu'il faut savoir

- **Rien à installer ni à activer** : lance Corde, lance le jeu, monte dans la voiture. L'accueil passe à
  « Jeu détecté — iRacing ». Corde ne voit le jeu que pendant une session.
- **Corde ne fait que lire.** Le jeu accepte aussi des commandes venues d'autres programmes (caméras,
  demandes d'arrêt aux stands, messages) : Corde n'en envoie aucune.
- Le jeu décrit lui-même les mesures qu'il publie, voiture par voiture : une mesure que ta voiture n'a pas
  (antipatinage, ABS, batterie…) reste simplement vide.
- **Les chronos sont ceux du jeu.** Le jeu publie le temps d'un tour jusqu'à une seconde après la ligne :
  Corde l'attend. Un tour que le jeu refuse de chronométrer est compté sans temps.
- **Essayé en jeu le 21 novembre 2026** (Ferrari 488 GT3 et Audi R8 LMS à Spa, sessions d'essai) : le jeu
  est reconnu, les tours ont le chrono officiel du jeu, le carburant, les rapports, les aides et les
  températures d'eau et d'huile sont justes.
- **Ce que le jeu ne publie pas en roulant** — et que Corde ne peut donc pas montrer : la position de la
  voiture, les températures, pressions et usure des pneus (seulement à l'arrêt aux stands), les
  températures des freins, les hauteurs de caisse, les temps par secteur.
- **La carte du circuit** est reconstruite d'après ta vitesse et ton cap (1.9.1) : elle apparaît après un
  tour. Comparée au même circuit dans Le Mans Ultimate, elle tombe juste à une quinzaine de mètres près.
- **Corrigé en 1.9.1 d'après ces sessions** : le sens du lacet, l'accélération verticale (le jeu y compte la
  pesanteur), l'amplitude du volant, la catégorie de la voiture quand tu roules seul.
- **Reste à confirmer** : le classement et les écarts avec d'autres voitures en piste (les essais étaient
  en solo), l'amplitude du volant sur une voiture dont on connaît le braquage.
- Les noms des autres pilotes s'affichent dans le classement, comme dans le jeu, et sont enregistrés avec
  ta session sur ton PC. Ils ne sortent pas de chez toi.

### Aider à vérifier iRacing

Dans l'archive `corde-<version>-portable-windows.zip` de la page des versions, l'outil
**`iracing-probe.exe`** montre ce que Corde lit du jeu :

1. Lance iRacing et monte dans la voiture (une session d'essai suffit).
2. Ouvre une invite de commandes dans le dossier de l'archive et lance
   `iracing-probe.exe --dump copie-iracing.map`.
3. Compare les lignes affichées (circuit, voiture, tour, vitesse, rapport, carburant, chronos) avec ce que
   montre le jeu. Fais-le une fois juste après la sortie des stands, et une fois après un tour complet.

Le fichier `copie-iracing.map` contient une copie de ce que le jeu publiait à cet instant, **sans aucun nom
ni identifiant** : ni le tien, ni ceux des autres pilotes, ni les équipes, ni ton numéro de compte, ni le nom
de ton réglage. Il ne garde que ce qui décrit le circuit, la session et les voitures. C'est le plus utile à
joindre à un signalement sur [GitHub](https://github.com/dultra123/corde/issues) quand une valeur est
fausse. `iracing-probe.exe --vars` liste en plus toutes les mesures que ta voiture publie.

### Aider à vérifier Assetto Corsa Competizione ou Assetto Corsa

Dans l'archive `corde-<version>-portable-windows.zip` de la page des versions, l'outil **`acc-probe.exe`**
montre ce que Corde lit du jeu :

1. Lance Assetto Corsa Competizione (ou Assetto Corsa) et entre en piste.
2. Ouvre une invite de commandes dans le dossier de l'archive et lance `acc-probe.exe --dump copie-acc`.
3. Compare les lignes affichées (jeu reconnu, circuit, voiture, tour, vitesse, rapport, carburant, chronos)
   avec ce que montre le jeu.

Le dossier `copie-acc` contient une copie de ce que le jeu publiait à cet instant, **sans ton prénom, ton nom
ni ton pseudo** (l'outil les efface). C'est le plus utile à joindre à un signalement sur
[GitHub](https://github.com/dultra123/corde/issues) quand une valeur est fausse.

## Pour les curieux

La décision et ses raisons : [ADR 0020](adr/0020-plusieurs-jeux-detection-automatique.md). Le code : un
module par jeu (`crates/corde-lmu` pour Le Mans Ultimate et rFactor 2, `crates/corde-acc` pour les quatre Assetto Corsa, `crates/corde-iracing`), tous derrière la même interface
(`corde_core::TelemetrySource`) ; `corde_core::AutoSource` choisit celui dont le jeu tourne.
