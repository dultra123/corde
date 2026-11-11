# L'ingénieur de course (`crates/corde-engineer`)

> Comment une session enregistrée devient un rapport d'analyse et trois orientations de réglage.
> Lot « ingénieur » de la phase 2 (novembre 2026) : un calcul déterministe, typé, mis en cache par le
> serveur. Lots suivants : le ressenti du pilote est interprété (hors-ligne par défaut) et fait itérer le
> plan de réglage (§ 7) ; les plans sont appliqués au **vrai réglage** du pilote (ses fichiers `.svm`) et
> Corde écrit des fichiers prêts à charger (§ 8).
> Les règles et les diagnostics viennent de [`knowledge/`](../knowledge/README.md) ; les formules du brief
> [`docs/research/02-ingenieur-de-course.md`](research/02-ingenieur-de-course.md) §B, §C, §E.

## 1. Vue d'ensemble

```
Store ──load──▶ LapSeries (tours propres) ──corners──▶ virages + phases
                    │                                     │
                    └──metrics──▶ Computed ◀──────────────┘
                                     │
                           diagnostics ──▶ Findings ──rules──▶ SetupPlans (agressif / sûr / pluie)
                                     │                               │
                                     └────────── report ◀────────────┘
```

| Module | Rôle |
|---|---|
| `load.rs` | Lit la session (`Store::get_session`, `Store::lap_samples`), garde les tours **propres** : `valid`, ni `in_lap` ni `out_lap`, et ≥ 60 % des échantillons attendus (`lap_time_s × record_rate_hz`). 20 canaux scalaires + 15 canaux par roue (noms réels de `corde-store::channels` ; une colonne absente d'un ancien fichier se lit comme inconnue). |
| `series.rs` | `LapSeries` : colonnes `f64` nommées (NaN = absent), `SessionMeta`. C'est aussi ce que les tests construisent sans base. |
| `corners.rs` | Segmentation des virages, phases, appariement d'un tour à l'autre. |
| `metrics.rs` | Toutes les formules (équilibre, blocages, pneus, freins, plateforme, amortisseurs, style, tours). |
| `analysis.rs` | Orchestration → `Computed` (tous les blocs du rapport sauf constats et plans) ; matrice d'équilibre. |
| `diagnostics.rs` | Constats (`Finding`) avec seuils, gravité, confiance, preuves chiffrées, textes FR/EN, symptômes. |
| `rules.rs` | Base de connaissances embarquée (`include_str!` de `setup-rules.yaml` + `setup-parameters.yaml`, surcharge par dossier) et moteur de règles → 3 plans. |
| `report.rs` | `AnalysisReport` + résumés FR/EN. |
| `feedback.rs` | Ressenti en texte libre → `FeedbackInterpretation` (symptômes, questions, fragments non compris) : `DictionaryExtractor` (défaut) et `AnthropicExtractor` (optionnel, § 7). |
| `iterate.rs` | `iterate_plan` : ressenti + constats du dernier rapport → `PlanIteration` (delta de 1 à 3 changements, plan cumulé, concordances / désaccords). |
| `setup.rs` | Plans et itérations appliqués à un fichier `.svm` réel (clés, sens de l'index, crans, bornes, libellés appris, paramètres non réglables écartés) et écriture du nouveau fichier (§ 8). |

API publique :

```rust
corde_engineer::analyse_session(&store, "20261104-…-race", &AnalyseOptions::default()) -> Result<AnalysisReport>
corde_engineer::analyse_samples(&meta, &laps, &opts)   // sans base, pour les tests
AnalyseOptions { knowledge: Option<Knowledge>, max_changes_per_plan: usize (8), corners: CornerConfig }
Knowledge::embedded() / Knowledge::from_dir(&path)     // setup-rules.yaml + setup-parameters.yaml
```

Erreurs : `EngineerError::Store(NotFound)` → session inconnue (404), `NotEnoughData(msg_fr)` → aucun tour
exploitable (422), `Knowledge` → YAML illisible (500).

Serveur : `GET /api/sessions/{id}/analysis` calcule à la demande (dans `spawn_blocking`) et met en cache
dans la table `analyses(session_id PRIMARY KEY, generated_ms, report_json)` (migration v3 de `corde.db`) —
sauf pendant l'enregistrement de la session, où le rapport est recalculé à chaque appel ;
`POST /api/sessions/{id}/analysis/recompute` ignore le cache. La suppression d'une session supprime son
rapport (cascade). Types TypeScript : `@corde/protocol` (`AnalysisReport`, `Finding`, `SetupPlan`…).

## 2. Prétraitement

### Tours propres

Le jeu est l'arbitre : un tour est **valide** quand le jeu l'a chronométré et qu'il ne passe pas par
les stands (`LapRecord::valid`, voir `corde_core::LapTracker` et `corde_store::reconcile`). Pour
l'analyse, `load::classify` garde un tour valide sauf si :

| Écarté | Pourquoi |
|---|---|
| non chronométré par le jeu (`not_timed`) | limites de piste, coupe : le jeu affiche `--.---` |
| tour d'entrée / de sortie des stands | pas un tour lancé |
| abandonné (`aborted`) | retour au garage (Échap) |
| tour de départ de course (`start_lap`) | départ arrêté / lancé, trafic : chronométré et compté, mais pas représentatif |
| à plus de 107 % du meilleur tour (`MAX_PACE_RATIO`) | tête-à-queue, trafic : brouillerait le diagnostic |
| trop peu d'échantillons | moins de 60 % (`MIN_COVERAGE`) de ce que les autres tours de la session contiennent par seconde |

La couverture est jugée par rapport à ce que **la session contient réellement** (médiane des tours
chronométrés), pas par rapport à la cadence nominale : les sessions enregistrées par l'alpha 1
tenaient ≈ 30 échantillons/s pour 50 annoncés, et le seuil fixe écartait des tours propres au hasard.

Le détail des tours retenus et écartés (numéros du jeu, à partir de 1) est écrit en français dans
`DataQuality::notes` et affiché en tête de la page Ingénieur.
Moins de 3 tours retenus ⇒ confiance de **tous** les constats divisée par 2 (`coverage_factor`).

Sessions de l'alpha 1 : leurs échantillons portent l'horloge du classement (pas de 0,2 s) et la
distance du classement (marches de 14 m à 250 km/h). `corde_store::legacy::repair_axes` reconstruit
les deux axes (horloge murale et vitesse enregistrées), puis `load::resample_uniform` remet le tour
sur une grille régulière avant tout calcul.

### Virages (`corners::segment`)
- `|g_lat|` lissé sur 3 échantillons ; un virage commence à **> 0,5 g** et finit à **< 0,35 g** (hystérésis) ;
- segments de même sens séparés de **< 0,5 s** fusionnés ; segments **< 0,4 s** ignorés ;
- apex = minimum de vitesse ; sens = signe moyen de `g_lat` (positif = droite, convention du protocole) ;
- classe : vitesse mini **≤ 90 km/h lent, 90–160 moyen, > 160 rapide** ; Hypercar / LMP2 / LMP3 : +20 km/h.

### Phases (par échantillon du virage)
- **milieu** : à ±0,25 s de l'apex, ou `|g_lat| ≥ 90 %` du max du virage hors freinage / remise des gaz ;
- **entrée** : avant l'apex avec `brake > 5 %` ou `g_long < −0,05 g` ;
- **sortie** : après l'apex avec `throttle > 10 %`.

### Appariement d'un tour à l'autre (`corners::match_corners`)
Même virage si l'apex est à **± 40 m** (avec bouclage sur la longueur du circuit), même sens, et au plus un
passage par tour. Chaque `CornerStat` du rapport porte le nombre de passages.

## 3. Formules

### Équilibre sous-/survirage (D-BAL-01) — par rapport à ta référence de la session

Depuis le lot S2 (calibration sur données réelles, § 3.1), l'équilibre n'est plus jugé dans l'absolu mais
**par rapport au comportement habituel de cette voiture dans cette session**. Deux estimateurs par
échantillon en charge (`|g_lat| > 0,3 g`) :

1. **Angles de dérive** (si `slip_angle_deg_*` vivant), avec `α_AV = (|α_FL| + |α_FR|)/2` et
   `α_AR = (|α_RL| + |α_RR|)/2` :
   - **rapport d'équilibre** `ρ = (α_AV − α_AR) / (α_AV + α_AR)` (−1…+1, calculé si `α_AV + α_AR ≥ 0,4°`) :
     c'est lui qui sert au diagnostic, parce qu'il ne dépend pas du niveau de charge (§ 3.1) ;
   - différence `Δα = α_AV − α_AR` en degrés : gardée pour l'**indice absolu** (`raw_index`).
2. **Gradient cinématique** : `US = (δ − L·ψ̇/V) × signe(g_lat)` avec `δ = angle_volant / i_s`,
   `L` = 2,7 m GT3/GTE, 3,0 m LMP2/LMP3, 3,1 m Hypercar, `ψ̇` = `yaw_rate_dps`, `V` en m/s ; `K = US / |g_lat|`
   en °/g. Le rapport de direction `i_s` est estimé en régime établi (`0,2 < |g_lat| < 0,5 g`, médiane de
   `SWA / (L·ψ̇/V)`), repli 12. **Inactif sur LMU aujourd'hui** : `steering_angle_deg` vaut 0 dans toutes
   les sessions réelles (§ 3.1).

**Référence de la session** (`BalanceAnalysis::reference`) : pour chaque estimateur, médiane et dispersion
robuste `σ = max(1,4826 × MAD, plancher)` des valeurs **par passage** (moyenne des échantillons d'un
(virage, phase, tour)), sur tous les passages de la session ; il en faut au moins 5 (`MIN_REFERENCE_PASSES`),
sinon l'équilibre n'est pas évalué. Planchers : 0,05 pour `ρ`, 0,5 °/g pour `K` (une session parfaitement
homogène ne transforme pas le bruit en constat). On prend les passages plutôt que les échantillons bruts pour
qu'un long virage ne pèse pas plus que dix courts.

**Indice relatif** d'un passage : chaque estimateur devient un écart `z = (valeur − médiane) / σ`, fusion
`0,6 × z_dérive + 0,4 × z_gradient`, puis `indice = z / 5` borné à ±1 (**±1 = 5 σ**). Accord = 1,0 si même
signe (ou tous deux sous le seuil), 0,3 si contradictoires, 0,7 si un seul estimateur. Agrégation : médiane
par cellule (phase × classe de vitesse) ; `CornerStat::balance_*` = médiane des passages du virage.
Confiance de la cellule = couverture (1 passage 0,4 ; 2 → 0,6 ; ≥ 3 → 1,0) × accord × (0,5 si < 3 tours)
× (0,8 sans canaux de dérive).

**Constat** quand `|indice| > 0,3` (**1,5 σ**, `BALANCE_TRIGGER_INDEX`) sur **au moins 2 passages**
(`MIN_PASSES_FOR_FINDING`) et confiance ≥ 0,15 ; `> 0,6` (**3 σ**) = majeur ; confiance × `min(1, 0,4 +
|indice|)`. Le titre dit « Plus de sous-virage / survirage … (par rapport à ta référence) » ; les preuves
donnent l'écart en σ, l'indice relatif, l'indice absolu de la cellule et celui de la référence. Les virages
**moyens** déclenchent les symptômes `_SLOW` (grip mécanique), faute de règles « medium » dans le YAML.

Conséquences voulues : une voiture « normale » ne produit **aucun** constat d'équilibre ; une voiture
uniformément sous-vireuse non plus (les données seules ne peuvent pas le distinguer d'une voiture dont les
pneus avant dérivent naturellement plus) — c'est le **ressenti du pilote** qui le dit, et il passe devant
(§ 7.3). Un constat apparaît quand une phase ou une plage de vitesse se détache nettement du reste.

L'**indice absolu** d'avant S2 (`Δα / (2 × seuil)`, seuil 1° lent / moyen et 0,5° rapide ; `K / 3`) reste dans
le rapport (`BalanceCell::raw_index`, `BalanceReference::raw_index`) pour comparaison, jamais pour décider.
Les rapports mis en cache par un moteur plus ancien (`AnalysisReport::engine_version` < 2) sont recalculés
par le serveur à la lecture.

### 3.1 Calibration sur données réelles (lot S2, novembre 2026)

Étude sur cinq sessions du pilote (copie locale, jamais versionnée) : Spa essais et course (BMW M4 GT3,
2 tours propres chacune), Road Atlanta course (Porsche 992 GT3 R, 15 tours), Long Beach essais ×2 (BMW M4
GT3, 4 et 5 tours) ; 93 à 540 passages (virage, phase, tour) par session. **Aucune session prototype**
(LMP2/LMP3/Hypercar) n'a d'échantillons dans cette copie. Chiffres agrégés seulement.

| Constat | Chiffres |
|---|---|
| Indice absolu toujours positif | 43 cellules sur 45 > 0 ; médiane des cellules par session +0,28 … +0,41 ; 29 constats de sous-virage au total (4 à 7 par session), aucun de survirage |
| Gradient cinématique mort | `steering_angle_deg` = 0 sur 100 % des échantillons (LMU ne donne pas `mPhysicalSteeringWheelRange`, `TODO(verify-on-pc)` dans `corde-lmu::convert`) : seul l'estimateur de dérive a parlé, le rapport de direction n'a jamais été estimé. Essai hors dépôt avec `steering` (−1…1) et un gain estimé à 0,3–0,5 g : virages lents à −4 … −9 σ à Spa (volant non linéaire / grand braquage) — **non retenu** |
| Les dérives avant dépassent l'arrière à toute charge | médianes par tranche de `|g_lat|` : 1,2–1,5 g → `α_AV` 2,4–3,5°, `α_AR` 1,4–1,7° ; 0,3–0,6 g → 0,5–0,6° contre 0,3–0,4° |
| `Δα` (degrés) croît avec la charge | médiane 0,13–0,29° à 0,3–0,6 g, 0,9–1,5° à 1,2–1,5 g, 1,1–2,3° à 1,5–1,8 g (× 4 à 8) : avec une référence globale sur `Δα`, les **milieux de virages moyens** (là où `|g|` est maximal) ressortaient du côté sous-vireur dans **toutes** les sessions (+0,3 … +1,5 σ) — un artefact de charge, pas un défaut |
| `ρ` est plat en charge | médiane 0,13–0,35 dans chaque tranche de 0,3 à 1,8 g, toutes sessions : d'où le choix de `ρ` pour le diagnostic |
| Référence `ρ` par session | médiane 0,21 – 0,27, `σ` robuste 0,08 – 0,12 (au-dessus du plancher 0,05) ; indice absolu de référence +0,32 … +0,41 |
| Cellules relatives | 43 cellules sur 45 entre −1,4 σ et +1,4 σ ; deux se détachent : sortie de virages lents à Spa (essais) −3,1 σ (6 passages), entrée de virages rapides à Long Beach (essais 2) −3,3 σ sur **1 seul passage** (écartée : < 2 passages) |
| Tendance commune | la sortie des virages lents est du côté survireur dans les 5 sessions (−0,4 … −3,1 σ) : motricité, cohérent avec deux voitures GT3 à moteur puissant — **hypothèse**, à confronter au ressenti |

Ce qui est établi par les données : l'indice absolu ne distingue pas les voitures ni les circuits (il
mesure surtout que les pneus avant dérivent plus) ; `Δα` est dominé par le niveau de charge, `ρ` non ; le
gradient cinématique est indisponible sur LMU. Ce qui reste une hypothèse : que les écarts de plus de
1,5 σ correspondent à un défaut ressenti (aucun ressenti du pilote n'est encore associé à ces sessions) ; la
valeur des planchers (0,05 ; 0,5 °/g) ; le facteur 5 σ de l'échelle.

### Blocages / patinage (D-BRK-03, D-TRC-01/02)
- blocage : `slip_ratio < −0,15` pendant **≥ 60 ms**, `brake_raw > 30 %`, `V > 40 km/h`, par roue ;
- patinage : `slip_ratio > +0,15` pendant ≥ 60 ms, `throttle_raw > 50 %`.
Constat blocage si **≥ 0,5 / tour** (majeur ≥ 2 / tour) ; part avant ≥ 70 % ⇒ `BRK_LOCK_FRONT`, ≤ 30 % ⇒
`BRK_LOCK_REAR`, sinon les deux à confiance réduite ; roue avant intérieure en appui pour ≥ 70 % des
blocages avant ⇒ `BRK_LOCK_INNER_FRONT`. Patinage arrière ≥ 1 / tour : `TRC_WHEELSPIN_BOTH` si les deux
roues arrière patinent à ≥ 50 % l'une de l'autre, sinon `TRC_WHEELSPIN_INNER`.

### Pneus (D-TYR-01/02/04/05/06)
- remappage **intérieur / milieu / extérieur** depuis l'ordre gauche / centre / droite du jeu : roues
  gauches (FL, RL) intérieur = `temp_r_c`, extérieur = `temp_l_c` ; roues droites l'inverse
  (`metrics::inner_middle_outer`, testé) ;
- moyennes sur les échantillons en charge (`|g_lat| > 0,3 g`) où la roue est **extérieure** (≥ 50
  échantillons, sinon toute charge, sinon tout le tour) ;
- écart I−O : **> +12 °C ⇒ trop de carrossage** (`TYR_HOT_INNER`), **< 0 ⇒ pas assez**
  (`TYR_HOT_OUTER`, confiance 0,4 : le brief vise +5…+12 et « pas assez » sous +2 — à calibrer) ;
- `ΔM = milieu − (I + O)/2` : > +3 °C surgonflé, < −3 °C sous-gonflé (par essieu) ;
- gradient carcasse AV − AR : > +8 °C ⇒ `TYR_HOT_FRONT`, < −8 ⇒ `TYR_HOT_REAR` ;
- dérive de pression = moyenne du dernier tour − moyenne du premier ; > 8 kPa ⇒ info ;
- usure / tour = (usure début − usure fin) × 100 / nb de tours ;
- température optimale (D-TYR-03) : canal `optimal_temp_c_*` enregistré depuis S2 (la valeur que le jeu
  donne par roue, nulle s'il ne la donne pas) ; `optimal_c` = médiane des échantillons positifs,
  `delta_to_optimal_c` = carcasse moyenne − optimum, `has_optimal_temp` vrai dès qu'une roue l'a. Constat par
  essieu (les deux roues connues) : écart moyen **< −8 °C ⇒ `TYR_COLD`**, **> +8 °C ⇒ `TYR_HOT_FRONT` /
  `TYR_HOT_REAR`** (seuils de `knowledge/diagnostics.yaml`, gravité mineure, confiance 0,6). Sessions
  enregistrées avant S2 : colonne absente = inconnue, comportement d'avant (pas de constat, note dans
  `data_quality`). **Pas encore vu sur une vraie session** : à vérifier que LMU remplit bien le champ.

### Freins (D-BRK-01/04/05)
Fenêtres indicatives sur le **pic typique** par roue (médiane, sur les tours analysés, du maximum de chaque
tour — `typical_peak_c` ; un seul gros freinage ne suffit plus) : GT3/GTE (acier) **350–750 °C**, prototypes
(carbone) **400–1000 °C**, autres 350–800 °C. `Cold` si le pic typique reste sous la borne basse, `Hot` s'il
dépasse la haute ; constat par essieu quand ses deux roues sont dans le même état. Origine : bornes de
`knowledge/setup-parameters.yaml` (`thermal_windows.brakes` : bas de l'optimum, et « chaud » / « fade »).
Vérifié sur les cinq sessions GT3 réelles (§ 3.1) : pics typiques avant 665–805 °C, arrière 580–716 °C ;
l'ancienne borne de 650 °C appliquée au maximum de la session déclarait « chauds » 6 essieux sur 10 (4
sessions sur 5) ; avec 750 °C sur le pic typique, seul l'avant à Long Beach (circuit urbain, 772 et 805 °C)
l'est. Borne basse jamais atteinte (pics par freinage ≥ 460 °C) ; fenêtre carbone non vérifiée (aucune
session prototype). ABS (GT3) : `abs_active` pendant > 30 % du temps de freinage ⇒
`ABS_TOO_ACTIVE`. Stabilité : corrections de volant > 10° (volant lissé 5 points) en freinage droit
(`brake > 0,3`, `|g_lat| < 0,2`) ; > 1 / tour ⇒ `BRK_UNSTABLE_STRAIGHT` (majeur > 3).

### Plateforme (D-PLT-01/03) et amortisseurs (D-DMP-01)
- talonnage : `ride_height_m < 5 mm` pendant ≥ 40 ms (une roue) ; sans canal de hauteur, repli : suspension
  à ≥ 98 % de sa plage observée pendant ≥ 40 ms (seulement si la plage dépasse 5 mm). > 0,5 / tour ⇒
  `PLT_BOTTOMING` (majeur > 2) ;
- saturation de course : % du temps à ≥ 98 % de la plage observée par roue ; > 10 % ⇒ `PLT_BOTTOMING` ;
- histogramme : `v = d(susp_deflection)/dt` (dérivée centrale, lissage 3 points), **+ = compression**,
  lent si `|v| < 25 mm/s` ; `skew_slow = % bump lent − % rebound lent` (cible < 5 points, sinon info) ;
  `pct_slow < 35 %` ⇒ « flotte », `> 92 %` ⇒ « nerveuse » — **informatif** (gravité Info) tant que ces
  seuils ne sont pas calibrés par voiture.

### Style (S-01 … S-05, S-09)
- **agressivité au freinage** = 50 % × (400 − temps de montée 0→80 % du pic en ms)/300 + 50 % × (pic − 0,5)/0,5,
  sur les applications de frein dont le pic dépasse 30 % ; > 75 ⇒ « freinage agressif » ;
- **trail braking** = % des échantillons d'entrée avec `brake > 5 %` et `|g_lat| > 0,3 g` ; > 50 % ⇒ marqué ;
- **lissage** (gaz, volant) = `100 × (1 − inversions/s / référence)` sur la dérivée du signal filtré à 0,1 s,
  inversions comptées au-delà de 0,5/s (gaz) ou 20 °/s (volant), références 4/s et 6/s ; canal absent ⇒ 50 ;
  volant < 40 ⇒ `PLT_NERVOUS` (confiance 0,4) ;
- **vibreurs** = % des échantillons avec une roue sur `Rumble` ou `|g_vert − 1| > 0,5 g` ; > 8 % ⇒
  `PLT_KERB_UNSETTLED` ;
- **roue libre** = % du temps à > 60 km/h sans gaz ni frein ; > 15 % ⇒ info (S-09, nouveau) ;
- `lockup_rate`, `wheelspin_rate` = événements par tour, toutes roues.

### Tours (D-CON-01/02, D-TYR-06)
Régularité = `100 − 20 × CV %` (CV = σ / moyenne des tours propres), bornée ; < 60 ⇒ `PHILOSOPHY_SAFE`,
> 85 ⇒ `PHILOSOPHY_AGGRESSIVE` (gravité mineure : ils soutiennent les règles de philosophie). Dégradation =
pente des temps au tour propres vs numéro de tour (≥ 4 tours, **non corrigée du carburant**) ; > 0,08 s/tour
⇒ `TYR_WEAR_FAST` (majeur > 0,25), pneu limitant = usure > 1,3 × les autres.

### Conditions (D-WET-01)
Pluie si `rain > 0,2` ou `wetness_avg > 0,3` (météo de départ, tours ou échantillons) ⇒ `WET_BASELINE`
(majeur) ; carcasse moyenne < 55 °C sous la pluie ⇒ `WET_NO_TEMP`.

## 4. Moteur de règles (`rules.rs`)

Pour chaque philosophie :

1. **Filtre** : surface `dry|any` pour agressif / sûr, `wet|any` pour pluie ; philosophie `both` + la
   sienne (le plan pluie prend aussi les règles `safe`) ; classe de la voiture dans `classes` ; un
   changement dont le paramètre n'existe pas pour la classe (`brake.migration` sur GT3…) est ignoré.
2. **Déclenchement** : une règle tire quand un constat de gravité ≥ mineure porte son `symptom` ;
   poids = confiance × (1 mineur, 1,5 majeur). La règle de base de la philosophie (`PHI-AGGRESSIVE-01`,
   `PHI-SAFE-01`, `WET-PACK-01`) tire toujours — poids 0,45 et rationale « sans diagnostic » si rien ne la
   soutient.
3. **Score** d'un changement = poids × confiance du changement (high 0,9 / medium 0,65 / low 0,4)
   × `1 / (1 + 0,35 (prio − 1))` × biais de famille 1,25 (sûr : braking, oversteer, platform, tyres ;
   agressif : understeer, traction, straightline — ARB-02 / ARB-05).
4. **Agrégation par paramètre** : même sens ⇒ score = meilleur + ½ des autres, règles cumulées dans
   `rule_ids` ; sens opposés ⇒ le meilleur gagne (ARB-01), le perdant va dans `conflicts_with`, avertissement
   si son score atteint 50 % du gagnant.
5. **Limite** : les `max_changes_per_plan` (8) meilleurs scores ; `priority` 1..5 par rang ; conflits
   déclarés (`conflicts:` du YAML) entre règles survivantes sur des paramètres différents ⇒ avertissement
   ARB-02 ; liste finale ordonnée selon la méthodologie E.1 : pneus → aéro → freins → barres → ressorts /
   3e élément / packers → amortisseurs → différentiel → géométrie → électronique.
6. Directions : `increase`, `decrease`, `toward_zero`, `set` (valeur dans `magnitude`) ; `check` et
   `either` deviennent des avertissements ; `meta` est ignoré (les règles d'équilibre tirent déjà sur leurs
   propres constats). `steps` = entier en tête de la magnitude quand il y en a un (« 2 clics » → 2).
7. `parameter_key` / `section` = clé `.svm` de `setup-parameters.yaml` quand elle existe, sinon l'identifiant
   logique. Les magnitudes restent **relatives** dans le rapport mis en cache ; le serveur les applique au
   réglage réel du pilote à chaque lecture (§ 8).

Le plan pluie est produit même sur une session sèche (avertissement « paquet pluie générique »).

## 5. Ce qui reste à calibrer sur données réelles

| Sujet | Hypothèse actuelle | À faire |
|---|---|---|
| Orientation gauche / centre / droite des 3 zones de température | convention rFactor (`temp_c[0]` = gauche du pneu) | vérifier en braquant d'un côté sur le PC du pilote |
| Seuils carrossage I−O | > 12 trop, < 0 pas assez | brief : cible +5…+12, pas assez < +2 ; mesurer sur GT3 et Hypercar |
| Fenêtres freins | pic typique par tour : 350–750 (GT3, vérifié sur 5 sessions réelles), 400–1000 (proto, base de connaissances seulement) | confirmer sur la couleur du HUD (v1.4) ; une session prototype réelle |
| Équilibre relatif | `ρ` des dérives, référence = médiane des passages, σ = 1,4826 × MAD (plancher 0,05), constat à 1,5 σ sur ≥ 2 passages, ±1 = 5 σ | associer des ressentis du pilote aux constats ; gradient cinématique quand LMU donnera l'angle volant |
| Seuil lent / rapide des amortisseurs | 25 mm/s | HDV par voiture ; seuils « flotte / nerveuse » (35 % / 92 %) non calibrés |
| Empattements et rapport de direction | 2,7 / 3,0 / 3,1 m, `i_s` estimé (repli 12) ; sans effet sur LMU tant que `steering_angle_deg` = 0 | par voiture |
| Classes de vitesse des virages | 90 / 160 km/h (+20 protos) | par circuit / classe |
| Blocage / patinage | slip ±0,15, 60 ms | le brief dit −0,25 / 80 ms pour le blocage : à trancher sur de vrais tours |
| Régularité | `100 − 20 × CV %` | comparer au ressenti du pilote |
| Dégradation | pente brute | corriger du carburant (`k ≈ 0,03 s/L`, D-TYR-06) |
| Température optimale | enregistrée depuis S2 (`optimal_temp_c_*`, OptF32), D-TYR-03 à ±8 °C | vérifier sur une session réelle que LMU la remplit ; D-TYR-08 (tours pour entrer en fenêtre) |
| Simulateur | `slip_ratio` ≤ 0,03 en valeur absolue, hauteurs ≥ 49 mm | aucun blocage / talonnage n'est jamais produit par le sim : les constats correspondants ne sont testés qu'en synthétique |

## 6. Tests

- `cargo test -p corde-engineer` : séries synthétiques (un virage à droite avec entrée / milieu / sortie,
  indice absolu de sous-virage par angles de dérive, `ρ` indépendant de la charge, référence robuste et
  indice relatif (plancher, saturation, accord des estimateurs), blocage AV compté une fois, remappage I/O,
  pneus, freins (pic typique, fenêtres acier / carbone), plateforme, histogramme, style, statistiques de
  tours), fixture YAML minimale du moteur (conflit sur un paramètre, agrégation, limite, plan générique, plan
  pluie), bout en bout sur 3 tours synthétiques : milieu de virage plus sous-vireur que l'entrée et la
  sortie ⇒ `US_MID_SLOW` ; voiture **uniformément** sous-vireuse ⇒ aucun constat d'équilibre ; survirage en
  sortie seulement ⇒ `OS_EXIT_SLOW` ; température optimale absente / présente (D-TYR-03) ; déterminisme,
  JSON.
- `cargo test -p corde-store` : canal `optimal_temp_c_*` (nullable) ; un fichier Parquet écrit sans une
  colonne se relit avec des nulls pour elle (`samples::read_columns`).
- `cargo test -p corde-server --test analysis_sim` : session simulateur enregistrée (tours de 30 s joués
  ×3, 20 Hz), 404 / 400 / 422, rapport typé en direct (référence d'équilibre, température optimale 90 °C
  du simulateur), cache après fermeture, recalcul, rapport en cache d'un moteur plus ancien recalculé, puis
  `analyse_session` sur le store rouvert.
- `cargo test -p corde-engineer dump_synthetic_report -- --ignored --nocapture` : imprime le rapport JSON
  des tours synthétiques.

## 7. Ressenti du pilote → symptômes → itération du plan

```
texte libre ──extract──▶ FeedbackInterpretation ──iterate_plan──▶ PlanIteration (delta + plan cumulé)
                                 │                       ▲
                                 │     constats du dernier rapport (AnalysisReport::findings)
                                 └──── stocké avec le ressenti (feedback.interpretation_json)
```

Serveur : `POST /api/sessions/{id}/feedback` `{plan?, text}` interprète le texte (dans `spawn_blocking`),
stocke ressenti + interprétation, construit l'itération et renvoie **201**
`{feedback, interpretation, iteration}` ; `GET …/feedback` joint l'interprétation ;
`GET /api/sessions/{id}/iterations?plan=` liste les itérations (plus ancienne d'abord). Stockage :
migration **v6** de `corde.db` (`feedback.interpretation_json`, table `plan_iterations`, cascade avec la
session). `/api/health` indique `"llm": "dictionary" | "anthropic"`.

### 7.1 Dictionnaire hors-ligne (défaut)

`DictionaryExtractor` = `knowledge/feedback-dictionary.yaml` (embarqué ; `FeedbackDictionary::from_dir`
pour une surcharge par fichier) + expressions régulières FR / EN écrites dans `feedback.rs` pour les
tournures réelles (« ça sous-vire », « l'arrière part à la remise des gaz », « it snaps on exit »…).

1. **Normalisation** : minuscules, accents repliés, apostrophes et ponctuation → espaces ; `.;!?` et
   retours à la ligne séparent les phrases, `, : ( ) /` et « mais / et / puis / but / and » les
   propositions. Un caractère de sortie par caractère d'entrée : `matched_text` cite le texte d'origine.
2. **Symptômes** : chaque proposition est cherchée (phrases du YAML, confiance de base 0,85, et motifs
   intégrés). L'identifiant est celui de `setup-rules.yaml` quand le texte est assez précis
   (`US_ENTRY_SLOW`), sinon une **famille générique** (`US`, `US_ENTRY`, `OS_EXIT`, `BRK_LOCK`,
   `TRC_WHEELSPIN`, `TYR_HOT`, `NO_GRIP`, `JUMPS`, `UNSTABLE`) que l'itération développe avec les données.
3. **Modificateurs** de la proposition : phase (entrée / freinage, milieu / corde, sortie / remise des
   gaz), vitesse (lents / épingles, rapides / courbes), essieu, intensité (« un peu » → `mild`,
   « beaucoup » → `marked`, « trop », « énormément » → `severe`), **négation** (« pas de », « ne … plus »,
   « no … anymore »), pluie. Une proposition sans symptôme prête ses modificateurs à sa voisine dans la même
   phrase (« ça sous-vire, surtout dans les lents ») ; une nouvelle phrase repart de zéro.
4. **Questions** : ce qui manque (phase, type de virage, essieu) + les `clarify` du YAML, en FR et EN
   alignés ; le bouton « Préciser » de l'UI les recopie dans la zone de texte. Les morceaux non reconnus
   vont dans `unmatched_fragments`, les jugements (« c'est mieux ») dans `notes_fr`.
5. **Confiance** globale = confiance moyenne des symptômes × (0,5 + 0,5 × part des propositions
   comprises) ; 0 si rien n'est reconnu (une question invite alors à reformuler).

Tests : `cargo test -p corde-engineer feedback` — 45 phrases réalistes FR / EN dans le tableau de cas, plus une dizaine de tests dédiés (sous-virage, survirage,
freins, motricité, plateforme, pneus, pluie, négations, fautes de frappe et majuscules), messages mixtes
(« le train avant pousse en entrée mais l'arrière part à la remise des gaz dans les lents » → deux
symptômes avec chacun leur phase, vitesse et essieu), phrases à négation et symptôme présent mêlés.

### 7.2 Modèle de langage (optionnel, expérimental)

`AnthropicExtractor` : **désactivé par défaut**. Activé par la variable `CORDE_ANTHROPIC_API_KEY` ou par
`--llm anthropic` (qui exige aussi la clé ; `--llm none` force le dictionnaire même si la variable
existe). Appel unique à l'API Messages (`ureq`, délai global **8 s**, modèle `claude-haiku-5-5`), réponse
contrainte par un schéma JSON strict (`output_config.format`), identifiants de symptômes limités à ceux
des règles (les autres sont écartés avec une note). **Repli silencieux** sur le dictionnaire en cas
d'erreur, de délai dépassé ou de réponse sans symptôme (une note `notes_fr` le signale). Jamais testé
contre le vrai service : les tests vérifient la requête construite, l'analyse d'une réponse écrite en dur
et le repli sur un port fermé.

### 7.3 Itération (`iterate_plan`)

1. **Constats virtuels** : chaque symptôme ressenti devient un `Finding` `FEEL/<symptôme>` ; confiance =
   celle de l'extracteur × intensité (0,8 « un peu », 0,9 neutre, 1 marqué) ; gravité majeure si
   « très marqué » (ou marqué et confiance ≥ 0,6). Une famille générique est développée : si le dernier
   rapport voit déjà un des cas (`US_ENTRY` → `US_ENTRY_SLOW`), c'est celui-là ; sinon tous les cas, à
   confiance réduite (lent 0,85 / rapide 0,6 ; milieu 0,8 / entrée et sortie 0,6).
2. **Fusion avec les données** (constats ≥ mineurs du rapport de la session) :
   - même symptôme des deux côtés → **concordance**, confiance du constat + 0,15 (bloc vert) ;
   - équilibre opposé sur la même phase (et la même vitesse si dite) → **désaccord** (bloc orange) :
     les **données gagnent si leur confiance > 0,6** (et ≥ 3 tours analysés), sinon le **ressenti** ;
     l'explication dit lequel est suivi. **Exception depuis S2** : un constat d'équilibre de l'ingénieur
     (D-BAL-01) est relatif à la référence de la session, le ressenti est absolu — le **ressenti gagne
     toujours** (« ton ressenti passe devant »). Rien dans l'itération ne dépend du signe absolu de
     l'indice : elle ne lit que les symptômes des constats ;
   - symptôme **nié** (« ne glisse plus ») : il retire un constat de données faible (≤ 0,6) — et tout constat
     d'équilibre relatif ; un autre constat fort est gardé avec un désaccord « à revérifier sur 5–8 tours » ;
   - ressenti sans écho dans les données : suivi, confiance × 0,9, avec une note.
3. **Moteur de règles en mode itération** (`rules::plan_scored` sur les constats fusionnés, puis filtre) :
   - un paramètre déjà dans le plan avec le même sens est ignoré ;
   - **pas de retour arrière** sur un paramètre réglé dans l'autre sens à l'itération précédente, sauf si
     le ressenti le demande **explicitement** : symptôme précis (pas une famille devinée, sauf si les
     données la confirment) et formulé au moins normalement (« un peu » ne suffit pas) ; le retour est
     alors expliqué (« inversion demandée par le ressenti »), sinon une note dit pourquoi on n'inverse pas ;
   - les changements portés par le ressenti passent devant ceux des seules données ;
   - **une seule famille** de paramètres, celle du meilleur candidat (ordre méthodologique : pneus → aéro →
     freins → barres → ressorts → amortisseurs → différentiel → géométrie → électronique), au plus
     **3 nouveaux changements** (`MAX_NEW_CHANGES`, ARB-03), chacun d'**un cran**
     (`one_step_magnitude` : `default_step` de `setup-parameters.yaml`, borne basse d'une fourchette,
     sinon la magnitude de la règle ramenée à une unité) ; les familles suivantes
     sont citées en « pistes suivantes ».
4. **Sortie** : `delta` (ce qui change maintenant, justification préfixée « Itération n — »), `changes`
   (plan cumulé : plan de départ ou itération précédente + delta, un paramètre une fois), numéro
   d'itération par (session, plan), `data_findings_used`, `agreements_fr`, `disagreements_fr`, `notes_fr`.
   Plan de départ de la 1ʳᵉ itération : le plan du rapport pour la philosophie testée (`Feedback::plan`,
   sinon le dernier plan itéré de la session, sinon « sûr »). Sans rapport (moins de 3 tours propres),
   l'itération repose sur le ressenti seul (note). Un ressenti sans symptôme reconnu crée quand même une
   itération, à delta vide, qui renvoie aux questions.

Tests : `cargo test -p corde-engineer iterate` (développement des familles, crans, une famille à la fois,
2ᵉ itération et delta, concordance / désaccord, négation, retour arrière refusé puis accepté) ;
`cargo test -p corde-server --test feedback_api` (« l'avant ne tourne pas en entrée des virages lents » →
`US_ENTRY_SLOW` entrée / lent, itération 1 avec un changement de la règle `US-ENTRY-SLOW-01` ; ressenti
suivant → itération numérotée avec un delta sur un autre paramètre ; filtres, 400 / 404, cascade).

### 7.4 À calibrer sur de vrais ressentis

| Sujet | Hypothèse actuelle |
|---|---|
| Seuil d'arbitrage données / ressenti | 0,6 (`DATA_WINS_ABOVE`) |
| Bonus de concordance | + 0,15 |
| Poids des cas d'une famille générique | lent 0,85 / rapide 0,6 ; milieu 0,8 / entrée, sortie 0,6 |
| Vocabulaire | tournures du pilote à collecter (les ressentis stockés servent de corpus) ; ajouter les manquantes au YAML |
| Un cran | `default_step` du catalogue pour le delta ; converti en crans du jeu sur le vrai réglage (§ 8) |

## 8. Le vrai réglage : fichiers `.svm` lus, plans résolus, fichiers écrits

```
UserData/player/Settings/<Circuit>/*.svm ──scan──▶ bibliothèque ──voiture + circuit + date──▶ réglage de base
                                                       │ (même voiture)                         │
                                                       └──▶ LabelBook (index ↔ libellé, pas) ───┤
garage du jeu (8 GET, lecture seule) ──▶ diagnostics/lmu-api/garage/<voiture>/ ──▶ bornes ───────┤
AnalysisReport.plans / PlanIteration (relatifs) ──────── setup::resolve_* ◀──────────────────────┘
        └──▶ plans résolus (index avant → après, libellés, statut) ──build_setup──▶ Corde_<Plan>_<base>.svm
```

### 8.1 Ce que le fichier dit (vérifié sur six fichiers réels du pilote, trois voitures)

- CRLF, UTF-8 (« Détaché »), en-tête `VehicleClassSetting="…"` (ordre des mots variable : `BMW_M4_LMGT3 GT3
  WEC2026`, `GT3 Porsche_911_GT3_R_LMGT3 WEC2025`, `ELMS2025 LMP3 Ligier_JS_P325`), `UpgradeSetting`, lignes
  `//…`, 15 sections séparées par une ligne vide, `Clé=index//libellé`, `Notes="…"` dans `[GENERAL]`.
  `corde_lmu::svm` relit et réécrit ces fichiers **à l'octet près** (test sur les fixtures
  `crates/corde-lmu/tests/fixtures/svm/` ; `CORDE_SVM_DIR=… cargo test -p corde-lmu --test svm_fixtures --
  --ignored` pour un dossier entier).
- **Le jeu ne lit que l'index.** Le libellé après `//` est réécrit par le jeu quand on sauve dans le garage ; le
  pilote en écrit parfois à la main (« +2 crans ») : Corde ne s'en sert jamais pour calculer
  (`setup::is_hand_label`).
- **Non réglable** : libellé `Non-adjustable`, `N/A`, `Fixed` ou `Détaché` (index 0) — ou `min == max` dans le
  garage du jeu. BMW M4 GT3 : `DiffPowerSetting` / `DiffCoastSetting` non réglables, `DiffPreloadSetting`
  réglable ; Ligier LMP3 : les trois, et les 3ᵉˢ éléments.

### 8.2 Sens de l'index (`svm.index_sign` de `setup-parameters.yaml`)

L'index suit toujours le même sens d'une voiture à l'autre (les libellés, non) :

| Index ↑ = | Paramètres | `index_sign` (pour le « + » des règles) |
|---|---|---|
| plus d'aileron, barre / ressort / amortisseur / packer plus dur, caisse plus haute, plus de pincement, plus de pression, plus de précharge, plus de pression de frein | `aero.rear_wing`, `susp.*`, `aero.rh_*`, `geo.toe_*`, `tyre.pressure_*`, `diff.*`, `brake.pressure` | +1 |
| carrossage **moins négatif** (BMW 22 ↔ −2.20°, 40 ↔ −1.00°) | `geo.camber_*` (« + » = plus négatif) | −1 |
| **plus de frein à l'arrière** (BMW 22 ↔ 51.5:48.5, 34 ↔ 48.5:51.5) | `brake.bias_front` (« + » = plus à l'avant) | −1 |
| écopes **plus fermées** (0 Open, 1 33 %, 2 67 %, 3 Closed) | `brake.duct_*` (« + » = plus ouvert) | −1 |
| plus de degrés de braquage (direction moins directe) | `steer.lock` (« + » = plus direct) | −1 |
| numéro de la cartographie affiché | `elec.tc*` (`label_is_index`), `brake.abs_map` | +1 |
| valeur discrète (composé, rapports…) | `tyre.compound`, `trans.ratio_set` | 0 : à la main |

Attention aux libellés trompeurs : les amortisseurs de la Porsche affichent un nombre qui **décroît** quand
l'index monte (5 ↔ « 13 », 8 ↔ « 10 ») ; le sens de l'index, lui, ne change pas.

`aero.rake` n'a pas de clé : il est appliqué par la hauteur de caisse **arrière** (`svm_translation`), l'avant ne
bouge pas ; s'il arrive avec un changement de la hauteur arrière dans le même plan, le second est marqué
`duplicate`.

### 8.3 Bornes

- Minimum : 0 (le premier index), toujours.
- Maximum : pas dans le fichier. Il vient du garage du jeu quand Corde l'a lu (`corde_lmu::garage::extract_bounds`
  sur `getPlayerGarageData.json`, objets `VM_*` ou clés `.svm`, plusieurs formes acceptées, « inconnu » sinon) ;
  la correspondance `VM_*` ↔ paramètre passe par `rest_key` du catalogue. **Forme réelle de la réponse non
  encore vue** : à lire sur le PC du pilote.
- Sans maximum connu, une hausse est limitée à **+2 crans** par changement (`MAX_STEPS_WITHOUT_BOUND`) et
  marquée `unknown_bounds` (« à vérifier au garage »).

Capture (`corde-server::lmu_garage`) : jeu détecté, pilote au garage / au stand / dans les menus (pas en piste),
une fois par voiture et par session, au plus une tentative par minute (trois échecs et on laisse), **liste
fermée** de 8 `GET` (`/rest/garage/getPlayerGarageData`, `/summary`, `/UIScreen/CarSetupOverview`, `/setup`,
`/tireinfo`, `/brakeinfo`, `/rest/race/car`, `/rest/race/track`), réponses brutes dans
`<données>/diagnostics/lmu-api/garage/<voiture>/<nom>.json` (réécrites seulement si elles changent), une ligne
de journal par capture, coupé par `--no-game-api`.

### 8.4 Crans et libellés appris (`setup::LabelBook`)

Pour une voiture (même `VehicleClassSetting`), tous les fichiers de la bibliothèque donnent des couples
index ↔ libellé par clé (roues avant ensemble, arrière ensemble ; libellés écrits à la main et valeurs non
réglables ignorés).

1. **Libellé après** : celui vu pour cet index dans un fichier, sinon une **loi linéaire** apprise sur au
   moins deux libellés numériques de même forme (`-2.20 deg`, `P2`, `51.5:48.5`, `136 kPa`, `33%`, `13` ;
   refus de `120 kgf (100%)`, `1 (tendre)`, `96% Medium`…), vérifiée (écart ≤ 0,2 pas + arrondi), utilisée au
   plus à une étendue (≥ 3 crans) des index connus, jamais hors de 0–100 pour un pourcentage ; les cartographies
   TC reprennent l'index. Sinon `null` : l'UI montre `cran 7` et le fichier porte `//Corde : -1 cran`.
2. **Magnitude → crans** : `1 cran`, `2 clics` → tel quel ; une quantité physique (`−3 kPa`, `0.2–0.3°` →
   borne basse, `0.5–1 %`, `2 mm`, `15 Nm`) est divisée par le **pas appris** (pente de la loi, unités
   converties mm/cm, psi/kPa), sinon par `index_step` du catalogue s'il est de confiance ≥ medium (répartition
   0,25 %/cran, hauteurs et packers 1 mm/cran), sinon **1 cran** et une note. Exemple vérifié : carrossage BMW
   appris sur deux fichiers (22 ↔ −2.20°, 40 ↔ −1.00°) → 0,0667°/cran ; « 0.2–0.3° plus négatif » = −3 index,
   22 → 19, libellé `-2.40 deg`.
