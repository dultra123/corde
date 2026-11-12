# ADR 0009 — Mises à jour depuis l'application : releases GitHub signées

- **Statut** : **décidée** — 2026-11-12 (demande du propriétaire, 11 nov. : « un bouton dans les réglages
  pour faire une mise à jour, plus rapide et plus facile » que de retélécharger l'installeur). Livrée en 0.6.0.
- **Source** : ADR 0007 (signature de code Windows), ADR 0008 (dépôt public `dultra123/corde`),
  `docs/SECURITY-MODEL.md` §5 bis, `PRIVACY.md`.

## Contexte

Jusqu'à 0.5.1, mettre Corde à jour = aller sur GitHub, télécharger l'installeur, passer SmartScreen, quitter
Corde, installer. Le pilote ne code pas : chaque version coûte un aller-retour. La coquille Tauri 2 a un
module officiel de mise à jour (`tauri-plugin-updater`) qui télécharge un installeur **signé** et l'exécute.
Il lui faut une adresse fixe qui décrit la dernière version (`latest.json`) et des installeurs signés avec
une clé dont la partie publique est dans l'application.

## Décision

1. **Source des mises à jour : les releases GitHub du dépôt public**, rien d'autre (pas de serveur à nous).
   Adresse fixe : `https://github.com/dultra123/corde/releases/latest/download/latest.json`. GitHub sert
   le fichier de la release marquée « latest » : la plus récente des **versions numérotées** (une
   pré-release, comme `continuous`, n'est jamais « latest »).
2. **Releases numérotées automatiques** (CI, job `version`) : quand le numéro de `tauri.conf.json` change
   sur `main`, la release `v<version>` est créée une fois, avec l'installeur, sa signature, l'archive
   portable et `latest.json` ; les notes viennent de `CHANGELOG.md`. Le job `release` (étiquette poussée à
   la main) reste pour les cas particuliers et ne marque jamais sa release « latest ».
3. **Signature minisign de Tauri** (clé générée le 11 nov., sans mot de passe ; publique dans
   `apps/desktop/updater-public-key.txt` et `tauri.conf.json`, privée chez le propriétaire et en secret
   `TAURI_SIGNING_PRIVATE_KEY`). `requireSignedVersion` : la version signée doit être celle annoncée
   (pas de retour à un ancien installeur authentique). Sans le secret (PR, fork), la CI construit
   l'installeur sans artefacts de mise à jour et ne publie pas de release numérotée.
4. **L'application vérifie, le pilote décide** : vérification 30 s après le lancement puis toutes les 6 h
   (désactivable, activée par défaut), notification Windows une fois par version et jamais en jeu ;
   installation **seulement** sur « Installer et redémarrer », refusée pendant que le jeu est détecté ou
   qu'un enregistrement est en cours. Mode passif de l'installeur NSIS (barre de progression, aucune
   question), qui relance Corde.

## Conséquences

- La signature de mise à jour **n'est pas** une signature de code Windows : SmartScreen / Smart App Control
  ne changent pas (ADR 0007). Elle garantit l'origine de ce que le bouton installe, pas la qualité du code.
- **La clé privée devient critique** : la perdre = plus de mise à jour depuis l'application pour les
  versions installées (réinstallation à la main d'une version qui embarque une nouvelle clé publique) ; la
  voir fuiter = quiconque contrôle aussi la page des versions peut pousser une mise à jour. Changer de clé
  = publier d'abord, signée avec l'ancienne, une version qui embarque la nouvelle clé publique (le contrôle
  « même clé que `tauri.conf.json` » de `.github/scripts/updater_manifest.py` est à assouplir pour cette
  version-là).
- **Tout ce qui arrive sur `main` avec un nouveau numéro est publié** : écrire la section du
  `CHANGELOG.md` (le job échoue sans elle), garder la protection de `main` et la double authentification.
- Le dépôt doit être **public** : tant qu'il est privé, `latest.json` répond 404 et l'application affiche
  « Impossible de vérifier pour l'instant » (sans conséquence).
- Vie privée : GitHub voit l'adresse IP du PC à chaque vérification ; rien d'autre n'est envoyé (`PRIVACY.md`).
- La 0.6.0, première version dotée du bouton, s'installe encore à la main.

## Écarté

- **Serveur de mises à jour à nous** (point d'entrée dynamique) : un service à héberger et à surveiller,
  pour rien de plus que ce que GitHub sert déjà.
- **Mise à jour silencieuse automatique** : le pilote peut être en course ou en train d'enregistrer ;
  redémarrer Corde sous ses roues est exclu. Il garde la main.
- **Installeurs `v1Compatible` (zip)** : inutile, aucune version antérieure n'a le module de mise à jour.
- **Pré-release `continuous` comme source** : reconstruite à chaque poussée, sans numéro croissant ; elle
  reste le canal « essayer le dernier état », à installer à la main.
