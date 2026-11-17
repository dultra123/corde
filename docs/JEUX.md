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
| **Assetto Corsa Competizione** | **Expérimental** depuis la 1.7.0 : écrit d'après la documentation du jeu, **pas encore vérifié face au jeu** | rien | mémoire partagée du jeu |
| **Assetto Corsa** (le premier) | **Expérimental** depuis la 1.8.0 : écrit d'après le fichier de référence fourni avec le jeu, **pas encore vérifié face au jeu** | rien | mémoire partagée du jeu (les mêmes pages qu'ACC, dans une version plus ancienne) |
| **iRacing** | **Expérimental** depuis la 1.9.0 : écrit d'après le kit de développement du jeu, **pas encore vérifié face au jeu** | rien | mémoire partagée du jeu |
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
| Temps par secteur | oui | pas encore | pas encore | non : le jeu ne les publie pas (le secteur en cours est connu quand le circuit en a trois) |
| Comparaison de tours, graphes | oui | oui | oui | oui |
| Classement, écarts avec les autres voitures | oui | écart avec la voiture devant et derrière seulement : le jeu ne donne les autres pilotes que par une autre interface, pas encore branchée | non : le jeu ne publie que ta voiture | oui : places, tours, meilleur et dernier tour de chacun, écart sur la piste ; écart au premier en course |
| Températures des pneus intérieur / milieu / extérieur, usure | oui | non : le jeu publie une seule température par pneu, et pas l'usure | oui | trois températures par pneu, en direct ; usure et pressions : celles du dernier arrêt (le jeu ne les mesure qu'aux stands) |
| Températures des freins | oui | oui | oui | non : le jeu ne les publie pas |
| Niveaux d'antipatinage et d'ABS, tour annulé | oui | oui | non : le jeu ne les publie pas | les réglages au volant que la voiture possède ; un tour est marqué quand le jeu y compte un incident |
| Carte du circuit | oui | oui | oui | oui, après un premier tour (le jeu ne donne la position que de ta voiture : les autres sont placées d'après leur avancée sur le tour) |
| Ingénieur de course : analyse du pilotage | oui | indicative (sens de certaines mesures à vérifier) | indicative | indicative |
| Ingénieur de course : réglages proposés, fichiers de setup | oui | non : il connaît les réglages de Le Mans Ultimate | non | non |
| Import des résultats officiels, page Profil | oui | non | non | non |
| Planning des courses en ligne, rappels | oui | non (c'est le planning de Le Mans Ultimate) | non | non |

## Assetto Corsa Competizione : ce qu'il faut savoir

- **Rien à installer ni à activer** : lance Corde, lance le jeu, entre en piste. L'accueil passe à « Jeu
  détecté — Assetto Corsa Competizione ».
- Corde ne voit le jeu **que pendant une session** (dans les menus, le jeu ne publie rien : l'accueil reste
  sur « En attente d'un jeu… »).
- **Pas encore vérifié face au jeu.** L'adaptateur est écrit d'après la documentation de la mémoire
  partagée du jeu et recoupé avec deux lecteurs libres ; il est essayé sur des pages fabriquées, pas sur le
  jeu. Des valeurs peuvent manquer ou être fausses. Ce qui reste à confirmer en priorité : l'unité du
  carburant (litres), le sens des accélérations latérale et longitudinale, la répartition de freinage (le
  jeu la publie avant un décalage propre à chaque voiture), le moment exact où le temps du tour est publié.
- La longueur des circuits livrés avec le jeu est connue ; pour un autre circuit, elle est mesurée sur le
  premier tour complet.

## Assetto Corsa (le premier) : ce qu'il faut savoir

- **Rien à installer ni à activer**, comme pour Competizione. L'accueil passe à « Jeu détecté — Assetto
  Corsa ». Corde ne voit le jeu que pendant une session.
- Les deux jeux publient des pages du même nom ; Corde les distingue par le numéro de leur interface
  (« 1.7 » au plus pour le premier Assetto Corsa). Sa page de session est plus ancienne et plus courte :
  elle est décrite d'après `sim_info.py`, le fichier que le jeu fournit à ses applications Python.
- **Pas encore vérifié face au jeu.** À confirmer en priorité : l'échelle de l'usure des pneus (supposée de
  100 à 0), l'ordre intérieur / extérieur des températures, le sens des accélérations.
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
- **Pas encore vérifié face au jeu.** L'adaptateur est écrit d'après le kit de développement du jeu et
  recoupé avec des lecteurs libres ; il est essayé sur une mémoire fabriquée, pas sur le jeu. À confirmer en
  priorité : la façon dont le jeu compte les tours à la sortie des stands, le sens des accélérations et du
  volant, l'angle maximal du volant, l'orientation de la carte.
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