3. `toward_zero` (pincement) : sens donné par la valeur actuelle (libellé ou loi), sans dépasser zéro.

### 8.5 Plans et itérations résolus

`resolve_report_plans` : les trois plans du rapport (en cache, relatifs) sont **recalculés** par le moteur de
règles **sans les paramètres que la voiture ne permet pas** (`unavailable_params` : toutes les clés absentes
ou non réglables) — le levier suivant prend la place —, puis chaque changement reçoit `resolved` :

| Statut | Sens | Dans le fichier |
|---|---|---|
| `applied` | appliqué dans les bornes | oui |
| `clamped_min` / `clamped_max` | arrêté à 0 / au maximum du garage | oui (si l'index bouge) |
| `unknown_bounds` | hausse sans maximum connu, ≤ +2 crans | oui |
| `manual` | valeur discrète (pneus pluie), sens inconnu | non : à faire à la main |
| `not_adjustable` / `unknown_key` / `duplicate` | écarté (`plan.dropped`, avertissement) | non |

Les itérations sont stockées relatives et résolues à la lecture (`resolve_iteration`, `current_setup_known`,
`dropped`) ; une nouvelle itération ne propose pas de paramètre non réglable
(`IterationContext::unavailable_params`).

### 8.6 Choix du réglage de base (`corde_lmu::svm_library`, `corde-server::setups`)

- Bibliothèque : `Settings/*/*.svm` (un niveau, fichiers illisibles listés, jamais bloquants).
- Voiture : les mots distinctifs de `vehicle_model` (`BMW M4 LMGT3` → `bmw`, `m4` ; classes, séries et saisons
  ignorées) doivent tous figurer dans `VehicleClassSetting` (lettres et chiffres comparés).
- Circuit : table d'alias des dossiers de LMU (`Longbeach` ← « Long Beach », `Road Atlanta` ← « Michelin
  Raceway », `Spa`, `Lemans` ← « Sarthe », `Paulricard`, `Sebring`, `Daytonarc`, `Lagunaseca`, `Circuit De
  Barcelona`, `Circuit Of The Americas`, `Interlagos`, `Portimao` ← « Algarve », `Qatar` ← « Lusail », `Fuji`,
  `Imola`, `Monza`, `Bahrain`, `Silverstone`) puis rapprochement flou sur les dossiers existants.
