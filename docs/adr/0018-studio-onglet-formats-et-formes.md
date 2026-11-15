# ADR 0018 — Studio : onglet dans la barre du haut, formats d'écran et forme de la page

- **Statut** : accepté (novembre 2026 ; le numéro de version est fixé à la fusion), complète les
  [ADR 0015](0015-studio-tableaux-de-bord.md) et [0016](0016-studio-medias-et-personnalisation.md)
- **Contexte** : demande du propriétaire : « Continue sur le Studio : donne-lui un onglet à part entière
  dans l'en-tête, là où il y a Accueil, Direct, Sessions, Courses, Profil. Et rends possible le changement de
  format du tableau de bord : par exemple certains volants ont un petit écran rond au milieu du volant.
  Sinon, pouvoir faire la taille et la forme qu'on veut. » Le format se changeait déjà, mais seulement dans
  le panneau de droite de l'éditeur, quand rien n'était choisi : il ne l'a pas trouvé. Et une page était
  toujours un rectangle. Les règles de fond restent : une définition est une donnée inerte, aucune requête
  nouvelle, modèle d'accès inchangé.

## Décision

### A. Le Studio est une section de l'application

- **Un sixième onglet « Studio »** dans la barre du haut (`dashboard/TopBar.tsx`), après Profil. Les routes
  ne changent pas (`#/dash-studio`, `#/dash-studio/<id>`) ; `Section` gagne `studio` et `sectionOf` le rend
  pour ces routes (elles étaient rangées sous « Direct »).
- **Deux barres, l'une sous l'autre** : la barre de l'application (logo, onglets, état, plein écran,
  réglages), puis la barre du Studio (bibliothèque : titre, Nouveau, Importer ; éditeur : Bibliothèque, nom,
  Format, Annuler, Rétablir, Course de démonstration, Aperçu plein écran, Exporter, Enregistrer). Une seule
  rangée ne tient pas : la barre de l'éditeur occupe déjà toute la largeur à 1280 px. À 1280 × 720, mesuré,
  il reste 760 × 596 px à la page (la barre de l'application en prend 38 de haut). Aucune commande de
  l'éditeur n'est retirée. Seul le lien « ← Direct » de la bibliothèque disparaît : c'est exactement
  l'onglet Direct.
- **Où l'onglet n'est pas montré** : sur un téléphone — son navigateur ou l'application — par la règle qui
  choisit déjà le tableau de bord du téléphone (`useAutoPicksDash` de `dash/view.ts` : l'application, ou un
  écran tactile dont le petit côté fait 500 px au plus), et sur le serveur Corde (rôle hub), qui n'a pas de
  Studio. Sur le serveur Corde, l'adresse `#/dash-studio` montre ce que montre `#/` (« le direct est sur le
  PC »), sous l'onglet Direct ; avant, une page vide.
- **La barre du haut avec six onglets** : chaque côté prend au moins la place de ce qu'il contient, l'état
  reste au milieu de la fenêtre tant que les deux côtés y tiennent et se décale sinon. Sous 1180 px de
  large, seul l'onglet courant garde son libellé (les autres leur icône, leur nom pour les lecteurs d'écran
  et leur infobulle) ; sous 900 px, les boutons de droite perdent le leur ; le nom de la disposition puis le
  texte de l'état sont coupés en dernier. Mesuré
  avant ce changement : avec six onglets, le nom de la disposition disparaissait à 1100 px et les onglets
  passaient sur l'état à 1024 px.
- **Navigation automatique inchangée** (`nav/auto.ts`) : ses règles lisent la page (`route.page`), pas la
  section. Dans le Studio comme avant : entrer en piste ouvre le Direct (si l'option est active, et 15 s
  après un changement de page fait à la main), rentrer au garage n'ouvre jamais le résumé par-dessus le
  Studio. Des essais le fixent.

### B. La forme de la page : `format.shape` et `format.radius`

```text
format: { preset, width, height, shape?, radius? }
```

