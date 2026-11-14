# Journal des versions

Une section par version, la plus récente en haut. **La CI publie les notes d'une version à partir de sa
section** : quand le numéro de `apps/desktop/src-tauri/tauri.conf.json` change sur `main`, le job `version`
crée la release `v<version>` et copie la section `## [<version>]` dans ses notes et dans `latest.json`
(ce que l'application affiche sous « Version X disponible »). Pas de section, pas de release : le job échoue.

Écrire pour le pilote : ce qui change pour lui, en quelques lignes, sans jargon. Le détail technique reste
dans `docs/STATUS.md` et l'historique git.

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
