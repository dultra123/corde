# ADR 0008 — Passage en open source : licence, historique git, publication

- **Statut** : **décidée** — 2026-11-11 (le propriétaire : « mettre ce projet en open source et le laisser
  accessible à tous tout en travaillant dessus »). **Licence : GPL-3.0-or-later** (§2). **Nom : Corde**.
  **Dépôt public : `dultra123/corde`** (§3), créé neuf ; le dépôt de développement actuel reste une archive
  privée.
- **Source** : ADR 0005 (licences, header S397), ADR 0007 (signature Windows), `docs/STATUS.md`.

## Contexte

Jusqu'ici le dépôt de développement est privé et sans licence (« tous droits réservés pour l'instant »,
ADR 0005 §5). Le code a été écrit dès le départ pour pouvoir être ouvert sans réécriture. Trois sujets
restent à régler avant que qui que ce soit d'autre puisse voir le dépôt.

## 1. État des lieux (vérifié le 2026-11-11)

| Sujet | Constat |
|---|---|
| Dépendances Rust (serveur : 285 paquets ; coquille de bureau : 610) | toutes permissives : MIT, Apache-2.0, BSD, ISC, Zlib, Unicode-3.0, CC0 ; MPL-2.0 pour quelques bibliothèques non modifiées (`cssparser`, `selectors`, `option-ext`…). Aucune GPL / AGPL / LGPL imposée. |
| Dépendances web (64 paquets, dont 11 livrés dans l'application) | MIT, Apache-2.0, BSD, ISC ; MPL-2.0 pour `lightningcss` (outil de build) ; polices Barlow sous OFL-1.1. |
| Code tiers copié | aucun : pas de code GPL (TinyPedal, plugin rF2, winecarte), header S397 non versionné (ADR 0005). |
| Fichiers du jeu | `reference/samples/` : deux fichiers de résultats **anonymisés** (noms inventés), deux petits fichiers de configuration ; trois réglages `.svm` du propriétaire en fixtures de test (aucune donnée personnelle). |
| Arbre courant | pas de nom réel, d'adresse e-mail, d'identifiant Steam ni de chemin personnel (les `192.168.1.20` sont des exemples). Le pseudonyme public du propriétaire apparaît comme auteur. |
| **Historique git** | **non publiable** : 16 des 26 commits portent une adresse e-mail privée comme auteur ; d'anciennes versions des échantillons, de tests et d'une capture d'écran contiennent les vrais noms des pilotes d'une course en ligne. |

Les trois licences envisagées (§2) sont donc toutes compatibles avec les dépendances.

## 2. Licence — choix du propriétaire : GPL-3.0-or-later

**Décision du 2026-11-11** : Corde est publiée sous **GPL-3.0-or-later** (GNU General Public License,
version 3 ou toute version ultérieure). Fichier `LICENSE` à la racine (texte officiel de la FSF, sans
modification), champ `license` des `Cargo.toml` (`[workspace.package]` des deux workspaces) et des
`package.json`, mention de copyright « Copyright (C) 2026 dultra__ et les contributeurs » (README, bundle
Windows). Les polices Barlow restent sous OFL-1.1, les dépendances sous leurs licences (§1), toutes
compatibles avec la GPL-3.0. La règle « aucune copie de code tiers » de l'ADR 0005 reste en vigueur.

Les options étudiées :

La question qui départage : **accepte-t-on que quelqu'un reprenne le code pour en faire un logiciel fermé
(éventuellement payant) sans rien reverser ?**

| Licence | En une phrase | Reprise dans un logiciel fermé | Remarques |
|---|---|---|---|
| **MIT** | « faites-en ce que vous voulez, gardez la mention d'auteur » | autorisée | la plus simple et la plus répandue ; aucune protection contre une copie fermée |
| **Apache-2.0** | comme MIT, avec une clause sur les brevets et l'obligation de signaler les fichiers modifiés | autorisée | courante dans l'écosystème Rust (souvent en double licence MIT / Apache-2.0) |
| **GPL-3.0** | « toute version distribuée, modifiée ou non, doit rester ouverte sous la même licence » | **interdite** | protège le projet d'une récupération fermée ; c'est la licence de TinyPedal ; certaines entreprises évitent le code GPL |

Écartées : les licences « non commerciales » (CC BY-NC, PolyForm NC…). Elles ne sont pas open source au sens
de l'OSI, ce qui ferme la signature gratuite (ADR 0007, option F) et décourage les contributions.

Une licence publiée ne se retire pas des versions déjà diffusées : on peut en changer pour la suite (tant
que tous les contributeurs sont d'accord), pas reprendre ce qui a été donné. Ce document n'est pas un avis
juridique.

Les contributions entrent sous la même licence que le projet (règle écrite dans `CONTRIBUTING.md`), et
Corde reste non affiliée à Studio 397 / Motorsport Games (mention au README).

## 3. Historique git : dépôt public neuf, un seul commit de départ

Réécrire l'historique sur place ne suffit pas : sur GitHub, la référence de la PR n° 1
(`refs/pull/1/head`), les branches `feat/desktop-shell` et `wip/feedback-interpretation` et les commits
devenus orphelins restent accessibles par leur identifiant, et deviendraient publics en même temps que le
dépôt.

**Décision** : le dépôt actuel **reste privé** (archive de l'historique de développement). La version
publique part d'un **dépôt neuf** dont le premier commit est l'arbre courant, sans historique.

Procédure (les étapes 1 et 5 ne peuvent être faites que par le propriétaire du compte GitHub) :

1. Propriétaire : garder le dépôt actuel en archive privée, créer le dépôt public **`dultra123/corde`** vide
   (sans README ni licence) et y donner accès à l'agent.
2. Agent : ajouter `LICENSE` (texte officiel), le champ `license` des `Cargo.toml` / `package.json`, la
   section Licence du README. **Fait le 2026-11-11** (avec le renommage en Corde, version 0.3.0).
3. Agent : repasser le contrôle « données personnelles » (noms, e-mails, identifiants, chemins) sur l'arbre,
   y compris les images.
4. Agent : créer le commit de départ, pousser `main`, vérifier la CI et la pré-release `continuous` (lien
   d'installation du README). Les commits faits par l'agent sont signés `Claude <noreply@anthropic.com>` ;
   le droit d'auteur est au nom du pseudonyme public du propriétaire (`dultra__`, dans `LICENSE` /
   README / `Cargo.toml`), **jamais** son adresse personnelle.
5. Propriétaire : passer le dépôt en public, coller la description et les sujets (« About »).

## 4. Ce que le dépôt public change

- **Signature du programme d'installation** : un projet open source peut demander la signature gratuite de
  la SignPath Foundation, ou un certificat Certum « Open Source » (ADR 0007, options F et C bis). C'est la
  réponse à SmartScreen et à Smart App Control. Demande à faire une fois le dépôt public.
- **CI** : les minutes GitHub Actions sont gratuites pour un dépôt public.
- **Données des utilisateurs** : les fichiers de résultats du jeu contiennent les noms des autres pilotes.
  Les modèles de ticket doivent demander de ne pas les joindre tels quels ; Corde garde tout en local
  (ADR 0006) et n'envoie rien.
- **Fichiers de diagnostic** (`diagnostics/lmu-api/`) : ils décrivent l'API locale du jeu et ne sont pas
  versionnés.
- **Contributions** : `docs/CONTRIBUTING.md` s'applique ; ajouter un modèle de ticket et un modèle de PR.

## Conséquences

- L'historique détaillé (26 commits) n'est pas public ; il reste consultable dans l'archive privée.
- La licence est choisie (GPL-3.0-or-later) : la publication ne dépend plus que du dépôt public neuf
  (étapes 1, 4 et 5 du §3).
- Toute version distribuée de Corde, modifiée ou non, doit rester sous GPL avec son code source.
