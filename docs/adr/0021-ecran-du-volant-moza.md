# ADR 0021 — L'écran d'un volant MOZA : un export du Studio, pas un pilote

- **Statut** : **décidée** — 2026-11-26 (demande du propriétaire, 22 nov. : que l'écran de son volant
  MOZA KS Pro affiche les tableaux de bord de Corde, « en synergie avec MOZA Pit House » ; puis, 26 nov. :
  cet écran ne se pilote par autre chose que Pit House fermé, « c'est pas grave, fais en sorte qu'on puisse
  le faire quand même, on laisse la possibilité aux gens de le faire ou pas »).
- **Source** : `packages/ui/src/dashdef/mzdash.ts`, `packages/ui/src/studio/MozaExport.tsx`, le modèle
  « Écran de volant MOZA » de `packages/ui/src/dashdef/builtin.ts`, le préréglage `wheel-moza`
  (`model.ts`, `corde-server/src/dashes.rs`), `docs/VOLANT-MOZA.md`.
- **État** : **expérimental**. Rien de ceci n'a encore été vu sur un volant.

## Contexte

L'écran d'un volant MOZA KS Pro ou CS Pro (780 × 248 points) n'est pas un moniteur : on ne peut pas y
ouvrir une page de Corde comme sur un téléphone. Il affiche des **tableaux de bord au format du
fabricant** (`.mzdash`), faits dans son éditeur, envoyés au volant, puis **alimentés en données par le
logiciel qui pilote la base** : MOZA Pit House. Ce que le volant montre, c'est ce que ce logiciel lui
envoie, donnée nommée par donnée nommée (`v1/gameData/Rpm`…).

Un logiciel tiers ne peut parler à la base que Pit House fermé : les deux se disputent le même port
série. Un greffon libre pour un outil de tableaux de bord très répandu chez les pilotes le fait (envoi
des fichiers `.mzdash` et flux de données ; ils sont nommés dans `docs/VOLANT-MOZA.md`, là où le pilote
en a besoin), et sa documentation décrit le protocole : sessions, niveaux de données, flux compactés bit
à bit, variantes selon le micrologiciel.

Le format `.mzdash`, lui, est du JSON lisible : une fenêtre, des écrans (les pages), et sur chaque écran
des textes, des rectangles et des jauges dont les propriétés peuvent être liées à une petite expression
sur ces données nommées. Il n'est pas documenté par son fabricant.

## Décision

### A. Corde n'écrit pas de pilote pour la base

Corde **ne parle pas au volant**. Trois raisons, par ordre de poids :

1. **C'est une base à retour de force.** Envoyer à un moteur des commandes d'un protocole non documenté,
   qui change avec le micrologiciel, sans pouvoir l'essayer, n'est pas un risque à faire prendre à un
   pilote. Le reste de Corde ne fait que lire (ADR 0020) ; ceci aurait été sa première écriture vers du
   matériel.
2. **Il faudrait fermer Pit House**, donc perdre les réglages de la base pendant qu'on roule : l'inverse
   de la « synergie » demandée d'abord.
3. Le travail existe déjà, libre, entretenu et essayé sur ce matériel : le refaire à l'identique dans
   Corde n'apporte rien au pilote.

### B. Le Studio exporte un tableau de bord au format du volant

« **Exporter pour le volant MOZA** » écrit un fichier `<Nom>.mzdash` à partir d'un tableau de bord du
Studio. Le même fichier sert des deux façons :

- **avec Pit House** (la voie normale) : on l'importe, on l'envoie au volant, Pit House l'alimente ;
- **Pit House fermé**, avec un logiciel tiers qui pilote l'écran du volant : au choix du pilote, à ses
  risques, et documenté comme tel (`docs/VOLANT-MOZA.md`). Corde n'installe rien, ne lance rien et ne
  dépend de rien pour cela.

Le fichier est **fabriqué dans la page et remis au navigateur comme un téléchargement**. Rien n'est
envoyé nulle part ; Corde n'écrit pas dans les dossiers de Pit House (une installation en un clic depuis
l'application PC ne viendra, si elle vient, qu'après un premier essai réussi sur un volant, et sur une
action explicite).

### C. Les données sont celles du logiciel qui pilote le volant, pas celles de Corde

C'est la conséquence de A, et elle est dite au pilote à chaque export : sur le volant, un nombre vient
de Pit House (ou de l'outil tiers), calculé à sa façon. Une donnée du Studio est donc **traduite vers
la donnée de même sens** de ce côté, avec la conversion d'unité quand il en faut une (un pourcentage
ramené à une part de 0 à 1, par exemple), d'après le catalogue de données du fabricant.

**Une donnée sans équivalent sûr n'est pas devinée : son élément est laissé de côté et signalé.**
Laissées de côté à ce jour :

| Donnée du Studio | Pourquoi |
| --- | --- |
| Huile, angle du volant, forces g, position de classe, écart au premier, arrêts, tours restants, temps écoulé, carburant pour finir, énergie par tour, régénération, pluie, piste mouillée, secteurs | le volant ne les reçoit pas |
| Usure des pneus | reçue, mais sans dire si c'est l'usure ou ce qui reste |
| Heure en jeu, secteur en cours | reçues, mais sans unité ni origine dites |
| Migration de freinage | la donnée du même nom est décrite comme autre chose |

À rouvrir une par une quand un essai sur le volant aura tranché.

### D. Seuls les éléments simples ont un équivalent

