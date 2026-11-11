# ADR 0007 — « Windows ne bloque pas Corde » : pare-feu, installeur par utilisateur, SmartScreen et signature de code

- **Statut** : points 1 et 2 **acceptés et appliqués** (2026-11-10) ; point 3 (signature de code) **à décider par
  le propriétaire** — voir « Ce que le propriétaire doit décider »
- **Source** : retour du pilote après ses premiers tests (« il faut faire en sorte que Windows ne bloque pas
  l'application ») ; documentation Microsoft, Tauri et autorités de certification consultée le 10 novembre
  2026 (liens en bas de page)

## Contexte

Le pilote a vu Windows bloquer Corde au moins une fois. Sous Windows, trois mécanismes distincts peuvent
donner cette impression ; ils n'ont ni la même cause ni le même remède.

| Mécanisme | Ce que l'utilisateur voit | Déclencheur |
|---|---|---|
| **Pare-feu Windows Defender** | « Le Pare-feu Windows Defender a bloqué certaines fonctionnalités de cette application » | un programme se met à écouter sur le réseau (pas seulement sur la boucle locale) sans règle existante |
| **Microsoft Defender SmartScreen** | écran bleu « Windows a protégé votre ordinateur », bouton « Ne pas exécuter » | premier lancement d'un exécutable **téléchargé**, non signé ou d'éditeur encore inconnu |
| **Avertissement du navigateur** | Edge / Chrome : « … n'est pas couramment téléchargé » | même base de réputation que SmartScreen, au moment du téléchargement |

Avec le zip de la phase 0, `corde-server.exe` écoutait sur `0.0.0.0` (toutes les interfaces) : le pare-feu
posait donc sa question au premier lancement, et l'exécutable, non signé et téléchargé, passait par
SmartScreen. Les deux ont probablement été vus.

## Décision

### 1. Pare-feu : écouter sur la boucle locale par défaut (appliqué)

L'application de bureau démarre le serveur sur **`127.0.0.1`** tant que le réglage `lan_access` de
`settings.json` n'est pas à `true` (`apps/desktop/src-tauri/src/settings.rs`, fonction `resolve_bind_ip`).
Le trafic de boucle locale n'est pas filtré par le pare-feu : **aucune invite**, et Corde fonctionne
entièrement sur le PC (fenêtre de l'application, source navigateur d'OBS sur le même PC).

Quand l'utilisateur active l'accès depuis le téléphone (`lan_access = true`, puis redémarrage de Corde), le
serveur écoute sur `0.0.0.0` et Windows affiche **une fois** son invite pour `Corde.exe` :

- il faut cocher **Réseaux privés** et cliquer **Autoriser l'accès** ; créer la règle demande les droits
  administrateur (invite UAC) ;
- si l'utilisateur annule, ou n'est pas administrateur, Windows crée des règles de **blocage** : Corde continue
  de fonctionner sur le PC, mais le téléphone ne peut pas se connecter, et l'invite ne revient pas tant que ces
  règles existent (`Sécurité Windows → Pare-feu → Autoriser une application via le pare-feu`) ;
- la règle est liée au chemin de l'exécutable (`%LOCALAPPDATA%\Programs\Corde\Corde.exe`), qui ne change pas lors d'une
  mise à jour ;
- le Wi-Fi doit être classé « réseau privé » dans Windows, sinon la règle « privé » ne s'applique pas.

Ce qu'on ne fait pas : créer la règle depuis l'installeur. Microsoft le recommande pour les applications
réseau, mais cela exige un installeur avec droits administrateur, ce qui contredit le point 2, pour un réglage
que la plupart des utilisateurs n'activeront pas.

### 2. Installeur par utilisateur, métadonnées complètes, pas de console (appliqué)

- Installeur **NSIS en mode `currentUser`** : installation dans `%LOCALAPPDATA%\Programs\Corde` (les données restent dans `%LOCALAPPDATA%\Corde`, voir `windows/hooks.nsh`), **aucun droit
  administrateur**, aucune invite UAC, entrée de désinstallation dans « Applications installées », raccourci
  dans le menu Démarrer.
- Métadonnées renseignées dans l'exécutable et l'installeur : nom du produit (Corde), éditeur (`dultra__`),
  version, description, icône. Elles ne suppriment pas l'avertissement SmartScreen (seule une signature le
  peut), mais un exécutable anonyme est plus suspect pour les antivirus et pour l'utilisateur.
