# ADR 0012 — Widgets iOS et activité en direct : extension WidgetKit, trousseau partagé, aucune notification push

- **Statut** : **décidée** — 2026-11-14 (le pilote utilise Corde 0.8.0 sur son iPhone par TestFlight
  depuis le matin ; il veut retrouver les widgets de son script « LMU Courses » et un compte à rebours dans
  la Dynamic Island). Livrée en 0.9.0 (application iPhone seulement). Complétée le même jour (K à O) par
  le widget « Courses en images », livré en 0.10.0 ; puis (P) par le certificat de distribution stable
  (secret facultatif `APPLE_DIST_KEY_BASE64`), qui remplace en partie J.
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
Dans les droits de l'application, ce groupe partagé vient **en second**, après le groupe propre à
l'application (`$(AppIdentifierPrefix)io.github.dultra123.corde`) : iOS range dans le premier groupe de la
liste tout élément écrit sans groupe nommé, donc rien d'autre que le bloc des widgets ne peut arriver par
défaut dans le groupe que lit l'extension (aujourd'hui l'application ne range rien d'autre dans le
trousseau ; la règle protège la suite). L'extension, elle, ne déclare que le groupe partagé.
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

**J. Signature** : un certificat « Apple Distribution » par exécution (inchangé ; réutilisé d'un build à
l'autre depuis l'ajout P quand le secret facultatif existe) et **un profil App Store
par identifiant** ; l'identifiant de l'extension est créé par l'API s'il manque (jamais celui de
l'application) ; les deux cibles passent en signature manuelle sur le runner seulement ; le contrôle avant
envoi vérifie aussi l'extension et le droit de trousseau dans les deux binaires.

## Ajout — widget « Courses en images » (0.10, 2026-11-14)

Le pilote a les widgets de la 0.9 sur son iPhone (le trousseau partagé est bien lu par l'extension signée)
et demande la version de son ancien widget qui faisait défiler les photos : une course par page, sa photo,
le SR / DR qu'il faut, « page 2/6 », la page qui change à chaque rafraîchissement.

**K. Un widget de plus, `CordeSlides`** (petit, moyen, grand), sans rien retirer. Ses pages sont la liste
du widget « Courses » à l'instant de l'entrée (`races(at:)`, 8 au plus) ; en petit, le prochain départ
choisi d'abord. Pas de seconde liste dans le bloc : deux champs **facultatifs** sur les courses et les
choix, `img` (clé de la photo : FNV-1a 64 bits de son adresse, `i` + 16 hexadécimaux, calculée en
TypeScript) et `req` (SR / DR requis et `miss`, ceux que le profil n'atteint pas — même règle que
`missing`, jamais les notes du profil). Le format reste en version 1 : un bloc sans ces champs se lit, et
l'extension 0.9 ignore ces champs.

**L. Défilement par la chronologie.** WidgetKit n'anime pas un widget en continu. Une entrée par minute
pendant une heure, puis toutes les 5 minutes pendant 3 heures (si iOS tarde à recharger), plus une à chaque
départ ; `.after(maintenant + 1 h)`. Page = (position de l'entrée + décalage) modulo le nombre de pages ; la
position continue d'une chronologie à l'autre grâce à une ancre gardée dans les `UserDefaults.standard` de
l'extension (privés : aucun groupe nécessaire). Boutons ‹ › : `Button(intent:)` avec un `AppIntent`
défini dans l'extension seulement (iOS 17), qui change le décalage ; WidgetKit recharge alors la
chronologie. Tous les widgets en images partagent ce décalage (ils avancent ensemble). Transition : page
identifiée par `.id`, `.push(from: .trailing)`, fond en fondu.

**M. Les photos sont téléchargées par l'application, jamais par l'extension** (la règle « l'extension ne
fait aucune requête réseau » reste vraie, et un essai le contrôle). Méthode `setWidgetImages({ items })` du
module `CordeNative` : pour chaque photo absente, `URLSession` éphémère (ni cookies, ni cache, ni
`Referer`), `Accept: image/*`, 15 s, 4 Mo, `https` seulement, aucun hôte en adresse IP, `localhost` ni
`.local` (redirections comprises) ; réduction ImageIO (vignette ≤ 720 px), JPEG de qualité 0,6 puis moins
jusqu'à tenir sous **48 Kio** ; suppression de celles qui ne sont plus listées ; rechargement du seul widget
en images. La partie JavaScript n'envoie la liste qu'après l'écriture du bloc, quand elle a changé (ou
15 minutes après un échec), et vide quand le planning est coupé. Le widget lit la photo au moment du
dessin (pas dans l'entrée : la mémoire d'une extension de widgets est limitée à ~30 Mo), réduite encore à
480 px pour le petit.

**N. Les photos sont rangées dans le trousseau partagé**, un élément par photo (service
`io.github.dultra123.corde.widgets.img`, compte = la clé, même groupe, `AfterFirstUnlock`, jamais
synchronisé), au plus 8 × 48 Kio. Choix assumé, pour la même raison que C : un App Group demanderait trois
formulaires au propriétaire. Limites : le trousseau n'est pas fait pour des images — aucune taille maximale
n'est documentée, les ennuis connus commencent vers le mégaoctet (on reste à 48 Kio par élément) — et ses
éléments **survivent à la désinstallation** de l'application (rien de secret ; ils sont remplacés ou
effacés par la version réinstallée, et couper le planning efface les photos). Repli prévu si iOS refusait
ces éléments sur un iPhone (diagnostic : nombre de photos, poids, dernier `OSStatus`) : un App Group, qui ne
toucherait que `SharedStore.swift` (et un formulaire du propriétaire).

**O. Voir sans Mac** : trois planches de plus (`slides-fr-dark`, `slides-fr-light`, `slides-en-dark`) avec
des photos d'exemple **dessinées par le code** (aucune photo dans le dépôt), passées par la vraie réduction ;
l'essai de fumée fait aussi un aller-retour du magasin des photos (écrire ~40 Kio, relire, lister,
supprimer) et la réduction d'une image fabriquée de 4000 × 3000. Aucun téléchargement dans la CI.