| Élément du Studio | Sur le volant |
| --- | --- |
| Valeur | un texte lié à la donnée, écrit comme le Studio l'écrit (mêmes décimales, mêmes signes), avec son étiquette et son cadre |
| Barre / jauge | une jauge de 0 à 100 sur son fond, horizontale ou verticale |
| Texte, panneau, séparateur, forme | textes et rectangles (un ovale : un rectangle aux coins arrondis au maximum) |
| Rampe de régime | un rectangle par voyant, allumé à partir de sa part du régime, tous de la couleur de passage en haut |
| Bandeau d'alerte | rectangle et texte, clignotants si demandé |
| Horloge « temps restant » | un texte lié au temps restant de la session |
| Jauge circulaire | la valeur seule (l'arc n'a pas d'équivalent sûr) : signalé |
| « Afficher seulement quand… » | la visibilité de l'élément liée à la même condition ; caché tant qu'elle n'est pas vraie |
| Règles de couleur, couleur propre d'une donnée | la couleur liée à la même règle |
| Fond, contour, coins, opacité d'un élément | un rectangle derrière lui |

Quelques **blocs de Corde** qui ont une forme simple évidente sont écrits sous cette forme (rampe de
régime, rapport, écart au meilleur tour, position, tour en cours) et signalés quand le bloc montrait
davantage. Les autres blocs, les pages entières, les widgets, **les images et les vidéos** sont laissés
de côté et signalés. Une page dont rien n'a pu être écrit n'est pas écrite (ce serait un écran noir).

Pour qu'un export soit complet du premier coup, le Studio propose un **modèle « Écran de volant MOZA »**
(trois pages : course, pneus et freins, carburant et énergie), fait d'éléments simples sur des données
que le volant reçoit, et un **format prêt** « Écran de volant MOZA », 780 × 248. Le bouton d'export
n'est proposé que pour un tableau de bord à ce format.

### E. Le fichier imite ceux de l'éditeur du fabricant, clé pour clé

Le format a été **appris en lisant** des tableaux de bord faits avec l'éditeur du fabricant, et la
documentation publique du greffon libre cité plus haut. Rien n'en est copié : ni fichier, ni code. Ce
qui en est retenu, et que les tests vérifient :

- chaque nœud porte la même enveloppe, plus le bloc de son type, **et aucun autre** (les ombres ne sont
  pas les mêmes sur un texte, un rectangle ou une jauge : un bloc inattendu est la façon la plus sûre
  d'obtenir un élément vide, sans message) ;
- les identifiants sont des entiers uniques sous 10 000, la fenêtre portant 0 ; l'export s'arrête avant
  et le dit ;
- contour et coins sont écrits aux deux endroits où le format les tient ;
- une liaison est une chaîne de deux étapes : le calcul, puis son résultat tel quel ;
- seules sont liées des propriétés vues liées dans des fichiers de l'éditeur ;
- clés triées, retrait de quatre espaces, UTF-8 sans marque d'ordre : comme l'éditeur écrit ;
- le nom du fichier ne garde que lettres sans accent, chiffres, espaces, `-` et `_` : le volant en tire
  le nom qu'il affiche.

### F. Rien de ce que le pilote écrit ne devient du code

Une définition de tableau de bord ne contient ni adresse, ni HTML, ni script (ADR 0015) ; le fichier
exporté contient, lui, des expressions — c'est son format. **Elles sont toutes écrites par Corde** à
partir d'une liste fermée de données et de formes. Ce qui vient de la définition n'y entre que
vérifié : un nombre fini pour un seuil, une couleur `#rrggbb` pour une couleur, un identifiant de donnée
seulement s'il est dans la table. Les textes du pilote (étiquettes, titres, noms) vont dans des champs
de texte du fichier, jamais dans une expression. Les tables se lisent par leurs clés propres : un nom
comme `constructor` n'est ni une donnée ni un alignement.

## Conséquences

- Corde ne gagne **aucune permission** et n'ouvre **aucune connexion** : pas de port série, pas
  d'accès aux dossiers d'un autre logiciel, pas de requête. Le modèle de sécurité ne change pas.
- La fidélité est celle d'une traduction : la police, les tailles de lettres et le détail du dessin
  sont ceux du volant, pas ceux du Studio. L'aperçu du Studio montre la disposition, pas le rendu exact.
- Tant qu'un volant n'a pas montré un export, tout ceci est **expérimental** et dit comme tel dans
  l'application. Les premiers retours diront : si le fichier s'importe, si les polices tiennent dans
  leurs cases, si les unités sont les bonnes, quelles données laissées de côté peuvent revenir.
- **Deux volants** : un tableau de bord exporté est un fichier par volant à écran ; un volant sans écran
  n'est pas concerné. Rien à faire de plus dans Corde.

## Écarté

- **Un pilote série dans Corde** : voir A.
- **Diffuser une page de Corde vers l'écran** : ce n'est pas un moniteur.
- **Alimenter l'écran avec les données de Corde à travers l'outil tiers** (un greffon de Corde pour cet
  outil) : un deuxième logiciel à écrire et à suivre, pour des nombres qui diffèrent peu. À reconsidérer
  si des pilotes le demandent.
- **Transporter les images** : le format les range dans un dossier commun de Pit House, sous un nom
  tiré de leur contenu, et un fichier importé les porte autrement ; sans volant pour l'essayer, mieux
  vaut un export sans image qu'un export aux images cassées.
- **Deviner les données incertaines** (usure, heure, secteur) : un nombre faux sur un volant, en
  course, est pire qu'une case absente.
