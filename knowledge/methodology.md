# Méthodologie de l'ingénieur Corde

> Comment Corde passe des données et du ressenti à une proposition de réglage, puis l'affine.
> Source : `docs/research/02-ingenieur-de-course.md` §E (E.1–E.5), §C.9 (arbitrage) et §B.1 (prétraitement).
> Niveaux de confiance repris du brief : **[H]** établi, **[M]** pratique d'ingénierie probable, **[L]** indicatif.
> Les fichiers machine qui accompagnent ce document : `setup-rules.yaml`, `diagnostics.yaml`,
> `setup-parameters.yaml`, `feedback-dictionary.yaml` (schémas dans `README.md`).

## 1. Principes

1. **Jamais de valeur absolue codée en dur.** Les règles produisent des directions et des pas relatifs
   (« −1 cran de barre avant », « +3 kPa ») ; la plage réelle et le cran courant se lisent dans le setup
   chargé (REST `getPlayerGarageData`, fichier `.svm`, `metadata.CarSetup` DuckDB). [H]
2. **Sans pneus dans la fenêtre, tout diagnostic d'équilibre est faux.** Les conditions (composé,
   pressions, conduits) se règlent avant tout le reste. [H]
3. **Une recommandation est explicable.** Chaque changement proposé porte la règle qui l'a produit, le
   diagnostic ou le ressenti qui l'a déclenchée, sa confiance et ses effets secondaires attendus. Le
   modèle de langage, s'il est utilisé, ne fait que traduire le ressenti en symptômes ; il ne produit
   jamais une valeur de réglage.
4. **Un seul tour ne prouve rien.** On ne corrige par le setup qu'un symptôme présent sur plusieurs tours
   propres (≥ 3) et plusieurs passages du même virage. [M]

## 2. Ordre d'itération (« du moins cher au plus structurant ») — E.1

| Étape | Ce qu'on règle | Diagnostics à regarder | Conf. |
|---|---|---|---|
| 1 | **Conditions et pneus** : composé, pressions à froid pour atteindre la pression/température cible à chaud après ~3 tours (lire `optimal_temp_c`), conduits de frein pour la fenêtre freins | D-TYR-03, D-TYR-04, D-TYR-08, D-BRK-01 | H |
| 2 | **Aéro et hauteurs** : niveau d'appui pour le circuit (aileron arrière), puis rake/hauteurs pour l'équilibre haute vitesse et l'absence de talonnage avec la charge de carburant visée | D-PLT-01 (à plein), D-PLT-02, D-BAL-01 (rapide) | H |
| 3 | **Freinage** : répartition, migration (Hypercar), map ABS (LMGT3), pression de pédale — une seule de ces variables à la fois (consigne officielle ABS) | D-BRK-03, D-BRK-04, D-BRK-05, D-BRK-06 | H |
| 4 | **Barres anti-roulis** : équilibre milieu de virage lent/moyen | D-BAL-01 (mid, slow/medium), D-PLT-04 | H |
| 5 | **Ressorts, 3e élément, packers** : plateforme, talonnage, bosses, vibreurs | D-PLT-01, D-PLT-03, S-04 | M |
| 6 | **Amortisseurs** : transitoires (entrée/sortie) et vibreurs, validés par histogramme | D-DMP-01, D-BAL-02, D-BAL-04 | M |
| 7 | **Différentiel** : motricité et rotation sous gaz / au lâcher | D-TRC-01, D-TRC-02, D-BAL-03 | M |
| 8 | **Géométrie fine** (carrossage, pince, chasse) d'après les températures intérieur/milieu/extérieur, en dernier et par petits pas | D-TYR-01, D-TYR-02 | M |
| 9 | **Électronique de course** (TC, motor map, régén, map moteur, fuel ratio) : en continu, y compris en roulant au MFD — c'est le levier d'adaptation *intra-relais* | D-TRC-03, D-CON-02, D-CON-03 | H |

