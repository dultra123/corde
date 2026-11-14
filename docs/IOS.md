# Corde sur iPhone

> Version 0.10.0 — widget « Courses en images » (0.9.0 : widgets et compte à rebours dans la Dynamic
> Island ; iOS 17 au minimum ; 0.8.0 : première version). Décisions :
> [ADR 0011](adr/0011-application-mobile-capacitor.md),
> [ADR 0012](adr/0012-widgets-activite-en-direct.md).
> Sécurité : [SECURITY-MODEL.md §8](SECURITY-MODEL.md#8-lapplication-iphone). Vie privée :
> [PRIVACY.md](../PRIVACY.md#application-iphone).

## 1. Ce que fait l'application

C'est **la même interface que dans le navigateur**, dans une vraie application iPhone (icône, écran de
lancement, notifications, onglets en bas de l'écran).

| Écran | Avec un PC ou un serveur Corde | Sans rien |
|---|---|---|
| **Accueil** | état du jeu et de la session, dernière session, dernières courses, carte « Ton serveur Corde » | défilé des courses en ligne (lu par le téléphone), carte « Ajouter un serveur » |
| **Direct** | le tableau de bord en direct (le téléphone peut se tourner en paysage sur cette page) | explique comment ajouter le PC |
| **Sessions**, **Ingénieur**, **Profil** | comme dans le navigateur | expliquent comment ajouter le PC |
| **Courses** | **toujours celles du téléphone** | planning lu par le téléphone, tes choix, tes rappels en **notifications** (application fermée, téléphone verrouillé) |
| **Mes serveurs** (Accueil → « Mes serveurs ») | ajouter un PC ou un serveur Corde (adresse ou QR code), tester, choisir le serveur actif, appairer un serveur Corde avec son code | |

- **Courses sans PC** : le téléphone lit lui-même `racecontrol.gg` et `lmu.tools` (seulement ces deux pages,
  seulement quand tu as activé le planning, au plus toutes les 10 / 60 minutes), garde tes choix et tes
  réglages, et programme les rappels comme notifications de l'iPhone. C'est ce qui remplace le script
  « LMU Courses ». Les choix du téléphone et ceux du PC restent **séparés**.
- **L'autorisation des notifications** est demandée **à ton premier choix de course**, pas au lancement.
- **Le PC** : sur le PC, Accueil → « Activer l'accès depuis mon téléphone » (Corde 0.8 ou plus récent sur le
  PC), puis, dans l'application, Mes serveurs → saisir l'adresse affichée (`192.168.1.20:8471`) ou
  « Scanner le QR code ». Le téléphone doit être sur le même Wi-Fi.
- **Le serveur Corde (Raspberry Pi)** : ajouter son adresse (sur le Wi-Fi, ou `10.8.0.1` dans ton VPN), puis
  « Appairer ce téléphone » avec un code obtenu sur le serveur (`sudo corde-server pair`).
- Au premier contact avec un serveur, l'iPhone demande l'autorisation **« Réseau local »** : il faut
  l'accepter (sinon : Réglages de l'iPhone → Confidentialité et sécurité → Réseau local → Corde).

## 2. Widgets et Dynamic Island

### Ajouter un widget

- **Écran d'accueil** : appui long sur un espace vide → **Modifier** (en haut à gauche) → **Ajouter un
  widget** → chercher **Corde** → choisir le widget et sa taille → **Ajouter le widget**.
- **Écran verrouillé** : appui long sur l'écran verrouillé → **Personnaliser** → **Écran verrouillé** →
  toucher la zone des widgets sous l'heure → **Corde** → **Prochain départ**.
- Il faut avoir ouvert Corde au moins une fois depuis l'installation de la 0.9 : avant, les widgets disent
  « Ouvre Corde pour actualiser ».

### Ce que montre chacun

