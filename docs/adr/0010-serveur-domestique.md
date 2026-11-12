# ADR 0010 — Serveur domestique : rôle « hub », appairage, synchronisation depuis le PC

- **Statut** : **décidée** — 2026-11-12 (demande du propriétaire : consulter sessions, analyses, profil et
  courses depuis le téléphone **PC éteint**, et depuis l'extérieur par un VPN qu'il monte lui-même ; « être
  sûr que personne à part moi ne puisse accéder à mes informations et à mon réseau privé »). Livrée en 0.7.0.
- **Source** : `docs/SERVEUR.md`, `docs/ACCES-EXTERIEUR.md`, `docs/SECURITY-MODEL.md` §7, `PRIVACY.md`,
  ADR 0006 (stockage local d'abord).

## Contexte

Corde est local d'abord (ADR 0006) : tout vit sur le PC de jeu, le téléphone l'ouvre sur le Wi-Fi quand
l'accès réseau est activé, sans compte (le réseau de la maison est la frontière). Le PC est souvent éteint,
et le pilote veut relire ses sessions ou préparer une course depuis son téléphone, à la maison ou dehors. Il
a un Raspberry Pi 5 toujours allumé. Les crates `corde-store` et `corde-engineer` tournent déjà sous Linux.

## Décision

1. **Un seul programme, deux rôles.** `corde-server --role hub` est le serveur de la maison : pas de source
   de télémétrie, pas d'enregistreur, pas d'import des fichiers du jeu, pas d'appel à l'API du jeu ; écoute
   sur toutes les interfaces ; mêmes pages Sessions / Tour / Ingénieur / Profil / Courses, **calculées sur
   place** à partir des données reçues (l'analyse du serveur est identique à celle du PC, vérifié par test).
   Le rôle par défaut (`desktop`) ne change pas.
2. **Le PC reste la seule source.** Il enregistre comme avant, puis **pousse** au serveur (`hub_link`) :
   au démarrage (retard accumulé), à la fin de chaque enregistrement, toutes les 5 minutes et sur demande.
   Le direct reste servi par le PC.
3. **Protocole de synchronisation idempotent** (`sync_wire`) : le PC demande au serveur ce qu'il a (un
   manifeste : empreintes SHA-256 des paquets reçus, sommes de contrôle des fichiers, identifiants des
   ressentis), puis n'envoie que la différence. Un paquet de session (lignes `sessions`, `laps`, `events`,
   `setups`, `sample_files`, résultat recalé) **remplace** ce que le serveur a ; un fichier Parquet part en
   morceaux de 512 Kio, **reprend** là où une coupure l'a laissé, et n'est validé que si sa somme de contrôle
   correspond ; les paquets sont compressés (zstd) et bornés. Les résultats importés partent avant les
   sessions (qui les citent). Versions vérifiées des deux côtés (`corde-sync-version`, version du schéma) :
   un serveur plus ancien que le PC refuse avec « mets à jour le serveur ».
4. **Ressentis dans les deux sens, sans vérité cachée.** Les ressentis et itérations sont les seules lignes
   que les deux côtés créent. Ils reçoivent un identifiant aléatoire stable (schéma v7 : `uid`) et
   l'itération dont ils partent (`parent_uid`) ; chaque côté ajoute ce qui lui manque, ne supprime ni ne
   modifie rien, puis renumérote les itérations d'un plan par ordre chronologique : les deux bases finissent
   identiques. Deux itérations calculées en parallèle (ressentis saisis des deux côtés entre deux
   synchronisations) sont gardées toutes les deux, et la seconde le **dit** dans ses notes.
5. **Pas de synchronisation des choix de courses** dans ce lot : la page Courses du serveur est autonome
   (planning, choix, rappels 24 h/24) ; sur le PC appairé, un lien y mène. Limite notée dans STATUS.
