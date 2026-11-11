# Base de connaissances « ingénieur » (`knowledge/`)

> Ce dossier contient les **données** (pas le code) qui permettent à Corde de passer de la télémétrie et du
> ressenti du pilote à une proposition de réglage. Tout est dérivé des briefs de recherche
> `docs/research/02-ingenieur-de-course.md` (règles, diagnostics, ressenti, méthode) et
> `docs/research/01-telemetrie-lmu.md` (format `.svm`, API REST, DuckDB). Aucun chiffre n'a été ajouté par
> rapport aux briefs ; chaque entrée porte son niveau de confiance et ses sources.

| Fichier | Rôle | Entrées | Consommé par |
|---|---|---|---|
| `setup-parameters.yaml` | Dictionnaire des paramètres de setup LMU : identifiant canonique, clé `.svm`, objet REST, champ live du protocole, classes, BoP, plages indicatives, pas par défaut | 52 paramètres, 12 leviers BoP, 2 aides pilote, fenêtres thermiques | moteur de règles, écriture `.svm`, UI « setup » |
| `diagnostics.yaml` | Catalogue des diagnostics calculables à partir des `TelemetryFrame` (méthode, seuils, sorties, symptômes déclenchés) | 5 étapes de prétraitement, 46 diagnostics | `corde-engineer` (`diagnostics.rs`, voir §6) |
| `setup-rules.yaml` | Règles « symptôme → modifications ordonnées » avec effets secondaires, conflits et philosophie | 46 règles, ≈ 270 changements, 23 effets croisés, 5 règles d'arbitrage | `corde-engineer` (`rules.rs`, embarqué par `include_str!`) |
| `feedback-dictionary.yaml` | Ressenti pilote en texte libre (FR/EN) → identifiants `FEEL_*` → symptômes, avec questions de clarification | 56 entrées + 5 alias | interprète de ressenti (formulaire, regex, LLM) |
| `methodology.md` | Comment enchaîner tout cela : ordre d'itération, paquets vs variable unique, pondération ressenti/données, adaptation par circuit, génération des 3 setups, boucle d'affinage | document | développeurs et relecteurs humains |

Les quatre YAML sont valides (`python3 -c "import yaml; yaml.safe_load(open('…'))"`) ; `setup-parameters.yaml`
est en `schema_version: 2` (correspondance avec le fichier `.svm` réel, §3.1), les trois autres en `schema_version: 1`.
Toute évolution de structure incrémente `schema_version` et met à jour ce README.

---

## 1. Niveaux de confiance

Les briefs utilisent `[C=H] / [C=M] / [C=L]` (ou `[VÉRIFIÉ] / [PROBABLE] / [À CONFIRMER PC]`). Les YAML les
reprennent en toutes lettres :

| Valeur | Sens | Ce que le code a le droit d'en faire |
|---|---|---|
| `high` | Documentation officielle, code source lu, fichier réel, ou plusieurs sources concordantes | Peut servir de base à un comportement par défaut |
| `medium` | Source unique crédible, ou principe de dynamique véhicule classique dont la transposition à LMU est probable | Comportement par défaut **affiché avec sa provenance**, désactivable |
| `low` | Valeur indicative (forum, expérience rF2) — **à vérifier dans le jeu** | Jamais codé en dur : contrôle de vraisemblance uniquement, ou proposition « à confirmer » |

Dans `diagnostics.yaml`, la confiance est scindée en `method` (la méthode de calcul est-elle établie ?) et
`thresholds` (les seuils sont-ils fiables ?) : une méthode `high` avec des seuils `low` signifie « calculez
l'indicateur, mais calibrez les seuils par classe et par circuit avant d'en tirer une conclusion ».

**Règle d'architecture commune** (brief 02) : *jamais de valeur absolue de setup codée en dur*. Les règles
produisent des directions et des pas relatifs (« −1 cran », « +3 kPa ») ; la plage réelle et le cran courant
se lisent à l'exécution dans le setup chargé (`GET /rest/garage/getPlayerGarageData`, fichier `.svm`,
`metadata.CarSetup` d'un fichier DuckDB natif).

---

## 2. Identifiants partagés entre fichiers

