//! The knowledge base (`knowledge/setup-rules.yaml`, `knowledge/setup-parameters.yaml`)
//! and the rule engine that turns findings into the three setup plans.
//!
//! Both YAML files are embedded at compile time ([`Knowledge::embedded`]) and
//! can be overridden by a directory holding the same two files
//! ([`Knowledge::from_dir`]) so that the knowledge can be tuned without a
//! rebuild.
//!
//! Engine (per philosophy, see `docs/ENGINEER.md` §4):
//! 1. keep the rules whose surface / philosophy / class fit the plan;
//! 2. a rule fires when one of its symptoms is carried by a finding of
//!    severity ≥ Minor (weight = confidence × severity); the philosophy's base
//!    rule (`PHI-*`, `WET-PACK-01`) always fires, with a low "intent" weight
//!    when no finding supports it;
//! 3. every change of a firing rule becomes a candidate scored
//!    `weight × change confidence × 1 / (1 + 0.35 (prio − 1))`, with a ×1.25
//!    bias for the families the philosophy protects (ARB-02 / ARB-05);
//! 4. candidates are merged per parameter — same direction: scores add up
//!    (best + half of the others), opposite directions: the best wins and a
//!    warning is written (ARB-01);
//! 5. the best `max_changes` survive, declared conflicts between surviving
//!    rules are annotated, and the list is ordered by the methodology
//!    (tyres → aero → brakes → bars → springs → dampers → diff → geometry →
//!    electronics).

use crate::error::{EngineerError, Result};
use crate::feedback::FeedbackDictionary;
use corde_protocol::{
    ChangeDirection, Finding, SetupChange, SetupPhilosophy, SetupPlan, SetupPlans, Severity,
    VehicleClass,
};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

pub const RULES_YAML: &str = include_str!("../../../knowledge/setup-rules.yaml");
pub const PARAMETERS_YAML: &str = include_str!("../../../knowledge/setup-parameters.yaml");

