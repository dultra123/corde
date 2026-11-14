# ADR 0013 — Application Android : la même application Capacitor, sans Google, compilée par la CI

- **Statut** : **décidée** — 2026-11-14 (le propriétaire : « Mets-toi à faire l'app Android »). Premier lot :
  l'application à parité avec l'iPhone, **hors widgets et compte à rebours** (lot suivant). Version 1.0.0.
- **Source** : `docs/ANDROID.md`, `docs/SECURITY-MODEL.md` §9, `PRIVACY.md` (« Application Android »),
  ADR 0011 (application iPhone : Capacitor), ADR 0012 (widgets iOS).

## Contexte

L'application iPhone (ADR 0011) est l'interface de Corde dans Capacitor 8 ; elle fonctionne sur l'iPhone du
pilote. Capacitor sait produire une application Android avec le **même code** : l'ADR 0011 le notait
(« Android n'est pas prévu dans ce lot »). Contraintes du projet : personne ne code ni n'a de téléphone
Android de test ; tout se compile et s'essaie sur GitHub Actions ; le dépôt est public ; pas de service
tiers, pas de télémétrie. Les décisions A à K ont été fixées par le propriétaire avant le lot ; ce document
dit comment elles sont appliquées, et où il a fallu trancher.

## Décisions

**A. Le même projet Capacitor.** Plateforme Android ajoutée à `apps/mobile` (`@capacitor/android` 8.6.0,
comme `@capacitor/core`), dossier `apps/mobile/android/` versionné comme `ios/` (généré par `cap add
android`, puis complété). Même identifiant `io.github.dultra123.corde`, nom « Corde ». Version : celle de
`apps/mobile/package.json`, lue par Gradle ; `versionCode` : numéro d'exécution du workflow Android
(`-PcordeVersionCode`). SDK cible et de compilation : **36** (Android 16, le plus récent que Capacitor 8.6
connaît). **SDK minimum : 26** (Android 8.0), et non 24 comme Capacitor : le module du lecteur de QR code
(`@capacitor/barcode-scanner` 3.1) l'exige (son README et son `build.gradle`). Téléphones seulement ; sur
une tablette l'interface s'affiche en grand, sans mise en page dédiée. Scripts : `sync` fait les deux
plateformes, `sync:ios` / `sync:android` une seule ; `ios.yml` appelle toujours `cap sync ios` lui-même.

**B. Aucune dépendance Google.** Pas de Firebase, pas de Google Play services, ni analyse d'usage ni
publicité ; le plugin Gradle « Google Services » du gabarit est retiré. Le lecteur de QR code dépend de deux
bibliothèques : ZXing (libre, sans service) et **ML Kit** de Google (qui tire Play services, Firebase et
envoie des rapports d'usage à Google). Corde demande ZXing (`android.scanningLibrary: "zxing"`, seulement
sur Android) et **exclut** ML Kit, Play services, Firebase et `datatransport` de l'application
(`configurations.configureEach { exclude … }` dans `app/build.gradle`). Le code du lecteur **cite** encore
ML Kit sans l'embarquer : il n'y passe que si on lui demande ML Kit. La CI vérifie les dépendances et les
classes définies dans l'APK (`apkanalyzer`), et ouvre le lecteur sur un émulateur **sans** services Google
(image AOSP). Le bloc de dépendances qu'AGP chiffre pour la Play Console est aussi désactivé
(`dependenciesInfo`).

**C. Le code partagé ne change pas sur iPhone.** Ce qui diffère est décidé à un seul endroit,
`packages/ui/src/mobile/device.ts` (`traitsOf(Capacitor.getPlatform())`), derrière les ports existants :
- sur Android, `WidgetSync` (widgets, photos, compte à rebours, diagnostic — tout ce qui passe par le module
  natif `CordeNative`) **n'est pas créé** : aucun appel au module, et les composants qui en dépendent
  (réglage « Compte à rebours… », bouton « Compte à rebours », diagnostic) ne s'affichent pas, comme ils le
  faisaient déjà sans lui ;
- `mobile/android.ts` (Android seulement) : canal des rappels, alarmes exactes, bouton retour, entrées de
  l'essai de fumée ;
- les phrases qui nomment le système (« Réglages de l'iPhone → … ») ont une version Android ;
- le style du clavier et son mode de redimensionnement (réglages iOS) ne sont plus demandés sur Android.
Sur iPhone, les notifications programmées sont identiques champ pour champ (essai), le journal ajoute
seulement « (ios) » à « application started ». Essais Vitest : `mobile/android.test.tsx`.

**D. Notifications de rappel.**
- **Canal** `corde-races` « Rappels des courses » (importance haute, vibration, visible sur l'écran
  verrouillé), créé au démarrage avec le son de Corde, recréé quand la langue change (Android garde le son
  et les choix du pilote, met à jour le nom). Le son est la ressource **`res/raw/corde_reminder.wav`** — le
  nom `corde-reminder.wav` demandé est impossible : un nom de ressource Android ne contient pas de `-`.
- **Autorisation** `POST_NOTIFICATIONS` demandée au premier choix de course, comme sur iPhone.
- **Alarmes exactes : `SCHEDULE_EXACT_ALARM`, avec repli inexact**, et non `USE_EXACT_ALARM`. Raison :
  `USE_EXACT_ALARM` est accordée d'office mais le Play Store la réserve aux réveils, minuteurs et agendas
  dont c'est la fonction principale ; Corde est une application de télémétrie dont les rappels sont une
  fonction parmi d'autres, elle serait refusée. `SCHEDULE_EXACT_ALARM` est permise à toute application qui
  en a besoin, mais **refusée par défaut depuis Android 14** : juste après l'autorisation des notifications
  (premier choix), Corde **explique** pourquoi et propose d'ouvrir l'écran « Alarmes et rappels » (une seule
  fois) ; la page Courses dit ensuite si les rappels sont exacts et garde un bouton **Autoriser**. Refusée :
  le rappel est programmé avec `setAndAllowWhileIdle` (il arrive, mais Android peut le retarder de quelques
  minutes en veille). Accordée : `setExactAndAllowWhileIdle`. Corde ne passe **jamais**
  `isExactNotification: true` sans l'autorisation : le module ouvrirait alors lui-même l'écran du système à
  chaque planification. Un changement de l'autorisation reprogramme les rappels (`extra.exact`).
- **Redémarrage** : le module déclare `RECEIVE_BOOT_COMPLETED` et un récepteur
  (`LocalNotificationRestoreReceiver`, `LOCKED_BOOT_COMPLETED` / `BOOT_COMPLETED`) qui reprogramme les
  rappels gardés dans son stockage ; vérifié dans le code du module 8.3.1. Un rappel dont l'heure est passée
  téléphone éteint sonne 15 s après le redémarrage (choix du module).
- **Petite icône** monochrome `ic_stat_corde` (le signe du logo, dessin vectoriel blanc) et couleur ambre.
- Le module crée aussi un canal « Default » qu'il utilise par défaut : Corde ne s'en sert pas, on ne peut pas
  l'empêcher sans modifier le module.

**E. Réseau.**
- **Origine des pages : `http://localhost`** (`server.androidScheme: "http"`, au lieu du `https` par défaut
  de Capacitor). Avec `https://localhost`, chaque appel au PC (`http://192.168.…`) serait du « contenu
  mixte », bloqué par la webview, sauf à l'autoriser partout (`allowMixedContent`). `http://localhost` est
  aussi une origine que **tout serveur Corde accepte déjà** : `net::origin_is_local` la range parmi les
  pages de « ce PC », mais les routes « ce PC seulement » exigent en plus que la connexion vienne de la
  boucle locale (`may_change_settings`), ce qu'un téléphone n'est jamais (essai Rust ajouté). Aucun
  changement du serveur, aucune nouvelle version du PC nécessaire.
- **Trafic en clair** : `network_security_config.xml` l'autorise pour toutes les adresses — Android ne sait
  pas le limiter à des plages d'adresses privées (un `domain-config` prend des noms d'hôtes) — et le
  **refuse** explicitement vers `racecontrol.gg` et `lmu.tools`. La règle « `http://` seulement vers le
  réseau privé » reste celle de l'application (`addresses.ts`), comme sur iPhone. Seules les autorités de
  certification du système sont acceptées.
- **Autorisations** : INTERNET ; POST_NOTIFICATIONS, SCHEDULE_EXACT_ALARM, RECEIVE_BOOT_COMPLETED, WAKE_LOCK
  (module des notifications) ; CAMERA (lecteur de QR code, demandée au toucher de « Scanner le QR code ») ;
  plus la permission « signature » `…DYNAMIC_RECEIVER_NOT_EXPORTED_PERMISSION` qu'androidx.core ajoute à
  toute application pour ses propres récepteurs (aucun écran, aucune portée hors de Corde). Aucune
  localisation (Android 13+ ne la demande pas pour joindre le réseau local). La CI vérifie la liste exacte.

**F. Vie privée du stockage.** `android:allowBackup="false"` et `android:fullBackupContent="false"`
(aucune sauvegarde avant Android 12) et
`data_extraction_rules.xml` qui exclut **tous** les domaines de la sauvegarde dans le nuage et du transfert
d'appareil à appareil (Android 12+, qui ignore `allowBackup` pour ce transfert). Le jeton d'appairage (dans
les préférences de l'application, `SharedPreferences`) ne part ni chez Google ni sur un autre téléphone.
Effet : changer de téléphone, c'est réinstaller et appairer à nouveau. Le débogage de la webview n'est
ouvert que dans le build Debug (réglage par défaut de Capacitor).

**G. Apparence.**
- **Icône adaptative** (fond `#0b0e12`, avant-plan : le signe ambre dans la zone sûre de 66 dp) et
  **monochrome** (icônes à thème), écrites par `make-assets.mjs` (`android-assets.mjs`) en **dessins
  vectoriels** depuis le même signe que l'icône iPhone : aucun PNG, aucun Chromium pour cette partie, et un
  essai vérifie que les fichiers du projet sont ceux du générateur. Le SDK minimum 26 rend les icônes
  bitmap inutiles.
- **Écran de lancement** : API SplashScreen d'Android 12 (`core-splashscreen` avant), fond sombre et signe ;
  le thème de la fenêtre est sombre (rien de blanc avant la première page).
- **Bord à bord** sur toutes les versions (`EdgeToEdge.enable`, barres transparentes, icônes claires) ;
  le module SystemBars de Capacitor passe les marges de la barre d'état et de la barre de gestes à la page
  (`env(safe-area-inset-*)`, ou un retrait de la webview sur une webview plus ancienne) : les mêmes marges
  de sécurité que sur iPhone, sans changer le CSS.
- **Retour** : sans écouteur, le module `App` revient en arrière dans la webview mais ne quitte jamais
  l'application. Corde écoute `backButton` : page précédente de l'interface s'il y en a une, sinon
  `App.exitApp()`. « Il y en a une » est la réponse de la webview (`canGoBack`) : Chromium y ignore les
  entrées d'historique créées sans geste du pilote (un lien `corde://`, une notification) ; une page
  ouverte ainsi se quitte donc par « retour », ce qui est le comportement attendu sur Android (vu sur
  l'émulateur : après un toucher d'onglet, « retour » revient à la page précédente, puis ferme Corde).
- Paysage seulement sur le Direct (même code que l'iPhone, `ScreenOrientation`).

**H. CI : `.github/workflows/android.yml`** (sur le modèle d'`ios.yml`). Jobs `build` (Debug, Release sans
signature, essais unitaires Gradle, lint, contrôles), `emulateur` (Android 15 AOSP sans services Google,
KVM, `reactivecircus/android-emulator-runner` épinglé ; essai de fumée `android-smoke.sh` : démarrage, lien,
retour, appel `http://` d'une adresse privée avec l'origine relevée, lecteur de QR code, **rappel de test
reçu application tuée et Doze forcé** ; captures en base64 dans le journal), `signature-essai` (sur une
branche : clé jetable) et `release` (sur `main` du dépôt d'origine, secrets `ANDROID_KEYSTORE_BASE64` et
`ANDROID_KEYSTORE_PASSWORD`). Les entrées de l'essai de fumée n'existent que dans le build Debug
(`src/debug`, `DebugHooks.java` ; `src/release` en a une vide) et ne touchent pas aux données du pilote.
**Publication** : `Corde_<version>_android.apk` dans la pré-release `continuous` (remplacé) et dans
`v<version>` si elle existe sans APK (jamais remplacé). Le job `continuous` de `ci.yml` **recrée** cette
pré-release à chaque poussée sur `main` : (1) il **reprend** l'APK qui s'y trouve avant de la supprimer et le
republie (avec une ligne dans les notes) ; (2) le job Android **attend** que `ci.yml` ait fini pour le même
commit avant de publier. (1) couvre les poussées qui ne touchent pas l'application (Android ne tourne pas),
(2) les poussées qui touchent les deux, et garantit que `v<version>` créée par `ci.yml` existe quand
Android veut la compléter.

**I. Pas de mise à jour automatique dans ce lot** ; la version est déjà affichée. Le lot suivant choisira
entre le Play Store (le bundle `.aab` signé est déjà produit, gardé en artefact) et une vérification des
nouvelles versions sur GitHub.

## Ce qui est reporté

- **Widgets Android** (écran d'accueil) et **compte à rebours en notification** (l'équivalent de l'activité
  en direct) : lot suivant. Le code partagé est prêt (`traits.widgets` et le bloc de `widgets.ts` ne
  dépendent pas d'iOS) ; il faudra un module natif Android.
- **Play Store** et mises à jour (I).
- **Mise en page tablette.**

## Conséquences

- Une seule interface, un seul code de rappels et de serveurs pour les deux téléphones ; le projet Android
  ajoute environ 30 fichiers versionnés, la plupart générés par Capacitor.
- L'application Android n'a **jamais tourné sur un vrai téléphone** avant le premier essai du pilote : ce
  que l'émulateur a vérifié et ce qui reste à voir sont listés dans `docs/ANDROID.md` (§4, §6) et
  `docs/STATUS.md`.
- La sécurité côté serveur ne change pas (§9 du modèle). Côté téléphone, le trafic en clair est permis
  partout au niveau d'Android : c'est l'application qui le limite au réseau privé.
- La clé de signature est celle du propriétaire, jamais dans le dépôt ; la perdre empêcherait toute mise à
  jour par-dessus l'application installée.
