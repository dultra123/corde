//! Plan iteration: feedback + latest data → a small, explained adjustment of
//! the plan the driver tested ([`PlanIteration`]).
//!
//! 1. The detected symptoms become *virtual findings* (ids `FEEL/<symptom>`,
//!    same symptom ids as the diagnostics, confidence = the extractor's ×
//!    intensity). Generic families (`US_ENTRY`, `BRK_LOCK`…) are expanded
//!    with the data when it pinpoints the case, else into their most likely
//!    members at reduced confidence.
//! 2. They are merged with the data findings of the latest report: the same
//!    symptom on both sides → confidence boosted (`agreements_fr`); the
//!    opposite balance on the same phase → `disagreements_fr`, the data wins
//!    when its confidence is above 0.6, else the feeling (methodology §4) —
//!    except the balance findings (`D-BAL-01`): relative to the session
//!    reference, they never overrule the driver's absolute feeling.
//!    A symptom the driver says is gone cancels a weak data finding (and any
//!    relative balance finding).
//! 3. The rule engine runs on the merged findings, then the *iteration* filter
//!    keeps at most [`MAX_NEW_CHANGES`] new changes from **one** parameter
//!    family (tyres → aero → brakes → bars → springs → dampers → diff →
//!    geometry → electronics), each at one engineer step, never reversing a
//!    parameter touched before unless the feeling asks for it (and says so).
//! 4. The cumulative plan and the delta are written, and the iteration is
//!    numbered after the previous one of the same session and plan.

use crate::feedback::{parse_balance_id, symptom_label};
use crate::rules::{self, group_order, Knowledge};
use corde_protocol::{
    AnalysisReport, ChangeDirection, CornerPhase, CornerSpeed, DetectedSymptom, Evidence,
    FeedbackInterpretation, Finding, PlanIteration, SetupChange, SetupPhilosophy, Severity,
    SymptomIntensity, VehicleClass,
};
use std::collections::HashMap;

/// New changes per iteration (arbitration ARB-03: 3 parameters, one family).
pub const MAX_NEW_CHANGES: usize = 3;
/// Above this confidence the data wins a contradiction with the feeling.
pub const DATA_WINS_ABOVE: f32 = 0.6;
/// The balance diagnostic. Its findings are **relative** to the session
/// reference ("more understeer here than usual for this car"), while the
/// driver states an absolute feeling: they never overrule it.
pub const RELATIVE_BALANCE_DIAGNOSTIC: &str = "D-BAL-01";

/// Can this data finding win against what the driver says?
fn can_overrule_feeling(d: &Finding, few_laps: bool) -> bool {
    d.confidence > DATA_WINS_ABOVE && !few_laps && d.diagnostic_id != RELATIVE_BALANCE_DIAGNOSTIC
}

/// Why a relative balance finding gives way to the feeling (French).
const RELATIVE_NOTE_FR: &str = "les données d'équilibre sont relatives à ta référence de la session, ton ressenti passe devant";

/// What the iteration starts from.
#[derive(Debug, Clone)]
pub struct IterationContext<'a> {
    pub session_id: &'a str,
    pub philosophy: SetupPhilosophy,
    pub vehicle_class: VehicleClass,
    pub wet_session: bool,
    /// The plan the driver tested when there is no previous iteration (the
    /// report's plan of this philosophy, or empty).
    pub base: &'a [SetupChange],
    /// The last iteration of this session and philosophy, if any.
    pub previous: Option<&'a PlanIteration>,
    /// The analysis of the session the feedback is about, if it exists.
    pub latest_report: Option<&'a AnalysisReport>,
    pub feedback_id: Option<i64>,
    /// Parameters the driver's car does not let change (read in the base
    /// `.svm`): the engine never proposes them (empty = no base known).
    pub unavailable_params: &'a [String],
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn phase_tag(p: CornerPhase) -> &'static str {
    match p {
        CornerPhase::Entry => "ENTRY",
        CornerPhase::Mid => "MID",
        CornerPhase::Exit => "EXIT",
    }
}

fn speed_tag(s: CornerSpeed) -> &'static str {
    match s {
        CornerSpeed::Fast => "FAST",
        _ => "SLOW",
    }
}

/// Concrete rule symptoms behind a (possibly generic) detected symptom, with
/// a confidence factor each. `data` lets the report pinpoint the case.
pub fn expand_symptom(id: &str, data: &[Finding]) -> Vec<(String, f32)> {
    let has = |sym: &str| {
        data.iter()
            .any(|f| f.severity >= Severity::Minor && f.symptoms.iter().any(|s| s == sym))
    };
    if let Some((fam, phase, speed)) = parse_balance_id(id) {
        let phases: Vec<(CornerPhase, f32)> = match phase {
            Some(p) => vec![(p, 1.0)],
            None => vec![
                (CornerPhase::Mid, 0.8),
                (CornerPhase::Entry, 0.6),
                (CornerPhase::Exit, 0.6),
            ],
        };
        let speeds: Vec<(CornerSpeed, f32)> = match speed {
            Some(s) => vec![(s, 1.0)],
            None => vec![(CornerSpeed::Slow, 0.85), (CornerSpeed::Fast, 0.6)],
        };
        let all: Vec<(String, f32)> = phases
            .iter()
            .flat_map(|(p, pf)| {
                speeds.iter().map(move |(s, sf)| {
                    (
                        format!("{fam}_{}_{}", phase_tag(*p), speed_tag(*s)),
                        pf * sf,
                    )
                })
            })
            .collect();
        // The data already saw some of these cases: those are the ones.
        let seen: Vec<(String, f32)> = all
            .iter()
            .filter(|(s, _)| has(s))
            .map(|(s, _)| (s.clone(), 1.0))
            .collect();
        return if seen.is_empty() { all } else { seen };
    }
    let generic: &[(&str, f32)] = match id {
        "BRK_LOCK" => &[("BRK_LOCK_FRONT", 0.75), ("BRK_LOCK_REAR", 0.55)],
        "TRC_WHEELSPIN" => &[("TRC_WHEELSPIN_BOTH", 0.75), ("TRC_WHEELSPIN_INNER", 0.6)],
        "TYR_HOT" => &[("TYR_HOT_FRONT", 0.65), ("TYR_HOT_REAR", 0.65)],
        "NO_GRIP" => &[("TYR_COLD", 0.5)],
        "JUMPS" => &[("PLT_KERB_UNSETTLED", 0.6), ("PLT_BUMPY_LOSS", 0.5)],
        "UNSTABLE" => &[("PLT_NERVOUS", 0.6)],
        _ => return vec![(id.to_string(), 1.0)],
    };
    let seen: Vec<(String, f32)> = generic
        .iter()
        .filter(|(s, _)| has(s))
        .map(|(s, _)| (s.to_string(), 1.0))
        .collect();
    if seen.is_empty() {
        generic.iter().map(|(s, f)| (s.to_string(), *f)).collect()
    } else {
        seen
    }
}