// ---------------------------------------------------------------------------
// YAML model (knowledge/README.md §3.3 and §3.1)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, Default)]
pub struct RuleSet {
    #[serde(default)]
    pub schema_version: u32,
    #[serde(default)]
    pub params: Vec<ParamSign>,
    #[serde(default)]
    pub default_steps: HashMap<String, String>,
    #[serde(default)]
    pub rules: Vec<Rule>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ParamSign {
    pub id: String,
    #[serde(default)]
    pub unit: String,
    #[serde(default)]
    pub sign: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Rule {
    pub id: String,
    pub symptom: String,
    #[serde(default)]
    pub family: String,
    #[serde(default = "any")]
    pub phase: String,
    #[serde(default = "any")]
    pub corner_speed: String,
    #[serde(default = "any")]
    pub surface: String,
    #[serde(default)]
    pub classes: Vec<String>,
    #[serde(default = "both")]
    pub philosophy: String,
    #[serde(default)]
    pub trigger: Trigger,
    #[serde(default)]
    pub changes: Vec<Change>,
    #[serde(default)]
    pub justification: String,
    #[serde(default)]
    pub sources: Vec<String>,
    #[serde(default)]
    pub brief_section: String,
}

fn any() -> String {
    "any".into()
}

fn both() -> String {
    "both".into()
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Trigger {
    #[serde(default)]
    pub telemetry: Option<String>,
    #[serde(default)]
    pub diagnostics: Vec<String>,
    #[serde(default)]
    pub feel: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Change {
    #[serde(default = "one")]
    pub prio: u32,
    pub param: String,
    pub direction: String,
    #[serde(default)]
    pub value: Option<String>,
    #[serde(default)]
    pub magnitude: Option<String>,
    #[serde(default = "medium")]
    pub confidence: String,
    #[serde(default)]
    pub side_effects: Option<String>,
    #[serde(default)]
    pub conflicts: Vec<String>,
}

fn one() -> u32 {
    1
}

fn medium() -> String {
    "medium".into()
}

impl RuleSet {
    pub fn from_yaml(text: &str) -> Result<Self> {
        serde_yaml::from_str(text)
            .map_err(|e| EngineerError::Knowledge(format!("setup-rules.yaml : {e}")))
    }

    pub fn rule(&self, id: &str) -> Option<&Rule> {
        self.rules.iter().find(|r| r.id == id)
    }
}

/// The parts of `setup-parameters.yaml` the engine needs.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ParamCatalog {
    #[serde(default)]
    pub parameters: Vec<ParamEntry>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ParamEntry {
    pub id: String,
    #[serde(default)]
    pub label_fr: String,
    #[serde(default)]
    pub group: String,
    #[serde(default)]
    pub svm: Option<SvmKey>,
    /// Vehicle classes (snake_case) where the parameter exists; empty = all.
    #[serde(default)]
    pub classes: Vec<String>,
    #[serde(default)]
    pub unit: Option<String>,
    #[serde(default)]
    pub default_step: Option<String>,
    /// `VM_*` object of the garage data, when known.
    #[serde(default)]
    pub rest_key: Option<String>,
    /// Physical size of one index step by default (schema v2).
    #[serde(default)]
    pub index_step: Option<IndexStep>,
    /// A logical parameter without key, applied through another one (schema v2).
    #[serde(default)]
    pub svm_translation: Option<SvmTranslation>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct SvmKey {
    #[serde(default)]
    pub section: String,
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub per_wheel: bool,
    /// +1: the index goes up when the parameter increases (its `sign` in
    /// `setup-rules.yaml`), −1: the other way, 0: discrete / unknown.
    #[serde(default)]
    pub index_sign: i32,
    /// The game shows the index itself as the label (TC maps: `8//8`).
    #[serde(default)]
    pub label_is_index: bool,
}

/// `index_step: {value, unit, confidence, source}`.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct IndexStep {
    pub value: f64,
    #[serde(default)]
    pub unit: String,
    #[serde(default = "medium")]
    pub confidence: String,
    #[serde(default)]
    pub source: String,
}

/// `svm_translation: {param, note}`.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct SvmTranslation {
    pub param: String,
    #[serde(default)]
    pub note: String,
}

impl ParamCatalog {
    pub fn from_yaml(text: &str) -> Result<Self> {
        serde_yaml::from_str(text)
            .map_err(|e| EngineerError::Knowledge(format!("setup-parameters.yaml : {e}")))
    }

    pub fn get(&self, id: &str) -> Option<&ParamEntry> {
        self.parameters.iter().find(|p| p.id == id)
    }
}

/// Rules + parameter catalogue + feedback dictionary.
#[derive(Debug, Clone, Default)]
pub struct Knowledge {
    pub rules: RuleSet,
    pub params: ParamCatalog,
    pub feedback: FeedbackDictionary,
}

impl Knowledge {
    /// The YAML files compiled into the binary.
    pub fn embedded() -> Result<Self> {
        Ok(Self {
            rules: RuleSet::from_yaml(RULES_YAML)?,
            params: ParamCatalog::from_yaml(PARAMETERS_YAML)?,
            feedback: FeedbackDictionary::embedded()?,
        })
    }

    /// `<dir>/setup-rules.yaml`, `<dir>/setup-parameters.yaml` and
    /// `<dir>/feedback-dictionary.yaml` (the last two fall back to the
    /// embedded ones when absent).
    pub fn from_dir(dir: &Path) -> Result<Self> {
        let rules = std::fs::read_to_string(dir.join("setup-rules.yaml")).map_err(|e| {
            EngineerError::Knowledge(format!("{} : {e}", dir.join("setup-rules.yaml").display()))
        })?;
        let params = match std::fs::read_to_string(dir.join("setup-parameters.yaml")) {
            Ok(p) => ParamCatalog::from_yaml(&p)?,
            Err(_) => ParamCatalog::from_yaml(PARAMETERS_YAML)?,
        };
        Ok(Self {
            rules: RuleSet::from_yaml(&rules)?,
            params,
            feedback: FeedbackDictionary::from_dir(dir)?,
        })
    }

    /// Every `symptom` a rule can fire on (sorted, unique).
    pub fn rule_symptoms(&self) -> Vec<String> {
        let mut v: Vec<String> = self.rules.rules.iter().map(|r| r.symptom.clone()).collect();
        v.sort();
        v.dedup();
        v
    }
}

// ---------------------------------------------------------------------------
// Engine
// ---------------------------------------------------------------------------

/// Default cap of changes per plan.
pub const MAX_CHANGES_PER_PLAN: usize = 8;
/// Weight of the philosophy's base rule when no finding supports it.
const INTENT_WEIGHT: f64 = 0.45;

fn confidence_weight(c: &str) -> f64 {
    match c {
        "high" => 0.9,
        "medium" => 0.65,
        "low" => 0.4,
        _ => 0.5,
    }
}

fn prio_weight(prio: u32) -> f64 {
    1.0 / (1.0 + 0.35 * (prio.max(1) - 1) as f64)
}

fn severity_weight(s: Severity) -> f64 {
    match s {
        Severity::Major => 1.5,
        Severity::Minor => 1.0,
        Severity::Info => 0.0,
    }
}

fn class_slug(class: VehicleClass) -> Option<&'static str> {
    match class {
        VehicleClass::Hypercar => Some("hypercar"),
        VehicleClass::Lmp2 => Some("lmp2"),
        VehicleClass::Lmp3 => Some("lmp3"),
        VehicleClass::Gt3 => Some("gt3"),
        VehicleClass::Gte => Some("gte"),
        _ => None,
    }
}

/// Which rule family the philosophy favours (ARB-02 / ARB-05).
fn family_bias(philosophy: SetupPhilosophy, family: &str) -> f64 {
    match philosophy {
        SetupPhilosophy::Safe => match family {
            "braking" | "oversteer" | "platform" | "tyres" => 1.25,
            _ => 1.0,
        },
        SetupPhilosophy::Aggressive => match family {
            "understeer" | "traction" | "straightline_and_response" => 1.25,
            _ => 1.0,
        },
        SetupPhilosophy::Wet => 1.0,
    }
}

/// Rule the plan always starts from.
fn base_rule_id(philosophy: SetupPhilosophy) -> &'static str {
    match philosophy {
        SetupPhilosophy::Aggressive => "PHI-AGGRESSIVE-01",
        SetupPhilosophy::Safe => "PHI-SAFE-01",
        SetupPhilosophy::Wet => "WET-PACK-01",
    }
}

fn rule_fits(rule: &Rule, philosophy: SetupPhilosophy, class: VehicleClass) -> bool {
    let surface_ok = match philosophy {
        SetupPhilosophy::Wet => rule.surface == "wet" || rule.surface == "any",
        _ => rule.surface == "dry" || rule.surface == "any",
    };
    let phil_ok = match philosophy {
        SetupPhilosophy::Aggressive => rule.philosophy == "both" || rule.philosophy == "aggressive",
        SetupPhilosophy::Safe | SetupPhilosophy::Wet => {
            rule.philosophy == "both" || rule.philosophy == "safe"
        }
    };
    let class_ok = match class_slug(class) {
        Some(slug) => rule.classes.is_empty() || rule.classes.iter().any(|c| c == slug),
        None => true,
    };
    surface_ok && phil_ok && class_ok
}

/// Methodology order of the parameter groups (E.1): 0 tyres, 1 aero, 2 brakes,
/// 3 anti-roll bars, 4 springs / packers, 5 dampers, 6 differential, 7 geometry,
/// 8 electronics, 9 other.
pub fn group_order(param: &str) -> u8 {
    let group = param.split('.').next().unwrap_or("");
    match group {
        "tyre" => 0,
        "aero" => 1,
        "brake" => 2,
        "susp" => {
            if param.contains("arb_") {
                3
            } else if param.contains("bump") || param.contains("rebound") {
                5
            } else {
                4
            }
        }
        "diff" => 6,
        "geo" | "steer" => 7,
        "elec" | "energy" | "fuel" => 8,
        _ => 9,
    }
}

/// Leading whole number of a magnitude ("2 clics" → 2, "+2 mm AR" → 2,
/// "−1 mm AR" → −1, "0.5–1 %" → None, "1–2 crans" → None).
pub fn parse_steps(magnitude: &str) -> Option<i32> {
    let s = magnitude.trim();
    let (sign, rest) = if let Some(r) = s.strip_prefix('+') {
        (1, r)
    } else if let Some(r) = s.strip_prefix('−').or(s.strip_prefix('-')) {
        (-1, r)
    } else {
        (1, s)
    };
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    let after = &rest[digits.len()..];
    // A decimal or a range is not a whole number of steps.
    if after.starts_with('.')
        || after.starts_with(',')
        || after.starts_with('–')
        || after.starts_with('-')
    {
        return None;
    }
    digits.parse::<i32>().ok().map(|v| v * sign)
}

#[derive(Debug, Clone)]
struct Candidate {
    param: String,
    direction: ChangeDirection,
    magnitude: String,
    score: f64,
    confidence: f64,
    rule_id: String,
    finding_fr: String,
    finding_en: String,
    justification: String,
    side_effects: Option<String>,
    conflicts: Vec<String>,
    without_diagnostic: bool,
}

/// The best finding behind a rule: `(weight, title_fr, title_en, confidence)`.
fn best_support(rule: &Rule, findings: &[Finding]) -> Option<(f64, String, String, f64)> {
    findings
        .iter()
        .filter(|f| f.symptoms.iter().any(|s| s == &rule.symptom))
        .map(|f| {
            (
                f.confidence as f64 * severity_weight(f.severity),
                f.title_fr.clone(),
                f.title_en.clone(),
                f.confidence as f64,
            )
        })
        .filter(|(w, ..)| *w > 0.0)
        .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal))
}

