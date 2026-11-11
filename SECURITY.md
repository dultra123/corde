# Sécurité

## Signaler une faille

**Ne signale pas une faille dans un ticket public.** Utilise le signalement privé de GitHub :

1. ouvre l'onglet **Security** du dépôt ([dultra123/corde](https://github.com/dultra123/corde/security)) ;
2. clique sur **Report a vulnerability** ;
3. décris le problème : la version de Corde (affichée dans les réglages de l'application, ou dans le nom de
   l'installeur `Corde_<version>_x64-setup.exe`), ce qu'il faut faire pour le reproduire, ce qu'un
   attaquant peut obtenir. Une preuve de concept minimale aide beaucoup.

Ne joins pas de fichier de résultats du jeu tel quel (il contient les noms d'autres pilotes) ni de journal
sans l'avoir relu (il peut contenir des chemins de ton PC).

Corde est un projet tenu sur le temps libre : il n'y a pas de délai de réponse garanti ni de prime. Chaque
signalement sera lu ; une faille confirmée est corrigée en priorité, puis rendue publique (avec ton nom ou
ton pseudonyme si tu le souhaites) une fois la correction disponible.

## Versions suivies

Seule la **dernière version** reçoit des corrections de sécurité : la pré-release
[« Corde — dernière version »](https://github.com/dultra123/corde/releases/tag/continuous), reconstruite à
chaque modification, et la dernière version étiquetée s'il y en a une. Mets Corde à jour avant de signaler
un problème.

## Ce que Corde protège, et ce qu'il ne protège pas

Corde n'a ni compte ni mot de passe : son serveur local n'écoute que sur ton PC par défaut, et l'accès
depuis le téléphone (réseau local) s'active à la demande. Le détail — qui peut appeler quoi, les contrôles
appliqués, ce qui reste exposé quand l'accès réseau est activé — est dans
[docs/SECURITY-MODEL.md](docs/SECURITY-MODEL.md). Ce que Corde lit et garde : [PRIVACY.md](PRIVACY.md).

Hors sujet : une machine du même Wi-Fi qui lit les données quand l'accès réseau est activé (c'est le
fonctionnement prévu, voir le modèle), et un programme malveillant déjà installé sur le PC.

---

## English summary

**Please do not report security issues in public issues.** Use GitHub's private reporting: the **Security**
tab of [dultra123/corde](https://github.com/dultra123/corde/security) → **Report a vulnerability**. Include
the Corde version, steps to reproduce and the impact; do not attach raw game results files (they contain
other drivers' names) or unreviewed logs (they may contain local paths). Only the **latest version** (the
`continuous` pre-release and the latest tagged release, if any) receives security fixes. This is a
spare-time project: no guaranteed response time and no bounty. The threat model (local server, no accounts,
LAN access off by default) is described in French in [docs/SECURITY-MODEL.md](docs/SECURITY-MODEL.md);
privacy in [PRIVACY.md](PRIVACY.md).