| Widget | Tailles | Ce qu'il montre |
|---|---|---|
| **Courses** | petit, moyen, grand | les prochaines courses du planning (la prochaine ; 3 ; jusqu'à 6), à travers tes filtres de la page Courses (catégories, niveaux) : catégorie en couleur (quotidiennes ambre, hebdomadaires bleu, événements spéciaux rouge, RC Pro violet ; pour les quotidiennes, la couleur du niveau), nom, circuit et classes, heure de départ et temps restant |
| **Courses en images** | petit, moyen, grand | **une course par page**, qui change toute seule chaque minute : la photo de la course en fond (sans photo : un fond aux couleurs de la catégorie avec le nom de la piste), la catégorie / le niveau, le **SR et le DR requis** (cadenas rouge quand ton profil est en dessous, réglages de la page Courses), « Ma course » si le départ affiché est choisi, « 2/6 », le nom, piste · durée (et les classes en grand), l'heure du prochain départ et le temps restant ; en moyen et en grand, les départs suivants (ceux que tu as choisis en ambre) ; pour un événement spécial, ses dates. Les mêmes courses que le widget « Courses », dans le même ordre, 8 au plus ; en petit, la première page est ton prochain départ choisi. Boutons **›** (course suivante) et, en moyen et en grand, **‹** (précédente) |
| **Mes courses** | petit, moyen | le compte à rebours jusqu'à ton prochain départ choisi (et, en moyen, les trois suivants) ; « Choisis un départ dans Corde » quand il n'y en a pas |
| **Prochain départ** | écran verrouillé : rectangulaire, en ligne, rond | ton prochain départ choisi, à défaut la prochaine course du planning : nom, heure, compte à rebours |

Un toucher sur un widget ouvre la page **Courses** (sauf sur les boutons ‹ › du widget en images). Une heure
précédée de « ≈ » est **estimée** : le site ne
montre que les prochaines heures, Corde prolonge les quotidiennes au même rythme pour que le widget reste
juste sans ouvrir l'application.

### Le compte à rebours (Dynamic Island, écran verrouillé)

- Il démarre **tout seul** quand ton prochain départ choisi est dans **moins de 2 heures** et que Corde est
  ouverte (à l'ouverture, au retour dans l'application, ou quand tu choisis un départ proche). Pour un départ
  plus lointain (jusqu'à 8 heures avant) : bouton **Compte à rebours** sur ce départ, dans « Mes courses ».
- Un seul à la fois, toujours pour le départ choisi le plus proche.
- Il s'arrête quand tu retires le départ ou coupes le réglage. **Après le départ**, il affiche « Départ
  donné » et reste jusqu'à la prochaine ouverture de Corde (ou jusqu'à ce que tu le balaies) : l'application
  ne tourne pas en arrière-plan pour l'enlever.
- Réglage : page Courses → **Rappels** → « Compte à rebours sur l'écran verrouillé et dans la Dynamic
  Island » (activé par défaut). iOS a aussi le sien : Réglages de l'iPhone → Corde → **Activités en direct**.

### Quand ils se mettent à jour

- À chaque changement fait dans Corde (planning lu, départ choisi ou retiré, filtres, correction d'heure,
  langue), Corde écrit les données des widgets et demande à iOS de les redessiner.
- Entre deux ouvertures, les comptes à rebours tournent seuls et la liste avance d'elle-même à chaque
  départ. Le planning, lui, n'est relu que quand Corde est ouverte (comme la page Courses).
- **Courses en images** : iOS ne sait pas animer un widget en continu ; Corde lui donne d'avance une page par
  minute pour l'heure qui vient, puis une toutes les 5 minutes pendant 3 heures (si iOS tarde à redemander
  la suite), plus une à chaque départ. Le bouton › passe à la course suivante tout de suite ; tous les
  widgets « Courses en images » avancent ensemble. Les **photos** sont téléchargées par Corde (jamais par le
  widget), au plus 8, quand le planning est activé et que Corde est ouverte, puis gardées réduites sur le
  téléphone (48 Kio chacune au plus). Une photo pas encore téléchargée (ou qui n'a pas pu l'être) laisse
  le fond dessiné ; un nouvel essai a lieu 15 minutes plus tard, à la prochaine mise à jour.
- **Limites** : sans ouvrir Corde pendant plus de 4 jours, ou une fois passés tous les horaires écrits (deux
  jours environ), les widgets disent « Ouvre Corde pour actualiser ». Un changement du site (nouvelle
  semaine de courses) n'apparaît qu'à la prochaine ouverture. iOS décide du moment exact où il redessine un
  widget : quelques secondes de retard sont possibles.
