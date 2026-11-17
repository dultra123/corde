# Installer Corde sur ton PC (Windows)

> Guide pour le pilote. Compte 5 minutes. Corde est maintenant une vraie application : une icône, une
> fenêtre, plus de console ni de navigateur. Rien de ce que tu fais ici ne modifie le jeu ni tes réglages :
> Corde se contente de **lire** ce que Le Mans Ultimate publie.
> Si quelque chose ne correspond pas à ce qui est écrit, fais une capture d'écran et envoie-la nous avec le
> fichier journal (§ 8) : c'est exactement ce dont on a besoin.

## En bref

1. Activer les « plugins » dans LMU (une case, une fois pour toutes).
2. Télécharger l'installeur sur la page de la **dernière version**.
3. Le lancer, passer l'avertissement de Windows (expliqué plus bas), installer.
4. Ouvrir **Corde** depuis le menu Démarrer, puis lancer le jeu.

---

## 1. Activer les plugins dans Le Mans Ultimate

Depuis la version 1.2 (décembre 2025), le jeu publie lui-même sa télémétrie, **à condition** que l'option
suivante soit activée :

1. Lance LMU.
2. `Settings` (Paramètres) → `Gameplay` → **`Enable Plugins`** : mets-le sur **ON**.
3. **Quitte complètement le jeu et relance-le.** Sans ce redémarrage, rien n'est publié.

Le fichier `rFactor2SharedMemoryMapPlugin64.dll` déjà présent dans ton dossier `plugins\` vient d'un autre
outil (SimHub, Crew Chief…). Il est inoffensif et Corde n'en a pas besoin : tu peux le laisser.
L'option `Automatically Record Telemetry` peut rester désactivée : Corde enregistre elle-même.

---

## 2. Télécharger l'installeur

1. Ouvre la page de la dernière version :
   **<https://github.com/dultra123/corde/releases/latest>**.
2. Tout en bas, section **Assets**, clique sur le fichier dont le nom finit par **`-setup.exe`**
   (par exemple `Corde_1.8.1_x64-setup.exe`).
3. Ton navigateur peut hésiter : Edge affiche parfois « … n'est pas couramment téléchargé ». Clique sur
   les **…** à côté du fichier → **Conserver** → **Conserver quand même**.

L'autre fichier Windows, `corde-…-portable-windows.zip`, est l'ancien mode console : tu n'en as pas besoin (voir
l'annexe).

---

## 3. Installer

1. Double-clique sur le fichier téléchargé.
2. **Windows affiche un écran bleu « Windows a protégé votre ordinateur ».** Clique sur
   **Informations complémentaires**, puis sur le bouton **Exécuter quand même** qui apparaît.
3. Suis l'installeur (Suivant → Installer → Terminer). Il ne demande **pas** de droits administrateur.

**Pourquoi cet avertissement, honnêtement.** Windows (SmartScreen) affiche cet écran pour tout programme
téléchargé dont il ne connaît pas l'éditeur. Corde n'est pas encore *signé numériquement* — une signature
s'achète auprès d'un organisme, ce n'est pas décidé pour l'instant. Ce n'est **pas** une détection de virus,
et Corde ne fait rien de plus que ce qui est décrit ici. L'avertissement reviendra **à chaque nouvel
installeur** que tu téléchargeras, tant que Corde n'est pas signé ; une fois installé, Corde se lance sans
avertissement. Les solutions et leur prix sont détaillés dans
[l'ADR 0007](adr/0007-windows-smartscreen-signature.md).

Ce que fait l'installeur :

| Quoi | Où |
|---|---|
| Le programme | `%LOCALAPPDATA%\Programs\Corde\` (c'est-à-dire `C:\Users\<toi>\AppData\Local\Programs\Corde\`) |
| Un raccourci **Corde** | menu Démarrer (et le bureau si tu coches la case à la fin) |
| Le moteur d'affichage WebView2 de Microsoft | déjà présent sur Windows 11 ; téléchargé automatiquement sinon |

Il ne touche ni au jeu, ni à tes données Corde existantes (§ 7).

---

## 4. Premier lancement

1. Menu Démarrer → **Corde**. Une fenêtre sombre « Démarrage de Corde… » apparaît une seconde ou deux,
   puis l'**écran d'accueil**.
2. Windows ne pose **aucune question de pare-feu** : par défaut, Corde n'est joignable que depuis ton PC.
3. Lance LMU. Quand tu **entres en piste**, le tableau de bord s'affiche tout seul. Si la fenêtre de Corde
   était fermée ou réduite, elle réapparaît à ce moment-là, là où tu l'avais laissée (ton second écran par
   exemple) ; dans tous les cas Corde ne passe jamais devant le jeu et ne lui prend pas le clavier.

### Fermer la fenêtre ne quitte pas Corde

Quand tu fermes la fenêtre (la croix), Corde **reste active** : elle continue d'attendre le jeu et
d'enregistrer tes sessions. Son icône — un **M** orange — est dans la zone de notification, près de
l'horloge (Windows la range souvent derrière la petite flèche **^** : tu peux la faire glisser à côté de
l'horloge pour la garder visible). La première fois, une notification te le rappelle.

| Geste sur l'icône | Effet |
|---|---|
| Clic gauche | rouvre la fenêtre |
| Clic droit → **Ouvrir Corde** | rouvre la fenêtre |
| Clic droit → **Dashboard** | rouvre la fenêtre sur le tableau de bord |
| Clic droit → **Quitter** | arrête vraiment Corde (l'enregistrement en cours est terminé proprement) |

Laisser le pointeur sur l'icône indique où en est Corde : « en attente du jeu », « jeu détecté »,
« en piste ».

Relancer Corde depuis le menu Démarrer alors qu'elle tourne déjà rouvre simplement sa fenêtre : il n'y a
jamais deux Corde en même temps.

Sur la page **Ingénieur** d'une session, « Ton ressenti » est interprété **hors-ligne** (dictionnaire
intégré, rien n'est envoyé sur Internet) : tu vois les symptômes compris et le plan ajusté en 1 à 3
changements. Un modèle de langage est possible en option, expérimental, en définissant la variable
d'environnement `CORDE_ANTHROPIC_API_KEY` (clé d'API Anthropic ; en mode console, `--llm none` le
coupe même si la variable existe) ; sans réponse en 8 s, Corde repasse sur le dictionnaire.

---

## 5. Démarrer Corde avec Windows (facultatif)

Dans les **Réglages** de Corde, active « Démarrer avec Windows ». Au prochain démarrage du PC, Corde se
lance **sans fenêtre**, seulement l'icône près de l'horloge, et la fenêtre apparaît quand tu entres en
piste (ou quand tu cliques sur l'icône).

Pour l'arrêter : le même réglage, ou `Gestionnaire des tâches` → `Applications de démarrage` → Corde →
Désactiver.

---

## 6. Le tableau de bord sur ton téléphone (facultatif)

Par défaut c'est désactivé, pour que Windows ne te demande rien. Pour l'activer :

1. Dans les **Réglages** de Corde, active l'accès depuis le réseau local (téléphone), puis **redémarre
   Corde** quand il le propose.
2. Windows affiche alors, une seule fois, « Le Pare-feu Windows Defender a bloqué certaines
   fonctionnalités de cette application ». Coche **Réseaux privés** et clique **Autoriser l'accès**
   (Windows demande une confirmation administrateur). Si tu cliques sur Annuler, Corde marche toujours sur
   le PC mais le téléphone ne pourra pas se connecter (dépannage, point 4).
3. L'écran d'accueil de Corde affiche maintenant, dans le bloc **Sur ton téléphone**, l'adresse à ouvrir
   et un **QR code**. Ils sont **floutés par défaut** (pratique en direct ou pour une capture d'écran) :
   clique **Afficher l'adresse** pour les voir, **Masquer l'adresse** pour les cacher à nouveau. Ils
   redeviennent masqués à chaque retour sur l'accueil.
4. Sur le téléphone, **connecté au même Wi-Fi**, scanne le QR code avec l'appareil photo (ou tape
   l'adresse affichée dans le navigateur, par exemple `http://192.168.1.20:8471` ; c'est bien `http`,
   pas `https`).

