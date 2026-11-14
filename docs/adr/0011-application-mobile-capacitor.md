# ADR 0011 — Application iPhone : l'interface existante dans Capacitor, courses lues par le téléphone

- **Statut** : **décidée** — 2026-11-12 (le propriétaire a souscrit au programme développeur d'Apple et veut
  Corde sur son iPhone ; il n'a pas de Mac). Livrée en 0.8.0. Complétée par l'ADR 0012 (widgets, activité
  en direct ; iOS 17 minimum depuis 0.9.0, signature manuelle par certificat créé à chaque envoi, ou
  réutilisé avec le secret facultatif `APPLE_DIST_KEY_BASE64`, ADR 0012 P) et par
  l'ADR 0013 (la même application pour Android, 1.0.0).
- **Source** : `docs/IOS.md`, `docs/SECURITY-MODEL.md` §8, `PRIVACY.md`, ADR 0001 (pile), ADR 0006 (local
  d'abord), ADR 0010 (serveur domestique).

## Contexte

L'interface de Corde (React, `packages/ui`) tourne déjà dans le navigateur du téléphone quand le PC (ou le
serveur Corde) est joignable. Le pilote veut une vraie application : icône, notifications qui sonnent
application fermée, et surtout **la page Courses sans PC** — elle remplace son script iPhone « LMU Courses »,
qui lisait lui-même racecontrol.gg et lmu.tools. ARCHITECTURE.md prévoyait « Tauri iOS/Android ». Contraintes :
personne n'a de Mac ni d'iPhone de test ; la seule machine macOS est le runner de GitHub (gratuit, dépôt
public) ; chaque erreur de compilation coûte un aller-retour.

## Options

| Option | Pour | Contre |
|---|---|---|
| **Tauri 2 mobile** | même outil que le bureau | iOS encore jeune côté Tauri, projet Xcode généré à la compilation (Rust pour iOS sur le runner, temps de build longs), modules de notifications locales et de stockage moins éprouvés ; rien de Rust n'est utile sur le téléphone |
| **Application native (Swift)** | intégration parfaite | réécrire toute l'interface, impossible à vérifier sans Mac |
| **Capacitor** (Ionic) | réutilise l'interface telle quelle (WKWebView) ; modules **officiels** pour les notifications locales, le stockage, le HTTP natif, l'orientation, le clavier, le QR code ; projet Xcode **versionné** et généré par un outil qui tourne sous Linux (`cap add ios`, `cap sync ios`) ; Swift Package Manager (pas de CocoaPods, donc pas de Ruby sur le runner) | une webview (pas une interface native) ; dépendance à l'écosystème Ionic |

## Décision

1. **Capacitor 8** (dernière version stable sur npm au 12 nov. 2026 : 8.6.0), projet iOS en **Swift Package
   Manager**, dans `apps/mobile/`. Identifiant `io.github.dultra123.corde`, nom affiché « Corde », iOS 16
   minimum (l'interface utilise des fonctions de Safari 15.4+), iPhone seulement.
2. **La même interface**, construite en mode `mobile` (`vite build --mode mobile`). Le code propre à
   l'application (`packages/ui/src/mobile/`) n'est chargé que dans ce mode ; les pages partagées n'en
   connaissent que des emplacements (`mobile/slots.ts`) — le navigateur ne change pas.
3. **Une source de courses abstraite** (`races/backend.ts`) : `server` (le serveur Corde, comme avant) ou
   `phone` (l'application). Dans l'application, **Courses est toujours celle du téléphone** : planning lu
   par le **HTTP natif de Capacitor** (pas de CORS) avec les mêmes règles que le serveur, choix et réglages
   dans le stockage natif, rappels en **notifications locales**. Les choix du téléphone et du PC restent
   séparés (comme ceux du PC et du serveur domestique, ADR 0010).
4. **Les autres pages lisent un serveur choisi dans « Mes serveurs »** (le PC pour le direct, le serveur
   domestique pour le reste), par `fetch` et WebSocket depuis la webview. Le serveur accepte l'origine
   `capacitor://localhost` (Origin et CORS, `Authorization` compris), jamais comme une page du PC. Sur le
   serveur domestique, l'application s'appaire comme un appareil `app` : jeton en `Authorization: Bearer`,
   et sur le WebSocket en entrée `corde.token.<jeton>` du sous-protocole (une page ne peut pas mettre
   d'en-tête sur un WebSocket, et un jeton dans l'adresse finirait dans le journal des requêtes).
5. **Compilation et distribution uniquement par la CI** (`.github/workflows/ios.yml`) : simulateur sans
   aucun secret à chaque modification (avec captures d'écran dans le journal), TestFlight quand les secrets
   App Store Connect existent (signature automatique dans le cloud d'Apple avec une clé d'API : aucun
   certificat ni profil à gérer).

## Conséquences

- L'application dépend de modules npm `@capacitor/*` officiels uniquement (le module QR code tire
  `html5-qrcode` pour sa version web : jamais chargé, l'application appelle le module natif directement).
- Le Corde du PC doit être en 0.8 ou plus pour accepter l'application.
- Ce qui n'a pas pu être vérifié sans Mac (compilation Xcode, comportement de WKWebView, ATS, réseau local,
  signature dans le cloud) l'est à la première exécution de la CI ; la liste est dans `docs/STATUS.md`.
- ARCHITECTURE.md : la phase 3 « application mobile » se fait avec Capacitor, pas Tauri. Android n'est pas
  prévu dans ce lot (Capacitor le permettrait avec le même code).