6. **Authentification obligatoire en mode hub** (`hub::auth`, `hub::hub_guard`) :
   - **appairage par code** : 8 chiffres, usage unique, 10 minutes, obtenu **uniquement sur le serveur**
     (`sudo corde-server pair`, qui parle au service par une route locale protégée par une clé du fichier
     `hub/control.json`, lisible par le compte du service et root seulement) ; trois erreurs l'annulent ;
     5 essais par adresse et 30 au total par tranche de 10 minutes. Le code n'est jamais écrit dans le
     journal (la variante « lire le code dans le journal du service » a été écartée pour cette raison) ;
   - **jeton d'appareil** de 256 bits (générateur du système via `ring`), stocké sous forme de SHA-256 dans
     `hub/devices.json` (mode 0600), comparé en temps constant (`subtle`) ; cookie `HttpOnly`,
     `SameSite=Strict`, `Path=/`, sans `Secure` (HTTP sur le réseau privé, le VPN chiffre dehors) pour un
     navigateur ; `Authorization: Bearer` pour le PC ;
   - toutes les routes `/api/*` et `/ws` exigent un jeton, sauf l'état d'appairage et l'envoi du code ;
     l'interface reste servie et affiche l'écran de code ; une révocation est immédiate, WebSocket compris ;
   - la garde existante (`Host`, `Origin`, tailles) reste devant tout.
7. **Rôle administrateur au lieu de « depuis ce PC ».** Sur le serveur, il n'y a pas de « PC qui fait tourner
   Corde » : les actions réservées au PC deviennent soit **réservées à un appareil administrateur** (le premier
   appairé ; ensuite désignés par un administrateur) — supprimer une session (une pierre tombale empêche le
   PC de la renvoyer), recalculer une analyse, supprimer des dispositions, activer le planning en ligne —
   soit **interdites** — écrire les fichiers de réglage `.svm` (hors du dossier de données, et le jeu n'est
   pas là), réimporter les résultats du jeu, démarrer ou arrêter un enregistrement. Raison : ce qui ne touche
   que les données du serveur peut être confié à l'appareil du propriétaire ; ce qui sortirait du dossier de
   données ou suppose le jeu n'a pas de sens sur le serveur et n'y existe pas.
8. **Jamais exposé à Internet.** Ni redirection du port 8471, ni DMZ, ni UPnP, ni tunnel public : l'accès
   extérieur passe par un VPN privé (WireGuard sur le Pi recommandé, un seul port UDP ; box VPN ; réseau
   maillé en CG-NAT). Documenté pour un non-informaticien (`ACCES-EXTERIEUR.md`).
9. **Paquet Linux** : binaire `corde-server` pour Linux arm64 et x64, lié contre une glibc 2.28 (zig via
   `cargo-zigbuild`) pour ne pas dépendre de la machine de construction ; SQLite embarqué, TLS rustls/ring ;
   archive avec l'interface, une unité systemd durcie (`User=corde`, `ProtectSystem=strict`,
   `ReadWritePaths=/var/lib/corde`…), `install.sh` / `uninstall.sh` idempotents. Construite par la CI (job
   `server-linux`) et jointe à `continuous` et aux versions numérotées. Mise à jour : relancer `install.sh`
   avec la nouvelle archive (les données restent).

## Conséquences

- Un second programme à tenir à jour chez le pilote (le serveur doit être au moins aussi récent que le PC).
- Le schéma passe en v7 (ajouts seulement, sans perte ; une base v7 reste lisible par 0.6).
- Les données personnelles existent en deux endroits (PC et serveur) : sauvegarde et suppression sont
  documentées des deux côtés (`PRIVACY.md`, `SERVEUR.md`). Une session supprimée sur le PC reste sur le
  serveur ; une session supprimée sur le serveur n'y revient pas.
- Pas de HTTPS sur le réseau local : un attaquant qui contrôle le Wi-Fi de la maison peut lire le trafic et
  voler un cookie. Accepté (réseau privé, VPN chiffré dehors), écrit dans le modèle de menace.
- Non vérifié ici : un vrai Raspberry Pi, systemd réel (unité vérifiée par `systemd-analyze verify`, script
  essayé avec un systemctl simulé), WireGuard, la CI elle-même.

## Alternatives écartées

- **Le serveur lit les fichiers du PC par un partage réseau** : PC éteint = rien ; et il faudrait ouvrir le
  PC au réseau.
- **Synchronisation par un service en ligne** : contraire à « tout reste chez moi » (ADR 0006).
- **Comptes et mots de passe** : plus de choses à retenir et à deviner que l'appairage d'un appareil par un
  code lu sur le serveur.
- **HTTPS avec un certificat auto-signé** : avertissements effrayants sur le téléphone, faux sentiment de
  sécurité sans gestion de la confiance ; le VPN chiffre déjà ce qui sort de la maison.
- **Binaire musl statique** : ne pouvait pas être construit ici (cible absente, téléchargement bloqué) ;
  zig + glibc 2.28 donne un binaire qui tourne sur toute distribution récente.
