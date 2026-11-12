# Ouvrir Corde serveur depuis l'extérieur, sans exposer Corde

> À lire après [SERVEUR.md](SERVEUR.md) (le serveur doit déjà marcher à la maison).
> Ce guide reste **générique** : chaque box et chaque application a ses menus, qui changent d'une version à
> l'autre. Les commandes d'outils tiers (WireGuard, applications mobiles, Tailscale…) sont données à titre
> indicatif : **vérifie-les dans leur documentation officielle** au moment de les utiliser.

## Le principe : on n'ouvre jamais Corde, on entre chez soi par un VPN

Corde serveur parle en HTTP simple et n'a pas été conçu pour être face à Internet. Le bon schéma est celui
du client VPN d'une entreprise :

```text
  téléphone dehors ──(tunnel VPN chiffré)──▶ box ──▶ Raspberry Pi ──▶ Corde serveur (port 8471)
                                     un seul port UDP, celui du VPN
```

Le téléphone établit d'abord un **tunnel chiffré** vers chez toi ; une fois « dedans », il ouvre Corde comme
s'il était sur le Wi-Fi de la maison. Le port 8471 de Corde **n'est jamais redirigé** par la box : depuis
Internet, il n'existe pas.

Et Corde garde sa propre protection : même à travers le VPN, un appareil doit être **appairé** (code obtenu
sur le serveur) pour voir quoi que ce soit.

## Choisir sa voie

| Ta situation | Voie |
|---|---|
| Ta box te donne une adresse IPv4 publique et sait rediriger un port | **A. WireGuard sur le Pi** (recommandé, tout chez toi) |
| Ta box sait elle-même être serveur VPN WireGuard | **B. Le VPN de la box** |
| Ton opérateur ne te donne pas d'adresse publique (CG-NAT) ou tu ne peux pas toucher à la box | **C. Un réseau maillé** (Tailscale, ZeroTier, NetBird…) |

Pour savoir si tu es derrière un **CG-NAT** : compare l'« adresse IPv4 WAN » affichée dans l'interface de ta
box avec celle que donne un site du type « quelle est mon adresse IP ». Si elles diffèrent, ou si celle de
la box commence par `100.64.` à `100.127.` (plage réservée au CG-NAT), la redirection de port est
impossible : voie C, ou demande à ton opérateur une adresse IPv4 publique (certains le font sur demande).

## A. WireGuard sur le Raspberry Pi (recommandé)

### Pourquoi un port ouvert pour WireGuard n'est pas le même risque qu'ouvrir Corde

Il faut rediriger **un seul port UDP** de la box vers le Pi (51820 par défaut), pour le VPN lui-même. Ce
n'est pas comparable à rediriger le port de Corde :

- **WireGuard ne répond à aucun paquet qui n'est pas authentifié** par une clé qu'il connaît. Un balayage
  de ports depuis Internet ne voit rien : le port paraît fermé.
- L'authentification se fait par **paires de clés** (une par appareil), pas par mot de passe : rien à
  deviner, rien à essayer en boucle.
- Le code de WireGuard est petit, intégré au noyau Linux, et très étudié.
- Le trafic dans le tunnel est **chiffré** : dehors, même sur un Wi-Fi public, personne ne lit ce que
  Corde affiche (Corde lui-même parle en HTTP non chiffré).

Ouvrir directement le port 8471, au contraire, mettrait sur Internet un serveur web non chiffré dont la
seule défense serait l'appairage.

### Étapes

On utilise le paquet `wireguard` fourni par Raspberry Pi OS, sans script tiers. (Le script communautaire
PiVPN, souvent cité, a annoncé l'arrêt de son développement en 2024 : ne pars pas dessus pour une nouvelle
installation.)

Le montage est le plus simple possible : le téléphone ne parle **qu'au Pi lui-même**, par une adresse qui
n'existe que dans le tunnel (`10.8.0.1`). Le Pi ne fait pas suivre le trafic : le reste de ton réseau reste
hors d'atteinte du téléphone.

1. **Nom de domaine dynamique.** L'adresse publique de ta box peut changer. Un service de **DNS dynamique**
   donne un nom fixe (par exemple `chez-moi.exemple-dyndns.net`) qui suit ton adresse. Beaucoup de box en
   proposent un dans leurs réglages.
2. **Installer WireGuard et créer les clés**, sur le Pi :

   ```bash
   sudo apt update && sudo apt install wireguard qrencode
   sudo -i
   cd /etc/wireguard
   umask 077
   wg genkey | tee serveur.key | wg pubkey > serveur.pub
   wg genkey | tee telephone.key | wg pubkey > telephone.pub
   ```

