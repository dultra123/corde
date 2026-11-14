# ADR 0015 — Studio : tableaux de bord personnalisés

- **Statut** : accepté (Corde 1.2.0, novembre 2026)
- **Contexte** : le propriétaire veut le tableau de bord de course aussi sur PC, et pouvoir **créer ses
  propres tableaux de bord** (placer les données qu'il veut, où il veut, sur plusieurs pages), une
  possibilité qu'offrent les éditeurs de tableaux de bord d'autres logiciels de course (source
  d'inspiration). La 1.1.0 avait quatre pages codées à la main (`packages/ui/src/dash/`, ADR 0014).

## Décision

### A. Un tableau de bord est une définition

`DashDefinition` (type Rust dans `corde-protocol`, type TypeScript généré par ts-rs) :

```text
{ v: 1, id, name, author?, format: {preset, width, height},
  theme: {background, panel, line, text, dim, accent},
  pages: [{ id, name, cols, rows, elements: [{ id, type, x, y, w, h, props }] }],
  updated_ms }
```

- **Format de dessin** : une taille de référence et des préréglages (téléphone paysage 852 × 393, téléphone
  portrait 393 × 852, tablette 4:3, écran 16:9, bandeau large type écran de volant 800 × 240, personnalisé)
  et `fill` (toute la place, quelle que soit sa forme). Le rendu met la page à l'échelle de sa boîte en
  gardant les proportions du format (`dashdef/Renderer.tsx`) ; l'unité `--u` des blocs vient de la taille
  du dessin, donc un tableau de bord est le même, plus grand ou plus petit, sur tout écran.
- **Grille** par page (24 × 12 par défaut, de 4 × 2 à 48 × 32). Les éléments sont placés en cases
  entières ; ils peuvent se chevaucher (l'ordre de la liste est l'ordre de dessin).
- **Thème** : six couleurs `#rrggbb`.

### B. Catalogue d'éléments et de champs

- **Blocs de Corde** (`dash/blocks/`) : les quatre pages de la 1.1.0 ont été découpées en blocs que les
  pages réassemblent à l'identique (captures avant / après) et que le Studio propose un par un : rampe de
  régime et drapeaux, rapport et vitesse, position, tours, écart au meilleur tour, tour en cours,
  secteurs, écarts devant / derrière, bandeau voiture, mini-pneus, pneu d'un coin, fenêtre / répartition,
  freins, carburant, énergie virtuelle, hybride, session, météo, tableau des secteurs, derniers tours,
  autour de moi. Plus les **quatre pages entières**.
- **Éléments génériques** : Valeur (étiquette + donnée du catalogue, décimales, unité, couleur automatique
  ou fixe, alignement, cadre), Barre / jauge (donnée de 0 à 100 %, horizontale ou verticale), Texte,
  Panneau / séparateur, Heure / temps de session.
- **Widgets du PC** (carte, classement, entrées…) par un adaptateur, avec leurs options.
- **Champs** (`dashdef/fields.ts`) : une soixantaine de données du protocole (voiture, commandes, chronos,
  course, session, carburant et énergie, électronique, pneus et freins par coin, météo), chacune avec son
  libellé FR / EN, son groupe, sa lecture dans la trame, son format, son unité et sa couleur de sens
  (plus rapide / plus lent, froid / chaud, niveau bas…). **Ni formule ni script.** Les noms du catalogue
  sont écrits dans le catalogue lui-même en FR et EN (les widgets gardent leurs clés de traduction).
- Chaque élément a un réglage commun « Taille du texte » (× 0,5 à × 3).
- **« Corde »** est une définition fournie, en lecture seule (`dashdef/builtin.ts`, id réservé `corde`) :
  quatre pages, chacune une page entière, format `fill`. Une telle page est dessinée exactement comme en
  1.1.0 (`wholePageType`). « Dupliquer pour le modifier » en fait une copie modifiable ; le modèle
  « Corde en blocs » donne les mêmes pages découpées en blocs, à réarranger.

### C. Le Studio (PC)

`#/dash-studio` (bibliothèque) et `#/dash-studio/<id>` (édition), dans l'application de bureau et le
navigateur, pensé pour la souris et le clavier ; pas proposé sur un téléphone. Bibliothèque : nouveau (vide,
« Corde en blocs », « Endurance »), dupliquer, renommer, supprimer, utiliser sur cet appareil, importer,
exporter. Édition : palette (clic = première place libre, glisser = à l'endroit), page aimantée au format
choisi (glisser, coin pour redimensionner), clavier (flèches, Maj + flèches, Suppr, Ctrl + D, Échap,
Ctrl + Z / Ctrl + Y, Ctrl + S), propriétés à droite, onglets des pages (ajouter, renommer, déplacer,
supprimer), course de démonstration ou données en direct, aperçu plein écran, enregistrement explicite,
**brouillon** gardé sur l'appareil à chaque changement (repris à la réouverture).

### D. Utilisation

Chaque appareil choisit son tableau de bord (menu des dispositions du Direct sur PC, menu ☰ du tableau de
bord, Réglages « Tableau de bord utilisé »), retenu dans le stockage de l'appareil. Définition absente ou
invalide → « Corde ». Chaque appareil garde une **copie bornée** (20 définitions, 2 Mio) de la dernière
liste reçue du PC, pour afficher son tableau de bord quand le PC est éteint ; la démonstration marche avec
n'importe lequel.

### E. Sur PC

Le menu des dispositions du Direct a un groupe **Tableaux de bord** (« Corde » et ceux du Studio, lien vers
le Studio) ; le tableau de bord s'affiche **dans la page**, sous la barre du haut, plein écran sur demande
(révision de l'ADR 0014).

### F. Stockage et routes

Sur le PC, un fichier JSON par définition dans `<données>/dashes/<id>.json` (`corde-server/src/dashes.rs`,
calqué sur `layouts.rs`, écriture atomique). Routes `/api/dashes` : `GET` liste, `GET / PUT / DELETE
/api/dashes/{id}`, `DELETE /api/dashes`. **Pas de remplacement complet par `PUT /api/dashes`** (qui
permettrait de supprimer depuis n'importe quel appareil). Droits : **exactement ceux des dispositions de
widgets** — lecture et enregistrement ouverts à tout appareil autorisé à joindre le serveur, suppression
depuis ce PC seulement (administrateur sur le serveur Corde). Voir `docs/SECURITY-MODEL.md`.

### G. Validation stricte, des deux côtés

Les mêmes règles côté serveur (`dashes::validate`) et interface (`dashdef/model.ts`), éprouvées par les
mêmes cas : ≤ 256 Kio (JSON compact), ≤ 50 définitions, ≤ 12 pages, ≤ 80 éléments par page,
identifiants `[a-z0-9-]{1,40}` (choisis par Corde, `corde` réservé), types `[a-z0-9.-]{1,40}`, noms
≤ 60 caractères (pages ≤ 40), réglages : objet plat de ≤ 32 clés `[A-Za-z][A-Za-z0-9_]{0,31}`, valeurs
chaîne ≤ 200 caractères, nombre fini |n| ≤ 10⁶, booléen, nul ou liste de ≤ 16 de ces valeurs ; couleurs
`#rrggbb` (ou `auto` pour les réglages de couleur). Aucune chaîne ne peut contenir `<`, `>`, de caractère
de contrôle, `://`, `www.`, ni commencer par `javascript:`, `data:`, `vbscript:`, `file:` : **pas de HTML,
pas d'adresse, pas d'image, pas de script**. Les types d'éléments et les réglages inconnus sont **conservés**
et dessinés « élément inconnu » (définition d'une version plus récente). L'interface n'injecte jamais de
HTML : React insère chaque texte comme texte, chaque couleur passe par `validColor` avant un `style`.

### H. Partage par fichier

`Exporter` écrit `<nom>.cordedash.json` : la définition seule — ni nom d'appareil, ni adresse de serveur,
ni chemin ; l'auteur seulement s'il a été saisi. `Importer` applique la même validation, refuse avec un
message clair (« Fichier refusé : … »), et donne un nouvel identifiant si le sien est pris (ou `corde`).

### I. Plus tard (hors de la 1.2.0)

- images et logos ;
- placement libre au pixel (la grille reste la règle) ;
- formules, conditions d'affichage (« montrer si… »), pages automatiques ;
- polices personnalisées ;
- galerie en ligne de tableaux de bord ;
- affichage sur l'écran intégré d'un volant : Corde n'écrit pas sur l'écran du volant, qui reste piloté par
  le logiciel de son fabricant.

## Conséquences

- La 1.1.0 continue de s'afficher pareil ; les pages sont maintenant des assemblages de blocs.
- Les définitions vivent sur le PC ; le téléphone les reçoit à la connexion et en garde une copie.
- Ajouter un élément = un bloc + une entrée du catalogue ; ajouter une donnée = une entrée de `fields.ts`.
- Une définition faite par une version plus récente reste lisible (éléments inconnus conservés), une
  définition d'un autre format (`v ≠ 1`) est refusée.
