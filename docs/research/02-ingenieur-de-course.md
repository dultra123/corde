# Base de connaissances « ingénieur de course » — Le Mans Ultimate (LMU)

Document de référence pour le moteur de recommandation de setup (Hypercar LMH/LMDh, LMP2, LMGT3).
Rédigé le 2 novembre 2026. État du jeu considéré : LMU v1.4.x (juillet 2026) avec API mémoire partagée native introduite en v1.2 (décembre 2025).

Convention de confiance utilisée partout :
- **[C=H]** haute : confirmé par documentation officielle / code source / plusieurs sources concordantes.
- **[C=M]** moyenne : source unique crédible, ou principe de dynamique véhicule classique dont la transposition à LMU est très probable mais non vérifiée chiffre par chiffre.
- **[C=L]** basse : valeur indicative issue de l'expérience rF2/LMU ou d'un forum ; **à vérifier dans le garage du jeu ou dans le fichier de setup** avant d'être codée en dur.

Règle d'architecture qui découle de cette revue : **ne jamais coder en dur les bornes et pas des réglages**. Les lire à l'exécution via l'API REST du jeu (port 6397, groupe `enable_garage_setup_info`) ou via le fichier de setup, et ne garder dans la base de règles que des directions, des pas relatifs (« ±1 cran », « ±2 mm », « ±2 kPa ») et des ordres de grandeur.

---

## 0. Résumé exécutif