Ce qui ne peut être vérifié que sur un iPhone : le téléchargement réel des photos des deux sites, les
éléments de 48 Kio dans le trousseau partagé de l'extension signée, la lecture au dessin dans la mémoire de
l'extension, le rythme réel du défilement (iOS peut retarder des entrées), les boutons ‹ ›.

## Ajout — certificat de distribution stable (2026-11-14)

Après quatre envois, le propriétaire a reçu quatre e-mails d'Apple « your certificate has been revoked » :
la décision J (un certificat par exécution, révoqué par la suivante) en envoie un **à chaque build**.

**P. Certificat réutilisé quand un secret facultatif donne sa clé.** Nouveau secret
`APPLE_DIST_KEY_BASE64` : une clé privée RSA 2048, PEM encodé en base64, donnée à la seule étape de
signature du job `testflight`. Avec lui, `ios-signing.sh` signe avec cette clé (CSR faite sur le runner à
partir d'elle) et `asc-api.mjs` réutilise le certificat `DISTRIBUTION` du compte dont la **clé publique
est la même** (comparaison SPKI DER, `X509Certificate` de Node sur `certificateContent`) et qui expire
dans **plus de 30 jours** ; sinon il en crée un avec la même clé. Le ménage supprime toujours les profils
« Corde CI… » (inchangé) mais ne révoque un certificat qui leur est attaché que s'il porte une autre clé
(ancien mode, nettoyé une dernière fois) ou s'il expire dans moins de 30 jours (remplacé). **Sans le
secret, rien ne change** (mêmes requêtes, vérifié par les essais) : la fusion était sans risque avant que
le propriétaire colle le secret ; un secret invalide arrête l'étape avec un message clair, sans la valeur.

Options écartées : un `.p12` complet (clé + certificat) dans les secrets — il faudrait le refaire à la main
chaque année et après toute révocation, alors que l'API recrée le certificat seule à partir de la clé ;
ne plus révoquer du tout dans l'ancien mode — les certificats s'accumuleraient jusqu'à la limite d'Apple
(`409`) ; garder le certificat dans un cache de GitHub Actions — un cache n'est pas fait pour une clé
privée (lisible par toute exécution du dépôt, y compris d'autres branches).

Ce qui ne se vérifie qu'avec le vrai compte : la forme de `certificateContent` dans une **liste**
(`GET /v1/certificates`, `GET /v1/profiles/{id}/certificates` ; s'il manque, chaque certificat est relu
seul), l'effet réel de `filter[certificateType]` (s'il est refusé, la liste entière est lue ; le type est
toujours revérifié), l'acceptation par Apple d'une seconde CSR de la même clé au renouvellement, et la
limite du nombre de certificats de distribution (le renouvellement révoque avant de créer).

## Conséquences

- Le propriétaire n'a **rien à faire côté Apple** : l'identifiant « Corde Widgets » apparaît tout seul au
  premier envoi, avec un deuxième profil « Corde CI … widgets ».
- Les widgets ne voient que ce que l'application leur a écrit : sans ouverture de Corde pendant plus de
  quelques jours (ou après la fin des horaires écrits), ils disent « Ouvre Corde pour actualiser ».
- Ce qui ne peut être vérifié que sur un vrai iPhone : le trousseau partagé lu depuis l'extension signée,
  l'affichage réel des widgets et de l'activité, le lien à froid, la fin automatique de l'activité ;
  ordre des essais dans `docs/IOS.md`.
