# ADR 0012 — Widgets iOS et activité en direct : extension WidgetKit, trousseau partagé, aucune notification push

- **Statut** : **décidée** — 2026-11-14 (le pilote utilise Corde 0.8.0 sur son iPhone par TestFlight
  depuis le matin ; il veut retrouver les widgets de son script « LMU Courses » et un compte à rebours dans
  la Dynamic Island). Livrée en 0.9.0 (application iPhone seulement).
- **Source** : `docs/IOS.md` (« Widgets et Dynamic Island »), `docs/SECURITY-MODEL.md` §8.2 bis et §8.4,
  `PRIVACY.md`, ADR 0011 (application iPhone).

## Contexte

L'application est une webview Capacitor (ADR 0011). Un widget iOS, lui, est forcément une **extension
native** (WidgetKit, SwiftUI), un autre programme que l'application, qui ne peut ni exécuter le JavaScript
de Corde ni garder une connexion ouverte : iOS lui demande une « chronologie » (des vues datées) et la
redessine quand il le décide. Une activité en direct (écran verrouillé, Dynamic Island) est dessinée par la
même extension, démarrée par l'application. Contraintes inchangées : personne n'a de Mac (tout se compile
sur GitHub Actions), pas de service tiers, pas de télémétrie, et le dépôt est public.

## Décisions

**A. iOS 17 minimum** pour l'application et l'extension (au lieu de 16). WidgetKit gagne avec iOS 17
`containerBackground` (obligatoire pour qu'un widget s'affiche correctement sur 17) et ActivityKit est
complet depuis 16.2 (`ActivityContent`, `staleDate`, `isStale`) : avec 17, aucune branche de
disponibilité dans le code Swift, qu'on ne peut pas essayer sur un vieil iOS. Le pilote est sur un iPhone
récent (avec Dynamic Island).

**B. Une cible d'extension `CordeWidgets`** (`com.apple.product-type.app-extension`, point d'extension
`com.apple.widgetkit-extension`, identifiant `io.github.dultra123.corde.widgets`), embarquée par la phase
« Embed Foundation Extensions » de l'application. Mêmes `MARKETING_VERSION` et `CURRENT_PROJECT_VERSION`
(App Store Connect refuse une extension dont la version diffère) : le workflow les passe à `xcodebuild`
pour toutes les cibles. Le projet (`objectVersion` 60) est édité à la main — l'outil Ruby `xcodeproj`
n'était pas installable ici — et un essai Node (`apps/mobile/scripts/xcodeproj.test.mjs`) lit le fichier et
vérifie chaque référence, les deux cibles, l'embarquement et les réglages. `cap sync ios` ne réécrit que
`CapApp-SPM/Package.swift` (qui suit la cible de déploiement : `.iOS(.v17)`).

**C. Les données passent par un élément de trousseau partagé, pas par un App Group.** Un App Group est le
moyen habituel (des `UserDefaults` communs), mais il se crée à la main sur le portail d'Apple, puis
s'ajoute aux deux identifiants : l'API App Store Connect ne sait pas le faire, il aurait fallu faire remplir
trois formulaires au propriétaire. Le partage de trousseau ne demande rien côté portail : tous les profils
de provisionnement contiennent déjà `keychain-access-groups = <équipe>.*`. Les deux cibles déclarent
`$(AppIdentifierPrefix)io.github.dultra123.corde.shared` ; le préfixe d'équipe n'étant pas connu du code,
Xcode écrit le nom complet dans la clé `CordeKeychainGroup` de leur `Info.plist`, lue à l'exécution.
`SharedStore.swift` (compilé dans les deux cibles) lit et écrit un bloc de données ; passer plus tard à un
App Group ne toucherait que ce fichier. Il rend l'`OSStatus` de chaque opération, affiché dans le
diagnostic de la page Courses : c'est par là que le pilote peut rapporter un échec.

**D. Le contenu est un bloc JSON versionné produit par le TypeScript** (`packages/ui/src/mobile/widgets.ts`,
testé avec Vitest), décodé en Swift avec tolérance (`WidgetData.swift`) : un champ inconnu est ignoré, une
autre version donne « Ouvre Corde pour actualiser ». Il ne contient **aucun secret** : planning public,
choix du pilote, langue. Les quotidiennes sont prolongées sur 48 h (même intervalle si les départs connus
sont réguliers, sinon même heure le lendemain) pour que les widgets restent justes sans ouvrir Corde ; ces
horaires sont marqués estimés. La logique de liste (« la prochaine course de chacune, par départ ») existe
deux fois, en TypeScript (`racesAt`, testée) et en Swift (`races(at:)`), commentées comme miroirs.

