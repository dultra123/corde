# Mettre à jour Corde serveur sur un NAS Synology, pas à pas

> Pour qui : tu as déjà Corde serveur sur ton NAS ([SYNOLOGY.md](SYNOLOGY.md)) et une nouvelle version de
> Corde vient de sortir. Compte **2 minutes**. Rien à télécharger, rien à écrire : le NAS va chercher la
> nouvelle version tout seul.
>
> Les noms des boutons sont ceux d'un vrai NAS (DS220+, DSM 7.4.1, Container Manager 24.0.2, en français).
> Le téléchargement de l'image par le NAS y a été vu le 22 novembre 2026 ; l'enchaînement complet
> ci-dessous sert pour la première fois avec la version 1.13.0.

## Quand le faire

- Quand Corde sur le PC annonce une nouvelle version (**Réglages** → **Mises à jour**), ou affiche
  **« Serveur à mettre à jour »**.
- Dans l'ordre que tu veux : le NAS avant le PC, ou juste après. Le serveur doit seulement finir avec une
  version au moins aussi récente que celle du PC.
- Pas pendant que tu roules : le serveur est coupé une minute. (Le PC ne perd rien : il renvoie ensuite ce
  qu'il n'a pas pu envoyer.)

Tes sessions et tes appareils appairés **ne bougent pas** : ils sont dans le dossier `docker/corde/data`,
que la mise à jour ne touche pas.

## Les étapes

1. **Ouvre le NAS** dans le navigateur du PC : l'adresse de ton NAS suivie de `:5000` (par exemple
   `http://192.168.1.50:5000`), et connecte-toi avec ton compte administrateur.
2. Ouvre **Container Manager** (dans le **Menu principal**, en haut à gauche).
3. Dans le menu de gauche, clique sur **Projet**.
4. **Double-clique** sur la ligne **`corde`**. La page du projet s'ouvre : en haut, le nom `corde` et son
   dossier (`/volume1/docker/corde`) ; en haut à droite, le bouton **Arrêt** et le menu **Action**.
5. Clique sur **Arrêt**.
   - Une fenêtre **Terminal : Arrêter - corde** s'ouvre et affiche `Container corde Stopping`, puis
     `Container corde Stopped`, puis **`Exit Code: 0`**.
   - Clique sur **Fermer**. Le bouton **Arrêt** est devenu **Démarrer** : c'est normal.
6. Clique sur **Action** (à côté de **Démarrer**), puis sur **Créer**.
   - Une fenêtre **Terminal : Créer - corde** s'ouvre. Elle affiche le téléchargement de la nouvelle
     version (des lignes « Pulling », « Extracting », puis `corde Pulled`), ce qui prend moins d'une minute,
     puis `Container corde Started` et **`Exit Code: 0`**.
   - La ligne « Your kernel does not support PIDs limit capabilities… » est normale sur un NAS Synology.
   - Clique sur **Fermer**.
7. **C'est fini.** La pastille du projet est verte.

## Vérifier que c'est bien la nouvelle version

- **Le plus simple** : dans Corde sur le PC, **Réglages** → **Serveur Corde** : l'état est **Connecté**
  (laisse-lui une minute).
- **Pour lire le numéro** : Container Manager → **Conteneur** (menu de gauche) → double-clic sur **`corde`**
  → onglet **Journal**. Dans le champ **Rechercher**, tape `corde-server` : la ligne
  `corde-server <numéro> — serveur Corde (rôle hub)` donne la version qui tourne (la ligne la plus récente
  est en haut).

## Si ça ne se passe pas comme prévu

| Ce que tu vois | Quoi faire |
|---|---|
| À l'étape 6, **aucune ligne « Pulling »**, ou le journal montre encore l'ancien numéro | Le NAS n'a pas repris l'image. Rien n'est cassé : le serveur tourne dans l'ancienne version. Signale-le (c'est la procédure qu'il faut alors corriger). |
| La fenêtre finit par une **erreur** (« pull access denied », « no such host », « timeout »…) au lieu de `Exit Code: 0` | Le NAS n'a pas pu joindre Internet à cet instant. Clique sur **Fermer**, puis sur **Démarrer** : le serveur repart dans l'ancienne version. Réessaie plus tard. |
| Tu as fermé la page après l'étape 5 | Le projet est simplement arrêté. Reviens dans **Projet** → `corde`, et reprends à l'étape 6 (ou **Démarrer** pour le relancer tel quel). |
| Le PC dit encore **« Serveur à mettre à jour »** après une minute | Vérifie le numéro dans le journal (ci-dessus). S'il est ancien : première ligne de ce tableau. |

À ne pas faire :

- Dans le menu **Action**, ne clique pas sur **Nettoyer** ni sur **Supprimer** : ils défont le projet (tes
  données resteraient dans leur dossier, mais il faudrait recréer le projet).
- Ne lance jamais Corde depuis la section **Image** (bouton **Exécuter**) : cela crée un second conteneur,
  sans tes données ni le bon port. Si c'est arrivé, voir
  [SYNOLOGY.md §14](SYNOLOGY.md#14-dépannage).

## Si ton serveur a été installé avant la version 1.12.0

Il prend encore son image dans un fichier importé : il faut d'abord faire, **une seule fois**, le passage
décrit dans [SYNOLOGY.md §11](SYNOLOGY.md#11-mettre-à-jour). Ensuite, les étapes ci-dessus suffisent.