fn intent_fr(philosophy: SetupPhilosophy) -> &'static str {
    match philosophy {
        SetupPhilosophy::Aggressive => "Voiture vive, orientée temps au tour (sprint / qualif) : plus de rotation, moins d'électronique, usure acceptée.",
        SetupPhilosophy::Safe => "Voiture stable et prévisible pour les relais longs : stabilité au freinage, motricité, usure et consommation contenues.",
        SetupPhilosophy::Wet => "Paquet pluie : pneus pluie, plus d'appui et de garde au sol, suspensions plus souples, freinage plus avant, plus d'électronique.",
    }
}

pub(crate) fn philosophy_fr(p: SetupPhilosophy) -> &'static str {
    match p {
        SetupPhilosophy::Aggressive => "agressive",
        SetupPhilosophy::Safe => "sûre",
        SetupPhilosophy::Wet => "pluie",
    }
}

/// Build one plan.
pub fn plan(
    k: &Knowledge,
    philosophy: SetupPhilosophy,
    findings: &[Finding],
    class: VehicleClass,
    wet_session: bool,
    max_changes: usize,
) -> SetupPlan {
    plan_scored(k, philosophy, findings, class, wet_session, max_changes).0
}

/// [`plan`] plus the engine score of each change (same order as `changes`),
/// which the iteration mode uses to pick the lever to try first.
pub fn plan_scored(
    k: &Knowledge,
    philosophy: SetupPhilosophy,
    findings: &[Finding],
    class: VehicleClass,
    wet_session: bool,
    max_changes: usize,
) -> (SetupPlan, Vec<f64>) {
    plan_scored_excluding(
        k,
        philosophy,
        findings,
        class,
        wet_session,
        max_changes,
        &|_| false,
    )
}