**E. Widgets statiques** (sans intents de configuration) : « Courses » (petit, moyen, grand), « Mes
courses » (petit, moyen), « Prochain départ » (écran verrouillé : rectangulaire, en ligne, rond). Une
entrée de chronologie maintenant, une à chaque départ à venir et une heure avant chaque départ choisi ;
les comptes à rebours (`Text(timerInterval:)`, `.relative`) tournent sans rechargement. L'application
demande à iOS de les recharger (`WidgetCenter.reloadAllTimelines`) à chaque changement du bloc.

**F. Une seule activité en direct, sans mise à jour à distance.** Pas d'APNs : il faudrait un serveur qui
envoie des notifications push, donc un service et un jeton par appareil — contraire au principe « local
d'abord » (ADR 0006) et inutile ici, car un départ ne bouge pas : `Text(timerInterval:)` décompte seul,
`staleDate` = l'heure du départ, et la vue affiche « Départ donné » quand iOS la marque périmée. Les règles
sont en TypeScript (`planActivity`, testées) : démarrage automatique au premier plan si le départ choisi le
plus proche est à moins de 2 h (à la main jusqu'à 8 h), fin quand le choix est retiré, le réglage coupé, ou
au retour dans l'application après départ + 2 min ; jamais deux à la fois ; réconciliation avec
`Activity.activities` à chaque passage au premier plan. Limite assumée : après le départ, l'activité reste
à l'écran jusqu'à la prochaine ouverture de Corde (ou un balayage) — iOS la retire de lui-même au bout de
quelques heures.

**G. Le pont natif est un module Capacitor local** (`CordeNativePlugin`, dans la cible de l'application,
enregistré par `CordeBridgeViewController.capacitorDidLoad()`), pas un paquet npm ni un module tiers :
quelques méthodes (écrire le bloc, démarrer / terminer / lister l'activité, diagnostic). Côté TypeScript,
un port (`NativePort`) et un double pour les essais.

**H. Le lien `corde://races`** (toucher d'un widget ou de l'activité) fonctionnait déjà : les captures
identiques de la CI venaient de `xcrun simctl openurl`, auquel iOS répond par la fenêtre « Ouvrir dans
“Corde” ? » qu'aucun outil ne peut toucher (la capture le montre). L'essai de fumée fait désormais ouvrir
le lien par l'application elle-même (argument de lancement, build Debug), ce qui passe par le même chemin
(`scene(_:openURLContexts:)` → Capacitor → `appUrlOpen`) qu'un toucher de widget. Le lancement à froid passe
par `SceneDelegateProxy` (Capacitor garde l'adresse jusqu'au chargement des modules) : non vérifiable sur
le simulateur sans cette fenêtre.

**I. Voir sans Mac** : en build Debug, l'application dessine chaque vue des widgets et de l'activité avec
`ImageRenderer` (données inventées, fr / en, clair / sombre) ; l'essai de fumée imprime ces planches en
base64 dans le journal, et photographie une vraie activité d'exemple dans la Dynamic Island du simulateur.
Les vues SwiftUI sont donc dans des fichiers compilés par les deux cibles (`Shared/`), séparées des
`Widget` / `ActivityConfiguration` de l'extension.

**J. Signature** : un certificat « Apple Distribution » par exécution (inchangé) et **un profil App Store
par identifiant** ; l'identifiant de l'extension est créé par l'API s'il manque (jamais celui de
l'application) ; les deux cibles passent en signature manuelle sur le runner seulement ; le contrôle avant
envoi vérifie aussi l'extension et le droit de trousseau dans les deux binaires.

## Conséquences

- Le propriétaire n'a **rien à faire côté Apple** : l'identifiant « Corde Widgets » apparaît tout seul au
  premier envoi, avec un deuxième profil « Corde CI … widgets ».
- Les widgets ne voient que ce que l'application leur a écrit : sans ouverture de Corde pendant plus de
  quelques jours (ou après la fin des horaires écrits), ils disent « Ouvre Corde pour actualiser ».
- Ce qui ne peut être vérifié que sur un vrai iPhone : le trousseau partagé lu depuis l'extension signée,
  l'affichage réel des widgets et de l'activité, le lien à froid, la fin automatique de l'activité ;
  ordre des essais dans `docs/IOS.md`.