### Le téléphone (ou une tablette) comme tableau de bord

Sur un téléphone, la page **Direct** n'est plus la grille de widgets mais un **tableau de bord de course**,
comme sur l'écran d'un volant de course : grand rapport, rampe de régime, vitesse, écart au meilleur tour,
carburant, et trois autres pages (pneus et freins, carburant et énergie, chronos et écarts) qu'on fait
glisser du doigt. Pose le téléphone **en paysage** à côté du volant : l'écran reste allumé tant que le
tableau de bord est affiché (si le navigateur le permet ; sinon, règle la mise en veille du téléphone).

- **Une tablette ou un second écran du PC** : ouvre l'adresse du téléphone suivie de **`#/dash`** (par
  exemple `http://192.168.1.20:8471/#/dash`, ou `http://127.0.0.1:8471/#/dash` sur le PC lui-même) : c'est
  toujours le tableau de bord, quel que soit l'appareil. Sur le PC, les flèches ← → changent de page.
- **Le tableau de bord sur le PC** : dans le Direct, ouvre le menu des dispositions (le bouton avec le nom
  de la disposition, en haut à gauche) et choisis **Tableaux de bord → Corde**. Le tableau de bord prend la
  place des widgets, la barre du haut reste ; le bouton **Plein écran** (en haut à droite du tableau de
  bord, à côté de ☰) l'agrandit, **Échap** ou le même bouton le remet dans la page. Pour revenir aux
  widgets, choisis une disposition (« Course »…) dans le même menu. Le choix est retenu sur ce PC.
- **Préférer les widgets sur le téléphone** : le menu **☰** du tableau de bord propose « Passer aux
  widgets » ; le bouton **Tableau de bord** de la barre (ou le menu des dispositions) y ramène. Le réglage
  **Affichage du Direct** (Automatique / Tableau de bord / Widgets) est le même état que ce menu :
  « Automatique » choisit le tableau de bord sur un téléphone et les widgets ailleurs.
- **L'essayer sans le jeu** : `http://127.0.0.1:8471/?demo=1#/dash` montre une course de démonstration.

---

### Créer ton propre tableau de bord (Studio)

Le **Studio** est l'éditeur de tableaux de bord de Corde, sur le PC (à la souris et au clavier).

1. **Ouvre-le** : l'onglet **Studio** de la barre du haut, à droite de Profil (ou Direct → menu des
   dispositions → **Studio : créer un tableau de bord…**, ou Réglages → « Ouvrir le Studio », ou ☰ du tableau
   de bord). Les autres onglets en font sortir. Il n'y a pas d'onglet Studio sur un téléphone.
