# Corde sur Android

> Version 1.0.0 — première version Android (la même que l'iPhone, sans les widgets ni le compte à
> rebours). Décision : [ADR 0013](adr/0013-application-android.md). Sécurité :
> [SECURITY-MODEL.md §9](SECURITY-MODEL.md#9-lapplication-android). Vie privée :
> [PRIVACY.md](../PRIVACY.md#application-android).
>
> **État honnête** : l'application est compilée et essayée sur un émulateur Android par la CI à chaque
> modification ; **elle n'a encore jamais tourné sur un vrai téléphone**. Le §4 dit quoi essayer, dans
> l'ordre, la première fois.

## 1. Ce que fait l'application

C'est **la même application que sur iPhone** ([IOS.md §1](IOS.md#1-ce-que-fait-lapplication)) : la même
interface, avec les onglets en bas de l'écran.

| Écran | Avec un PC ou un serveur Corde | Sans rien |
|---|---|---|
| **Accueil** | état du jeu et de la session, dernière session, dernières courses, carte « Ton serveur Corde » | défilé des courses en ligne (lu par le téléphone), carte « Ajouter un serveur » |
| **Direct** | le tableau de bord en direct (le téléphone peut se tourner en paysage sur cette page) | explique comment ajouter le PC |
| **Sessions**, **Ingénieur**, **Profil** | comme dans le navigateur | expliquent comment ajouter le PC |
| **Courses** | **toujours celles du téléphone** | planning lu par le téléphone, tes choix, tes rappels en **notifications** (application fermée, téléphone verrouillé ou en veille) |
| **Mes serveurs** (Accueil → « Mes serveurs ») | ajouter un PC ou un serveur Corde (adresse ou QR code), tester, choisir le serveur actif, appairer un serveur Corde avec son code | |

- **Pas encore sur Android** : les widgets de l'écran d'accueil et le compte à rebours (sur iPhone, dans la
  Dynamic Island). Ils viendront dans un prochain lot. Le réglage « Compte à rebours… », le bouton
  « Compte à rebours » et le diagnostic des widgets n'apparaissent donc pas sur Android.
- **Le bouton (ou geste) retour** d'Android revient à la page précédente de Corde ; depuis la première page
  ouverte, il ferme l'application.
- **Le PC** : sur le PC, Accueil → « Activer l'accès depuis mon téléphone », puis, dans l'application,
  Mes serveurs → saisir l'adresse affichée (`192.168.1.20:8471`) ou « Scanner le QR code ». Le téléphone doit
  être sur le même Wi-Fi (ou dans ton VPN, [ACCES-EXTERIEUR.md](ACCES-EXTERIEUR.md)).
- **Le serveur Corde (Raspberry Pi)** : ajouter son adresse, puis « Appairer ce téléphone » avec un code
  obtenu sur le serveur (`sudo corde-server pair`). Le nom proposé pour le téléphone est « Android ».
- **Pas d'autorisation « réseau local »** à donner : Android n'en a pas (contrairement à l'iPhone).

## 2. Installer l'application (pour le pilote)

L'application n'est pas sur le Play Store : on l'installe à partir d'un fichier **APK** publié sur GitHub.

1. Sur le téléphone, ouvre la page des versions de Corde :
   <https://github.com/dultra123/corde/releases>. Prends le fichier **`Corde_1.0.0_android.apk`** :
   - dans la version numérotée (**Corde 1.0.0**), s'il y est : c'est la version stable ;
   - sinon dans **Corde — dernière version** (la pré-release `continuous`), refaite à chaque modification.
2. Le navigateur prévient que ce type de fichier peut être dangereux : **Télécharger quand même**.
3. Ouvre le fichier téléchargé (la notification « Téléchargement terminé », ou l'application **Fichiers** →
   Téléchargements).
4. La première fois, Android dit que ton navigateur (ou Fichiers) **n'a pas le droit d'installer des
   applications inconnues** : touche **Paramètres**, active **Autoriser cette source**, puis reviens (flèche
   retour). Le nom exact varie selon le téléphone : « Installer des applis inconnues », « Sources
   inconnues »…
5. **Installer**. Sur un téléphone avec les services Google, **Play Protect** peut afficher « Application
   inconnue » ou proposer d'analyser l'application : **Plus d'infos** → **Installer quand même** (ou
   **Installer sans analyser**). Corde n'est pas sur le Play Store, Play Protect ne la connaît pas ; ce n'est
   pas une détection de virus. Sur un téléphone sans services Google, il n'y a pas cet écran.
6. **Ouvrir**. Tu peux ensuite retirer l'autorisation donnée à l'étape 4 (Paramètres → Applications → ton
   navigateur → Installer des applis inconnues).

**Mettre à jour** : refais exactement la même chose avec le nouvel APK ; Android l'installe par-dessus et
**garde tes données** (serveurs, appairage, courses choisies, réglages). Ça ne marche que si les deux APK
sont signés par la même clé — c'est le cas de tous ceux que publie la CI de Corde. Si Android refuse
(« Application non installée », « conflit avec un package existant »), c'est que l'APK installé n'a pas été
signé par cette clé : désinstalle-le d'abord (les données sont alors perdues).

**Vérifier que l'APK vient bien de Corde** (facultatif) : le journal de la CI (GitHub → Actions → Android →
job « APK signé et publication ») affiche l'empreinte SHA-256 du certificat ; elle ne change jamais d'une
version à l'autre.

**Désinstaller** : appui long sur l'icône → **Désinstaller** (ou Paramètres → Applications → Corde). Tout
ce que l'application garde est effacé.

## 3. Les autorisations, et pourquoi

| Autorisation | Quand Android la demande | Pourquoi |
|---|---|---|
| **Notifications** (Android 13 ou plus récent) | à ton **premier choix de course**, pas au lancement | les rappels des courses. Refusée : les rappels ne sonnent pas (page Courses → Rappels dit comment l'autoriser après) |
| **Alarmes et rappels** | juste après, Corde explique et propose d'ouvrir le réglage (une seule fois ; un bouton **Autoriser** reste sur la page Courses) | pour que le rappel sonne **à la minute près**, même téléphone en veille. Sans elle (refusée par défaut depuis Android 14), le rappel arrive quand même, mais Android peut le **retarder de quelques minutes** quand le téléphone dort |
| **Appareil photo** | seulement quand tu touches **« Scanner le QR code »** | lire le QR code que Corde affiche sur ton PC. Aucune image n'est gardée |
| Internet | jamais (donnée à l'installation) | les serveurs de « Mes serveurs », les deux pages du planning |

Rien d'autre : **pas de localisation**, pas d'accès à tes fichiers, à tes contacts ni au micro. Corde déclare
aussi deux autorisations techniques sans écran : relancer les rappels **après un redémarrage** du téléphone,
et garder le téléphone éveillé le temps d'afficher un rappel.

Le **canal de notification** « Rappels des courses » (Paramètres → Applications → Corde → Notifications)
porte le son de Corde ; tu peux y changer le son, la vibration ou l'affichage sur l'écran verrouillé. Un
second canal, « Default », est créé par le module de notifications : Corde ne s'en sert pas.

## 4. À essayer sur un vrai téléphone (la première fois, dans l'ordre)

Ce que la CI a vérifié sur l'émulateur est au §6. Le reste ne peut l'être que sur un vrai téléphone :

1. **Installation** (§2) : l'icône Corde (le « C » ambre sur fond sombre) apparaît ; avec les icônes à thème
   d'Android 13+, elle prend les couleurs du thème.
2. **Lancement** : écran sombre avec le signe de Corde, puis l'Accueil. Rien ne doit passer **sous la barre
   d'état** (en haut) ni **sous la barre de gestes** (en bas) ; les onglets du bas sont entiers.
3. **Courses sans PC** : page Courses → activer le planning → les courses apparaissent (avec leurs photos).
4. **Premier choix** d'une course qui part dans moins d'une heure, avec un rappel à 5 min (ou « au départ ») :
   Android demande les notifications → **Autoriser** ; Corde propose « Alarmes et rappels » → **OK** →
   active **Autoriser à définir des alarmes et des rappels** → retour. La page Courses dit « Les rappels
   sonnent à la minute près ».
5. **Rappel application fermée** : ferme Corde (bouton des applications récentes → balayer Corde), éteins
   l'écran, attends l'heure. La notification doit arriver **à l'heure**, avec **le son de Corde**, et un
   toucher sur elle ouvre la page Courses.
6. **Redémarrage** : choisis un autre départ, redémarre le téléphone, n'ouvre pas Corde : le rappel doit
   quand même arriver.
7. **Mes serveurs** : ajoute ton PC par **« Scanner le QR code »** (Android demande l'appareil photo à ce
   moment-là seulement), puis teste. Ouvre **Direct** pendant une session : tourne le téléphone, le tableau
   passe en paysage.
8. **Serveur Corde** (si tu l'as) : ajoute-le, « Appairer ce téléphone » avec un code, puis Sessions.
9. **Clavier** : dans Mes serveurs, en tapant une adresse, le champ reste visible au-dessus du clavier et les
   onglets du bas se cachent.
10. **Bouton retour** : Accueil → Courses → retour revient à l'Accueil ; encore retour ferme Corde.
11. **Économie de batterie** (si un rappel arrive en retard ou pas du tout) : certains fabricants (Xiaomi,
    Huawei, Oppo, OnePlus, Samsung…) arrêtent les applications en arrière-plan plus durement qu'Android.
    Paramètres → Applications → Corde → Batterie → **Sans restriction** (le nom varie).

Dis ce qui ne va pas avec une capture d'écran : c'est la seule façon pour l'équipe de voir le téléphone.

## 5. Limites connues

- **Jamais vue sur un vrai téléphone** avant le premier essai du pilote (§4).
- **Pas de widgets ni de compte à rebours** dans ce lot.
- **Pas de mise à jour automatique** : un nouvel APK s'installe à la main (§2). Le prochain lot décidera entre
  le Play Store et une vérification des nouvelles versions sur GitHub.
- **Rappels** : au plus 64 programmés à la fois (les plus proches ; les suivants le sont à la prochaine
  ouverture de Corde, comme sur iPhone). Sans « Alarmes et rappels », Android peut retarder un rappel de
  quelques minutes quand le téléphone dort. Un rappel dont l'heure est passée pendant que le téléphone était
  **éteint** sonne une quinzaine de secondes après le redémarrage (comportement du module de notifications).
- **Le planning n'est lu que quand l'application est ouverte** ; les rappels, eux, sonnent application
  fermée.
- **Le direct est en HTTP non chiffré** sur le réseau de la maison, comme dans le navigateur et sur l'iPhone ;
  dehors, passe par ton VPN. Android ne sait pas limiter le trafic en clair aux adresses privées : c'est
  l'application qui refuse `http://` vers une adresse publique ([SECURITY-MODEL.md §9](SECURITY-MODEL.md#9-lapplication-android)).
- **Pas de sauvegarde** : ni la sauvegarde Google ni le transfert vers un nouveau téléphone n'emportent les
  données de Corde (c'est voulu : la clé d'appairage ne doit pas quitter le téléphone). Sur un nouveau
  téléphone : réinstaller, rajouter les serveurs, appairer avec un nouveau code.
- **Android 8 au minimum** (le lecteur de QR code l'exige ; Capacitor seul demanderait Android 7). Pensée
  pour les téléphones : sur une tablette elle s'ouvre, en plus grand, sans mise en page dédiée.
- **Le Corde du PC et Corde serveur** : 0.8 ou plus récent, comme pour l'iPhone (le PC accepte l'origine
  de l'application Android depuis plus longtemps, voir §9 du modèle de sécurité, mais l'appairage d'une
  application et le reste de l'échange datent de la 0.8).

## 6. Pour le propriétaire du dépôt

### Ce que fait la CI (`.github/workflows/android.yml`)

Le workflow **Android** tourne à chaque poussée sur `main` qui touche `apps/mobile/`, `packages/ui/` ou le
workflow lui-même, et à la demande (GitHub → Actions → **Android** → **Run workflow**, sur n'importe quelle
branche).

| Job | Quand | Ce qu'il fait |
|---|---|---|
| **Compilation** | toujours | interface en mode mobile, `cap sync android`, APK Debug (et Release sans signature, pour compiler son code), essais unitaires Gradle, lint Android, essais des scripts ; contrôle la version et le numéro de build, la liste exacte des autorisations, et qu'**aucune bibliothèque Google Play services, Firebase ou ML Kit** n'est dans l'APK. APK Debug joint (artefact `android-debug`) |
| **Émulateur** | toujours, après la compilation | Android 15 **sans services Google** (image AOSP), en français, taille Pixel 6 : installe l'APK Debug, le lance, ouvre `corde://races`, essaie le bouton retour, un appel `http://` vers une adresse privée (faux serveur Corde), le lecteur de QR code, puis un **rappel de test** programmé comme un vrai : l'application est **tuée** et le téléphone mis **en veille** (Doze forcé), la notification doit arriver quand même. Captures **dans le journal** (ci-dessous) |
| **Signature (essai)** | sur une autre branche que `main` | le chemin de signature du job suivant, avec une **clé jetable** créée pour l'occasion (aucun secret lu) : APK et bundle Release signés, puis contrôlés. Rien n'est publié |
| **APK signé et publication** | sur `main` du dépôt `dultra123/corde` seulement, jamais une PR ni un fork, et seulement si les deux secrets existent | APK Release et bundle AAB signés avec **ta** clé, signature contrôlée (seule l'empreinte SHA-256 du certificat est affichée), puis **`Corde_<version>_android.apk`** publié dans la pré-release **continuous** (remplacé à chaque build) et ajouté à la release **v&lt;version&gt;** si elle existe et n'a pas encore d'APK (jamais remplacé ensuite). Le bundle `.aab` (pour un futur Play Store) reste dans l'artefact `android-release`. Sans les secrets : « secrets absents, pas d'APK signé », et le job réussit |

La version de l'APK est celle de `apps/mobile/package.json` (la même que l'iPhone) ; son numéro de build
(`versionCode`) est le numéro d'exécution du workflow, qui augmente toujours (Android refuse d'installer
par-dessus un numéro plus petit).

**La pré-release `continuous`** est recréée par le workflow **CI** à chaque poussée sur `main` : il reprend
l'APK Android qui s'y trouve avant de la supprimer et le republie avec le reste. Le job de publication
Android attend, lui, que **CI** ait fini pour le même commit avant de publier : l'APK n'est jamais perdu, et
la release numérotée que CI crée existe déjà quand Android veut la compléter. (Une version numérotée publiée
par une poussée qui ne touche pas l'application reçoit son APK au prochain build Android : lancer **Android →
Run workflow** sur `main` le fait tout de suite.)

**Voir une capture du journal** : job **Émulateur**, étape « Essai de fumée » ; copier les lignes entre
`-----BEGIN CORDE SCREENSHOT <nom>-----` et `-----END CORDE SCREENSHOT-----` dans `capture.txt`, puis
`base64 -d capture.txt > capture.jpg` (Linux, macOS) ou, sous Windows (PowerShell) :
`[IO.File]::WriteAllBytes("capture.jpg", [Convert]::FromBase64String((Get-Content capture.txt -Raw)))`.
Captures : `home` (Accueil), `races` (Courses, par le lien), `scan` (lecteur de QR code), `notification`
(volet des notifications avec le rappel de test). Les mêmes fichiers sont dans l'artefact `android-smoke`.

### Les deux secrets (une fois)

La clé de signature prouve que chaque nouvel APK vient de toi : Android n'installe une mise à jour que si
elle est signée par **la même clé**. **Garde-la précieusement** (gestionnaire de mots de passe, clé USB) :
perdue, plus aucune mise à jour ne s'installera par-dessus l'application des pilotes (il faudrait
désinstaller) ; volée, quelqu'un pourrait publier une fausse mise à jour.

1. **Créer la clé** (sur ton PC, une seule fois). Il faut l'outil `keytool`, livré avec Java : installe
   « Eclipse Temurin JDK 21 » (<https://adoptium.net>) si tu ne l'as pas. Dans PowerShell :

   ```powershell
   keytool -genkeypair -storetype PKCS12 -keystore corde-android.p12 -alias corde `
     -keyalg RSA -keysize 4096 -validity 10000 -dname "CN=Corde"
   ```

   Il demande un mot de passe (deux fois) : choisis-en un long, et note-le. Le nom `CN=Corde` est volontaire :
   le certificat est lisible par n'importe qui dans l'APK, **n'y mets pas ton vrai nom**. Le nom de la clé
   (`-alias corde`) doit rester `corde`.
2. **L'encoder en base64** (PowerShell, dans le même dossier) :

   ```powershell
   [Convert]::ToBase64String([IO.File]::ReadAllBytes("corde-android.p12")) | Set-Content corde-android.b64
   ```

3. **Les donner à GitHub** : dépôt → **Settings** → **Secrets and variables** → **Actions** → **New
   repository secret**, deux fois :
   - **`ANDROID_KEYSTORE_BASE64`** : tout le contenu de `corde-android.b64` (une longue ligne) ;
   - **`ANDROID_KEYSTORE_PASSWORD`** : le mot de passe de l'étape 1 (le même sert pour le magasin et la clé).
4. Efface `corde-android.b64` ; range `corde-android.p12` et le mot de passe en lieu sûr.
5. Lance **Actions → Android → Run workflow** sur `main` : le job « APK signé et publication » affiche
   « Magasin de clés : PKCS12, … octets, clé « corde », certificat SHA-256 … » puis publie l'APK.

Un secret mal collé donne un message clair dans ce job (« ne s'ouvre pas avec ANDROID_KEYSTORE_PASSWORD »,
« n'est pas du base64 »…), jamais la valeur. Le magasin n'est écrit que dans un dossier temporaire de la
machine de GitHub (lisible par ce seul compte) et effacé à la fin du job, même en cas d'échec.

## 7. Pour les développeurs

```
apps/mobile/
├─ package.json            # @corde/mobile : Capacitor 8 (core, ios, android, cli) et les modules officiels
├─ capacitor.config.json   # io.github.dultra123.corde, Corde ; server.androidScheme « http » (§9 du modèle)
├─ scripts/
│  ├─ make-assets.mjs      # icônes, écran de lancement, son ; --android : seulement Android (sans Chromium)
│  ├─ android-assets.mjs   # les dessins vectoriels Android, depuis le signe du logo (+ .test.mjs)
│  ├─ android-signing.sh   # setup | throwaway | check | teardown (CI ; + .test.sh)
│  └─ android-smoke.sh     # essai de fumée sur l'émulateur (CI)
└─ android/                # projet généré par `cap add android`, complété
   ├─ variables.gradle     # SDK minimum 26 (lecteur de QR code), cible et compilation 36
   ├─ build.gradle         # sans le plugin Google Services
   ├─ capacitor.settings.gradle, app/capacitor.build.gradle   # écrits par `cap sync android`
   └─ app/
      ├─ build.gradle      # version de package.json, -PcordeVersionCode, signature par l'environnement,
      │                    # ML Kit / Play services / Firebase exclus
      └─ src/
         ├─ main/AndroidManifest.xml   # sauvegarde coupée, réseau, corde://, écran de lancement
         ├─ main/java/…/MainActivity.java   # écran de lancement, bord à bord
         ├─ main/res/      # icône adaptative et monochrome, petite icône, son (raw), thèmes, xml/
         ├─ debug/java/…/DebugHooks.java, DebugRequest.java   # entrées de l'essai de fumée (Debug seulement)
         ├─ release/java/…/DebugHooks.java                    # vide
         └─ testDebug/java/…/DebugRequestTest.java            # essais unitaires Gradle
```

```bash
pnpm --filter @corde/mobile build              # interface en mode mobile → apps/mobile/www
cd apps/mobile && pnpm exec cap sync android     # copie www dans le projet, réécrit capacitor.settings.gradle
cd android && ./gradlew :app:assembleDebug :app:testDebugUnitTest :app:lintDebug   # JDK 21 + SDK Android
node apps/mobile/scripts/make-assets.mjs --android   # régénère les dessins et le son d'Android
node --test apps/mobile/scripts/android-assets.test.mjs && bash apps/mobile/scripts/android-signing.test.sh
```

- **Une seule base de code** : le code de l'application est dans `packages/ui/src/mobile/` (chargé seulement
  par le build `mobile`). Ce qui diffère entre iPhone et Android est décidé par `mobile/device.ts`
  (`traitsOf(Capacitor.getPlatform())`) : sur Android, `WidgetSync` (widgets, compte à rebours, module natif
  `CordeNative`) n'est pas créé — aucun appel, aucun réglage, aucun bouton — et `mobile/android.ts` ajoute
  le canal des rappels, les alarmes exactes, le bouton retour et les entrées de l'essai de fumée. Les phrases
  qui nomment le système (« Réglages de l'iPhone ») ont une version `…android` (`phoneTextKey`). Essais :
  `mobile/android.test.tsx`.
- **Rappels** : `reminders.ts`, `notificationOf(…, { exact })` ajoute sur Android le canal `corde-races`, la
  petite icône `ic_stat_corde`, le son `corde_reminder.wav` (ressource `raw` : pas de `-` dans un nom de
  ressource), `allowWhileIdle`, et `isExactNotification` **seulement si** l'autorisation est donnée (sinon
  le module ouvrirait de lui-même l'écran « Alarmes et rappels » à chaque planification). Un changement de
  l'autorisation reprogramme les rappels (`extra.exact`).
- **Origine** : `server.androidScheme` = `http`, donc les pages ont l'origine `http://localhost` : pas de
  « contenu mixte » vers `http://192.168.…`, et le serveur l'accepte déjà (§9 du modèle de sécurité).
- **Entrées de l'essai de fumée** (build Debug seulement, `DebugHooks.java`) :
  `adb shell am start -n io.github.dultra123.corde/.MainActivity --es cordeDebug reminder --ei cordeDelay 60`
  (ou `fetch --es cordeUrl http://10.0.2.2:8471/api/health`, ou `scan`) : la page reçoit un évènement
  `cordeDebug`, agit et l'écrit dans le journal (`adb logcat -s Capacitor/Console`). Aucune ne touche aux
  données du pilote ; le build Release n'en a aucune.
- **Liens** : `adb shell am start -a android.intent.action.VIEW -d corde://races io.github.dultra123.corde`
  (pas de fenêtre de confirmation sur Android).