- Application **sans console** (`windows_subsystem = "windows"`), journal dans
  `%LOCALAPPDATA%\Corde\logs\corde-desktop.log`.
- Démarrage avec Windows par la clé `HKCU\…\Run` (aucun service, aucune tâche planifiée, aucun droit
  administrateur), désactivé par défaut.
- WebView2 : installé par le programme d'amorçage de Microsoft si absent (il est préinstallé sur Windows 11).

### 3. SmartScreen : ce qui reste, et ce que ça coûte de le faire disparaître (à décider)

**Le constat, sans détour.** Sans certificat de signature de code, Windows affiche « Windows a protégé votre
ordinateur » au premier lancement de l'installeur téléchargé. L'utilisateur doit cliquer **Informations
complémentaires → Exécuter quand même**. D'après Microsoft, « un fichier non signé doit reconstruire sa
réputation à chaque nouvelle version, en repartant de zéro » : avec une version continue reconstruite à
chaque modification, **l'avertissement revient à chaque nouvel installeur téléchargé**. Une fois installé,
Corde se lance sans avertissement (les fichiers écrits par l'installeur ne portent pas la marque « téléchargé
depuis Internet »). Aucune astuce gratuite ne change cela ; ce n'est pas une détection de virus.

**Signer ne suffit pas tout de suite.** Depuis 2024, plus aucun certificat ne donne la confiance immédiate :
« les certificats EV ne contournent plus SmartScreen ». Un fichier signé affiche le nom vérifié de l'éditeur,
et la réputation se construit ensuite par éditeur — « il n'y a pas de seuil exact, mais cela peut prendre
plusieurs semaines et des centaines d'installations sans incident ». L'avantage décisif de la signature est
que cette réputation **se transmet aux versions suivantes** signées avec la même identité.

**Options (novembre 2026).**

| Option | Coût | Qui peut l'obtenir | Effet sur SmartScreen | Remarques |
|---|---|---|---|---|
| **A. Ne pas signer** (état actuel) | 0 | tout le monde | avertissement à chaque nouvel installeur | acceptable pour une alpha privée à un utilisateur |
| **B. Azure Artifact Signing** (ex-Trusted Signing) | 9,99 $/mois (5 000 signatures, 1 profil) ; 99,99 $/mois (100 000 signatures) | **organisations** : États-Unis, Canada, UE, Royaume-Uni et, d'après la page de prérequis, Australie, Nouvelle-Zélande, Japon, Corée du Sud, Singapour, **Suisse**, Norvège, Israël ; **particuliers : États-Unis et Canada uniquement** | nom d'éditeur vérifié ; « la réputation se construit avec le temps, avertissements initiaux attendus » | pas de jeton matériel, pensé pour la CI ; certificats de courte durée + horodatage ; l'éditeur affiché est le nom légal validé |
| **C. Certificat OV / IV d'une autorité** (Sectigo, DigiCert, SSL.com, Certum…) | 150–300 $/an | monde entier, entreprises et particuliers (validation d'identité) | identique à B | depuis juin 2023 la clé doit être sur un HSM ou un jeton matériel → pour la CI, prendre l'offre « cloud » de l'autorité ; validité limitée à 460 jours depuis le 1ᵉʳ mars 2026 |
| C bis. Certum « Open Source Code Signing in the Cloud » | 49 € (affiché en rupture de stock le jour de la consultation) | développeurs publiant sous licence open source ; éditeur affiché : « Open Source Developer, <nom> » | identique à B | la moins chère pour un particulier, si le projet est open source |
| **D. Certificat EV** | 400 $/an et plus | entreprises surtout | **identique à OV depuis 2024** | la prime ne se justifie plus pour SmartScreen |
| **E. Microsoft Store** | inscription **gratuite pour les particuliers** depuis septembre 2025 (≈ 200 marchés) | particuliers et entreprises, vérification par pièce d'identité | **aucun avertissement** : un paquet MSIX est re-signé par Microsoft | il faut un paquet **MSIX**, que Tauri ne produit pas (NSIS / MSI seulement) : empaquetage à construire et certification du Store à passer ; soumettre l'installeur EXE tel quel est possible mais il doit alors déjà être signé (retour à B ou C) |
| **F. SignPath Foundation** | gratuit | **projets open source uniquement** : licence approuvée par l'OSI, aucun composant propriétaire, binaires construits de façon vérifiable, politique de signature publiée, approbation manuelle de chaque version | identique à B ; l'éditeur affiché est « SignPath Foundation » | inapplicable tant que le dépôt est privé |
| Certificat auto-signé | 0 | — | aussi mauvais que A | inutile hors parc d'entreprise |

