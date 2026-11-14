# ADR 0014 — Tableau de bord du téléphone : une vue à part, quatre pages, écran allumé, démonstration locale

- **Statut** : **décidée** — 2026-11-15 (demande du pilote : « Sur téléphone, le format comme sur PC c'est
  pas terrible, il faudrait plutôt un Dashboard comme sur mon volant KS Pro de MOZA ou comme SimHub »).
  Maquette montrée au pilote avant le travail. Livrée en 1.1.0 (branche `phone-dash`).
- **Source** : `packages/ui/src/dash/`, `packages/ui/src/styles/dash.css`, `docs/IOS.md` (« Le tableau de
  bord »), `docs/TESTING-ON-PC.md` §6, `PRIVACY.md`, ADR 0011 et 0013 (applications mobiles).

## Contexte

La page Direct montrait sur un téléphone la même grille de widgets que sur le PC, sur quatre colonnes au
lieu de douze, qu'on fait défiler : rien ne se lit d'un coup d'œil, et rien n'est pensé pour un téléphone
posé en paysage à côté du volant. Les écrans de volant (Moza KS Pro) et les tableaux de bord de SimHub font
l'inverse : peu d'informations, très grandes, toujours au même endroit, et quelques pages qu'on change d'un
geste. Contraintes inchangées : pas de service tiers, aucune donnée inventée, interface en français et en
anglais, une seule interface pour le navigateur, l'iPhone et Android.

## Décisions

**A. Une vue à part (`src/dash/`), pas une disposition de widgets.** Une grille gridstack ne peut pas donner
un rapport qui occupe la moitié de l'écran ni des chiffres qui grandissent avec l'écran sans déborder.
Le tableau de bord est un composant à lui, qui remplace la grille sur la page Direct :

- toujours dans l'application mobile ;
- dans un navigateur, quand l'appareil est un téléphone (pointeur tactile **et** petit côté ≤ 500 px CSS) :
  un PC, même dans une petite fenêtre, et une tablette gardent les widgets ;
- réglage **Affichage du Direct** (Automatique / Tableau de bord / Widgets), enregistré avec les autres
  réglages de l'appareil ;
- **`#/dash`** : toujours le tableau de bord (tablette en kiosque, second écran, essais). La route est une
  variante de `dashboard` (`{ page: "dashboard", dash: true }`) : la navigation automatique, la porte « il
  faut un serveur » et l'onglet Direct la traitent comme le Direct.

Le code des widgets n'est pas recopié : les règles partagées (secteurs, classement autour du pilote) sont
sorties dans `widgets/shared/`, l'autonomie vient de `lib/fuel`, les couleurs de `lib/color`.

**B. Quatre pages, glissées du doigt.** Course, Pneus & freins, Carburant & énergie, Chronos & écarts. Une
bande qui défile horizontalement avec `scroll-snap` (défilement natif, élan compris), plutôt qu'un
glissement écrit à la main ; les points en bas, les flèches du clavier et le menu la déplacent. La dernière
page est retenue sur l'appareil (stockage local, simple commodité).

**C. Rampe de régime et drapeaux.** 15 feux (5 verts, 5 jaunes, 5 rouges) de 55 % à 96 % du régime maximal,
toute la rampe clignote en bleu à partir de 97 % ; limiteur de stands : feux alternés et bandeau. Les feux
de drapeau et la teinte du cadre ne montrent que ce que le protocole donne : jaune de la session
(`phase`, `yellow_flag`) ou du secteur du pilote (`sector_flags`), bleu (`flag_blue` du pilote), damier
(`finish_status`). Avec `prefers-reduced-motion`, rien ne clignote.

**D. Les valeurs rapides hors de React.** Rapport, vitesse, rampe, écart et tour en cours sont écrits dans
la page par `useFrameDraw` (une fois par image qui apporte une trame), en ne changeant que le texte et des
attributs `data-*` ; la barre d'écart est une transformation (`scaleX`), sans mise en page. Le reste (positions,
températures, carburant…) se redessine quatre fois par seconde par React. Les cases à chiffres rapides ont
`contain: layout paint`.

**E. Tailles en unités de conteneur.** Chaque page est un conteneur (`container-type: size`) ; une unité
`--u` = min(1 % de la hauteur, 0,46 % de la largeur) en paysage, et une autre en portrait, règle toutes les
tailles. Le même dessin va de 667 × 375 à la tablette et à la fenêtre de PC ; en portrait les cases
s'empilent (pas d'écran « tournez votre téléphone »). Zones sûres respectées (encoche des deux côtés, barre
d'accueil).

**F. Plein écran, un menu ☰.** Pas de barre du haut ni d'onglets sur le tableau de bord ; un bouton ☰ (cible
de 44 px) ouvre les pages, « Passer aux widgets », les réglages et les autres sections. Le bouton retour
d'Android ne change pas. Quand un téléphone a choisi les widgets, la barre du haut propose de revenir au
tableau de bord (jamais sur un PC : « Automatique » n'y choisit pas le tableau de bord).

**G. Écran allumé, par le système du téléphone.** Tant que le tableau de bord est affiché et la page
visible : dans l'application, une méthode `setKeepAwake` du module natif CordeNative
(`UIApplication.isIdleTimerDisabled` sur iPhone ; sur Android un tout petit module du même nom qui pose
`FLAG_KEEP_SCREEN_ON`) ; dans un navigateur, `navigator.wakeLock`, repris quand la page revient au premier
plan. Aucune autorisation. Réglage, activé par défaut.

**H. Une démonstration dans l'application, locale et éphémère.** « Voir une démonstration » (Mes serveurs,
et l'écran « aucun serveur ») lance le générateur de course qui existait déjà (`src/demo/`), sur le
téléphone, sans réseau, et ouvre le tableau de bord avec l'étiquette « Démonstration ». Pour qu'elle ne se
mêle jamais aux données d'un vrai serveur, la connexion ne s'ouvre pas tant qu'elle tourne (même quand
l'application teste à nouveau son serveur). Elle n'est pas retenue au redémarrage, et **aucun lien
`corde://` ne la lance** : un lien ne doit pas pouvoir faire passer l'écran d'un pilote en course sur des
données inventées. But : essayer l'application sans PC, et la montrer aux relecteurs des magasins.

**I. Captures en CI par les entrées de débogage existantes.** L'essai de fumée Android envoie une demande
`demo` (intention de débogage, build Debug seulement) ; celui de l'iPhone lance l'application avec
`-CordeDemoDash` (build Debug seulement), qui envoie le même événement à la page. Pages et orientations sont
prises dans une liste fermée.

## Conséquences

- La grille de widgets du PC ne change pas (vérifié : mêmes positions et tailles avant et après).
- Le tableau de bord est chargé à part (import dynamique) : un PC qui ne l'ouvre jamais ne le télécharge pas.
- Ce qui n'est vérifié que sur la course de démonstration et sur simulateur / émulateur : la lisibilité à
  bout de bras, l'écran gardé allumé sur un vrai téléphone, le comportement avec le jeu (régime maximal,
  drapeaux, limiteur, voitures sans ABS / sans énergie virtuelle).
- Pas encore : choix des cases par le pilote, seuils de la rampe réglables par voiture, lumière de passage
  propre au rapport.