3. **Le réglage du Pi** : crée `/etc/wireguard/wg0.conf` (par exemple avec `nano wg0.conf`), en remplaçant
   les deux lignes entre parenthèses par le contenu des fichiers (`cat serveur.key`, `cat telephone.pub`) :

   ```ini
   [Interface]
   Address = 10.8.0.1/24
   ListenPort = 51820
   PrivateKey = (contenu de serveur.key)

   [Peer]
   # telephone
   PublicKey = (contenu de telephone.pub)
   AllowedIPs = 10.8.0.2/32
   ```

4. **Le réglage du téléphone** : crée `telephone.conf` dans le même dossier :

   ```ini
   [Interface]
   PrivateKey = (contenu de telephone.key)
   Address = 10.8.0.2/32

   [Peer]
   PublicKey = (contenu de serveur.pub)
   Endpoint = chez-moi.exemple-dyndns.net:51820
   AllowedIPs = 10.8.0.1/32
   PersistentKeepalive = 25
   ```

   `AllowedIPs = 10.8.0.1/32` veut dire : seul le trafic vers le Pi passe par le tunnel. Le reste de la
   navigation du téléphone ne passe pas par chez toi.
5. **Démarrer le VPN** (et à chaque démarrage du Pi) :

   ```bash
   systemctl enable --now wg-quick@wg0
   ```

6. **Rediriger le port UDP 51820** de la box vers l'adresse **fixe** du Pi (menu de la box souvent appelé
   « redirection de ports », « NAT/PAT » ou « serveurs virtuels »). **Seulement celui-là, seulement en UDP.**
7. **Sur le téléphone**, installe l'application officielle **WireGuard** (App Store ou Google Play). Sur le
   Pi, affiche le réglage en QR code, scanne-le dans l'application (« ajouter un tunnel » → « depuis un QR
   code »), puis **efface du Pi** la clé privée du téléphone : elle ne doit exister que sur le téléphone.

   ```bash
   qrencode -t ansiutf8 < telephone.conf
   shred -u telephone.conf telephone.key
   exit
   ```

8. **Essaie** depuis le réseau mobile (Wi-Fi coupé) : active le tunnel, ouvre `http://10.8.0.1:8471`,
   appaire le téléphone ([SERVEUR.md](SERVEUR.md)).

Un appareil = une paire de clés = un bloc `[Peer]` avec sa propre adresse (`10.8.0.3`, `10.8.0.4`…). Ne
partage jamais un réglage entre deux appareils : tu ne pourrais plus en révoquer un seul.

**Deux adresses, deux appairages.** Dans Corde, l'appairage vaut pour une adresse. Dehors, le téléphone
utilise `http://10.8.0.1:8471` ; à la maison sans VPN, l'adresse locale du Pi. Appaire le téléphone pour
chacune des deux, ou garde le tunnel actif aussi à la maison si ta box le permet. L'application WireGuard
sait s'activer toute seule hors de ton Wi-Fi (réglage « activation à la demande » sur iPhone).

### Ne donner au téléphone que l'accès au Pi

Avec le montage ci-dessus, c'est déjà le cas, pour deux raisons indépendantes :

1. **Le Pi ne fait pas suivre le trafic.** Tant que le routage est désactivé, ce qui arrive par le tunnel
   s'arrête au Pi. Vérifie-le :

   ```bash
   sysctl net.ipv4.ip_forward     # doit répondre : net.ipv4.ip_forward = 0
   ```

   Certains logiciels l'activent en s'installant (Docker, par exemple). Si la réponse est `1`, le pare-feu
   ci-dessous devient indispensable.
2. **Un pare-feu sur le Pi**, qui ne laisse entrer que ce qu'il faut. `AllowedIPs` se règle sur le
   téléphone : un voleur qui a le téléphone en main peut le modifier ; la vraie barrière est sur le Pi.
   Exemple avec `ufw`, **à adapter à ta plage d'adresses** (`wg0` est l'interface WireGuard) :

   ```bash
   sudo apt install ufw
   sudo ufw allow from 192.168.1.0/24 to any port 22 proto tcp     # SSH, depuis la maison seulement
   sudo ufw allow from 192.168.1.0/24 to any port 8471 proto tcp   # Corde, depuis la maison
   sudo ufw allow in on wg0 to any port 8471 proto tcp             # Corde, depuis le tunnel
   sudo ufw allow 51820/udp                                        # le VPN lui-même
   sudo ufw enable
   ```

   Par défaut, `ufw` refuse tout le reste en entrée et refuse de faire suivre le trafic.

   **Attention** : avant `ufw enable`, assure-toi que la règle SSH correspond bien à ton réseau, sinon tu
   perds l'accès SSH (il faudra brancher un écran et un clavier). Si d'autres services tournent sur le Pi
   (Pi-hole, partage de fichiers…), ajoute leurs règles.

### Téléphone perdu ou volé

Fais **les deux**, dans cet ordre, depuis le PC ou un autre appareil :

1. **Retire sa clé WireGuard** sur le Pi : supprime le bloc `[Peer]` du téléphone dans
   `/etc/wireguard/wg0.conf`, puis `sudo systemctl restart wg-quick@wg0`. Le téléphone ne peut plus entrer
   chez toi.