Deux points d'attention si l'éditeur est un particulier, ou est établi en Suisse (question posée pour ce
projet) :

- **Azure Artifact Signing n'accepte pas les particuliers hors États-Unis / Canada.** La voie la moins chère
  (≈ 120 $/an) n'est donc ouverte que si Corde est édité par une entreprise enregistrée. La Suisse figure
  dans la liste des pays pour les organisations sur la page de prérequis (mise à jour en mai 2026), mais pas
  sur la page comparative de Microsoft (août 2026, « USA, Canada, UE, Royaume-Uni ») : **à vérifier dans le
  portail Azure au moment de s'inscrire**.
- Un particulier, en Suisse comme ailleurs, peut obtenir un certificat **C** (validation individuelle) ou
  passer par le **Store (E)**.

**Recommandation.**

1. **Maintenant (alpha privée, un seul utilisateur) : option A.** Ne rien payer, expliquer honnêtement
   « Informations complémentaires → Exécuter quand même » dans le guide d'installation et dans les notes de la
   version continue. Deux clics par mise à jour.
2. **Réduire le nombre de fois où l'avertissement apparaît** sans rien signer : un lot « mise à jour
   intégrée » (`tauri-plugin-updater`) ferait télécharger l'installeur par Corde lui-même ; un fichier que le
   navigateur n'a pas téléchargé ne porte pas la marque « provient d'Internet » et n'est en principe pas
   soumis à l'écran SmartScreen. L'avertissement ne resterait qu'à la première installation. (À vérifier sur
   le PC du pilote ; demande aussi un hébergement des mises à jour accessible sans connexion GitHub, le dépôt
   étant privé.)
3. **Avant toute diffusion à d'autres pilotes : signer**, selon la destination du projet (décision déjà
   ouverte dans `docs/STATUS.md`) :
   - projet **open source** → F (gratuit) ou C bis (49 €/an) ;
   - édité par une **entreprise** → B (≈ 120 $/an), l'étape de CI est déjà câblée ;
   - **particulier, code fermé** → C (150–300 $/an, offre cloud), ou E si le coût doit rester nul et que
     l'on accepte le travail d'empaquetage MSIX et la publication sur le Store.
   Dans tous les cas, prévenir les premiers utilisateurs que l'avertissement peut encore apparaître quelques
   semaines.

### Étape de signature dans la CI (présente, inactive)

Le job `desktop` de `.github/workflows/ci.yml` contient une étape de signature **optionnelle** pour l'option
B. Elle ne s'exécute que si le dépôt possède à la fois :

| Type | Nom | Contenu |
|---|---|---|
| secret | `AZURE_CLIENT_ID`, `AZURE_CLIENT_SECRET`, `AZURE_TENANT_ID` | inscription d'application Azure autorisée à signer |
| variable | `ARTIFACT_SIGNING_ENDPOINT` | ex. `https://weu.codesigning.azure.net` |
| variable | `ARTIFACT_SIGNING_ACCOUNT`, `ARTIFACT_SIGNING_PROFILE` | compte et profil de certificat |

