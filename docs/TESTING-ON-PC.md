# Installer Corde sur ton PC (Windows)

> Guide pour le pilote. Compte 5 minutes. Corde est maintenant une vraie application : une icône, une
> fenêtre, plus de console ni de navigateur. Rien de ce que tu fais ici ne modifie le jeu ni tes réglages :
> Corde se contente de **lire** ce que Le Mans Ultimate publie.
> Si quelque chose ne correspond pas à ce qui est écrit, fais une capture d'écran et envoie-la nous avec le
> fichier journal (§ 8) : c'est exactement ce dont on a besoin.

## En bref

1. Activer les « plugins » dans LMU (une case, une fois pour toutes).
2. Télécharger l'installeur sur la page **Corde — dernière version**.
3. Le lancer, passer l'avertissement de Windows (expliqué plus bas), installer.
4. Ouvrir **Corde** depuis le menu Démarrer, puis lancer le jeu.

---

## 1. Activer les plugins dans Le Mans Ultimate

Depuis la version 1.2 (décembre 2025), le jeu publie lui-même sa télémétrie, **à condition** que l'option
suivante soit activée :

1. Lance LMU.
2. `Settings` (Paramètres) → `Gameplay` → **`Enable Plugins`** : mets-le sur **ON**.
3. **Quitte complètement le jeu et relance-le.** Sans ce redémarrage, rien n'est publié.