- **`shape`**, d'une **liste fermée** : `rect` (rectangle), `rounded` (rectangle à coins arrondis), `round`
  (le cercle, ou l'ellipse, inscrit dans `width` × `height`). **Absent : rectangle.**
- **`radius`** : le rayon des coins, entier, en **pixels de la taille de référence** (la même unité que
  `width` et `height`, celle qu'on lit sur la fiche d'un écran), de **1 à la moitié du petit côté**.
  Obligatoire avec `rounded`, refusé avec toute autre forme.
- **« Tout l'écran » (`fill`) n'a pas de forme** : la page y prend celle de l'écran ; `fill` avec `rounded`
  ou `round` est refusé.
- La forme vaut pour **tout le tableau de bord** (toutes ses pages) : c'est celle de l'écran.
- Les bornes de la taille ne changent pas : côtés de 100 à 4000, proportions de 8:1 au plus. « N'importe
  quelle taille » se règle donc dans ces bornes, avec le format `custom`.
- **Mêmes règles des deux côtés** (`dashes::check_format` et `format()` de `dashdef/model.ts`), éprouvées
  par les **mêmes cas** (`page_shapes_are_a_closed_list_with_a_bounded_radius`, `shapes_in_json`, et leurs
  jumeaux de `dashdef/format.test.ts`) : forme hors liste, rayon de 0, rayon au-delà de la moitié, rayon sans
  coins arrondis, coins arrondis sans rayon, valeur du mauvais type, `fill` avec une forme.
- Ni chemin, ni polygone, ni chaîne CSS, ni image de masque : un mot d'une liste et un nombre borné.

### C. Compatibilité

- **`v` reste 1.** Les deux champs sont facultatifs et omis quand ils sont absents : une définition
  existante reste valide et se **réécrit à l'identique** (essai des deux côtés).
- **Les structures refusent-elles un champ inconnu ? Non**, ni côté serveur (serde sans
  `deny_unknown_fields`) ni côté interface (`validateDefinition` rend une copie propre, avec les seules clés
  connues). Donc :
  - **un lecteur plus ancien** (application du téléphone pas encore à jour, serveur Corde plus ancien) qui
    reçoit une définition avec une forme **l'ignore** : la page est dessinée en rectangle `width` × `height`,
    entière, sans rognage. Rien n'est refusé ; sur un écran rond, c'est l'écran qui coupe les coins. Un nom
    de préréglage inconnu (`wheel-round`…) était déjà accepté et montré « Personnalisé » ;
  - **une version plus ancienne qui enregistre** (son Studio, ou un serveur plus ancien qui reçoit un
    `PUT`) **perd la forme** : la copie propre ne la connaît pas. C'est la règle de l'ADR 0016 §H. Sur le PC,
    l'interface et le serveur sont toujours de la même version ;
  - **un lecteur à jour** lit une définition sans forme comme un rectangle.
- **Limite assumée** : la liste est fermée, comme celle des polices. Une forme ajoutée plus tard sera
  **refusée** par cette version (l'appareil retombe sur « Corde »). Une future forme devra en tenir compte.
- **Préréglages** : la liste est écrite des deux côtés (`PRESETS`), mais un préréglage n'est qu'un **nom**
  pour une taille et une forme. La validation n'en dépend pas : un nom inconnu reste accepté et dessiné à sa
  taille, comme avant. La fermer aurait invalidé des définitions existantes et empêché cette version de lire
  les préréglages des suivantes.

### D. Rendu : rien hors de la forme

- `Stage` (`dashdef/Renderer.tsx`) pose sur le dessin `data-shape` et `--shape-r` : `50%` pour `round`,
  `a% / b%` pour `rounded` (le rayon rapporté à la largeur et à la hauteur : le même arrondi quelle que soit
  la taille du dessin). La feuille de style en fait un `border-radius` et un `clip-path: inset(0 round …)` —
  les deux, parce que certains moteurs laissent dépasser une vidéo d'un simple `overflow` arrondi.
- Tout ce qui est dessiné est **dans** cette boîte : les éléments, le fond de page, son image, sa vidéo.
  **Autour de la forme : noir** (et non la couleur de fond du thème).
- Tous les endroits passent par `Stage` : tableau de bord du téléphone, `#/dash`, page Direct du PC,
  vignettes de la bibliothèque, aperçu plein écran.
- **Dans l'éditeur, rien n'est rogné** : la page entière reste visible, l'extérieur de la forme est
  assombri et **hachuré**, la forme tracée en pointillés (`ShapeMask`, un SVG au-dessus des éléments, sous
  les poignées). On peut poser un élément n'importe où.
- **Plein écran avec une souris** (un PC et ses écrans) : un tableau de bord du Studio qui a un format à
  lui prend toute la fenêtre, sans marge ; les points des pages n'apparaissent qu'au survol, comme ☰ et les
  flèches. Un tableau de bord 480 × 480 remplit ainsi exactement un écran 480 × 480. Sur un écran tactile,
  les marges des boutons restent ; dans la page Direct du PC aussi.
- **Corrigé en chemin** : les règles « portrait » des pages de Corde (`@container dpage (orientation…)`)
  s'appliquaient aux blocs d'un tableau de bord du Studio dès que l'écran était plus haut que large — ou
  **carré**, donc sur un écran rond — et pas dans l'éditeur : le rapport passait à côté de la vitesse sur
  l'écran, dessous dans l'éditeur. Une page qui contient un dessin ne s'appelle plus `dpage`
  (`.dash-page:has(> .dstage-wrap)`) : le dessin est le même partout.

### E. Formats proposés

Téléphone en paysage 852 × 393, téléphone en portrait 393 × 852, tablette 4:3 1024 × 768, écran 16:9
1920 × 1080, bandeau large de volant 800 × 240 — et quatre nouveaux : **écran rond de volant 480 × 480
(rond)**, **écran carré 720 × 720**, **petit écran 5:3 800 × 480**, **petit écran 3:2 480 × 320** ; plus
« Tout l'écran » et « Personnalisé » (taille et forme libres). Les libellés disent un genre d'écran, une
taille, une forme : jamais une marque ni un modèle.

### F. Studio : le bouton « Format »

- Dans la barre de l'éditeur, **« Format »** dit le format en clair (« 852 × 393 », « Rond 480 × 480 »,
  « Coins arrondis 800 × 480 », « Tout l'écran ») et ouvre un panneau : les préréglages en vignettes à leur
  forme, largeur et hauteur, un bouton qui les inverse, la forme, le rayon des coins. Le panneau de droite
  n'a plus de liste de formats : il dit le format et renvoie au bouton (« Changer le format… »).
- **Nouveau** (bibliothèque) ouvre une fenêtre : modèle de départ **et** format, les mêmes champs.
- Au clavier : le bouton ouvre, le focus va au format en cours, Échap ferme et rend le focus au bouton ;
  chaque champ a son libellé, la forme est un groupe de boutons radio.
- **Annuler** : chaque changement est une étape ; une taille tapée (ou un rayon réglé) fait **une** étape,
  du champ pris au champ quitté. Ce qui est tapé s'applique dès que c'est une taille valide, et s'applique
  au format d'avant la saisie — taper « 1000 » passe par « 100 », qui sans cela réduirait le rayon pour de
  bon. Une taille hors proportions est refusée et dite.

### G. Changer de format garde les éléments

- Les éléments sont placés en cases : la page garde sa composition, dessinée aux nouvelles proportions.
  **Rien n'est déplacé, réduit ni supprimé**, et rien n'est changé en silence.
- Quand les proportions changent beaucoup (paysage → carré), les cases ne sont plus carrées (24 × 12 sur
  un carré : deux fois plus hautes que larges). Au-delà d'un rapport de 1,6 entre largeur et hauteur d'une
  case, le panneau **le dit** et **propose** « Adapter la grille » : les colonnes ou les lignes sont
  **coupées en 2, 3 ou 4** (dans la limite de 48 × 32) et les cases de chaque élément multipliées
  d'autant — le dessin est exactement le même, la grille plus fine. Une page vide prend la grille de son
  format. Si aucune grille plus fine n'aide, le panneau l'explique et renvoie à « Grille de cette page ».
- À la **création** dans un format, la grille des pages du modèle est adaptée d'emblée : il n'y a encore
  aucun travail de l'utilisateur.

### H. L'afficher sur un petit écran

Rien de nouveau, vérifié : sur l'écran voulu, une fenêtre de navigateur à l'adresse `…#/dash`, en plein
écran ; le tableau de bord se choisit dans ☰ → « Tableau de bord utilisé » et reste retenu dans ce
navigateur. Essayé avec un navigateur sans tête : un tableau de bord rond 480 × 480 remplit exactement une
fenêtre 480 × 480 (coins noirs) ; dans une fenêtre 800 × 480 il est centré, noir autour.

## Alternatives écartées

- **Un rayon seul, sans forme** : ne sait pas dire l'ellipse d'un écran qui n'est pas carré, et confond
  « très arrondi » et « rond ».
- **Un rayon en pourcentage** : un écran s'annonce en pixels ; le rendu fait la conversion.
- **Une chaîne CSS, un tracé SVG, une image de masque** : plus souple, mais ce n'est plus une donnée inerte
  et bornée.
- **Une forme par page** : un écran n'a qu'une forme.
- **Passer `v` à 2** : les lecteurs plus anciens auraient refusé tout le tableau de bord, au lieu de le
  dessiner en rectangle.
- **Remettre la composition à l'échelle** au changement de format (la garder « non déformée », centrée) :
  il faudrait arrondir des cases, donc bouger des éléments ; la grille plus fine ne bouge rien.
- **Rogner aussi dans l'éditeur** : on ne verrait plus ce qu'on a posé hors de la forme.
- **Une seule rangée** pour les onglets et la barre de l'éditeur : pas la place à 1280 px.

## Conséquences

- Hors de ce changement, dit plutôt que construit : choisir le tableau de bord **par fenêtre** (deux
  fenêtres du même navigateur partagent le choix ; un second navigateur, ou l'application de bureau et un
  navigateur, ont chacun le leur), ouvrir une seconde fenêtre depuis l'application de bureau, écrire sur
  l'écran intégré d'un volant (toujours exclu, ADR 0015 §I).
- Les images `studio-edition.png`, `studio-image.png` et `studio-couleurs.png` montrent l'éditeur d'avant
  l'onglet.
- **Non vérifié** : le rognage (surtout d'une vidéo de fond) dans Safari et sur iPhone ; un vrai écran rond.