Pour un pilote débutant, le gain de temps au tour vient dans l'ordre : **aileron, hauteurs, packers,
répartition de frein, TC** ; précharge, carrossage et pince comptent peu (Hoekstra, GT3). [M]

Sur Hypercar et LMP2, l'aéro/hauteurs est le levier n°1 ; en GT3 l'aileron reste le n°1. [H]

## 3. Une variable à la fois ou paquets ? — E.2

- **Exploration** (nouveau circuit, nouveau pilote, première session) : paquets cohérents de 3 à 5
  changements allant dans le même sens (« paquet pluie », « paquet Monza bas appui », « paquet sûr ») ;
  on valide la **direction** sur 5–8 tours, pas la magnitude. [M]
- **Convergence** : **une variable** (ou deux non interagissantes, ex. pression avant + conduit arrière)
  par run de 5–8 tours propres ; comparer la médiane des 3 meilleurs tours, l'écart-type et les
  diagnostics ciblés ; revenir en arrière si le gain est inférieur au bruit. Bruit typique :
  0,1–0,2 s en GT3, 0,2–0,3 s en Hypercar sur piste évolutive. [M]
- **Taille des pas** : commencer par 2 « crans d'ingénieur » (`default_steps` dans `setup-rules.yaml`)
  pour voir un effet, puis affiner à 1. Si aucun effet mesurable après 2 crans, le paramètre n'est pas le
  bon levier pour ce symptôme (logique HP Academy : « si le changement n'est pas visible au-delà du
  bruit, il était trop petit »). [M]
- **Verrouiller l'électronique** (TC, map, migration) pendant les comparaisons de setup mécanique, sinon
  le pilote compense et masque l'effet. [H]
- **Toujours noter** : setup, carburant, composé, températures piste/air, heure (nuit), niveau de gomme
  (`track_grip_level`), pour ne comparer que des runs comparables. Les setups du commerce sont créés à
  20 °C air / Real Road ; des conditions très différentes exigent au minimum des retouches de pression. [H]

### Limites par itération (arbitrage C.9)

1. Jamais deux changements de signe opposé sur le même paramètre dans une même itération.
2. Si deux symptômes se contredisent sur un paramètre (ex. `US_ENTRY_SLOW` veut `bias −`,
   `BRK_LOCK_REAR` veut `bias +`) : en philosophie **sûre/soft**, le symptôme de sécurité gagne ; en
   philosophie **agressive**, le symptôme de performance gagne ; on cherche un autre levier pour l'autre.
3. **3 paramètres maximum** par itération (1 seul en phase de validation).
4. Toute modification de hauteur ou d'aileron sur Hypercar relance la vérification de talonnage (D-PLT-01).
5. Priorité des symptômes : **sécurité** (blocages, instabilité au freinage, snaps rapides) >
   **pilotabilité** (prévisibilité, fatigue) > **performance** (rotation, Vmax) > **usure / consommation**
   — en mode sûr/endurance, usure/consommation remonte en 2e position.

## 4. Pondérer ressenti et données — E.3

Score de confiance d'un diagnostic = **cohérence × couverture × qualité du pilote** : [M]

- **Cohérence** : les 3 estimateurs d'équilibre de D-BAL-01 concordent → poids 1,0 ; deux sur trois →
  0,6 ; contradictoires → 0,2 (on demande au pilote).
- **Couverture** : ≥ 3 tours propres et ≥ 3 passages du virage ; sinon poids divisé par 2.
- **Qualité pilote** (S-07 marge au grip, D-CON-01 régularité) :
  - pilote régulier et près de la limite (S-07 ≥ 0,6, σ faible) → **données 0,7 / ressenti 0,3** ;
  - pilote loin de la limite ou irrégulier → **ressenti 0,6 / données 0,4** pour la *pilotabilité*,
    mais les données gardent la priorité pour les **faits physiques** qui ne se négocient pas :
    talonnage, températures, blocages, état de charge batterie.

