# Corde serveur : tes sessions sur un Raspberry Pi, PC éteint

> Pour qui : le pilote qui veut consulter ses sessions, l'ingénieur, son profil et ses courses depuis son
> téléphone **sans allumer le PC de jeu**. Pas besoin de savoir programmer : il faut savoir copier des
> commandes. Compte une heure la première fois.
> Accès depuis l'extérieur de la maison : [ACCES-EXTERIEUR.md](ACCES-EXTERIEUR.md) (à lire **après** ce guide).
> Sécurité : [SECURITY-MODEL.md §7](SECURITY-MODEL.md#7-le-serveur-corde-mode-hub). Décision : [ADR 0010](adr/0010-serveur-domestique.md).

## En bref

```text
  PC de jeu (Corde)  ── chaque session terminée ──▶  Raspberry Pi (Corde serveur, allumé 24 h/24)
        │                                                     │
   le direct (en roulant)                       sessions, ingénieur, profil, courses, rappels
        ▼                                                     ▼
  téléphone (même Wi-Fi)                        téléphone (à la maison, ou dehors par ton VPN)
```

- Le **PC** reste le seul à lire le jeu. Il enregistre comme avant, puis **envoie** chaque session terminée au
  serveur (et, au démarrage, tout ce qui n'est pas encore parti).
- Le **serveur** garde tout et le montre à tes appareils **appairés**, et à eux seuls : chaque appareil entre
  une fois un code à 8 chiffres que tu obtiens sur le serveur lui-même.
- Le **direct** (tableau de bord pendant que tu roules) reste sur le PC, comme aujourd'hui.
- Le serveur n'est **jamais** ouvert sur Internet. Depuis l'extérieur, on passe par un VPN que tu montes
  toi-même ([ACCES-EXTERIEUR.md](ACCES-EXTERIEUR.md)).

## 1. Le matériel

| Pièce | Conseil |
|---|---|
| **Raspberry Pi 5** | 4 Go de mémoire suffisent ; 8 Go si le Pi sert aussi à autre chose. |
| **Alimentation** | l'alimentation officielle USB-C du Pi 5 (27 W). Une alimentation trop faible provoque des plantages difficiles à comprendre. |
| **Stockage** | une carte microSD de bonne qualité (classe A2, 32 Go ou plus) suffit pour commencer. Pour un serveur allumé en permanence, un **SSD** (carte d'extension M.2 pour Pi 5, ou SSD USB) est plus durable et plus rapide. Une heure d'enregistrement occupe environ 60 Mo. |
| **Boîtier** | avec refroidissement (le ventilateur officiel du Pi 5, ou un boîtier ventilé). |
| **Réseau** | un **câble Ethernet** vers ta box de préférence (plus stable que le Wi-Fi). |

## 2. Installer Raspberry Pi OS Lite 64 bits

1. Sur ton PC, installe **Raspberry Pi Imager** (site officiel de Raspberry Pi) et branche la carte microSD
   (ou le SSD) au PC.
2. Dans Imager, choisis l'appareil **Raspberry Pi 5**, puis le système **Raspberry Pi OS Lite (64-bit)**
   (souvent rangé sous « Raspberry Pi OS (other) ») : pas besoin de bureau graphique.
   Corde serveur fonctionne sur Raspberry Pi OS « Bookworm » et plus récent, **64 bits uniquement**.
3. Imager propose de **personnaliser** l'installation (les menus changent selon sa version) : donne un
   **nom d'hôte** (par exemple `corde`), crée ton **utilisateur** avec un mot de passe solide, règle le
   fuseau horaire et le clavier, et **active SSH**. Le Wi-Fi n'est utile que si tu ne branches pas de câble.
4. Écris la carte, mets-la dans le Pi, branche le réseau puis l'alimentation. Attends 2 minutes.
5. Depuis ton PC (Windows 10/11 a SSH intégré : ouvre « Terminal » ou « PowerShell ») :

   ```powershell
   ssh ton-utilisateur@corde.local
   ```

   Si `corde.local` ne répond pas, cherche l'adresse du Pi dans la page « appareils connectés » de ta box et
   utilise-la : `ssh ton-utilisateur@192.168.1.50` (exemple).
6. Une fois connecté au Pi, mets le système à jour :

   ```bash
   sudo apt update && sudo apt full-upgrade -y
   sudo reboot
   ```

**Conseil important** : dans ta box, **réserve une adresse fixe** pour le Pi (souvent appelé « bail statique »
ou « réservation DHCP »). Sinon son adresse peut changer et le PC ne le retrouvera plus.

## 3. Installer Corde serveur

1. Télécharge l'archive **`corde-server-<version>-linux-arm64.tar.gz`** sur la page des versions de Corde
   (Releases → « Corde — dernière version » ou la dernière version numérotée, section *Assets*).
   - Tant que le dépôt est **privé**, le Pi ne peut pas la télécharger tout seul : télécharge-la sur le PC
     (connecté à ton compte GitHub), puis copie-la sur le Pi depuis PowerShell :

     ```powershell
     scp $HOME\Downloads\corde-server-0.7.1-linux-arm64.tar.gz ton-utilisateur@corde.local:~
     ```
   - Un PC Linux (pas un Pi) prend l'archive `…-linux-x64.tar.gz`.
2. Sur le Pi (par SSH) :

   ```bash
   tar -xzf corde-server-0.7.1-linux-arm64.tar.gz
   cd corde-server-0.7.1-linux-arm64
   sudo ./install.sh
   ```

3. Le script crée un compte système `corde` (sans mot de passe ni connexion), installe le programme dans
   `/opt/corde`, garde les données dans `/var/lib/corde`, active le service `corde` (démarré à chaque
   allumage, relancé s'il s'arrête) et affiche à la fin :

   ```text
   Adresse à ouvrir sur ton téléphone ou à indiquer à Corde sur le PC :
     http://192.168.1.50:8471
   Pour appairer un appareil (le PC, puis le téléphone), lance ici :
     sudo corde-server pair
   ```

Le script peut être relancé sans risque : il ne touche jamais aux données.

## 4. Appairer le PC

1. Sur le Pi : `sudo corde-server pair`. Il affiche un code, par exemple `4815 1623`, **valable 10 minutes,
   une seule fois**. Trois codes faux l'annulent (relance alors la commande).
2. Sur le PC, dans Corde : **Réglages** (roue dentée) → section **Serveur Corde** → **Adresse du serveur**
   (`192.168.1.50`, le port 8471 est ajouté tout seul) → **Code d'appairage** → **Appairer**.
   Cette section n'apparaît que sur le PC lui-même (pas depuis le téléphone).
3. L'état passe à **Connecté** et le PC envoie aussitôt tout ce qu'il a déjà enregistré (la première fois,
   cela peut prendre quelques minutes : « En attente d'envoi » décompte). Ensuite il envoie chaque session
   dès qu'elle se termine, au démarrage de Corde ce qui n'était pas parti (serveur éteint, PC hors du
   réseau…), et vérifie toutes les 5 minutes.

Le premier appareil appairé devient **administrateur** du serveur (voir §6). Si tu veux que ce soit ton
téléphone, appaire-le en premier, ou utilise `sudo corde-server pair --admin`.

Dans la section « Serveur Corde » du PC :

| Bouton | Ce qu'il fait |
|---|---|
| **Synchroniser maintenant** | envoie tout de suite ce qui manque au serveur |
| **Tout renvoyer** | renvoie toutes les sessions et tous les résultats (les fichiers déjà présents sur le serveur, reconnus par leur somme de contrôle, ne repartent pas) |
| **Dissocier** | oublie le serveur et demande au serveur de révoquer ce PC ; les données déjà envoyées restent sur le serveur |

États possibles : **Connecté**, **Envoi en cours**, **Injoignable** (serveur éteint, PC sur un autre réseau ou
VPN coupé : Corde réessaie toute seule), **Serveur à mettre à jour** (le serveur est plus ancien que le PC :
§7), **Appareil révoqué** (appaire de nouveau).

## 5. Appairer le téléphone

1. Le téléphone sur le Wi-Fi de la maison, ouvre `http://192.168.1.50:8471` dans le navigateur.
2. La page **Serveur Corde** demande un code : sur le Pi, `sudo corde-server pair`, puis tape le code et un
   nom (« iPhone », « Téléphone »…) → **Appairer cet appareil**.
3. Le navigateur garde une clé dans un cookie protégé. Ajoute la page à l'écran d'accueil pour l'ouvrir
   comme une application.

Sur le serveur, tu retrouves : **Accueil** (dernière synchronisation, appareils, dernières sessions),
**Sessions** (tours, graphes, comparaison), **Ingénieur** (analyse calculée sur le serveur, ressentis),
**Profil** (résultats importés par le PC), **Courses** (planning, choix, rappels, 24 h/24). L'onglet
**Direct** explique que le direct se regarde sur le PC, avec un lien vers lui quand le PC l'a donné.

> **L'appareil est appairé pour une adresse.** Le navigateur range la clé avec l'adresse tapée : si tu ouvres
> le serveur une fois par `http://192.168.1.50:8471` et une autre fois par une autre adresse (nom en
> `.local`, adresse du VPN), il faut appairer pour chacune. Le plus simple : **toujours la même adresse**
> (voir [ACCES-EXTERIEUR.md](ACCES-EXTERIEUR.md) pour garder la même dehors).

### Ce qui marche sur le serveur, et ce qui reste au PC

| Action | Sur le serveur |
|---|---|
| Voir sessions, tours, analyses, profil, résultats | oui, tout appareil appairé |
| Écrire un ressenti (page Ingénieur) | oui : il **redescend vers le PC** à la synchronisation suivante, et ceux du PC remontent. Si tu en écris un sur le PC et un sur le serveur pour la même session avant qu'ils ne se soient synchronisés, les deux sont gardés des deux côtés ; le second dit qu'il a été calculé en parallèle |
| Choisir des courses, rappels | oui, sur le serveur, indépendamment du PC (les choix ne sont **pas** synchronisés entre les deux : sur le PC appairé, la page Courses propose un lien vers celle du serveur) |
| Activer le planning en ligne | appareil **administrateur** seulement |
| Supprimer une session | appareil administrateur ; elle n'est plus renvoyée par le PC (elle reste sur le PC) |
| Recalculer une analyse | appareil administrateur |
| Créer les fichiers de réglage `.svm`, réimporter les résultats, démarrer un enregistrement | **jamais** sur le serveur (le jeu et ses dossiers sont sur le PC) |

Supprimer une session **sur le PC** ne la supprime pas du serveur (le serveur garde tout).

## 6. Les appareils

Page **Appareils** (Accueil du serveur → « Gérer les appareils ») : nom, type, date d'appairage, dernière
utilisation. Chacun peut se renommer ou se dissocier lui-même ; un appareil **administrateur** peut aussi
révoquer les autres et désigner d'autres administrateurs. Une révocation est **immédiate** (même une page
ouverte se ferme). Le dernier administrateur ne peut pas se retirer ses droits (pour ne jamais se retrouver
sans administrateur) ; si cela arrive quand même, `sudo corde-server pair --admin` en fait un nouveau.

Sur le Pi, pour dépanner :

```bash
sudo corde-server devices          # liste (identifiant, nom, administrateur, dates)
sudo corde-server revoke 3f9a1c2e  # révoque l'appareil 3f9a1c2e tout de suite
```

**Téléphone perdu ou volé** : révoque-le (page Appareils depuis un autre appareil, ou `sudo corde-server
revoke`), **et** retire sa clé du VPN ([ACCES-EXTERIEUR.md](ACCES-EXTERIEUR.md#téléphone-perdu-ou-volé)).

## 7. Mettre à jour

Le serveur doit avoir **la même version que Corde sur le PC, ou plus récente**. Si le PC est plus récent,
la section « Serveur Corde » affiche « Serveur à mettre à jour » et rien n'est envoyé (rien n'est perdu :
tout partira après la mise à jour).

1. Copie la nouvelle archive sur le Pi (§3).
2. Sur le Pi :

   ```bash
   tar -xzf corde-server-0.7.1-linux-arm64.tar.gz
   cd corde-server-0.7.1-linux-arm64
   sudo ./install.sh
   ```

Le script remplace le programme et l'interface, garde `/var/lib/corde` (sessions, appareils appairés,
courses) et redémarre le service. Les appareils restent appairés. Il n'y a pas de mise à jour automatique
du serveur.

## 8. Sauvegarder

Tout est dans **`/var/lib/corde`** : la base `corde.db`, les fichiers d'échantillons `sessions/`, les
appareils `hub/`, les courses `races/`. Pour une copie cohérente, arrête le service le temps de l'archive :

```bash
sudo systemctl stop corde
sudo tar -czf ~/corde-sauvegarde-$(date +%F).tar.gz -C /var/lib corde
sudo systemctl start corde
```

Puis récupère-la sur le PC (PowerShell) : `scp ton-utilisateur@corde.local:~/corde-sauvegarde-*.tar.gz .`

Garde cette sauvegarde **privée** : elle contient tes sessions, les noms des autres pilotes des résultats
importés et la liste des appareils (les clés y sont sous une forme hachée, inutilisable telle quelle, mais
inutile de la laisser traîner).

Restaurer (même version de Corde serveur, ou plus récente) :

```bash
sudo systemctl stop corde
sudo rm -rf /var/lib/corde
sudo tar -xzf corde-sauvegarde-AAAA-MM-JJ.tar.gz -C /var/lib
sudo chown -R corde:corde /var/lib/corde
sudo systemctl start corde
```

Le PC, lui, garde ses propres données : en cas de perte du serveur, réinstaller puis appairer le PC
(« Tout renvoyer ») recrée tout, sauf les ressentis saisis seulement sur le serveur, les choix de courses
du serveur et les sessions supprimées du PC.

## 9. Commandes utiles

```bash
sudo systemctl status corde                 # le service tourne-t-il ?
sudo journalctl -u corde -n 100 --no-pager  # son journal (jamais de code ni de clé dedans)
sudo systemctl restart corde                # le redémarrer
sudo corde-server pair                      # un code d'appairage
sudo corde-server devices                   # les appareils appairés
```

**Changer le port** (8471 par défaut) ou **ajouter un nom** sous lequel tu ouvres le serveur (un nom donné
par ton VPN, par exemple) : `sudo systemctl edit corde`, puis écris

```ini
[Service]
ExecStart=
ExecStart=/opt/corde/corde-server --role hub --data-dir /var/lib/corde --ui-dir /opt/corde/ui --port 8471 --allow-host corde.mon-vpn.example
```

et `sudo systemctl restart corde`. Les adresses IP et le nom du Pi (`corde`, `corde.local`) sont acceptés
sans rien faire. Après un changement de port, mets la nouvelle adresse dans les réglages du PC.

## 10. Désinstaller

```bash
sudo /opt/corde/uninstall.sh           # retire le programme et le service, garde /var/lib/corde
sudo /opt/corde/uninstall.sh --purge   # retire aussi les données et le compte « corde »
```

Sur le PC : Réglages → Serveur Corde → **Dissocier**.

## 11. Ce qui protège tes données (résumé)

- Le serveur ne répond qu'aux **appareils appairés** : sans clé valide, toutes les pages de données et le
  flux en direct répondent « non appairé ». Seuls l'écran de code et l'envoi du code sont ouverts.
- Le **code** ne s'obtient que **sur le serveur** (commande `sudo`), sert une fois, expire en 10 minutes,
  meurt après 3 erreurs, et les essais sont limités par appareil et au total.
- Chaque appareil reçoit une **clé de 256 bits** ; le serveur n'en garde qu'une empreinte.
- Le service tourne sous un compte sans droits, enfermé par systemd : il ne peut écrire que dans
  `/var/lib/corde`.
- **Limites** : le trafic est en HTTP simple sur ton réseau local (pas chiffré : quelqu'un qui contrôle ton
  Wi-Fi pourrait le lire) ; un appareil appairé volé garde l'accès **jusqu'à ce que tu le révoques**. Dehors,
  le VPN chiffre tout. Détails : [SECURITY-MODEL.md §7](SECURITY-MODEL.md#7-le-serveur-corde-mode-hub).