/// [`plan_scored`] without the parameters `exclude` refuses (not adjustable
/// on the driver's car): the next best candidates take their place.
pub fn plan_scored_excluding(
    k: &Knowledge,
    philosophy: SetupPhilosophy,
    findings: &[Finding],
    class: VehicleClass,
    wet_session: bool,
    max_changes: usize,
    exclude: &dyn Fn(&str) -> bool,
) -> (SetupPlan, Vec<f64>) {
    let max_changes = max_changes.clamp(1, 12);
    let mut warnings: Vec<String> = Vec::new();
    let mut candidates: Vec<Candidate> = Vec::new();
    let base_id = base_rule_id(philosophy);
    let mut any_diagnostic = false;

    for rule in k
        .rules
        .rules
        .iter()
        .filter(|r| rule_fits(r, philosophy, class))
    {
        let support = best_support(rule, findings);
        let is_base = rule.id == base_id;
        let (weight, finding_fr, finding_en, finding_conf, without_diagnostic) = match support {
            Some((w, fr, en, c)) => (w, fr, en, c, false),
            None if is_base => (
                INTENT_WEIGHT,
                format!("Orientation {} sans diagnostic", philosophy_fr(philosophy)),
                format!("Generic {:?} orientation (no diagnostic)", philosophy).to_lowercase(),
                0.4,
                true,
            ),
            None => continue,
        };
        if !without_diagnostic {
            any_diagnostic = true;
        }
        let bias = family_bias(philosophy, &rule.family);
        for ch in &rule.changes {
            if !param_exists_for(k, &ch.param, class) || exclude(&ch.param) {
                continue;
            }
            let direction = match ch.direction.as_str() {
                "increase" => ChangeDirection::Increase,
                "decrease" => ChangeDirection::Decrease,
                "toward_zero" => ChangeDirection::TowardZero,
                "set" => ChangeDirection::Set,
                "check" => {
                    if let Some(m) = &ch.magnitude {
                        warnings.push(format!(
                            "{} : à vérifier avant de toucher au setup — {m}.",
                            rule.id
                        ));
                    }
                    continue;
                }
                "either" => {
                    if let Some(m) = &ch.magnitude {
                        warnings.push(format!(
                            "{} : {} à régler selon le cas — {m}.",
                            rule.id,
                            label_of(k, &ch.param)
                        ));
                    }
                    continue;
                }
                _ => continue, // meta: the balance rules already fire on their own findings
            };
            let magnitude = match (&ch.value, &ch.magnitude) {
                (Some(v), Some(m)) => format!("{v} ({m})"),
                (Some(v), None) => v.clone(),
                (None, Some(m)) => m.clone(),
                (None, None) => k
                    .params
                    .get(&ch.param)
                    .and_then(|p| p.default_step.clone())
                    .unwrap_or_else(|| "1 pas".into()),
            };
            let cw = confidence_weight(&ch.confidence);
            candidates.push(Candidate {
                param: ch.param.clone(),
                direction,
                magnitude,
                score: weight * cw * prio_weight(ch.prio) * bias,
                confidence: (finding_conf * cw).clamp(0.0, 1.0),
                rule_id: rule.id.clone(),
                finding_fr: finding_fr.clone(),
                finding_en: finding_en.clone(),
                justification: rule.justification.clone(),
                side_effects: ch.side_effects.clone(),
                conflicts: ch.conflicts.clone(),
                without_diagnostic,
            });
        }
    }

    // Merge per parameter (ARB-01: never two opposite changes on one parameter).
    let mut by_param: HashMap<String, Vec<Candidate>> = HashMap::new();
    for c in candidates {
        by_param.entry(c.param.clone()).or_default().push(c);
    }
    struct Merged {
        best: Candidate,
        rule_ids: Vec<String>,
        score: f64,
        side_effects: Vec<String>,
        /// Conflicts declared by the YAML (`conflicts: [...]`).
        declared: Vec<String>,
        /// Rules that wanted the opposite direction on this parameter.
        opposed: Vec<String>,
    }
    let mut merged: Vec<Merged> = Vec::new();
    // Deterministic order (HashMap iteration is not): by parameter id.
    let mut groups: Vec<(String, Vec<Candidate>)> = by_param.into_iter().collect();
    groups.sort_by(|a, b| a.0.cmp(&b.0));
    for (_param, mut group) in groups {
        group.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.rule_id.cmp(&b.rule_id))
        });
        let best = group[0].clone();
        let mut score = best.score;
        let mut rule_ids = vec![best.rule_id.clone()];
        let mut side_effects: Vec<String> = best.side_effects.iter().cloned().collect();
        let mut declared = best.conflicts.clone();
        let mut opposed: Vec<String> = Vec::new();
        for other in group.iter().skip(1) {
            if other.direction == best.direction {
                score += 0.5 * other.score;
                if !rule_ids.contains(&other.rule_id) {
                    rule_ids.push(other.rule_id.clone());
                }
                if let Some(se) = &other.side_effects {
                    if !side_effects.contains(se) {
                        side_effects.push(se.clone());
                    }
                }
                for c in &other.conflicts {
                    if !declared.contains(c) {
                        declared.push(c.clone());
                    }
                }
            } else {
                if !opposed.contains(&other.rule_id) {
                    opposed.push(other.rule_id.clone());
                }
                // Only a real contest is worth a warning (ARB-01 / ARB-02).
                if other.score >= 0.5 * best.score {
                    warnings.push(format!(
                        "{} : {} et {} s'opposent (score {:.2} vs {:.2}) — on garde {} (ARB-01/02).",
                        label_of(k, &best.param),
                        best.rule_id,
                        other.rule_id,
                        best.score,
                        other.score,
                        best.rule_id
                    ));
                }
            }
        }
        merged.push(Merged {
            best,
            rule_ids,
            score,
            side_effects,
            declared,
            opposed,
        });
    }
    merged.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.best.param.cmp(&b.best.param))
    });
    let kept: Vec<Merged> = merged.into_iter().take(max_changes).collect();

    // Declared conflicts between surviving rules (ARB-02: keep both, say it).
    // A declared conflict matters when the other rule also drives a surviving
    // change on another parameter (same-parameter contests were settled above).
    let mut changes: Vec<(f64, SetupChange)> = Vec::new();
    for (rank, m) in kept.iter().enumerate() {
        let live_conflicts: Vec<String> = m
            .declared
            .iter()
            .filter(|c| {
                !m.rule_ids.contains(c)
                    && kept
                        .iter()
                        .any(|o| o.best.param != m.best.param && o.rule_ids.contains(c))
            })
            .cloned()
            .collect();
        for c in &live_conflicts {
            let pair = if m.best.rule_id < *c {
                (m.best.rule_id.clone(), c.clone())
            } else {
                (c.clone(), m.best.rule_id.clone())
            };
            let w = format!(
                "{} et {} agissent en sens contraire : garder un seul des deux leviers par itération (ARB-02).",
                pair.0, pair.1
            );
            if !warnings.contains(&w) {
                warnings.push(w);
            }
        }
        let mut conflicts_with = live_conflicts.clone();
        for o in &m.opposed {
            if !conflicts_with.contains(o) {
                conflicts_with.push(o.clone());
            }
        }
        let entry = k.params.get(&m.best.param);
        let (key, section) = match entry.and_then(|e| e.svm.as_ref()) {
            Some(svm) if !svm.key.is_empty() => (svm.key.clone(), svm.section.clone()),
            _ => (m.best.param.clone(), String::new()),
        };
        let label_fr = entry
            .map(|e| e.label_fr.clone())
            .filter(|l| !l.is_empty())
            .unwrap_or_else(|| m.best.param.clone());
        let rationale_fr = if m.best.without_diagnostic {
            format!(
                "{} — {}",
                m.best.finding_fr,
                if m.best.justification.is_empty() {
                    m.best.rule_id.clone()
                } else {
                    m.best.justification.clone()
                }
            )
        } else {
            format!("{} → {}", m.best.finding_fr, m.best.justification)
        };
        let rationale_en = format!(
            "{} → rule {}{}",
            m.best.finding_en,
            m.best.rule_id,
            if m.rule_ids.len() > 1 {
                format!(" (also {})", m.rule_ids[1..].join(", "))
            } else {
                String::new()
            }
        );
        changes.push((
            m.score,
            SetupChange {
                parameter_id: m.best.param.clone(),
                parameter_key: key,
                section,
                label_fr,
                direction: m.best.direction,
                magnitude: m.best.magnitude.clone(),
                steps: parse_steps(&m.best.magnitude),
                priority: (1 + rank / 2).min(5) as u8,
                rule_ids: m.rule_ids.clone(),
                rationale_fr,
                rationale_en,
                side_effects_fr: m.side_effects.clone(),
                conflicts_with,
                confidence: m.best.confidence as f32,
                resolved: None,
            },
        ));
    }
    changes.sort_by(|a, b| {
        group_order(&a.1.parameter_id)
            .cmp(&group_order(&b.1.parameter_id))
            .then(b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal))
            .then(a.1.parameter_id.cmp(&b.1.parameter_id))
    });

    if !any_diagnostic {
        warnings.insert(
            0,
            if philosophy == SetupPhilosophy::Wet {
                "Aucun constat lié à la pluie dans cette session : paquet pluie générique, confiance basse.".into()
            } else {
                "Aucun diagnostic exploitable : orientations génériques de la philosophie, confiance basse.".into()
            },
        );
    }
    if philosophy == SetupPhilosophy::Wet && !wet_session {
        warnings.insert(
            0,
            "Session sèche : paquet pluie générique, à affiner après un premier run sous la pluie."
                .into(),
        );
    }
    let scores = changes.iter().map(|(s, _)| *s).collect();
    (
        SetupPlan {
            philosophy,
            intent_fr: intent_fr(philosophy).to_string(),
            changes: changes.into_iter().map(|(_, c)| c).collect(),
            warnings_fr: warnings,
            dropped: Vec::new(),
        },
        scores,
    )
}