Quand le ressenti contredit les données :

1. vérifier que le pilote et la télémétrie parlent du **même virage et de la même phase** (questions de
   clarification de `feedback-dictionary.yaml`) ;
2. chercher une **cause de pilotage** (S-08 : entrée trop rapide → « sous-virage » ressenti alors que la
   voiture est neutre ; S-02 : trail braking) ;
3. si le ressenti persiste, privilégier un changement **réversible et peu coûteux** (répartition, TC,
   barre) et re-tester ;
4. ne jamais corriger par le setup un symptôme vu sur **un seul tour**.

### Protocole de débrief (≈ 60 s)

Phase du virage → vitesse du virage → début/fin de relais → pneus (température/pression au HUD) →
batterie et TC (Hypercar) → « est-ce que c'est pire qu'avant la dernière modification ? ».

Le ressenti est saisi soit en texte libre (interprété en identifiants `FEEL_*` par le dictionnaire,
avec ou sans modèle de langage), soit par un **formulaire structuré** (sous-/survirage × entrée/milieu/
sortie × lent/rapide) qui alimente directement les règles — c'est aussi le chemin le moins ambigu et le
mode hors-ligne.

## 5. Adapter un setup d'un circuit à un autre — E.4

On part du setup « mixte » validé du pilote (référence : Spa, Portimão, Imola), on applique le **paquet
de typologie** du nouveau circuit, puis on reconverge (§3). [M]

| Typologie | Exemples LMU | Transformations (paquet) |
|---|---|---|
| **Rapide / aéro, faible traînée** | Le Mans, Monza, Paul Ricard | aileron AR −2 à −4 crans (GT3 : jusqu'à l'angle mini BoP) ; flap AV − pour garder l'équilibre ; rake légèrement + ; hauteurs −1 à −2 mm si pas de talonnage ; répartition +0,5–1 % ; migration 2,0–2,5F ; barres +1 (chicanes) ; fast bump − ; ressorts AR + ; rapports « Le Mans » ; motor map LMH haut |
| **Mixte** | Spa, Portimão, Imola, Fuji, Silverstone, Interlagos | setup de référence ; packers / 3e élément et hauteurs validés à plein sur les compressions (D-PLT-01) ; Interlagos : fast bump − ; anti-horaire → surveiller l'avant droit |
| **Lent / mécanique** | Bahreïn (S2), Qatar, COTA (S1), Sebring hors bosses | aileron +1–2 ; barres −1 ; ressorts −1 ; diff power +1 et précharge + ; TC +1 si surchauffe AR ; répartition −0,5 % ; coast −1 ; pressions AR au mini ; carrossage AV + ; conduits + si freins chauds |
| **Bosselé** | Sebring, Interlagos, COTA | ressorts −1 à −2 ; 3e élément −1 ; fast bump −2 à −4 ; fast rebound −2 ; slow rebound −1 ; hauteurs +2–4 mm ; packers − ; barres −1 ; pressions −2 kPa ; répartition +0,5 % ; TC +1 ; accepter une plateforme aéro moins stable |
| **Chaud / froid** | Bahreïn, Qatar (chaud, nuit froide) ; Spa, Le Mans de nuit | chaud : pressions −2–3 kPa, composé plus dur, conduits +1, TC +1 ; froid/nuit : pressions +3–5 kPa, conduits −1, composé plus tendre, répartition +1 % pendant la chauffe, TC/ABS − pendant la chauffe |

Processus de transfert : (1) setup mixte validé → (2) paquet de typologie → (3) 3 tours de contrôle :
talonnage (D-PLT-01), pressions/températures (D-TYR-03/04), freins (D-BRK-01) → (4) 5–8 tours
d'équilibre : matrice D-BAL-01 → règles → (5) relais long simulé en mode sûr pour la dégradation
(D-TYR-06) et l'énergie (D-CON-02/03).

