# Corde sur iPhone

> Version 0.8.0 — première version de l'application iPhone. Décision : [ADR 0011](adr/0011-application-mobile-capacitor.md).
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

## 2. Comment sortent les versions iPhone

Personne dans le projet n'a de Mac : **l'application n'est compilée que par GitHub**, sur un Mac de GitHub
(gratuit pour un dépôt public), par le workflow **iOS** (`.github/workflows/ios.yml`). Il tourne à chaque
poussée sur `main` qui touche `apps/mobile/`, `packages/ui/` ou le workflow lui-même, et à la demande
(GitHub → Actions → **iOS** → **Run workflow**).

1. **Simulateur** (toujours, sans aucun secret) : compile l'application pour le simulateur d'iPhone, la lance,
   prend deux captures d'écran (Accueil, puis Courses) et les écrit **dans le journal** entre les lignes
   `-----BEGIN CORDE SCREENSHOT <nom>-----` et `-----END CORDE SCREENSHOT-----` (image JPEG en base64), avec
   les dernières lignes du journal de l'application. Les mêmes fichiers sont joints à l'exécution
   (artefact `ios-smoke`, téléchargeable par le propriétaire du dépôt).
2. **TestFlight** (seulement sur `dultra123/corde`, jamais sur un fork, et seulement si les **quatre secrets**
   du §3 existent) : archive, signature automatique par Apple avec la clé d'API, envoi vers TestFlight. Le
   numéro de build est le numéro d'exécution du workflow (il augmente toujours, comme Apple l'exige) ; la
   version affichée est celle de `apps/mobile/package.json` (0.8.0).

Sans les secrets, le job TestFlight affiche une note et ne fait rien : le simulateur suffit à vérifier que
tout compile.

**Voir une capture du journal** : copier les lignes entre les deux marqueurs dans un fichier `capture.txt`,
puis `base64 -d capture.txt > capture.jpg` (Linux, macOS) ou, sous Windows (PowerShell) :
`[IO.File]::WriteAllBytes("capture.jpg", [Convert]::FromBase64String((Get-Content capture.txt -Raw)))`.

## 3. Ce que le propriétaire fait côté Apple (une fois, dans l'ordre)

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
   tard en rouvrant l'identifiant (**Identifiers** → `io.github.dultra123.corde` → cocher → **Save**) ; ce
   sera le cas d'**App Groups** le jour où Corde aura des widgets d'écran d'accueil. Le **Bundle ID**, lui,
   ne se modifie plus après l'enregistrement.
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
   accepter les conditions) → nom `GitHub Actions`, accès **Gestionnaire d'apps** → **Générer**.

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

Si l'archive ou l'export échoue avec une erreur de **permission de signature dans le cloud**
(« Cloud signing permission error », « cloud-managed distribution certificate »), recréer la clé d'API avec
l'accès **Admin** au lieu de « Gestionnaire d'apps » et remplacer les deux secrets `APPSTORE_API_KEY` et
`APPSTORE_API_KEY_ID` : selon les comptes, Apple réserve la création du certificat de distribution géré
dans le cloud à ce rôle (non vérifié ici). Si l'envoi échoue parce que l'application « n'existe pas »,
l'étape 4 manque.

## 4. Durée de vie d'un build

- Un build TestFlight **expire 90 jours** après son envoi : l'application ne s'ouvre plus. Un nouveau build
  part à chaque poussée sur `main` qui touche l'application, ou à la main (**Run workflow**) ; TestFlight
  propose alors la mise à jour.
- Une mise à jour **garde les données** de l'application (serveurs, appairage, courses choisies, réglages).
  Supprimer l'application les efface.
- Les testeurs internes (jusqu'à 100 membres de l'équipe App Store Connect) n'attendent aucune
  vérification d'Apple. Des testeurs externes demanderaient une vérification (« Beta App Review ») et une
  fiche plus complète.

## 5. Limites connues de cette première version

- **iPhone seulement, iOS 16 au minimum** (un iPad l'ouvre en mode iPhone). Pas d'Android, pas de widget
  d'écran d'accueil (lot suivant).
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

## 6. Pour les développeurs

```
apps/mobile/
├─ package.json            # @corde/mobile : Capacitor 8 (core, ios, cli) et les modules officiels
├─ capacitor.config.json   # identifiant io.github.dultra123.corde, nom Corde, fond sombre, notifications
├─ assets/corde-reminder.wav, scripts/make-assets.mjs, scripts/finish-web.mjs
├─ www/                    # l'interface construite en mode mobile (ignoré par git)
└─ ios/App/                # projet Xcode généré par `cap add ios` (gabarit Swift Package Manager)
   ├─ App.xcodeproj        #   iOS 16, iPhone, version 0.8.0, schéma partagé « App »
   ├─ App/Info.plist       #   réseau local, ATS, appareil photo, corde://, chiffrement
   ├─ App/{fr,en}.lproj/InfoPlist.strings
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
  `corde://profile` : ouvrent une page, rien d'autre (la CI s'en sert pour la capture de Courses).