1. LMU expose un garage de type rFactor 2 : ailerons, hauteurs de caisse, ressorts, 3e élément (prototypes), amortisseurs bump/rebound lent/rapide, packers, barres, carrossage/pince/chasse, pressions, répartition et pression de frein, conduits, différentiel (précharge/power/coast), électronique (TC 3 paramètres 1–11, ABS GT3 uniquement, maps moteur, hybride, brake migration 0–2,5), énergie virtuelle et ratio carburant. Rapports de boîte verrouillés (jeux de rapports par circuit).
2. Le BoP verrouille : masse/lest, puissance (kW ou %), énergie max par relais (MJ), angle d'aileron arrière minimal (LMGT3, en degrés), traînée, et pour les Hypercars la vitesse de déploiement du MGU avant (150/190 km/h). Les aides « pilote » (ABS/stabilité sur Hypercar) coûtent 20–45 kg.
3. La télémétrie disponible (mémoire partagée native `LMU_Data` à 100 Hz, ou plugin rF2 à 50 Hz) fournit par roue : 3 températures de surface, 3 de couche interne, carcasse, pression kPa, usure, T° frein, déflexion de suspension, hauteur de caisse, charge pneu, grip fraction, vitesses de patch latérale/longitudinale, rotation ; et véhicule : vitesses/accélérations/rotations locales, volant, pédales filtrées (TC/ABS) et non filtrées, appuis aéro avant/arrière, traînée, déflexion 3e élément, répartition de frein, batterie, carburant. Le REST ajoute setup chargé, énergie virtuelle par tour, pneus autorisés et **température optimale des pneus** (à lire plutôt qu'à deviner).
4. Diagnostics calculables de façon robuste : gradient de sous-virage par phase (angle volant − L·ψ̇/V), écart d'angles de dérive AV/AR, saturation via grip fraction, slip ratio par roue (blocage/patinage), répartition I/M/O des températures (carrossage/pression), histogrammes de vitesse d'amortisseur (asymétrie bump/rebound, zone lente 5–25 mm/s), talonnage (hauteur mini + saturation packer), dégradation par relais, consommation carburant/énergie, et indicateurs de style (agressivité frein, trail braking, lissage volant, vibreurs).
5. Les règles symptôme→modification sont livrées en YAML valide (46 règles, ≈300 couples symptôme→paramètre priorisés) avec phase, vitesse de virage, direction, magnitude, effets secondaires et conflits, déclinées sec/pluie et selon deux philosophies : « agressif » (vif, pointe) et « soft/sûr » (stable, long relais).
6. Points de vigilance spécifiques LMU : (a) le grip dépend surtout de la température de carcasse, pas des pics de surface ; (b) les Hypercars décrochent du fond plat si trop basses/molles ; (c) batterie à 100 % = plus de régén = déséquilibre au freinage ; (d) la pince et le carrossage comptent moins qu'ailerons, hauteurs, packers, répartition de frein et TC pour le temps au tour ; (e) après v1.4 les freins ont des courbes friction/température réalistes : surveiller les T° de disque.
7. Méthodologie : pressions → aéro/hauteurs → barres → ressorts/amortisseurs → différentiel → électronique ; une variable à la fois sauf « paquets » cohérents entre circuits ; pondérer ressenti et données par la cohérence inter-tours, jamais sur un seul tour.

---

## A. Inventaire des paramètres réglables dans LMU, par classe

### A.0 Structure du garage et nommage programmatique

LMU hérite du garage rFactor 2 (menus **Général/Châssis, Suspension (par roue), Aéro, Powertrain/Électronique, Freins, Pneus, Transmission**) et de son format de fichier de setup (clés de type `FrontWingSetting`, `RearWingSetting`, `RideHeightSetting`, `SpringSetting`, `SlowBumpSetting`, `FastBumpSetting`, `SlowReboundSetting`, `FastReboundSetting`, `PackerSetting`, `FrontAntiSwaySetting`, `RearAntiSwaySetting`, `Front3rdSpringSetting`, `Rear3rdSpringSetting`, `CamberSetting`, `ToeInSetting`, `CasterSetting`, `PressureSetting`, `RearBrakeSetting` (répartition), `BrakePressureSetting`, `BrakeDuctSetting`, `RadiatorSetting`, `DiffPumpSetting`, `DiffPowerSetting`, `DiffCoastSetting`, `DiffPreloadSetting`, `SteerLockSetting`, `FuelSetting`, `GearSetting`, `TractionControlSetting`). Chaque clé stocke un **index de cran** ; la valeur physique (N/mm, mm, °, kPa) n'est connue que par la définition du véhicule. **[C=M]** pour l'héritage rF2 ; **[C=L]** pour l'exactitude de chaque nom de clé dans la version actuelle de LMU (LMU a ajouté des clés propres : TC power cut, TC slip, ABS map, brake migration, motor map, regen, engine mixture, virtual energy, fuel ratio).

Conséquence pour l'application : exposer les réglages en **crans relatifs** et lire la plage réelle via `GET http://localhost:6397/rest/garage/...` (groupe REST `enable_garage_setup_info`, activé par défaut dans ModernTinyPedal/TinyPedal) ou par sauvegarde auto du setup (`Enable Auto Backup Car Setup`). **[C=M]**

Sources : https://github.com/Keenny38/ModernTinyPedals/wiki/Game-Setup · https://github.com/Swizzjack/lmu-pitwall · https://lemansultimate.wiki.gg/wiki/Setup_Guide

### A.1 Tableau des paramètres — Hypercar (LMH / LMDh)

| Groupe | Paramètre | Unité / forme dans LMU | Plage typique (indicative) | Verrouillé BoP ? | Confiance | Notes |
|---|---|---|---|---|---|---|
| Aéro | Aileron arrière | cran (°) | selon voiture (ex. 1–10 crans) ; Peugeot 9X8 2023 « sans aile » = pas de réglage | Non (mais BoP ajuste traînée/puissance) | M | « Un seul cran change le caractère de la voiture » (CDA). Variante 9X8 avec aile dispo. |
| Aéro | Volet/flap avant | cran | car-spécifique, parfois absent | — | L | Sur certains châssis seulement. À détecter via l'API garage. |
| Aéro | Hauteur de caisse AV / AR | mm (statique, sans carburant) | ≈ 40–80 mm | Non | M (plage L) | Très sensible : « quelques mm » changent l'équilibre ; trop bas = talonnage/décrochage de fond plat en zone rapide ; trop haut = grosse perte d'appui. |
| Suspension | Ressorts AV / AR | N/mm (cran) | car-spécifique | Non | M | Ressorts très mous = « floor stall » et bascule d'équilibre imprévisible (CDA). |
| Suspension | 3e élément (heave) AV / AR + packer 3e élément | N/mm, mm | présents sur prototypes | Non | M | Champs `mFront3rdDeflection` / `mRear3rdDeflection` existent dans la télémétrie, donc modélisés ; présence réglable à vérifier par voiture. |
| Suspension | Packers / butées | mm | 0–30 mm | Non | M | « Rear packers » cités par CDA pour stabiliser l'arrière du 963 ; empêchent le talonnage sans changer la hauteur statique. |
| Suspension | Amortisseurs : bump lent/rapide, rebound lent/rapide, par roue | crans | 1–20 ou 1–30 selon voiture | Non | M | Nommage rF2. Seuil lent/rapide défini par voiture (≈ 25–50 mm/s) [L]. |
| Suspension | Barres anti-roulis AV / AR | cran (ex. P1–P7) ou N/mm | car-spécifique | Non | H | Certaines Hypercars ont une barre **réglable en roulant** (champ « ARB » dans l'électronique de la mémoire partagée). |
| Géométrie | Carrossage AV / AR | ° | AV −2,0 à −3,5 ; AR −1,0 à −2,5 | Non | L | Impact faible sur le chrono en LMU selon certains setupeurs. |
| Géométrie | Pince (toe) AV / AR | ° ou mm | AV −0,10 à +0,05 ; AR +0,05 à +0,30 | Non | L | Pince AR positive = stabilité sortie (CDA 963). |
| Géométrie | Chasse (caster) | ° | car-spécifique | Non | M | Plus de chasse = plus de retour volant et de gain de carrossage en braquage. |
| Pneus | Composé | Soft / Medium / Hard / Wet (Michelin) | allocation par session | Allocation par session | H | Hard : 3–5 tours de chauffe ; Soft : 2–3 (CDA). Pas de couvertures chauffantes. |
| Pneus | Pression à froid, par roue | kPa (UI possible en psi) | ≈ 165–195 kPa | Non | L (plage) | Recommandation répandue : rouler au minimum autorisé. Réel Michelin : mini 26,1 psi AV / 27,6 psi AR (≈ 180/190 kPa). |
| Freins | Répartition AV | % AV | 52–58 % | Non | M | Réglable en roulant. Exemple CDA : 54:46 + migration 2,0F ⇒ 56:44 à 100 % de pédale. |
| Freins | Brake migration | 0 à 2,5 par pas de 0,5, direction F/R | 0–2,5 | Non | H | Décale la répartition vers l'avant quand la pédale approche 100 %, revient en relâchant (aide au trail braking). Recommandation Traxion : 2,5F. Réglable en roulant. |
| Freins | Pression de frein / force pédale max | % | 70–100 % | Non | M | CDA LMP3 : 85–91 % pour pédales de charge. |
| Freins | Conduits de frein | cran 0–6 | — | Non | M | Influence T° disque et (modèle rF2) T° de jante/pneu. |
| Freins | ABS | aide « pilote » uniquement | Low/High | **Pénalité** +25 kg (Low) / +45 kg (High) | H | Pas d'ABS natif en Hypercar. |
| Freins | Contrôle de stabilité (aide) | Low/High | — | **Pénalité** +20/+40 kg | H | |
| Différentiel | Précharge | Nm (cran) | 0–150 Nm | Non | M | |
| Différentiel | Verrouillage en charge (power) / en retenue (coast) | % ou ° de rampe (cran) | 20–80 % / 10–60 % | Non | M | rF2 expose aussi parfois « pump » (visqueux). |
| Transmission | Rapports | jeux de rapports fixes (« Standard », « Le Mans », …) | choix du jeu | **Oui** (jeux imposés) | H | Oreca : jeu « Le Mans » uniquement au Mans ; Corvette : doublon « Standard » corrigé. |
| Électronique | TC (3 paramètres : TC, Power Cut, Slip) | 1–11 chacun | 1–11 | Non | H | Garage *Advanced Setup → Powertrain* et MFD ⚡ en roulant. Base 7-7-7, perf 6-6-6, sécurité 9-9-9 ; Hypercar : TC & slip 9–11 avec power cut 1–2 (simracingcockpit). |
| Électronique | Motor map (déploiement) | kW (crans) | LMDh 0–50 kW ; LMH 0–200 kW (cible relais 40–100 kW) | Puissance totale 500 kW plafonnée | H | LMDh : déploiement dès 0 km/h ; LMH : seulement > 190 km/h (Ferrari, Toyota) ou > 150 km/h (Peugeot). |
| Électronique | Regen level | crans | max recommandé | — | H | Régén max : LMDh 170 kW, LMH 200 kW. Batterie à 100 % = plus de régén = perte de freinage et déséquilibre (LMH bloque l'AR, LMDh l'AV). Soft-clipping batterie 15 %. |
| Électronique | Engine map / mixture | crans | car-spécifique | — | M | Lean = économie, perte de perf. |
| Énergie | Énergie virtuelle (NRG) | % du max relais | 100 % par défaut | **Max relais fixé par BoP (MJ)** | H | ≈ 40–50 min de relais ; 0 % ⇒ stop & go 100 s. |
| Énergie | Fuel ratio | sans unité | ≈ 0,90–1,00 | Non | H | 0,96 ⇒ 4 % de l'énergie virtuelle vient de la batterie ; fixe la charge carburant réelle. |
| Énergie | Carburant | L | dérivé du ratio/VE | Capacité fixée | H | |
| Direction | Butée de direction / rapport | ° | car-spécifique | Non | M | |
| Refroidissement | Caches radiateur | cran | — | Non | M | Garder eau/huile < 120 °C (CDA LMP3). |
| Masse | Lest / répartition | — | — | **Oui** (WeightPenalty kg, position lest fixée) | H | |
| Puissance | Puissance max | kW | — | **Oui** (MaxPowerOutput kW par circuit) | H | |

Sources : https://coachdaveacademy.com/tutorials/the-ultimate-le-mans-ultimate-setup-guide/ · https://coachdaveacademy.com/tutorials/hypercar-electric-motor-maps-in-le-mans-ultimate-explained/ · https://traxion.gg/le-mans-ultimate-how-to-master-hypercar-braking/ · https://simracingcockpit.gg/le-mans-ultimate-how-to-drive-the-lmgt3-and-hypercar/ · https://guide.lemansultimate.com/hc/en-gb/articles/13152322265231-Hypercar-Category-LMH-LMDh · https://guide.lemansultimate.com/hc/en-gb/articles/13182869047311-How-do-I-configure-my-traction-control-in-Le-Mans-Ultimate · https://traxion.gg/le-mans-ultimate-update-adjusts-lmgt3-bop-and-increases-driver-aid-weight-penalties/ · https://www.bsimracing.com/le-mans-ultimate-update-2-patch-4-deployed/ · https://traxion.gg/widespread-bop-adjustments-nerf-bmw-lmgt3-and-alpine-a424-in-new-le-mans-ultimate-update/ · https://coachdaveacademy.com/tutorials/lmu-porsche-963-guide-the-inside-line/ · https://a-e.li/news/motorsport/wec/michelin-le-mans-hypercar-tires/ · https://arnouthoekstra.substack.com/p/confused-by-tc-you-should-be · https://lemansultimate.com/?p=3955

### A.2 Tableau des paramètres — LMP2 (Oreca 07 Gibson)

| Groupe | Paramètre | Unité | Plage indicative | Verrouillé BoP ? | Confiance | Notes |
|---|---|---|---|---|---|---|
| Aéro | Aileron arrière | cran | car-spécifique | Non | M | Kit Le Mans (faible traînée) via jeu de rapports « Le Mans » + setup défaut. |
| Aéro | Flap avant / dive planes | cran | — | — | L | Présence à confirmer par l'API garage. |
| Aéro | Hauteurs AV/AR | mm | ≈ 30–70 mm | Non | M | Voiture « très sensible aux hauteurs, à l'équilibre aéro et aux pressions » (wiki). |
| Suspension | Ressorts, 3e élément, packers, amortisseurs 4 voies, barres | idem Hypercar | — | Non | M | |
| Géométrie | Carrossage / pince / chasse | ° | AV −2,0 à −3,5, AR −1,0 à −2,5 | Non | L | |
| Pneus | Composés Goodyear | Soft / Medium / Hard / Wet (selon allocation) | — | Allocation | M | CDA mentionne soft et hard ; lire l'allocation via REST « allowed tyres ». |
| Pneus | Pressions | kPa | ≈ 150–185 kPa | Non | L | v1.4 : chauffe corrigée (pressions ne montaient plus sans fin après blocages). |
| Freins | Répartition AV | % | 54–60 % | Non | M | Pas d'ABS natif ; freins « utilisables à froid » (CDA). |
| Freins | Pression, conduits | %, cran | — | Non | M | |
| Freins | Brake migration | — | **non** (spécifique Hypercar, lié au MGU) | — | M | |
| Différentiel | Précharge / power / coast | Nm, % | — | Non | M | |
| Transmission | Rapports | jeux fixes | — | **Oui** | H | |
| Électronique | TC 3 paramètres | 1–11 | — | Non | H | Pas de système hybride. |
| Électronique | Engine mixture | crans | — | — | M | |
| Énergie | Énergie virtuelle / carburant | %, L | — | **Max relais BoP** | M | Le système NRG s'applique à toutes les classes (BoP LMGT3 publie « MaxStintEnergy [MJ] »), sans part batterie. |
| Masse | 930–950 kg (+7 kg en v1.4) | kg | — | **Oui** | H | |

Sources : https://lemansultimate.wiki.gg/wiki/LMP2 · https://coachdaveacademy.com/tutorials/complete-guide-to-the-lmp2-class-in-le-mans-ultimate/ · https://traxion.gg/latest-le-mans-ultimate-update-to-improve-hypercar-cold-tyre-performance-patch-notes-released/

### A.3 Tableau des paramètres — LMGT3

| Groupe | Paramètre | Unité | Plage indicative | Verrouillé BoP ? | Confiance | Notes |
|---|---|---|---|---|---|---|
| Aéro | Aileron arrière | **degrés** | car-spécifique | **Angle minimum imposé par circuit** (ex. Ferrari 296 : 3,85° à Monza ; McLaren 720S : 4,80° au Mans/Monza) | H | Le paramètre le plus important pour l'équilibre haute vitesse (A. Hoekstra) ; < 30 % d'aile = « très rapide » sur circuits à longues lignes droites. |
| Aéro | Splitter / dive planes | — | généralement fixes | — | M | |
| Aéro | Hauteurs AV/AR | mm | ≈ 50–90 mm | Mini possible par BoP (wiki) | M | Moins sensible que les prototypes. |
| Suspension | Ressorts AV/AR | N/mm | car-spécifique | Non | H | |
| Suspension | 3e élément | — | **absent** en général | — | M | |
| Suspension | Packers / bump stops (course, raideur) | mm | — | Non | M | « Packer distances » = facteur n°2 de pilotabilité (Hoekstra). |
| Suspension | Amortisseurs 4 voies par roue | crans | — | Non | H | |
| Suspension | Barres AV/AR | crans | — | Non | H | |
| Géométrie | Carrossage / pince / chasse | ° | AV −2,5 à −4,0, AR −1,5 à −3,0 | Non | L | |
| Pneus | Composés Goodyear | allocation par session (souvent un seul sec + Wet) | — | Allocation | L | Lire l'allocation via REST. Modèle v1.2+ : moins sensible aux pics de chaleur ; usure Porsche « extrême ». |
| Pneus | Pressions | kPa | ≈ 140–170 kPa | Non | L | « Au minimum » recommandé. |
| Freins | Répartition | % AV | 48–56 % | Non | M | Peut être plus arrière grâce à l'ABS ; plus avant en début de relais pneus froids. |
| Freins | ABS | 1–10 (avant juin 2025) puis **maps ABS** | — | Non | H | Seule classe avec ABS natif. Régler la map puis la répartition, pas les deux à la fois. |
| Freins | Pression / conduits | %, cran | — | Non | M | v1.4 : courbes friction-température des plaquettes et thermique disque réalistes. |
| Différentiel | Précharge (power/coast selon voiture) | Nm | — | Non | M | BMW M4 : précharge réglable depuis v1.4. |
| Transmission | Rapports | fixes | — | **Oui** | H | |
| Électronique | TC 3 paramètres | 1–11 | — | Non | H | Rotation à l'accélération : slip & power cut à 1–2, TC principal moyen-haut. |
| Électronique | Engine mixture | crans | — | — | M | Baisser pour économiser (lift & coast). |
| Énergie | Carburant / énergie relais | L / MJ | — | **MaxStintEnergy [MJ] par circuit** | H | |
| Masse | Lest | kg | — | **Oui** (WeightPenalty par circuit) | H | |
| Puissance | — | % | — | **Oui** (PowerOutput % par circuit) | H | |
| Traînée | — | — | — | **Oui** (ajustée par BoP, ex. Corvette à Monza) | H | |

Sources : https://lemansultimate.wiki.gg/wiki/LMGT3 · https://guide.lemansultimate.com/hc/en-gb/articles/13211078435983-ABS-Anti-Lock-Braking-System-for-LMGT3-Cars · https://traxion.gg/widespread-bop-adjustments-nerf-bmw-lmgt3-and-alpine-a424-in-new-le-mans-ultimate-update/ · https://arnouthoekstra.substack.com/p/fixed-setups-are-a-blessing-in-lmu · https://coachdaveacademy.com/tutorials/the-complete-guide-to-the-lmgt3-cars-in-le-mans-ultimate/ · https://traxion.gg/latest-le-mans-ultimate-update-to-improve-hypercar-cold-tyre-performance-patch-notes-released/

### A.4 Fenêtres de température pneus et pressions : ce qui est connu

- **Aucune valeur officielle publique par composé.** Le menu Pits du jeu affiche la température idéale par type de pneu, et le REST expose « optimal tyre temperature » (TinyPedal l'utilise pour ses overlays). **Le moteur doit lire cette valeur à l'exécution.** [C=M]
- Ordres de grandeur communautaires : cible de chauffe ≈ 78–80 °C (carcasse) pour GT3/Hypercar ; pluie ≈ 60–80 °C ; LMP3 (CDA) : conduits de frein à régler pour garder les pneus dans 85–95 °C ; GT3 iRacing transposable : 80–90 °C à chaud. Au-dessus de ~110–115 °C de surface prolongés, on parle de surchauffe. [C=L]
- Le grip rF2/LMU dépend principalement de la **température de carcasse** (`mTireCarcassTemperature`) ; les températures de surface (`mTemperature[3]`) montent à plusieurs centaines de degrés lors d'un blocage et redescendent en un tour : les utiliser pour la *répartition* I/M/O, pas pour le niveau absolu. [C=M]
- Pression : dans rF2, plus de pression ⇒ moins de flexion de carcasse ⇒ moins d'échauffement interne et moins de grip mécanique ; la communauté LMU recommande le mini autorisé, sauf par temps froid (monter pour accélérer la chauffe). v1.2.3 : pics de T° réduits, chauffe plus longue ; v1.4 : grip à froid Hypercar amélioré (out-lap jusqu'à 5 s plus rapide). [C=M]
- Loi des gaz pour prédire la pression à chaud : `p_chaud ≈ p_froid × (T_chaud + 273,15)/(T_froid + 273,15)` (pression absolue ; soit ≈ +0,5–0,6 kPa par °C de gaz pour 180 kPa manométrique). [C=H physique, C=M pour la fidélité du modèle rF2]

Sources : https://steamcommunity.com/app/2399420/discussions/0/777534074828971181 · https://steamcommunity.com/app/2399420/discussions/0/4637113154621450877 · https://github.com/Keenny38/ModernTinyPedals/wiki/Game-Setup · https://overtake.gg/threads/not-a-popular-question-but-needs-answering-none-the-less.190351 · https://traxion.gg/le-mans-ultimates-hypercars-given-massive-tyre-update/ · https://coachdaveacademy.com/tutorials/understanding-tire-pressures-in-iracing/ · https://lemansultimate.com/?p=3955

### A.5 Ce que le BoP verrouille (synthèse)

| Levier BoP | Classes | Unité | Exemple publié | Confiance |
|---|---|---|---|---|
| WeightPenalty (lest, position fixée) | toutes | kg | Lamborghini Huracán +45 kg Le Mans ; Porsche 963 +10 kg partout | H |
| MaxPowerOutput | Hypercar | kW | BMW/Cadillac/Lamborghini −5 kW Le Mans | H |
| PowerOutput | LMGT3 | % | Ford −2 % Monza Curva Grande | H |
| MaxStintEnergy | LMGT3 (et logiquement LMP2/Hypercar via NRG) | MJ | Porsche 911 +22 MJ Bahreïn | H |
| MinimumAllowedRearWing | LMGT3 | ° | 3,85° (296, Monza), 4,80° (720S, Le Mans/Monza) | H |
| Traînée aéro | LMGT3 | — | Corvette Monza | H |
| Puissance totale combinée | Hypercar | 500 kW | tous | H |
| Déploiement MGU avant | LMH | km/h | 190 (Ferrari/Toyota), 150 (Peugeot) | H |
| Puissance MGU | LMH 200 kW / LMDh 50 kW ; régén 200 / 170 kW | kW | — | H |
| Pénalités aides | Hypercar | kg | ABS +25/+45 ; stabilité +20/+40 | H |
| Hauteur de caisse mini | LMGT3 (cité par le wiki) | mm | non publié | L |
| Jeux de rapports | toutes | — | « Le Mans » vs « Standard » | H |

Sources : https://traxion.gg/widespread-bop-adjustments-nerf-bmw-lmgt3-and-alpine-a424-in-new-le-mans-ultimate-update/ · https://www.bsimracing.com/le-mans-ultimate-update-2-patch-4-deployed/ · https://www.bsimracing.com/le-mans-ultimate-v1-3-and-new-hotfix-released/ · https://traxion.gg/how-to-manage-virtual-energy-in-le-mans-ultimate/

---
## B. Catalogue de diagnostics calculables à partir de la télémétrie

### B.0 Données disponibles (rF2 Shared Memory Map Plugin / API native LMU)

Deux voies d'accès :
1. **API mémoire partagée native LMU** (`LMU_Data`, LMU ≥ 1.2, décembre 2025) : activer *Settings → Gameplay → Enable Plugins*. Télémétrie à **100 Hz**, scoring à 5 Hz, flux FFB 400 Hz. Expose en plus de la structure rF2 : **TC, ABS, Engine Map, ARB, Regen, Brake Migration** (états électroniques courants), lisible en ligne (EAC). [C=M]
2. **Plugin rF2SharedMemoryMapPlugin** (legacy, `$rFactor2SMMP_Telemetry$` 50 Hz, `$rFactor2SMMP_Scoring$` 5 Hz, `$rFactor2SMMP_Extended$`) : même layout de structures, déprécié mais fonctionnel. [C=H]
3. **REST local** `http://localhost:6397` (port = `WebUI port` de `UserData\player\Settings.JSON`) : `GET /rest/strategy/usage` (historique énergie virtuelle par tour), `GET /rest/garage/UIScreen/RepairAndRefuel` (usure aéro/freins/suspension), groupes `enable_vehicle_info`, `enable_race_info`, `enable_garage_setup_info` (setup chargé), `enable_session_info` (pneus autorisés, tracé officiel), `enable_weather_info`. Fournit la **température optimale des pneus**, le nom du setup chargé, les estimations carburant/énergie par tour. [C=M]
4. Fichiers MoTeC `.ld/.csv` écrits dans `Documents/Le Mans Ultimate/Telemetry` (analyse hors ligne, canaux damper velocity, suspension travel, ride height, etc.). [C=M]

Sources : https://github.com/Swizzjack/lmu-pitwall · https://github.com/Keenny38/ModernTinyPedals/wiki/Game-Setup · https://github.com/TheIronWolfModding/rF2SharedMemoryMapPlugin · https://lemansultimate.wiki.gg/wiki/Telemetry · https://github.com/coasting-nc/LMUFFB

#### B.0.1 Champs par roue (`rF2Wheel`, ×4, ordre FL, FR, RL, RR) [C=H sauf mention]

| Champ | Unité | Usage diagnostic |
|---|---|---|
| `mTemperature[3]` | **Kelvin** (−273,15 → °C) ; ordre gauche/centre/droite de la bande (⇒ intérieur/milieu/extérieur selon le côté de la voiture) | Répartition I/M/O → carrossage, pression |
| `mTireInnerLayerTemperature[3]` | K | Couche interne (moins bruitée que la surface) |
| `mTireCarcassTemperature` | K | **Niveau thermique qui pilote le grip** → fenêtre optimale |
| `mPressure` | kPa | Pression courante (chaude) |
| `mWear` | fraction 0–1 (1 = neuf dans rF2) | Dégradation par relais |
| `mBrakeTemp` | °C | Fenêtre freins → conduits |
| `mBrakePressure` | 0–1 (après répartition) | Effort de freinage par roue |
| `mSuspensionDeflection` | m | Course → vitesse d'amortisseur (dérivée), talonnage, raideur |
| `mRideHeight` | m | Hauteur par roue → talonnage, rake dynamique |
| `mSuspForce` | N (effort pushrod) | Transfert de charge |
| `mTireLoad` | N | Charge verticale pneu → transfert de charge, décollage de roue intérieure |
| `mLateralForce`, `mLongitudinalForce` | N | Utilisation du cercle d'adhérence |
| `mGripFract` | fraction du patch qui glisse (0 = adhère, →1 = saturé) | Saturation AV vs AR → sous/survirage |
| `mLateralPatchVel`, `mLongitudinalPatchVel` | m/s (vitesse de glissement au patch) | Angle de dérive, slip ratio |
| `mLateralGroundVel`, `mLongitudinalGroundVel` | m/s (vitesse sol au contact) | Dénominateurs des slips |
| `mRotation` | rad/s | Vitesse de roue → blocage/patinage |
| `mCamber`, `mToe` | rad (dynamiques) | Géométrie dynamique |
| `mVerticalTireDeflection` | m | Charge/pression |
| `mSurfaceType` (0 sec, 1 mouillé, 2 herbe, 3 terre, 4 gravier, 5 vibreur, 6 spécial) [C=M], `mTerrainName` | enum | Usage des vibreurs, sorties |
| `mFlat`, `mDetached` | bool | Crevaison (modélisée physiquement depuis v1.4) |

#### B.0.2 Champs véhicule (`rF2VehicleTelemetry`) [C=H]

| Champ | Unité | Usage |
|---|---|---|
| `mLocalVel` (x, y, z) | m/s, repère voiture (rF2 : **z négatif vers l'avant**, x latéral, y vertical) [C=M pour les signes : calibrer empiriquement] | vitesse `V = ‖mLocalVel‖`, dérive `β` |
| `mLocalAccel` (x, y, z) | m/s² | G latéral (x), G longitudinal (−z), vertical (y) |
| `mLocalRot` (x, y, z) | rad/s | **vitesse de lacet = y**, roulis = z, tangage = x [C=M] |
| `mLocalRotAccel` | rad/s² | accélération de lacet (réactivité, « snap ») |
| `mUnfilteredSteering` (−1…1), `mFilteredSteering`, `mPhysicalSteeringWheelRange` (°) | — | angle volant `SWA = steering × range/2` |
| `mUnfilteredThrottle/Brake/Clutch` vs `mFilteredThrottle/Brake` (0–1) | — | **écart = intervention TC (throttle) ou ABS (brake)** |
| `mSteeringShaftTorque` | Nm | chute de couple = perte de pneumatic trail = saturation AV |
| `mFrontRideHeight`, `mRearRideHeight`, `mFrontWingHeight` | m | rake dynamique, talonnage |
| `mFront3rdDeflection`, `mRear3rdDeflection` | m | course du 3e élément (heave) |
| `mFrontDownforce`, `mRearDownforce`, `mDrag` | N | équilibre aéro dynamique, décrochage de fond plat |
| `mRearBrakeBias` | fraction | répartition effective (inclut migration ?) [C=M] |
| `mEngineRPM`, `mGear`, `mEngineTorque`, `mTurboBoostPressure`, `mEngineWaterTemp`, `mEngineOilTemp` | — | maps moteur, refroidissement |
| `mFuel`, `mFuelCapacity` | L | consommation |
| `mBatteryChargeFraction`, `mElectricBoostMotorTorque`, `mElectricBoostMotorRPM`, `mElectricBoostMotorTemperature`, `mElectricBoostWaterTemperature`, `mElectricBoostMotorState` | —, Nm, tr/min, °C, enum | gestion hybride (ajoutés plugin v3.7.15.0) |
| `mFrontTireCompoundName`, `mRearTireCompoundName`, index | texte | composé monté |
| `mDeltaBest` | s | delta temps |
| `mLapNumber`, `mLapStartET`, `mElapsedTime`, `mCurrentSector` | — | découpage en tours/secteurs |

#### B.0.3 Scoring / météo (`rF2ScoringInfo`, `rF2VehicleScoring`, buffer Weather 1 Hz) [C=H]
`mTrackTemp`, `mAmbientTemp`, `mRaining` (0–1), `mDarkCloud`, `mWind[3]`, `mMinPathWetness/mMaxPathWetness/mAvgPathWetness` (0–1), `mBestLapTime`, `mLastLapTime`, `mLastSector1/2`, `mCurSector1/2`, `mLapDist` (m sur le tracé), `mPathLateral`, `mTrackEdge`, `mInPits`, `mNumPitstops`, `mCountLapFlag`/validité, `mSession`, `mGamePhase`.

#### B.0.4 Données hors mémoire partagée
- Setup courant complet (REST garage) → permet de rattacher chaque tour à un setup.
- Énergie virtuelle par tour (REST `/rest/strategy/usage`).
- Températures optimales des pneus (REST ; sinon menu Pits).
- Réglages électroniques courants (API native : TC/ABS/map/regen/migration/ARB).

Sources : https://domaslau.github.io/rF2SharedMemoryNet/api/rF2SharedMemoryNet.RF2Data.Structs.Wheel.html · https://docs.rs/crate/kerb/latest/source/src/lmu/structs.rs · https://github.com/TheIronWolfModding/rF2SharedMemoryMapPlugin · https://github.com/Swizzjack/lmu-pitwall

### B.1 Prétraitement commun

1. **Segmentation** : découper par tour (`mLapNumber`), rejeter in-lap/out-lap, tours invalides, tours sous drapeau jaune, tours avec `mSurfaceType ∉ {0,1,5}` > 2 % du temps (sortie de piste). [C=H méthode]
2. **Segmentation en virages** : virage = segment où `|a_y| > 0,25 g` (prototypes : 0,35 g) pendant > 0,4 s, fusion si écart < 0,3 s. Identifier apex = minimum de vitesse. Classer la **vitesse du virage** : lent `V_apex < 110 km/h`, moyen 110–180, rapide > 180 (Hypercar/LMP2 : décaler +20 km/h). [C=M, seuils à calibrer par classe]
3. **Phases** (par virage) :
   - *Entrée* : du début de freinage (`brake > 0,05`) jusqu'à `brake < 0,05` **ou** apex, avec braquage croissant.
   - *Milieu* : autour de l'apex, `throttle < 0,2` et `brake < 0,05`, `|a_y|` ≥ 90 % de son max.
   - *Sortie* : de la première remontée de gaz (`throttle > 0,2`) jusqu'à `|a_y| < 0,25 g` ou volant < 20 % de son max.
4. **Filtrage** : passe-bas 5–10 Hz pour les vitesses/accélérations, 2–3 Hz pour les températures ; dérivée centrale pour les vitesses d'amortisseur.
5. **Repère** : vérifier les signes rF2 à la première capture (ex. en ligne droite `−mLocalVel.z > 0`), stocker une table de signes par version du jeu.

### B.2 Diagnostics pneus

| ID | Diagnostic | Formule / méthode | Données | Seuils indicatifs | Conf. |
|---|---|---|---|---|---|
| D-TYR-01 | **Répartition I/M/O → carrossage** | Sur les phases *milieu* des virages où la roue est **extérieure**, moyenner `T_in`, `T_mid`, `T_out` (surface ou couche interne). Calculer `ΔIO = T_in − T_out`. | `mTemperature[3]` ou `mTireInnerLayerTemperature[3]` (orienter : pour la roue gauche, index 2 = intérieur ; pour la droite, index 0 = intérieur — **à vérifier sur capture**) | Cible `ΔIO ∈ [+5, +12] °C` (intérieur un peu plus chaud). `ΔIO > +15` ⇒ trop de carrossage négatif ; `ΔIO < +2` ou négatif ⇒ pas assez. Référence pyromètre Carroll Smith : intérieur 10–25 °C plus chaud (pneus réels, plus chaud que la cible sim). | M |
| D-TYR-02 | **Milieu vs bords → pression** | `ΔM = T_mid − (T_in + T_out)/2` sur tour complet. | idem | `ΔM > +3 °C` ⇒ surgonflé ; `ΔM < −3 °C` ⇒ sous-gonflé. Caveat rF2 : effet faible, pondérer avec D-TYR-04. | M |
| D-TYR-03 | **Niveau thermique vs fenêtre** | `T_core = mTireCarcassTemperature − 273,15` moyenné sur les secteurs roulants ; comparer à `T_opt` (REST) ± tolérance. | carcasse, REST | Froid si `T_core < T_opt − 8` ; chaud si `> T_opt + 8` ; surchauffe si surface `> 115 °C` pendant > 20 % du tour. | M |
| D-TYR-04 | **Pression chaude cible et dérive** | `p_hot(t)` par roue ; dérive sur relais `dp/dt`. Comparer `p_hot_mean` entre AV/AR et G/D. Prédire la pression à froid nécessaire : `p_cold_new = p_cold × (p_target / p_hot_mean)`. | `mPressure` | Écart G/D > 4 kPa à pression froide identique ⇒ asymétrie de charge/température (circuit tournant d'un côté) ; tolérer. | M |
| D-TYR-05 | **Gradient thermique AV/AR** | `T_front − T_rear` (carcasse, moyennes). | carcasse | > +8 °C ⇒ avant surtravaillé (sous-virage, trop d'appui AR ou pressions AV basses) ; < −8 °C ⇒ arrière surtravaillé (survirage, patinage). | M |
| D-TYR-06 | **Dégradation par relais** | Régression `laptime_corr = f(lap)` après correction carburant `laptime_corr = laptime − k_fuel × fuel_L` (k ≈ 0,03 s/L, à calibrer par classe) ; pente en s/tour. Et `Δwear/lap` par roue. | `mLastLapTime`, `mFuel`, `mWear` | Pente > 0,08 s/tour (GT3) ou roue dont l'usure est > 1,3× la moyenne des autres ⇒ pneu limitant. | M |
| D-TYR-07 | **Surchauffe par glissement** | Corrélation entre pics de surface (`T_surface − T_inner`) et `mGripFract`, slip ratio. | surfaces, grip | Pics > +25 °C répétés sur l'axe AR en sortie ⇒ motricité ; sur l'AV en entrée ⇒ blocages/sous-virage. | M |
| D-TYR-08 | **Pneus froids en début de relais** | Nombre de tours avant `T_core ≥ T_opt − 5`. | carcasse | > 3 tours (Hypercar hard : 3–5 attendus ; soft 2–3) ⇒ pressions trop basses pour les conditions, conduits trop ouverts, ou pilotage trop doux en out-lap. | M |

Sources : https://www.sau.com.au/forums/topic/302303-car-set-up-cause-and-effect-by-carroll-smith · https://coachdaveacademy.com/tutorials/understanding-tire-pressures-in-iracing/ · https://www.overtake.gg/threads/optimal-camber-my-thought-process-on-tyre-workloads.195866/ · https://coachdaveacademy.com/announcements/the-latest-le-mans-ultimate-hypercar-setup-guide · https://overtake.gg/threads/not-a-popular-question-but-needs-answering-none-the-less.190351

### B.3 Diagnostics freins

| ID | Diagnostic | Méthode | Données | Seuils indicatifs | Conf. |
|---|---|---|---|---|---|
| D-BRK-01 | **Fenêtre thermique freins → conduits** | Par roue : T° max par tour, T° min (fin de ligne droite), T° moyenne en phase de freinage. | `mBrakeTemp` | GT3 (acier) : optimum ≈ 350–650 °C AV, froid < 250, chaud > 750 ; prototypes/Hypercar (carbone) : optimum ≈ 400–800, froid < 300, fade > 1000. Depuis v1.4 LUT friction/température réalistes : utiliser la couleur HUD du jeu comme référence si disponible. | L–M |
| D-BRK-02 | **Freins froids en début de relais / de nuit** | T° au premier freinage de chaque tour. | idem, `mAmbientTemp` | < seuil froid ⇒ fermer conduits d'un cran, avancer la répartition au 1er tour. LMH : fade AV en fin de relais décale la répartition vers l'AR. | M |
| D-BRK-03 | **Blocage de roue** | Slip ratio `κ_i = mLongitudinalPatchVel_i / max(|mLongitudinalGroundVel_i|, 1)` (négatif au freinage) ou `κ_i = (ω_i·r_i − V)/V` avec `r_i` = rayon dynamique estimé en ligne droite. Blocage si `κ_i < −0,25` pendant > 80 ms avec `brake > 0,3` et `V > 40 km/h` ; blocage total si `|ω_i| < 2 rad/s`. Compter par roue et par phase (droit / entrée en courbe). | `mRotation`, patch/ground vel, `mUnfilteredBrake`, `V` | Si blocages AV ≫ AR : répartition trop avant (ou migration trop forte) ; AR ≫ AV : trop arrière ; roue intérieure seule en entrée : trail braking trop appuyé / rebound AV trop fort / trop de carrossage. | H (méthode), M (seuils) |
| D-BRK-04 | **Activité ABS (LMGT3)** | `ABS_act = mUnfilteredBrake − mFilteredBrake` ; % du temps de freinage avec `ABS_act > 0,05`. | pédales filtrées/non filtrées | > 30 % ⇒ pilote au-delà de l'ABS : map ABS plus intrusive (sûr) ou moins (agressif), ou répartition. Objectif réel : « freiner au seuil sans déclencher régulièrement ». | M |
| D-BRK-05 | **Stabilité au freinage** | Pendant la phase entrée en ligne droite (`|a_y| < 0,2 g`) : écart-type de la vitesse de lacet et correction de volant `std(SWA)`; en entrée courbe : sur-rotation `ψ̇ − ψ̇_ackermann > 0`. | `mLocalRot.y`, SWA | Corrections > 10° de volant répétées en freinage droit ⇒ instabilité (répartition trop AR, pince AR insuffisante, rebound AV / bump AR, migration). | M |
| D-BRK-06 | **Cohérence régén / batterie (Hypercar)** | Marquer les freinages où `mBatteryChargeFraction ≥ 0,98` ; comparer décélération max et blocages à ceux où SOC < 0,9. | batterie, décélération | Perte de décélération ou blocages spécifiques à SOC plein ⇒ problème de gestion motor map, pas de setup. | H |

Sources : https://traxion.gg/le-mans-ultimate-how-to-master-hypercar-braking/ · https://guide.lemansultimate.com/hc/en-gb/articles/13211078435983-ABS-Anti-Lock-Braking-System-for-LMGT3-Cars · https://traxion.gg/latest-le-mans-ultimate-update-to-improve-hypercar-cold-tyre-performance-patch-notes-released/ · https://docs.rs/crate/kerb/latest/source/src/lmu/structs.rs

### B.4 Diagnostics d'équilibre (sous-virage / survirage par phase)

Trois estimateurs indépendants, à fusionner (vote pondéré) :

**(a) Angle de sous-virage cinématique** [C=H méthode]
- `δ = SWA / i_s` (angle roues, rad ; `i_s` = rapport de direction, estimer en régime établi : `i_s ≈ SWA / (L·ψ̇/V)` sur les virages lents à faible g).
- `δ_Ackermann = L · ψ̇ / V` (L = empattement, ≈ 3,1 m Hypercar, 3,0 m LMP2, 2,6–2,8 m GT3 [C=M]).
- **US = δ − δ_Ackermann** (rad). `US > 0` ⇒ sous-virage ; `US < 0` ⇒ survirage (volant qui se rouvre = contre-braquage).
- Normaliser par `a_y` : gradient `K = US / a_y` (°/g). Référence : neutre si `|K| < 1 °/g` ; sous-virage marqué > 3 °/g ; survirage < −1 °/g. [C=M seuils]

**(b) Angles de dérive essieux** [C=H méthode]
- `α_i = atan2(mLateralPatchVel_i, |mLongitudinalGroundVel_i|)` par roue ; `α_F = moyenne(FL, FR)`, `α_R = moyenne(RL, RR)` (ou pondérer par `mTireLoad`).
- `Δα = |α_F| − |α_R|` : `> +1°` ⇒ sous-virage ; `< −1°` ⇒ survirage (seuils 0,5° en virage rapide). Si `mLateralPatchVel` s'avère être une vitesse de glissement déjà relative (c'est le cas : « lateral velocity at contact patch »), la formule est directe.

**(c) Saturation d'adhérence** [C=M]
- `G_F = moyenne(mGripFract FL, FR)`, `G_R = moyenne(RL, RR)` ; `ΔG = G_F − G_R`.
- `ΔG > +0,15` ⇒ l'avant glisse davantage (sous-virage) ; `< −0,15` ⇒ survirage.
- Complément : chute de `mSteeringShaftTorque` alors que `SWA` augmente ⇒ dépassement du pic de pneumatic trail AV (sous-virage imminent).

**Attribution par phase et par vitesse** : agréger (médiane sur ≥ 3 tours propres) chaque estimateur par (virage, phase). Produire une matrice `balance[phase ∈ {entrée, milieu, sortie}][vitesse ∈ {lent, moyen, rapide}] ∈ [−1, +1]` (−1 survirage fort, +1 sous-virage fort). Les règles de la section C consomment cette matrice.

**Signatures particulières** :
- *Survirage à la remise des gaz* : `US < 0` apparaissant dans les 0,5 s suivant `throttle > 0,3`, avec `κ_RL ≠ κ_RR` (roue intérieure qui patine ⇒ différentiel/power) ou les deux (⇒ TC/rigidité AR/appui). [C=M]
- *Survirage au lâcher de frein / entrée* : `US < 0` pendant que `brake` décroît et `a_x` reste négatif ⇒ transfert de charge, coast diff, rebound AV trop fort / bump AR, migration de frein. [C=M]
- *Sous-virage de sortie « on-throttle » LMH* : lié au déploiement sur l'essieu avant au-dessus de 150/190 km/h ⇒ motor map, pas suspension. [C=H]
- *Snap oversteer* : `|mLocalRotAccel.y|` > seuil (ex. > 1,5 rad/s²) avec correction volant > 40° en < 200 ms ⇒ voiture « pointue » (amortissement rapide AR, barre AR, packers AR, aileron). [C=M]

Sources : Milliken & Milliken, *Race Car Vehicle Dynamics* (chap. 5 « Simplified steady-state stability and control », définition du gradient de sous-virage) ; Carroll Smith, *Tune to Win* (chap. « Understeer and oversteer ») ; https://github.com/coasting-nc/LMUFFB (détection de saturation par chute de couple de crémaillère) ; https://domaslau.github.io/rF2SharedMemoryNet/api/rF2SharedMemoryNet.RF2Data.Structs.Wheel.html

### B.5 Diagnostics plateforme : talonnage, hauteurs, 3e élément

| ID | Diagnostic | Méthode | Données | Seuils | Conf. |
|---|---|---|---|---|---|
| D-PLT-01 | **Talonnage (bottoming)** | Événement si `mRideHeight_i < h_min` (ex. 3 mm) **ou** `mSuspensionDeflection_i` ≥ 98 % de la course max observée + pic `|a_z| > 1,5 g` sur < 50 ms. Compter par tour et localiser (`mLapDist`). Pour Hypercar : chute brutale de `mFrontDownforce` (> 20 % en < 0,2 s à vitesse constante) = **décrochage de fond plat**. | `mRideHeight`, déflexions, `mLocalAccel.y`, appuis | > 2 événements/tour hors vibreurs ⇒ monter la hauteur (+2 mm), packers (+2 mm), 3e élément/ressorts plus raides. | M |
| D-PLT-02 | **Rake dynamique** | `rake(t) = mRearRideHeight − mFrontRideHeight` ; moyennes en ligne droite à V max, en freinage max, en appui max. | hauteurs AV/AR | Rake qui s'inverse (< 0) en freinage ⇒ avant qui plonge trop (bump lent AV, ressorts AV, 3e élément AV). Rake qui grimpe en sortie ⇒ squat AR. | M |
| D-PLT-03 | **Utilisation de la course** | Histogramme de `mSuspensionDeflection` par roue ; % du temps sur packer. | déflexions | > 10 % du tour sur butée ⇒ packer trop court ou ressort trop mou (voiture « sur les butées ») — effet : raideur soudaine, perte de grip sur bosses. | M |
| D-PLT-04 | **Roulis & répartition AV/AR du roulis** | `φ ≈ (defl_L − defl_R)/voie` par essieu ; `roll_ratio = φ_F/φ_R`. | déflexions | Écart AV/AR important ⇒ barres déséquilibrées ; roulis total élevé en virage rapide ⇒ perte d'appui (Hypercar). | M |
| D-PLT-05 | **Transfert de charge, roue intérieure délestée** | `mTireLoad` roue intérieure < 10 % de la charge statique pendant > 0,3 s. | charges | Intérieure AV délestée en milieu ⇒ barre AV trop raide / ressort AV ; intérieure AR délestée en sortie ⇒ patinage intérieur ⇒ barre AR trop raide, diff power, précharge. | M |

### B.6 Histogrammes de vitesse d'amortisseur

Méthode (HP Academy / OptimumG, transposable 1:1 au canal `mSuspensionDeflection` à 100 Hz) [C=H méthode] :
1. `v_damper = d(mSuspensionDeflection)/dt` (dérivée centrale, lissage 3 points), convention **+ = compression (bump)**, − = détente (rebound).
2. Histogramme en % du temps du tour, bins de 20 mm/s (sim : 10 mm/s acceptable), plage ±250 mm/s. Tours complets et rapides seulement.
3. Bandes : 0–5 mm/s friction ; **5–25 mm/s = zone « lente »** (équilibre transitoire, contrôle de plateforme/aéro, c'est là qu'agissent les crans *slow*) ; 25–200 mm/s bosses ; > 200 mm/s vibreurs/chocs (*fast*).
4. **Critères** : (a) symétrie bump/rebound par roue (énergie absorbée similaire dans chaque sens) ; (b) symétrie gauche/droite d'un même essieu ; (c) une voiture à fort appui de fond plat peut volontairement avoir plus de rebound lent (maintien d'assiette).
5. **Lecture** : histogramme asymétrique vers le rebound ⇒ + bump lent ou − rebound lent (et inversement) ; pointe trop étroite et haute dans 0–25 mm/s avec plateau faible ⇒ très amorti (voiture « nerveuse ») ; distribution large et plate ⇒ sous-amorti (« flotte »). Un changement doit déplacer visiblement l'histogramme au-delà du bruit tour-à-tour, sinon il est trop petit.
6. Indicateurs numériques pour le moteur : `skew = (%bump − %rebound)` dans la bande lente (cible |skew| < 5 points) ; `pct_slow = % temps |v| < 25 mm/s` ; `pct_fast = % temps |v| > 100 mm/s`, à comparer entre essieux et entre circuits (Sebring/Interlagos : `pct_fast` élevé attendu).

Source : https://www.hpacademy.com/courses/professional-motorsport-data-analysis/damper-analysis-damping-histogram/

### B.7 Diagnostics de motricité

| ID | Diagnostic | Méthode | Données | Seuils | Conf. |
|---|---|---|---|---|---|
| D-TRC-01 | **Patinage en sortie** | Slip ratio AR `κ_RL, κ_RR > +0,10` pendant que `throttle > 0,5` en phase sortie ; durée cumulée/tour. | rotation, vitesses patch | > 0,5 s/tour cumulée ⇒ motricité insuffisante. | H |
| D-TRC-02 | **Patinage intérieur seul (diff ouvert)** | `|κ_RL − κ_RR| > 0,08` avec charge intérieure faible. | idem + `mTireLoad` | ⇒ + power lock, + précharge, − barre AR. | M |
| D-TRC-03 | **Taux d'intervention TC** | `TC_cut = mUnfilteredThrottle − mFilteredThrottle` ; % temps de sortie avec `TC_cut > 0,05`, et énergie coupée `∫TC_cut dt`. | pédales | > 25 % du temps de sortie ⇒ soit TC trop intrusif (perte de perf), soit voiture qui patine vraiment (voir D-TRC-01 pour trancher). | M |
| D-TRC-04 | **Survirage de puissance vs survirage d'appui** | Si l'excès de lacet apparaît avec `κ_R` élevé ⇒ puissance ; si `κ_R` faible mais `α_R` élevé ⇒ manque de grip latéral AR (aéro, barre, ressorts, pressions). | combinaison | — | M |
| D-TRC-05 | **Effet du déploiement hybride** | Corréler patinage/variation de lacet à `mElectricBoostMotorTorque` (LMDh : à-coup de couple AR ; LMH : sous-virage > 150/190 km/h). | hybride | — | M |

### B.8 Régularité, consommation, stratégie

| ID | Diagnostic | Méthode | Conf. |
|---|---|---|---|
| D-CON-01 | Régularité : `σ(laptime)` sur tours propres, et `σ` par secteur ; indice `CV = σ/mean`. CV secteur > 2× les autres ⇒ secteur « difficile » (lier aux virages). | H |
| D-CON-02 | Consommation carburant : `ΔmFuel` par tour (médiane), autonomie ; énergie virtuelle : REST `/rest/strategy/usage` par tour ; ratio `MJ/tour` vs `MaxStintEnergy` ⇒ nombre de tours par relais ; comparer `L/tour` au ratio énergie ⇒ ajuster `fuel ratio`. | H |
| D-CON-03 | Hybride : SOC min/max par tour ; temps à SOC ≥ 98 % (perte de régén) ; temps à SOC ≤ 5 % ⇒ recommander motor map ± et regen. Cible : démarrer le relais ≈ 60–75 %, rester entre 20 et 90 %. | H |
| D-CON-04 | Usure projetée : `wear_end = wear_now − Δwear/lap × laps_restants` par roue ; alerte si < seuil (ex. 0,2). | M |

### B.9 Indicateurs de style de pilotage

| ID | Indicateur | Formule | Interprétation pour le setup | Conf. |
|---|---|---|---|---|
| S-01 | **Agressivité au freinage** | pic `mUnfilteredBrake` ; temps 10→90 % (ms) ; pente max (1/s). | Pic > 0,95 et montée < 150 ms = attaque forte ⇒ voiture sensible aux blocages, préférer migration plus faible / répartition avant / ABS plus intrusif. | M |
| S-02 | **Trail braking** | % de la phase entrée avec `brake > 0,05` **et** `|a_y| > 0,3 g` ; chevauchement frein/volant ; frein résiduel à l'apex. | Trail braking marqué ⇒ pilote qui tourne la voiture au frein ⇒ accepte un peu de survirage d'entrée ; peu de trail ⇒ a besoin d'une voiture qui tourne seule (barre AR, coast ouvert). | M |
| S-03 | **Lissage du volant** | RMS de `d(SWA)/dt` en milieu de virage ; énergie spectrale > 2 Hz ; nombre d'inversions de signe du couple volant par virage. | Volant « nerveux » ⇒ voiture trop pointue ou pilote qui cherche le grip ⇒ setup sûr. | M |
| S-04 | **Usage des vibreurs** | % temps `mSurfaceType == 5` par tour ; pics `|a_z|` associés ; perte de stabilité (ψ̈) après vibreur. | Gros usage ⇒ amortissement rapide plus souple, packers plus longs, ressorts moins raides, hauteur un peu plus haute. | M |
| S-05 | **Application des gaz** | temps apex → 100 % gaz ; linéarité ; nombre de relâchements partiels (« pumping »). | Gaz brutaux ⇒ TC plus haut, diff power plus fermé, barre AR plus souple. | M |
| S-06 | **Chevauchement frein/gaz, rétrogradages** | rétrogradage pendant blocage ; régime à l'entrée. | Blocages AR aux rétrogradages ⇒ engine braking/coast/map. | M |
| S-07 | **Marge au grip** | moyenne de `mGripFract` en milieu de virage (0,6–0,8 = au limite ; < 0,4 = sous-exploité). | Pilote loin de la limite ⇒ données d'équilibre peu fiables, pondérer le ressenti. | M |
| S-08 | **Profil de vitesse mini** | V_apex vs référence (meilleur tour / référence externe). | Sous-virage « de pilotage » : entrée trop rapide ⇒ ne pas corriger au setup. | M |

Sources générales B : Carroll Smith, *Tune to Win* ; Milliken RCVD ; https://www.hpacademy.com/courses/professional-motorsport-data-analysis/damper-analysis-damping-histogram/ ; https://github.com/coasting-nc/LMUFFB ; https://simracingcockpit.gg/le-mans-ultimate-how-to-drive-the-lmgt3-and-hypercar/

---
## C. Règles « symptôme → modification » (base exploitable par programme)

### C.0 Schéma et conventions

```text
# Schéma d'une règle (pseudo-YAML descriptif ; les blocs de règles C.1–C.8 sont, eux, du YAML valide)
rule:
  id: string                 # unique, préfixe par famille (US, OS, BRK, TRC, PLT, TYR, STR, WET, PHI)
  symptom: string            # identifiant de symptôme (réutilisé par le dictionnaire D)
  phase: entry|mid|exit|braking|straight|any
  corner_speed: slow|medium|fast|any
  surface: dry|wet|any
  classes: [HY, LMP2, GT3]   # HY = Hypercar (LMH+LMDh)
  philosophy: both|aggressive|safe   # quelle philosophie privilégie cette règle (both = neutre)
  trigger:                   # condition sur la matrice de diagnostics (section B) ou sur le ressenti (D)
    telemetry: string        # expression informelle, à implémenter
    feel: [symptom ids]      # ressentis mappés (D)
  changes:                   # ordonnées par priorité décroissante (1 = essayer d'abord)
    - {prio: 1, p: <param>, dir: "+|-", mag: "<pas>", conf: H|M|L, side: "<effets secondaires>", conflicts: [<params ou rule ids>]}
  justification: string
  sources: [urls]

# Dictionnaire des paramètres canoniques (p)
params:
  aero.rear_wing            # cran/°   (+ = plus d'appui AR)
  aero.front_flap           # cran     (+ = plus d'appui AV ; si absent → utiliser rake)
  aero.rh_front             # mm       (+ = plus haut)
  aero.rh_rear              # mm
  aero.rake                 # = rh_rear − rh_front (dérivé)
  susp.spring_front / susp.spring_rear          # cran (+ = plus raide)
  susp.third_front / susp.third_rear            # 3e élément, cran (+ = plus raide) — prototypes
  susp.packer_front / susp.packer_rear          # mm (+ = butée plus longue = course restante plus courte)
  susp.arb_front / susp.arb_rear                # cran (+ = plus raide)
  susp.sbump_front / susp.sbump_rear            # bump lent, clics (+ = plus ferme)
  susp.srebound_front / susp.srebound_rear      # rebound lent
  susp.fbump_front / susp.fbump_rear            # bump rapide
  susp.frebound_front / susp.frebound_rear      # rebound rapide
  geo.camber_front / geo.camber_rear            # ° (+ = plus négatif, i.e. « plus de carrossage »)
  geo.toe_front / geo.toe_rear                  # ° (+ = plus de pince (toe-in) ; − = ouverture)
  geo.caster                                    # °
  tyre.pressure_front / tyre.pressure_rear      # kPa (par roue possible)
  tyre.compound                                 # soft|medium|hard|wet
  brake.bias_front                              # % AV (+ = plus avant)
  brake.migration                               # 0–2,5 (HY)
  brake.pressure                                # % force pédale max
  brake.duct_front / brake.duct_rear            # cran (+ = plus ouvert = plus froid)
  brake.abs_map                                 # GT3 (+ = plus intrusif)
  diff.preload                                  # Nm
  diff.power / diff.coast                       # % verrouillage (+ = plus fermé)
  elec.tc / elec.tc_cut / elec.tc_slip          # 1–11 (+ = plus d'intervention pour tc et cut ; pour slip : + = plus de glissement toléré avant intervention → voir note)
  elec.motor_map                                # kW de déploiement (HY)
  elec.regen                                    # niveau (HY)
  elec.engine_map                               # mixture (+ = plus riche/perf)
  energy.fuel_ratio / energy.virtual_energy / fuel.load
  steer.lock                                    # °
# Note elec.tc_slip : le guide officiel décrit « Slip Angle » comme la dérive latérale cible avant activation
# et la communauté lit « slip toléré » ; dans les deux lectures, une valeur basse = intervention plus tôt.
```

**Magnitudes par défaut d'un « cran d'ingénieur »** (à utiliser quand `mag` dit « 1 pas ») [C=M] : barre ±1 cran ; ressort ±1 cran (≈ ±5–10 %) ; amortisseur ±2 clics ; hauteur ±2 mm (Hypercar AV : ±1 mm) ; aileron ±1 cran (GT3 : ±0,5–1°) ; pression ±3 kPa (≈ ±0,4 psi) ; carrossage ±0,2° ; pince ±0,05° ; répartition de frein ±0,5–1 % ; migration ±0,5 ; précharge ±15 Nm ; power/coast ±1 cran (≈ ±5–10 %) ; TC ±1 ; motor map ±10 kW (LMH) / ±5 kW (LMDh) ; packer ±2 mm.

### C.1 Sous-virage (US)

```yaml
- id: US-ENTRY-SLOW-01
  symptom: US_ENTRY_SLOW
  phase: entry
  corner_speed: slow
  surface: dry
  classes: [HY, LMP2, GT3]
  philosophy: both
  trigger: {telemetry: "balance[entry][slow] > +0.3 ET brake > 0.05 pendant la dérive", feel: [FEEL_FRONT_WONT_TURN_IN, FEEL_PUSH_ON_BRAKES]}
  changes:
    - {prio: 1, p: brake.bias_front, dir: "-", mag: "0.5–1 %", conf: H, side: "risque de blocage AR en fin de freinage, instabilité en ligne droite", conflicts: [BRK-LOCK-REAR-01]}
    - {prio: 2, p: diff.coast, dir: "-", mag: "1 cran", conf: M, side: "plus de rotation au lâcher de frein, peut devenir survireur en entrée rapide", conflicts: [OS-ENTRY-FAST-01]}
    - {prio: 3, p: susp.arb_front, dir: "-", mag: "1 cran", conf: H, side: "plus de roulis AV, réponse un peu plus lente, pneu AV ext. plus chargé", conflicts: [STR-LAZY-01]}
    - {prio: 4, p: susp.sbump_front, dir: "+", mag: "2 clics", conf: M, side: "transitoire : plus de charge AV à la plongée, voiture plus sèche sur bosses à l'entrée", conflicts: [PLT-BUMPY-01]}
    - {prio: 5, p: susp.srebound_rear, dir: "+", mag: "2 clics", conf: M, side: "délestage AR transitoire = rotation ; trop = AR léger au freinage", conflicts: [OS-ENTRY-SLOW-01]}
    - {prio: 6, p: geo.toe_front, dir: "-", mag: "0.05° (vers ouverture)", conf: M, side: "usure/chauffe AV, moins stable en ligne droite", conflicts: []}
    - {prio: 7, p: tyre.pressure_front, dir: "-", mag: "3 kPa (si > mini)", conf: M, side: "plus de chauffe carcasse AV, réponse moins vive", conflicts: [TYR-HOT-FRONT-01]}
    - {prio: 8, p: geo.camber_front, dir: "+", mag: "0.2° (si ΔIO < 12 °C)", conf: M, side: "intérieur AV plus chaud, freinage droit un peu moins bon", conflicts: [TYR-HOT-INNER-01]}
    - {prio: 9, p: brake.migration, dir: "-", mag: "0.5 (HY)", conf: M, side: "moins de charge AV à pleine pédale → moins de freinage pur, mais plus de rotation en trail", conflicts: []}
  justification: "En virage lent sous freinage, l'avant sature parce que la répartition et le transfert de charge lui demandent trop ; on redonne de la rotation par le frein, le coast du différentiel et la raideur en roulis AV avant de toucher à la géométrie."
  sources: ["https://coachdaveacademy.com/tutorials/the-ultimate-le-mans-ultimate-setup-guide/", "https://lemansultimate.com/?p=3955", "https://www.sau.com.au/forums/topic/302303-car-set-up-cause-and-effect-by-carroll-smith"]

- id: US-ENTRY-FAST-01
  symptom: US_ENTRY_FAST
  phase: entry
  corner_speed: fast
  surface: dry
  classes: [HY, LMP2, GT3]
  philosophy: both
  trigger: {telemetry: "balance[entry][fast] > +0.3", feel: [FEEL_FRONT_WONT_TURN_IN, FEEL_FRONT_LIGHT_HIGH_SPEED]}
  changes:
    - {prio: 1, p: aero.rake, dir: "+", mag: "+1–2 mm AR ou −1 mm AV", conf: H, side: "plus d'appui AV ; HY : risque de décrochage de fond plat/talonnage si AV trop bas", conflicts: [PLT-BOTTOM-01, PLT-STALL-01]}
    - {prio: 2, p: aero.front_flap, dir: "+", mag: "1 cran (si dispo)", conf: H, side: "un peu de traînée, AR relativement moins stable", conflicts: [OS-EXIT-FAST-01]}
    - {prio: 3, p: aero.rear_wing, dir: "-", mag: "1 cran", conf: H, side: "moins de stabilité AR en appui et au freinage, gain de Vmax", conflicts: [OS-MID-FAST-01, OS-ENTRY-FAST-01]}
    - {prio: 4, p: brake.bias_front, dir: "-", mag: "0.5 %", conf: M, side: "blocages AR possibles en freinage appuyé", conflicts: [BRK-LOCK-REAR-01]}
    - {prio: 5, p: susp.arb_front, dir: "-", mag: "1 cran", conf: M, side: "plus de roulis = plateforme aéro moins stable (HY)", conflicts: [PLT-ROLL-01]}
    - {prio: 6, p: susp.third_front, dir: "-", mag: "1 cran (prototypes)", conf: M, side: "AV plus bas en appui → plus d'appui AV mais talonnage possible", conflicts: [PLT-BOTTOM-01]}
    - {prio: 7, p: susp.sbump_front, dir: "+", mag: "2 clics", conf: L, side: "transitoire seulement", conflicts: []}
  justification: "En entrée rapide l'équilibre est aéro : d'abord le rake et les éléments d'appui, ensuite les transferts de charge."
  sources: ["https://coachdaveacademy.com/tutorials/the-ultimate-le-mans-ultimate-setup-guide/", "https://arnouthoekstra.substack.com/p/fixed-setups-are-a-blessing-in-lmu"]

- id: US-MID-SLOW-01
  symptom: US_MID_SLOW
  phase: mid
  corner_speed: slow
  surface: dry
  classes: [HY, LMP2, GT3]
  philosophy: both
  trigger: {telemetry: "balance[mid][slow] > +0.3 ; ΔG = G_F − G_R > 0.15 ; T_front − T_rear > 6 °C", feel: [FEEL_FRONT_WONT_TURN, FEEL_PUSH_MID]}
  changes:
    - {prio: 1, p: susp.arb_front, dir: "-", mag: "1 cran", conf: H, side: "réponse moins immédiate, plus de roulis", conflicts: [STR-LAZY-01]}
    - {prio: 2, p: susp.arb_rear, dir: "+", mag: "1 cran", conf: H, side: "AR plus délesté à l'intérieur → patinage intérieur en sortie, moins tolérant sur vibreurs", conflicts: [TRC-INNER-01, PLT-KERB-01]}
    - {prio: 3, p: susp.spring_front, dir: "-", mag: "1 cran", conf: M, side: "plus de plongée au freinage, hauteur AV dynamique plus basse (HY : revérifier talonnage)", conflicts: [PLT-BOTTOM-01]}
    - {prio: 4, p: tyre.pressure_front, dir: "-", mag: "3 kPa", conf: M, side: "chauffe AV", conflicts: [TYR-HOT-FRONT-01]}
    - {prio: 5, p: geo.camber_front, dir: "+", mag: "0.2°", conf: M, side: "intérieur chaud, usure", conflicts: [TYR-HOT-INNER-01]}
    - {prio: 6, p: diff.preload, dir: "-", mag: "15 Nm", conf: M, side: "voiture plus libre en transition, moins stable en sortie", conflicts: [OS-EXIT-SLOW-01]}
    - {prio: 7, p: diff.power, dir: "-", mag: "1 cran", conf: M, side: "patinage intérieur en sortie", conflicts: [TRC-INNER-01]}
    - {prio: 8, p: geo.caster, dir: "+", mag: "0.5°", conf: L, side: "volant plus lourd, plus de gain de carrossage en braquage", conflicts: []}
    - {prio: 9, p: steer.lock, dir: "+", mag: "1 cran", conf: L, side: "direction plus vive (non physique, mais utile sur circuits lents)", conflicts: []}
  justification: "Virage lent = grip mécanique : répartition de raideur en roulis (barres) d'abord, ressorts ensuite, puis pneus/géométrie ; le différentiel compte car un diff serré pousse l'avant."
  sources: ["https://lemansultimate.wiki.gg/wiki/Setup_Guide", "https://coachdaveacademy.com/tutorials/the-ultimate-le-mans-ultimate-setup-guide/", "Carroll Smith, Tune to Win"]

- id: US-MID-FAST-01
  symptom: US_MID_FAST
  phase: mid
  corner_speed: fast
  surface: dry
  classes: [HY, LMP2, GT3]
  philosophy: both
  trigger: {telemetry: "balance[mid][fast] > +0.3 ; mFrontDownforce/mRearDownforce bas vs référence", feel: [FEEL_FRONT_WONT_TURN, FEEL_FRONT_LIGHT_HIGH_SPEED]}
  changes:
    - {prio: 1, p: aero.rear_wing, dir: "-", mag: "1 cran", conf: H, side: "moins de stabilité AR rapide, moins de traînée (Vmax +)", conflicts: [OS-MID-FAST-01]}
    - {prio: 2, p: aero.rake, dir: "+", mag: "+2 mm AR", conf: H, side: "HY : sensibilité ; LMP2 : idem", conflicts: [PLT-BOTTOM-01]}
    - {prio: 3, p: aero.front_flap, dir: "+", mag: "1 cran", conf: H, side: "traînée", conflicts: []}
    - {prio: 4, p: susp.arb_front, dir: "-", mag: "1 cran", conf: M, side: "roulis, plateforme", conflicts: []}
    - {prio: 5, p: susp.third_rear, dir: "+", mag: "1 cran (prototypes)", conf: M, side: "AR tenu plus haut en appui → rake dynamique +, mais moins de compliance AR", conflicts: [PLT-BUMPY-01]}
    - {prio: 6, p: susp.spring_rear, dir: "+", mag: "1 cran (GT3)", conf: M, side: "moins de motricité sur bosses", conflicts: [TRC-BOTH-01]}
  justification: "À haute vitesse, l'équilibre est dicté par la répartition d'appui et l'assiette dynamique."
  sources: ["https://coachdaveacademy.com/tutorials/the-ultimate-le-mans-ultimate-setup-guide/"]

- id: US-EXIT-SLOW-01
  symptom: US_EXIT_SLOW
  phase: exit
  corner_speed: slow
  surface: dry
  classes: [HY, LMP2, GT3]
  philosophy: both
  trigger: {telemetry: "balance[exit][slow] > +0.3 avec throttle > 0.5 ; κ_R faible", feel: [FEEL_PUSH_ON_THROTTLE]}
  changes:
    - {prio: 1, p: diff.power, dir: "-", mag: "1 cran", conf: H, side: "patinage intérieur, moins de stabilité à la remise des gaz", conflicts: [TRC-INNER-01]}
    - {prio: 2, p: diff.preload, dir: "-", mag: "15 Nm", conf: M, side: "idem, voiture plus libre partout", conflicts: [OS-EXIT-SLOW-01]}
    - {prio: 3, p: elec.motor_map, dir: "-", mag: "10–20 kW (LMH uniquement, si V > 150/190 km/h)", conf: H, side: "moins de déploiement = batterie se remplit → perte de régén", conflicts: [HYB-SOC-FULL-01]}
    - {prio: 4, p: susp.arb_rear, dir: "+", mag: "1 cran", conf: M, side: "patinage intérieur", conflicts: [TRC-INNER-01]}
    - {prio: 5, p: susp.srebound_front, dir: "-", mag: "2 clics", conf: M, side: "AV garde plus de charge à l'accélération → plus de rotation ; trop = AV qui remonte vite", conflicts: []}
    - {prio: 6, p: geo.toe_rear, dir: "-", mag: "0.05° (moins de pince)", conf: M, side: "AR moins stable en entrée", conflicts: [OS-ENTRY-SLOW-01]}
  justification: "Le sous-virage à la réaccélération vient d'un différentiel trop fermé (effet 'pousse-avant') ou, sur LMH, du déploiement sur l'essieu avant."
  sources: ["https://coachdaveacademy.com/tutorials/hypercar-electric-motor-maps-in-le-mans-ultimate-explained/", "https://lemansultimate.wiki.gg/wiki/Setup_Guide"]

- id: US-EXIT-FAST-01
  symptom: US_EXIT_FAST
  phase: exit
  corner_speed: fast
  surface: dry
  classes: [HY, LMP2, GT3]
  philosophy: both
  trigger: {telemetry: "balance[exit][fast] > +0.3", feel: [FEEL_PUSH_ON_THROTTLE]}
  changes:
    - {prio: 1, p: aero.rake, dir: "+", mag: "+1–2 mm AR", conf: M, side: "squat réduit l'effet ; vérifier hauteur AR mini", conflicts: []}
    - {prio: 2, p: susp.spring_rear, dir: "+", mag: "1 cran", conf: M, side: "moins de squat → garde l'appui AV ; motricité sur bosses −", conflicts: [TRC-BOTH-01]}
    - {prio: 3, p: susp.sbump_rear, dir: "+", mag: "2 clics", conf: M, side: "même mécanisme en transitoire", conflicts: [PLT-KERB-01]}
    - {prio: 4, p: aero.rear_wing, dir: "-", mag: "1 cran", conf: M, side: "stabilité", conflicts: [OS-EXIT-FAST-01]}
    - {prio: 5, p: diff.power, dir: "-", mag: "1 cran", conf: M, side: "stabilité", conflicts: []}
  justification: "En sortie rapide, le squat déplace l'équilibre aéro vers l'arrière ; on contrôle l'assiette avant de toucher le diff."
  sources: ["https://coachdaveacademy.com/tutorials/the-ultimate-le-mans-ultimate-setup-guide/"]
```

### C.2 Survirage (OS)

```yaml
- id: OS-ENTRY-SLOW-01
  symptom: OS_ENTRY_SLOW
  phase: entry
  corner_speed: slow
  surface: dry
  classes: [HY, LMP2, GT3]
  philosophy: both
  trigger: {telemetry: "balance[entry][slow] < −0.3 pendant brake décroissant ; corrections volant > 15°", feel: [FEEL_REAR_LOOSE_ON_BRAKES, FEEL_REAR_STEPS_OUT_TURN_IN]}
  changes:
    - {prio: 1, p: brake.bias_front, dir: "+", mag: "0.5–1 %", conf: H, side: "sous-virage d'entrée, blocages AV", conflicts: [US-ENTRY-SLOW-01, BRK-LOCK-FRONT-01]}
    - {prio: 2, p: diff.coast, dir: "+", mag: "1 cran", conf: H, side: "moins de rotation au lâcher, sous-virage d'entrée", conflicts: [US-ENTRY-SLOW-01]}
    - {prio: 3, p: susp.srebound_rear, dir: "-", mag: "2 clics", conf: M, side: "AR garde sa charge à la plongée ; trop mou = AR qui 'pompe'", conflicts: []}
    - {prio: 4, p: susp.sbump_front, dir: "-", mag: "2 clics", conf: M, side: "bite AV moins brutal", conflicts: [US-ENTRY-SLOW-01]}
    - {prio: 5, p: geo.toe_rear, dir: "+", mag: "0.05–0.10°", conf: H, side: "usure/chauffe AR, légère traînée ; stabilise aussi la sortie", conflicts: [US-EXIT-SLOW-01]}
    - {prio: 6, p: susp.arb_rear, dir: "-", mag: "1 cran", conf: H, side: "sous-virage milieu, plus de roulis AR", conflicts: [US-MID-SLOW-01]}
    - {prio: 7, p: diff.preload, dir: "+", mag: "15 Nm", conf: M, side: "sous-virage d'entrée/milieu", conflicts: [US-MID-SLOW-01]}
    - {prio: 8, p: brake.migration, dir: "+", mag: "0.5 (HY)", conf: M, side: "plus avant à pleine pédale : moins de rotation en trail", conflicts: []}
    - {prio: 9, p: elec.regen, dir: "check", mag: "si SOC ≥ 98 % : baisser motor_map, pas le setup", conf: H, side: "", conflicts: []}
  justification: "L'arrière se délestant au freinage, on le stabilise par la répartition, le coast, la pince AR et l'amortissement transitoire."
  sources: ["https://coachdaveacademy.com/tutorials/lmu-porsche-963-guide-the-inside-line/", "https://lemansultimate.com/?p=3955", "https://traxion.gg/le-mans-ultimate-how-to-master-hypercar-braking/"]

- id: OS-ENTRY-FAST-01
  symptom: OS_ENTRY_FAST
  phase: entry
  corner_speed: fast
  surface: dry
  classes: [HY, LMP2, GT3]
  philosophy: both
  trigger: {telemetry: "balance[entry][fast] < −0.3 ; |ψ̈| élevé", feel: [FEEL_REAR_LOOSE_ON_BRAKES, FEEL_SNAP_HIGH_SPEED]}
  changes:
    - {prio: 1, p: aero.rear_wing, dir: "+", mag: "1 cran", conf: H, side: "Vmax −, sous-virage rapide", conflicts: [US-MID-FAST-01, STR-TOPSPEED-01]}
    - {prio: 2, p: aero.rake, dir: "-", mag: "−1–2 mm AR (ou +1 mm AV)", conf: H, side: "moins d'appui AV", conflicts: [US-ENTRY-FAST-01]}
    - {prio: 3, p: brake.bias_front, dir: "+", mag: "0.5 %", conf: M, side: "blocages AV", conflicts: []}
    - {prio: 4, p: susp.packer_rear, dir: "+", mag: "2 mm (ou 3e élément AR +1)", conf: M, side: "AR tenu en appui ; dur si trop", conflicts: [PLT-BUMPY-01]}
    - {prio: 5, p: susp.spring_rear, dir: "+", mag: "1 cran", conf: M, side: "motricité", conflicts: [TRC-BOTH-01]}
    - {prio: 6, p: susp.srebound_rear, dir: "-", mag: "2 clics", conf: M, side: "", conflicts: []}
    - {prio: 7, p: geo.toe_rear, dir: "+", mag: "0.05°", conf: M, side: "", conflicts: []}
  justification: "CDA 963 : « rear ride height plus bas + ressorts AR plus raides + packers AR » pour la stabilité haute vitesse ; l'aileron reste le premier levier."
  sources: ["https://coachdaveacademy.com/tutorials/lmu-porsche-963-guide-the-inside-line/", "https://coachdaveacademy.com/tutorials/the-ultimate-le-mans-ultimate-setup-guide/"]

- id: OS-MID-SLOW-01
  symptom: OS_MID_SLOW
  phase: mid
  corner_speed: slow
  surface: dry
  classes: [HY, LMP2, GT3]
  philosophy: both
  trigger: {telemetry: "balance[mid][slow] < −0.3 ; ΔG < −0.15", feel: [FEEL_REAR_LOOSE_MID]}
  changes:
    - {prio: 1, p: susp.arb_rear, dir: "-", mag: "1 cran", conf: H, side: "réponse plus lente, sous-virage si trop", conflicts: [US-MID-SLOW-01]}
    - {prio: 2, p: susp.arb_front, dir: "+", mag: "1 cran", conf: H, side: "sous-virage d'entrée", conflicts: [US-ENTRY-SLOW-01]}
    - {prio: 3, p: susp.spring_rear, dir: "-", mag: "1 cran", conf: M, side: "plus de squat, hauteur AR dynamique plus basse", conflicts: [PLT-BOTTOM-01]}
    - {prio: 4, p: tyre.pressure_rear, dir: "-", mag: "3 kPa", conf: M, side: "chauffe AR", conflicts: [TYR-HOT-REAR-01]}
    - {prio: 5, p: geo.camber_rear, dir: "+", mag: "0.2°", conf: M, side: "motricité en ligne un peu moindre, intérieur AR chaud", conflicts: []}
    - {prio: 6, p: diff.preload, dir: "+", mag: "15 Nm", conf: M, side: "", conflicts: []}
  justification: "Répartition de raideur en roulis vers l'avant pour recharger l'AR."
  sources: ["https://lemansultimate.wiki.gg/wiki/Setup_Guide", "Carroll Smith, Tune to Win"]

- id: OS-MID-FAST-01
  symptom: OS_MID_FAST
  phase: mid
  corner_speed: fast
  surface: dry
  classes: [HY, LMP2, GT3]
  philosophy: both
  trigger: {telemetry: "balance[mid][fast] < −0.3", feel: [FEEL_REAR_LOOSE_MID, FEEL_SNAP_HIGH_SPEED]}
  changes:
    - {prio: 1, p: aero.rear_wing, dir: "+", mag: "1 cran", conf: H, side: "Vmax", conflicts: [STR-TOPSPEED-01]}
    - {prio: 2, p: aero.rake, dir: "-", mag: "−2 mm AR", conf: H, side: "", conflicts: [US-MID-FAST-01]}
    - {prio: 3, p: aero.front_flap, dir: "-", mag: "1 cran", conf: M, side: "", conflicts: []}
    - {prio: 4, p: susp.third_rear, dir: "-", mag: "1 cran (prototypes)", conf: M, side: "AR descend plus en appui → moins de rake dynamique", conflicts: [PLT-BOTTOM-01]}
    - {prio: 5, p: susp.arb_rear, dir: "-", mag: "1 cran", conf: M, side: "", conflicts: []}
    - {prio: 6, p: susp.spring_rear, dir: "+", mag: "1 cran (HY si plateforme instable)", conf: L, side: "contre-intuitif : moins de variation d'assiette = appui AR plus constant", conflicts: [OS-MID-SLOW-01]}
  justification: "Survirage rapide = déficit d'appui AR ou assiette qui bascule ; v1.2 a d'ailleurs reculé l'équilibre aéro GT3 pour réduire les glissades sèches."
  sources: ["https://coachdaveacademy.com/tutorials/the-complete-guide-to-the-lmgt3-cars-in-le-mans-ultimate/", "https://coachdaveacademy.com/tutorials/the-ultimate-le-mans-ultimate-setup-guide/"]

- id: OS-EXIT-SLOW-01
  symptom: OS_EXIT_SLOW
  phase: exit
  corner_speed: slow
  surface: dry
  classes: [HY, LMP2, GT3]
  philosophy: both
  trigger: {telemetry: "balance[exit][slow] < −0.3 dans 0.5 s après throttle > 0.3 ; κ_R > 0.1", feel: [FEEL_REAR_SLIDES_ON_THROTTLE, FEEL_WHEELSPIN]}
  changes:
    - {prio: 1, p: elec.tc, dir: "+", mag: "1 (et/ou tc_cut +1, tc_slip −1)", conf: H, side: "perte de perf si trop, chauffe moins les AR", conflicts: [TRC-TC-INTRUSIVE-01]}
    - {prio: 2, p: diff.power, dir: "+", mag: "1 cran", conf: H, side: "sous-virage à la remise des gaz", conflicts: [US-EXIT-SLOW-01]}
    - {prio: 3, p: diff.preload, dir: "+", mag: "15 Nm", conf: M, side: "", conflicts: [US-MID-SLOW-01]}
    - {prio: 4, p: susp.arb_rear, dir: "-", mag: "1 cran", conf: H, side: "sous-virage milieu", conflicts: [US-MID-SLOW-01]}
    - {prio: 5, p: susp.spring_rear, dir: "-", mag: "1 cran", conf: M, side: "squat, hauteur AR", conflicts: []}
    - {prio: 6, p: susp.srebound_front, dir: "+", mag: "2 clics", conf: M, side: "délestage AV transitoire = moins de rotation sous gaz ; trop = AV qui 'colle' au sol et sous-vire", conflicts: [US-EXIT-SLOW-01]}
    - {prio: 7, p: susp.sbump_rear, dir: "-", mag: "2 clics si perte sur bosses/vibreurs ; +2 si asphalte lisse", conf: L, side: "voir B.6 : confirmer par histogramme", conflicts: []}
    - {prio: 8, p: geo.toe_rear, dir: "+", mag: "0.05°", conf: M, side: "", conflicts: []}
    - {prio: 9, p: tyre.pressure_rear, dir: "-", mag: "3 kPa", conf: M, side: "", conflicts: [TYR-HOT-REAR-01]}
    - {prio: 10, p: geo.camber_rear, dir: "-", mag: "0.2° (moins négatif)", conf: M, side: "moins de grip latéral AR milieu", conflicts: [OS-MID-SLOW-01]}
    - {prio: 11, p: elec.motor_map, dir: "-", mag: "5 kW (LMDh : couple AR instantané dès 0 km/h)", conf: M, side: "batterie se remplit", conflicts: [HYB-SOC-FULL-01]}
  justification: "Le survirage de puissance se traite d'abord par l'électronique et le différentiel, puis par la répartition de raideur, enfin par la géométrie/pressions."
  sources: ["https://simracingcockpit.gg/le-mans-ultimate-how-to-drive-the-lmgt3-and-hypercar/", "https://arnouthoekstra.substack.com/p/confused-by-tc-you-should-be", "https://coachdaveacademy.com/tutorials/the-ultimate-le-mans-ultimate-setup-guide/"]

- id: OS-EXIT-FAST-01
  symptom: OS_EXIT_FAST
  phase: exit
  corner_speed: fast
  surface: dry
  classes: [HY, LMP2, GT3]
  philosophy: both
  trigger: {telemetry: "balance[exit][fast] < −0.3", feel: [FEEL_REAR_SLIDES_ON_THROTTLE, FEEL_SNAP_HIGH_SPEED]}
  changes:
    - {prio: 1, p: aero.rear_wing, dir: "+", mag: "1 cran", conf: H, side: "Vmax", conflicts: [STR-TOPSPEED-01]}
    - {prio: 2, p: susp.spring_rear, dir: "-", mag: "1 cran", conf: M, side: "squat = moins d'appui AV (sous-virage sortie rapide)", conflicts: [US-EXIT-FAST-01]}
    - {prio: 3, p: diff.power, dir: "+", mag: "1 cran", conf: M, side: "", conflicts: []}
    - {prio: 4, p: elec.tc_slip, dir: "-", mag: "1", conf: M, side: "intervention plus tôt", conflicts: []}
    - {prio: 5, p: aero.rake, dir: "-", mag: "−1 mm AR", conf: M, side: "", conflicts: []}
  justification: "Même logique que OS-MID-FAST avec le diff en appui."
  sources: ["https://coachdaveacademy.com/tutorials/the-ultimate-le-mans-ultimate-setup-guide/"]
```

### C.3 Freinage (BRK)

```yaml
- id: BRK-UNSTABLE-01
  symptom: BRK_UNSTABLE_STRAIGHT
  phase: braking
  corner_speed: any
  surface: dry
  classes: [HY, LMP2, GT3]
  philosophy: both
  trigger: {telemetry: "D-BRK-05 : corrections volant > 10° répétées en freinage droit ; std(ψ̇) élevé", feel: [FEEL_UNSTABLE_BRAKING, FEEL_REAR_WANDERS_BRAKING]}
  changes:
    - {prio: 1, p: brake.bias_front, dir: "+", mag: "1 %", conf: H, side: "blocages AV, sous-virage d'entrée", conflicts: [BRK-LOCK-FRONT-01]}
    - {prio: 2, p: geo.toe_rear, dir: "+", mag: "0.05–0.10°", conf: H, side: "chauffe AR", conflicts: []}
    - {prio: 3, p: diff.coast, dir: "+", mag: "1 cran", conf: M, side: "moins de rotation d'entrée", conflicts: [US-ENTRY-SLOW-01]}
    - {prio: 4, p: susp.srebound_rear, dir: "-", mag: "2 clics", conf: M, side: "", conflicts: []}
    - {prio: 5, p: susp.sbump_front, dir: "-", mag: "2 clics", conf: M, side: "plongée plus rapide", conflicts: []}
    - {prio: 6, p: geo.toe_front, dir: "+", mag: "0.05° (si toe-out marqué)", conf: M, side: "turn-in moins vif", conflicts: [US-ENTRY-SLOW-01]}
    - {prio: 7, p: brake.migration, dir: "+", mag: "0.5 (HY)", conf: M, side: "", conflicts: []}
    - {prio: 8, p: elec.engine_map, dir: "check", mag: "frein moteur/rétrogradages : rétrograder plus tard", conf: L, side: "", conflicts: []}
  justification: "Instabilité en ligne droite = AR trop délesté ou trop freiné ; pince AR et répartition sont les remèdes classiques (Smith : 'rear toe-in too little → unstable')."
  sources: ["https://www.sau.com.au/forums/topic/302303-car-set-up-cause-and-effect-by-carroll-smith", "https://coachdaveacademy.com/tutorials/the-ultimate-le-mans-ultimate-setup-guide/"]

- id: BRK-LOCK-FRONT-01
  symptom: BRK_LOCK_FRONT
  phase: braking
  corner_speed: any
  surface: dry
  classes: [HY, LMP2, GT3]
  philosophy: both
  trigger: {telemetry: "D-BRK-03 : blocages AV ≫ AR, surtout en fin de freinage (basse vitesse, appui faible)", feel: [FEEL_FRONT_LOCKS]}
  changes:
    - {prio: 1, p: brake.bias_front, dir: "-", mag: "0.5–1 %", conf: H, side: "instabilité/blocages AR", conflicts: [BRK-UNSTABLE-01]}
    - {prio: 2, p: brake.migration, dir: "-", mag: "0.5 (HY)", conf: H, side: "moins de charge AV à pleine pédale", conflicts: []}
    - {prio: 3, p: brake.pressure, dir: "-", mag: "3–5 %", conf: M, side: "pédale plus longue, moins de modulation fine", conflicts: []}
    - {prio: 4, p: brake.abs_map, dir: "+", mag: "1 (GT3)", conf: M, side: "distances un peu plus longues", conflicts: []}
    - {prio: 5, p: geo.camber_front, dir: "-", mag: "0.2° (moins négatif)", conf: M, side: "moins de grip latéral AV", conflicts: [US-MID-SLOW-01]}
    - {prio: 6, p: susp.sbump_front, dir: "-", mag: "2 clics", conf: L, side: "plongée plus souple = moins de pic de charge", conflicts: []}
    - {prio: 7, p: tyre.pressure_front, dir: "-", mag: "3 kPa", conf: L, side: "", conflicts: []}
  justification: "Le blocage AV en fin de freinage est le cas standard Hypercar quand l'appui s'effondre ; migration et répartition sont faits pour ça. Vérifier SOC batterie (LMDh bloque l'AV à 100 %)."
  sources: ["https://traxion.gg/le-mans-ultimate-how-to-master-hypercar-braking/", "https://simracingcockpit.gg/le-mans-ultimate-how-to-drive-the-lmgt3-and-hypercar/"]

- id: BRK-LOCK-REAR-01
  symptom: BRK_LOCK_REAR
  phase: braking
  corner_speed: any
  surface: dry
  classes: [HY, LMP2, GT3]
  philosophy: both
  trigger: {telemetry: "blocages AR, souvent au rétrogradage ou en trail braking", feel: [FEEL_REAR_LOCKS]}
  changes:
    - {prio: 1, p: brake.bias_front, dir: "+", mag: "0.5–1 %", conf: H, side: "sous-virage d'entrée", conflicts: [US-ENTRY-SLOW-01]}
    - {prio: 2, p: brake.migration, dir: "+", mag: "0.5 (HY)", conf: M, side: "", conflicts: []}
    - {prio: 3, p: diff.coast, dir: "+", mag: "1 cran", conf: M, side: "", conflicts: [US-ENTRY-SLOW-01]}
    - {prio: 4, p: elec.motor_map, dir: "+", mag: "si SOC ≥ 98 % (LMH : régén coupée = AR bloque)", conf: H, side: "", conflicts: []}
    - {prio: 5, p: susp.srebound_rear, dir: "-", mag: "2 clics", conf: M, side: "", conflicts: []}
    - {prio: 6, p: brake.duct_rear, dir: "+/-", mag: "selon T° : AR froids → fermer 1 ; AR en fade → ouvrir 1", conf: M, side: "", conflicts: []}
  justification: "Blocage AR = répartition trop arrière ou AR délesté ; sur LMH, perte de régén à batterie pleine."
  sources: ["https://traxion.gg/le-mans-ultimate-how-to-master-hypercar-braking/", "https://traxion.gg/how-to-manage-virtual-energy-in-le-mans-ultimate/"]

- id: BRK-LOCK-INNER-01
  symptom: BRK_LOCK_INNER_FRONT
  phase: entry
  corner_speed: slow
  surface: dry
  classes: [HY, LMP2, GT3]
  philosophy: both
  trigger: {telemetry: "blocage de la roue AV intérieure seule en trail braking ; charge intérieure faible", feel: [FEEL_INNER_FRONT_LOCKS]}
  changes:
    - {prio: 1, p: susp.arb_front, dir: "-", mag: "1 cran", conf: M, side: "", conflicts: []}
    - {prio: 2, p: susp.srebound_front, dir: "-", mag: "2 clics (roue intérieure se détend plus vite = garde du contact)", conf: M, side: "", conflicts: []}
    - {prio: 3, p: brake.bias_front, dir: "-", mag: "0.5 %", conf: M, side: "", conflicts: []}
    - {prio: 4, p: geo.camber_front, dir: "-", mag: "0.2°", conf: L, side: "", conflicts: []}
    - {prio: 5, p: brake.abs_map, dir: "+", mag: "1 (GT3)", conf: M, side: "", conflicts: []}
  justification: "Smith : trop de rebound 'pulls the inside wheel off the road' ; un peu moins de raideur en roulis AV recharge l'intérieure."
  sources: ["https://www.sau.com.au/forums/topic/302303-car-set-up-cause-and-effect-by-carroll-smith"]

- id: BRK-COLD-01
  symptom: BRK_COLD
  phase: braking
  corner_speed: any
  surface: any
  classes: [HY, LMP2, GT3]
  philosophy: both
  trigger: {telemetry: "D-BRK-02 : T° disque sous la fenêtre au 1er freinage ; décélération max faible à froid", feel: [FEEL_NO_BRAKES_COLD, FEEL_LONG_PEDAL]}
  changes:
    - {prio: 1, p: brake.duct_front, dir: "-", mag: "1 cran (et AR si concerné)", conf: H, side: "risque de surchauffe en fin de relais / jour", conflicts: [BRK-HOT-01]}
    - {prio: 2, p: brake.bias_front, dir: "+", mag: "1 % au 1er tour puis revenir (réglage en roulant)", conf: M, side: "", conflicts: []}
    - {prio: 3, p: brake.pressure, dir: "+", mag: "3 %", conf: L, side: "", conflicts: []}
  justification: "v1.4 : courbes friction/température des plaquettes ; BMW M Hybrid : freins 'très sensibles à la température, mauvais à froid' (CDA)."
  sources: ["https://coachdaveacademy.com/tutorials/lmu-bmw-m-hybrid-v8-guide-the-inside-line/", "https://traxion.gg/latest-le-mans-ultimate-update-to-improve-hypercar-cold-tyre-performance-patch-notes-released/"]

- id: BRK-HOT-01
  symptom: BRK_OVERHEAT
  phase: braking
  corner_speed: any
  surface: dry
  classes: [HY, LMP2, GT3]
  philosophy: both
  trigger: {telemetry: "T° disque max > seuil chaud ; décélération qui décroît tour après tour (fade) ; LMH : répartition qui glisse vers l'AR en fin de relais", feel: [FEEL_BRAKES_FADE]}
  changes:
    - {prio: 1, p: brake.duct_front, dir: "+", mag: "1 cran", conf: H, side: "traînée (faible), pneus/jantes plus froids", conflicts: [BRK-COLD-01, TYR-COLD-FRONT-01]}
    - {prio: 2, p: brake.migration, dir: "-", mag: "0.5 (HY, si migration > 2.0)", conf: M, side: "", conflicts: []}
    - {prio: 3, p: elec.regen, dir: "max", mag: "s'assurer regen max et SOC < 90 % (régén = moins de travail frein)", conf: H, side: "", conflicts: []}
    - {prio: 4, p: brake.bias_front, dir: "-", mag: "0.5 % si AV seuls chauffent", conf: M, side: "", conflicts: []}
  justification: "Traxion : trop de migration peut surchauffer les freins ; CDA : fade AV LMH en long relais."
  sources: ["https://traxion.gg/le-mans-ultimate-how-to-master-hypercar-braking/", "https://simracingcockpit.gg/le-mans-ultimate-how-to-drive-the-lmgt3-and-hypercar/"]

- id: BRK-ABS-01
  symptom: ABS_TOO_ACTIVE
  phase: braking
  corner_speed: any
  surface: dry
  classes: [GT3]
  philosophy: both
  trigger: {telemetry: "D-BRK-04 : ABS actif > 30 % du temps de freinage", feel: [FEEL_ABS_PULSING, FEEL_LONG_BRAKING]}
  changes:
    - {prio: 1, p: brake.abs_map, dir: "-", mag: "1 (agressif) / +1 (sûr, si le pilote 'attaque' l'ABS en permanence)", conf: M, side: "", conflicts: []}
    - {prio: 2, p: brake.bias_front, dir: "±", mag: "0.5 % vers l'essieu qui ne déclenche pas", conf: M, side: "ne pas changer ABS et répartition en même temps", conflicts: []}
    - {prio: 3, p: brake.pressure, dir: "-", mag: "3 %", conf: L, side: "", conflicts: []}
  justification: "Guide officiel : choisir la map ABS puis ajuster la répartition ; freiner à la limite sans déclencher régulièrement."
  sources: ["https://guide.lemansultimate.com/hc/en-gb/articles/13211078435983-ABS-Anti-Lock-Braking-System-for-LMGT3-Cars"]
```

### C.4 Motricité et électronique (TRC / HYB)

```yaml
- id: TRC-BOTH-01
  symptom: TRC_WHEELSPIN_BOTH
  phase: exit
  corner_speed: slow
  surface: dry
  classes: [HY, LMP2, GT3]
  philosophy: both
  trigger: {telemetry: "D-TRC-01 : κ_RL ≈ κ_RR > 0.1 ; TC_cut fort", feel: [FEEL_WHEELSPIN, FEEL_NO_TRACTION]}
  changes:
    - {prio: 1, p: elec.tc, dir: "+", mag: "1", conf: H, side: "", conflicts: [TRC-TC-INTRUSIVE-01]}
    - {prio: 2, p: susp.arb_rear, dir: "-", mag: "1 cran", conf: H, side: "sous-virage milieu", conflicts: [US-MID-SLOW-01]}
    - {prio: 3, p: susp.spring_rear, dir: "-", mag: "1 cran", conf: M, side: "squat", conflicts: [US-EXIT-FAST-01]}
    - {prio: 4, p: tyre.pressure_rear, dir: "-", mag: "3 kPa", conf: M, side: "chauffe", conflicts: []}
    - {prio: 5, p: geo.camber_rear, dir: "-", mag: "0.2°", conf: M, side: "", conflicts: []}
    - {prio: 6, p: susp.fbump_rear, dir: "-", mag: "2 clics (si perte sur bosses)", conf: M, side: "", conflicts: []}
    - {prio: 7, p: elec.engine_map, dir: "-", mag: "1 (map plus douce)", conf: L, side: "perf", conflicts: []}
    - {prio: 8, p: aero.rear_wing, dir: "+", mag: "1 cran (si la perte est en sortie moyenne/rapide)", conf: M, side: "", conflicts: []}
  justification: "Les deux roues patinent : pas un problème de différentiel mais de charge/grip AR et de couple."
  sources: ["https://coachdaveacademy.com/tutorials/the-ultimate-le-mans-ultimate-setup-guide/"]

- id: TRC-INNER-01
  symptom: TRC_WHEELSPIN_INNER
  phase: exit
  corner_speed: slow
  surface: dry
  classes: [HY, LMP2, GT3]
  philosophy: both
  trigger: {telemetry: "D-TRC-02 : |κ_RL − κ_RR| > 0.08 ; charge intérieure < 10 %", feel: [FEEL_INNER_WHEELSPIN]}
  changes:
    - {prio: 1, p: diff.power, dir: "+", mag: "1 cran", conf: H, side: "pousse-avant à la remise des gaz", conflicts: [US-EXIT-SLOW-01]}
    - {prio: 2, p: diff.preload, dir: "+", mag: "15 Nm", conf: H, side: "", conflicts: [US-MID-SLOW-01]}
    - {prio: 3, p: susp.arb_rear, dir: "-", mag: "1 cran", conf: H, side: "", conflicts: []}
    - {prio: 4, p: susp.srebound_rear, dir: "-", mag: "2 clics (roue intérieure suit le sol)", conf: M, side: "", conflicts: []}
    - {prio: 5, p: elec.tc_slip, dir: "-", mag: "1", conf: M, side: "", conflicts: []}
  justification: "Smith : 'inside wheelspin → rear bar too stiff' + diff ; sur GT3 précharge seule parfois disponible."
  sources: ["https://www.sau.com.au/forums/topic/302303-car-set-up-cause-and-effect-by-carroll-smith", "https://lemansultimate.com/?p=3955"]

- id: TRC-TC-INTRUSIVE-01
  symptom: TC_TOO_INTRUSIVE
  phase: exit
  corner_speed: any
  surface: dry
  classes: [HY, LMP2, GT3]
  philosophy: aggressive
  trigger: {telemetry: "D-TRC-03 : TC_cut > 25 % du temps de sortie alors que κ_R < 0.05 (pas de vrai patinage)", feel: [FEEL_TC_CUTS_POWER, FEEL_SLUGGISH_EXIT]}
  changes:
    - {prio: 1, p: elec.tc_cut, dir: "-", mag: "1–2", conf: M, side: "moins de coupure quand ça intervient", conflicts: []}
    - {prio: 2, p: elec.tc_slip, dir: "+", mag: "1", conf: M, side: "intervient plus tard", conflicts: []}
    - {prio: 3, p: elec.tc, dir: "-", mag: "1 (jamais < 6-6-6 sans test)", conf: M, side: "tête-à-queue en virage lent", conflicts: []}
  justification: "Hoekstra : 7-7-7 base, 6-6-6 perf ; sous ~7 on perd plus qu'on ne gagne. Hypercar : TC & slip 9–11 avec cut 1–2 (lecture simracingcockpit)."
  sources: ["https://arnouthoekstra.substack.com/p/confused-by-tc-you-should-be", "https://simracingcockpit.gg/le-mans-ultimate-how-to-drive-the-lmgt3-and-hypercar/"]

- id: HYB-SOC-FULL-01
  symptom: HYB_SOC_FULL
  phase: any
  corner_speed: any
  surface: any
  classes: [HY]
  philosophy: both
  trigger: {telemetry: "D-CON-03 : temps à SOC ≥ 98 % > 5 % du tour ; blocages corrélés", feel: [FEEL_BRAKES_CHANGE_MID_STINT, FEEL_REAR_LOCKS]}
  changes:
    - {prio: 1, p: elec.motor_map, dir: "+", mag: "10 kW LMH / 5 kW LMDh", conf: H, side: "énergie virtuelle consommée via batterie (ratio)", conflicts: []}
    - {prio: 2, p: energy.fuel_ratio, dir: "-", mag: "0.01 (plus d'énergie prévue par la batterie)", conf: M, side: "moins de carburant embarqué", conflicts: []}
  justification: "Batterie pleine = régén coupée = perte de freinage et snaps ; cible 60–75 % en relais."
  sources: ["https://coachdaveacademy.com/tutorials/hypercar-electric-motor-maps-in-le-mans-ultimate-explained/", "https://www.trophi.ai/post/how-to-make-the-most-of-le-mans-ultimates-hybrid-systems"]

- id: HYB-SOC-EMPTY-01
  symptom: HYB_SOC_EMPTY
  phase: any
  corner_speed: any
  surface: any
  classes: [HY]
  philosophy: both
  trigger: {telemetry: "SOC ≤ 5 % > 5 % du tour ; LMDh : perte de couple bas régime", feel: [FEEL_NO_BOOST]}
  changes:
    - {prio: 1, p: elec.motor_map, dir: "-", mag: "10 kW LMH / 5 kW LMDh", conf: H, side: "", conflicts: []}
    - {prio: 2, p: elec.regen, dir: "max", mag: "", conf: H, side: "", conflicts: []}
  justification: "LMDh ne repart pas des stands à 0 % ; LMH perd le 4RM."
  sources: ["https://coachdaveacademy.com/tutorials/hypercar-electric-motor-maps-in-le-mans-ultimate-explained/"]
```

### C.5 Plateforme, bosses, vibreurs (PLT / STR)

```yaml
- id: PLT-BOTTOM-01
  symptom: PLT_BOTTOMING
  phase: any
  corner_speed: fast
  surface: dry
  classes: [HY, LMP2, GT3]
  philosophy: both
  trigger: {telemetry: "D-PLT-01 : > 2 talonnages/tour hors vibreurs ; hauteur mini < 3 mm", feel: [FEEL_BOTTOMING, FEEL_SPARKS_UNSETTLE]}
  changes:
    - {prio: 1, p: aero.rh_front, dir: "+", mag: "1–2 mm (ou AR selon l'extrémité qui touche)", conf: H, side: "perte d'appui (HY : importante)", conflicts: [US-ENTRY-FAST-01]}
    - {prio: 2, p: susp.packer_front, dir: "+", mag: "2 mm", conf: H, side: "butée dure plus tôt : perte de grip si on la touche en appui", conflicts: [PLT-BUMPY-01]}
    - {prio: 3, p: susp.third_front, dir: "+", mag: "1 cran (prototypes)", conf: H, side: "moins de compliance en heave", conflicts: []}
    - {prio: 4, p: susp.spring_front, dir: "+", mag: "1 cran (GT3)", conf: M, side: "moins de grip mécanique", conflicts: [US-MID-SLOW-01]}
    - {prio: 5, p: susp.sbump_front, dir: "+", mag: "2 clics", conf: M, side: "plus sec", conflicts: []}
  justification: "LMP3 guide CDA : « packers pour empêcher le talonnage sans changer la hauteur statique » ; HY : « trop bas = talonnage en zone rapide »."
  sources: ["https://lemansultimate.com/?p=3955", "https://coachdaveacademy.com/tutorials/the-ultimate-le-mans-ultimate-setup-guide/"]

- id: PLT-STALL-01
  symptom: PLT_FLOOR_STALL
  phase: any
  corner_speed: fast
  surface: dry
  classes: [HY, LMP2]
  philosophy: both
  trigger: {telemetry: "chute > 20 % de mFrontDownforce en < 0.2 s à vitesse quasi constante ; hauteur AV très basse ; perte soudaine d'équilibre", feel: [FEEL_SUDDEN_LOSS_HIGH_SPEED, FEEL_BALANCE_SHIFT_UNPREDICTABLE]}
  changes:
    - {prio: 1, p: aero.rh_front, dir: "+", mag: "1–2 mm", conf: H, side: "appui", conflicts: []}
    - {prio: 2, p: susp.third_front, dir: "+", mag: "1 cran", conf: H, side: "", conflicts: []}
    - {prio: 3, p: susp.spring_front, dir: "+", mag: "1 cran", conf: M, side: "", conflicts: []}
    - {prio: 4, p: susp.srebound_front, dir: "+", mag: "2 clics (maintien d'assiette, cf. B.6 exception aéro)", conf: M, side: "", conflicts: []}
  justification: "CDA : « suspension trop souple → floor stalls ou bascules d'équilibre imprévisibles » (Hypercar)."
  sources: ["https://coachdaveacademy.com/tutorials/the-ultimate-le-mans-ultimate-setup-guide/"]

- id: PLT-KERB-01
  symptom: PLT_KERB_UNSETTLED
  phase: any
  corner_speed: any
  surface: dry
  classes: [HY, LMP2, GT3]
  philosophy: both
  trigger: {telemetry: "S-04 : pics |a_z| et ψ̈ après mSurfaceType==5 ; perte de ψ̇ contrôlé", feel: [FEEL_JUMPS_ON_KERBS, FEEL_KERBS_KICK_REAR]}
  changes:
    - {prio: 1, p: susp.fbump_rear, dir: "-", mag: "2 clics (et AV si concerné)", conf: H, side: "plus de mouvement sur gros chocs", conflicts: []}
    - {prio: 2, p: susp.frebound_rear, dir: "-", mag: "2 clics", conf: M, side: "roue qui 'rebondit' si trop mou", conflicts: []}
    - {prio: 3, p: susp.arb_rear, dir: "-", mag: "1 cran", conf: H, side: "CDA : barre AR raide = AR imprévisible sur vibreurs", conflicts: [US-MID-SLOW-01]}
    - {prio: 4, p: susp.packer_rear, dir: "-", mag: "2 mm (plus de course)", conf: M, side: "talonnage", conflicts: [PLT-BOTTOM-01]}
    - {prio: 5, p: susp.spring_rear, dir: "-", mag: "1 cran", conf: M, side: "", conflicts: []}
    - {prio: 6, p: aero.rh_rear, dir: "+", mag: "2 mm", conf: M, side: "appui", conflicts: []}
    - {prio: 7, p: diff.preload, dir: "+", mag: "15 Nm (si le kick vient d'un délestage d'une roue motrice)", conf: L, side: "", conflicts: []}
  justification: "Vibreurs = hautes vitesses d'amortisseur : agir sur fast bump/rebound, puis sur la raideur en roulis."
  sources: ["https://coachdaveacademy.com/tutorials/the-ultimate-le-mans-ultimate-setup-guide/", "https://www.hpacademy.com/courses/professional-motorsport-data-analysis/damper-analysis-damping-histogram/"]

- id: PLT-BUMPY-01
  symptom: PLT_BUMPY_LOSS
  phase: any
  corner_speed: any
  surface: dry
  classes: [HY, LMP2, GT3]
  philosophy: both
  trigger: {telemetry: "pct_fast élevé (B.6) ; grip fraction qui oscille avec a_z ; circuits type Sebring/Interlagos", feel: [FEEL_SKIPS_OVER_BUMPS, FEEL_LOSES_GRIP_ON_BUMPS]}
  changes:
    - {prio: 1, p: susp.fbump_front, dir: "-", mag: "2 clics (+ AR)", conf: H, side: "", conflicts: []}
    - {prio: 2, p: susp.spring_front, dir: "-", mag: "1 cran (+ AR)", conf: M, side: "talonnage, plateforme aéro", conflicts: [PLT-BOTTOM-01, PLT-STALL-01]}
    - {prio: 3, p: susp.frebound_front, dir: "-", mag: "2 clics", conf: M, side: "", conflicts: []}
    - {prio: 4, p: aero.rh_front, dir: "+", mag: "2 mm (+ AR)", conf: M, side: "appui", conflicts: []}
    - {prio: 5, p: susp.arb_front, dir: "-", mag: "1 cran (+ AR)", conf: M, side: "", conflicts: []}
    - {prio: 6, p: tyre.pressure_front, dir: "-", mag: "3 kPa (+ AR)", conf: M, side: "", conflicts: []}
  justification: "Steam/communauté : « softer front fast bump adds grip on tight/bumpy tracks » ; Smith : 'too much high-speed bump → tyres hop over ripples'."
  sources: ["https://steamcommunity.com/app/2399420/discussions/0/4853281047676507449", "https://www.sau.com.au/forums/topic/302303-car-set-up-cause-and-effect-by-carroll-smith"]

- id: PLT-FLOATY-01
  symptom: PLT_FLOATY
  phase: any
  corner_speed: any
  surface: dry
  classes: [HY, LMP2, GT3]
  philosophy: both
  trigger: {telemetry: "histogramme large/plat ; oscillations de hauteur après bosse > 1 cycle ; pct_slow faible", feel: [FEEL_WALLOWS, FEEL_FLOATY, FEEL_SLOW_TO_SETTLE]}
  changes:
    - {prio: 1, p: susp.srebound_front, dir: "+", mag: "2 clics (+ AR)", conf: H, side: "", conflicts: []}
    - {prio: 2, p: susp.sbump_front, dir: "+", mag: "2 clics (+ AR)", conf: H, side: "", conflicts: []}
    - {prio: 3, p: susp.spring_front, dir: "+", mag: "1 cran (+ AR)", conf: M, side: "", conflicts: [PLT-BUMPY-01]}
  justification: "CDA : « soft dampers → the car wallows » ; Smith : 'too little overall → floating, oscillation'."
  sources: ["https://coachdaveacademy.com/tutorials/the-ultimate-le-mans-ultimate-setup-guide/"]

- id: PLT-NERVOUS-01
  symptom: PLT_NERVOUS
  phase: any
  corner_speed: any
  surface: dry
  classes: [HY, LMP2, GT3]
  philosophy: safe
  trigger: {telemetry: "S-03 : volant nerveux ; |ψ̈| élevé ; histogramme très pointu", feel: [FEEL_SKITTISH, FEEL_NERVOUS, FEEL_TOO_POINTY]}
  changes:
    - {prio: 1, p: susp.sbump_front, dir: "-", mag: "2 clics (+ AR)", conf: H, side: "", conflicts: [PLT-FLOATY-01]}
    - {prio: 2, p: susp.srebound_front, dir: "-", mag: "2 clics (+ AR)", conf: M, side: "", conflicts: []}
    - {prio: 3, p: susp.arb_front, dir: "-", mag: "1 cran (+ AR)", conf: M, side: "", conflicts: []}
    - {prio: 4, p: geo.toe_front, dir: "+", mag: "0.05° (réduire l'ouverture)", conf: M, side: "", conflicts: []}
    - {prio: 5, p: steer.lock, dir: "-", mag: "1 cran", conf: L, side: "", conflicts: []}
  justification: "CDA : « stiff dampers → nervous, skittish » ; Smith : 'too much low-speed bump → reactions too sudden'."
  sources: ["https://coachdaveacademy.com/tutorials/the-ultimate-le-mans-ultimate-setup-guide/"]

- id: STR-LAZY-01
  symptom: STR_SLOW_DIRECTION_CHANGE
  phase: any
  corner_speed: medium
  surface: dry
  classes: [HY, LMP2, GT3]
  philosophy: aggressive
  trigger: {telemetry: "retard ψ̇ vs SWA > 150 ms en chicane ; roulis élevé", feel: [FEEL_LAZY, FEEL_SLOW_IN_CHICANES]}
  changes:
    - {prio: 1, p: susp.arb_front, dir: "+", mag: "1 cran (+ AR)", conf: H, side: "sous-virage si AV seul", conflicts: [US-MID-SLOW-01]}
    - {prio: 2, p: susp.sbump_front, dir: "+", mag: "2 clics (+ AR)", conf: M, side: "", conflicts: [PLT-NERVOUS-01]}
    - {prio: 3, p: susp.spring_front, dir: "+", mag: "1 cran (+ AR)", conf: M, side: "", conflicts: [PLT-BUMPY-01]}
    - {prio: 4, p: tyre.pressure_front, dir: "+", mag: "3 kPa", conf: M, side: "moins de grip mécanique", conflicts: []}
    - {prio: 5, p: geo.toe_front, dir: "-", mag: "0.05°", conf: M, side: "", conflicts: [BRK-UNSTABLE-01]}
  justification: "Smith : 'bars too soft → lazy, reluctant to change direction in chicanes'."
  sources: ["https://www.sau.com.au/forums/topic/302303-car-set-up-cause-and-effect-by-carroll-smith"]

- id: STR-TOPSPEED-01
  symptom: STR_TOP_SPEED_LOW
  phase: straight
  corner_speed: any
  surface: dry
  classes: [HY, LMP2, GT3]
  philosophy: both
  trigger: {telemetry: "Vmax < référence − 3 km/h ; mDrag élevé à V donnée ; régime max non atteint", feel: [FEEL_SLOW_ON_STRAIGHTS]}
  changes:
    - {prio: 1, p: aero.rear_wing, dir: "-", mag: "1 cran (GT3 : respecter l'angle mini BoP)", conf: H, side: "stabilité rapide, freinage", conflicts: [OS-MID-FAST-01, OS-ENTRY-FAST-01]}
    - {prio: 2, p: aero.front_flap, dir: "-", mag: "1 cran (pour garder l'équilibre)", conf: M, side: "", conflicts: []}
    - {prio: 3, p: aero.rake, dir: "-", mag: "−1 mm AR", conf: M, side: "moins d'appui AV", conflicts: []}
    - {prio: 4, p: brake.duct_front, dir: "-", mag: "1 cran si freins froids", conf: L, side: "", conflicts: [BRK-HOT-01]}
    - {prio: 5, p: elec.motor_map, dir: "+", mag: "LMDh : 50 kW ; LMH : déploiement > 190 km/h", conf: M, side: "batterie", conflicts: []}
  justification: "Hoekstra : setups bas appui (< 30 % d'aile) 'crazy fast' sur circuits à longues lignes droites ; BoP fixe l'angle mini GT3."
  sources: ["https://arnouthoekstra.substack.com/p/fixed-setups-are-a-blessing-in-lmu", "https://traxion.gg/widespread-bop-adjustments-nerf-bmw-lmgt3-and-alpine-a424-in-new-le-mans-ultimate-update/"]
```

### C.6 Pneus (TYR)

```yaml
- id: TYR-HOT-FRONT-01
  symptom: TYR_HOT_FRONT
  phase: any
  corner_speed: any
  surface: dry
  classes: [HY, LMP2, GT3]
  philosophy: both
  trigger: {telemetry: "D-TYR-03 : T_core AV > T_opt + 8 ; D-TYR-05 : T_front − T_rear > 8", feel: [FEEL_FRONT_TYRES_OVERHEAT, FEEL_FRONT_GOES_OFF]}
  changes:
    - {prio: 1, p: tyre.pressure_front, dir: "+", mag: "3 kPa", conf: M, side: "moins de grip mécanique AV (sous-virage)", conflicts: [US-MID-SLOW-01]}
    - {prio: 2, p: "balance", dir: "réduire le sous-virage", mag: "appliquer US-MID-* (le pneu AV surchauffe parce qu'il glisse)", conf: H, side: "", conflicts: []}
    - {prio: 3, p: brake.duct_front, dir: "+", mag: "1 cran (si T° frein élevée : chaleur jante → pneu)", conf: M, side: "", conflicts: [BRK-COLD-01]}
    - {prio: 4, p: geo.camber_front, dir: "-", mag: "0.2° si intérieur ≫ extérieur", conf: M, side: "", conflicts: []}
    - {prio: 5, p: geo.toe_front, dir: "→0", mag: "réduire |toe| de 0.05°", conf: M, side: "", conflicts: []}
    - {prio: 6, p: aero.front_flap, dir: "+", mag: "1 cran (moins de glissement AV en rapide)", conf: L, side: "", conflicts: []}
    - {prio: 7, p: tyre.compound, dir: "harder", mag: "si allocation", conf: H, side: "chauffe plus longue (3–5 tours)", conflicts: []}
  justification: "Un pneu qui surchauffe est un pneu qui glisse : corriger l'équilibre d'abord, puis la pression ; la pince/le carrossage ajoutent de la chaleur parasite."
  sources: ["https://lemansultimate.wiki.gg/wiki/Setup_Guide", "https://coachdaveacademy.com/tutorials/understanding-tire-pressures-in-iracing/"]

- id: TYR-HOT-REAR-01
  symptom: TYR_HOT_REAR
  phase: any
  corner_speed: any
  surface: dry
  classes: [HY, LMP2, GT3]
  philosophy: both
  trigger: {telemetry: "T_core AR > T_opt + 8 ; pics surface AR en sortie", feel: [FEEL_REAR_TYRES_OVERHEAT, FEEL_REAR_GOES_OFF]}
  changes:
    - {prio: 1, p: elec.tc, dir: "+", mag: "1", conf: H, side: "", conflicts: [TRC-TC-INTRUSIVE-01]}
    - {prio: 2, p: tyre.pressure_rear, dir: "+", mag: "3 kPa", conf: M, side: "motricité", conflicts: [TRC-BOTH-01]}
    - {prio: 3, p: "balance", dir: "réduire le survirage", mag: "appliquer OS-EXIT-* / OS-MID-*", conf: H, side: "", conflicts: []}
    - {prio: 4, p: geo.toe_rear, dir: "-", mag: "0.05° (si pince > 0.2°)", conf: M, side: "stabilité", conflicts: [BRK-UNSTABLE-01]}
    - {prio: 5, p: geo.camber_rear, dir: "-", mag: "0.2°", conf: M, side: "", conflicts: []}
    - {prio: 6, p: aero.rear_wing, dir: "+", mag: "1 cran", conf: M, side: "Vmax", conflicts: []}
    - {prio: 7, p: elec.engine_map, dir: "-", mag: "1", conf: L, side: "", conflicts: []}
  justification: "Hypercar : TC plus haut = moins de patinage = moins d'usure AR (simracingcockpit) ; v1.4 a réduit l'usure AR Hypercar."
  sources: ["https://simracingcockpit.gg/le-mans-ultimate-how-to-drive-the-lmgt3-and-hypercar/", "https://traxion.gg/latest-le-mans-ultimate-update-to-improve-hypercar-cold-tyre-performance-patch-notes-released/"]

- id: TYR-COLD-FRONT-01
  symptom: TYR_COLD
  phase: any
  corner_speed: any
  surface: any
  classes: [HY, LMP2, GT3]
  philosophy: both
  trigger: {telemetry: "D-TYR-08 : > 3 tours pour atteindre T_opt − 5 ; T_core < T_opt − 8 en régime", feel: [FEEL_NO_GRIP_COLD, FEEL_TAKES_LAPS_TO_COME_IN]}
  changes:
    - {prio: 1, p: tyre.pressure_front, dir: "+", mag: "3–5 kPa (+ AR) — chauffe plus vite, moins de flexion", conf: M, side: "moins de grip chaud ; note : certains recommandent au contraire de baisser pour plus de flexion — tester", conflicts: []}
    - {prio: 2, p: brake.duct_front, dir: "-", mag: "1 cran (+ AR)", conf: M, side: "freins chauds", conflicts: [BRK-HOT-01]}
    - {prio: 3, p: elec.tc, dir: "-", mag: "1 pendant la chauffe (en roulant)", conf: M, side: "", conflicts: []}
    - {prio: 4, p: geo.toe_front, dir: "-", mag: "0.05° (plus d'ouverture = scrub = chaleur)", conf: L, side: "usure", conflicts: []}
    - {prio: 5, p: geo.camber_front, dir: "+", mag: "0.2°", conf: L, side: "", conflicts: []}
    - {prio: 6, p: susp.sbump_front, dir: "+", mag: "2 clics (travail de carcasse)", conf: L, side: "", conflicts: []}
    - {prio: 7, p: tyre.compound, dir: "softer", mag: "si allocation", conf: H, side: "usure", conflicts: []}
    - {prio: 8, p: brake.bias_front, dir: "+", mag: "1 % au 1er tour (GT3 : avancer puis reculer)", conf: M, side: "", conflicts: []}
  justification: "Communauté LMU pneus froids : fermer conduits, monter pression, baisser TC/ABS, pilotage plus agressif en out-lap ; GT3 : répartition plus avant jusqu'à température."
  sources: ["https://steamcommunity.com/app/2399420/discussions/0/777534074828971181", "https://simracingcockpit.gg/le-mans-ultimate-how-to-drive-the-lmgt3-and-hypercar/"]

- id: TYR-HOT-INNER-01
  symptom: TYR_HOT_INNER
  phase: any
  corner_speed: any
  surface: dry
  classes: [HY, LMP2, GT3]
  philosophy: both
  trigger: {telemetry: "D-TYR-01 : ΔIO > +15 °C sur la roue extérieure en milieu de virage", feel: []}
  changes:
    - {prio: 1, p: geo.camber_front, dir: "-", mag: "0.2–0.3° (moins négatif) sur l'essieu concerné", conf: H, side: "moins de grip latéral en appui", conflicts: [US-MID-SLOW-01]}
    - {prio: 2, p: geo.toe_front, dir: "→0", mag: "0.05°", conf: M, side: "", conflicts: []}
  justification: "Règle pyromètre : intérieur un peu plus chaud (sim : +5–12 °C) ; au-delà, trop de carrossage."
  sources: ["https://www.sau.com.au/forums/topic/302303-car-set-up-cause-and-effect-by-carroll-smith", "https://lemansultimate.wiki.gg/wiki/Setup_Guide"]

- id: TYR-HOT-OUTER-01
  symptom: TYR_HOT_OUTER
  phase: any
  corner_speed: any
  surface: dry
  classes: [HY, LMP2, GT3]
  philosophy: both
  trigger: {telemetry: "ΔIO < +2 °C (ou négatif) sur la roue extérieure", feel: []}
  changes:
    - {prio: 1, p: geo.camber_front, dir: "+", mag: "0.2–0.3° (plus négatif) sur l'essieu concerné", conf: H, side: "freinage/motricité en ligne légèrement moindres", conflicts: []}
    - {prio: 2, p: susp.arb_front, dir: "+", mag: "1 cran (moins de roulis = moins de perte de carrossage)", conf: M, side: "équilibre", conflicts: [US-MID-SLOW-01]}
    - {prio: 3, p: geo.caster, dir: "+", mag: "0.5° (AV uniquement)", conf: M, side: "", conflicts: []}
  justification: "Extérieur chaud = pas assez de carrossage négatif pour le roulis présent."
  sources: ["https://www.sau.com.au/forums/topic/302303-car-set-up-cause-and-effect-by-carroll-smith"]

- id: TYR-HOT-MIDDLE-01
  symptom: TYR_HOT_MIDDLE
  phase: any
  corner_speed: any
  surface: dry
  classes: [HY, LMP2, GT3]
  philosophy: both
  trigger: {telemetry: "D-TYR-02 : ΔM > +3 °C", feel: []}
  changes:
    - {prio: 1, p: tyre.pressure_front, dir: "-", mag: "3 kPa (essieu concerné, si > mini)", conf: M, side: "", conflicts: []}
  justification: "Centre plus chaud que les bords = surgonflé (bombé)."
  sources: ["https://coachdaveacademy.com/tutorials/understanding-tire-pressures-in-iracing/"]

- id: TYR-COLD-MIDDLE-01
  symptom: TYR_COLD_MIDDLE
  phase: any
  corner_speed: any
  surface: dry
  classes: [HY, LMP2, GT3]
  philosophy: both
  trigger: {telemetry: "ΔM < −3 °C", feel: []}
  changes:
    - {prio: 1, p: tyre.pressure_front, dir: "+", mag: "3 kPa (essieu concerné)", conf: M, side: "", conflicts: []}
  justification: "Bords plus chauds que le centre = sous-gonflé."
  sources: ["https://coachdaveacademy.com/tutorials/understanding-tire-pressures-in-iracing/"]

- id: TYR-WEAR-01
  symptom: TYR_WEAR_FAST
  phase: any
  corner_speed: any
  surface: dry
  classes: [HY, LMP2, GT3]
  philosophy: safe
  trigger: {telemetry: "D-TYR-06 : pente > 0.08 s/tour ou roue > 1.3× l'usure moyenne", feel: [FEEL_TYRES_DROP_OFF, FEEL_CANT_DOUBLE_STINT]}
  changes:
    - {prio: 1, p: "balance", dir: "neutraliser", mag: "corriger l'essieu qui glisse (US-*/OS-*)", conf: H, side: "", conflicts: []}
    - {prio: 2, p: elec.tc, dir: "+", mag: "1 (si AR)", conf: H, side: "", conflicts: []}
    - {prio: 3, p: tyre.pressure_front, dir: "+", mag: "2–3 kPa sur l'essieu concerné", conf: M, side: "", conflicts: []}
    - {prio: 4, p: geo.camber_front, dir: "-", mag: "0.2° si usure intérieure", conf: M, side: "", conflicts: []}
    - {prio: 5, p: geo.toe_front, dir: "→0", mag: "0.05°", conf: M, side: "", conflicts: []}
    - {prio: 6, p: aero.rear_wing, dir: "+", mag: "1 cran (moins de glissement)", conf: M, side: "Vmax", conflicts: [STR-TOPSPEED-01]}
    - {prio: 7, p: susp.arb_front, dir: "-", mag: "1 cran (+ AR : 'softer bars = friendlier for endurance')", conf: M, side: "", conflicts: []}
    - {prio: 8, p: tyre.compound, dir: "harder", mag: "", conf: H, side: "", conflicts: []}
  justification: "CDA : endurance = moins agressif, préserver les pneus ; Porsche GT3 : usure 'extrême'."
  sources: ["https://coachdaveacademy.com/tutorials/the-ultimate-le-mans-ultimate-setup-guide/", "https://coachdaveacademy.com/tutorials/the-complete-guide-to-the-lmgt3-cars-in-le-mans-ultimate/"]
```

### C.7 Pluie (WET)

```yaml
- id: WET-PACK-01
  symptom: WET_BASELINE
  phase: any
  corner_speed: any
  surface: wet
  classes: [HY, LMP2, GT3]
  philosophy: both
  trigger: {telemetry: "mRaining > 0.2 ou mAvgPathWetness > 0.3 ; pneus Wet montés", feel: []}
  changes:
    - {prio: 1, p: tyre.compound, dir: "wet", mag: "", conf: H, side: "", conflicts: []}
    - {prio: 2, p: aero.rear_wing, dir: "+", mag: "1–2 crans", conf: H, side: "Vmax (peu pertinent sous la pluie)", conflicts: []}
    - {prio: 3, p: aero.rh_front, dir: "+", mag: "+3–5 mm (+ AR, garder le rake)", conf: H, side: "moins d'appui ; évite aquaplaning/talonnage dans les flaques", conflicts: []}
    - {prio: 4, p: susp.arb_front, dir: "-", mag: "1–2 crans (+ AR)", conf: H, side: "", conflicts: []}
    - {prio: 5, p: susp.spring_front, dir: "-", mag: "1 cran (+ AR)", conf: M, side: "", conflicts: []}
    - {prio: 6, p: susp.sbump_front, dir: "-", mag: "2 clics (tous)", conf: M, side: "", conflicts: []}
    - {prio: 7, p: brake.bias_front, dir: "+", mag: "1–2 %", conf: H, side: "", conflicts: []}
    - {prio: 8, p: brake.migration, dir: "-", mag: "0.5 (HY)", conf: M, side: "", conflicts: []}
    - {prio: 9, p: elec.tc, dir: "+", mag: "1–2 (cut +1, slip −1)", conf: H, side: "", conflicts: []}
    - {prio: 10, p: brake.abs_map, dir: "+", mag: "1 (GT3)", conf: M, side: "", conflicts: []}
    - {prio: 11, p: diff.power, dir: "-", mag: "1 cran ; preload −15 Nm", conf: M, side: "", conflicts: []}
    - {prio: 12, p: brake.duct_front, dir: "-", mag: "1–2 crans (+ AR)", conf: M, side: "", conflicts: []}
    - {prio: 13, p: tyre.pressure_front, dir: "+", mag: "0 à +5 kPa (pneus pluie : besoin de chaleur ; cible ≈ 60–80 °C)", conf: L, side: "", conflicts: []}
    - {prio: 14, p: geo.camber_front, dir: "-", mag: "0.3° (moins négatif : surface de contact en ligne)", conf: M, side: "", conflicts: []}
    - {prio: 15, p: elec.engine_map, dir: "-", mag: "1 (couple plus doux)", conf: M, side: "", conflicts: []}
    - {prio: 16, p: elec.motor_map, dir: "-", mag: "LMDh 5–10 kW (couple AR) ; LMH : couper le déploiement AV si sous-virage sous la pluie", conf: M, side: "", conflicts: []}
  justification: "Transposition des pratiques GT3/F1 sim (plus haut, plus souple, plus d'aile, plus avant, plus de TC) ; GO Setups : pluie = 'ajustements de TC, aéro et pressions'."
  sources: ["https://gosetups.gg/blog/lmu-setups-bmw-m4-and-wet-setups/", "https://simracingsetup.com/f1-2020-news-guides/f1-2020-convert-dry-setup-to-wet-setup/", "https://www.bsimracing.com/le-mans-ultimate-update-2-patch-4-deployed/"]

- id: WET-OS-EXIT-01
  symptom: OS_EXIT_SLOW
  phase: exit
  corner_speed: slow
  surface: wet
  classes: [HY, LMP2, GT3]
  philosophy: both
  trigger: {telemetry: "idem OS-EXIT-SLOW-01 sous pluie", feel: [FEEL_REAR_SLIDES_ON_THROTTLE]}
  changes:
    - {prio: 1, p: elec.tc, dir: "+", mag: "1 (cut +1)", conf: H, side: "", conflicts: []}
    - {prio: 2, p: diff.power, dir: "-", mag: "1 cran (pluie : diff plus ouvert = plus progressif)", conf: M, side: "note : inverse du sec", conflicts: []}
    - {prio: 3, p: susp.arb_rear, dir: "-", mag: "1 cran", conf: H, side: "", conflicts: []}
    - {prio: 4, p: elec.engine_map, dir: "-", mag: "1", conf: M, side: "", conflicts: []}
  justification: "Sous la pluie on cherche la progressivité : moins de verrouillage, plus d'électronique."
  sources: ["https://gosetups.gg/blog/lmu-setups-bmw-m4-and-wet-setups/"]

- id: WET-US-01
  symptom: US_MID_SLOW
  phase: mid
  corner_speed: any
  surface: wet
  classes: [HY, LMP2, GT3]
  philosophy: both
  trigger: {telemetry: "balance > +0.3 sous pluie", feel: [FEEL_FRONT_WONT_TURN]}
  changes:
    - {prio: 1, p: susp.arb_front, dir: "-", mag: "1 cran", conf: H, side: "", conflicts: []}
    - {prio: 2, p: brake.bias_front, dir: "-", mag: "0.5 % (rester plus avant qu'au sec)", conf: M, side: "", conflicts: []}
    - {prio: 3, p: aero.front_flap, dir: "+", mag: "1 cran / rake +2 mm", conf: M, side: "", conflicts: []}
    - {prio: 4, p: tyre.pressure_front, dir: "-", mag: "3 kPa", conf: L, side: "", conflicts: []}
    - {prio: 5, p: elec.motor_map, dir: "-", mag: "LMH : couper déploiement AV", conf: M, side: "", conflicts: []}
  justification: "Même logique qu'au sec, mais en gardant la sécurité au freinage."
  sources: ["https://coachdaveacademy.com/tutorials/hypercar-electric-motor-maps-in-le-mans-ultimate-explained/"]

- id: WET-NOTEMP-01
  symptom: WET_NO_TEMP
  phase: any
  corner_speed: any
  surface: wet
  classes: [HY, LMP2, GT3]
  philosophy: both
  trigger: {telemetry: "T_core wet < 55 °C en régime ; piste s'asséchant (mAvgPathWetness < 0.2) avec pneus pluie", feel: [FEEL_NO_GRIP_COLD]}
  changes:
    - {prio: 1, p: brake.duct_front, dir: "-", mag: "1 cran (+ AR)", conf: M, side: "", conflicts: []}
    - {prio: 2, p: tyre.pressure_front, dir: "+", mag: "3 kPa (+ AR)", conf: L, side: "", conflicts: []}
    - {prio: 3, p: tyre.compound, dir: "slick", mag: "si piste séchante : passer en slick (wets surchauffent sur sec)", conf: H, side: "", conflicts: []}
  justification: "Pneus pluie : fenêtre ≈ 60–80 °C (communauté) ; surchauffent vite sur ligne sèche."
  sources: ["https://steamcommunity.com/app/2399420/discussions/0/4637113154621450877"]
```

### C.8 Modificateurs de philosophie (appliqués en dernier, comme offsets au setup de base)

```yaml
- id: PHI-AGGRESSIVE-01
  symptom: PHILOSOPHY_AGGRESSIVE
  phase: any
  corner_speed: any
  surface: dry
  classes: [HY, LMP2, GT3]
  philosophy: aggressive
  trigger: {telemetry: "mode sprint / qualif / pilote régulier (σ laps faible, S-07 ≥ 0.6)", feel: [FEEL_WANT_MORE_ROTATION, FEEL_TOO_SAFE]}
  changes:
    - {prio: 1, p: brake.bias_front, dir: "-", mag: "0.5–1 % vs base", conf: H, side: "", conflicts: []}
    - {prio: 2, p: elec.tc, dir: "-", mag: "1 (6-6-6 GT3/LMP2 ; HY : cut 1–2, slip 9–11)", conf: M, side: "", conflicts: []}
    - {prio: 3, p: susp.arb_rear, dir: "+", mag: "1 cran (ou arb_front −1)", conf: H, side: "", conflicts: []}
    - {prio: 4, p: diff.coast, dir: "-", mag: "1 cran ; preload −15 Nm", conf: M, side: "", conflicts: []}
    - {prio: 5, p: aero.rear_wing, dir: "-", mag: "1 cran (circuits rapides) ; rake +1 mm", conf: M, side: "", conflicts: []}
    - {prio: 6, p: aero.rh_front, dir: "-", mag: "1–2 mm (moins de carburant en sprint)", conf: M, side: "talonnage", conflicts: [PLT-BOTTOM-01]}
    - {prio: 7, p: tyre.pressure_front, dir: "-", mag: "au mini (+ AR)", conf: M, side: "", conflicts: []}
    - {prio: 8, p: geo.toe_front, dir: "-", mag: "0.05° ; toe_rear −0.05°", conf: M, side: "", conflicts: []}
    - {prio: 9, p: susp.sbump_front, dir: "+", mag: "2 clics (+ AR) : voiture plus 'pointue'", conf: M, side: "", conflicts: [PLT-NERVOUS-01]}
    - {prio: 10, p: brake.migration, dir: "+", mag: "2.5F (HY) : freinage max", conf: M, side: "blocages AV si pilote agressif", conflicts: []}
    - {prio: 11, p: elec.engine_map, dir: "+", mag: "map perf", conf: M, side: "consommation", conflicts: []}
  justification: "CDA : sprint = moins de carburant, hauteurs plus basses, setup plus agressif, usure acceptée."
  sources: ["https://coachdaveacademy.com/tutorials/the-ultimate-le-mans-ultimate-setup-guide/", "https://traxion.gg/le-mans-ultimate-how-to-master-hypercar-braking/"]

- id: PHI-SAFE-01
  symptom: PHILOSOPHY_SAFE
  phase: any
  corner_speed: any
  surface: dry
  classes: [HY, LMP2, GT3]
  philosophy: safe
  trigger: {telemetry: "relais longs / endurance / pilote irrégulier (σ laps élevé, corrections volant élevées, S-07 < 0.5)", feel: [FEEL_WANT_STABILITY, FEEL_TIRING_TO_DRIVE]}
  changes:
    - {prio: 1, p: brake.bias_front, dir: "+", mag: "0.5–1 % vs base", conf: H, side: "", conflicts: []}
    - {prio: 2, p: elec.tc, dir: "+", mag: "1–2 (8-8-8 / 9-9-9)", conf: H, side: "", conflicts: []}
    - {prio: 3, p: susp.arb_rear, dir: "-", mag: "1 cran (+ arb_front −1 : 'softer bars friendlier')", conf: H, side: "", conflicts: []}
    - {prio: 4, p: diff.power, dir: "+", mag: "1 cran ; preload +15 Nm", conf: M, side: "", conflicts: []}
    - {prio: 5, p: aero.rear_wing, dir: "+", mag: "1 cran", conf: H, side: "Vmax", conflicts: []}
    - {prio: 6, p: aero.rh_front, dir: "+", mag: "+1–2 mm (plein de carburant, pas de talonnage)", conf: H, side: "", conflicts: []}
    - {prio: 7, p: geo.toe_rear, dir: "+", mag: "0.05–0.10°", conf: H, side: "chauffe AR", conflicts: []}
    - {prio: 8, p: susp.sbump_front, dir: "-", mag: "2 clics (+ AR) ; fbump −2", conf: M, side: "", conflicts: []}
    - {prio: 9, p: tyre.pressure_front, dir: "+", mag: "2–3 kPa au-dessus du mini (+ AR) : moins de pic thermique sur relais long", conf: L, side: "", conflicts: []}
    - {prio: 10, p: brake.migration, dir: "-", mag: "1.5–2.0F (HY)", conf: M, side: "", conflicts: []}
    - {prio: 11, p: elec.engine_map, dir: "-", mag: "map éco ; fuel_ratio adapté à lift & coast", conf: M, side: "", conflicts: []}
    - {prio: 12, p: geo.camber_front, dir: "-", mag: "0.2° (usure plus homogène)", conf: M, side: "", conflicts: []}
  justification: "CDA : endurance = plus de carburant, plus de stabilité, moins agressif ; TC plus haut réduit l'usure."
  sources: ["https://coachdaveacademy.com/tutorials/the-ultimate-le-mans-ultimate-setup-guide/", "https://arnouthoekstra.substack.com/p/confused-by-tc-you-should-be"]
```

### C.9 Matrice de conflits et d'effets croisés (à utiliser pour l'arbitrage)

| Paramètre | Effet principal | Effets croisés à surveiller |
|---|---|---|
| `aero.rear_wing +` | stabilité rapide, grip AR | Vmax −, sous-virage rapide, consommation + |
| `aero.rake +` | appui AV, rotation rapide | talonnage AV, décrochage fond plat (HY), instabilité AR rapide |
| `aero.rh_* −` | appui + | talonnage, packers sollicités, bascule d'équilibre |
| `susp.arb_front +` | réponse, moins de roulis | sous-virage, roue AV int. délestée (blocage) |
| `susp.arb_rear +` | rotation | patinage int., vibreurs, usure AR |
| `susp.spring_* +` | plateforme, assiette | grip sur bosses −, vibreurs − |
| `susp.packer_* +` | anti-talonnage | raideur soudaine en appui |
| `susp.third_* +` | assiette aéro | compliance heave − |
| `sbump` ± | transitoire (charge immédiate sur l'extrémité qui se comprime) | nervosité / flottement |
| `srebound` ± | transitoire (délestage de l'extrémité qui se détend) | roue intérieure soulevée, 'jacking down' |
| `fbump/frebound` − | vibreurs, bosses | mouvement de caisse sur gros chocs |
| `geo.camber +` | grip latéral | intérieur chaud, freinage/motricité en ligne − |
| `geo.toe_front −` | turn-in | stabilité en ligne −, chauffe |
| `geo.toe_rear +` | stabilité entrée/sortie | chauffe/usure AR, traînée |
| `tyre.pressure −` | grip mécanique, compliance | chauffe carcasse +, réponse −, (rF2 : mini imposé) |
| `brake.bias_front +` | stabilité freinage | sous-virage entrée, blocage AV |
| `brake.migration +` | freinage max en appui, rotation en trail | blocages AV à pleine pédale, chauffe freins |
| `diff.power +` | motricité, stabilité sortie | pousse-avant sous gaz |
| `diff.coast +` | stabilité entrée | sous-virage entrée |
| `diff.preload +` | stabilité transitions | voiture moins libre |
| `elec.tc +` | sécurité, usure AR − | perf sortie − |
| `elec.motor_map +` | perf, vide la batterie | LMH : sous-virage sous gaz > 150/190 km/h ; SOC vide |
| `brake.duct +` | freins plus froids | pneus/jantes plus froids, (légère) traînée |

Règles d'arbitrage : (1) ne jamais appliquer deux changements de **signe opposé** sur le même paramètre dans une même itération ; (2) si deux symptômes appellent des changements contradictoires sur un paramètre (ex. US_ENTRY_SLOW veut `bias −`, BRK_LOCK_REAR veut `bias +`), privilégier le **symptôme de sécurité** (freinage/stabilité) en mode « sûr » et le **symptôme de performance** en mode « agressif », et chercher un autre levier pour le second symptôme ; (3) limiter une itération à **3 paramètres** (1 en phase de validation) ; (4) toute modification de hauteur/aileron sur Hypercar réactive automatiquement la vérification de talonnage.

---
## D. Dictionnaire « ressenti pilote → symptôme »

Principe : le texte libre est normalisé (minuscules, sans accents, lemmatisation légère), puis on cherche des **motifs** (regex ou embeddings) ; chaque motif renvoie un `FEEL_*`, lequel pointe vers un ou plusieurs symptômes C avec une **phase/vitesse par défaut** qu'il faut confirmer par une **question de clarification** si la télémétrie ne tranche pas. Les motifs sont donnés en français puis en anglais. Les expressions sont volontairement nombreuses (variantes orales de pilotes sim francophones).

| FEEL id | Expressions FR | Expressions EN | Symptôme(s) C candidats | Phase / vitesse par défaut | Questions de clarification (ordre) |
|---|---|---|---|---|---|
| FEEL_FRONT_WONT_TURN_IN | « l'avant ne tourne pas à l'inscription », « ça n'inscrit pas », « je tourne le volant et rien », « ça pousse à l'entrée », « sous-virage en entrée », « l'avant s'enfuit au freinage » | "won't turn in", "pushes on entry", "no front end on turn-in", "understeer on entry", "front washes out under braking" | US_ENTRY_SLOW / US_ENTRY_FAST | entry ; vitesse à demander | 1) Dans les virages lents, rapides ou les deux ? 2) Toujours sur les freins à ce moment-là, ou déjà relâchés ? 3) Ça empire en fin de relais ? |
| FEEL_FRONT_WONT_TURN | « sous-virage en milieu », « ça pousse au milieu », « je dois attendre pour remettre les gaz », « l'avant glisse en appui », « ça élargit » | "mid-corner understeer", "pushes mid-corner", "front slides at the apex", "runs wide" | US_MID_SLOW / US_MID_FAST | mid | 1) Lents ou rapides ? 2) Avec combien de gaz ? 3) Plus avec le plein ? |
| FEEL_PUSH_ON_THROTTLE | « ça sous-vire à la remise des gaz », « quand j'accélère ça élargit », « l'avant s'ouvre sous gaz » | "power understeer", "pushes on throttle", "opens up on power" | US_EXIT_SLOW / US_EXIT_FAST (HY : vérifier motor map) | exit | 1) Vitesse du virage ? 2) Hypercar LMH : au-dessus de 150/190 km/h ? 3) Batterie pleine ou vide à ce moment ? |
| FEEL_PUSH_ON_BRAKES | « ça pousse sur les freins », « l'avant bloque et pousse » | "pushes under braking", "locks and pushes" | US_ENTRY_SLOW + BRK_LOCK_FRONT | entry | 1) La roue bloque vraiment ou ça glisse juste ? 2) Début ou fin de freinage ? |
| FEEL_FRONT_LIGHT_HIGH_SPEED | « l'avant est léger à haute vitesse », « flotte dans les courbes rapides », « pas de confiance dans les rapides » | "front is light at speed", "vague in fast corners", "no confidence in high-speed" | US_ENTRY_FAST / US_MID_FAST | fast | 1) Plutôt à l'inscription ou en appui ? 2) Freins encore engagés ? |
| FEEL_REAR_LOOSE_ON_BRAKES | « l'arrière se dérobe au freinage », « ça survire au freinage », « l'arrière passe devant quand je freine », « instable en entrée », « l'arrière décroche au lâcher de frein » | "rear steps out under braking", "loose on entry", "oversteer on turn-in", "rear comes around when I release the brake" | OS_ENTRY_SLOW / OS_ENTRY_FAST / BRK_UNSTABLE_STRAIGHT | entry | 1) En ligne droite (avant de tourner) ou une fois le volant tourné ? 2) Au rétrogradage ? 3) Lents ou rapides ? 4) Hypercar : batterie à 100 % ? |
| FEEL_REAR_STEPS_OUT_TURN_IN | « l'arrière part à l'inscription », « ça pivote trop à l'entrée », « trop pointu à l'entrée » | "rear snaps on turn-in", "too pointy on entry", "rotates too much on entry" | OS_ENTRY_SLOW / OS_ENTRY_FAST | entry | 1) Lents ou rapides ? 2) Plutôt bienvenu (rotation) ou dangereux ? |
| FEEL_REAR_LOOSE_MID | « l'arrière glisse en appui », « survirage en milieu de virage », « ça dérive de l'arrière sans gaz » | "loose mid-corner", "rear slides at the apex", "oversteer off-throttle mid-corner" | OS_MID_SLOW / OS_MID_FAST | mid | 1) Lents ou rapides ? 2) Pneus chauds ou froids ? 3) Sur bosses/vibreurs ? |
| FEEL_REAR_SLIDES_ON_THROTTLE | « ça glisse de l'arrière à la remise des gaz », « survirage à l'accélération », « l'arrière part quand j'accélère », « ça part en travers en sortie », « ça patine et ça part » | "oversteer on throttle", "rear slides on power", "power oversteer", "snaps on exit" | OS_EXIT_SLOW / OS_EXIT_FAST ; TRC_* | exit | 1) Les deux roues patinent ou seulement l'intérieure ? (ou laisser la télémétrie trancher) 2) Lents ou rapides ? 3) Dès le début du relais ou en fin ? 4) Avec le TC à combien ? |
| FEEL_WHEELSPIN | « ça patine », « pas de motricité », « je ne peux pas remettre les gaz », « ça cire » | "wheelspin", "no traction", "can't get on the power", "spins up" | TRC_WHEELSPIN_BOTH / TRC_WHEELSPIN_INNER | exit, slow | 1) Intérieure seule ? 2) Sur bosses ? 3) TC réglé à combien ? |
| FEEL_INNER_WHEELSPIN | « la roue intérieure patine », « ça cire d'une roue » | "inside wheel spins", "one-wheel peel" | TRC_WHEELSPIN_INNER | exit, slow | 1) Sur les vibreurs intérieurs ? |
| FEEL_TC_CUTS_POWER | « le TC coupe trop », « ça bride en sortie », « le moteur s'étouffe » | "TC too intrusive", "TC kills the exit", "bogs down on exit" | TC_TOO_INTRUSIVE | exit | 1) Est-ce que ça patinerait sans ? 2) Valeurs TC/cut/slip actuelles ? |
| FEEL_SLUGGISH_EXIT | « mou en sortie », « manque de punch » | "sluggish exit", "no punch" | TC_TOO_INTRUSIVE / STR_TOP_SPEED_LOW / HYB_SOC_EMPTY | exit | 1) Batterie vide ? 2) Map moteur ? |
| FEEL_SNAP_HIGH_SPEED | « ça décroche d'un coup dans les rapides », « snap à haute vitesse », « l'arrière part sans prévenir à 250 » | "snap oversteer at high speed", "sudden loss in fast corners", "rear goes without warning" | OS_MID_FAST / OS_ENTRY_FAST / PLT_FLOOR_STALL | fast | 1) Au freinage, en appui ou sous gaz ? 2) Sur une bosse/compression précise (Eau Rouge, Porsche Curves) ? 3) Sensation de talonnage ? |
| FEEL_SUDDEN_LOSS_HIGH_SPEED | « perte d'appui soudaine », « la voiture s'affaisse puis décroche », « ça décroche du fond plat » | "floor stall", "sudden loss of downforce", "bottoms then lets go" | PLT_FLOOR_STALL / PLT_BOTTOMING | fast | 1) Étincelles / bruit de fond plat ? 2) Avec le plein ? |
| FEEL_BALANCE_SHIFT_UNPREDICTABLE | « l'équilibre change d'un virage à l'autre », « imprévisible », « je ne sais jamais ce que la voiture va faire » | "unpredictable balance", "inconsistent", "never know what it'll do" | PLT_FLOOR_STALL / PLT_NERVOUS / TYR_COLD | any | 1) Plutôt rapide ou lent ? 2) Pneus dans la fenêtre ? 3) Plus en début ou fin de relais ? |
| FEEL_UNSTABLE_BRAKING | « instable au freinage », « ça louvoie au freinage », « l'arrière flotte au freinage en ligne droite », « ça serpente » | "unstable under braking", "wanders under braking", "rear moves around in a straight line", "snakes" | BRK_UNSTABLE_STRAIGHT | braking | 1) Avant de tourner le volant ? 2) Au rétrogradage ? 3) Sur bosses (Sebring) ? |
| FEEL_REAR_WANDERS_BRAKING | « l'arrière balade », « l'arrière est léger au freinage » | "rear is light under braking", "tail wags" | BRK_UNSTABLE_STRAIGHT / BRK_LOCK_REAR | braking | idem |
| FEEL_FRONT_LOCKS | « ça bloque de l'avant », « blocage avant », « je bloque les roues avant en fin de freinage » | "front locks", "locking the fronts", "front lock-up at the end of braking" | BRK_LOCK_FRONT | braking | 1) Début (haute vitesse) ou fin (basse vitesse) du freinage ? 2) Les deux roues ou l'intérieure ? 3) Hypercar : migration réglée à combien ? Batterie ? |
| FEEL_REAR_LOCKS | « ça bloque de l'arrière », « blocage arrière au rétrogradage » | "rear locks", "rear lock-up on downshift" | BRK_LOCK_REAR | braking | 1) Au rétrogradage ? 2) LMH : batterie pleine ? |
| FEEL_INNER_FRONT_LOCKS | « la roue intérieure bloque en trail », « blocage de l'intérieure en entrée » | "inside front locks", "inner wheel locks on trail brake" | BRK_LOCK_INNER_FRONT | entry, slow | 1) Avec beaucoup de volant encore sur les freins ? |
| FEEL_NO_BRAKES_COLD | « pas de freins à froid », « la pédale est longue au premier tour », « pas de freins la nuit » | "no brakes when cold", "long pedal on the out-lap", "brakes don't work at night" | BRK_COLD | braking | 1) Combien de tours avant que ça vienne ? 2) Conduits actuels ? |
| FEEL_LONG_PEDAL | « pédale longue », « freins spongieux » | "long pedal", "spongy brakes" | BRK_COLD / BRK_OVERHEAT | braking | 1) Début ou fin de relais ? |
| FEEL_BRAKES_FADE | « les freins s'évanouissent », « fading », « je freine de plus en plus tôt » | "brake fade", "brakes go away", "braking earlier and earlier" | BRK_OVERHEAT | braking | 1) Après combien de tours ? 2) Avant ou arrière ? |
| FEEL_BRAKES_CHANGE_MID_STINT | « le freinage change pendant le relais », « d'un coup ça freine différemment » | "braking feel changes mid-stint", "suddenly brakes differently" | HYB_SOC_FULL / BRK_OVERHEAT | braking | 1) Hypercar : état de charge quand ça arrive ? |
| FEEL_ABS_PULSING | « l'ABS pompe tout le temps », « ça vibre au freinage » | "ABS kicks in all the time", "pulsing under braking" | ABS_TOO_ACTIVE | braking | 1) Vous cherchez plus de rotation ou plus de sécurité ? |
| FEEL_LONG_BRAKING | « je freine trop long », « distances de freinage trop longues » | "braking distances too long" | ABS_TOO_ACTIVE / BRK_COLD / BRK_LOCK_* | braking | 1) Avec blocages ou sans ? |
| FEEL_BOTTOMING | « ça talonne », « ça touche », « des étincelles partout », « ça tape le fond » | "bottoming out", "scraping", "sparks everywhere", "hits the floor" | PLT_BOTTOMING | fast | 1) Où sur le circuit ? 2) Avec le plein ? 3) Sur bosses ou en compression aéro ? |
| FEEL_SPARKS_UNSETTLE | « ça décroche quand ça touche » | "it lets go when it bottoms" | PLT_BOTTOMING / PLT_FLOOR_STALL | fast | idem |
| FEEL_JUMPS_ON_KERBS | « ça saute sur les vibreurs », « les vibreurs la déstabilisent », « elle décolle sur les vibreurs », « ça rebondit » | "jumps on the kerbs", "kerbs unsettle it", "bounces off the kerbs" | PLT_KERB_UNSETTLED | any | 1) Vibreurs intérieurs (apex) ou extérieurs (sortie) ? 2) C'est l'avant ou l'arrière qui réagit ? |
| FEEL_KERBS_KICK_REAR | « l'arrière saute sur les vibreurs en sortie », « coup de pied sur vibreur » | "rear kicks on exit kerbs" | PLT_KERB_UNSETTLED (AR) | exit | — |
| FEEL_SKIPS_OVER_BUMPS | « ça sautille sur les bosses », « ça perd le contact sur les bosses », « voiture trop raide » | "skips over bumps", "loses contact on bumps", "too stiff" | PLT_BUMPY_LOSS | any | 1) Freinage, virage ou ligne droite ? |
| FEEL_LOSES_GRIP_ON_BUMPS | « ça décroche sur la bosse de … » | "loses grip on the bump at…" | PLT_BUMPY_LOSS | any | 1) Quel virage ? |
| FEEL_WALLOWS | « ça flotte », « ça roule beaucoup », « ça tangue », « mou », « bateau » | "wallows", "floaty", "rolls a lot", "boat-like", "soggy" | PLT_FLOATY | any | 1) En roulis (virages) ou en tangage (freinage/accélération) ? |
| FEEL_SLOW_TO_SETTLE | « elle met du temps à se poser », « oscille après les bosses » | "slow to settle", "oscillates after bumps" | PLT_FLOATY | any | — |
| FEEL_SKITTISH | « nerveuse », « sautillante », « trop vive », « pas posée » | "skittish", "nervous", "darty", "edgy" | PLT_NERVOUS | any | 1) Surtout sur bosses ou partout ? |
| FEEL_TOO_POINTY | « trop pointue », « trop réactive », « je n'ose pas tourner le volant » | "too pointy", "too reactive", "oversensitive steering" | PLT_NERVOUS / OS_ENTRY_* | entry | 1) Entrée ou milieu ? 2) Rapides ? |
| FEEL_LAZY | « paresseuse », « lente à changer de direction », « molle dans les chicanes », « elle ne veut pas changer d'appui » | "lazy", "slow to change direction", "sluggish in chicanes", "won't take a set" | STR_SLOW_DIRECTION_CHANGE | medium | 1) Chicanes rapides ou lentes ? |
| FEEL_SLOW_ON_STRAIGHTS | « je me fais déposer en ligne droite », « pas de vitesse de pointe », « trop d'aile » | "slow on the straights", "no top speed", "too much wing" | STR_TOP_SPEED_LOW | straight | 1) Même carburant/ERS que les autres ? 2) Régime max atteint ? |
| FEEL_FRONT_TYRES_OVERHEAT | « les pneus avant surchauffent », « l'avant chauffe », « les avants sont cuits » | "front tyres overheat", "fronts are cooked" | TYR_HOT_FRONT | any | 1) Dès le début ou après X tours ? 2) Ça pousse en même temps ? |
| FEEL_REAR_TYRES_OVERHEAT | « les pneus arrière surchauffent », « les arrières sont morts », « ça surchauffe de l'arrière » | "rears overheat", "rear tyres are gone" | TYR_HOT_REAR | any | 1) Patinage en sortie ? 2) TC ? |
| FEEL_FRONT_GOES_OFF | « l'avant s'en va après 10 tours », « l'avant dégrade » | "front goes off after 10 laps", "front degrades" | TYR_HOT_FRONT / TYR_WEAR_FAST | any | 1) Pression/température vues au HUD ? |
| FEEL_REAR_GOES_OFF | « l'arrière s'en va en fin de relais » | "rear falls off late in the stint" | TYR_HOT_REAR / TYR_WEAR_FAST | any | — |
| FEEL_TYRES_DROP_OFF | « les pneus tombent », « grosse dégradation », « je perds 1 s après 15 tours » | "tyres drop off", "big deg", "lose a second after 15 laps" | TYR_WEAR_FAST | any | 1) Avant ou arrière ? 2) Quel composé ? |
| FEEL_CANT_DOUBLE_STINT | « impossible de doubler le relais » | "can't double stint" | TYR_WEAR_FAST | any | — |
| FEEL_NO_GRIP_COLD | « pas de grip à froid », « les pneus ne montent pas en température », « il faut 5 tours pour que ça vienne » | "no grip when cold", "tyres won't come in", "takes 5 laps" | TYR_COLD / WET_NO_TEMP | any | 1) Température piste ? 2) Composé ? 3) Pression actuelle vs mini ? |
| FEEL_TAKES_LAPS_TO_COME_IN | « ça met des tours à venir » | "takes laps to come in" | TYR_COLD | any | — |
| FEEL_NO_BOOST | « plus de boost », « batterie vide », « le moteur électrique ne pousse plus » | "no boost", "battery flat", "no deployment" | HYB_SOC_EMPTY | any | 1) Motor map / regen actuels ? |
| FEEL_WANT_MORE_ROTATION | « je veux qu'elle tourne plus », « trop sûre », « trop sous-vireuse pour attaquer » | "want more rotation", "too safe", "too much understeer to push" | PHILOSOPHY_AGGRESSIVE (+ US_*) | any | 1) Où manque la rotation : entrée, milieu, sortie ? |
| FEEL_WANT_STABILITY | « je veux plus de stabilité », « trop vive pour un relais », « fatigante à conduire » | "want more stability", "too lively for a stint", "tiring to drive" | PHILOSOPHY_SAFE (+ OS_*) | any | 1) Quelle phase vous coûte le plus ? |
| FEEL_TOO_SAFE | « on peut y aller plus fort », « il y a de la marge » | "can be more aggressive", "there's margin" | PHILOSOPHY_AGGRESSIVE | any | — |
| FEEL_TIRING_TO_DRIVE | « épuisante », « je dois corriger tout le temps » | "exhausting", "constantly correcting" | PHILOSOPHY_SAFE / PLT_NERVOUS | any | — |
| FEEL_STEERING_HEAVY | « volant trop lourd », « FFB trop dur en appui » | "steering too heavy" | (pas de setup : caster −, FFB) | any | 1) Réglage FFB vérifié ? |
| FEEL_STEERING_LIGHT | « volant léger, pas de retour », « je ne sens pas l'avant » | "no feel in the steering", "light steering" | US_MID_* (saturation AV) / caster + | any | 1) Léger tout le temps ou seulement quand ça pousse ? |
| FEEL_SNAPS_ON_DOWNSHIFT | « l'arrière passe au rétrogradage », « ça bloque au rétrogradage » | "rear locks on downshift", "snaps on downshift" | BRK_LOCK_REAR (+ engine braking/coast) | braking | 1) Rétrogradages tardifs ? 2) LMDh : autoblip corrigé en 1.2.3 — version du jeu ? |
| FEEL_FUEL_HIGH | « je consomme trop », « je n'arrive pas à finir le relais en énergie » | "burning too much fuel", "can't make the stint on energy" | ENERGY_SHORT | any | 1) Lift & coast pratiqué ? 2) Map moteur ? 3) Fuel ratio ? |