- Base proposée : le fichier de la voiture modifié le plus récemment **avant la fin de la session** dans le
  dossier du circuit, sinon dans un autre dossier, sinon le plus récent ; un fichier écrit par Corde après le
  début de la session n'est jamais proposé. Le pilote peut en choisir un autre (`?base=Dossier/Fichier.svm`).

### 8.7 Écriture (`build_setup`, `POST /api/sessions/{id}/setup-files`)

- Le fichier de base, indices modifiés, libellé `//<libellé déduit>` ou `//Corde : +1 cran`,
  `Notes="Corde - <plan> - base : <fichier> - RearAntiSway -1, Camber AV -3…"` (ASCII, ≤ 200 caractères), tout
  le reste identique à l'octet (CRLF, UTF-8, commentaires). Vérifié : « barre arrière −1 cran » appliqué à
  `BMW_LongBeach.svm` redonne le `BMW_LongBeach_v2.svm` du pilote, notes à part.
- Uniquement sur demande explicite (bouton de l'UI), depuis ce PC (même garde que `PUT /api/settings`), dans
  le dossier de la base, nom `Corde_<Agressif|Sur|Pluie|IterationN_Sur>_<base>.svm`, `_2`, `_3`… si le nom
  existe (lien physique d'un fichier temporaire : jamais d'écrasement), chemins `Dossier/Fichier.svm` validés
  (aucune sortie de `Settings`, extension `.svm`), 409 si `UserData` est inconnu. Chaque fichier créé est
  journalisé.

### 8.8 Limites

- Forme réelle de `getPlayerGarageData` inconnue : sans elle, pas de maximum (≤ +2 crans) ; le catalogue `rest_key`
  ne couvre que quelques `VM_*`.
- Pas physiques par défaut mesurés sur une ou deux voitures : ceux de confiance `low` (carrossage, pincement,
  pression, aileron GT3, précharge) ne servent qu'appris sur les fichiers du pilote.
- Un libellé déduit est une interpolation : le jeu le réécrit à la sauvegarde, seul l'index compte.
- Le réglage réellement chargé pendant la session n'est pas lu (le choix de la base est une déduction par
  dates) ; le pilote le corrige dans la liste.
- Le cumul d'une itération remplace la magnitude d'un paramètre déjà dans le plan (comportement de § 7.3).

Tests : `cargo test -p corde-lmu --test svm_fixtures` (aller-retour à l'octet, v1 → v2, non-réglables,
bibliothèque), `-p corde-lmu --lib svm_library garage`, `-p corde-engineer --lib setup` et `--test setup_real`
(v2 reproduit, BMW / Ligier, pas du carrossage appris), `-p corde-server --lib lmu_garage setups` (8 GET
seulement, une fois par voiture et session, noms de fichiers, jamais d'écrasement) et `--test setups_api`
(bibliothèque, analyse et itérations résolues, aperçu, création, `_2`, 400 hors `Settings`, 403 hors de ce PC,
409 sans dossier du jeu).
