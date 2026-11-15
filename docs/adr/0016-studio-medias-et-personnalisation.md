# ADR 0016 — Studio : images, vidéos et personnalisation

- **Statut** : accepté (Corde 1.3.0, novembre 2026), complète l'[ADR 0015](0015-studio-tableaux-de-bord.md)
- **Contexte** : retour du propriétaire sur le Studio de la 1.2 : « c'est pas mal, mais je veux que ce soit
  plus personnalisable : par exemple rajouter une photo ou une vidéo, ou d'autres choses ». L'ADR 0015
  excluait les images, les conditions d'affichage et les polices. Les règles de fond restent : aucune
  requête sortante nouvelle, aucune adresse ni script dans une définition, modèle d'accès inchangé.

## Décision

### A. Médias : seulement des fichiers du PC, cités par leur empreinte

- Éléments **Image** et **Vidéo** (réglage `asset` + ajustement `contain` / `cover` / `fill`) et **fond de
  page** (`DashPageDef.background` : couleur, média, ajustement, opacité).
- **Formats** : images PNG, JPEG, WebP, GIF (animé accepté) ; vidéos MP4 (MOV accepté : même famille de
  fichiers) et WebM. Pas de SVG (il peut porter un script), pas de son. Le type est lu dans les **octets de
  signature**, des deux côtés (`dash_assets::sniff`, `media.ts::sniff`), jamais dans l'extension ni le
  `Content-Type`.