- **Diagnostic** (si un widget reste vide) : page Courses → Rappels → **Diagnostic des widgets**. Il dit si
  les données sont écrites (et à quelle heure), le code d'iOS en cas d'échec, si les activités en direct sont
  autorisées, combien de photos de courses sont gardées (et leur poids, et le dernier code d'erreur), et la
  version d'iOS. C'est ce qu'il faut nous recopier.

### Ce qu'il faut essayer sur l'iPhone (0.9, première fois)

1. Ouvrir Corde, page Courses → Rappels → **Diagnostic des widgets** : « écrites à … », « groupe trouvé ».
   Sinon, recopier les lignes.
2. Ajouter le widget **Courses** (moyen) : il doit montrer les mêmes courses que la page (planning activé).
   Toucher le widget : Corde s'ouvre sur Courses — application déjà ouverte, puis **fermée** (balayée vers
   le haut dans le sélecteur d'applications).
3. Choisir un départ dans moins de 2 heures, revenir à l'écran d'accueil : le compte à rebours doit être
   dans la Dynamic Island ; toucher l'île l'agrandit (appui long), un toucher ouvre Courses.
4. Verrouiller : le compte à rebours sur l'écran verrouillé ; ajouter **Prochain départ** sous l'heure.
5. Retirer le départ dans Corde : le compte à rebours disparaît.
6. Laisser passer un départ choisi (compte à rebours actif) : « Départ donné » à l'heure, puis disparition à
   la réouverture de Corde.
7. Le lendemain, sans ouvrir Corde : les widgets montrent encore des courses (horaires « ≈ »).

### Ce qu'il faut essayer sur l'iPhone (0.10, widget « Courses en images »)

1. Planning activé, ouvrir Corde et attendre quelques secondes, puis page Courses → Rappels → **Diagnostic
   des widgets** : « Photos des courses … : N sur le téléphone (… Ko) ». Avec 0 et un code : le recopier.
2. Ajouter **Courses en images** en moyen : une course avec sa photo, le SR / DR requis (renseigner son
   profil et les exigences dans les réglages de la page Courses pour voir le cadenas), « 1/8 ».
3. Attendre une minute : la page change (avec un glissement). Toucher **›** : la course suivante tout de
   suite ; **‹** : la précédente. Toucher ailleurs : Corde s'ouvre sur Courses.
4. En petit : la première page est ton prochain départ choisi (« Ma course », compte à rebours).
5. Une photo claire et une photo sombre : le texte reste lisible sur les deux (sinon, une capture).
6. Couper le planning dans Corde : le widget dit « Active le planning dans Corde » et le diagnostic montre
   0 photo.

## 3. Comment sortent les versions iPhone

Personne dans le projet n'a de Mac : **l'application n'est compilée que par GitHub**, sur un Mac de GitHub
(gratuit pour un dépôt public), par le workflow **iOS** (`.github/workflows/ios.yml`). Il tourne à chaque
poussée sur `main` qui touche `apps/mobile/`, `packages/ui/` ou le workflow lui-même, et à la demande
(GitHub → Actions → **iOS** → **Run workflow**).

1. **Simulateur** (toujours, sans aucun secret) : compile l'application et son extension de widgets pour le
   simulateur d'iPhone (signature locale « ad hoc », sans certificat), puis la lance quatre fois : Accueil ;
   Courses, par le lien `corde://races` que l'application ouvre elle-même (comme un toucher de widget) ;
   planches des **vues des widgets et du compte à rebours**, dessinées par l'application avec des données
   inventées (fr / en, clair / sombre) ; une activité d'exemple dans la **Dynamic Island** (dessinée par la
   vraie extension, Réglages au premier plan). Les images sont écrites **dans le journal** entre les lignes
   `-----BEGIN CORDE SCREENSHOT <nom>-----` et `-----END CORDE SCREENSHOT-----` (JPEG en base64), avec les
   journaux de l'application et une liste de contrôles (« ✓ bloc des widgets écrit… »). Les mêmes fichiers
   sont joints à l'exécution (artefact `ios-smoke`, téléchargeable par le propriétaire du dépôt). Puis il
   compile aussi la configuration Release pour un iPhone, sans signature, pour voir ses erreurs avant
   `main`.
2. **Clé d'API** (une minute) : essaie les scripts (signature avec un faux serveur d'Apple et un faux
   `codesign`, cohérence du projet Xcode), puis contrôle que les quatre secrets sont utilisables (sans
   jamais afficher une valeur).
3. **TestFlight** (seulement sur `dultra123/corde`, depuis `main`, jamais sur un fork, et seulement si les
   **quatre secrets** du §4 existent) : avec la clé d'API, crée pour cette exécution un **certificat de
   distribution** et un **profil App Store par identifiant** (l'application, et l'extension de widgets dont
   l'identifiant est créé s'il manque ; ceux de l'exécution précédente sont retirés), compile et signe,
   contrôle la signature de l'application et de l'extension, puis l'envoie vers TestFlight. Le numéro de
   build est le numéro d'exécution du workflow (il augmente toujours, comme Apple l'exige), le même pour
   l'application et l'extension ; la version affichée est celle de `apps/mobile/package.json` (0.10.0).

Lancé à la main sur une autre branche que `main`, le workflow s'arrête après les deux premiers jobs : c'est
le moyen d'essayer du code Swift avant de le fusionner.

Sans les secrets, le job TestFlight affiche une note et ne fait rien : le simulateur suffit à vérifier que
tout compile.

**Voir une capture du journal** : copier les lignes entre les deux marqueurs dans un fichier `capture.txt`,
puis `base64 -d capture.txt > capture.jpg` (Linux, macOS) ou, sous Windows (PowerShell) :
`[IO.File]::WriteAllBytes("capture.jpg", [Convert]::FromBase64String((Get-Content capture.txt -Raw)))`.

## 4. Ce que le propriétaire fait côté Apple (une fois, dans l'ordre)

Tout se fait dans un navigateur, sans Mac.