2. **Nouveau** : choisis le **modèle de départ** — **Vide**, **Corde en blocs** (les quatre pages de Corde
   découpées, à réarranger) ou l'exemple **Endurance** — et le **format** (voir le point suivant), puis
   **Créer**. Le tableau de bord **Corde** lui-même ne se modifie pas : **Dupliquer pour le modifier**.
3. **Format** : le bouton **Format** de la barre de l'éditeur, à droite du nom, dit le format en cours
   (« 852 × 393 », « Rond 480 × 480 »…) et ouvre le panneau. Tu y trouves :
   - les **formats courants** : Téléphone en paysage (852 × 393), Téléphone en portrait (393 × 852),
     Tablette 4:3 (1024 × 768), Écran 16:9 (1920 × 1080), Bandeau large de volant (800 × 240), **Écran rond
     de volant** (480 × 480, rond), Écran carré (720 × 720), Petit écran 5:3 (800 × 480), Petit écran 3:2
     (480 × 320), Tout l'écran, Personnalisé ;
   - la **taille** : **Largeur** et **Hauteur**, de 100 à 4000 pixels, 8 fois plus large que haut au
     maximum (ou l'inverse), et le bouton **Inverser largeur et hauteur** ;
   - la **forme de la page** : **Rectangle**, **Coins arrondis** (avec le **rayon des coins**) ou **Rond**.

   La page garde ces proportions sur n'importe quel écran. Avec une forme, rien n'est dessiné en dehors :
   dans l'éditeur, la zone hachurée montre ce qui sera caché (tu peux quand même y poser des éléments).
   Changer de format **garde tes éléments** ; si les cases de la grille deviennent très allongées (d'un
   format paysage à un carré, par exemple), le panneau le dit et propose **Adapter la grille** : aucun
   élément ne bouge. **Annuler** (Ctrl + Z) revient au format d'avant.
4. **Ajoute des éléments** depuis la liste de gauche : un clic les pose à la première place libre, ou fais
   les glisser sur la page. **Valeur** affiche une donnée au choix (vitesse, carburant, écart, pression d'un
   pneu… une soixantaine), **Barre / jauge** une donnée de 0 à 100 % (accélérateur, frein, réservoir,
   usure…) ; il y a aussi du texte, des panneaux, l'heure, tous les blocs des pages de Corde, et les widgets
   du PC.
5. **Place-les** : glisse un élément pour le déplacer, son coin orange pour le redimensionner ; tout
   s'aimante sur la grille. Au clavier : flèches (déplacer), Maj + flèches (redimensionner), Suppr, Ctrl + D
   (dupliquer), Ctrl + Z / Ctrl + Y (annuler / rétablir).
6. **Règle-les** à droite : donnée, étiquette, décimales, couleur (automatique : verte quand tu es plus
   rapide, rouge quand tu es plus lent…), taille du texte, cadre.
7. **Pages** : onglets en haut (+ Page, renommer, déplacer, supprimer).
8. **Essaie-le** : **Course de démonstration** anime la page sans le jeu ; **Aperçu plein écran** le montre
   comme sur le téléphone (Échap pour revenir).
9. **Enregistrer** (ou Ctrl + S) : il est enregistré sur ce PC. Tant que tu n'as pas enregistré, tes
   changements restent en brouillon sur ce PC et sont repris quand tu reviens.
10. **Utilise-le** : sur le PC, dans le menu des dispositions (groupe **Tableaux de bord**) ; sur le
    téléphone, dans le menu ☰ → « Tableau de bord utilisé ». Chaque appareil a le sien.

11. **Plusieurs éléments à la fois** : Maj + clic, ou trace un cadre sur une partie vide de la page. Glisse
    l'un d'eux pour les déplacer ensemble (des repères roses montrent les bords et les centres alignés).
    **Ctrl + C** puis **Ctrl + V** les copie, aussi sur une autre page ou un autre tableau de bord ;
    Ctrl + A choisit toute la page. La case **Rechercher un élément** filtre la liste de gauche.
12. **Brouillon** : quand un brouillon est repris, **Abandonner le brouillon** revient à la version enregistrée.

#### Un petit écran en plus (écran rond de volant, écran de tableau de bord)

Corde affiche un tableau de bord dans une fenêtre : elle n'écrit pas sur l'écran intégré d'un volant, qui
reste piloté par le logiciel de son fabricant. Pour un petit écran branché au PC comme un moniteur :

1. Dans le Studio, crée le tableau de bord au **format de l'écran** (par exemple **Écran rond de volant**,
   480 × 480) et enregistre-le.
2. Ouvre un navigateur sur ce petit écran, à l'adresse **`http://127.0.0.1:8471/#/dash`**, et passe-le en
   plein écran (F11). Ajoute `?autonav=0` avant le `#` (`http://127.0.0.1:8471/?autonav=0#/dash`) pour que
   la page ne change jamais toute seule.
3. Passe la souris sur la page, ouvre **☰** et choisis ton tableau de bord sous **Tableau de bord
   utilisé**. Le choix est retenu dans ce navigateur.

Un tableau de bord dessiné aux dimensions de l'écran le remplit exactement ; sur un écran d'autres
proportions, il est centré, avec du noir autour. Les boutons (☰, flèches, points des pages) n'apparaissent
que quand la souris est sur la page. À savoir : deux fenêtres du **même** navigateur partagent le choix du
tableau de bord ; pour afficher autre chose que l'écran principal, utilise l'application Corde sur l'écran
principal et un navigateur sur le petit écran (ou deux navigateurs différents).

#### Une photo, un logo, une vidéo (1.3)