Le fichier `rFactor2SharedMemoryMapPlugin64.dll` déjà présent dans ton dossier `plugins\` vient d'un autre
outil (SimHub, Crew Chief…). Il est inoffensif et Corde n'en a pas besoin : tu peux le laisser.
L'option `Automatically Record Telemetry` peut rester désactivée : Corde enregistre elle-même.

---

## 2. Télécharger l'installeur

1. Ouvre la page (tant que le dépôt est privé, connecte-toi d'abord à GitHub)
   **<https://github.com/dultra123/corde/releases/tag/continuous>** — elle s'appelle
   « Corde — dernière version » et elle est reconstruite à chaque modification du projet.
2. Tout en bas, section **Assets**, clique sur le fichier dont le nom finit par **`-setup.exe`**
   (par exemple `Corde_0.3.0_x64-setup.exe`).
3. Ton navigateur peut hésiter : Edge affiche parfois « … n'est pas couramment téléchargé ». Clique sur
   les **…** à côté du fichier → **Conserver** → **Conserver quand même**.

L'autre fichier, `corde-portable-windows.zip`, est l'ancien mode console : tu n'en as pas besoin (voir
l'annexe).

---

## 3. Installer

1. Double-clique sur le fichier téléchargé.
2. **Windows affiche un écran bleu « Windows a protégé votre ordinateur ».** Clique sur
   **Informations complémentaires**, puis sur le bouton **Exécuter quand même** qui apparaît.
3. Suis l'installeur (Suivant → Installer → Terminer). Il ne demande **pas** de droits administrateur.

**Pourquoi cet avertissement, honnêtement.** Windows (SmartScreen) affiche cet écran pour tout programme
téléchargé dont il ne connaît pas l'éditeur. Corde n'est pas encore *signé numériquement* — une signature
s'achète auprès d'un organisme, ce n'est pas décidé pour l'instant. Ce n'est **pas** une détection de virus,
et Corde ne fait rien de plus que ce qui est décrit ici. L'avertissement reviendra **à chaque nouvel
installeur** que tu téléchargeras, tant que Corde n'est pas signé ; une fois installé, Corde se lance sans
avertissement. Les solutions et leur prix sont détaillés dans
[l'ADR 0007](adr/0007-windows-smartscreen-signature.md).

Ce que fait l'installeur :

| Quoi | Où |
|---|---|
| Le programme | `%LOCALAPPDATA%\Programs\Corde\` (c'est-à-dire `C:\Users\<toi>\AppData\Local\Programs\Corde\`) |
| Un raccourci **Corde** | menu Démarrer (et le bureau si tu coches la case à la fin) |
| Le moteur d'affichage WebView2 de Microsoft | déjà présent sur Windows 11 ; téléchargé automatiquement sinon |

Il ne touche ni au jeu, ni à tes données Corde existantes (§ 7).

---

## 4. Premier lancement

1. Menu Démarrer → **Corde**. Une fenêtre sombre « Démarrage de Corde… » apparaît une seconde ou deux,
   puis l'**écran d'accueil**.
2. Windows ne pose **aucune question de pare-feu** : par défaut, Corde n'est joignable que depuis ton PC.
3. Lance LMU. Quand tu **entres en piste**, le tableau de bord s'affiche tout seul. Si la fenêtre de Corde
   était fermée ou réduite, elle réapparaît à ce moment-là, là où tu l'avais laissée (ton second écran par
   exemple) ; dans tous les cas Corde ne passe jamais devant le jeu et ne lui prend pas le clavier.

### Fermer la fenêtre ne quitte pas Corde

Quand tu fermes la fenêtre (la croix), Corde **reste active** : elle continue d'attendre le jeu et
d'enregistrer tes sessions. Son icône — un **M** orange — est dans la zone de notification, près de
l'horloge (Windows la range souvent derrière la petite flèche **^** : tu peux la faire glisser à côté de
l'horloge pour la garder visible). La première fois, une notification te le rappelle.

| Geste sur l'icône | Effet |
|---|---|
| Clic gauche | rouvre la fenêtre |
| Clic droit → **Ouvrir Corde** | rouvre la fenêtre |
| Clic droit → **Dashboard** | rouvre la fenêtre sur le tableau de bord |
| Clic droit → **Quitter** | arrête vraiment Corde (l'enregistrement en cours est terminé proprement) |

Laisser le pointeur sur l'icône indique où en est Corde : « en attente du jeu », « jeu détecté »,
« en piste ».

Relancer Corde depuis le menu Démarrer alors qu'elle tourne déjà rouvre simplement sa fenêtre : il n'y a
jamais deux Corde en même temps.

Sur la page **Ingénieur** d'une session, « Ton ressenti » est interprété **hors-ligne** (dictionnaire
intégré, rien n'est envoyé sur Internet) : tu vois les symptômes compris et le plan ajusté en 1 à 3
changements. Un modèle de langage est possible en option, expérimental, en définissant la variable
d'environnement `CORDE_ANTHROPIC_API_KEY` (clé d'API Anthropic ; en mode console, `--llm none` le
coupe même si la variable existe) ; sans réponse en 8 s, Corde repasse sur le dictionnaire.

---

## 5. Démarrer Corde avec Windows (facultatif)

Dans les **Réglages** de Corde, active « Démarrer avec Windows ». Au prochain démarrage du PC, Corde se
lance **sans fenêtre**, seulement l'icône près de l'horloge, et la fenêtre apparaît quand tu entres en
piste (ou quand tu cliques sur l'icône).

Pour l'arrêter : le même réglage, ou `Gestionnaire des tâches` → `Applications de démarrage` → Corde →
Désactiver.

---

## 6. Le tableau de bord sur ton téléphone (facultatif)

Par défaut c'est désactivé, pour que Windows ne te demande rien. Pour l'activer :

1. Dans les **Réglages** de Corde, active l'accès depuis le réseau local (téléphone), puis **redémarre
   Corde** quand il le propose.
2. Windows affiche alors, une seule fois, « Le Pare-feu Windows Defender a bloqué certaines
   fonctionnalités de cette application ». Coche **Réseaux privés** et clique **Autoriser l'accès**
   (Windows demande une confirmation administrateur). Si tu cliques sur Annuler, Corde marche toujours sur
   le PC mais le téléphone ne pourra pas se connecter (dépannage, point 4).
3. L'écran d'accueil de Corde affiche maintenant, dans le bloc **Sur ton téléphone**, l'adresse à ouvrir
   et un **QR code**. Ils sont **floutés par défaut** (pratique en direct ou pour une capture d'écran) :
   clique **Afficher l'adresse** pour les voir, **Masquer l'adresse** pour les cacher à nouveau. Ils
   redeviennent masqués à chaque retour sur l'accueil.
4. Sur le téléphone, **connecté au même Wi-Fi**, scanne le QR code avec l'appareil photo (ou tape
   l'adresse affichée dans le navigateur, par exemple `http://192.168.1.20:8471` ; c'est bien `http`,
   pas `https`).

---

## 7. Où sont tes données