1. **Attendre l'activation du compte développeur** (e-mail d'Apple « Welcome to the Apple Developer
   Program »).
2. **Le Team ID** : [developer.apple.com/account](https://developer.apple.com/account) → **Membership
   details** → **Team ID** (10 caractères, lettres et chiffres). → secret `APPLE_TEAM_ID`. C'est aussi la
   valeur affichée à l'étape 3 sous **App ID Prefix** (« … (Team ID) »).
3. **L'identifiant de l'application** : developer.apple.com → **Certificates, IDs & Profiles** →
   **Identifiers** → **+** → **App IDs** → **Continue** → **App** → **Continue**. La page **Register an
   App ID** se remplit ainsi, champ par champ :

   | Champ | Quoi mettre |
   |---|---|
   | **Platform** | ne rien changer |
   | **Description** | `Corde` — un simple nom pour s'y retrouver dans la liste, jamais montré au public ; lettres et chiffres seulement (Apple refuse `@ & * ' "`) |
   | **App ID Prefix** | déjà rempli : c'est le Team ID (étape 2), rien à choisir |
   | **Bundle ID** | **Explicit**, puis `io.github.dultra123.corde` — exactement, en minuscules, sans espace |
   | **Capabilities** (longue liste de cases) | **ne rien cocher** ; si une case est déjà cochée et grisée, la laisser |
   | onglets **App Services** et **Capability Requests** | ne rien cocher, ne rien demander |

   Puis **Continue** → **Register**.

   Pourquoi aucune case : Corde ne demande à Apple aucun service particulier. Les rappels de course sont
   des notifications **locales** (programmées par le téléphone lui-même) : la case **Push Notifications**
   ne sert qu'aux notifications envoyées par un serveur. Rien n'est définitif : une capacité s'ajoute plus
   tard en rouvrant l'identifiant (**Identifiers** → `io.github.dultra123.corde` → cocher → **Save**). Les
   widgets (0.9) n'en demandent aucune : ils partagent leurs données par le trousseau, que tous les profils
   autorisent déjà (ADR 0012), et leur propre identifiant est créé tout seul par la compilation. Le **Bundle
   ID**, lui, ne se modifie plus après l'enregistrement.
4. **La fiche de l'application** : [App Store Connect](https://appstoreconnect.apple.com) → **Apps** → **+**
   → **Nouvelle app**. La fenêtre se remplit ainsi :

   | Champ | Quoi mettre |
   |---|---|
   | **Plateformes** | cocher **iOS** seulement |
   | **Nom** | `Corde` ; si Apple répond que le nom est déjà utilisé, un nom proche, par exemple `Corde Racing` (le nom sous l'icône de l'iPhone reste « Corde ») |
   | **Langue principale** | **Français** |
   | **Identifiant de lot** | `Corde - io.github.dultra123.corde` dans la liste (c'est l'identifiant de l'étape 3) |
   | **UGS** | `corde-ios` — une référence interne, jamais montrée au public |
   | **Accès utilisateur** | **Accès complet** : tous les membres du compte voient l'application (« Accès limité » sert à la cacher à une partie d'une équipe) ; le bouton **Créer** reste gris tant qu'aucun des deux n'est choisi |

   Puis **Créer**. La fiche publique (description, captures) n'est **pas** nécessaire pour TestFlight.
   Après **Créer**, App Store Connect ouvre la page de l'application (« Distribution », version 1.0, page
   produit) : **ne rien y remplir** et ne pas cliquer sur « Ajouter pour vérification ». Cette page ne sert
   qu'à une sortie publique sur l'App Store.
5. **La clé d'API** : App Store Connect → **Utilisateurs et accès** → onglet **Intégrations** → **App Store
   Connect API** → **Clés d'équipe** → **Générer une clé API** (la première fois : **Demander l'accès** et
   accepter les conditions) → nom `GitHub Actions`, accès **Admin** → **Générer**.

   Pourquoi **Admin** : à chaque envoi, la compilation crée avec cette clé le certificat qui signe
   l'application (voir « Ce que tu verras dans ton compte Apple », plus bas). C'est le seul accès avec
   lequel l'envoi a été vérifié ; avec « Gestionnaire d'apps », le premier essai s'est arrêté sur une erreur
   de permission. Une clé ne se modifie pas après coup : pour changer son accès, il faut en générer une
   nouvelle. Cette clé peut tout faire dans App Store Connect sauf ce qui est réservé au titulaire du compte
   (contrats, paiements) : elle ne va **que** dans les secrets GitHub.

   Où est « Utilisateurs et accès » : dans la barre du haut (Apps, Xcode Cloud, Tendances, Rapports,
   Business…). Quand la fenêtre est étroite, il est rangé derrière les **trois points « … »** à droite de
   « Business ». Lien direct :
   [appstoreconnect.apple.com/access/integrations/api](https://appstoreconnect.apple.com/access/integrations/api).

   Puis :
   - **Télécharger la clé** (`AuthKey_XXXXXXXXXX.p8`) — **une seule fois possible** ; ne l'envoyer à
     personne, ne la mettre dans aucun dépôt ;
   - noter l'**ID de clé** (colonne « ID de clé ») et l'**ID d'émetteur** (« Issuer ID », en haut de la
     page).
