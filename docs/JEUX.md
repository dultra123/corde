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
| Assetto Corsa (le premier) | Prévu | rien | les mêmes pages qu'ACC, dans une version plus courte |
| Assetto Corsa EVO | Prévu (jeu en accès anticipé : son format bouge encore) | rien | mémoire partagée du jeu |
| iRacing | Prévu | rien | mémoire partagée du jeu |
| rFactor 2 | Prévu | installer un plugin tiers de mémoire partagée | mémoire partagée du plugin |
| Automobilista 2, Project CARS 2 | Prévu | activer la mémoire partagée dans les options | mémoire partagée du jeu |
| RaceRoom Racing Experience | Prévu | rien | mémoire partagée du jeu |
| F1 (EA), Forza, EA WRC, DiRT Rally 2 | À étudier | activer la télémétrie réseau (UDP) dans les options | paquets UDP envoyés par le jeu |
| Jeux sur console (Gran Turismo 7…) | Pas prévu | — | pas d'interface officielle |

« Prévu » veut dire : le jeu publie de quoi le lire et le modèle de données de Corde l'accueille ; rien n'est
encore écrit. L'ordre dépendra des jeux qu'on pourra essayer.

## Ce qui marche avec chaque jeu

| | Le Mans Ultimate | Assetto Corsa Competizione |
|---|---|---|
| Direct : widgets et tableau de bord de course (PC, téléphone, OBS) | oui | oui |
| Enregistrement des sessions, chronos des tours | oui | oui (chronos officiels du jeu) |
| Temps par secteur | oui | pas encore |
| Comparaison de tours, graphes | oui | oui |
| Classement, écarts avec toutes les voitures | oui | écart avec la voiture devant et derrière seulement : le jeu ne donne les autres pilotes que par une autre interface, pas encore branchée |
| Usure des pneus, températures intérieur / milieu / extérieur | oui | non : le jeu ne les publie pas (une température par pneu) |
| Carte du circuit | oui | oui |
| Ingénieur de course : analyse du pilotage | oui | indicative (sens de certaines mesures à vérifier) |
| Ingénieur de course : réglages proposés, fichiers de setup | oui | non : il connaît les réglages de Le Mans Ultimate |
| Import des résultats officiels, page Profil | oui | non |
| Planning des courses en ligne, rappels | oui | non (c'est le planning de Le Mans Ultimate) |

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

### Aider à le vérifier

Dans l'archive `corde-<version>-portable-windows.zip` de la page des versions, l'outil **`acc-probe.exe`**
montre ce que Corde lit du jeu :

1. Lance Assetto Corsa Competizione et entre en piste.
2. Ouvre une invite de commandes dans le dossier de l'archive et lance `acc-probe.exe --dump copie-acc`.
3. Compare les dix lignes affichées (circuit, voiture, tour, vitesse, rapport, carburant, chronos) avec ce
   que montre le jeu.

Le dossier `copie-acc` contient une copie de ce que le jeu publiait à cet instant, **sans ton prénom, ton nom
ni ton pseudo** (l'outil les efface). C'est le plus utile à joindre à un signalement sur
[GitHub](https://github.com/dultra123/corde/issues) quand une valeur est fausse.

## Pour les curieux

La décision et ses raisons : [ADR 0020](adr/0020-plusieurs-jeux-detection-automatique.md). Le code : un
module par jeu (`crates/corde-lmu`, `crates/corde-acc`), tous derrière la même interface
(`corde_core::TelemetrySource`) ; `corde_core::AutoSource` choisit celui dont le jeu tourne.