```
feedback-dictionary.entries[].symptoms ──► setup-rules.rules[].symptom
diagnostics.diagnostics[].symptoms   ──► setup-rules.rules[].symptom
setup-rules.rules[].trigger.feel     ──► feedback-dictionary.entries[].id  (ou aliases[].id)
setup-rules.rules[].trigger.diagnostics ─► diagnostics.diagnostics[].id
setup-rules.rules[].changes[].param  ──► setup-parameters.parameters[].id  (= setup-rules.params[].id)
setup-rules.rules[].changes[].conflicts ► setup-rules.rules[].id
setup-parameters.parameters[].live_field ► chemin dans corde_protocol::TelemetryFrame
diagnostics.*.inputs                 ──► chemin dans corde_protocol::TelemetryFrame / LapRecord
```

Familles d'identifiants :

- **Symptômes** (`setup-rules.rules[].symptom`) : `US_*` sous-virage, `OS_*` survirage (suffixes
  `_ENTRY|_MID|_EXIT` × `_SLOW|_FAST`), `BRK_*` freinage, `TRC_*` motricité, `TC_TOO_INTRUSIVE`,
  `ABS_TOO_ACTIVE`, `HYB_SOC_*` hybride, `PLT_*` plateforme, `STR_*` ligne droite / réponse, `TYR_*` pneus,
  `WET_*` pluie, `PHILOSOPHY_AGGRESSIVE` / `PHILOSOPHY_SAFE`. Dans `feedback-dictionary.yaml`, un symptôme
  terminé par `*` est un motif glob (`US_*` = toutes les règles de sous-virage).
- **Règles** : `<FAMILLE>-<QUALIFIANT>-NN` (`US-ENTRY-SLOW-01`, `PHI-SAFE-01`, `WET-PACK-01`).
- **Diagnostics** : `D-<CAT>-NN` (`D-TYR-01`, `D-BAL-01`, `D-PLT-01`…) et `S-NN` pour les indicateurs de style
  de pilotage (`S-01` … `S-08`).
- **Ressenti** : `FEEL_<DESCRIPTION>` (`FEEL_FRONT_WONT_TURN_IN`).
- **Paramètres** : `<groupe>.<nom>` (`aero.rear_wing`, `susp.arb_front`, `elec.tc_slip`), suffixes `_front` /
  `_rear` pour les paramètres par essieu.

---

## 3. Schémas

### 3.1 `setup-parameters.yaml`

```yaml
schema_version: 2
sources: [<urls et fichiers>]
svm_format:            # structure d'un fichier .svm (emplacement, en-tête, sections, forme des valeurs, encodage, rechargement)
parameters:            # liste
  - id: aero.rear_wing             # identifiant canonique (clé étrangère de setup-rules)
    label_fr: "Aileron arrière"
    group: aero                    # aero | suspension | dampers | geometry | tyres | brakes | differential |
                                   # electronics | energy | transmission | strategy | steering | cooling | legacy
    svm:                           # null pour un pseudo-paramètre dérivé (ex. aero.rake)
      section: REARWING            # section INI ; "FRONTLEFT / FRONTRIGHT" quand per_wheel
      key: RWSetting               # clé ; la valeur stockée est un INDEX, l'affichage suit après `//`
      per_wheel: false             # true = existe dans [FRONTLEFT] [FRONTRIGHT] [REARLEFT] [REARRIGHT]
      confidence: high             # high = lue dans un fichier réel ; medium = héritage rF2 probable
      index_sign: 1                # v2 : +1 = l'index monte quand le paramètre augmente (son `sign` dans
                                   # setup-rules.params), −1 = l'inverse (carrossage, répartition, écopes,
                                   # braquage), 0 = valeur discrète / sens inconnu (jamais modifiée par Corde)
      label_is_index: false        # v2 : true = le jeu affiche l'index lui-même (cartographies TC `8//8`)
    # `section` : "A / B" = plusieurs sections (roues d'un essieu), "FRONTLEFT … REARRIGHT" = les 4 roues ;
    # `key` : "K1 / K2" = plusieurs clés (chasse gauche / droite). Corde applique le changement à chacune.
    index_step:                    # v2, optionnel : pas physique d'un cran par défaut, quand la bibliothèque du
      value: 0.25                  # pilote ne permet pas d'apprendre celui de SA voiture ; utilisé seulement
      unit: "%"                    # en confiance medium/high (sinon « 1 cran » et une note)
      confidence: medium
      source: "BMW M4 LMGT3 : 22 ↔ 51.5:48.5, 34 ↔ 48.5:51.5"
    svm_translation:               # v2, optionnel : paramètre logique sans clé appliqué par un autre
      param: aero.rh_rear          # (aero.rake → hauteur arrière), avec une note expliquée au pilote
      note: "…"
    related_svm_keys: [...]        # optionnel : clés voisines (packer / tender du 3e élément, etc.)
    svm_display_example: "..."     # optionnel : exemple de libellé affiché après `//`
    rest_key: VM_BRAKE_BALANCE     # optionnel : objet VM_* de GET /rest/garage/getPlayerGarageData
    live_field: player.electronics.brake_bias_rear   # optionnel : champ du protocole exposé en roulant
    derived_from: [aero.rh_front, aero.rh_rear]      # optionnel : pseudo-paramètre
    unit: "cran ou °"
    classes: [hypercar, lmp2, lmp3, gt3]   # valeurs de corde_protocol::VehicleClass (snake_case)
    bop_locked: false              # false | true | "partial: <explication>"
    live_adjustable: false         # modifiable au volant / MFD en roulant
    typical_range:                 # optionnel, INDICATIF, par classe
      gt3: {value: "…", confidence: low}
    default_step: "±1 cran"        # « cran d'ingénieur » (brief 02 §C.0)
    notes: "…"
    confidence: high               # confiance globale de l'entrée
    sources: [<urls>]