6. **Les quatre secrets** : GitHub → dépôt `dultra123/corde` → **Settings** → **Secrets and variables** →
   **Actions** → **New repository secret**, quatre fois :

   | Nom | Valeur |
   |---|---|
   | `APPSTORE_API_KEY` | le **contenu** du fichier `.p8` (l'ouvrir avec le Bloc-notes, tout copier, lignes `-----BEGIN PRIVATE KEY-----` et `-----END PRIVATE KEY-----` comprises) |
   | `APPSTORE_API_KEY_ID` | l'ID de clé (10 caractères) |
   | `APPSTORE_API_ISSUER_ID` | l'ID d'émetteur (de la forme `xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx`) |
   | `APPLE_TEAM_ID` | le Team ID (étape 2) |

7. **Lancer** : GitHub → **Actions** → **iOS** → **Run workflow** → `main`. Compter 15 à 30 minutes.
8. **TestFlight** : App Store Connect → **Apps** → l'application → onglet **TestFlight**. Le build apparaît après le traitement par
   Apple (souvent 5 à 30 minutes après la fin du workflow). La question de chiffrement est déjà réglée
   (`ITSAppUsesNonExemptEncryption = false` : Corde n'utilise que le chiffrement d'iOS). Créer un groupe
   de **testeurs internes**, s'y ajouter, puis installer l'application **TestFlight** sur l'iPhone et
   accepter l'invitation : Corde s'installe.

Si le job **« Clé d'API (script et secrets) »** échoue, un secret est mal collé : son message dit lequel
et pourquoi (« on dirait un identifiant », « pas de ligne END »…), sans jamais afficher la valeur. Corriger
le secret (GitHub → Settings → Secrets and variables → Actions → le crayon à droite du nom → coller la
nouvelle valeur → **Update secret**), puis relancer (**Actions** → **iOS** → **Run workflow**). Le secret
`APPSTORE_API_KEY` accepte le contenu du fichier `.p8` même avec des fins de ligne Windows ou sur une seule
ligne ; l'important est de coller **tout** le fichier.

Si l'étape **« Certificat et profil de distribution »** échoue :

- avec `403` ou `FORBIDDEN` : la clé d'API n'a pas l'accès **Admin**. Générer une nouvelle clé avec l'accès
  **Admin** (étape 5), remplacer les deux secrets `APPSTORE_API_KEY` (contenu du nouveau fichier `.p8`) et
  `APPSTORE_API_KEY_ID` (nouvel ID de clé) — l'ID d'émetteur et le Team ID ne changent pas —, relancer, puis
  révoquer l'ancienne clé (page des clés → **Modifier** → cocher la clé → **Révoquer la clé**) ;
- avec `409` et « nombre maximal de certificats » : le compte a déjà trop de certificats de distribution.
  En révoquer un sur developer.apple.com → **Certificates, IDs & Profiles** → **Certificates**, puis relancer.

Si l'envoi échoue parce que l'application « n'existe pas »,
l'étape 4 manque.

### Ce que tu verras dans ton compte Apple

Sur developer.apple.com → **Certificates, IDs & Profiles**, la compilation laisse ceci, à ne pas
toucher :

- sous **Certificates**, un certificat **Distribution** (créé « via API ») ;
- sous **Profiles**, deux profils nommés **`Corde CI <numéro>`** (l'application) et **`Corde CI <numéro>
  widgets`** (l'extension de widgets, depuis la 0.9) ;
- sous **Identifiers**, à côté de `io.github.dultra123.corde`, l'identifiant **Corde Widgets**
  (`io.github.dultra123.corde.widgets`), créé au premier envoi de la 0.9 et gardé ensuite. Aucune case à
  cocher dessus.

À chaque nouvel envoi, la compilation **révoque ce certificat et supprime ces profils**, puis en crée de
nouveaux : il n'y a jamais plus d'un certificat à la fois. Elle ne touche qu'au certificat attaché à un profil
« Corde CI … » — jamais à un autre. Apple peut envoyer un e-mail « certificat révoqué » à ce moment-là :
c'est normal, et sans effet sur les builds déjà envoyés. La clé privée du certificat n'existe que le temps
de la compilation, sur la machine de GitHub ; elle n'est enregistrée nulle part.

## 5. Durée de vie d'un build

- Un build TestFlight **expire 90 jours** après son envoi : l'application ne s'ouvre plus. Un nouveau build
  part à chaque poussée sur `main` qui touche l'application, ou à la main (**Run workflow**) ; TestFlight
  propose alors la mise à jour.
- Une mise à jour **garde les données** de l'application (serveurs, appairage, courses choisies, réglages).
  Supprimer l'application les efface.
- Les testeurs internes (jusqu'à 100 membres de l'équipe App Store Connect) n'attendent aucune
  vérification d'Apple. Des testeurs externes demanderaient une vérification (« Beta App Review ») et une
  fiche plus complète.

## 6. Limites connues

- **iPhone seulement, iOS 17 au minimum** depuis la 0.9 (un iPad l'ouvre en mode iPhone). Pas d'Android.
- **Widgets et compte à rebours** : limites au §2 (mise à jour à l'ouverture de Corde, « Départ donné »
  jusqu'à la prochaine ouverture). Pas de widget configurable (catégorie au choix) pour l'instant : ils
  suivent les filtres de la page Courses. **Courses en images** : la page change à la minute (pas plus
  vite : iOS ne redessine pas un widget plus souvent) ; les photos sont gardées dans le trousseau partagé, qui
  **survit à la suppression de l'application** (8 photos de 48 Kio au plus ; réinstaller Corde puis couper
  le planning les efface).
- **Rappels** : iOS garde au plus **64 notifications en attente** par application. Les plus proches sont
  programmées ; les suivantes le sont quand l'application revient au premier plan ou à chaque changement de
  choix. Avec beaucoup de choix et l'application jamais ouverte pendant des jours, des rappels lointains
  peuvent attendre. Les rappels sonnent sans réseau.
- **Le planning n'est lu que quand l'application est ouverte** (pas de lecture en arrière-plan) ; les
  rappels, eux, sonnent application fermée.
- **Le direct est en HTTP non chiffré** sur le réseau de la maison (comme dans le navigateur) ; dehors,
  passer par ton VPN ([ACCES-EXTERIEUR.md](ACCES-EXTERIEUR.md)).
- **La clé d'appairage** est gardée dans le stockage de l'application (Capacitor Preferences, c'est-à-dire
  `UserDefaults`, dans le bac à sable de l'application), pas dans le trousseau iOS : pas de module officiel
  pour le trousseau. Elle est sauvegardée avec l'iPhone (sauvegardes iCloud ou chiffrées).
- **Le Corde du PC doit être en 0.8 ou plus** : les versions précédentes refusent l'origine de l'application
  (« origine non autorisée »).
- Rien de tout cela n'a tourné sur un vrai iPhone avant la première exécution de la CI : la liste de ce qui
  n'a pas pu être vérifié est dans le compte rendu du lot (et `docs/STATUS.md`).

## 7. Pour les développeurs

```
apps/mobile/
├─ package.json            # @corde/mobile : Capacitor 8 (core, ios, cli) et les modules officiels
├─ capacitor.config.json   # identifiant io.github.dultra123.corde, nom Corde, fond sombre, notifications
├─ assets/corde-reminder.wav, scripts/make-assets.mjs, scripts/finish-web.mjs
├─ www/                    # l'interface construite en mode mobile (ignoré par git)
├─ scripts/*.test.*       # essais des scripts de la CI et du projet Xcode (voir plus bas)
└─ ios/App/                # projet Xcode généré par `cap add ios` (gabarit Swift Package Manager), complété
   ├─ App.xcodeproj        #   deux cibles (App, CordeWidgets), iOS 17, iPhone, 0.10.0, schéma partagé « App »
   ├─ App/                 #   cible de l'application
   │  ├─ Info.plist        #     réseau local, ATS, appareil photo, corde://, chiffrement, activités en direct
   │  ├─ App.entitlements  #     keychain-access-groups (trousseau partagé avec les widgets)
   │  ├─ SceneDelegate.swift, CordeBridgeViewController.swift   # contrôleur de Capacitor + module local
   │  ├─ CordeNativePlugin.swift                                  # pont « CordeNative »
   │  ├─ WidgetImages.swift #    photos du widget en images : téléchargement, réduction (application seule)
   │  └─ WidgetShots.swift #     build Debug : images des vues pour la CI, lien ouvert par l'application
   ├─ Shared/              #   compilé dans les deux cibles : SharedStore, WidgetData, CordeRaceAttributes,
   │                       #   CordeStyle, WidgetViews, SlideViews, ActivityViews, SampleData
   ├─ CordeWidgets/        #   l'extension : widgets (dont SlidesWidget : chronologie, AppIntent ‹ ›),
   │                       #   activité en direct, Info.plist, .entitlements
   └─ CapApp-SPM/          #   paquet Swift écrit par `cap sync ios` (ne pas modifier à la main)
```

```bash
pnpm --filter @corde/mobile build          # interface en mode mobile → apps/mobile/www (+ son des rappels)
cd apps/mobile && pnpm exec cap sync ios    # copie www dans le projet Xcode, réécrit CapApp-SPM/Package.swift
node apps/mobile/scripts/make-assets.mjs    # icône, écran de lancement, son (Playwright + Chromium)
node packages/ui/scripts/screenshot-app.mjs # aperçu Chromium avec un faux pont Capacitor → docs/images/iphone.png
```

- Le code de l'application est dans `packages/ui/src/mobile/` ; il n'est chargé que par le build `mobile`
  (`import.meta.env.MODE === "mobile"`), jamais par le navigateur. Les pages partagées n'en voient que des
  « emplacements » (`mobile/slots.ts`) et la source des courses (`races/backend.ts`).
- La logique (adresses, planning, rappels, serveurs) est testée avec des doubles des modules Capacitor
  (`mobile/doubles.ts`).
- `cap sync ios` fonctionne sous Linux. `Package.swift` contient des chemins vers le magasin de pnpm
  (`node_modules/.pnpm/…`) : la CI le réécrit à chaque exécution.
- Liens `corde://home`, `corde://races`, `corde://servers`, `corde://live`, `corde://sessions`,
  `corde://profile` : ouvrent une page, rien d'autre (les widgets et l'activité ouvrent `corde://races`).
  Sur le simulateur, `xcrun simctl openurl` fait apparaître la fenêtre « Ouvrir dans “Corde” ? » : la CI
  lance l'application avec `-CordeOpenURL corde://races`, et c'est elle qui ouvre le lien (build Debug).

### Widgets : cible, données, pont

- **Cible `CordeWidgets`** (`io.github.dultra123.corde.widgets`), embarquée dans `PlugIns/` par la phase
  « Embed Foundation Extensions ». Mêmes version et numéro de build que l'application (le workflow passe
  `MARKETING_VERSION` et `CURRENT_PROJECT_VERSION` à `xcodebuild`). Le projet est édité à la main ;
  `node --test apps/mobile/scripts/xcodeproj.test.mjs` vérifie chaque référence, les deux cibles,
  l'embarquement et les réglages. `cap sync ios` ne touche pas au projet (seulement à `Package.swift`).
- **Données** : l'application écrit un bloc JSON dans un élément du trousseau partagé
  (`Shared/SharedStore.swift` ; groupe `$(AppIdentifierPrefix)io.github.dultra123.corde.shared`, déclaré
  dans les deux `.entitlements` et dans la clé `CordeKeychainGroup` des deux `Info.plist` ; dans les
  droits de l'application, il vient après le groupe propre à l'application, pour que rien d'autre n'y
  soit rangé par défaut). Sur le
  simulateur compilé sans droits, `SharedStore` retombe sur le trousseau de l'application seule.
- **Format v1** (produit par `packages/ui/src/mobile/widgets.ts`, `buildPayload` ; décodé par
  `Shared/WidgetData.swift`) — heures en millisecondes UTC, affichage dans le fuseau du téléphone :

  ```jsonc
  {
    "v": 1,                      // version ; une autre → « Ouvre Corde pour actualiser »
    "at": 1794772800000,         // écrit à (plus de 4 jours → « Ouvre Corde »)
    "lang": "fr",                // fr | en (langue de l'interface)
    "sched": "on",               // on | off (planning désactivé) | none (rien pu lire)
    "until": 1794945600000,      // dernier instant annoncé (0 : rien d'annoncé)
    "races": [                   // le planning à travers les filtres, quotidiennes → hebdo → spéciaux → RC Pro
      { "id": "sprint-gt3-beginner", "cat": "daily", "lvl": "beginner", "name": "Sprint GT3",
        "track": "Monza", "cls": ["GT3"], "dur": "20 min",
        "s": [1794773520000, 1794775320000],   // départs connus (spécial : premier jour 00:00)
        "x": [1794777120000],                  // départs estimés (quotidiennes, ≤ 48 h)
        "img": "i3c1f0a9e7b2d4c51",            // facultatif (0.10) : clé de la photo (FNV-1a 64 de l'adresse)
        "req": { "sr": "S2", "dr": "B2", "miss": ["sr"] } },  // facultatif (0.10) : SR / DR requis,
                                               // et ceux au-dessus du profil (jamais les notes du profil)
      { "id": "special-6h", "cat": "special", "lvl": null, "name": "6 Heures", "track": "Bahreïn",
        "cls": [], "dur": "6 h", "s": [1794960000000], "to": 1795046340000 }
    ],
    "mine": [                    // départs choisis à venir, par départ
      { "id": "sprint-gt3-beginner@1794775320000", "race": "sprint-gt3-beginner", "cat": "daily",
        "lvl": "beginner", "name": "Sprint GT3", "track": "Monza", "cls": ["GT3"], "dur": "20 min",
        "t": 1794775320000 }     // + "img", "req" facultatifs, comme une course
    ]
  }
  ```

  Au plus 48 Kio (les horaires estimés sont raccourcis d'abord). Jamais d'adresse de serveur ni de jeton,
  ni l'adresse d'une photo (seulement sa clé). `img` et `req` sont facultatifs : un bloc sans eux (écrit par
  la 0.9) se lit encore. La liste du widget « Courses » à un instant donné est calculée deux fois, à
  l'identique : `racesAt` (TypeScript, testé) et `races(at:)` (Swift) ; les pages du widget en images en
  viennent (`slides(at:small:)`, 8 au plus).
- **Photos du widget « Courses en images »** : après chaque écriture du bloc, `widget-sync.ts` envoie la
  liste des photos des pages (`widgetImages` : le prochain départ choisi, puis les pages, puis les autres
  choix ; 8 au plus ; rien quand le planning est coupé) à `setWidgetImages`, seulement si elle a changé (ou
  15 minutes après un échec). L'application (`App/WidgetImages.swift`, jamais l'extension) télécharge celles
  qui manquent — `URLSession` éphémère, sans cookies ni cache ni `Referer`, `Accept: image/*`, 15 s, 4 Mo au
  plus, `https` seulement, pas d'adresse IP écrite telle quelle ni `localhost` / `.local`, redirections
  comprises —, les réduit (ImageIO, ≤ 720 px, JPEG de qualité 0,6 puis moins, ≤ 48 Kio, sans métadonnées)
  et les range dans le trousseau partagé, **un élément par photo** (service
  `io.github.dultra123.corde.widgets.img`, compte = la clé, même groupe, `AfterFirstUnlock`, jamais
  synchronisé) ; celles qui ne sont plus listées sont supprimées ; `reloadTimelines(ofKind: "CordeSlides")`
  quand quelque chose a changé. L'extension lit la photo au moment du dessin et la réduit encore pour le
  petit widget (480 px).
- **Défilement** (`CordeWidgets/SlidesWidget.swift`) : chronologie d'une entrée par minute pendant 60 min,
  puis toutes les 5 min pendant 3 h, plus chaque départ ; `.after(maintenant + 1 h)`. Chaque entrée porte
  une position ; page = position modulo le nombre de pages. La position continue d'une chronologie à
  l'autre (ancre gardée dans les `UserDefaults.standard` de l'extension) ; ‹ › sont des `Button(intent:)`
  (`SlideStepIntent`, dans l'extension seulement) qui ajoutent ±1 à un décalage gardé au même endroit.
- **Quand c'est écrit** (`mobile/widget-sync.ts`) : au lancement, après chaque lecture du planning, choix,
  retrait, changement de réglage ou de langue, au retour au premier plan ; seulement si le contenu a changé.
  Le dernier planning lu est gardé 3 jours (Preferences) pour les lancements sans réseau.
- **Pont** : module Capacitor local `CordeNative` (`App/CordeNativePlugin.swift`, enregistré par
  `CordeBridgeViewController`) — `setWidgetData({ json })`, `setWidgetImages({ items })` (rend `written`,
  `kept`, `failed`, `removed`, `bytes`, `status`), `activities()`, `startActivity(…)`,
  `endActivities({ ids? })`, `diagnostics()` (avec `images`, `imageBytes`, `imageStatus`). Côté
  TypeScript : `NativePort` (`mobile/ports.ts`), le double
  `FakeNative` (`mobile/doubles.ts`). Rien de cela n'est chargé hors du build `mobile`.
- **Activité en direct** : `CordeRaceAttributes` (Shared), vues dans `Shared/ActivityViews.swift`,
  configuration dans `CordeWidgets/CordeLiveActivity.swift`. Règles : `planActivity` (`mobile/widgets.ts`).
  Réglage du pilote : clé `corde.races.liveActivity` (Preferences, vrai par défaut).
- **Voir les vues sans Mac** : l'application en build Debug, lancée avec `-CordeRenderShots`, dessine toutes
  les vues (`App/WidgetShots.swift`, données de `Shared/SampleData.swift`) ; `-CordeDemoActivity` démarre une
  activité d'exemple. Décoder une planche du journal : même méthode que les captures (§3). Planches
  `widgets-<langue>-<mode>` (widgets 0.9, activité) et `slides-fr-dark`, `slides-fr-light`, `slides-en-dark`
  (« Courses en images » : photos d'exemple dessinées par le code, passées par la vraie réduction). Le même
  lancement fait un aller-retour du magasin des photos (`image store: …`, OSStatus de chaque opération) et
  réduit une image fabriquée de 4000 × 3000 (`image reduce: … ok`). Aucun téléchargement dans la CI.