Elle installe `artifact-signing-cli` et ajoute à la configuration Tauri une `signCommand` : Tauri signe alors
`Corde.exe`, l'installeur et le désinstalleur, puis une étape vérifie la signature de l'installeur. **Cette
étape n'a jamais tourné** (aucun compte de signature n'existe) : prévoir un tour de CI de mise au point le
jour où les secrets sont créés. Pour l'option C, le point d'accroche est le même : remplacer la commande par
l'outil de l'autorité choisie (`%1` = fichier à signer).

## Ce que le propriétaire doit décider

1. **La destination du projet** (personnel, open source, commercial) : elle ouvre ou ferme les options
   gratuites (F, C bis).
2. **L'identité affichée comme éditeur** : son nom civil (validation individuelle, option C), une entreprise
   (options B, C, D), ou « SignPath Foundation » / le Microsoft Store.
3. **Le budget** : 0 (A ou E, avec du travail pour E), ≈ 10 $/mois (B, si entreprise), 150–300 $/an (C).
4. **Le moment** : rester en A tant que Corde n'est installé que chez lui est raisonnable ; la signature
   devient nécessaire avant de donner l'installeur à d'autres.

## Conséquences

- Par défaut, plus aucune invite de pare-feu ; l'accès téléphone devient un choix explicite dont le guide
  d'installation décrit l'invite.
- L'installation ne demande jamais de droits administrateur ; la désinstallation non plus.
- L'avertissement SmartScreen reste, à chaque nouvel installeur, tant que rien n'est signé : c'est écrit dans
  `docs/TESTING-ON-PC.md` et dans les notes de la version continue plutôt que caché.
- Un antivirus peut aussi signaler un exécutable récent et non signé qui lit la mémoire d'un autre programme
  (c'est le principe de la télémétrie) : si cela arrive, noter le nom de l'alerte ; la signature (point 3)
  réduit aussi ce risque.
- Sur les PC où **Smart App Control** est actif (Windows 11, installations récentes), les exécutables non
  signés peuvent être refusés sans bouton « Exécuter quand même » : cas non rencontré, à surveiller.

## Références

Consultées le 10 novembre 2026.

- Réputation SmartScreen pour les développeurs (Microsoft Learn, mis à jour le 6 mai 2026) :
  https://learn.microsoft.com/windows/apps/package-and-deploy/smartscreen-reputation
- Options de signature de code pour les applications Windows (Microsoft Learn, mis à jour le 29 août 2026) :
  https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/code-signing-options
- Azure Artifact Signing — prérequis et pays pris en charge (mis à jour le 21 mai 2026) :
  https://learn.microsoft.com/azure/artifact-signing/quickstart — FAQ :
  https://learn.microsoft.com/azure/artifact-signing/faq
- Disponibilité générale et tarifs d'Azure Artifact Signing (DevClass, 14 janvier 2026) :
  https://www.devclass.com/security/2026/01/14/code-signing-windows-apps-may-be-easier-and-more-secure-with-new-azure-artifact-service/4079554
- Inscription gratuite au Microsoft Store pour les particuliers (blog Windows Developer, 10 septembre 2025) :
  https://blogs.windows.com/windowsdeveloper/2025/09/10/free-developer-registration-for-individual-developers-on-microsoft-store/
- Validité des certificats de signature réduite à 460 jours (scrutin CSC-31 du CA/Browser Forum) :
  https://sslinsights.com/code-signing-certificate-validity-reduced-460-days/
- Certum, Open Source Code Signing in the Cloud :
  https://shop.certum.eu/open-source-code-signing-on-simplysign.html
- SignPath Foundation, conditions pour les projets open source : https://signpath.org/terms
- Règles du pare-feu Windows et invite à la première écoute (Microsoft Learn) :
  https://learn.microsoft.com/windows/security/operating-system-security/network-security/windows-firewall/rules
- Tauri 2 — signature de code Windows (`signCommand`, Azure Artifact Signing) :
  https://v2.tauri.app/distribute/sign/windows/ — installeur Windows (NSIS, WebView2) :
  https://v2.tauri.app/distribute/windows-installer/