- **Stockage** sur le PC : `<données>/dash-assets/<sha256>.<ext>`. L'identifiant (`[0-9a-f]{64}.(png|jpg|
  webp|gif|mp4|webm)`) est le nom du fichier : choisi par le serveur, jamais un chemin ni un nom de
  périphérique de Windows ; un même contenu n'est gardé qu'une fois. Limites : image ≤ 4 Mio, vidéo
  ≤ 30 Mio, 200 fichiers et 300 Mio.
- **Routes** `/api/dash-assets` : `GET` (liste, un média) ouvert comme `GET /api/dashes` ; `POST` (le corps
  est le fichier) et `DELETE` **depuis ce PC seulement** (administrateur sur le serveur Corde). Réponse avec
  son type, `nosniff`, `CSP: sandbox`, cache immuable, `ETag`, **plages d'octets** (indispensables à la
  lecture vidéo d'un iPhone). Voir `docs/SECURITY-MODEL.md` §5 bis-3.
- **Ménage** : un média qu'aucune définition ne cite est supprimé (démarrage, enregistrement et suppression
  d'un tableau de bord) **après un jour** : le Studio envoie le fichier dès qu'il est choisi, la définition
  qui le cite n'est enregistrée qu'ensuite (Ctrl + S), et un brouillon peut le citer longtemps. Sans ce
  délai, un ménage déclenché entre les deux effacerait une image tout juste choisie.

### B. Vie privée des fichiers choisis

- Une **photo** est redessinée dans un canvas du navigateur et encodée à nouveau (même type quand le
  navigateur sait l'écrire, sinon PNG / WebP / JPEG), orientation appliquée, **2560 px** au plus : il ne
  reste aucune métadonnée (EXIF, GPS, appareil, date). Un **GIF** est gardé tel quel (son animation ;
  il ne porte pas d'EXIF).
- Un **MP4 / MOV** : les boîtes `udta`, `meta` et XMP (`uuid` d'Adobe) du premier niveau, de `moov` et de
  chaque `trak` sont **renommées `free` et remplies de zéros** — même taille, aucun décalage des
  échantillons (les tables d'offsets restent justes), la vidéo se lit encore. Vérifié sur un MP4 fabriqué
  avec une position (essai Vitest) et sur une vraie vidéo VP9 de ffmpeg envoyée par l'éditeur : fichier
  stocké de même taille, plus de position, décodable par ffmpeg et lu par Chromium. Un **WebM** est gardé tel
  quel ; l'export prévient toujours qu'une vidéo est partagée telle quelle.

### C. Partage : `.cordedash`

Une archive zip faite et lue **dans le navigateur** (fflate, MIT, ~ 8 Kio utiles) : `dashboard.json` + un
`media/<id>` par média cité, stocké sans compression, date fixe. Lecture stricte (`archive.ts`) : taille du
fichier, nombre d'entrées, taille décompressée de chaque entrée et du tout bornées et **vérifiées pendant la
décompression** (par pas de 16 Kio, la taille annoncée n'est pas crue) ; noms inattendus ignorés sans être
décompressés ; signature de chaque média vérifiée ; chaque média repasse par la préparation de B puis par
`POST /api/dash-assets`, et la définition est réécrite avec les nouveaux identifiants. Un `.cordedash.json`
de la 1.2 est toujours lu.

### D. Couleurs et affichage selon les données, sans formule

- **Seuils** (`DashElement.thresholds`, ≤ 4) sur Valeur, Barre (en %) et Jauge circulaire :
  `{op, value, color}`, la première règle qui s'applique gagne, sinon la couleur choisie / automatique.
- **« Afficher quand… »** (`DashElement.show`) sur tout élément : `limiter`, `pits`, `yellow`, `blue`, ou
  `field` + `op` + `value` (un champ du catalogue comparé à un nombre). Évalué à chaque trame et après
  chaque rendu ; un élément caché garde sa place ; dans l'éditeur il reste visible en transparence.
- Opérateurs d'une liste fermée (`lt`, `le`, `ge`, `gt` — pas de `<` dans une définition), nombres finis
  bornés, champ `[a-z0-9_]{1,40}` (un champ inconnu de l'interface ne s'applique jamais). Validés des deux
  côtés, mêmes cas d'essai.

### E. Style et polices

- `DashElement.style` : fond, opacité, rayon des coins (0–40), contour (couleur, 0–8), dans l'unité du
  dessin (`--su`), indépendants de la taille du texte.
- `DashTheme.font` : `condensed` (Barlow Semi Condensed, défaut), `barlow`, `digital` (Share Tech Mono),
  `wide` (Orbitron) — deux polices libres ajoutées par `@fontsource` (OFL 1.1), **embarquées** dans
  l'application, chargées par le navigateur seulement si un tableau de bord les utilise.

### F. Nouveaux éléments

Jauge circulaire (arc de 270°, valeur au centre, minimum / maximum, maximum 0 du régime = celui de la
voiture), Forme (rectangle, cercle, ligne), Bandeau d'alerte (grand texte, ajouté avec « afficher quand le
limiteur est actif »), Rampe de régime réglable (3 à 30 segments, trois couleurs, couleur de passage,
seuil d'allumage ; limiteur comme la rampe de Corde).

### G. Éditeur

Sélection multiple (Maj + clic, cadre tracé, Ctrl + A), déplacement groupé (tenu dans la grille), repères
d'alignement (bords et centres des autres éléments, milieu de la page), Ctrl + C / Ctrl + V (presse-papiers
de la visite, aussi vers une autre page ou un autre tableau de bord), recherche dans la liste des éléments,
« Abandonner le brouillon ». Sur un tableau de bord du Studio, ☰ (et le plein écran) n'apparaissent qu'au
survol sur PC ; sur écran tactile la page leur réserve leur place.

### H. Compatibilité

`v` reste 1 : tous les champs nouveaux sont facultatifs et omis quand ils sont vides. Une définition de la
1.2 se lit et s'écrit à l'identique. Une définition 1.3 lue par une 1.2 perd ses champs nouveaux (copie
propre) et montre « élément inconnu » pour les types nouveaux.

## Conséquences

- Le tableau de bord sur le téléphone lit ses médias sur le PC : PC éteint, une place vide (la copie bornée
  de l'ADR 0015 ne garde que les définitions). Le cache HTTP du navigateur peut en garder une partie.
- Un serveur Corde (rôle hub) sert les médias aux appareils appairés ; l'application les lit alors par
  `fetch` avec son jeton (un `<img>` ne peut pas porter d'en-tête), sans plages d'octets.
- Toujours pas : formules, placement au pixel, galerie en ligne, son, adresse web.
