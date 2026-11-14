# Corde — fiche de présentation

> À jour pour la **version 1.2.1** (15 novembre 2026). Cette fiche rassemble, en un seul endroit, ce qu'il
> faut pour présenter Corde (site, page de présentation, annonce) : ce qu'elle fait, où elle tourne, ce qui
> est nouveau, ce qui arrive, les images et les liens. Elle ne dit que ce qui existe ; le détail et l'état
> exact de chaque partie sont dans le [README](../README.md) et dans [STATUS.md](STATUS.md).

## En une phrase

**Corde — télémétrie en direct, analyse des tours et ingénieur de course pour Le Mans Ultimate.**

Version plus longue : Corde tourne à côté du jeu. Elle lit la télémétrie de Le Mans Ultimate, l'affiche en
direct (widgets ou tableau de bord de course, sur le PC comme sur le téléphone), enregistre chaque session, analyse ta
conduite et te propose des ajustements de réglage que tu affines avec ton ressenti. Tout reste chez toi : pas
de compte, pas de cloud.

Le nom vient du **point de corde** : l'endroit où la trajectoire idéale touche l'intérieur du virage.

## En bref

| | |
|---|---|
| Version actuelle | **1.2.1** (15 novembre 2026) |
| Prix | Gratuit |
| Licence | Logiciel libre, GPL-3.0-or-later ; code source public |
| Jeu | Le Mans Ultimate |
| PC | Windows (64 bits), installeur `Corde_<version>_x64-setup.exe` |
| Téléphone | Navigateur du téléphone, application iPhone (iOS 17 ou plus récent), application Android (Android 8 ou plus récent) |
| Serveur maison (facultatif) | Corde serveur, pour Raspberry Pi 5 (Linux arm64) ou Linux x64 |
| Langues | Français et anglais |
| Auteur | dultra__ et les contributeurs |
| Site officiel | <http://corderacing.com/> |
| Dépôt | <https://github.com/dultra123/corde> |

## Ce que fait Corde

Chaque bloc ci-dessous peut devenir une section de page. Les images sont dans [`docs/images/`](images/) ;
elles viennent du simulateur intégré et de la course de démonstration (aucune donnée réelle de pilote).

### 1. Le tableau de bord en direct (PC)

Dix widgets : tours et écart, commandes (volant, pédales, régime), pneus (températures sur trois zones,
pressions, usure), freins, carburant et énergie, écarts, classement, carte du circuit, session et météo,
électronique. Chaque widget se déplace, s'étire, se supprime ; plusieurs dispositions peuvent être gardées.

Images : `dashboard.png` (le tableau de bord), `dashboard-edition.png` (le mode édition), `accueil.png`
(l'écran d'accueil).

### 2. Le tableau de bord de course (téléphone, tablette, PC)

Un vrai tableau de bord de course, comme l'écran d'un volant : grand rapport au centre, rampe de régime qui
clignote au moment de passer le rapport, vitesse, écart au meilleur tour, position, tours, carburant. Quatre
pages : **Course**, **Pneus & freins**, **Carburant & énergie**, **Chronos & écarts**.

- **Sur le téléphone** : plein écran, pages à faire glisser du doigt, écran qui reste allumé. Pensé pour un
  téléphone posé en paysage à côté du volant ; il marche aussi en portrait.
- **Sur le PC et la tablette** (depuis la 1.2.0) : il se choisit dans le menu des dispositions de la page
  Direct, à côté des dispositions de widgets, et s'affiche dans la page. Un clic pour y aller, un clic pour
  revenir aux widgets ; plein écran sur demande.

Images : `tableau-de-bord-telephone.png` (page Course), `tableau-de-bord-pneus.png`,
`tableau-de-bord-carburant.png`, `tableau-de-bord-chronos.png`, `tableau-de-bord-portrait.png` (page Course
en portrait), `tableau-de-bord-pc.jpg` (dans la page Direct du PC), `tableau-de-bord-menu.png` (le menu des
dispositions avec le groupe « Tableaux de bord »).

### 3. Le Studio : tes propres tableaux de bord (nouveau en 1.2.0)

Un éditeur, sur le PC, pour composer ses propres tableaux de bord de course : des pages d'éléments placés sur
une grille. Une **valeur au choix** parmi une soixantaine de données (vitesse, écart, carburant, pressions,
températures, électronique…), des jauges, du texte, l'heure, les blocs du tableau de bord Corde et les
widgets. Plusieurs formats : téléphone, tablette, écran, bandeau large. On glisse, on redimensionne, on
annule, on regarde le résultat avec une course de démonstration.

