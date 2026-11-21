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
| Assetto Corsa EVO | Prévu (jeu en accès anticipé : son format bouge encore) | rien | mémoire partagée du jeu |
| rFactor 2 | Prévu | installer un plugin tiers de mémoire partagée | mémoire partagée du plugin |
| Automobilista 2, Project CARS 2 | Prévu | activer la mémoire partagée dans les options | mémoire partagée du jeu |
| RaceRoom Racing Experience | Prévu | rien | mémoire partagée du jeu |
| F1 (EA), Forza, EA WRC, DiRT Rally 2 | À étudier | activer la télémétrie réseau (UDP) dans les options | paquets UDP envoyés par le jeu |
| Jeux sur console (Gran Turismo 7…) | Pas prévu | — | pas d'interface officielle |

« Prévu » veut dire : le jeu publie de quoi le lire et le modèle de données de Corde l'accueille ; rien n'est
encore écrit. L'ordre dépendra des jeux qu'on pourra essayer.

## Ce qui marche avec chaque jeu

| | Le Mans Ultimate | Assetto Corsa Competizione | Assetto Corsa | iRacing |
|---|---|---|---|---|
| Direct : widgets et tableau de bord de course (PC, téléphone, OBS) | oui | oui | oui | oui |
| Enregistrement des sessions, chronos des tours | oui | oui (chronos officiels du jeu) | oui (chronos officiels du jeu) | oui (chronos officiels du jeu) |
| Temps par secteur | oui | pas encore | pas encore | non : le jeu ne les publie pas. Le secteur en cours est connu (quand le jeu découpe le circuit en plus de trois parties, Corde les regroupe en trois) |
| Comparaison de tours, graphes | oui | oui | oui | oui |
| Classement, écarts avec les autres voitures | oui | écart avec la voiture devant et derrière seulement : le jeu ne donne les autres pilotes que par une autre interface, pas encore branchée | non : le jeu ne publie que ta voiture | oui : places, tours, meilleur et dernier tour de chacun, écart sur la piste ; écart au premier en course |
| Températures des pneus intérieur / milieu / extérieur, usure | oui | non : le jeu publie une seule température par pneu, et pas l'usure | oui | **pas en roulant** : le jeu ne publie températures, pressions et usure des pneus qu'à l'arrêt aux stands. Entre deux arrêts, Corde montre celles du dernier arrêt |
| Températures des freins | oui | oui | oui | non : le jeu ne les publie pas |
| Niveaux d'antipatinage et d'ABS, tour annulé | oui | oui | non : le jeu ne les publie pas | les réglages au volant que la voiture possède ; un tour est marqué quand le jeu y compte un incident |
| Carte du circuit | oui | oui | oui | oui, après un premier tour : le jeu ne donne aucune position, Corde reconstruit le tracé d'après ta vitesse et ton cap, et place les autres voitures d'après leur avancée sur le tour |
| Ingénieur de course : analyse du pilotage | oui | indicative (sens de certaines mesures à vérifier) | indicative | indicative |
| Ingénieur de course : réglages proposés, fichiers de setup | oui | non : il connaît les réglages de Le Mans Ultimate | non | non |
| Import des résultats officiels, page Profil | oui | non | non | non |
| Planning des courses en ligne, rappels | oui | non (c'est le planning de Le Mans Ultimate) | non | non |

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
| Assetto Corsa Competizione | le widget Classement (le jeu ne nomme pas les autres pilotes) |
| Assetto Corsa | les widgets Classement, Écarts et Électronique (le jeu ne publie que ta voiture, et pas le niveau des aides) |
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
module par jeu (`crates/corde-lmu`, `crates/corde-acc`, `crates/corde-iracing`), tous derrière la même interface
(`corde_core::TelemetrySource`) ; `corde_core::AutoSource` choisit celui dont le jeu tourne.