**Ambiguïtés fréquentes à lever systématiquement** :
1. « ça glisse » sans précision → demander l'essieu (avant/arrière) et la phase (freins / sans pédale / gaz).
2. « instable » → ligne droite au freinage (BRK_UNSTABLE) vs entrée en virage (OS_ENTRY) vs haute vitesse (OS_*_FAST).
3. « pas de grip » → froid (TYR_COLD) vs surchauffe (TYR_HOT) vs équilibre.
4. « ça saute » → vibreurs (PLT_KERB) vs bosses (PLT_BUMPY) vs talonnage (PLT_BOTTOMING).
5. Le pilote parle souvent du **symptôme final** (« tête-à-queue en sortie ») alors que la cause est en amont (entrée trop rapide, freinage tardif) : croiser avec S-08 (vitesse mini) et S-02 (trail braking) avant de modifier le setup.

Protocole de questions (débrief de 60 s, ordre recommandé) : phase → vitesse de virage → début/fin de relais → pneus (T°/pression HUD) → batterie/TC (Hypercar) → « est-ce que c'est pire qu'avant la dernière modification ? ».

Sources : vocabulaire croisé de https://coachdaveacademy.com/tutorials/the-ultimate-le-mans-ultimate-setup-guide/ (FAQ sous-virage/survirage), https://www.sau.com.au/forums/topic/302303-car-set-up-cause-and-effect-by-carroll-smith (symptômes), https://simracingcockpit.gg/le-mans-ultimate-how-to-drive-the-lmgt3-and-hypercar/ ; expressions françaises : usage courant des pilotes sim (non sourçable, C=M).