Tout ce que Corde enregistre est dans **`%LOCALAPPDATA%\Corde\`**
(`C:\Users\<toi>\AppData\Local\Corde\`). Si tu avais l'ancienne version (avant 0.3.0, logo « M »), Corde
y reprend son dossier au premier lancement, avec toutes tes sessions (voir § 9) :

| Fichier ou dossier | Contenu |
|---|---|
| `corde.db` | sessions, tours, résultats importés, analyses, ressentis |
| `sessions\<date-circuit-type>\` | la télémétrie de chaque session (≈ 60 Mo par heure de roulage) |
| `settings.json` | tes réglages |
| `logs\corde-desktop.log` | le **journal** de l'application |

Rien n'est envoyé sur Internet. Corde n'écrit dans le dossier du jeu que lorsque tu le lui demandes
(paragraphe suivant).

### Tes fichiers de réglage (`.svm`)

Corde lit les réglages que tu as sauvegardés dans le garage du jeu
(`…\Le Mans Ultimate\UserData\player\Settings\<circuit>\*.svm`). Sur la page **Ingénieur** d'une session,
le bloc **« Ton réglage »** montre celui qu'il pense que tu as utilisé (le dernier enregistré pour cette voiture
dans le dossier du circuit avant la fin de la session) ; tu peux en choisir un autre dans la liste. Chaque
changement proposé s'affiche alors **avant → après** (par exemple `P2 → P1`), et les réglages que ta voiture
ne permet pas sont écartés avec la raison. Coche les plans voulus et clique **« Créer les fichiers de
réglage »** : Corde écrit `Corde_Sur_<ton fichier>.svm` (ou `_Agressif`, `_Pluie`, `_Iteration2_…`) **à côté de
ton fichier**, sans jamais en écraser un (il ajoute `_2`, `_3`…). Dans le jeu : **garage → Setups → rafraîchir
la liste → charger `Corde_…`**. Si aucun réglage n'apparaît, sauvegarde d'abord une fois ton réglage dans le
garage avec Corde ouverte. Quand tu passes au garage ou au stand, Corde lit aussi (en lecture seule, une fois
par voiture et par session) ce que le garage du jeu dit des réglages et le garde dans
`%LOCALAPPDATA%\Corde\diagnostics\lmu-api\garage\` : **envoie-nous ce dossier** (avec un de tes `.svm`), il
nous permettra de lire les vraies bornes de chaque réglage.

---

## 8. Quand quelque chose ne va pas : le journal

Corde n'a plus de fenêtre noire ; tout ce qu'il aurait écrit dedans va dans le journal.

1. Appuie sur `Windows + R`, tape `%LOCALAPPDATA%\Corde\logs` et valide.
2. Envoie-nous le fichier **`corde-desktop.log`** (et `corde-desktop.old.log` s'il existe), avec une
   capture d'écran et l'heure approximative du problème.

Si Corde n'arrive pas à démarrer, sa fenêtre affiche elle-même l'erreur, le chemin du journal et un bouton
**Réessayer**.

### Dépannage

**1. « Un autre Corde tourne déjà sur ce PC ».** Une ancienne fenêtre noire `corde-server.exe` est restée
ouverte : ferme-la, puis clique sur Réessayer.

**2. Le jeu n'est pas détecté** (l'icône reste sur « en attente du jeu »). Vérifie `Enable Plugins` = ON et
que tu as bien quitté puis relancé le jeu. Si le jeu est lancé « en tant qu'administrateur », Corde ne peut
pas le lire : lance le jeu normalement.

**3. La fenêtre ne s'affiche pas quand j'entre en piste.** Envoie-nous le journal : c'est le comportement
qu'on n'a pas pu essayer sans ton PC. En attendant, un clic sur l'icône près de l'horloge l'ouvre.

**4. Le téléphone ne se connecte pas.** Même Wi-Fi (pas de 4G, pas de réseau « invité ») ? Ensuite :
`Sécurité Windows` → `Pare-feu et protection du réseau` → `Autoriser une application via le pare-feu` →
**Corde** coché pour **Privé**. Si ton Wi-Fi est classé « Public », passe-le en « Privé » (`Wi-Fi` → ton
réseau → `Type de profil réseau`). Si le port 8471 était pris au démarrage, Corde en utilise un autre (8472
à 8480) : il est écrit dans le journal, à la ligne « Local : http://127.0.0.1:… ».

**5. L'antivirus bloque Corde.** Note le nom exact de l'alerte et envoie-le nous ; ne désactive pas ton
antivirus.

**6. Pour le tout premier test, ferme les autres outils de télémétrie** (SimHub, Crew Chief, TinyPedal,
RaceLab…). Corde est prévue pour cohabiter, mais on veut d'abord une mesure propre.

**7. Et l'anti-triche (EAC) ?** Corde lit uniquement ce que le jeu publie volontairement (le même
mécanisme que SimHub ou TinyPedal) et ne touche pas au processus du jeu. Par prudence, fais les premiers
tests hors ligne, et dis-nous avant de l'utiliser en session en ligne.

---

## 9. Mettre à jour

Il n'y a pas encore de mise à jour automatique. Pour passer à la dernière version :

1. Quitte Corde (clic droit sur l'icône → **Quitter**).
2. Télécharge le nouvel installeur sur la même page (§ 2) et lance-le **par-dessus** l'ancien, sans
   désinstaller. L'avertissement de Windows réapparaît (§ 3) : c'est normal.
3. Relance Corde. Tes sessions, tes dispositions et tes réglages sont conservés.

La version installée est indiquée dans les Réglages de Corde.

### Passer de l'ancienne version (avant 0.3.0, logo « M ») à Corde

L'application a changé de nom et de logo en version 0.3.0. Corde s'installe **à côté** de l'ancienne
version (pas par-dessus) et reprend toutes tes données au premier lancement : sessions, base, dispositions
du tableau de bord, réglages, fichiers de diagnostic.

1. **Quitte l'ancienne version** : clic droit sur son icône « M » près de l'horloge → **Quitter**. (Si tu
   oublies, rien n'est perdu, voir le point 4.)
2. Télécharge et installe Corde (§ 2 et § 3).
3. Lance **Corde** depuis le menu Démarrer. Avant d'ouvrir quoi que ce soit, elle renomme ton ancien dossier
   de données en `%LOCALAPPDATA%\Corde` (un simple renommage : rien n'est copié ni effacé). Ouvre
   **Sessions** : tes sessions sont là.
4. Si l'ancienne version tournait encore, Corde affiche « Une autre copie tourne déjà sur ce PC » : quitte
   l'ancienne (point 1), puis clique sur **Réessayer**. Corde ne démarre jamais sur un dossier vide : si elle
   n'a pas pu renommer l'ancien dossier, elle s'en sert tel quel et réessaie au lancement suivant.
5. Si tu avais coché « Lancer au démarrage de Windows », coche-le à nouveau dans les Réglages de Corde.
6. Quand tu as vérifié tes sessions dans Corde, **désinstalle l'ancienne version** : Paramètres Windows →
   Applications → Applications installées → l'ancienne application → Désinstaller. Ses fichiers de
   programme et son lancement automatique sont retirés ; tes données, déjà reprises par Corde, ne sont pas
   touchées.

En cas de doute, le journal `%LOCALAPPDATA%\Corde\logs\corde-desktop.log` dit ce qui a été repris (lignes
« data folder of the former name… » et « database of the former name… »).

---

## 10. Désinstaller

`Paramètres Windows` → `Applications` → `Applications installées` → **Corde** → `Désinstaller`.

La désinstallation enlève le programme, le raccourci et le démarrage avec Windows. Elle **garde tes
données** : pour tout effacer, supprime ensuite le dossier `%LOCALAPPDATA%\Corde\`. (La case « Supprimer
les données de l'application » du désinstalleur ne concerne que le cache d'affichage, pas tes sessions.)

---

## Annexe — le mode console (`corde-server.exe`)

L'ancien fonctionnement existe toujours, pour le diagnostic ou pour utiliser Corde sans l'installer : c'est
le fichier `corde-portable-windows.zip` de la même page. **Ne le lance pas en même temps que
l'application** (les deux liraient le jeu et écriraient dans les mêmes fichiers ; l'application refuse
d'ailleurs de démarrer si elle le voit).

1. Extrais le zip dans un dossier simple, par exemple `C:\Corde-console\`.
2. Dans ce dossier, tape `cmd` dans la barre d'adresse de l'Explorateur puis Entrée.
3. Utilise les outils :

| Commande | À quoi elle sert |
|---|---|
| `lmu-probe.exe` | vérifie que Corde voit le jeu : version, circuit, voiture, vitesse (deux fois par seconde ; `Ctrl+C` pour arrêter) |
| `lmu-probe.exe --once --dump frame.json` | écrit un instantané complet de ce que le jeu publie — le fichier le plus utile à nous envoyer quand une donnée semble fausse |
| `corde-server.exe` | le serveur seul ; ouvre ensuite `http://127.0.0.1:8471` dans un navigateur. Comme l'application, il n'est joignable que depuis ce PC par défaut ; avec `--lan` il écoute sur le réseau local et Windows pose la question du pare-feu (§ 6) |
| `corde-server.exe --source sim` | le même avec une course fictive, sans le jeu |
| `corde-store.exe sessions list` | liste les sessions enregistrées |

`--help` après chaque commande liste ses options. `Ctrl+C` dans la fenêtre noire arrête le programme.