- **Image** (groupe « Images, formes et jauges ») : pose l'élément, puis **Choisir un fichier…** à droite.
  PNG, JPEG, WebP ou GIF (animé accepté), 4 Mio au plus. Avant l'envoi, Corde **réenregistre la photo** sur
  ton PC : la position GPS, l'appareil, la date et les autres informations cachées dans le fichier sont
  retirées, et la photo est ramenée à 2560 pixels de large au plus. **Ajustement** : contenir (toute
  l'image), couvrir (remplit la case, rogne) ou étirer ; dans **Style** : opacité, coins arrondis.
- **Vidéo** : MP4 ou WebM, 30 Mio au plus, **sans son**, en boucle. Elle ne joue que sur la page affichée et
  s'arrête quand le téléphone est mis en veille ou l'application quittée ; si l'appareil demande « moins
  d'animations », elle reste sur sa première image. D'un MP4, Corde ne garde que les images : le son, la
  position, la date et les données de la caméra sont retirés avant l'envoi (un WebM est gardé tel quel).
- **Fond de la page** : rien de choisi sur la page → panneau de droite, **Fond de cette page** : une
  couleur, et / ou une image ou une vidéo, son ajustement et son opacité. Chaque page a le sien.
- Les fichiers sont gardés **sur ce PC** (dossier des données de Corde, `dash-assets`). Ajouter ou retirer
  une image se fait depuis le PC. Au plus 200 fichiers et 300 Mio ; un fichier qu'aucun tableau de bord
  n'utilise plus est effacé au bout d'un jour.
- Le téléphone les lit sur le PC : **PC éteint, l'image laisse une place vide**, le reste du tableau de bord
  s'affiche.

#### Couleurs selon la valeur, « Afficher quand… », style (1.3)

- **Couleur selon la valeur** (Valeur, Barre, Jauge circulaire) : jusqu'à 4 règles « si la valeur est < 3 →
  rouge ». La première qui s'applique gagne ; sinon, la couleur choisie plus haut. Pour une barre, la
  valeur est en %.
- **Afficher quand…** (tout élément) : toujours, limiteur des stands actif, dans les stands, drapeau jaune,
  drapeau bleu, ou une donnée comparée à un nombre (par exemple « Autonomie carburant < 2 »). Dans
  l'éditeur, un élément caché reste visible en transparence (avec un point bleu sur son cadre).
- **Bandeau d'alerte** : un grand texte coloré, qui arrive réglé sur « le limiteur est actif » (« LIMITEUR »).
  Change le texte et la condition pour « BOX », « CARBURANT »…
- **Style** (tout élément) : fond, opacité, coins arrondis, contour.
- **Tableau de bord** (rien de choisi) : couleurs (dont **Accent** et **Panneaux**) et **police des
  chiffres** (Corde, Barlow, afficheur numérique, large style course).
- Autres éléments : **Jauge circulaire** (vitesse, régime — maximum 0 : celui de la voiture — ou toute
  donnée), **Forme** (rectangle, cercle, ligne), **Rampe de régime réglable** (nombre de segments, couleurs,
  seuil d'allumage).

**Le partager** : **Exporter** (bibliothèque ou éditeur) enregistre un fichier `nom.cordedash` : la mise en
page **et ses images et vidéos** (ni nom de PC, ni adresse, ni données de course ; ton pseudo seulement si tu
l'as mis dans « Auteur »). S'il contient une vidéo, Corde te le rappelle : elle est partagée telle quelle.
Ton ami l'ouvre avec **Importer un fichier…** ; les images y repassent par le même nettoyage, et un fichier
abîmé, trop gros ou piégé est refusé avec la raison. Les fichiers `.cordedash.json` de la 1.2 s'ouvrent
toujours. Ne partage que des images dont tu as le droit.

Pas encore possible (plus tard) : placement au pixel près, formules, galerie en ligne. Corde n'affiche rien
sur l'écran intégré d'un volant (il reste piloté par le logiciel de son fabricant).

## 6 bis. Les courses en ligne et leurs rappels (facultatif)

L'onglet **Courses** (icône drapeau, entre Sessions et Profil) reprend le planning en ligne du jeu et te
prévient avant le départ des courses que tu choisis.

**Activer le planning** (une fois, sur le PC) :

1. Ouvre l'onglet **Courses**. Tant que le planning est désactivé, la page l'explique : Corde lira les pages
   publiques racecontrol.gg et lmu.tools depuis ce PC, au plus toutes les 10 minutes.
2. Clique **Activer le planning**. Les courses apparaissent en quelques secondes, rangées en courses
   quotidiennes, hebdomadaires, événements spéciaux et championnats RaceControl Pro. Ce bouton ne marche
   que sur le PC qui fait tourner Corde (depuis le téléphone, la page te le rappelle) ; pour revenir en
   arrière : **Affichage et heure** → **Ne plus lire le planning sur ces sites**.
3. Facultatif : dans **Mon profil pilote**, recopie ton DR et ton SR (en haut du menu Multijoueur du jeu).
   Les courses qui demandent un rang plus élevé sont marquées « Il te faut SR … ».

**Choisir une course** : touche un horaire doré. Il se remplit, la course apparaît dans **Mes courses** avec
un compte à rebours ; touche-le à nouveau pour l'annuler. Pour un événement spécial, **Me le rappeler**
programme un rappel la veille à 18:00 et le jour même à 10:00. Tes choix sont gardés par Corde : le
téléphone voit les mêmes. L'inscription se fait toujours **dans le jeu** (Multijoueur) : Corde ne
t'inscrit pas.

**Sur tous tes appareils** (depuis la 1.6.0) : l'**application Corde** du téléphone récupère les courses
choisies sur le PC, et le PC celles du téléphone, en moins d'une minute — il suffit que ce PC soit dans
**Mes serveurs** de l'application (Accueil → Mes serveurs). Hors de chez toi, ça passe par ton serveur
Corde (Raspberry Pi ou NAS, à jour lui aussi) et ton VPN. Sous **Mes courses**, l'application indique avec
qui elles sont partagées. Elle ne fait rien en arrière-plan : ouvre-la pour récupérer une course choisie sur
le PC.

**Les rappels** : 15 et 5 minutes avant le départ par défaut (**Rappels avant le départ** : 60, 30, 15, 10,
5, 2 min ou au départ). À l'heure dite, un bandeau apparaît en haut de n'importe quelle page de Corde —
« Départ dans 5 min — LMP3 Fixed, Monza, 21:30 » — avec un son court (plus insistant à 5 minutes et
moins), sur le PC **et** sur le téléphone si Corde y est ouverte.

**Sur l'Accueil** : sous **Ta dernière session**, la carte **Courses en ligne** montre une course à la fois
— d'abord tes courses choisies (marquées « Choisie »), puis les prochains départs du planning, quotidiennes
d'abord, selon tes filtres de niveau et de catégorie (8 au plus). Chaque course a son image (ou un dessin
aux couleurs de sa catégorie), son niveau, ses badges SR / DR, l'heure du départ et un compte à rebours
(« dans 12 min », à la seconde sous une minute). Elle passe à la suivante toutes les 8 secondes ; elle
**s'arrête** quand la souris est dessus ou que tu y navigues au clavier. Flèches et points en dessous,
flèches ← → du clavier, glissement du doigt sur le téléphone. Un clic sur la course ouvre la page
Courses ; **Choisir ce départ** / **Retirer** agit tout de suite. À vérifier : les images des sites
s'affichent (sinon le dessin les remplace, jamais d'image cassée), une course partie disparaît d'elle-même,
et si Windows est réglé sur « Afficher les animations » désactivé, la carte ne défile plus seule. Planning
désactivé : la carte le dit en une phrase, avec un lien **Activer dans Courses**.

**Tester la sonnerie** : sur la page Courses, bouton **Tester la sonnerie**. Fais-le une fois sur chaque
appareil : un navigateur (surtout sur téléphone) n'autorise le son qu'après un premier geste sur la page.
Dans l'application Windows, le son est autorisé d'office. Le son se coupe par appareil (**Son des rappels
sur cet appareil**). Sur le téléphone, la page doit rester ouverte et l'écran allumé : un navigateur en
veille ne reçoit rien.

Si les heures ne correspondent pas à celles du jeu (le site ne dit pas son fuseau horaire, Corde le
devine), corrige-les dans **Affichage et heure** → **Heure des courses quotidiennes** (−2 h à +2 h). Si
un site change sa page et que Corde n'y reconnaît plus aucune course, la page le dit ; envoie le fichier
`%LOCALAPPDATA%\Corde\diagnostics\races\<site>.html` avec ton signalement.

---

## 6 ter. Dans OBS (streamers)

Les widgets de Corde s'affichent par-dessus ton jeu dans OBS, **sur fond transparent**, sans la barre de
Corde ni rien d'autre. Corde doit rester ouverte pendant le stream (la fenêtre peut être cachée).

**Trouver l'adresse** : dans Corde, menu des dispositions du tableau de bord (en haut, à côté des onglets) →
**Adresses pour OBS…**, ou carte **Dans OBS** de l'Accueil. La page liste :

- les **dispositions overlay** : plusieurs widgets placés sur un canevas 1920 × 1080 (une seule source,
  de la taille de ta scène). Corde en propose une, « OBS » (circuit, écarts, classement relatif,
  commandes, pneus) ; « Modifier dans le tableau de bord » l'ouvre sur un damier (le damier sera
  transparent dans OBS), **Modifier** pour déplacer, étirer, ajouter ou retirer des widgets, **Terminer**
  pour enregistrer. Le menu des dispositions → **Nouvelle disposition overlay** en crée une autre ;
