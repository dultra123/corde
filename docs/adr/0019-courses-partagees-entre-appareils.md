# ADR 0019 — « Mes courses » partagées entre le PC, le serveur Corde et le téléphone

- **Statut** : **décidée** — 2026-11-16 (demande du propriétaire, 15 nov. : une course choisie sur le PC
  n'apparaît pas dans l'application du téléphone ; elle doit y être, y compris hors de chez lui, en 5G).
- **Source** : `crates/corde-server/src/races/{merge,store,mod}.rs`, `crates/corde-server/src/hub_link.rs`
  (`sync_picks_once`, `run_picks_periodic`), `packages/ui/src/races/picks-merge.ts`,
  `packages/ui/src/mobile/{race-book,picks-sync,servers}.ts`, `docs/SECURITY-MODEL.md` §4, §7.3, §8.2.

## Contexte

« Mes courses » existait en deux endroits qui ne se parlaient pas :

- **sur un serveur Corde** (le PC, ou le serveur de la maison) : `races/picks.json`, lu et écrit par les
  pages servies par ce serveur (l'application Windows, un navigateur) ;
- **dans l'application du téléphone** : son propre carnet (`mobile/race-book.ts`), parce que
  l'application lit le planning elle-même et sonne ses rappels sans PC.

Une course choisie sur le PC restait donc sur le PC. Hors de chez lui, le téléphone ne joint pas le PC : il
joint le serveur de la maison par le VPN (ACCES-EXTERIEUR.md, voie C). Le PC, lui, n'envoyait au serveur que
des sessions.

## Décision

### A. Chaque appareil garde sa copie ; les copies se fusionnent

Aucun appareil n'est « le maître » : le téléphone doit marcher sans serveur, le PC sans serveur de la
maison. Chaque copie (`PickSet`) contient :

- les courses choisies, chacune avec **l'instant de son dernier changement** (`changed_ms`) ;
- les courses **retirées**, chacune avec l'instant du retrait (`removed_ms`), gardées jusqu'à 3 h après le
  départ — comme une course choisie. Sans elles, un appareil qui a encore la course la renverrait à celui
  qui vient de la retirer.

Une course choisie avant ce partage n'a pas d'instant : elle compte pour 0.

### B. Une seule règle de fusion, écrite deux fois

`merge(a, b, maintenant)` (Rust : `races/merge.rs` ; TypeScript : `races/picks-merge.ts`) :

1. pour un même identifiant (`<course>@<départ>`), **le changement le plus récent gagne** ;
2. au même instant, **le retrait gagne** sur le choix ; entre deux choix du même instant, un ordre fixe de
   leurs champs décide (nom, circuit, durée, image, catégorie, source, niveau, rappel de la veille ; textes
   comparés par point de code dans les deux langages), pour que deux appareils gardent toujours le même ;
3. ce qui a commencé depuis plus de 3 h est oublié, choix et retraits ;
4. au plus 50 courses (les départs les plus proches) et 200 retraits (les plus récents).

L'ordre des deux copies ne compte pas, fusionner deux fois ne change rien, et trois copies donnent le même
résultat dans n'importe quel ordre (sous les deux limites) : c'est ce qui permet de relayer par le serveur
de la maison sans rien coordonner. Les deux écritures sont vérifiées sur **les mêmes cas**
(`crates/corde-server/src/races/merge-vectors.json`, lu par les essais Rust et par les essais TypeScript).

### C. Les horloges

Les instants viennent de l'horloge de chaque appareil. Un appareil qui change une course lui donne un
instant **postérieur à tout ce qu'il connaît d'elle** (`max(maintenant, connu + 1)`) : son propre changement
gagne toujours sur ce qu'il a vu, même si l'horloge d'un autre appareil avance. Deux changements faits
chacun de son côté, sans s'être vus, sont départagés par les horloges : avec un écart d'horloge, le plus
ancien peut gagner. Accepté (un pilote, quelques appareils à l'heure du réseau).

### D. Une route : `POST /api/races/picks/sync`

Corps : la copie de l'appareil qui appelle (`{picks: [{pick, changed_ms}], removed: [{id, removed_ms}]}`).
Le serveur la fusionne dans la sienne et répond la copie fusionnée ; l'appelant fusionne la réponse dans la
sienne. Un aller-retour suffit dans les deux sens.

- **Mêmes droits que `PUT` et `DELETE /api/races/picks/{id}`** : ouverte sur le PC, appareil appairé sur le
  serveur de la maison. Elle ne permet rien de plus qu'eux (ajouter, remplacer, retirer des choix).
- **Bornée** : 256 Kio, 50 choix, 200 retraits ; au-delà, 400. L'enveloppe est stricte ; **chaque entrée
  est validée comme un choix envoyé seul, et une entrée refusée est laissée de côté**, pas l'envoi entier :
  un appareil d'une autre version peut connaître un champ que celui-ci ignore, le reste doit passer.
- `PUT` et `DELETE` d'un choix, et la correction d'heure (qui déplace les courses quotidiennes d'un des
  deux sites : un retrait à l'ancienne heure, un choix à la nouvelle), posent les instants.

### E. Qui parle à qui

- **Le téléphone** (`mobile/picks-sync.ts`) échange avec **tous les serveurs de « Mes serveurs »**, côte à
  côte, 5 s au plus chacun : au lancement, au retour au premier plan, après chaque choix ou retrait, et
  toutes les minutes tant que l'application est à l'écran. Un PC ne demande pas d'appairage ; un serveur de
  la maison reçoit le jeton de l'appareil. Ce qu'un serveur apprend au téléphone est passé aux autres dans
  la foulée. Un serveur muet, non appairé ou trop ancien est laissé de côté : les courses restent sur le
  téléphone, l'échange est retenté plus tard. Sous « Mes courses », l'application dit avec qui elles sont
  partagées et ce qui manque (serveur injoignable, à appairer, à mettre à jour).
- **Le PC appairé à un serveur de la maison** (`hub_link.rs`) lui envoie sa copie **dès qu'elle change**,
  et toutes les 30 s pour apprendre ce que le téléphone y a déposé. C'est une tâche à part des envois de
  sessions : un long envoi ne la retarde pas, et elle ne touche pas à l'état affiché du lien. Le verrou des
  courses n'est jamais tenu pendant une requête.
- **Les pages d'un serveur** (application Windows, navigateur) relisent « Mes courses » toutes les 30 s,
  comme avant : pas de nouveau message sur le WebSocket.

Résultat : une course choisie sur le PC est sur le serveur de la maison dans la seconde, et sur le
téléphone à sa prochaine minute d'écran, à la maison comme en 5G par le VPN. Une course choisie ou retirée
sur le téléphone est sur le PC en moins d'une minute.

### F. Compatibilité

- `races/picks.json` **garde sa forme** (la liste des choix) ; les instants et les retraits sont à côté,
  dans `races/picks-sync.json`. Une version plus ancienne relit donc le dossier sans rien perdre d'autre que
  le partage. Sur le téléphone, même principe (une clé de stockage en plus).
- Un serveur plus ancien répond **405** à cette adresse (il n'y connaissait que `PUT` et `DELETE` d'un
  choix) : le PC et le téléphone le reconnaissent, ne signalent pas d'erreur et réessaient plus tard (le PC
  toutes les 10 minutes, avec une ligne dans son journal). **Le serveur de la maison doit donc être mis à
  jour** pour relayer.

## Ce qui n'est pas partagé

- **Les réglages de la page Courses** (délais de rappel, catégories affichées, notes, correction d'heure) :
  chacun les siens, un téléphone et un PC ne s'utilisent pas pareil. Conséquence : une correction d'heure
  faite sur un seul appareil y déplace les courses choisies, et les autres reçoivent ces courses à l'heure
  corrigée — sans que leur propre planning, lui, soit corrigé.
- **Les rappels déjà sonnés** : chaque appareil sonne les siens. Une course partagée sonne donc sur le PC
  (s'il est allumé) et sur le téléphone.

## Limites connues

- **Le téléphone ne fait rien en arrière-plan.** Une course choisie sur le PC arrive sur le téléphone — et
  ses rappels y sont programmés — à la prochaine ouverture de l'application. Les widgets suivent de la même
  façon (ils ne font aucune requête). Une récupération en arrière-plan demanderait un mode d'exécution que
  ni iOS ni Android ne garantissent à l'heure : écarté pour l'instant.
- **Au-delà de 50 courses** dans la réunion de deux copies, les départs les plus lointains sont laissés de
  côté des deux côtés.
- Hors de chez soi, il faut un serveur de la maison joignable (VPN) : sans lui, le téléphone garde ses
  courses et les échange au retour.

## Alternatives écartées

- **Le téléphone lit simplement les courses du serveur actif** : hors ligne il n'aurait plus rien, et ses
  rappels locaux dépendraient d'un serveur allumé.
- **Synchroniser dans les rounds de sessions du PC** (toutes les 5 minutes, après les envois de fichiers) :
  trop lent pour « je choisis sur le PC, je regarde le téléphone », et bloqué par un gros envoi.
- **Un compteur de version au lieu d'horloges** : il faudrait un identifiant par appareil et un vecteur par
  course ; disproportionné pour une liste de 50 lignes d'un seul pilote.
- **Refuser tout l'envoi à la première entrée invalide** : deux versions différentes ne partageraient plus
  rien, en silence.

## Vérification

- `races/merge.rs`, `races/picks-merge.test.ts` : les mêmes 15 cas dans les deux langages, dans les deux
  ordres, puis rejoués ; trois copies dans tous les ordres ; les deux limites ; lecture bornée d'un envoi.
- `races/store.rs`, `mobile/race-book.test.ts` : instants posés, retrait mémorisé puis oublié 3 h après le
  départ, horloge en avance, correction d'heure, relecture du disque.
- `tests/races_sync.rs` : trois vrais serveurs — un PC, un serveur de la maison, un « téléphone » — : une
  course choisie sur le PC arrive sur le serveur sans intervention, un retrait fait sur le téléphone revient
  au PC, une vieille copie ne ressuscite rien, le téléphone parle aussi au PC en direct, envois invalides ;
  et un serveur plus ancien (405) est reconnu.
- `mobile/picks-sync.test.ts` : PC muet et serveur joignable (le cas 5G), relais d'un serveur à l'autre,
  serveur éteint / trop ancien / non appairé, jeton jamais écrit dans le journal, un échange à la fois.
- **Non vérifié** : sur les appareils du propriétaire (PC, NAS, iPhone en 5G) ; Android.