bop_levers:            # liste {lever, label_fr, classes, unit, example, confidence}
driver_aids:           # liste {aid, classes, levels, weight_penalty_kg, confidence, source}
thermal_windows:       # {tyres: [...], brakes: [...], notes: [...], sources: [...]} — ordres de grandeur, confiance low/medium
to_verify_in_game:     # liste de chaînes : backlog de validation sur le PC du client
```

Quatre paramètres (`cooling.radiators`, `legacy.chassis`, `strategy.pitstops`, `trans.ratio_set`) ne sont
utilisés par aucune règle : ils documentent le `.svm` complet pour le parseur/écrivain et l'UI.

### 3.2 `diagnostics.yaml`

```yaml
schema_version: 1
source: docs/research/02-ingenieur-de-course.md
preprocessing:         # segmentation_laps, segmentation_corners, phases, filtering, frame_signs
  <étape>: {description, inputs: [...], rule|entry|mid|exit|…, confidence: {method}, calibration?, note?}
diagnostics:           # liste
  - id: D-TYR-01
    name_fr: "…"
    category: tyres                # tyres | brakes | balance | platform | dampers | traction | strategy |
                                   # consistency | driving_style | straightline | response | conditions
    corde_phase: 2                 # phase de la feuille de route (docs/ARCHITECTURE.md §6) où il est prévu
    inputs:                        # chemins RÉELS du protocole depuis TelemetryFrame
      - player.wheels[i].temp_c[0..2]     # i = 0 FL, 1 FR, 2 RL, 3 RR
      - player.motion.g_lat
      - LapRecord.lap_time_s              # ou un champ de LapRecord / standings[].xxx
    inputs_external: [...]         # optionnel : REST, .svm, DuckDB
    derived_missing: [...]         # optionnel : grandeur absente du protocole v1, à dériver ou à proposer
    method: "…"
    thresholds: {<nom>: "<valeur indicative>"}
    outputs: [<noms des indicateurs produits>]
    symptoms: [<symptômes de setup-rules que ce diagnostic peut déclencher>]
    confidence: {method: high|medium|low, thresholds: high|medium|low}
    calibration: "…"               # optionnel
    note: "…"                      # optionnel
    sources: [<urls ou § du brief>]