2. **Révoque l'appareil dans Corde** : page **Appareils** du serveur, ou sur le Pi
   `sudo corde-server devices` puis `sudo corde-server revoke <identifiant>`. Même revenu sur ton Wi-Fi, il
   ne verra plus rien.

Pour un nouveau téléphone : nouvelles clés, nouveau bloc `[Peer]`, nouvel appairage Corde.

## B. La box fait elle-même serveur VPN WireGuard

Certaines box proposent un **serveur VPN WireGuard intégré** : la box gère le port et les clés, et donne un
fichier ou un QR code à importer dans l'application WireGuard du téléphone. C'est simple, mais :

- le tunnel donne généralement accès à **tout le réseau** de la maison ; restreins `AllowedIPs` du
  téléphone à l'adresse du Pi (comme en A), et si la box permet de limiter ce qu'un client VPN peut joindre,
  fais-le ;
- tu dépends des mises à jour de sécurité de la box ;
- un profil par appareil, révocable séparément, comme en A ; en cas de perte : révoque le profil dans la
  box **et** l'appareil dans Corde.

## C. Pas d'adresse publique (CG-NAT) : un réseau maillé

Sans adresse publique, aucune redirection n'est possible. Des services comme **Tailscale**, **ZeroTier** ou
**NetBird** créent un réseau privé entre tes appareils **sans aucun port ouvert** : chaque appareil (le Pi,
le téléphone) installe leur application et se connecte à ton compte.

Ce que cela implique :

- **Un tiers coordonne les connexions** : son serveur connaît tes appareils, leurs adresses et quand ils se
  connectent. Le contenu reste chiffré de bout en bout dans les cas habituels (Tailscale et NetBird
  utilisent WireGuard), et quand une connexion directe est impossible, le trafic peut passer par un relais
  du service, toujours chiffré. Lis leur politique de confidentialité ; sécurise ton compte (double
  authentification) : **qui contrôle ton compte peut ajouter un appareil à ton réseau**.
- Restreins l'accès avec leurs règles (« ACL », « politiques d'accès ») pour que le téléphone n'atteigne que
  le Pi, port 8471.
- L'adresse du Pi dans ce réseau (souvent en `100.x.y.z`) ou son nom dans ce réseau s'ouvre comme d'habitude
  (`http://100.x.y.z:8471`). Une adresse IP est acceptée telle quelle par Corde ; pour un **nom** (par
  exemple `corde.ton-reseau.example`), ajoute-le au service avec `--allow-host` ([SERVEUR.md §9](SERVEUR.md#9-commandes-utiles)).
  Appaire le téléphone pour cette adresse.
- Certains de ces services permettent d'héberger soi-même le serveur de coordination (par exemple
  Headscale pour Tailscale), mais il faut alors… un serveur joignable depuis Internet.
- En cas de perte : retire l'appareil de ton réseau dans leur console **et** révoque-le dans Corde.

## Ce qu'il ne faut jamais faire

- **Rediriger le port 8471** (ou n'importe quel port de Corde) de la box vers le Pi ou le PC.
- Mettre le Pi (ou le PC) en **DMZ** dans la box : tous ses ports deviendraient joignables depuis Internet.
- Laisser l'**UPnP** activé dans la box : un programme pourrait ouvrir un port tout seul. Désactive-le si
  tu n'en as pas besoin.
- Publier Corde par un **tunnel public** (service de « partage d'un serveur local », lien public) sans
  authentification forte devant : c'est une redirection de port déguisée.
- Partager un **profil VPN** entre plusieurs appareils, ou l'envoyer par message.
- Donner un **code d'appairage** à quelqu'un : il donne accès à toutes tes données.

## Liste de vérification finale

- [ ] Depuis le réseau mobile, **sans** VPN, `http://<ton adresse publique>:8471` **ne répond pas**.
- [ ] Dans la box : **aucune** redirection vers le port 8471, **pas** de DMZ, UPnP **désactivé**.
- [ ] La seule redirection est le port **UDP** du VPN (voie A), vers l'adresse fixe du Pi.
- [ ] Avec le VPN actif, Corde s'ouvre, demande un code la première fois, puis affiche tes sessions.
- [ ] Avec le VPN actif, le téléphone **n'atteint pas** la page de la box ni le PC (seulement le Pi).
- [ ] Chaque appareil a **son propre** profil VPN et **son propre** appairage Corde.
- [ ] La page **Appareils** du serveur ne liste que tes appareils.
- [ ] Tu sais révoquer un téléphone perdu **des deux côtés** (VPN et Corde).
- [ ] Raspberry Pi OS est à jour (`sudo apt update && sudo apt full-upgrade`), et le compte du Pi a un mot de
      passe solide (ou SSH par clé seulement).