/// `false` when the catalogue says the parameter does not exist on this
/// class (`brake.migration` on a GT3…). Unknown parameters / classes pass.
fn param_exists_for(k: &Knowledge, param: &str, class: VehicleClass) -> bool {
    match (k.params.get(param), class_slug(class)) {
        (Some(p), Some(slug)) => p.classes.is_empty() || p.classes.iter().any(|c| c == slug),
        _ => true,
    }
}

pub(crate) fn label_of(k: &Knowledge, param: &str) -> String {
    k.params
        .get(param)
        .map(|p| p.label_fr.clone())
        .filter(|l| !l.is_empty())
        .unwrap_or_else(|| param.to_string())
}

/// The three plans without the parameters `exclude` refuses.
pub fn build_plans_excluding(
    k: &Knowledge,
    findings: &[Finding],
    class: VehicleClass,
    wet_session: bool,
    max_changes: usize,
    exclude: &dyn Fn(&str) -> bool,
) -> SetupPlans {
    let one = |p| plan_scored_excluding(k, p, findings, class, wet_session, max_changes, exclude).0;
    SetupPlans {
        current_setup_known: false,
        base: None,
        aggressive: one(SetupPhilosophy::Aggressive),
        safe: one(SetupPhilosophy::Safe),
        wet: one(SetupPhilosophy::Wet),
    }
}