Corde conserve **un setup par couple (circuit, typologie météo)** et un **journal des deltas appliqués**
pour apprendre les « coefficients » du pilote (ex. « ce pilote veut toujours −0,5 % de répartition par
rapport à la base ») : c'est ce journal qui fait évoluer les propositions avec les progrès du pilote.

## 6. Génération des 3 setups initiaux — E.5

Pour un couple (voiture, circuit) :

1. **Base** = setup par défaut du jeu, ou le setup Coach Dave intégré au jeu quand il existe (depuis
   v1.2 pour la plupart des Hypercars et plusieurs GT3 ; « plus performants que les défauts »). [H]
   Si le pilote a déjà un setup validé sur ce circuit, c'est lui la base.
2. Appliquer le **paquet typologie circuit** (§5) si la base n'est pas spécifique au circuit.
3. Dériver les trois philosophies :
   - **Agressif** = base + `PHI-AGGRESSIVE-01` ;
   - **Sûr / soft** = base + `PHI-SAFE-01` ;
   - **Pluie** = base + `WET-PACK-01` (+ `PHI-SAFE-01` allégé : TC, répartition).
4. Personnaliser par le **profil de style** (S-01…S-08) quand des données existent déjà :
   freinage agressif → migration −0,5 et répartition +0,5 ; trail braking fort → coast +1 ;
   gaz brutaux → TC +1, power +1 ; gros usage des vibreurs → fast bump −2, packers −2 ;
   volant nerveux → `PLT-NERVOUS-01`.
5. Valider les **invariants** avant d'écrire le fichier : angle d'aileron ≥ mini BoP ; pressions ≥ mini ;
   hauteurs ≥ mini ; énergie virtuelle 100 % ; fuel ratio cohérent avec la stratégie ; batterie de départ
   ≈ 75 % avec déploiement élevé puis réduit. Les bornes viennent du setup courant (REST / `.svm`), pas
   de `setup-parameters.yaml`.

Le fichier écrit est un `.svm` **complet** (copie du setup courant + deltas), déposé dans
`UserData\player\Settings\<Circuit>\`, jamais une édition partielle, jamais pendant que le jeu écrit.
En V1 Corde n'écrit rien dans le jeu via l'API REST.

## 7. Boucle « proposition → test → ressenti → ajustement »

```
   setup courant + télémétrie des tours précédents
                 │
                 ▼
   prétraitement (tours propres, virages, phases)  ──►  diagnostics (D-*, S-*)
                 │                                             │
                 ▼                                             ▼
   ressenti pilote (texte libre / formulaire) ──► FEEL_* ──► symptômes
                 │                                             │
                 └──────────────► fusion pondérée (§4) ◄───────┘
                                        │
                                        ▼
                          règles (setup-rules.yaml) + arbitrage (§3)
                                        │
                                        ▼
                 ≤ 3 changements expliqués, avec confiance et effets secondaires
                                        │
                                        ▼
                 fichier .svm proposé  →  le pilote roule  →  nouvelle session
                                        │
                                        ▼
                 comparaison avant/après (médiane 3 meilleurs tours, σ, diagnostics ciblés)
                 → gain > bruit : on garde, on note le delta dans le journal du pilote
                 → sinon : retour arrière, autre levier
```

À chaque tour de boucle, Corde demande : « est-ce mieux, pareil ou pire qu'avant ? ». La réponse,
croisée avec les chronos, alimente le journal des deltas qui personnalise les propositions suivantes.

## 8. Ce qui reste à vérifier en jeu (backlog)

Voir `to_verify_in_game` dans `setup-parameters.yaml` et la section F du brief : noms exacts des clés
`.svm`, orientation des 3 zones de température par côté, signes du repère véhicule dans l'API native,
seuil lent/rapide des amortisseurs, fenêtres de température des freins après v1.4, présence du 3e élément
et du flap avant par voiture, allocation des composés, fidélité de la loi des gaz pour les pressions.