---

## E. Méthodologie d'affinage

### E.1 Ordre des opérations (« du moins cher au plus structurant »)

1. **Conditions et pneus d'abord** : composé, pressions à froid pour atteindre la pression/température cible à chaud après 3 tours (lire `T_opt` via REST), conduits de frein pour la fenêtre freins. Sans pneus dans la fenêtre, tout diagnostic d'équilibre est faux. [C=H]
2. **Aéro et hauteurs** : niveau d'appui pour le circuit (aileron AR), puis rake/hauteurs pour l'équilibre haute vitesse et l'absence de talonnage avec la charge de carburant visée (vérifier D-PLT-01 à plein). Sur Hypercar et LMP2 c'est le levier n°1 ; en GT3 l'aileron reste le n°1 (Hoekstra). [C=H]
3. **Freinage** : répartition, migration (HY), map ABS (GT3), pression de pédale ; une seule de ces variables à la fois (consigne officielle ABS). [C=H]
4. **Barres anti-roulis** : équilibre milieu de virage lent/moyen. [C=H]
5. **Ressorts / 3e élément / packers** : plateforme, talonnage, bosses, vibreurs. [C=M]
6. **Amortisseurs** : transitoires (entrée/sortie) et vibreurs, validés par histogramme (B.6). [C=M]
7. **Différentiel** : motricité et rotation sous gaz / au lâcher. [C=M]
8. **Géométrie fine** (carrossage, pince, chasse) d'après les températures I/M/O, en dernier et par petits pas (la pince « en petites étapes », wiki). [C=M]
9. **Électronique de course** (TC, motor map, regen, engine map, fuel ratio) : en continu, y compris en roulant via MFD ; c'est le levier d'adaptation **intra-relais**. [C=H]