- les **widgets seuls** : une source par widget, le widget remplit la source.

Chaque carte montre un aperçu (données de démonstration), la **taille conseillée** et un bouton **Copier**.

**Créer la source dans OBS** :

1. Dans la scène : **Sources** → **+** → **Navigateur** → donne-lui un nom (par exemple « Corde –
   Commandes ») → OK.
2. **URL** : colle l'adresse copiée, par exemple `http://127.0.0.1:8471/#/overlay/widget/inputs`.
   Laisse « Fichier local » décoché.
3. **Largeur** et **Hauteur** : la taille conseillée de la carte (par exemple 640 et 384 pour les
   commandes, 1920 et 1080 pour une disposition overlay).
4. **CSS personnalisé** : laisse le champ vide, ou remplace ce qu'OBS y a mis par
   `body { background: rgba(0,0,0,0); }` (c'est déjà transparent). « Contrôler l'audio via OBS »,
   « Éteindre la source quand elle n'est pas visible » : comme tu veux. **« Actualiser le navigateur
   quand la scène devient active » est inutile** : la source se reconnecte toute seule si Corde
   redémarre.
5. OK, puis place et redimensionne la source dans la scène comme une image (Alt + glisser pour rogner).

**Les options** se cochent en haut de la page OBS de Corde ; elles complètent l'adresse (recopie-la après
un changement). Tu peux aussi les écrire à la main après l'adresse :