fn intensity_factor(i: Option<SymptomIntensity>) -> f32 {
    match i {
        Some(SymptomIntensity::Mild) => 0.8,
        None => 0.9,
        Some(SymptomIntensity::Marked) | Some(SymptomIntensity::Severe) => 1.0,
    }
}

fn intensity_fr(i: Option<SymptomIntensity>) -> &'static str {
    match i {
        Some(SymptomIntensity::Mild) => ", un peu",
        Some(SymptomIntensity::Marked) => ", beaucoup",
        Some(SymptomIntensity::Severe) => ", très marqué",
        None => "",
    }
}

/// A detected symptom as findings the rule engine understands.
pub fn virtual_findings(sym: &DetectedSymptom, data: &[Finding]) -> Vec<Finding> {
    expand_symptom(&sym.symptom_id, data)
        .into_iter()
        .map(|(id, factor)| {
            let confidence =
                (sym.confidence * intensity_factor(sym.intensity) * factor).clamp(0.0, 1.0);
            let severity = match sym.intensity {
                Some(SymptomIntensity::Severe) => Severity::Major,
                Some(SymptomIntensity::Marked) if confidence >= 0.6 => Severity::Major,
                _ => Severity::Minor,
            };
            let (label_fr, label_en) = symptom_label(&id);
            let (_, phase, speed) =
                parse_balance_id(&id).unwrap_or((&id, sym.phase, sym.corner_speed));
            Finding {
                id: format!("FEEL/{id}"),
                diagnostic_id: "FEEL".into(),
                symptoms: vec![id.clone()],
                severity,
                confidence,
                phase: phase.or(sym.phase),
                corner_speed: speed.or(sym.corner_speed),
                title_fr: format!("Ressenti : {label_fr}{}", intensity_fr(sym.intensity)),
                title_en: format!("Felt: {label_en}"),
                detail_fr: format!("Tu as écrit : « {} ».", sym.matched_text),
                detail_en: format!("You wrote: \"{}\".", sym.matched_text),
                evidence: vec![Evidence {
                    label: "confiance du ressenti".into(),
                    value: sym.confidence,
                    unit: "".into(),
                }],
                lap_numbers: vec![],
            }
        })
        .collect()
}

/// French name of a parameter family (`group_order`).
pub fn family_fr(order: u8) -> &'static str {
    match order {
        0 => "pneus / pressions",
        1 => "aéro / hauteurs",
        2 => "freinage",
        3 => "barres anti-roulis",
        4 => "ressorts / packers",
        5 => "amortisseurs",
        6 => "différentiel",
        7 => "géométrie",
        8 => "électronique",
        _ => "autre",
    }
}

/// One engineer step of a parameter: the catalogue's `default_step` without
/// its `±` and remarks (`±1 cran (≈ ±5–10 %)` → `1 cran`, `±0,5–1 %` →
/// `0,5 %`), else the rule's magnitude brought down to one unit.
pub fn one_step_magnitude(k: &Knowledge, param: &str, original: &str) -> String {
    if let Some(ds) = k.params.get(param).and_then(|p| p.default_step.as_deref()) {
        let s = ds.trim().trim_start_matches('±').trim();
        let s = s.split(['(', '/']).next().unwrap_or(s).trim();
        let num_len = s
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == ',')
            .count();
        if num_len > 0 {
            let num: String = s.chars().take(num_len).collect();
            let rest: String = s.chars().skip(num_len).collect();
            let rest = rest.trim_start();
            // A range: skip the upper bound.
            let rest = if let Some(r) = rest.strip_prefix('–').or(rest.strip_prefix('-')) {
                r.trim_start_matches(|c: char| c.is_ascii_digit() || c == '.' || c == ',')
            } else {
                rest
            };
            let unit = rest.trim();
            return if unit.is_empty() {
                num
            } else if unit.starts_with('°') || unit.starts_with('%') {
                format!("{num}{}", if unit.starts_with('°') { unit } else { " %" })
                    .trim()
                    .to_string()
            } else {
                format!("{num} {unit}")
            };
        }
    }
    match rules::parse_steps(original) {
        Some(n) if n.abs() > 1 => {
            let unit: String = original
                .trim()
                .trim_start_matches(['+', '−', '-'])
                .trim_start_matches(|c: char| c.is_ascii_digit())
                .trim()
                .to_string();
            let unit = unit
                .replace("crans", "cran")
                .replace("clics", "clic")
                .replace("clicks", "click")
                .replace("steps", "step");
            if unit.is_empty() {
                "1".into()
            } else {
                format!("1 {unit}")
            }
        }
        _ => original.to_string(),
    }
}