Le wiki LMU propose le même squelette (baseline → pressions/carrossage → aéro → hauteurs → suspension → électronique → relais longs → analyse). Pour un pilote débutant, le gain de temps au tour vient dans l'ordre : aileron, hauteurs, packers, répartition de frein, TC ; précharge, carrossage et pince comptent peu (Hoekstra, GT3). [C=M]

Sources : https://lemansultimate.wiki.gg/wiki/Setup_Guide · https://arnouthoekstra.substack.com/p/fixed-setups-are-a-blessing-in-lmu · https://guide.lemansultimate.com/hc/en-gb/articles/13211078435983-ABS-Anti-Lock-Braking-System-for-LMGT3-Cars

### E.2 Une variable à la fois vs paquets de changements

- **Phase d'exploration (nouveau circuit, nouveau pilote)** : paquets cohérents de 3–5 changements allant dans le même sens (ex. « paquet pluie », « paquet Monza bas appui », « paquet sûr ») ; on valide la **direction** sur 5–8 tours, pas la magnitude. [C=M]
- **Phase de convergence** : **une variable** (ou deux variables non interagissantes, ex. pression AV + conduit AR) par run de 5–8 tours propres ; comparer médiane des 3 meilleurs tours, σ, et les diagnostics ciblés ; revenir en arrière si le gain < bruit (bruit typique : 0,1–0,2 s GT3, 0,2–0,3 s Hypercar sur piste évolutive). [C=M]
- **Taille des pas** : commencer par 2 « crans d'ingénieur » (C.0) pour voir un effet, puis affiner à 1 ; si aucun effet mesurable après 2 crans, le paramètre n'est pas le bon levier pour ce symptôme (logique HP Academy pour les amortisseurs : « si le changement n'est pas visible au-delà du bruit, il était trop petit »). [C=M]
- **Verrouiller l'électronique** pendant les comparaisons de setup mécanique (TC, map, migration constants), sinon le pilote compense et masque l'effet. [C=H]
- **Toujours noter** : setup, carburant, composé, T° piste/air, heure (nuit), Real Road, pour pouvoir comparer des runs comparables (GO Setups et simracingsetup créent leurs setups à 20 °C air / Real Road ; des conditions très différentes exigent au minimum des retouches de pression). [C=H]

Sources : https://www.hpacademy.com/courses/professional-motorsport-data-analysis/damper-analysis-damping-histogram/ · https://support.gosetups.gg/hc/en-gb/articles/23222378953106-What-Conditions-Are-Our-LMU-Setups-Created-In

### E.3 Pondérer ressenti et données

Score de confiance d'un diagnostic = cohérence × couverture × qualité du pilote :
- **Cohérence** : les 3 estimateurs d'équilibre (B.4 a/b/c) concordent (poids 1,0), deux sur trois (0,6), contradictoires (0,2 → demander au pilote).
- **Couverture** : ≥ 3 tours propres et ≥ 3 occurrences du virage ; sinon diviser le poids par 2.
- **Qualité pilote** : S-07 (marge au grip) ≥ 0,6 et σ(laps) faible → les données priment (données 0,7 / ressenti 0,3) ; pilote loin de la limite ou irrégulier → le ressenti prime pour la *pilotabilité* (0,4 / 0,6) mais les données priment pour les **faits physiques** (talonnage, températures, blocages, SOC batterie) qui ne se négocient pas.
- **Conflit ressenti ≠ données** : (1) vérifier que le pilote et la télémétrie parlent du même virage/phase (question D) ; (2) vérifier une cause de pilotage (S-08 : entrée trop rapide → « sous-virage » ressenti alors que la voiture est neutre) ; (3) si le ressenti persiste, privilégier un changement **réversible et peu coûteux** (répartition, TC, barre) et re-tester ; (4) ne jamais corriger par le setup un symptôme présent sur **un seul tour**.
- **Priorisation** : sécurité (blocages, instabilité freinage, snaps rapides) > pilotabilité (prévisibilité, fatigue) > performance (rotation, Vmax) > usure/consommation en sprint — et usure/consommation remontent en n°2 en mode « sûr/endurance ».

[C=M — pratique d'ingénierie ; pas de source chiffrée unique.]

### E.4 Adapter un setup d'un circuit à un autre

Typologie et transformations recommandées (appliquer comme paquet, puis reconverger avec E.2) :

| Type de circuit | Exemples LMU | Caractéristiques dominantes | Transformations depuis un setup « mixte » (référence : Spa, Portimão, Imola) | Conf. |
|---|---|---|---|---|
| **Rapide / aéro, faible traînée** | Le Mans, Monza (Curva Grande), Paul Ricard | longues lignes droites, freinages violents depuis > 300 km/h, chicanes | aileron AR −2 à −4 crans (GT3 : jusqu'à l'angle mini BoP), flap AV − pour garder l'équilibre, rake légèrement + pour compenser l'appui AV perdu, hauteurs −1 à −2 mm si pas de talonnage, répartition +0,5–1 % (freinages longs stables), migration 2,0–2,5F, barres +1 (chicanes), fast bump − (vibreurs des chicanes), ressorts AR + (stabilité à Vmax), rapports « Le Mans », motor map LMH haut (déploiement > 190 km/h utile), map moteur selon énergie. | M |
| **Mixte** | Spa, Portimão, Imola, Fuji, Silverstone, Interlagos | enchaînement rapides/lents, dénivelé, compressions (Eau Rouge, Portimão) | setup de référence ; attention aux compressions : packers/3e élément et hauteurs validés à plein (D-PLT-01) ; Interlagos : fast bump − (bosses), anti-horaire → pressions/pneu AV droit. | M |
| **Lent / mécanique** | Bahreïn (secteur 2), Qatar (lent-moyen), COTA (secteur 1), Sebring (hors bosses) | virages lents, motricité, freinages en épingle | aileron +1–2 (traînée peu pénalisante), barres −1 (grip mécanique), ressorts −1, diff power +1 et preload + (motricité), TC +1 si surchauffe AR, répartition −0,5 % (rotation sur les freins), coast −1, pressions AR au mini, carrossage AV + (appui mécanique), conduits + si freins chauds (Bahreïn). | M |
| **Bosselé** | Sebring, Interlagos, COTA (bosses), Austin | bosses d'amplitude élevée, vibreurs agressifs | ressorts −1 à −2, 3e élément −1, fast bump −2 à −4, fast rebound −2, slow rebound −1, hauteurs +2–4 mm, packers − (plus de course), barres −1, pressions −2 kPa, répartition +0,5 % (stabilité au freinage sur bosses), TC +1 ; accepter une plateforme aéro moins stable (Hypercar : surveiller le décrochage de fond plat sur les compressions). CDA : 963/BMW « good composure over bumps » à Sebring. | M |
| **Chaud / froid** | Bahreïn/Qatar (chaud, nuit froide) ; Spa/Le Mans nuit | fenêtre thermique | chaud : pressions −2–3 kPa (chauffe plus vite, l'air réchauffe), composé plus dur, conduits +1, TC +1 ; froid/nuit : pressions +3–5 kPa, conduits −1, composé plus tendre, répartition +1 % pendant la chauffe, TC/ABS − pendant la chauffe. | M |

Processus de transfert : (1) partir du setup « mixte » validé ; (2) appliquer le paquet de la typologie ; (3) 3 tours de contrôle : talonnage (D-PLT-01), pressions/T° (D-TYR-03/04), freins (D-BRK-01) ; (4) 5–8 tours d'équilibre : matrice B.4 → règles C ; (5) relais long simulé en mode « sûr » pour la dégradation (D-TYR-06) et l'énergie (D-CON-02/03). Conserver un setup par couple (circuit, typologie météo) et un journal des deltas appliqués pour apprendre les « coefficients » du pilote (ex. « ce pilote veut toujours −0,5 % de répartition par rapport à la base »).

Sources : https://coachdaveacademy.com/tutorials/lmu-porsche-963-guide-the-inside-line/ · https://coachdaveacademy.com/tutorials/lmu-bmw-m-hybrid-v8-guide-the-inside-line/ · https://arnouthoekstra.substack.com/p/fixed-setups-are-a-blessing-in-lmu · https://traxion.gg/widespread-bop-adjustments-nerf-bmw-lmgt3-and-alpine-a424-in-new-le-mans-ultimate-update/ · https://steamcommunity.com/app/2399420/discussions/0/777534074828971181

### E.5 Génération des 3 setups initiaux (pluie / agressif / sûr)

1. **Base** = setup par défaut du jeu ou CDA intégré pour (voiture, circuit) — depuis v1.2 des setups CDA sont livrés in-game pour la plupart des Hypercars et plusieurs GT3 ; ils sont « plus performants que les défauts » et constituent une meilleure base. [C=H]
2. Appliquer le **paquet typologie circuit** (E.4) si la base n'est pas spécifique au circuit.
3. Dériver : **Agressif** = base + PHI-AGGRESSIVE-01 ; **Sûr** = base + PHI-SAFE-01 ; **Pluie** = base + WET-PACK-01 (+ PHI-SAFE-01 allégé : TC, répartition).
4. Personnaliser par le **profil de style** (B.9) si des données existent déjà : freinage agressif → migration −0,5 et répartition +0,5 ; trail braking fort → coast +1 ; gaz brutaux → TC +1, power +1 ; gros usage vibreurs → fast bump −2, packers −2 ; volant nerveux → PLT-NERVOUS-01.
5. Valider les invariants : angle d'aileron ≥ mini BoP ; pressions ≥ mini ; hauteurs ≥ mini ; énergie virtuelle 100 % ; fuel ratio cohérent avec la stratégie ; batterie de départ ≈ 75 % avec déploiement élevé puis réduit (CDA).

Sources : https://www.bsimracing.com/le-mans-ultimate-update-2-patch-4-deployed/ · https://coachdaveacademy.com/tutorials/hypercar-electric-motor-maps-in-le-mans-ultimate-explained/

---

## F. Limites connues et points à vérifier en jeu (backlog de validation)

1. Noms exacts des clés de setup LMU (v1.4) et bornes par voiture → extraire via REST `enable_garage_setup_info` et sauvegardes auto ; compléter A.1–A.3.
2. Ordre des 3 zones de `mTemperature[3]` (gauche/centre/droite de la bande) par côté de voiture → tester en braquant d'un côté.
3. Signes du repère `mLocalVel/mLocalAccel/mLocalRot` dans l'API native LMU (identique à rF2 en principe).
4. Seuil lent/rapide des amortisseurs par voiture (HDV) → à défaut utiliser 25 mm/s (HP Academy) et mesurer.
5. Fenêtres de température des freins par classe après la refonte v1.4 → calibrer sur la couleur HUD.
6. Présence effective du 3e élément, du flap avant et de la barre réglable en roulant, voiture par voiture.
7. Allocation de composés par classe et session (REST « allowed tyres »).
8. Fidélité de la relation pression froide → pression chaude (loi des gaz) dans le modèle pneu LMU 1.4.

---

## G. Bibliographie (URL consultées, novembre 2026)

- Coach Dave Academy — The Ultimate LMU Setup Guide : https://coachdaveacademy.com/tutorials/the-ultimate-le-mans-ultimate-setup-guide/
- Coach Dave Academy — Hypercar Electric Motor Maps : https://coachdaveacademy.com/tutorials/hypercar-electric-motor-maps-in-le-mans-ultimate-explained/
- Coach Dave Academy — Porsche 963 guide : https://coachdaveacademy.com/tutorials/lmu-porsche-963-guide-the-inside-line/
- Coach Dave Academy — BMW M Hybrid V8 guide : https://coachdaveacademy.com/tutorials/lmu-bmw-m-hybrid-v8-guide-the-inside-line/
- Coach Dave Academy — LMGT3 complete guide : https://coachdaveacademy.com/tutorials/the-complete-guide-to-the-lmgt3-cars-in-le-mans-ultimate/
- Coach Dave Academy — LMP2 complete guide : https://coachdaveacademy.com/tutorials/complete-guide-to-the-lmp2-class-in-le-mans-ultimate/
- Coach Dave Academy — Hypercar setup guide (sélection) : https://coachdaveacademy.com/announcements/the-latest-le-mans-ultimate-hypercar-setup-guide
- Coach Dave Academy — iRacing tyre pressures (principes I/M/O) : https://coachdaveacademy.com/tutorials/understanding-tire-pressures-in-iracing/
- LMU officiel — LMP3 setup guide (22/12/2025) : https://lemansultimate.com/?p=3955
- LMU support — Hypercar category : https://guide.lemansultimate.com/hc/en-gb/articles/13152322265231-Hypercar-Category-LMH-LMDh
- LMU support — Traction control : https://guide.lemansultimate.com/hc/en-gb/articles/13182869047311-How-do-I-configure-my-traction-control-in-Le-Mans-Ultimate
- LMU support — ABS LMGT3 : https://guide.lemansultimate.com/hc/en-gb/articles/13211078435983-ABS-Anti-Lock-Braking-System-for-LMGT3-Cars
- LMU wiki — Setup Guide : https://lemansultimate.wiki.gg/wiki/Setup_Guide ; Telemetry : https://lemansultimate.wiki.gg/wiki/Telemetry ; LMGT3 : https://lemansultimate.wiki.gg/wiki/LMGT3 ; LMP2 : https://lemansultimate.wiki.gg/wiki/LMP2
- Traxion — Hypercar braking (brake migration) : https://traxion.gg/le-mans-ultimate-how-to-master-hypercar-braking/
- Traxion — Virtual energy : https://traxion.gg/how-to-manage-virtual-energy-in-le-mans-ultimate/
- Traxion — BoP & driver-aid weights : https://traxion.gg/le-mans-ultimate-update-adjusts-lmgt3-bop-and-increases-driver-aid-weight-penalties/
- Traxion — Widespread BoP (leviers BoP) : https://traxion.gg/widespread-bop-adjustments-nerf-bmw-lmgt3-and-alpine-a424-in-new-le-mans-ultimate-update/
- Traxion — Hypercar tyre update v1.2.3 : https://traxion.gg/le-mans-ultimates-hypercars-given-massive-tyre-update/
- Traxion — v1.4 patch notes (pneus, freins) : https://traxion.gg/latest-le-mans-ultimate-update-to-improve-hypercar-cold-tyre-performance-patch-notes-released/
- Traxion — v1.3 patch notes : https://traxion.gg/le-mans-ultimate-march-2026-update-patch-notes/
- bsimracing — v1.2.4 patch notes : https://www.bsimracing.com/le-mans-ultimate-update-2-patch-4-deployed/ ; v1.3 : https://www.bsimracing.com/le-mans-ultimate-v1-3-and-new-hotfix-released/
- simracingcockpit — How to drive LMGT3 & Hypercar : https://simracingcockpit.gg/le-mans-ultimate-how-to-drive-the-lmgt3-and-hypercar/
- Arnout Hoekstra — Confused by TC : https://arnouthoekstra.substack.com/p/confused-by-tc-you-should-be ; Fixed setups : https://arnouthoekstra.substack.com/p/fixed-setups-are-a-blessing-in-lmu
- GO Setups — conditions de création : https://support.gosetups.gg/hc/en-gb/articles/23222378953106-What-Conditions-Are-Our-LMU-Setups-Created-In ; wet setups : https://gosetups.gg/blog/lmu-setups-bmw-m4-and-wet-setups/
- Steam — Setup discussion : https://steamcommunity.com/app/2399420/discussions/0/4853281047676507449 ; Tyre temps : https://steamcommunity.com/app/2399420/discussions/0/777534074828971181 ; https://steamcommunity.com/app/2399420/discussions/0/4637113154621450877
- Michelin Hypercar tyres (réel) : https://a-e.li/news/motorsport/wec/michelin-le-mans-hypercar-tires/
- rF2 Shared Memory Map Plugin : https://github.com/TheIronWolfModding/rF2SharedMemoryMapPlugin ; structures documentées : https://domaslau.github.io/rF2SharedMemoryNet/api/rF2SharedMemoryNet.RF2Data.Structs.Wheel.html ; https://docs.rs/crate/kerb/latest/source/src/lmu/structs.rs
- LMU Pitwall (API native LMU_Data, REST 6397) : https://github.com/Swizzjack/lmu-pitwall
- ModernTinyPedal — Game Setup (REST groups, optimal tyre temperature) : https://github.com/Keenny38/ModernTinyPedals/wiki/Game-Setup
- LMUFFB (interface mémoire partagée LMU 1.2, saturation par couple de crémaillère) : https://github.com/coasting-nc/LMUFFB
- HP Academy — Damper histogram : https://www.hpacademy.com/courses/professional-motorsport-data-analysis/damper-analysis-damping-histogram/
- Carroll Smith — cause & effect (extraits *Tune to Win*) : https://www.sau.com.au/forums/topic/302303-car-set-up-cause-and-effect-by-carroll-smith
- Overtake — camber & tyre workloads : https://www.overtake.gg/threads/optimal-camber-my-thought-process-on-tyre-workloads.195866/ ; rF2 tyre temps : https://overtake.gg/threads/not-a-popular-question-but-needs-answering-none-the-less.190351
- Ouvrages (principes, non consultés en ligne) : Carroll Smith, *Tune to Win* (1978) ; W. & D. Milliken, *Race Car Vehicle Dynamics* (SAE, 1995) ; OptimumG, *Tech Tips — Springs & Dampers*.