```

Convention d'orientation des températures : `temp_c[3]` = **gauche / centre / droite** de la bande (convention
rFactor, cf. `docs/ARCHITECTURE.md` §4), pas intérieur/extérieur. Intérieur = index 2 pour les roues gauches
(FL, RL), index 0 pour les roues droites (FR, RR). À confirmer sur capture réelle (brief 02 §F.2).

### 3.3 `setup-rules.yaml`

```yaml
schema_version: 1
source: docs/research/02-ingenieur-de-course.md
generated_from_sections: [C.0 … C.9]
enums:                 # valeurs autorisées : phase, corner_speed, surface, philosophy, direction, confidence
params:                # liste {id, unit, sign} — sens du « + » pour chaque paramètre (même id que setup-parameters)
default_steps:         # {"<motif de param>": "<pas par défaut>"} quand magnitude dit « 1 pas »
arbitration:           # liste {id: ARB-0n, rule} — règles d'arbitrage entre changements contradictoires
cross_effects:         # liste {param, direction, main_effect, watch} — matrice d'effets croisés C.9
known_gaps:            # liste {kind, id|symptom, note} — incohérences connues, volontairement conservées
rules:                 # liste
  - id: US-ENTRY-SLOW-01
    symptom: US_ENTRY_SLOW
    family: understeer
    phase: entry                   # entry | mid | exit | braking | straight | any
    corner_speed: slow             # slow | medium | fast | any
    surface: dry                   # dry | wet | any
    classes: [hypercar, lmp2, gt3]
    philosophy: both               # both | aggressive | safe
    trigger:
      telemetry: "…"               # expression informelle sur la matrice de diagnostics, à implémenter
      diagnostics: [D-BAL-01]      # diagnostics qui alimentent le déclencheur
      feel: [FEEL_FRONT_WONT_TURN_IN]
    changes:                       # ordonnés par prio croissante (1 = essayer d'abord)
      - prio: 1
        param: brake.bias_front
        direction: decrease        # increase | decrease | either | toward_zero | set | check | meta
        value: wet                 # seulement pour direction: set / meta
        magnitude: "0.5–1 %"       # pas RELATIF, jamais une valeur absolue
        confidence: high
        side_effects: "…"
        conflicts: [BRK-LOCK-REAR-01]   # règles dont les changements s'opposent
    justification: "…"
    sources: [<urls>]
    brief_section: C.1
```

Directions particulières : `check` ne modifie rien (vérifier la condition de `magnitude` avant d'agir) ;
`meta` applique un autre groupe de règles via le pseudo-paramètre `meta.balance`
(`value: reduce_understeer | reduce_oversteer | neutralise_balance`). `meta.balance` n'existe volontairement
pas dans `setup-parameters.yaml` : ce n'est pas un réglage de la voiture.

### 3.4 `feedback-dictionary.yaml`

```yaml
schema_version: 1
source: docs/research/02-ingenieur-de-course.md
confidence: medium     # les expressions FR sont de l'usage courant, non sourçable
normalisation: {lowercase, strip_accents, light_lemmatisation}
debrief_protocol: [...]            # ordre des questions d'un débrief de 60 s
ambiguities: [{expression, resolve}]
entries:
  - id: FEEL_FRONT_WONT_TURN_IN
    fr: ["l'avant ne tourne pas à l'inscription", …]
    en: ["won't turn in", …]
    symptoms: [US_ENTRY_SLOW, US_ENTRY_FAST]   # ou motif glob : US_*
    default_phase: entry           # phase supposée si le pilote ne précise pas
    default_corner_speed: null     # null = à demander
    default_note: "…"              # optionnel
    clarify: ["…", "…"]            # questions de clarification, dans l'ordre
    note: "…"                      # optionnel (ex. « pas de setup : FFB »)