- **Chaque appareil choisit son tableau de bord** ; le téléphone en garde une copie pour l'afficher PC éteint.
- **Partage par fichier** : un tableau de bord s'exporte et s'importe (`.cordedash.json`). Le fichier ne
  contient que la mise en page.
- Le tableau de bord « Corde » reste le modèle de départ : on le duplique pour le modifier.
- Pas encore dans le Studio : images et logos, formules.

Images : `studio-edition.png` (l'éditeur : éléments à gauche, page au centre, propriétés à droite),
`tableau-de-bord-personnalise.png` (un tableau de bord d'exemple fait avec le Studio).

### 4. Sessions et tours

Chaque session est enregistrée automatiquement, avec les temps officiels du jeu. On ouvre un tour ou on en
compare deux : écart de temps le long du tour, vitesse, gaz, frein, volant, rapport, régime, accélérations.
Export CSV.

Images : `session.png`, `tour-comparaison.png`.

### 5. L'ingénieur de course

Après une session d'au moins trois tours propres, l'ingénieur analyse les données : régularité, style de
pilotage, pneus, freins, équilibre de la voiture virage par virage, suspension. Il en tire des constats
chiffrés, puis propose trois orientations de réglage — **agressif**, **soft et sûr**, **pluie** — avec la
raison de chaque changement. Tu en essaies un, tu écris ton ressenti avec tes mots, et Corde propose l'étape
suivante. Elle lit tes propres fichiers de réglage et peut écrire les fichiers prêts à charger dans le
garage du jeu, sans jamais modifier ni écraser un fichier existant.

Images : `ingenieur-analyse.png`, `ingenieur-equilibre-constats.png`, `ingenieur-reglages.png`,
`ingenieur-ressenti.png`, `ingenieur-reglage-fichiers.png`.

### 6. Profil et résultats de course

Corde lit les fichiers de résultats du jeu : classements, positions, incidents, et un profil qui rassemble
toutes tes courses par circuit et par voiture.

Images : `profil.png`, `resultat-course.png`.

### 7. Courses en ligne et rappels

Le planning en ligne de Le Mans Ultimate : courses quotidiennes, hebdomadaires, événements spéciaux,
championnats, avec circuit, durée, catégories et niveaux SR / DR requis. Tu choisis tes départs ; Corde
affiche le compte à rebours et te prévient avant le départ, sur le PC et sur le téléphone. L'inscription se
fait toujours dans le jeu. Le planning est désactivé par défaut ; une fois activé, Corde lit deux pages
publiques (racecontrol.gg et lmu.tools), sans compte. Planning non officiel.

Image : `courses.png`.

### 8. L'application iPhone

La même interface dans une vraie application : onglets en bas, notifications, et la page **Courses sans PC**
(le téléphone lit lui-même le planning et sonne les rappels, application fermée). **Widgets** pour l'écran
d'accueil et l'écran verrouillé (prochaines courses, mes courses, « Courses en images »), **compte à rebours
dans la Dynamic Island**, tableau de bord de course, et **« Voir une démonstration »** pour l'essayer sans PC.

Distribution : en test par **TestFlight, sur invitation**. Elle n'est **pas sur l'App Store**.

Images : `iphone.png` (aperçu du mode application), `widgets-iphone.jpg` (planche d'essai des widgets et de
la Dynamic Island), `widget-courses-en-images.jpg` (planche d'essai du widget « Courses en images » ; les
photos y sont des dessins d'exemple).

### 9. L'application Android

La même application pour Android 8 ou plus récent, **sans aucune bibliothèque Google** : courses sans PC,
rappels en notifications à l'heure exacte, tableau de bord de course, démonstration sans PC. Pas encore de
widgets ni de compte à rebours.

Distribution : par un fichier APK à installer à la main, joint aux versions sur GitHub. **Au 14 novembre
2026, ce fichier n'est pas encore publié**, et l'application n'a été essayée que sur un émulateur. Elle
n'est **pas sur le Play Store**.

### 10. Dans OBS, pour les streamers

Chaque widget, ou une disposition entière, s'affiche sur fond transparent par-dessus le jeu dans une source
« Navigateur » d'OBS. Une page de Corde donne l'adresse à copier, la taille conseillée et un aperçu.

Images : `obs-overlay.png`, `obs-page.png`.

### 11. Corde serveur (Raspberry Pi), facultatif

Un petit serveur chez soi, toujours allumé, qui garde les sessions, l'ingénieur, le profil et les courses, et
les montre au téléphone PC éteint. Seuls les appareils appairés (code à 8 chiffres) y accèdent ; il ne
s'ouvre jamais sur Internet (de l'extérieur, on passe par son propre VPN).

Image : `serveur.png`.

### Mises à jour

Depuis l'application Windows : Réglages → Mises à jour → Rechercher une mise à jour, puis Installer et
redémarrer. Chaque version est signée et vérifiée avant l'installation ; rien ne s'installe sans un clic.

## Où ça tourne

| Appareil | Comment | État |
|---|---|---|
| PC Windows | Installeur depuis la page des versions | Utilisé avec le jeu |
| Téléphone ou tablette, navigateur | Même Wi-Fi que le PC, QR code à scanner | Fait |
| iPhone | Application, par TestFlight sur invitation | En test |
| Android | Application, fichier APK | Fichier pas encore publié |
| Raspberry Pi 5 / Linux | Corde serveur, archive `corde-server-<version>-linux-arm64.tar.gz` | Pas encore essayé sur un vrai Pi |

## Nouveautés

- **1.2.0 (15 novembre 2026)** — le tableau de bord de course se choisit aussi sur le PC, dans le menu des
  dispositions ; le Studio, pour composer ses propres tableaux de bord et les partager par fichier.
- **1.1.0 (14 novembre 2026)** — tableau de bord de course pour le téléphone (quatre pages, écran allumé),
  utilisable aussi sur tablette et second écran ; « Voir une démonstration » dans les applications.
- **1.0.0 (14 novembre 2026)** — première version officielle ; application Android (première version).
- **Avant** — widgets iPhone, Dynamic Island et « Courses en images » ; application iPhone ; Corde serveur ;
  widgets dans OBS ; mises à jour depuis l'application ; courses en ligne ; ingénieur de course.

Le détail de chaque version est dans le [journal des versions](../CHANGELOG.md).

## Ce qui arrive (pas encore disponible)

À présenter comme « à venir », sans date :

- Dans le Studio : images et logos, formules.
- Widgets sur Android.
- Publication sur l'App Store et le Play Store : pas décidée.
- Signature de l'installeur Windows (aujourd'hui, Windows affiche un avertissement à l'installation).
- D'autres simulateurs.

## Engagements

- **Tout reste chez toi** : pas de compte, pas de cloud, pas de télémétrie d'usage, pas de publicité.
- Corde ne contacte Internet que pour ce que tu actives (planning des courses, extracteur LLM optionnel) et
  pour chercher une mise à jour (désactivable).
- L'accès depuis le téléphone est fermé par défaut ; le port de Corde ne s'ouvre jamais sur Internet.
- Corde n'écrit dans le dossier du jeu que sur demande explicite, et n'écrase jamais un fichier.
- Code source public, sous licence libre.

Détails : [PRIVACY.md](../PRIVACY.md), [SECURITY-MODEL.md](SECURITY-MODEL.md).

## Liens utiles

- Site officiel : <http://corderacing.com/>
- Télécharger la dernière version : <https://github.com/dultra123/corde/releases/latest>
- Toutes les versions : <https://github.com/dultra123/corde/releases>
- Version et lien de l'installeur lisibles par un programme :
  <https://github.com/dultra123/corde/releases/latest/download/latest.json>
- Guide d'installation et d'utilisation : [TESTING-ON-PC.md](TESTING-ON-PC.md)
- Application iPhone : [IOS.md](IOS.md) · application Android : [ANDROID.md](ANDROID.md)
- Corde serveur : [SERVEUR.md](SERVEUR.md) · accès depuis l'extérieur : [ACCES-EXTERIEUR.md](ACCES-EXTERIEUR.md)
- Signaler une faille, en privé : onglet *Security* du dépôt → *Report a vulnerability*
- Une image du dépôt par son adresse directe :
  `https://raw.githubusercontent.com/dultra123/corde/main/docs/images/<fichier>`
- Icône : `apps/desktop/assets/corde-icon.svg` et `corde-icon-1024.png`

## Mentions à garder

- Corde n'est pas affiliée à Studio 397 ni à Motorsport Games. « Le Mans Ultimate » est une marque de ses
  ayants droit.
- Licence : GPL-3.0-or-later. Copyright (C) 2026 dultra__ et les contributeurs.
- Polices Barlow et Barlow Semi Condensed : SIL Open Font License 1.1.
- Le planning des courses est non officiel.

## À ne pas écrire

Pour rester exact :

- pas « disponible sur l'App Store » ni « sur le Play Store » (ce n'est pas le cas) ;
- pas « application officielle » de Le Mans Ultimate ;
- pas « installeur signé » ni « sans avertissement Windows » ;
- pas d'autre jeu que Le Mans Ultimate ;
- pas de promesse de date pour ce qui est « à venir » ;
- l'auteur est présenté sous son pseudonyme, **dultra__**.
