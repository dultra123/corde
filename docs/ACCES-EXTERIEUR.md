# Ouvrir Corde serveur depuis l'extérieur, sans exposer Corde

> À lire après [SERVEUR.md](SERVEUR.md) (le serveur doit déjà marcher à la maison).
> Ce guide reste **générique** : chaque box et chaque application a ses menus, qui changent d'une version à
> l'autre. Les commandes d'outils tiers (PiVPN, applications WireGuard, Tailscale…) sont données à titre
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

1. **Nom de domaine dynamique.** L'adresse publique de ta box peut changer. Un service de **DNS dynamique**
   donne un nom fixe (par exemple `chez-moi.exemple-dyndns.net`) qui suit ton adresse. Beaucoup de box en
   proposent un dans leurs réglages ; sinon, des services gratuits existent, avec un petit programme sur le
   Pi qui met le nom à jour. PiVPN (étape 2) propose aussi d'utiliser un tel nom.
2. **Installer WireGuard sur le Pi.** Deux façons :
   - **PiVPN**, un script communautaire qui installe et configure WireGuard pas à pas. D'après sa
     documentation (pivpn.io), il se lance par `curl -L https://install.pivpn.io | bash` ; il demande le
     protocole (choisis **WireGuard**), le port (51820 par défaut), l'adresse publique ou le nom de domaine
     dynamique, et les DNS. **Avant de l'utiliser, vérifie sur son site qu'il est toujours maintenu** et
     compatible avec ta version de Raspberry Pi OS ; lis le script avant de le lancer si tu le peux.
   - **À la main**, avec le paquet `wireguard` de Raspberry Pi OS et la documentation officielle de
     WireGuard (wireguard.com) : plus long, mais sans outil tiers.
3. **Rediriger le port UDP** du VPN (51820, ou celui choisi) de la box vers l'adresse **fixe** du Pi
   (menu de la box souvent appelé « redirection de ports », « NAT/PAT » ou « serveurs virtuels »). **Seulement
   celui-là, seulement en UDP.**
4. **Créer un profil par appareil.** Avec PiVPN, d'après sa documentation : `pivpn add` (donne un nom :
   `telephone`), puis `pivpn -qr` affiche un QR code. Un profil = une clé = un appareil : ne partage jamais
   un profil entre deux appareils, pour pouvoir en révoquer un seul.
5. **Sur le téléphone**, installe l'application officielle **WireGuard** (App Store ou Google Play), ajoute
   un tunnel en **scannant le QR code**.
6. **Ne donner au téléphone que l'accès au Pi** (voir ci-dessous).
7. **Essaie** depuis le réseau mobile (Wi-Fi coupé) : active le tunnel, ouvre l'adresse de Corde, vérifie
   que la page s'affiche.

### Ne donner au téléphone que l'accès au Pi

Par défaut, un VPN « maison » donne souvent accès à **tout** le réseau (box, PC, imprimante…) et fait passer
tout le trafic du téléphone par chez toi. Pour Corde, il suffit d'atteindre le Pi. Deux réglages, à faire
**tous les deux** :

1. **Côté téléphone : `AllowedIPs`.** Dans le profil WireGuard du téléphone, la ligne `AllowedIPs` dit quelles
   adresses passent par le tunnel. Mets **seulement l'adresse du Pi**, par exemple :

   ```ini
   [Peer]
   AllowedIPs = 192.168.1.50/32
   ```

   (l'adresse **locale fixe** du Pi, celle de [SERVEUR.md](SERVEUR.md)). En principe, le téléphone ouvre
   alors Corde par **la même adresse** à la maison et dehors (`http://192.168.1.50:8471`) : un seul
   appairage dans Corde. Le reste de son trafic ne passe pas par chez toi. *Non vérifié sur toutes les
   configurations* : si cela ne marche pas, utilise l'adresse du Pi **dans le VPN** (souvent `10.x.x.1`) dans
   `AllowedIPs` et dans le navigateur, et appaire le téléphone aussi pour cette adresse.
2. **Côté Pi : le pare-feu.** `AllowedIPs` se règle sur le téléphone : un voleur qui a le téléphone en main
   peut le modifier. La vraie barrière est sur le Pi : **interdire au tunnel de sortir vers le reste du
   réseau**, et n'y laisser entrer que Corde. Par exemple avec `ufw` (à adapter ; `wg0` est le nom
   habituel de l'interface WireGuard) :

   ```bash
   sudo apt install ufw
   sudo ufw allow from 192.168.1.0/24 to any port 22 proto tcp   # SSH depuis la maison seulement (adapte la plage)
   sudo ufw allow 51820/udp                                      # le VPN lui-même
   sudo ufw allow in on wg0 to any port 8471 proto tcp           # Corde, depuis le tunnel
   sudo ufw route deny in on wg0                                 # rien ne sort du tunnel vers le réseau
   sudo ufw enable
   ```

   **Attention** : avant `ufw enable`, assure-toi que la règle SSH correspond bien à ton réseau, sinon tu
   perds l'accès SSH (il faudra brancher un écran et un clavier). Si d'autres services tournent sur le Pi
   (Pi-hole, partage de fichiers…), ajoute leurs règles. PiVPN active en général le routage pour donner
   accès à tout le réseau : la règle `route deny` le neutralise pour le tunnel.

### Téléphone perdu ou volé

Fais **les deux**, dans cet ordre, depuis le PC ou un autre appareil :

1. **Révoque sa clé WireGuard** sur le Pi (avec PiVPN, d'après sa documentation : `pivpn revoke`, puis
   choisis le profil du téléphone) : le téléphone ne peut plus entrer chez toi.
2. **Révoque l'appareil dans Corde** : page **Appareils** du serveur, ou sur le Pi
   `sudo corde-server devices` puis `sudo corde-server revoke <identifiant>`. Même revenu sur ton Wi-Fi, il
   ne verra plus rien.

Pour un nouveau téléphone : nouveau profil WireGuard, nouvel appairage Corde.

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
