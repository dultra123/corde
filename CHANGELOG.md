# Journal des versions

Une section par version, la plus récente en haut. **La CI publie les notes d'une version à partir de sa
section** : quand le numéro de `apps/desktop/src-tauri/tauri.conf.json` change sur `main`, le job `version`
crée la release `v<version>` et copie la section `## [<version>]` dans ses notes et dans `latest.json`
(ce que l'application affiche sous « Version X disponible »). Pas de section, pas de release : le job échoue.

Écrire pour le pilote : ce qui change pour lui, en quelques lignes, sans jargon. Le détail technique reste
dans `docs/STATUS.md` et l'historique git.

## [1.6.0] — 2026-11-16

**« Mes courses » te suivent : une course choisie sur le PC est dans l'application du téléphone, et
l'inverse — à la maison comme en 5G.**

### Nouveautés

- **« Mes courses » partagées entre tes appareils.** Le PC, le serveur Corde (Raspberry Pi ou NAS) et
  l'application du téléphone s'échangent les courses que tu choisis et celles que tu retires. Choisis sur
  le PC, retrouve-la sur le téléphone ; retire-la sur le téléphone, elle disparaît du PC.
- **Aussi hors de chez toi** : en 5G, le téléphone passe par ton serveur Corde (avec le VPN), qui a reçu
  les courses du PC.
- **Sous « Mes courses », l'application du téléphone dit avec qui elles sont partagées** — et ce qui
  manque : un serveur injoignable, à appairer, ou à mettre à jour.

### À savoir

- **Mets à jour les trois** : Corde sur le PC, l'application du téléphone, et **Corde serveur** sur le
  Raspberry Pi ou le NAS. Un serveur resté sur une version plus ancienne ne relaie pas les courses (rien
  n'est perdu : elles restent sur chaque appareil).
- **Sur le téléphone, ouvre l'application** pour récupérer une course choisie sur le PC : elle ne fait rien
  en arrière-plan. Ses rappels sont programmés à ce moment-là ; les widgets suivent.
- L'échange prend moins d'une minute. Le PC et le téléphone sonnent chacun leur rappel.
- Les réglages de la page Courses (délais de rappel, catégories, correction d'heure) restent propres à
  chaque appareil.
- Pas encore essayé sur Android.

## [1.5.0] — 2026-11-16

**Le Studio a son onglet, et un tableau de bord prend le format et la forme que tu veux — jusqu'au petit
écran rond d'un volant.**

### Nouveautés

- **Un onglet « Studio »** dans la barre du haut du PC, à côté d'Accueil, Direct, Sessions, Courses et
  Profil. On y entre et on en sort par les onglets, comme pour le reste.
- **Le bouton « Format »** de l'éditeur dit le format en cours (« 852 × 393 », « Rond 480 × 480 »…) et
  ouvre un panneau pour le changer à tout moment.
- **Des formats prêts** : téléphone en paysage ou en portrait, tablette 4:3, écran 16:9, bandeau large de
  volant, **écran rond de volant (480 × 480)**, carré, petit écran 5:3, petit écran 3:2, tout l'écran.
- **Taille libre** : la largeur et la hauteur que tu veux, de 100 à 4000 pixels, et un bouton pour les
  inverser.
- **Forme de la page** : rectangle, **coins arrondis** (rayon au choix) ou **rond**. Rien n'est dessiné
  hors de la forme ; dans l'éditeur, la zone qui sera cachée est hachurée.
- **« Nouveau »** ouvre une fenêtre où tu choisis le modèle de départ (Vide, Corde en blocs, Endurance) et
  le format, avant de créer.
- Chaque tableau de bord de la bibliothèque affiche son format.

### À savoir

- **Changer de format garde tes éléments.** Si les cases de la grille deviennent très allongées, le panneau
  le dit et propose « Adapter la grille » : rien n'est supprimé, et Ctrl + Z revient au format d'avant.
- **Pour un petit écran branché au PC** (écran rond de volant, écran de tableau de bord) : ouvre un
  navigateur sur cet écran à l'adresse `http://127.0.0.1:8471/#/dash`, en plein écran, et choisis ton
  tableau de bord dans ☰. Le pas-à-pas est dans le guide « Tester sur ton PC ». Corde n'écrit pas sur
  l'écran intégré d'un volant, qui reste piloté par le logiciel de son fabricant.
- **Mets aussi à jour l'application du téléphone** : une version plus ancienne affiche un tableau de bord
  rond ou arrondi en rectangle, sans le rogner.
- Le rond et les coins arrondis ont été vérifiés dans l'application Windows et dans un navigateur ; pas
  encore sur un vrai écran rond, ni sur iPhone avec une vidéo en fond.

## [1.4.3] — 2026-11-16

**Corde change de logo.**

### Nouveautés

- **Nouveau logo : un compte-tours.** Un anneau gris qui devient ambre dans la zone rouge, le « C » ambre
  au centre et une aiguille blanche. Il remplace l'ancien partout : icône de Windows, de l'iPhone et
  d'Android (y compris l'icône à thème et celle des notifications), onglet du navigateur, barre du haut de
  l'application et page d'appairage du serveur.

### Corrections

- **Sur un NAS, la page « Serveur Corde » indique la bonne commande** pour obtenir un code d'appairage :
  `corde-server pair` dans le terminal du conteneur, avec le chemin dans Container Manager, au lieu de
  « sudo corde-server pair ». Il faut pour cela mettre à jour Corde serveur sur le NAS.

### À savoir

- Windows garde parfois l'ancienne icône en mémoire sur le bureau ou dans la barre des tâches : elle se
  met à jour au prochain redémarrage du PC.
- **Guide Synology** : la mise à jour du paquet du VPN par une tâche planifiée, vérifiée sur un vrai NAS.

## [1.4.2] — 2026-11-15

**Pour ouvrir Corde serveur depuis l'extérieur avec l'application du téléphone.**

### Nouveautés

- **L'application du téléphone accepte l'adresse que donne un VPN maillé** (une adresse en `100.x.y.z`,
  par exemple `100.101.102.103:8471`). Jusqu'ici, « Mes serveurs » → « Ajouter un serveur » la refusait
  (« seules les adresses du réseau de la maison… »), ce qui empêchait d'ouvrir Corde serveur dehors par ce
  genre de VPN, pourtant décrit dans le guide d'accès extérieur. Le serveur, lui, n'est toujours **jamais**
  ouvert sur Internet : on entre chez soi par le VPN, puis on appaire le téléphone pour cette adresse.

### Corrections

- **Sur un NAS, Corde serveur n'indique plus « sudo corde-server pair »** : dans un conteneur, la commande
  est `corde-server pair`, dans le terminal du conteneur. Le texte affiché avec le code dit aussi où le
  taper dans l'application du téléphone (Accueil → Mes serveurs → Ajouter un serveur).

### À savoir

- **Guide Synology** : les vrais noms des boutons pour obtenir un code d'appairage dans le terminal du
  conteneur (Créer → Lancer avec une commande), et les étapes de l'application du téléphone.
- Le PC n'a rien à faire d'autre que la mise à jour. Le serveur (Raspberry Pi ou NAS) n'a pas besoin d'être
  mis à jour pour utiliser un VPN.

## [1.4.1] — 2026-11-15

**Mise à jour à installer avant de relier un PC à Corde serveur.**

### Corrections

- **Corde sur le PC ne se fige plus au premier envoi vers Corde serveur.** Dès que le PC avait au moins un
  résultat de course importé du jeu, le premier envoi bloquait sa base de données : les pages Sessions et
  Profil ne répondaient plus, et plus rien ne s'enregistrait jusqu'au redémarrage de Corde. Toutes les
  versions depuis la 1.0.0 étaient concernées, dès que le PC était appairé à un serveur (Raspberry Pi ou
  NAS). Rien n'était perdu ; avec cette version, l'envoi va au bout. Si ton PC est resté figé : ferme Corde
  (icône près de l'horloge → Quitter), installe cette version, relance.
- **L'application du téléphone n'affiche plus la section « Serveur Corde » du PC** dans ses réglages : elle
  ne pouvait qu'y échouer (« timeout »). Le téléphone se relie au serveur par Accueil → Mes serveurs.

### À savoir

- **Guide Synology relu sur un vrai NAS** (DS220+, DSM 7.4.1) : vrais noms des menus, préparation d'un NAS
  neuf (création du stockage), et le journal du conteneur, que DSM affiche la ligne la plus récente en
  haut. Les textes de Corde disent maintenant « Raspberry Pi ou NAS » et où trouver le code sur un NAS.
- Le serveur (Raspberry Pi ou NAS) n'a pas besoin d'être mis à jour pour cette correction : elle est dans
  Corde sur le PC.

## [1.4.0] — 2026-11-15

**Corde serveur sur un NAS Synology**, sans acheter de Raspberry Pi et sans taper de commande.

### Nouveautés

- **Corde serveur en image de conteneur** : un fichier `corde-server-<version>-docker-x64.tar.gz` sur la
  page des versions, à importer dans **Container Manager** (DSM 7.2), puis un projet à créer en collant un
  texte tout prêt. Tes données restent dans un dossier du NAS ; une mise à jour n'y touche pas. Marche aussi
  sur tout autre NAS ou machine avec Docker. Pas à pas : `docs/SYNOLOGY.md`.
- **Premier appairage sans terminal** : tant qu'aucun appareil n'est appairé, le code d'appairage s'affiche
  dans le **journal du conteneur** à chaque démarrage (10 minutes, une seule fois). Les appareils suivants
  s'appairent avec `corde-server pair` dans le terminal du conteneur, ouvert depuis le navigateur.
- **Sauvegarde en une commande** sur le NAS (`corde-sauvegarde`), au même format que celle du Raspberry Pi :
  on passe de l'un à l'autre en gardant tout.

### À savoir

- Comme sur le Pi : **jamais** de port 8471 ouvert sur Internet, ni QuickConnect, ni proxy inversé ; dehors,
  on passe par un VPN.
- Image pour processeur ARM 64 bits fournie aussi, mais jamais essayée sur un vrai NAS.

## [1.3.0] — 2026-11-15

**Le Studio devient vraiment le tien** : photos, logos, vidéos et couleurs qui suivent la course.

### Nouveautés

- **Images et vidéos** : un logo, une photo ou une courte vidéo en boucle (sans son), en élément ou en
  **fond de page**, choisis sur ton PC. Ajustement (toute l'image, remplir, étirer), opacité, coins arrondis.
  Avant l'envoi, Corde **retire de tes photos la position GPS** et les autres informations de l'appareil
  photo ; d'une vidéo MP4, il ne garde que les images (ni son, ni position). Une vidéo ne joue que sur la
  page affichée et s'arrête quand le téléphone se met en veille.
- **Couleur selon la valeur** : par exemple autonomie sous 3 tours en rouge, sous 6 en orange, sinon en vert
  (Valeur, Barre, Jauge circulaire).
- **Afficher quand…** : un élément n'apparaît qu'avec le limiteur, dans les stands, sous drapeau jaune ou
  bleu, ou quand une donnée passe un seuil (par exemple « carburant < 2 tours »).
- **Nouveaux éléments** : jauge circulaire (vitesse, régime…), formes, **bandeau d'alerte** (« LIMITEUR »,
  « BOX »…) et rampe de régime réglable (nombre de feux, couleurs).
- **Style** de chaque élément (fond, transparence, coins arrondis, contour) et **police des chiffres** au
  choix (dont un style afficheur numérique et un style course large).
- **Éditeur plus confortable** : plusieurs éléments à la fois (Maj + clic ou un cadre), déplacés ensemble
  avec des repères d'alignement, **copier / coller** (Ctrl + C, Ctrl + V, aussi vers une autre page),
  recherche dans la liste des éléments, bouton « Abandonner le brouillon ».
- **Partage avec les images** : l'export devient un fichier `.cordedash` qui emporte ses images et vidéos.
  Les fichiers `.cordedash.json` de la 1.2 s'ouvrent toujours.

### Corrections

- Sur un tableau de bord du Studio, le bouton ☰ ne cache plus le coin en haut à droite : sur PC il
  n'apparaît qu'au passage de la souris, sur téléphone la page lui laisse sa place.

### À savoir

- Les images restent sur ton PC : PC éteint, le téléphone affiche une place vide à leur endroit.
- Ajouter une image ou une vidéo se fait depuis le PC. 200 fichiers et 300 Mo au plus.
- Une vidéo partagée l'est telle quelle : regarde ce qu'elle montre avant d'envoyer le fichier. Ne partage
  que des images dont tu as le droit.

## [1.2.1] — 2026-11-15

### Nouveautés

- **Changer de page à la souris** : sur le PC, deux flèches apparaissent sur les côtés du tableau de bord
  quand la souris est dessus, et les pages se font glisser à la souris comme au doigt. Plus besoin de passer
  par le menu ☰. Les flèches ← → du clavier et les points du bas marchent toujours.

### Corrections

- **Studio** : la couleur choisie pour une **Valeur** ou une **Barre** s'applique tout de suite. Avant, quand
  le jeu était fermé et la course de démonstration arrêtée, il fallait enregistrer et rouvrir le tableau de
  bord pour la voir ; pareil pour un changement de donnée ou de décimales.

## [1.2.0] — 2026-11-15

**Le tableau de bord arrive sur le PC**, sans quitter la page Direct.

### Nouveautés

- **Tableau de bord sur le PC, dans la page** : dans le Direct, le menu des dispositions (le bouton en haut à
  gauche) a maintenant un groupe **Tableaux de bord**. Un clic sur « Corde » affiche le tableau de bord à la
  place des widgets, la barre du haut reste là ; un clic sur une disposition de widgets les ramène.
- **Plein écran sur demande** : un bouton discret sur le tableau de bord l'agrandit à tout l'écran, Échap ou
  le même bouton le remet dans la page. L'adresse `…#/dash` reste le plein écran direct (second écran).
- **Sur le téléphone, rien ne change** : plein écran, ☰ « Passer aux widgets », et le menu des dispositions y
  ramène aussi en un geste.
- **Studio : crée tes propres tableaux de bord** (sur le PC, Direct → menu des dispositions → « Studio ») :
  des pages d'éléments placés sur une grille — une valeur au choix parmi une soixantaine (vitesse, écart,
  carburant, pressions…), des jauges, du texte, l'heure, tous les blocs du tableau de bord Corde et les
  widgets. Au format d'un téléphone, d'une tablette, d'un écran ou d'un bandeau large ; glisser,
  redimensionner, annuler / rétablir, aperçu avec une course de démonstration. Pars d'une page vide, de
  « Corde en blocs » ou de l'exemple « Endurance ».
- **Chaque appareil choisit son tableau de bord** (menu des dispositions sur PC, ☰ sur le téléphone,
  Réglages). Le téléphone en garde une copie pour l'afficher même PC éteint.
- **Partage** : exporte un tableau de bord en fichier `.cordedash.json` et importe celui d'un ami. Le fichier
  ne contient que la mise en page (ton pseudo seulement si tu l'as saisi).

### À savoir

- Le tableau de bord **Corde** reste le même ; pour le changer, duplique-le.
- Supprimer un tableau de bord se fait depuis le PC.
- Pas encore : images et logos, formules, polices, galerie en ligne, affichage sur l'écran d'un volant.

## [1.1.0] — 2026-11-14

**Le Direct devient un vrai tableau de bord sur le téléphone**, comme l'écran d'un volant de course.

### Nouveautés

- **Tableau de bord du téléphone** : sur un téléphone (navigateur, application iPhone et Android), la page
  **Direct** n'est plus la grille de widgets rétrécie mais un écran de course plein écran — grand rapport,
  rampe de régime qui clignote au passage (et alterne au limiteur), vitesse, écart au meilleur tour, position,
  tours, secteurs, écarts devant et derrière, carburant, électronique et pneus. Feux de drapeau (jaune, bleu,
  damier) quand le jeu les donne.
- **Quatre pages à faire glisser du doigt** : Course, Pneus & freins, Carburant & énergie (autonomie, ce qui
  manque pour finir, hybride, session, météo), Chronos & écarts (secteurs, derniers tours, les deux voitures
  devant et derrière toi).
- **L'écran reste allumé** tant que le tableau de bord est affiché (réglage, activé par défaut).
- **Pensé pour le paysage**, à côté du volant ; en portrait, les cases s'empilent. De l'iPhone SE à la
  tablette.
- **Tablette ou second écran** : l'adresse **`…#/dash`** ouvre toujours le tableau de bord ; le réglage
  **Affichage du Direct** (Automatique / Tableau de bord / Widgets) choisit pour chaque appareil. Sur le PC,
  rien ne change : les widgets restent là.
- **Application iPhone et Android : « Voir une démonstration »** (dans « Mes serveurs ») — une course inventée
  sur le tableau de bord, sans PC, calculée sur le téléphone.

### À savoir

- Le tableau de bord a été vérifié avec la course de démonstration, pas encore avec le jeu ni sur un vrai
  téléphone : dis-nous ce qui se lit mal à bout de bras.
- Les widgets restent disponibles sur le téléphone : menu ☰ → « Passer aux widgets ».
- **Application Android** : c'est sa première version. Quand le fichier `Corde_1.1.0_android.apk` figure sur
  cette page, il s'installe à la main (voir `docs/ANDROID.md`) ; pas encore de widgets sur Android.
- L'application iPhone passe par TestFlight, pas encore par l'App Store.

## [1.0.0] — 2026-11-14

**Première version officielle de Corde.** Elle réunit tout ce qui a été construit depuis les premières
versions de test ; les nouveautés suivantes arriveront par les mises à jour.

### Ce que fait Corde 1.0

- **Sur le PC (Windows), à côté de Le Mans Ultimate** : la télémétrie en direct dans un tableau de bord
  fait de widgets, l'enregistrement de chaque session, la comparaison de tours, le profil et les résultats
  de course, l'ingénieur (analyse de ta conduite, trois orientations de réglage, ajustement d'après ton
  ressenti, fichiers `.svm` prêts à charger), la page Courses (planning en ligne, tes départs, rappel
  sonore), les widgets à fond transparent pour OBS, et les mises à jour depuis l'application.
- **Sur le téléphone** : la même interface dans le navigateur, sur le Wi-Fi de la maison.
- **Application iPhone** (en test par TestFlight) : les courses sans PC avec rappels en notifications, les
  widgets (« Courses », « Mes courses », « Courses en images », écran verrouillé), le compte à rebours dans
  la Dynamic Island, et la connexion à ton PC ou à ton serveur Corde.
- **Corde serveur** (Raspberry Pi ou Linux) : tes sessions gardées à la maison, synchronisées depuis le PC.

### Ce qui change depuis la version précédente

- **Sur le PC et sur Corde serveur** (0.8.0 → 1.0.0) : rien d'autre que le numéro de version. Tes sessions,
  tes dispositions et tes réglages sont conservés.
- **Sur iPhone** (0.10.0 → 1.0.0) : rien d'autre que le numéro de version.

### À savoir

- L'installeur Windows n'est pas encore signé : au premier lancement, Windows affiche « Windows a protégé
  votre ordinateur » → **Informations complémentaires** → **Exécuter quand même**.
- L'application iPhone n'est pas encore sur l'App Store.
- Ce qui a été vérifié avec le jeu et ce qui ne l'a pas encore été est dit, ligne par ligne, dans le
  tableau « Où en est le projet » du README.

## [Android 1.0.0] — 2026-11-14

Première application Android. Corde sur le PC, Corde serveur et l'application iPhone ne changent pas. (Le
titre porte « Android » pour que cette section ne soit jamais reprise dans les notes d'une version du PC.)

### Nouveautés

- **Corde sur Android** (Android 8 ou plus récent) : la même application que sur iPhone — la page
  **Courses sans PC** (planning lu par le téléphone, tes choix, tes **rappels en notifications** même
  application fermée et téléphone en veille, avec le son de Corde), « Mes serveurs » (ton PC par son adresse
  ou son QR code, ton serveur Corde par un code d'appairage), et le direct, les sessions, l'ingénieur et le
  profil dès qu'un serveur est ajouté. Le Direct se tourne en paysage ; le bouton retour d'Android revient à
  la page précédente.
- **Sans Google** : aucune bibliothèque Google (ni Play services, ni Firebase, ni statistiques) ; Corde
  marche aussi sur un téléphone sans services Google.

### À savoir

- Elle s'installe à partir du fichier **`Corde_1.0.0_android.apk`** des releases (pas encore sur le Play
  Store) : il faut autoriser ton navigateur à « installer des applis inconnues ». Le guide pas à pas :
  [docs/ANDROID.md](docs/ANDROID.md).
- Android te demande les **notifications** à ton premier choix de course, puis Corde te propose
  « **Alarmes et rappels** » pour que les rappels sonnent à la minute près ; sans elle, ils peuvent arriver
  avec quelques minutes de retard quand le téléphone dort.
- Pas encore de **widgets** ni de **compte à rebours** sur Android.
- Tes données Corde ne partent ni dans la sauvegarde Google ni vers un nouveau téléphone : sur un nouveau
  téléphone, rajoute tes serveurs et appaire-le à nouveau.
- Compilée et essayée sur un émulateur par GitHub ; **pas encore essayée sur un vrai téléphone**.

## [iPhone 0.10.0] — 2026-11-14

Application iPhone seulement : Corde sur le PC et Corde serveur restent en 0.8.0.

### Nouveautés

- **Widget « Courses en images »** (petit, moyen, grand) : une course par page avec sa **photo** en fond,
  sa catégorie ou son niveau, le **SR et le DR qu'il faut** (cadenas rouge quand ton profil est en
  dessous), « Ma course » si tu as choisi ce départ, « 2/6 », le circuit et la durée, l'heure du prochain
  départ avec le temps restant, et les départs suivants (les tiens en ambre). La page change toute seule
  chaque minute ; le bouton **›** passe à la suivante (et **‹** revient, en moyen et en grand). En petit, la
  première page est ton prochain départ choisi. Sans photo, un fond aux couleurs de la catégorie avec le
  nom de la piste.

### À savoir

- Les photos sont téléchargées par Corde (8 au plus, seulement quand le planning est activé et que Corde
  est ouverte) et gardées en petit sur le téléphone pour le widget. Le site qui héberge une photo voit
  l'adresse IP du téléphone, comme quand l'application l'affiche.
- Le diagnostic des widgets (page Courses → Rappels) dit combien de photos sont gardées.
- Ces photos restent sur le téléphone si tu supprimes l'application : coupe le planning avant pour les
  effacer.

## [iPhone 0.9.0] — 2026-11-14

Application iPhone seulement : Corde sur le PC et Corde serveur restent en 0.8.0. (Le titre porte
« iPhone » pour que cette section ne soit jamais reprise dans les notes d'une version du PC.)

### Nouveautés

- **Widgets** sur l'écran d'accueil : « Courses » (les prochaines courses du planning, en petit, moyen ou
  grand) et « Mes courses » (le compte à rebours jusqu'à ton prochain départ choisi). Sur l'écran verrouillé :
  « Prochain départ ». Un toucher ouvre la page Courses.
- **Compte à rebours dans la Dynamic Island et sur l'écran verrouillé** : il démarre tout seul quand ton
  prochain départ choisi est dans moins de 2 heures et que Corde est ouverte, ou avec le bouton
  **Compte à rebours** sur un de tes départs (jusqu'à 8 heures avant). Réglage dans la page Courses, à côté
  des rappels.

### À savoir

- **iOS 17 au minimum** (au lieu d'iOS 16).
- Les widgets se mettent à jour quand Corde est ouverte ; ils restent justes plusieurs jours sans l'ouvrir
  (les horaires des quotidiennes au-delà de ce que montre le site sont estimés, marqués « ≈ »).
- Après le départ, le compte à rebours affiche « Départ donné » et reste jusqu'à la prochaine ouverture de
  Corde (ou jusqu'à ce que tu le balaies).
- Rien de plus n'est lu sur Internet, rien n'est envoyé : les widgets montrent une copie de ce que la page
  Courses a déjà lu.
- Côté Apple, rien à faire : l'identifiant des widgets est créé tout seul au premier envoi vers TestFlight.

## [0.8.0] — 2026-11-12

### Nouveautés

- **Corde sur iPhone** (première version, distribuée par TestFlight dès que le compte Apple est prêt) : la
  même interface dans une vraie application, avec les onglets en bas de l'écran.
- **Courses sans PC** : l'application lit elle-même le planning (racecontrol.gg et lmu.tools, si tu
  l'actives), garde tes choix et tes réglages sur le téléphone et programme les **rappels en notifications**
  de l'iPhone : ils sonnent même application fermée ou téléphone verrouillé. L'autorisation est demandée à
  ton premier choix de course.
- **Mes serveurs** : ajoute ton PC (adresse ou QR code de l'Accueil du PC) pour le direct, les sessions,
  l'ingénieur et le profil ; ajoute ton serveur Corde et appaire le téléphone avec un code.

### À savoir

- Sur le PC, rien ne change, sauf qu'il accepte maintenant l'application iPhone quand l'accès depuis le
  téléphone est activé. L'application a besoin de Corde 0.8 sur le PC.
- Les choix de courses du téléphone et ceux du PC restent séparés.
- Ce qu'il faut faire côté Apple : `docs/IOS.md`.

## [0.7.1] — 2026-11-12

### Nouveautés

- **Courses en ligne sur l'Accueil** : sous ta dernière session, une carte fait défiler tes courses
  choisies puis les prochains départs du planning, une course à la fois, avec l'image du site, le niveau,
  les badges SR / DR, l'heure du départ et un compte à rebours. Elle passe à la suivante toutes les
  8 secondes et s'arrête quand ta souris est dessus. Flèches, points, clavier ou glissement du doigt pour
  passer d'une course à l'autre ; un clic ouvre la page Courses, **Choisir ce départ** l'ajoute à tes
  courses.
- Elle remplace la ligne « Prochaine course » de l'Accueil.

### À savoir

- Rien de plus n'est lu sur Internet : la carte utilise le planning déjà lu pour la page Courses, et ne
  montre rien tant que le planning n'est pas activé.

## [0.7.0] — 2026-11-12

### Nouveautés

- **Corde serveur** : un petit serveur chez toi (Raspberry Pi 5), toujours allumé, qui garde tes sessions,
  l'ingénieur, ton profil et tes courses, et les montre à ton téléphone **même PC éteint**. Le PC lui envoie
  chaque session dès qu'elle se termine (et, au démarrage, tout ce qui n'était pas encore parti). Le direct
  reste sur le PC. Installation pas à pas : `docs/SERVEUR.md`.
- **Seuls tes appareils** ouvrent le serveur : chacun s'appaire une fois avec un code à 8 chiffres affiché
  sur le serveur lui-même (`sudo corde-server pair`). Page **Appareils** pour renommer ou révoquer, avec
  effet immédiat.
- Réglages → **Serveur Corde** sur le PC : adresse du serveur, appairage, état (connecté, injoignable,
  dernière synchronisation, sessions en attente), « Tout renvoyer », « Dissocier ».
- Les **ressentis** écrits sur le serveur (depuis le téléphone) redescendent sur le PC, et inversement.
- La page **Courses** marche aussi sur le serveur, rappels compris, 24 h/24.

### À savoir

- Le serveur ne doit **jamais** être ouvert sur Internet : pour y accéder dehors, on passe par un VPN
  (`docs/ACCES-EXTERIEUR.md`, WireGuard recommandé).
- Mets le serveur à jour en même temps que Corde sur le PC : un serveur plus ancien refuse les envois
  (rien n'est perdu, tout part après la mise à jour).
- Sans serveur, rien ne change sur le PC.

## [0.6.0] — 2026-11-12

### Nouveautés

- **Mise à jour depuis l'application** : Réglages → Mises à jour → « Rechercher une mise à jour », puis
  « Installer et redémarrer ». Corde télécharge la nouvelle version, vérifie sa signature, se ferme,
  s'installe et redémarre seule. Plus besoin de retélécharger l'installeur.
- Corde vérifie toute seule 30 secondes après son lancement, puis toutes les 6 heures (désactivable dans
  les réglages), et te prévient une fois par une notification Windows. Elle n'installe **jamais** rien
  sans ton clic, et refuse d'installer pendant que le jeu tourne ou qu'une session s'enregistre.
- Quand une mise à jour est disponible : une pastille sur l'icône des réglages et une ligne sur l'Accueil.

### À savoir

- Cette version-ci s'installe encore à la main (comme d'habitude). Les suivantes arriveront par le bouton.
- La vérification contacte GitHub (page des versions de Corde) : GitHub voit l'adresse IP de ta connexion,
  rien d'autre n'est envoyé. Détails dans `PRIVACY.md`.

## [0.5.1] — 2026-11-12

### Corrections

- Après l'installation d'une nouvelle version, la fenêtre affichait encore l'ancienne interface (cache du
  navigateur intégré). Corde charge désormais toujours l'interface de la version installée.

## [0.5.0] — 2026-11-12

### Nouveautés

- **Widgets dans OBS** : chaque widget, ou une disposition entière, peut devenir une source navigateur
  d'OBS avec un fond transparent. Page « OBS » (onglet Direct) : adresses à copier, aperçu, taille
  conseillée, options (fond, opacité, échelle, masquer hors piste, langue).
- Dispositions « overlay » (canevas 1920 × 1080) éditées dans le tableau de bord ; une disposition
  « OBS » est fournie par défaut.

### Corrections

- Mode overlay : pas d'info-bulle ni de panneau vide pour un widget sans données ; classement sur une
  ligne.

## [0.4.0] — 2026-11-11

### Nouveautés

- **Page « Courses »** : planning des courses en ligne de Le Mans Ultimate (racecontrol.gg et lmu.tools,
  désactivé par défaut), choix de tes courses et rappels avant le départ, sur le PC et le téléphone.

## [0.3.0] — 2026-11-11

### Nouveautés

- **L'application s'appelle désormais Corde** (le point de corde), avec un nouveau logo. Au premier
  lancement, Corde reprend toutes les données de l'ancienne version (sessions, réglages, dispositions).
- Corde devient un logiciel libre (licence GPL-3.0-or-later), publié dans `dultra123/corde`.
- Sécurité du serveur local renforcée pour l'accès depuis le téléphone ; l'adresse du PC et le QR code
  sont floutés par défaut sur l'Accueil (pratique en stream).