/// The three plans.
pub fn build_plans(
    k: &Knowledge,
    findings: &[Finding],
    class: VehicleClass,
    wet_session: bool,
    setup_known: bool,
    max_changes: usize,
) -> SetupPlans {
    SetupPlans {
        current_setup_known: setup_known,
        base: None,
        aggressive: plan(
            k,
            SetupPhilosophy::Aggressive,
            findings,
            class,
            wet_session,
            max_changes,
        ),
        safe: plan(
            k,
            SetupPhilosophy::Safe,
            findings,
            class,
            wet_session,
            max_changes,
        ),
        wet: plan(
            k,
            SetupPhilosophy::Wet,
            findings,
            class,
            wet_session,
            max_changes,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"
schema_version: 1
params:
  - {id: brake.bias_front, unit: "%", sign: "+ = plus avant"}
rules:
  - id: US-ENTRY-SLOW-01
    symptom: US_ENTRY_SLOW
    family: understeer
    phase: entry
    corner_speed: slow
    surface: dry
    classes: [gt3]
    philosophy: both
    changes:
      - {prio: 1, param: brake.bias_front, direction: decrease, magnitude: "0.5–1 %", confidence: high, side_effects: "blocage AR", conflicts: [BRK-LOCK-REAR-01]}
      - {prio: 2, param: susp.arb_front, direction: decrease, magnitude: "1 cran", confidence: high, side_effects: null, conflicts: []}
      - {prio: 3, param: geo.toe_front, direction: toward_zero, magnitude: "0.05°", confidence: medium, conflicts: []}
      - {prio: 4, param: elec.engine_map, direction: check, magnitude: "frein moteur : rétrograder plus tard", confidence: high, conflicts: []}
    justification: "L'avant sature sous freinage."
  - id: BRK-LOCK-REAR-01
    symptom: BRK_LOCK_REAR
    family: braking
    phase: braking
    surface: dry
    classes: [gt3]
    philosophy: both
    changes:
      - {prio: 1, param: brake.bias_front, direction: increase, magnitude: "1 %", confidence: high, conflicts: [US-ENTRY-SLOW-01]}
      - {prio: 2, param: susp.arb_front, direction: decrease, magnitude: "1 cran", confidence: medium, conflicts: []}
    justification: "L'arrière bloque."
  - id: HY-ONLY-01
    symptom: BRK_LOCK_REAR
    family: braking
    surface: dry
    classes: [hypercar]
    philosophy: both
    changes:
      - {prio: 1, param: brake.migration, direction: increase, magnitude: "0.5", confidence: high, conflicts: []}
  - id: PHI-AGGRESSIVE-01
    symptom: PHILOSOPHY_AGGRESSIVE
    family: philosophy
    surface: dry
    classes: [gt3, hypercar]
    philosophy: aggressive
    changes:
      - {prio: 1, param: elec.tc, direction: decrease, magnitude: "1", confidence: medium, conflicts: []}
      - {prio: 2, param: aero.rear_wing, direction: decrease, magnitude: "1 cran", confidence: medium, conflicts: []}
    justification: "Sprint."
  - id: PHI-SAFE-01
    symptom: PHILOSOPHY_SAFE
    family: philosophy
    surface: dry
    classes: [gt3, hypercar]
    philosophy: safe
    changes:
      - {prio: 1, param: elec.tc, direction: increase, magnitude: "1–2", confidence: high, conflicts: []}
      - {prio: 2, param: aero.rear_wing, direction: increase, magnitude: "1 cran", confidence: high, conflicts: []}
      - {prio: 3, param: tyre.pressure_front, direction: increase, magnitude: "2–3 kPa", confidence: low, conflicts: []}
    justification: "Endurance."
  - id: WET-PACK-01
    symptom: WET_BASELINE
    family: wet
    surface: wet
    classes: [gt3, hypercar]
    philosophy: both
    changes:
      - {prio: 1, param: tyre.compound, direction: set, value: wet, magnitude: null, confidence: high, conflicts: []}
      - {prio: 2, param: aero.rear_wing, direction: increase, magnitude: "1–2 crans", confidence: high, conflicts: []}
    justification: "Pluie."
"#;

    fn knowledge() -> Knowledge {
        Knowledge {
            rules: RuleSet::from_yaml(FIXTURE).unwrap(),
            params: ParamCatalog::from_yaml(PARAMETERS_YAML).unwrap(),
            feedback: FeedbackDictionary::default(),
        }
    }

    fn finding(symptom: &str, severity: Severity, confidence: f32) -> Finding {
        Finding {
            id: symptom.to_lowercase(),
            diagnostic_id: "D-TEST".into(),
            symptoms: vec![symptom.into()],
            severity,
            confidence,
            title_fr: format!("{symptom} (fr)"),
            title_en: format!("{symptom} (en)"),
            ..Default::default()
        }
    }

    #[test]
    fn embedded_knowledge_parses_and_is_consistent() {
        let k = Knowledge::embedded().unwrap();
        assert_eq!(k.rules.rules.len(), 46);
        assert!(
            k.params
                .get("aero.rear_wing")
                .unwrap()
                .svm
                .as_ref()
                .unwrap()
                .key
                == "RWSetting"
        );
        // Every parameter used by a rule exists in the catalogue (meta.balance excepted).
        for r in &k.rules.rules {
            for c in &r.changes {
                assert!(
                    c.param == "meta.balance" || k.params.get(&c.param).is_some(),
                    "{}: unknown param {}",
                    r.id,
                    c.param
                );
            }
        }
        for id in ["PHI-AGGRESSIVE-01", "PHI-SAFE-01", "WET-PACK-01"] {
            assert!(k.rules.rule(id).is_some(), "{id}");
        }
    }

    #[test]
    fn conflict_on_one_parameter_keeps_the_best_and_warns() {
        let k = knowledge();
        let findings = vec![
            finding("US_ENTRY_SLOW", Severity::Major, 0.8),
            finding("BRK_LOCK_REAR", Severity::Major, 0.7),
        ];
        let p = plan(
            &k,
            SetupPhilosophy::Aggressive,
            &findings,
            VehicleClass::Gt3,
            false,
            8,
        );
        let bias = p
            .changes
            .iter()
            .find(|c| c.parameter_id == "brake.bias_front")
            .expect("bias change");
        assert_eq!(bias.direction, ChangeDirection::Decrease, "{bias:?}");
        assert_eq!(bias.rule_ids, vec!["US-ENTRY-SLOW-01".to_string()]);
        assert!(bias
            .conflicts_with
            .contains(&"BRK-LOCK-REAR-01".to_string()));
        assert_eq!(bias.parameter_key, "RearBrakeSetting");
        assert_eq!(bias.section, "CONTROLS");
        assert!(
            p.warnings_fr.iter().any(|w| w.contains("s'opposent")),
            "{:?}",
            p.warnings_fr
        );
        // Same-direction changes aggregate their rule ids.
        let arb = p
            .changes
            .iter()
            .find(|c| c.parameter_id == "susp.arb_front")
            .unwrap();
        assert_eq!(arb.rule_ids.len(), 2);
        assert_eq!(arb.steps, Some(1));
        // `check` becomes a warning, `toward_zero` is kept, hypercar-only rule is ignored.
        assert!(p.warnings_fr.iter().any(|w| w.contains("à vérifier")));
        assert!(p
            .changes
            .iter()
            .any(|c| c.direction == ChangeDirection::TowardZero));
        assert!(!p
            .changes
            .iter()
            .any(|c| c.parameter_id == "brake.migration"));
        // The base rule is there too, and the list follows the methodology order.
        assert!(p
            .changes
            .iter()
            .any(|c| c.rule_ids.contains(&"PHI-AGGRESSIVE-01".to_string())));
        let orders: Vec<u8> = p
            .changes
            .iter()
            .map(|c| group_order(&c.parameter_id))
            .collect();
        assert!(orders.windows(2).all(|w| w[0] <= w[1]), "{orders:?}");
        assert!(!p.warnings_fr.iter().any(|w| w.contains("Aucun diagnostic")));
    }

    #[test]
    fn safe_philosophy_prefers_the_safety_symptom() {
        let k = knowledge();
        let findings = vec![
            finding("US_ENTRY_SLOW", Severity::Minor, 0.7),
            finding("BRK_LOCK_REAR", Severity::Minor, 0.7),
        ];
        let p = plan(
            &k,
            SetupPhilosophy::Safe,
            &findings,
            VehicleClass::Gt3,
            false,
            8,
        );
        let bias = p
            .changes
            .iter()
            .find(|c| c.parameter_id == "brake.bias_front")
            .unwrap();
        assert_eq!(
            bias.direction,
            ChangeDirection::Increase,
            "safe keeps the braking rule"
        );
        assert!(p
            .changes
            .iter()
            .any(|c| c.rule_ids.contains(&"PHI-SAFE-01".to_string())));
        assert!(!p
            .changes
            .iter()
            .any(|c| c.rule_ids.contains(&"PHI-AGGRESSIVE-01".to_string())));
    }

    #[test]
    fn limit_and_generic_plans() {
        let k = knowledge();
        let p = plan(&k, SetupPhilosophy::Safe, &[], VehicleClass::Gt3, false, 2);
        assert_eq!(p.changes.len(), 2);
        assert!(p.warnings_fr[0].contains("Aucun diagnostic"));
        assert!(p
            .changes
            .iter()
            .all(|c| c.rationale_fr.contains("sans diagnostic")));
        assert!(p.changes.iter().all(|c| c.confidence < 0.5));
        // Info findings do not fire rules.
        let p2 = plan(
            &k,
            SetupPhilosophy::Aggressive,
            &[finding("US_ENTRY_SLOW", Severity::Info, 0.9)],
            VehicleClass::Gt3,
            false,
            8,
        );
        assert!(!p2
            .changes
            .iter()
            .any(|c| c.parameter_id == "brake.bias_front"));
        // Wet plan on a dry session: generic package, compound set to wet, warning.
        let plans = build_plans(&k, &[], VehicleClass::Hypercar, false, true, 8);
        assert!(plans.current_setup_known);
        let wet = &plans.wet;
        let compound = wet
            .changes
            .iter()
            .find(|c| c.parameter_id == "tyre.compound")
            .unwrap();
        assert_eq!(compound.direction, ChangeDirection::Set);
        assert_eq!(compound.magnitude, "wet");
        assert!(wet.warnings_fr.iter().any(|w| w.contains("Session sèche")));
        assert!(!wet
            .changes
            .iter()
            .any(|c| c.rule_ids.iter().any(|r| r.starts_with("PHI-"))));
        assert!(plans
            .aggressive
            .changes
            .iter()
            .any(|c| c.parameter_id == "elec.tc" && c.direction == ChangeDirection::Decrease));
        assert!(plans
            .safe
            .changes
            .iter()
            .any(|c| c.parameter_id == "elec.tc" && c.direction == ChangeDirection::Increase));
    }

    #[test]
    fn steps_parsing() {
        assert_eq!(parse_steps("2 clics"), Some(2));
        assert_eq!(parse_steps("+2 mm AR"), Some(2));
        assert_eq!(parse_steps("−1 mm AR"), Some(-1));
        assert_eq!(parse_steps("1 cran (GT3)"), Some(1));
        assert_eq!(parse_steps("0.5–1 %"), None);
        assert_eq!(parse_steps("1–2 crans"), None);
        assert_eq!(parse_steps("wet"), None);
        assert_eq!(parse_steps("3 kPa"), Some(3));
    }
}