fn pct(c: f32) -> String {
    format!("{:.0} %", (c * 100.0).round())
}

fn opposite(a: ChangeDirection, b: ChangeDirection) -> bool {
    matches!(
        (a, b),
        (ChangeDirection::Increase, ChangeDirection::Decrease)
            | (ChangeDirection::Decrease, ChangeDirection::Increase)
    )
}

fn arrow(d: ChangeDirection) -> &'static str {
    match d {
        ChangeDirection::Increase => "▲",
        ChangeDirection::Decrease => "▼",
        ChangeDirection::TowardZero => "→ 0",
        ChangeDirection::Set => "=",
    }
}

/// Run one iteration (see the module documentation).
pub fn iterate_plan(
    ctx: &IterationContext<'_>,
    interpretation: &FeedbackInterpretation,
    k: &Knowledge,
) -> PlanIteration {
    let iteration = ctx.previous.map(|p| p.iteration + 1).unwrap_or(1);
    let previous_changes: &[SetupChange] = ctx
        .previous
        .map(|p| p.changes.as_slice())
        .unwrap_or(ctx.base);
    let mut agreements: Vec<String> = Vec::new();
    let mut disagreements: Vec<String> = Vec::new();
    let mut notes: Vec<String> = Vec::new();

    // --- data findings -----------------------------------------------------
    let data: Vec<Finding> = ctx
        .latest_report
        .map(|r| {
            r.findings
                .iter()
                .filter(|f| f.severity >= Severity::Minor)
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    let few_laps = ctx
        .latest_report
        .map(|r| r.data_quality.valid_laps < 3)
        .unwrap_or(false);
    match ctx.latest_report {
        None => notes.push(
            "Pas d'analyse de données pour cette session : itération fondée sur ton ressenti seul."
                .into(),
        ),
        Some(_) if few_laps => notes.push(
            "Moins de 3 tours propres analysés : les données pèsent peu face au ressenti.".into(),
        ),
        Some(_) => {}
    }

    // --- virtual findings and merge ------------------------------------------
    let mut merged: Vec<Finding> = data.clone();
    let mut feel_symptoms: Vec<String> = Vec::new();
    // Symptoms the driver states plainly enough to undo an earlier change: a precise case (not
    // a generic family guessed at reduced confidence, unless the data pinpoints it) worded at
    // least normally ("un peu" is not enough to go back on a change).
    let mut explicit_feel: Vec<String> = Vec::new();
    for sym in &interpretation.symptoms {
        let concrete = expand_symptom(&sym.symptom_id, &data);
        if !sym.negated && sym.intensity != Some(SymptomIntensity::Mild) {
            for (id, factor) in &concrete {
                if *factor >= 1.0 {
                    push_unique(&mut explicit_feel, id);
                }
            }
        }
        if sym.negated {
            for (id, _) in &concrete {
                if let Some(pos) = merged
                    .iter()
                    .position(|f| f.diagnostic_id != "FEEL" && f.symptoms.iter().any(|s| s == id))
                {
                    let d = merged[pos].clone();
                    let (label_fr, _) = symptom_label(id);
                    if can_overrule_feeling(&d, few_laps) {
                        disagreements.push(format!(
                            "Tu ne ressens plus « {label_fr} » mais les données le voient encore ({}, confiance {}) : on garde le constat, à revérifier sur 5–8 tours propres.",
                            d.title_fr, pct(d.confidence)
                        ));
                    } else if d.diagnostic_id == RELATIVE_BALANCE_DIAGNOSTIC
                        && d.confidence > DATA_WINS_ABOVE
                    {
                        merged.remove(pos);
                        agreements.push(format!(
                            "« {label_fr} » : plus ressenti ; les données le montrent encore ({}, confiance {}) mais {RELATIVE_NOTE_FR} : on ne le traite plus.",
                            d.title_fr,
                            pct(d.confidence)
                        ));
                    } else {
                        merged.remove(pos);
                        agreements.push(format!(
                            "« {label_fr} » : plus ressenti, et les données ne sont pas formelles ({}) : on ne le traite plus.",
                            pct(d.confidence)
                        ));
                    }
                } else if ctx.latest_report.is_some() && parse_balance_id(id).is_none() {
                    let (label_fr, _) = symptom_label(id);
                    agreements.push(format!(
                        "« {label_fr} » : ni ressenti, ni vu dans les données."
                    ));
                }
            }
            continue;
        }
        let virtuals = virtual_findings(sym, &data);
        let (label_fr, _) = symptom_label(&sym.symptom_id);
        let mut any_data = false;
        for v in virtuals {
            let vsym = v.symptoms[0].clone();
            // Same symptom in the data → agreement, confidence boosted.
            if let Some(d) = merged
                .iter_mut()
                .find(|f| f.diagnostic_id != "FEEL" && f.symptoms.contains(&vsym))
            {
                any_data = true;
                let before = d.confidence;
                d.confidence = (d.confidence.max(v.confidence) + 0.15).min(1.0);
                if d.severity < v.severity {
                    d.severity = v.severity;
                }
                agreements.push(format!(
                    "Ressenti « {label_fr} » confirmé par les données ({}, confiance {} → {}).",
                    d.title_fr,
                    pct(before),
                    pct(d.confidence)
                ));
                push_unique(&mut feel_symptoms, &vsym);
                continue;
            }
            // Opposite balance on the same phase → contradiction.
            if let Some((fam, Some(phase), speed)) = parse_balance_id(&vsym) {
                let other_fam = if fam == "US" { "OS" } else { "US" };
                if let Some(pos) = merged.iter().position(|f| {
                    f.diagnostic_id != "FEEL"
                        && f.phase == Some(phase)
                        && speed.is_none_or(|s| f.corner_speed.map(speed_tag) == Some(speed_tag(s)))
                        && f.symptoms
                            .iter()
                            .any(|s| s.starts_with(&format!("{other_fam}_")))
                }) {
                    any_data = true;
                    let d = merged[pos].clone();
                    if can_overrule_feeling(&d, few_laps) {
                        // The classic trap only applies to understeer felt on entry.
                        let hint = if fam == "US" && phase == CornerPhase::Entry {
                            " ; une entrée trop rapide donne souvent une sensation de sous-virage alors que la voiture est neutre"
                        } else {
                            ""
                        };
                        disagreements.push(format!(
                            "Tu ressens « {label_fr} » mais les données montrent « {} » (confiance {} > {}) : on suit les données. Vérifie qu'on parle du même virage et de la même phase{hint}.",
                            d.title_fr,
                            pct(d.confidence),
                            pct(DATA_WINS_ABOVE)
                        ));
                        continue;
                    }
                    if d.diagnostic_id == RELATIVE_BALANCE_DIAGNOSTIC {
                        disagreements.push(format!(
                            "Tu ressens « {label_fr} » alors que les données montrent « {} » (confiance {}) : {RELATIVE_NOTE_FR} (elles disent seulement que cette phase est moins / plus sous-vireuse que le reste de tes virages) ; on suit ton ressenti, avec un changement réversible et petit.",
                            d.title_fr,
                            pct(d.confidence)
                        ));
                    } else {
                        disagreements.push(format!(
                            "Tu ressens « {label_fr} » alors que les données penchent pour « {} » (confiance {} ≤ {}) : on suit ton ressenti, avec un changement réversible et petit.",
                            d.title_fr,
                            pct(d.confidence),
                            pct(DATA_WINS_ABOVE)
                        ));
                    }
                    merged.remove(pos);
                }
            }
            push_unique(&mut feel_symptoms, &vsym);
            let mut v = v;
            if ctx.latest_report.is_some() && !few_laps {
                v.confidence *= 0.9;
            }
            if let Some(existing) = merged.iter_mut().find(|f| f.id == v.id) {
                if v.confidence > existing.confidence {
                    *existing = v;
                }
            } else {
                merged.push(v);
            }
        }
        if !any_data && ctx.latest_report.is_some() && !few_laps {
            notes.push(format!(
                "« {label_fr} » : rien de correspondant dans les données ; on suit ton ressenti avec prudence (confiance réduite)."
            ));
        }
    }
    let data_findings_used: Vec<String> = merged
        .iter()
        .filter(|f| f.diagnostic_id != "FEEL")
        .map(|f| f.id.clone())
        .collect();

    // --- rule engine, iteration filter ---------------------------------------
    let unavailable = |p: &str| ctx.unavailable_params.iter().any(|u| u == p);
    let (plan, scores) = rules::plan_scored_excluding(
        k,
        ctx.philosophy,
        &merged,
        ctx.vehicle_class,
        ctx.wet_session,
        12,
        &unavailable,
    );
    let rule_symptom: HashMap<&str, &str> = k
        .rules
        .rules
        .iter()
        .map(|r| (r.id.as_str(), r.symptom.as_str()))
        .collect();
    struct Cand {
        change: SetupChange,
        score: f64,
        from_feel: bool,
        reversal: Option<SetupChange>,
    }
    let mut cands: Vec<Cand> = Vec::new();
    let no_symptom = interpretation.symptoms.iter().all(|s| s.negated);
    for (c, score) in plan.changes.iter().zip(scores.iter()) {
        let backed_by = |felt: &[String]| {
            c.rule_ids.iter().any(|r| {
                rule_symptom
                    .get(r.as_str())
                    .is_some_and(|s| felt.iter().any(|f| f == s))
            })
        };
        let from_feel = backed_by(&feel_symptoms);
        let explicit = backed_by(&explicit_feel);
        let prev = previous_changes
            .iter()
            .find(|p| p.parameter_id == c.parameter_id);
        match prev {
            Some(p)
                if p.direction == c.direction
                    && (p.direction != ChangeDirection::Set || p.magnitude == c.magnitude) =>
            {
                continue; // already in the plan
            }
            Some(p) if opposite(p.direction, c.direction) => {
                if explicit {
                    cands.push(Cand {
                        change: c.clone(),
                        score: *score,
                        from_feel,
                        reversal: Some(p.clone()),
                    });
                } else if from_feel {
                    notes.push(format!(
                        "{} : ton ressenti irait vers {} mais on l'a réglé {} à l'itération précédente ; on n'inverse pas sur un ressenti léger ou imprécis — dis-le franchement (phase, type de virage) si tu veux revenir en arrière.",
                        c.label_fr,
                        arrow(c.direction),
                        arrow(p.direction)
                    ));
                } else {
                    notes.push(format!(
                        "{} : les données demanderaient {} mais tu l'avais réglé {} à l'itération précédente ; on n'inverse pas tant que ton ressenti ne le demande pas.",
                        c.label_fr,
                        arrow(c.direction),
                        arrow(p.direction)
                    ));
                }
            }
            Some(_) => continue, // toward_zero / set vs other: leave the earlier choice alone
            None => cands.push(Cand {
                change: c.clone(),
                score: *score,
                from_feel,
                reversal: None,
            }),
        }
    }
    cands.sort_by(|a, b| {
        b.from_feel
            .cmp(&a.from_feel)
            .then(
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
            .then(a.change.parameter_id.cmp(&b.change.parameter_id))
    });
    let mut delta: Vec<SetupChange> = Vec::new();
    if no_symptom {
        notes.push(if interpretation.symptoms.is_empty() {
            "Aucun symptôme reconnu dans ton ressenti : pas de changement proposé. Réponds aux questions ci-dessus ou reformule.".into()
        } else {
            "Tu ne signales plus de symptôme : on garde le plan tel quel.".into()
        });
    } else if cands.is_empty() {
        notes.push(
            "Rien de nouveau à proposer : le plan courant couvre déjà ce que tu décris. Roule 5–8 tours propres avec, puis redis-moi."
                .into(),
        );
    } else {
        let family = group_order(&cands[0].change.parameter_id);
        let family_feel = cands[0].from_feel;
        let chosen: Vec<&Cand> = cands
            .iter()
            .filter(|c| group_order(&c.change.parameter_id) == family && c.from_feel == family_feel)
            .take(MAX_NEW_CHANGES)
            .collect();
        for (rank, cand) in chosen.iter().enumerate() {
            let mut c = cand.change.clone();
            if c.direction != ChangeDirection::Set {
                c.magnitude = one_step_magnitude(k, &c.parameter_id, &c.magnitude);
                c.steps = rules::parse_steps(&c.magnitude);
            }
            c.priority = (rank + 1).min(5) as u8;
            c.rationale_fr = format!("Itération {iteration} — {}", c.rationale_fr);
            c.rationale_en = format!("Iteration {iteration} — {}", c.rationale_en);
            if let Some(p) = &cand.reversal {
                notes.push(format!(
                    "{} : on revient en arrière ({} à l'itération précédente → {} maintenant) parce que ton ressenti le demande.",
                    c.label_fr,
                    arrow(p.direction),
                    arrow(c.direction)
                ));
                c.rationale_fr
                    .push_str(" (inversion demandée par le ressenti)");
            }
            delta.push(c);
        }
        notes.push(format!(
            "Itération {iteration} : {} changement{} sur la famille « {} », 1 cran chacun — une famille à la fois, puis 5–8 tours propres pour juger.",
            delta.len(),
            if delta.len() > 1 { "s" } else { "" },
            family_fr(family)
        ));
        let mut next: Vec<&str> = Vec::new();
        for c in cands
            .iter()
            .filter(|c| group_order(&c.change.parameter_id) != family)
        {
            let f = family_fr(group_order(&c.change.parameter_id));
            if !next.contains(&f) {
                next.push(f);
            }
            if next.len() == 2 {
                break;
            }
        }
        if !next.is_empty() {
            notes.push(format!(
                "Pistes suivantes si ça ne suffit pas : {}.",
                next.join(", ")
            ));
        }
    }

    // --- cumulative plan --------------------------------------------------------
    let mut changes: Vec<SetupChange> = previous_changes.to_vec();
    for d in &delta {
        match changes
            .iter_mut()
            .find(|c| c.parameter_id == d.parameter_id)
        {
            Some(c) => *c = d.clone(),
            None => changes.push(d.clone()),
        }
    }
    changes.sort_by(|a, b| {
        group_order(&a.parameter_id)
            .cmp(&group_order(&b.parameter_id))
            .then(a.priority.cmp(&b.priority))
            .then(a.parameter_id.cmp(&b.parameter_id))
    });
    // Engine warnings only when they concern a parameter changed now.
    for w in plan.warnings_fr.iter().filter(|w| {
        delta
            .iter()
            .any(|d| w.starts_with(&format!("{} :", d.label_fr)))
    }) {
        if !notes.contains(w) {
            notes.push(w.clone());
        }
    }

    PlanIteration {
        id: 0,
        session_id: ctx.session_id.to_string(),
        created_ms: now_ms(),
        iteration,
        base_plan: ctx.philosophy,
        feedback_id: ctx.feedback_id,
        interpretation: Some(interpretation.clone()),
        data_findings_used,
        changes,
        delta,
        agreements_fr: agreements,
        disagreements_fr: disagreements,
        notes_fr: notes,
        current_setup_known: false,
        dropped: Vec::new(),
    }
}

fn push_unique(v: &mut Vec<String>, s: &str) {
    if !v.iter().any(|x| x == s) {
        v.push(s.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::feedback::default_extractor;
    use crate::feedback::SymptomExtractor;
    use corde_protocol::{Axle, ExtractorKind};

    fn knowledge() -> Knowledge {
        Knowledge::embedded().unwrap()
    }

    fn finding(
        symptom: &str,
        severity: Severity,
        confidence: f32,
        phase: Option<CornerPhase>,
        speed: Option<CornerSpeed>,
    ) -> Finding {
        Finding {
            id: format!("D-TEST/{}", symptom.to_lowercase()),
            diagnostic_id: "D-TEST".into(),
            symptoms: vec![symptom.into()],
            severity,
            confidence,
            phase,
            corner_speed: speed,
            title_fr: format!("Données : {symptom}"),
            title_en: format!("Data: {symptom}"),
            ..Default::default()
        }
    }

    fn report(findings: Vec<Finding>, valid_laps: u32) -> AnalysisReport {
        let (meta, laps) = crate::tests::session_for_dump();
        let mut r =
            crate::analyse_samples(&meta, &laps, &crate::AnalyseOptions::default()).unwrap();
        r.findings = findings;
        r.data_quality.valid_laps = valid_laps;
        r
    }

    fn interp(text: &str) -> FeedbackInterpretation {
        default_extractor().extract(text, None)
    }

    fn ctx<'a>(
        base: &'a [SetupChange],
        previous: Option<&'a PlanIteration>,
        report: Option<&'a AnalysisReport>,
    ) -> IterationContext<'a> {
        IterationContext {
            session_id: "s1",
            philosophy: SetupPhilosophy::Safe,
            vehicle_class: VehicleClass::Gt3,
            wet_session: false,
            base,
            previous,
            latest_report: report,
            feedback_id: Some(7),
            unavailable_params: &[],
        }
    }

    #[test]
    fn expansion_uses_the_data_when_it_pinpoints_the_case() {
        let d = vec![finding(
            "US_ENTRY_FAST",
            Severity::Minor,
            0.7,
            Some(CornerPhase::Entry),
            Some(CornerSpeed::Fast),
        )];
        assert_eq!(
            expand_symptom("US_ENTRY", &d),
            vec![("US_ENTRY_FAST".to_string(), 1.0)]
        );
        let none = expand_symptom("US_ENTRY", &[]);
        assert_eq!(none[0].0, "US_ENTRY_SLOW");
        assert!(none[0].1 > none[1].1);
        assert_eq!(expand_symptom("US", &[]).len(), 6);
        assert_eq!(expand_symptom("BRK_LOCK", &[])[0].0, "BRK_LOCK_FRONT");
        assert_eq!(
            expand_symptom("BRK_LOCK_REAR", &[]),
            vec![("BRK_LOCK_REAR".to_string(), 1.0)]
        );
        assert_eq!(expand_symptom("JUMPS", &[]).len(), 2);
    }

    #[test]
    fn one_step_magnitudes() {
        let k = knowledge();
        assert_eq!(one_step_magnitude(&k, "susp.arb_front", "1 cran"), "1 cran");
        assert_eq!(
            one_step_magnitude(&k, "susp.sbump_front", "2 clics"),
            "2 clics",
            "one engineer step = 2 clicks"
        );
        assert_eq!(
            one_step_magnitude(&k, "brake.bias_front", "0.5–1 %"),
            "0,5 %"
        );
        assert_eq!(
            one_step_magnitude(&k, "tyre.pressure_front", "3 kPa (si > mini)"),
            "3 kPa"
        );
        assert_eq!(one_step_magnitude(&k, "diff.preload", "15 Nm"), "15 Nm");
        assert_eq!(one_step_magnitude(&k, "elec.tc", "1–2"), "1");
        assert_eq!(
            one_step_magnitude(&k, "geo.toe_front", "0.05° (vers ouverture)"),
            "0,05°"
        );
        assert_eq!(one_step_magnitude(&k, "unknown.param", "3 crans"), "1 cran");
        assert_eq!(one_step_magnitude(&k, "unknown.param", "wet"), "wet");
    }

    #[test]
    fn first_iteration_from_feeling_only_proposes_one_family_at_one_step() {
        let k = knowledge();
        let i = interp("l'avant ne tourne pas en entrée des virages lents");
        assert_eq!(i.symptoms[0].symptom_id, "US_ENTRY_SLOW");
        let it = iterate_plan(&ctx(&[], None, None), &i, &k);
        assert_eq!(it.iteration, 1);
        assert_eq!(it.base_plan, SetupPhilosophy::Safe);
        assert_eq!(it.feedback_id, Some(7));
        assert!(
            !it.delta.is_empty() && it.delta.len() <= MAX_NEW_CHANGES,
            "{:?}",
            it.delta
        );
        // Every new change comes from the understeer-entry-slow rule, in one family, at one step.
        let fam = group_order(&it.delta[0].parameter_id);
        for c in &it.delta {
            assert_eq!(group_order(&c.parameter_id), fam, "{c:?}");
            assert!(
                c.rule_ids.contains(&"US-ENTRY-SLOW-01".to_string()),
                "{c:?}"
            );
            assert!(
                c.rationale_fr
                    .starts_with("Itération 1 — Ressenti : Sous-virage en entrée (virages lents)"),
                "{}",
                c.rationale_fr
            );
            assert!(c.steps.is_none_or(|s| s.abs() <= 3), "{c:?}");
        }
        // The best-scored lever of that rule is the brake bias (prio 1, high confidence).
        assert_eq!(it.delta[0].parameter_id, "brake.bias_front");
        assert_eq!(it.delta[0].direction, ChangeDirection::Decrease);
        assert_eq!(it.delta[0].magnitude, "0,5 %");
        assert_eq!(it.delta[0].priority, 1);
        assert_eq!(
            it.changes.len(),
            it.delta.len(),
            "no base plan: the cumulative plan is the delta"
        );
        assert!(it.data_findings_used.is_empty());
        assert!(it.agreements_fr.is_empty() && it.disagreements_fr.is_empty());
        assert!(it.notes_fr.iter().any(|n| n.contains("ressenti seul")));
        assert!(
            it.notes_fr
                .iter()
                .any(|n| n.contains("Itération 1 : 1 changement sur la famille « freinage »")),
            "{:?}",
            it.notes_fr
        );
        assert!(it
            .notes_fr
            .iter()
            .any(|n| n.starts_with("Pistes suivantes")));
        assert_eq!(
            it.interpretation.as_ref().unwrap().extractor,
            ExtractorKind::Dictionary
        );
    }

    #[test]
    fn second_iteration_adds_a_delta_on_top_of_the_cumulative_plan() {
        let k = knowledge();
        let base_report = report(vec![], 3);
        let base = base_report.plans.safe.changes.clone();
        assert!(!base.is_empty());
        let first = iterate_plan(
            &ctx(&base, None, Some(&base_report)),
            &interp("l'avant ne tourne pas en entrée des virages lents"),
            &k,
        );
        assert_eq!(first.iteration, 1);
        assert!(!first.delta.is_empty());
        assert!(first.changes.len() >= base.len());
        for b in &base {
            assert!(
                first
                    .changes
                    .iter()
                    .any(|c| c.parameter_id == b.parameter_id),
                "base change kept: {}",
                b.parameter_id
            );
        }
        // Second feedback, another family comes out (the first lever is already in the plan).
        let second = iterate_plan(
            &ctx(&base, Some(&first), Some(&base_report)),
            &interp("toujours du sous-virage en entrée des lents, un peu moins"),
            &k,
        );
        assert_eq!(second.iteration, 2);
        assert!(!second.delta.is_empty(), "{:?}", second.notes_fr);
        for d in &second.delta {
            assert!(
                !first
                    .changes
                    .iter()
                    .any(|c| c.parameter_id == d.parameter_id),
                "new parameter: {}",
                d.parameter_id
            );
            assert!(d.rationale_fr.starts_with("Itération 2 —"));
        }
        assert!(second.changes.len() > first.changes.len());
        // Cumulative order follows the methodology.
        let orders: Vec<u8> = second
            .changes
            .iter()
            .map(|c| group_order(&c.parameter_id))
            .collect();
        assert!(orders.windows(2).all(|w| w[0] <= w[1]), "{orders:?}");
        // Everything of the first iteration is still there, untouched.
        for c in &first.changes {
            let still = second
                .changes
                .iter()
                .find(|x| x.parameter_id == c.parameter_id)
                .unwrap();
            assert_eq!(still.direction, c.direction);
        }
    }

    #[test]
    fn agreement_boosts_and_data_wins_a_strong_contradiction() {
        let k = knowledge();
        // Agreement: the driver feels what the data shows.
        let r = report(
            vec![finding(
                "US_ENTRY_SLOW",
                Severity::Minor,
                0.5,
                Some(CornerPhase::Entry),
                Some(CornerSpeed::Slow),
            )],
            4,
        );
        let it = iterate_plan(
            &ctx(&[], None, Some(&r)),
            &interp("l'avant ne tourne pas en entrée des lents"),
            &k,
        );
        assert_eq!(it.agreements_fr.len(), 1, "{:?}", it.agreements_fr);
        assert!(
            it.agreements_fr[0].contains("confiance 50 % → "),
            "{}",
            it.agreements_fr[0]
        );
        assert!(
            !it.agreements_fr[0].contains("→ 50 %"),
            "boosted: {}",
            it.agreements_fr[0]
        );
        assert_eq!(
            it.data_findings_used,
            vec!["D-TEST/us_entry_slow".to_string()]
        );
        assert!(it.disagreements_fr.is_empty());
        assert!(!it.delta.is_empty());
        assert!(
            it.delta[0].rationale_fr.contains("Données : US_ENTRY_SLOW"),
            "{}",
            it.delta[0].rationale_fr
        );

        // Contradiction, data sure: oversteer measured, understeer felt → data wins, no US lever.
        let r = report(
            vec![finding(
                "OS_ENTRY_SLOW",
                Severity::Major,
                0.8,
                Some(CornerPhase::Entry),
                Some(CornerSpeed::Slow),
            )],
            4,
        );
        let it = iterate_plan(
            &ctx(&[], None, Some(&r)),
            &interp("ça sous-vire en entrée des virages lents"),
            &k,
        );
        assert_eq!(it.disagreements_fr.len(), 1, "{:?}", it.disagreements_fr);
        assert!(it.disagreements_fr[0].contains("on suit les données"));
        assert!(it.disagreements_fr[0].contains("même virage"));
        assert!(
            it.delta
                .iter()
                .all(|c| !c.rule_ids.contains(&"US-ENTRY-SLOW-01".to_string())),
            "{:?}",
            it.delta
        );
        assert!(
            it.delta
                .iter()
                .any(|c| c.rule_ids.contains(&"OS-ENTRY-SLOW-01".to_string())),
            "{:?}",
            it.delta
        );

        // Contradiction, data unsure: the feeling wins.
        let r = report(
            vec![finding(
                "OS_ENTRY_SLOW",
                Severity::Minor,
                0.4,
                Some(CornerPhase::Entry),
                Some(CornerSpeed::Slow),
            )],
            4,
        );
        let it = iterate_plan(
            &ctx(&[], None, Some(&r)),
            &interp("ça sous-vire en entrée des virages lents"),
            &k,
        );
        assert_eq!(it.disagreements_fr.len(), 1);
        assert!(it.disagreements_fr[0].contains("on suit ton ressenti"));
        assert!(
            it.delta
                .iter()
                .any(|c| c.rule_ids.contains(&"US-ENTRY-SLOW-01".to_string())),
            "{:?}",
            it.delta
        );
        assert!(
            it.data_findings_used.is_empty(),
            "the weak data finding was dropped"
        );
    }

    #[test]
    fn relative_balance_findings_never_overrule_the_feeling() {
        let k = knowledge();
        // The engine's balance finding: strong, but relative to the session reference.
        let mut os = finding(
            "OS_ENTRY_SLOW",
            Severity::Major,
            0.9,
            Some(CornerPhase::Entry),
            Some(CornerSpeed::Slow),
        );
        os.diagnostic_id = RELATIVE_BALANCE_DIAGNOSTIC.into();
        os.id = "D-BAL-01/entry/slow".into();
        let r = report(vec![os.clone()], 6);
        let it = iterate_plan(
            &ctx(&[], None, Some(&r)),
            &interp("ça sous-vire en entrée des virages lents"),
            &k,
        );
        assert_eq!(it.disagreements_fr.len(), 1, "{:?}", it.disagreements_fr);
        assert!(it.disagreements_fr[0].contains("ton ressenti passe devant"));
        assert!(it.disagreements_fr[0].contains("on suit ton ressenti"));
        assert!(
            it.delta
                .iter()
                .any(|c| c.rule_ids.contains(&"US-ENTRY-SLOW-01".to_string())),
            "{:?}",
            it.delta
        );
        assert!(
            it.delta
                .iter()
                .all(|c| !c.rule_ids.contains(&"OS-ENTRY-SLOW-01".to_string())),
            "{:?}",
            it.delta
        );
        assert!(it.data_findings_used.is_empty());

        // "Plus de survirage en entrée" said by the driver removes even a strong relative finding.
        let r = report(vec![os], 6);
        let it = iterate_plan(
            &ctx(&[], None, Some(&r)),
            &interp("ça ne survire plus en entrée des lents"),
            &k,
        );
        assert!(
            it.data_findings_used.is_empty(),
            "{:?}",
            it.data_findings_used
        );
        assert!(
            it.agreements_fr
                .iter()
                .any(|a| a.contains("ton ressenti passe devant")),
            "{:?}",
            it.agreements_fr
        );
    }

    #[test]
    fn negated_symptom_cancels_a_weak_data_finding_and_reversal_needs_the_feeling() {
        let k = knowledge();
        let r = report(
            vec![finding("BRK_LOCK_FRONT", Severity::Minor, 0.5, None, None)],
            4,
        );
        let it = iterate_plan(
            &ctx(&[], None, Some(&r)),
            &interp("ça ne bloque plus de l'avant"),
            &k,
        );
        assert!(
            it.agreements_fr.iter().any(|a| a.contains("plus ressenti")),
            "{:?}",
            it.agreements_fr
        );
        assert!(it.data_findings_used.is_empty());
        assert!(it.delta.is_empty());
        assert!(it.notes_fr.iter().any(|n| n.contains("garde le plan")));

        // The previous iteration lowered the front bias for understeer; now the data (not the
        // driver) wants it up for rear locking → not reversed, and said so.
        let prev_plan = iterate_plan(
            &ctx(&[], None, None),
            &interp("l'avant ne tourne pas en entrée des lents"),
            &k,
        );
        assert_eq!(prev_plan.delta[0].parameter_id, "brake.bias_front");
        let r = report(
            vec![finding("BRK_LOCK_REAR", Severity::Major, 0.9, None, None)],
            4,
        );
        let it = iterate_plan(
            &ctx(&[], Some(&prev_plan), Some(&r)),
            &interp("ça saute sur les vibreurs"),
            &k,
        );
        assert!(
            it.notes_fr.iter().any(|n| n.contains("on n'inverse pas")),
            "{:?}",
            it.notes_fr
        );
        assert!(it
            .changes
            .iter()
            .any(|c| c.parameter_id == "brake.bias_front"
                && c.direction == ChangeDirection::Decrease));
        assert!(it
            .delta
            .iter()
            .all(|c| c.parameter_id != "brake.bias_front"));
        // When the driver asks for it, the reversal happens and is explained.
        let it = iterate_plan(
            &ctx(&[], Some(&prev_plan), None),
            &interp("ça bloque de l'arrière au freinage, beaucoup"),
            &k,
        );
        let bias = it
            .delta
            .iter()
            .find(|c| c.parameter_id == "brake.bias_front")
            .expect("reversal in delta");
        assert_eq!(bias.direction, ChangeDirection::Increase);
        assert!(bias.rationale_fr.contains("inversion demandée"));
        assert!(it
            .notes_fr
            .iter()
            .any(|n| n.contains("on revient en arrière")));
        assert_eq!(it.iteration, 2);
        let cum = it
            .changes
            .iter()
            .filter(|c| c.parameter_id == "brake.bias_front")
            .count();
        assert_eq!(cum, 1, "one entry per parameter in the cumulative plan");
    }

    #[test]
    fn nothing_recognised_gives_an_empty_delta_with_a_note() {
        let k = knowledge();
        let it = iterate_plan(&ctx(&[], None, None), &interp("belle journée"), &k);
        assert!(it.delta.is_empty());
        assert!(it
            .notes_fr
            .iter()
            .any(|n| n.contains("Aucun symptôme reconnu")));
        assert_eq!(it.iteration, 1);
        let json = serde_json::to_string(&it).unwrap();
        let back: PlanIteration = serde_json::from_str(&json).unwrap();
        assert_eq!(back, it);
    }

    #[test]
    fn mixed_feedback_targets_the_strongest_symptom_family_first() {
        let k = knowledge();
        let i = interp("le train avant pousse en entrée mais l'arrière part à la remise des gaz dans les lents");
        assert!(i.symptoms.iter().any(|s| s.axle == Some(Axle::Rear)));
        let it = iterate_plan(&ctx(&[], None, None), &i, &k);
        assert!(!it.delta.is_empty());
        let fam = group_order(&it.delta[0].parameter_id);
        assert!(it.delta.iter().all(|c| group_order(&c.parameter_id) == fam));
        assert!(it.delta.len() <= MAX_NEW_CHANGES);
        assert!(it
            .notes_fr
            .iter()
            .any(|n| n.starts_with("Pistes suivantes")));
    }

    #[test]
    fn a_mild_or_vague_feeling_does_not_undo_the_previous_change() {
        let k = knowledge();
        // Iteration 1 lowered the front brake bias for understeer on entry in slow corners.
        let prev = iterate_plan(
            &ctx(&[], None, None),
            &interp("l'avant ne tourne pas en entrée des lents"),
            &k,
        );
        assert_eq!(prev.delta[0].parameter_id, "brake.bias_front");
        assert_eq!(prev.delta[0].direction, ChangeDirection::Decrease);
        // "a bit" of rear locking, generic: the rule would raise the bias, but not on this.
        let it = iterate_plan(
            &ctx(&[], Some(&prev), None),
            &interp("ça bloque un peu de l'arrière au freinage"),
            &k,
        );
        assert!(
            it.delta
                .iter()
                .all(|c| c.parameter_id != "brake.bias_front"),
            "{:?}",
            it.delta
        );
        assert!(
            it.notes_fr
                .iter()
                .any(|n| n.contains("on n'inverse pas sur un ressenti léger")),
            "{:?}",
            it.notes_fr
        );
        assert!(it
            .changes
            .iter()
            .any(|c| c.parameter_id == "brake.bias_front"
                && c.direction == ChangeDirection::Decrease));
        assert_eq!(it.iteration, 2);
    }
}