| Option | Valeurs | Effet |
|---|---|---|
| `bg` | `none`, `glass` (défaut), `solid` | fond des widgets : transparent (contour sombre autour des chiffres), verre fumé, opaque |
| `opacity` | 0.1 à 1 (défaut 1) | opacité de tout l'overlay |
| `scale` | 0.5 à 3 (défaut 1) | taille des textes et des graphiques ; la taille conseillée grandit d'autant |
| `hide=offtrack` | — | l'overlay s'efface en fondu quand tu n'es pas en piste (garage, menus), 2 s après ta sortie |
| `lang` | `fr`, `en` | langue de l'overlay (sinon celle de Corde) |
| `title=0` | — | sans le titre des widgets |
| `w.mode`, `w.window` | selon le widget | réglage du widget seul : classement `relative` / `full`, fenêtre des traces `5`, `10`, `20` s |

Exemple : `http://127.0.0.1:8471/#/overlay/widget/standings?bg=none&scale=1.25&hide=offtrack`. Une valeur
invalide est ignorée (la valeur par défaut s'applique).

**OBS sur un second PC de stream** : active l'accès réseau (section 6), redémarre Corde, puis dans la page
OBS choisis **Un autre PC (réseau)** : l'adresse devient celle du PC de jeu (`http://192.168.x.x:8471/…`).
Elle n'est montrée qu'à ta demande ; ne l'affiche pas en direct. L'adresse `127.0.0.1` ne marche que si
OBS tourne sur le PC de jeu.

**Ce que l'overlay ne montre jamais** : ni adresse IP, ni chemin de dossier, ni tes réglages de voiture ;
un widget sans données (jeu fermé) n'affiche rien, pas même son panneau.

---

## 6 quater. Ton serveur Corde à la maison (facultatif, 0.7)

Si tu as installé **Corde serveur** sur un Raspberry Pi ([SERVEUR.md](SERVEUR.md)), le PC peut lui envoyer
tes sessions pour que tu les voies sur ton téléphone **PC éteint** :

1. Sur le Pi : `sudo corde-server pair` → un code à 8 chiffres (10 minutes, une seule fois).
2. Dans Corde sur le PC : **Réglages → Serveur Corde** → adresse du serveur (par exemple `192.168.1.50`) et
   code → **Appairer**. (La section n'apparaît que sur le PC lui-même, pas depuis le téléphone.)
3. L'état passe à **Connecté** et le PC envoie tout ce qu'il a ; ensuite chaque session part dès qu'elle se
   termine. « En attente d'envoi » montre ce qui n'est pas encore parti (serveur éteint, autre réseau…) :
   Corde réessaie toute seule, au démarrage et toutes les 5 minutes.

Rien ne change si tu n'as pas de serveur : la section reste « Pas appairé » et Corde ne contacte rien.
Mets à jour le serveur **avant** (ou en même temps que) Corde sur le PC : un serveur plus ancien que le PC
refuse les envois (« Serveur à mettre à jour »), sans rien perdre.

---

## 7. Où sont tes données

Tout ce que Corde enregistre est dans **`%LOCALAPPDATA%\Corde\`**
(`C:\Users\<toi>\AppData\Local\Corde\`). Si tu avais l'ancienne version (avant 0.3.0, logo « M »), Corde
y reprend son dossier au premier lancement, avec toutes tes sessions (voir § 9) :

| Fichier ou dossier | Contenu |
|---|---|
| `corde.db` | sessions, tours, résultats importés, analyses, ressentis |
| `sessions\<date-circuit-type>\` | la télémétrie de chaque session (≈ 60 Mo par heure de roulage) |
| `settings.json` | tes réglages |
| `races\` | tes courses choisies, les réglages de la page Courses, les rappels déjà envoyés |
| `diagnostics\races\` | la copie d'une page de planning que Corde n'a pas su lire (seulement dans ce cas) |
| `logs\corde-desktop.log` | le **journal** de l'application |
| `hub-link.json`, `sync-hashes.json` | seulement avec un serveur Corde : son adresse, la clé de ce PC auprès de lui (ne la partage pas), les sommes de contrôle des fichiers déjà envoyés |

Rien n'est envoyé sur Internet (le planning des courses, si tu l'actives, lit deux pages publiques sans
rien y envoyer de toi). Corde n'écrit dans le dossier du jeu que lorsque tu le lui demandes
(paragraphe suivant).

### Tes fichiers de réglage (`.svm`)

Corde lit les réglages que tu as sauvegardés dans le garage du jeu
(`…\Le Mans Ultimate\UserData\player\Settings\<circuit>\*.svm`). Sur la page **Ingénieur** d'une session,
le bloc **« Ton réglage »** montre celui qu'il pense que tu as utilisé (le dernier enregistré pour cette voiture
dans le dossier du circuit avant la fin de la session) ; tu peux en choisir un autre dans la liste. Chaque
changement proposé s'affiche alors **avant → après** (par exemple `P2 → P1`), et les réglages que ta voiture
ne permet pas sont écartés avec la raison. Coche les plans voulus et clique **« Créer les fichiers de
réglage »** : Corde écrit `Corde_Sur_<ton fichier>.svm` (ou `_Agressif`, `_Pluie`, `_Iteration2_…`) **à côté de
ton fichier**, sans jamais en écraser un (il ajoute `_2`, `_3`…). Dans le jeu : **garage → Setups → rafraîchir
la liste → charger `Corde_…`**. Si aucun réglage n'apparaît, sauvegarde d'abord une fois ton réglage dans le
garage avec Corde ouverte. Quand tu passes au garage ou au stand, Corde lit aussi (en lecture seule, une fois
par voiture et par session) ce que le garage du jeu dit des réglages et le garde dans
`%LOCALAPPDATA%\Corde\diagnostics\lmu-api\garage\` : **envoie-nous ce dossier** (avec un de tes `.svm`), il
nous permettra de lire les vraies bornes de chaque réglage.

---

## 8. Quand quelque chose ne va pas : le journal

Corde n'a plus de fenêtre noire ; tout ce qu'il aurait écrit dedans va dans le journal.

1. Appuie sur `Windows + R`, tape `%LOCALAPPDATA%\Corde\logs` et valide.
2. Envoie-nous le fichier **`corde-desktop.log`** (et `corde-desktop.old.log` s'il existe), avec une
   capture d'écran et l'heure approximative du problème.

Si Corde n'arrive pas à démarrer, sa fenêtre affiche elle-même l'erreur, le chemin du journal et un bouton
**Réessayer**.

### Dépannage

**1. « Un autre Corde tourne déjà sur ce PC ».** Une ancienne fenêtre noire `corde-server.exe` est restée
ouverte : ferme-la, puis clique sur Réessayer.

**2. Le jeu n'est pas détecté** (l'icône reste sur « en attente du jeu »). Vérifie `Enable Plugins` = ON et
que tu as bien quitté puis relancé le jeu. Si le jeu est lancé « en tant qu'administrateur », Corde ne peut
pas le lire : lance le jeu normalement.

**3. La fenêtre ne s'affiche pas quand j'entre en piste.** Envoie-nous le journal : c'est le comportement
qu'on n'a pas pu essayer sans ton PC. En attendant, un clic sur l'icône près de l'horloge l'ouvre.

**4. Le téléphone ne se connecte pas.** Même Wi-Fi (pas de 4G, pas de réseau « invité ») ? Ensuite :
`Sécurité Windows` → `Pare-feu et protection du réseau` → `Autoriser une application via le pare-feu` →
**Corde** coché pour **Privé**. Si ton Wi-Fi est classé « Public », passe-le en « Privé » (`Wi-Fi` → ton
réseau → `Type de profil réseau`). Si le port 8471 était pris au démarrage, Corde en utilise un autre (8472
à 8480) : il est écrit dans le journal, à la ligne « Local : http://127.0.0.1:… ».

**5. L'antivirus bloque Corde.** Note le nom exact de l'alerte et envoie-le nous ; ne désactive pas ton
antivirus.

**6. Pour le tout premier test, ferme les autres outils de télémétrie** (SimHub, Crew Chief, TinyPedal,
RaceLab…). Corde est prévue pour cohabiter, mais on veut d'abord une mesure propre.

**7. Et l'anti-triche (EAC) ?** Corde lit uniquement ce que le jeu publie volontairement (le même
mécanisme que SimHub ou TinyPedal) et ne touche pas au processus du jeu. Par prudence, fais les premiers
tests hors ligne, et dis-nous avant de l'utiliser en session en ligne.

**8. La source OBS reste vide.** Corde est-elle ouverte ? L'adresse s'ouvre-t-elle dans un navigateur du
même PC (widgets visibles quand tu es en piste) ? Un widget sans données n'affiche rien : entre en piste
(et retire `hide=offtrack` pour essayer au garage). Avec un second PC, il faut l'adresse réseau (section 6
ter) et l'accès réseau activé. Si un fond apparaît derrière les widgets, vide le champ « CSS
personnalisé » de la source. Clic droit → **Interagir** est inutile : l'overlay n'a aucun bouton.

---

## 9. Mettre à jour

### Depuis Corde (à partir de la 0.6.0)

1. Ouvre **Réglages** (roue dentée en haut à droite) → section **Mises à jour**.
2. Clique sur **Rechercher une mise à jour**. Corde répond « Corde est à jour » ou « Version X disponible »,
   avec la liste des nouveautés.
3. **Ferme le jeu** (et attends la fin de l'enregistrement de ta session), puis clique sur **Installer et
   redémarrer**. Corde télécharge la nouvelle version, vérifie qu'elle vient bien de nous (signature), se
   ferme ; une petite fenêtre d'installation s'affiche quelques secondes, sans question, puis Corde
   redémarre toute seule sur la nouvelle version. Tes sessions, dispositions et réglages sont conservés.

Bon à savoir :

- **Corde cherche aussi toute seule** 30 secondes après son lancement, puis toutes les 6 heures. Quand une
  version est disponible : une notification Windows (une seule fois par version, jamais pendant que le jeu
  tourne), une **pastille verte** sur la roue dentée et une ligne « Mise à jour » sur l'Accueil. Elle
  n'installe **jamais** rien sans ton clic. Pour couper la recherche automatique : décoche **Rechercher
  automatiquement** (le bouton reste utilisable).
- **Pendant que le jeu est ouvert ou qu'une session s'enregistre**, l'installation est refusée avec un
  message : Corde ne s'arrête jamais pendant que tu roules.
- « **Impossible de vérifier pour l'instant** » : pas d'Internet, ou GitHub injoignable. Le lien
  **Ouvrir la page des versions** mène à la page où télécharger l'installeur à la main.
- La recherche lit un fichier public sur GitHub, qui voit l'adresse IP de ta connexion ; rien d'autre n'est
  envoyé (voir `PRIVACY.md`).
- **La 0.6.0 elle-même s'installe encore à la main** (ci-dessous) : c'est elle qui apporte le bouton. Les
  versions suivantes arrivent par le bouton.
- Les mises à jour du bouton sont les **versions numérotées** (page des versions, « Corde 1.8.1 »…). La
  version de développement « Corde — dernière version » (pré-version reconstruite à chaque modification
  du projet, <https://github.com/dultra123/corde/releases/tag/continuous>) s'installe toujours à la main.

### À la main (toutes versions)

1. Quitte Corde (clic droit sur l'icône → **Quitter**).
2. Télécharge le nouvel installeur sur la page des versions (<https://github.com/dultra123/corde/releases>)
   ou sur la page de la dernière version de développement (§ 2), et lance-le **par-dessus** l'ancien, sans
   désinstaller. L'avertissement de Windows réapparaît (§ 3) : c'est normal.
3. Relance Corde. Tes sessions, tes dispositions et tes réglages sont conservés.

La version installée est indiquée dans les Réglages de Corde (sections Application et Mises à jour).

### Passer de l'ancienne version (avant 0.3.0, logo « M ») à Corde

L'application a changé de nom et de logo en version 0.3.0. Corde s'installe **à côté** de l'ancienne
version (pas par-dessus) et reprend toutes tes données au premier lancement : sessions, base, dispositions
du tableau de bord, réglages, fichiers de diagnostic.

1. **Quitte l'ancienne version** : clic droit sur son icône « M » près de l'horloge → **Quitter**. (Si tu
   oublies, rien n'est perdu, voir le point 4.)
2. Télécharge et installe Corde (§ 2 et § 3).
3. Lance **Corde** depuis le menu Démarrer. Avant d'ouvrir quoi que ce soit, elle renomme ton ancien dossier
   de données en `%LOCALAPPDATA%\Corde` (un simple renommage : rien n'est copié ni effacé). Ouvre
   **Sessions** : tes sessions sont là.
4. Si l'ancienne version tournait encore, Corde affiche « Une autre copie tourne déjà sur ce PC » : quitte
   l'ancienne (point 1), puis clique sur **Réessayer**. Corde ne démarre jamais sur un dossier vide : si elle
   n'a pas pu renommer l'ancien dossier, elle s'en sert tel quel et réessaie au lancement suivant.
5. Si tu avais coché « Lancer au démarrage de Windows », coche-le à nouveau dans les Réglages de Corde.
6. Quand tu as vérifié tes sessions dans Corde, **désinstalle l'ancienne version** : Paramètres Windows →
   Applications → Applications installées → l'ancienne application → Désinstaller. Ses fichiers de
   programme et son lancement automatique sont retirés ; tes données, déjà reprises par Corde, ne sont pas
   touchées.

En cas de doute, le journal `%LOCALAPPDATA%\Corde\logs\corde-desktop.log` dit ce qui a été repris (lignes
« data folder of the former name… » et « database of the former name… »).

---

## 10. Désinstaller

`Paramètres Windows` → `Applications` → `Applications installées` → **Corde** → `Désinstaller`.

La désinstallation enlève le programme, le raccourci et le démarrage avec Windows. Elle **garde tes
données** : pour tout effacer, supprime ensuite le dossier `%LOCALAPPDATA%\Corde\`. (La case « Supprimer
les données de l'application » du désinstalleur ne concerne que le cache d'affichage, pas tes sessions.)

---

## Annexe — le mode console (`corde-server.exe`)

L'ancien fonctionnement existe toujours, pour le diagnostic ou pour utiliser Corde sans l'installer : c'est
le fichier `corde-portable-windows.zip` de la même page. **Ne le lance pas en même temps que
l'application** (les deux liraient le jeu et écriraient dans les mêmes fichiers ; l'application refuse
d'ailleurs de démarrer si elle le voit).

1. Extrais le zip dans un dossier simple, par exemple `C:\Corde-console\`.
2. Dans ce dossier, tape `cmd` dans la barre d'adresse de l'Explorateur puis Entrée.
3. Utilise les outils :

| Commande | À quoi elle sert |
|---|---|
| `lmu-probe.exe` | vérifie que Corde voit le jeu : version, circuit, voiture, vitesse (deux fois par seconde ; `Ctrl+C` pour arrêter) |
| `lmu-probe.exe --once --dump frame.json` | écrit un instantané complet de ce que le jeu publie — le fichier le plus utile à nous envoyer quand une donnée semble fausse |
| `acc-probe.exe --dump copie-acc` | Assetto Corsa Competizione, en piste : affiche ce que Corde lit du jeu (circuit, voiture, tour, vitesse, chronos) et écrit une copie de diagnostic sans ton nom — voir [Les jeux que Corde sait lire](JEUX.md) |
| `corde-server.exe` | le serveur seul ; ouvre ensuite `http://127.0.0.1:8471` dans un navigateur. Comme l'application, il n'est joignable que depuis ce PC par défaut ; avec `--lan` il écoute sur le réseau local et Windows pose la question du pare-feu (§ 6) |
| `corde-server.exe --source sim` | le même avec une course fictive, sans le jeu |
| `corde-store.exe sessions list` | liste les sessions enregistrées |

`--help` après chaque commande liste ses options. `Ctrl+C` dans la fenêtre noire arrête le programme.