aliases: [{id: FEEL_FLOATY, alias_of: FEEL_WALLOWS}]   # identifiants référencés par les règles
```

### 3.5 `methodology.md`

Document narratif (sections 1–8) : principes, ordre d'itération E.1, paquets vs variable unique E.2,
pondération ressenti/données E.3, adaptation par circuit E.4, génération des 3 setups E.5, boucle
« proposition → test → ressenti → ajustement », backlog de validation. C'est la spécification de
l'orchestrateur ; les YAML en sont les tables.

---

## 4. Cohérence avec le protocole (`crates/corde-protocol`)

Vérification faite le 2026-11-03 par script (résolution de chaque chemin `player.*`, `session.*`,
`LapRecord.*`, `standings[].*` contre les structs de `lib.rs`) :

- **93 chemins distincts référencés, 93 résolus.** Aucun nom de champ à corriger.
- `live_field` de `setup-parameters.yaml` (26 entrées) : tous présents (`player.electronics.tc`,
  `player.energy.regen_kw`, `player.wheels[0..1].pressure_kpa`, `player.tyres.front_compound`…).
- `classes` utilisent bien les variantes snake_case de `VehicleClass` (`hypercar`, `lmp2`, `lmp3`, `gt3`).
- Unités cohérentes avec `docs/ARCHITECTURE.md` §4 (°C, kPa, degrés, km/h ; `temp_c` gauche/centre/droite).

Écarts et manques connus (à traiter côté code ou protocole, pas dans les YAML) :

| Où | Constat | Suite proposée |
|---|---|---|
| `diagnostics.yaml` D-BAL-02 (`derived_missing`) | L'accélération de lacet n'est pas dans `Motion` (le jeu expose `mLocalRotAccel`) | Dériver `yaw_rate_dps` à 100 Hz en attendant ; amendement protocole proposé : `Motion.local_rot_accel: [f32; 3]` |
| `setup-rules.yaml` `known_gaps` | `PLT-ROLL-01` référencé comme conflit mais inexistant ; `ENERGY_SHORT` (ressenti `FEEL_FUEL_HIGH`) sans règle | Le premier pointe vers D-PLT-04 ; le second relève de la stratégie (D-CON-02), pas du setup — le moteur doit tolérer un conflit vers une règle absente |
| `setup-parameters.yaml` `energy.virtual_energy` ↔ `Energy.virtual_energy` | Unité incertaine (fraction 0..1 ou %) des deux côtés | À lire sur le PC du client (`lmu-probe --once --dump frame.json`) |
| `Wheel.optimal_temp_c` est `Option` | D-TYR-03 en dépend | Repli REST (`inputs_external`) déjà prévu dans le diagnostic |
| `Wheel.brake_pressure` | 0..1 « pour l'instant », pas en kPa | D-BRK-* n'utilisent que des rapports : pas d'impact |
| `setup-rules.yaml` `trigger.telemetry` | Texte informel (`balance[entry][slow] > +0.3`) | Formaliser en expression évaluable lors de l'implémentation (phase 2) ; la matrice `balance[phase][vitesse]` est produite par D-BAL-01 |

---

## 5. Utiliser et compléter

**Lire depuis le code.** Charger les YAML au démarrage (`serde_yaml` ou équivalent), valider les clés
étrangères de la section 2 dans un test (`cargo test`) : un identifiant orphelin doit faire échouer la CI.
Ne jamais dériver une valeur numérique de setup d'un `typical_range` : seules `direction`, `magnitude` et
`default_step` sont destinées au moteur, et toujours bornées par `minValue`/`maxValue` lus dans le jeu.

**Afficher à l'utilisateur.** Chaque changement proposé doit montrer : la règle (`id`), ce qui l'a déclenchée
(`trigger.diagnostics` ou `trigger.feel`), sa `confidence`, ses `side_effects` et le `label_fr` du paramètre.
Une confiance `low` s'affiche comme « à vérifier ».

**Ajouter une règle.** Choisir un `symptom` existant (ou l'ajouter au dictionnaire de ressenti **et** à un
diagnostic), respecter les `enums`, n'utiliser que des `param` de `setup-parameters.yaml`, citer au moins une
source URL, attribuer une confiance honnête, puis relancer la vérification de cohérence.

**Ajouter un paramètre.** Vérifier la clé `.svm` sur un fichier réel (confiance `high`) ou la marquer
`medium` ; renseigner `rest_key` si l'objet `VM_*` est connu ; ajouter l'entrée miroir `{id, unit, sign}` dans
`setup-rules.params`.

**Calibrer un seuil.** Ne pas remplacer la valeur du brief : ajouter la valeur mesurée dans `calibration`
avec la classe, le circuit et la date, et monter la confiance seulement avec plusieurs mesures concordantes.

**Vérifier la cohérence** (à intégrer en CI, phase 2) :

```bash
python3 -c "import yaml,glob; [yaml.safe_load(open(f)) for f in glob.glob('knowledge/*.yaml')]"
```

puis un test Rust qui recharge les fichiers et contrôle les clés étrangères de la section 2 et les chemins
`inputs` / `live_field` contre les types de `corde-protocol`.

---

## 6. État d'implémentation (`crates/corde-engineer`, novembre 2026)

Le crate embarque `setup-rules.yaml` et `setup-parameters.yaml` à la compilation (`Knowledge::embedded()`)
et peut les recharger depuis un dossier (`Knowledge::from_dir`). Le test
`rules::tests::embedded_knowledge_parses_and_is_consistent` vérifie que les deux YAML se chargent, que les
46 règles sont là et que chaque `param` d'une règle existe dans `setup-parameters.yaml` (sauf `meta.balance`).
Les formules, seuils et confiances effectivement codés sont dans `docs/ENGINEER.md`.

| Diagnostic | État dans `corde-engineer` |
|---|---|
| D-BAL-01 | implémenté (estimateurs a + b fusionnés ; c `grip_fract` non utilisé) → `US_* / OS_*`, **par rapport à la référence de la session** depuis S2 (l'indice absolu penche vers le sous-virage sur toutes les voitures réelles ; `docs/ENGINEER.md` § 3.1) ; b inactif sur LMU (angle volant à 0) |
| D-BRK-01, D-BRK-03, D-BRK-04, D-BRK-05 | implémentés (D-BRK-05 : corrections de volant en freinage droit uniquement) ; D-BRK-01 sur le pic typique par tour avec les bornes de `thermal_windows.brakes` (GT3 350–750 °C vérifié sur 5 sessions réelles, carbone 400–1000 °C non vérifié) |
| D-TRC-01, D-TRC-02, D-TRC-03 | implémentés (D-TRC-02 par asymétrie des événements de patinage, sans `load_n`) |
| D-TYR-01, D-TYR-02, D-TYR-03, D-TYR-04 (info), D-TYR-05, D-TYR-06 | implémentés (D-TYR-03 sur le canal `optimal_temp_c_*` enregistré depuis S2, seuils ±8 °C, sans objet sur les sessions plus anciennes) ; D-TYR-08 non implémenté |
| D-PLT-01, D-PLT-03 | implémentés (D-PLT-01 sans le critère `g_vert`, décrochage de fond plat non détecté) |
| D-DMP-01 | implémenté (histogramme, skew ; « flotte / nerveuse » informatifs) |
| D-CON-01, D-CON-02 | implémentés (D-CON-01 sans les secteurs) |
| S-01 … S-05, S-09 | implémentés (S-03 sur le volant seulement, S-05 par lissage des gaz) |
| D-WET-01 | implémenté → `WET_BASELINE`, `WET_NO_TEMP` |
| D-TYR-07, D-BRK-02, D-BRK-06, D-BAL-02…05, D-PLT-02, D-PLT-04, D-PLT-05, D-TRC-04, D-TRC-05, D-CON-03, D-CON-04, S-06…S-08, D-STR-01, D-STR-02 | non implémentés (lot suivant ou données réelles nécessaires) |

Réglage réel (`setup.rs`, `docs/ENGINEER.md` § 8) : `svm.section` / `svm.key` / `index_sign` / `index_step` /
`label_is_index` / `svm_translation` / `rest_key` disent où et dans quel sens un changement s'applique dans un
fichier `.svm` ; le test `setup::tests` et `tests/setup_real.rs` les exercent sur les fichiers réels du pilote.
Quand le garage du jeu a été lu, `rest_key` relie un objet `VM_*` à son paramètre pour les bornes.

Moteur de règles : symptômes des constats de gravité ≥ mineure → règles filtrées par surface / philosophie /
classe (et existence du paramètre pour la classe), score = confiance × gravité × confiance du changement ×
priorité, agrégation par paramètre (ARB-01), biais de famille (ARB-02 / ARB-05), ≤ 8 changements, conflits
déclarés signalés, ordre E.1. `direction: check | either` → avertissements ; `meta` ignoré.

**Ce que le LLM a le droit de faire** (brief 03 §5) : traduire un texte libre en `FEEL_*` + phase + vitesse de
virage via `feedback-dictionary.yaml`, poser les questions `clarify`, rédiger l'explication. Il ne produit
jamais un paramètre, une direction ou une magnitude : cela vient uniquement de `setup-rules.yaml`.
