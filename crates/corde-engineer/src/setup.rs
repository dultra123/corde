//! Plans applied to the driver's real setup (`.svm`), and new setup files.
//!
//! The rule engine speaks in relative, physical terms ("rear bar −1 step",
//! "−3 kPa", "0.2–0.3° more camber"). This module turns that into the index
//! changes of a real file:
//!
//! 1. **Mapping** ([`mapping`]): logical parameter → `.svm` keys and sections
//!    (one per wheel for a per-wheel parameter), sign of the index
//!    (`svm.index_sign` of `knowledge/setup-parameters.yaml`: camber index ↑
//!    = less negative, brake balance index ↑ = more rear…), default physical
//!    step (`index_step`), translation of a parameter without key
//!    (`aero.rake` → rear ride height).
//! 2. **Labels** ([`LabelBook`]): the game reads the index only and writes the
//!    label after `//`. Pairs index ↔ label are collected from every file of
//!    the same car in the library (hand-written labels such as « +2 crans »
//!    are ignored); a label seen for an index is reused as is, else two or
//!    more numeric labels of the same shape give a linear law (interpolation,
//!    and the physical size of one step).
//! 3. **Resolution** ([`resolve_change`], [`resolve_plan`],
//!    [`resolve_report_plans`], [`resolve_iteration`]): magnitude → steps
//!    (learnt step, else the catalogue's step of confidence ≥ medium, else
//!    1 step and a note), bounds (minimum 0; maximum from the game's garage
//!    data when known, else at most +2 steps and a warning), parameters the
//!    car does not let change are left out with their reason.
//! 4. **Writing** ([`build_setup`]): the base file with the new indices and
//!    labels and new `Notes`, everything else identical to the byte.

use crate::rules::{self, IndexStep, Knowledge, MAX_CHANGES_PER_PLAN};
use corde_lmu::garage::{bound_for, GarageBounds};
use corde_lmu::svm::{is_adjustable, Setup, SetupNumber};
use corde_protocol::{
    AnalysisReport, ChangeDirection, PlanIteration, ResolveStatus, ResolvedChange, ResolvedValue,
    SetupChange, SetupFileInfo, SetupPlan, SetupPlans,
};
use std::collections::{BTreeMap, HashMap};

/// Without a known maximum, an increase is limited to this many steps.
pub const MAX_STEPS_WITHOUT_BOUND: i64 = 2;
/// Longest `Notes="…"` written by [`build_setup`].
pub const NOTES_MAX_CHARS: usize = 200;

// ---------------------------------------------------------------------------
// Mapping
// ---------------------------------------------------------------------------

/// One key of the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub section: String,
    pub key: String,
    /// `FL`, `FR`, `RL`, `RR` for the wheel sections.
    pub wheel: Option<&'static str>,
}

/// How a logical parameter lands in a `.svm` file.
#[derive(Debug, Clone)]
pub struct ParamMapping {
    /// The parameter of the plan (`aero.rake`).
    pub param_id: String,
    /// The parameter really changed (`aero.rh_rear` for the rake).
    pub applied_param: String,
    pub targets: Vec<Target>,
    /// +1 / −1, 0 = discrete or unknown (not changed by Corde).
    pub index_sign: i32,
    pub rest_key: Option<String>,
    pub index_step: Option<IndexStep>,
    /// Why the parameter is applied through another one.
    pub translation_note: Option<String>,
    /// The label is the index itself (TC maps).
    pub label_is_index: bool,
}

fn wheel_of(section: &str) -> Option<&'static str> {
    match section {
        "FRONTLEFT" => Some("FL"),
        "FRONTRIGHT" => Some("FR"),
        "REARLEFT" => Some("RL"),
        "REARRIGHT" => Some("RR"),
        _ => None,
    }
}

/// `"FRONTLEFT / FRONTRIGHT"` → both; `"FRONTLEFT … REARRIGHT"` → the four wheels.
fn split_sections(s: &str) -> Vec<String> {
    if s.contains('…') && s.trim_start().starts_with("FRONTLEFT") {
        return ["FRONTLEFT", "FRONTRIGHT", "REARLEFT", "REARRIGHT"]
            .iter()
            .map(|x| x.to_string())
            .collect();
    }
    s.split('/')
        .map(str::trim)
        .filter(|x| !x.is_empty() && !x.contains('…'))
        .map(str::to_string)
        .collect()
}

fn split_keys(s: &str) -> Vec<String> {
    s.split(['/', ','])
        .map(str::trim)
        .filter(|x| !x.is_empty() && !x.contains(".."))
        .map(str::to_string)
        .collect()
}

/// The `.svm` mapping of a parameter (`None`: no key and no translation).
pub fn mapping(k: &Knowledge, param_id: &str) -> Option<ParamMapping> {
    let entry = k.params.get(param_id)?;
    if let Some(tr) = &entry.svm_translation {
        let mut m = mapping(k, &tr.param)?;
        m.param_id = param_id.to_string();
        m.translation_note = Some(tr.note.clone());
        return Some(m);
    }
    let svm = entry.svm.as_ref()?;
    if svm.key.is_empty() {
        return None;
    }
    let mut targets = Vec::new();
    for section in split_sections(&svm.section) {
        for key in split_keys(&svm.key) {
            targets.push(Target {
                wheel: wheel_of(&section),
                section: section.clone(),
                key,
            });
        }
    }
    if targets.is_empty() {
        return None;
    }
    Some(ParamMapping {
        param_id: param_id.to_string(),
        applied_param: param_id.to_string(),
        targets,
        index_sign: svm.index_sign.signum(),
        rest_key: entry.rest_key.clone(),
        index_step: entry.index_step.clone(),
        translation_note: None,
        label_is_index: svm.label_is_index,
    })
}

// ---------------------------------------------------------------------------
// Labels
// ---------------------------------------------------------------------------

/// `true` for a label the driver wrote himself (« +2 crans », « -1 cran AV »):
/// it says nothing about the value.
pub fn is_hand_label(label: &str) -> bool {
    let l = label.to_lowercase();
    ["cran", "clic", "click", "step", "corde"]
        .iter()
        .any(|w| l.contains(w))
}

/// Shape of a numeric label, to write another value the same way.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct LabelShape {
    prefix: String,
    sep: String,
    suffix: String,
    decimals: usize,
    /// `51.5:48.5` (front:rear, the two parts add up to 100).
    ratio: bool,
}

impl LabelShape {
    fn same_kind(&self, other: &LabelShape) -> bool {
        self.prefix == other.prefix && self.suffix == other.suffix && self.ratio == other.ratio
    }

    fn render(&self, v: f64, decimals: usize) -> String {
        let fmt = |x: f64| {
            let s = format!("{x:.decimals$}");
            if s.starts_with('-') && s.trim_start_matches(['-', '0', '.']).is_empty() {
                s[1..].to_string() // no "-0.00"
            } else {
                s
            }
        };
        if self.ratio {
            format!("{}:{}", fmt(v), fmt(100.0 - v))
        } else {
            format!("{}{}{}{}", self.prefix, fmt(v), self.sep, self.suffix)
        }
    }

    /// The unit the numbers are in (`deg`, `kPa`, `%`…; `""` = none).
    fn unit(&self) -> &str {
        if self.ratio {
            "%"
        } else {
            self.suffix.as_str()
        }
    }
}

const LABEL_UNITS: &[&str] = &[
    "deg", "°", "cm", "mm", "kPa", "psi", "bar", "%", "Nm", "N/mm", "lbf/in", "kgf", "kW",
];

fn parse_number(s: &str) -> Option<(f64, usize)> {
    let t = s.trim();
    if t.is_empty()
        || !t
            .chars()
            .all(|c| c.is_ascii_digit() || matches!(c, '.' | '-' | '+'))
    {
        return None;
    }
    let v: f64 = t.parse().ok()?;
    let decimals = t.split_once('.').map(|(_, d)| d.len()).unwrap_or(0);
    Some((v, decimals))
}

/// `-2.20 deg` → (−2.2, shape) ; `P2` ; `51.5:48.5` ; `136 kPa` ; `3`.
/// Labels with words or several numbers (`120 kgf (100%)`, `1 (tendre)`)
/// are refused.
fn parse_label(label: &str) -> Option<(f64, LabelShape)> {
    let l = label.trim();
    if l.is_empty() || is_hand_label(l) {
        return None;
    }
    if let Some((a, b)) = l.split_once(':') {
        let (va, da) = parse_number(a)?;
        let (vb, _) = parse_number(b)?;
        if (va + vb - 100.0).abs() < 0.2 {
            return Some((
                va,
                LabelShape {
                    prefix: String::new(),
                    sep: String::new(),
                    suffix: String::new(),
                    decimals: da,
                    ratio: true,
                },
            ));
        }
        return None;
    }
    let (prefix, rest) = match l.strip_prefix('P') {
        Some(r) if r.starts_with(|c: char| c.is_ascii_digit()) => ("P", r),
        _ => ("", l),
    };
    let num_len = rest
        .char_indices()
        .take_while(|(i, c)| c.is_ascii_digit() || *c == '.' || (*i == 0 && matches!(c, '-' | '+')))
        .count();
    let (num, tail) = rest.split_at(num_len);
    let (v, decimals) = parse_number(num)?;
    let sep: String = tail.chars().take_while(|c| *c == ' ').collect();
    let suffix = tail[sep.len()..].to_string();
    if !(suffix.is_empty() || LABEL_UNITS.contains(&suffix.as_str())) {
        return None;
    }
    if !prefix.is_empty() && !suffix.is_empty() {
        return None;
    }
    Some((
        v,
        LabelShape {
            prefix: prefix.to_string(),
            sep,
            suffix,
            decimals,
            ratio: false,
        },
    ))
}

/// What the library says about one key of one car.
#[derive(Debug, Clone, Default)]
pub struct KeyLabels {
    /// Index → label seen in a file (the most frequent one).
    exact: BTreeMap<i64, String>,
    /// Linear law `value = a + b × index` with the shape to write it, when
    /// two or more numeric labels agree.
    law: Option<(f64, f64, LabelShape, usize)>,
    /// Indices the law was learnt from.
    learnt_from: Vec<(i64, f64)>,
}

impl KeyLabels {
    fn from_pairs(pairs: &[(i64, String)]) -> Self {
        let mut counts: BTreeMap<i64, HashMap<&str, usize>> = BTreeMap::new();
        for (i, l) in pairs {
            *counts.entry(*i).or_default().entry(l.as_str()).or_default() += 1;
        }
        let exact = counts
            .iter()
            .map(|(i, c)| {
                let best = c
                    .iter()
                    .max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0)))
                    .map(|(l, _)| l.to_string())
                    .unwrap_or_default();
                (*i, best)
            })
            .collect();
        // Numeric labels, grouped by shape; the largest group wins.
        let mut groups: Vec<(LabelShape, BTreeMap<i64, f64>, usize)> = Vec::new();
        for (i, l) in pairs {
            let Some((v, shape)) = parse_label(l) else {
                continue;
            };
            match groups.iter_mut().find(|(s, ..)| s.same_kind(&shape)) {
                Some((_, pts, dec)) => {
                    pts.entry(*i).or_insert(v);
                    *dec = (*dec).max(shape.decimals);
                }
                None => {
                    let dec = shape.decimals;
                    groups.push((shape, BTreeMap::from([(*i, v)]), dec));
                }
            }
        }
        groups.sort_by_key(|g| std::cmp::Reverse(g.1.len()));
        let mut law = None;
        let mut learnt_from = Vec::new();
        if let Some((shape, pts, decimals)) = groups.into_iter().next() {
            if pts.len() >= 2 {
                let n = pts.len() as f64;
                let mx = pts.keys().map(|i| *i as f64).sum::<f64>() / n;
                let my = pts.values().sum::<f64>() / n;
                let sxx: f64 = pts.keys().map(|i| (*i as f64 - mx).powi(2)).sum();
                let sxy: f64 = pts.iter().map(|(i, v)| (*i as f64 - mx) * (v - my)).sum();
                if sxx > 0.0 {
                    let b = sxy / sxx;
                    let a = my - b * mx;
                    let tol = 0.2 * b.abs() + 0.6 * 10f64.powi(-(decimals as i32));
                    let linear = pts
                        .iter()
                        .all(|(i, v)| (a + b * *i as f64 - v).abs() <= tol);
                    if b.abs() > 1e-9 && linear {
                        learnt_from = pts.iter().map(|(i, v)| (*i, *v)).collect();
                        law = Some((a, b, shape, decimals));
                    }
                }
            }
        }
        Self {
            exact,
            law,
            learnt_from,
        }
    }

    /// Label of an index: one seen in a file, else from the linear law
    /// (beyond the indices it was learnt from by at most their span, and at
    /// least 3).
    pub fn label_for(&self, index: i64) -> Option<String> {
        if let Some(l) = self.exact.get(&index) {
            return Some(l.clone());
        }
        let (a, b, shape, decimals) = self.law.as_ref()?;
        let first = self.learnt_from.first()?.0;
        let last = self.learnt_from.last()?.0;
        let margin = (last - first).max(3);
        let lo = first - margin;
        let hi = last + margin;
        if index < lo || index > hi {
            return None;
        }
        let v = a + b * index as f64;
        if shape.unit() == "%" && !(0.0..=100.0).contains(&v) {
            return None;
        }
        Some(shape.render(v, *decimals))
    }

    /// Numeric value of an index (from its label or the law).
    pub fn value_at(&self, index: i64) -> Option<f64> {
        if let Some((v, _)) = self.exact.get(&index).and_then(|l| parse_label(l)) {
            return Some(v);
        }
        let (a, b, ..) = self.law.as_ref()?;
        Some(a + b * index as f64)
    }

    /// Physical size of one step and its unit (from the law).
    pub fn step(&self) -> Option<(f64, String)> {
        let (_, b, shape, _) = self.law.as_ref()?;
        Some((*b, shape.unit().to_string()))
    }

    /// Pairs the law was learnt from (index, value).
    pub fn learnt_from(&self) -> &[(i64, f64)] {
        &self.learnt_from
    }
}

/// Axle group of a section (front wheels share their tables, rear ones too).
fn label_group(section: &str) -> &str {
    match section {
        "FRONTLEFT" | "FRONTRIGHT" => "FRONT",
        "REARLEFT" | "REARRIGHT" => "REAR",
        other => other,
    }
}

/// Index ↔ label knowledge for one car, from the files of its library.
#[derive(Debug, Clone, Default)]
pub struct LabelBook {
    keys: HashMap<(String, String), KeyLabels>,
}

impl LabelBook {
    /// `setups` must all be for the same car (same `VehicleClassSetting`).
    pub fn learn(setups: &[&Setup]) -> Self {
        let mut pairs: HashMap<(String, String), Vec<(i64, String)>> = HashMap::new();
        for s in setups {
            for (section, key, value) in s.values() {
                let SetupNumber::Int(i) = value.index else {
                    continue;
                };
                let label = value.label();
                if label.is_empty() || is_hand_label(label) || !is_adjustable(value) {
                    continue;
                }
                pairs
                    .entry((label_group(section).to_string(), key.to_string()))
                    .or_default()
                    .push((i, label.to_string()));
            }
        }
        Self {
            keys: pairs
                .into_iter()
                .map(|(k, v)| (k, KeyLabels::from_pairs(&v)))
                .collect(),
        }
    }

    pub fn key(&self, section: &str, key: &str) -> Option<&KeyLabels> {
        self.keys
            .get(&(label_group(section).to_string(), key.to_string()))
    }
}

// ---------------------------------------------------------------------------
// Magnitudes
// ---------------------------------------------------------------------------

/// A rule magnitude: lower bound of the range, in a normalised unit.
#[derive(Debug, Clone, PartialEq)]
struct Magnitude {
    value: f64,
    /// `steps`, `mm`, `deg`, `kPa`, `%`, `Nm`, `kW`, or the raw word.
    unit: String,
}

/// Normalised unit and the factor to it (`cm` → (`mm`, 10)).
fn norm_unit(u: &str) -> (String, f64) {
    let l = u.trim().to_lowercase();
    match l.as_str() {
        "" | "cran" | "crans" | "clic" | "clics" | "click" | "clicks" | "pas" | "step"
        | "steps" | "map" | "niveau" => ("steps".into(), 1.0),
        "mm" => ("mm".into(), 1.0),
        "cm" => ("mm".into(), 10.0),
        "°" | "deg" | "degré" | "degrés" | "degre" | "degres" => ("deg".into(), 1.0),
        "kpa" => ("kPa".into(), 1.0),
        "psi" => ("kPa".into(), 6.894_757),
        "bar" => ("kPa".into(), 100.0),
        "%" => ("%".into(), 1.0),
        "nm" => ("Nm".into(), 1.0),
        "kw" => ("kW".into(), 1.0),
        "n/mm" => ("N/mm".into(), 1.0),
        "lbf/in" => ("lbf/in".into(), 1.0),
        "kgf" => ("kgf".into(), 1.0),
        other => (other.to_string(), 1.0),
    }
}

/// `"0.2–0.3°"` → 0.2 deg ; `"−3 kPa"` → 3 kPa ; `"1 cran"` → 1 step ;
/// `"±2 mm"` → 2 mm ; `"0,5 %"` → 0.5 % ; `"wet (…)"` → `None`.
fn parse_magnitude(m: &str) -> Option<Magnitude> {
    let s = m
        .trim()
        .trim_start_matches(['±', '+', '−', '-', '~', '≈'])
        .trim();
    let num: String = s
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == ',')
        .collect();
    if num.is_empty() {
        return None;
    }
    let value: f64 = num.replace(',', ".").parse().ok()?;
    let mut rest = s[num.len()..].trim_start();
    // Range: skip the upper bound.
    if let Some(r) = rest.strip_prefix('–').or_else(|| rest.strip_prefix('-')) {
        rest = r
            .trim_start()
            .trim_start_matches(|c: char| c.is_ascii_digit() || c == '.' || c == ',')
            .trim_start();
    }
    let word: String = if rest.starts_with('°') || rest.starts_with('%') {
        rest.chars().take(1).collect()
    } else {
        rest.chars()
            .take_while(|c| c.is_alphanumeric() || *c == '/' || *c == '°')
            .collect()
    };
    let (unit, factor) = norm_unit(&word);
    Some(Magnitude {
        value: value * factor,
        unit,
    })
}

// ---------------------------------------------------------------------------
// Resolution
// ---------------------------------------------------------------------------

/// What a plan is resolved against.
#[derive(Debug, Clone, Copy)]
pub struct ResolveInput<'a> {
    /// The base setup.
    pub base: &'a Setup,
    /// Every setup of the same car in the library (the base may be one of them).
    pub library: &'a [&'a Setup],
    /// Bounds read in the game's garage for this car, when known.
    pub bounds: Option<&'a GarageBounds>,
}

fn status_rank(s: ResolveStatus) -> u8 {
    match s {
        ResolveStatus::Applied => 0,
        ResolveStatus::UnknownBounds => 1,
        ResolveStatus::ClampedMin => 2,
        ResolveStatus::ClampedMax => 3,
        ResolveStatus::Manual => 4,
        ResolveStatus::Duplicate => 5,
        ResolveStatus::UnknownKey => 6,
        ResolveStatus::NotAdjustable => 7,
    }
}

/// `true` for a status whose change stays in the plan (and in the file).
pub fn is_kept(s: ResolveStatus) -> bool {
    !matches!(
        s,
        ResolveStatus::NotAdjustable | ResolveStatus::UnknownKey | ResolveStatus::Duplicate
    )
}

/// How many index steps a change asks for, with the notes explaining it.
fn steps_for(
    change: &SetupChange,
    m: &ParamMapping,
    labels: Option<&KeyLabels>,
    fr: &mut Vec<String>,
    en: &mut Vec<String>,
) -> i64 {
    let Some(mag) = parse_magnitude(&change.magnitude) else {
        fr.push(format!(
            "Magnitude « {} » non chiffrée : 1 cran.",
            change.magnitude
        ));
        en.push(format!(
            "Magnitude \"{}\" has no number: 1 step.",
            change.magnitude
        ));
        return 1;
    };
    if mag.unit == "steps" {
        return (mag.value.round() as i64).max(1);
    }
    // Learnt on the driver's own files.
    if let Some((step, unit)) = labels.and_then(KeyLabels::step) {
        let (u, factor) = norm_unit(&unit);
        if u == mag.unit {
            let per_step = (step * factor).abs();
            let n = ((mag.value / per_step).round() as i64).max(1);
            fr.push(format!(
                "{} → {n} cran{} (pas appris sur tes fichiers : {} {} par cran).",
                change.magnitude.trim(),
                plural(n),
                fmt_num(per_step),
                mag.unit
            ));
            en.push(format!(
                "{} → {n} step{} (step learnt from your files: {} {} per step).",
                change.magnitude.trim(),
                if n > 1 { "s" } else { "" },
                fmt_num(per_step),
                mag.unit
            ));
            return n;
        }
    }
    // The catalogue's default, when it is trusted.
    if let Some(st) = m
        .index_step
        .as_ref()
        .filter(|s| s.confidence == "medium" || s.confidence == "high")
    {
        let (u, factor) = norm_unit(&st.unit);
        if u == mag.unit && st.value > 0.0 {
            let per_step = st.value * factor;
            let n = ((mag.value / per_step).round() as i64).max(1);
            fr.push(format!(
                "{} → {n} cran{} (pas supposé : {} {} par cran, d'après {}).",
                change.magnitude.trim(),
                plural(n),
                fmt_num(per_step),
                mag.unit,
                st.source
            ));
            en.push(format!(
                "{} → {n} step{} (assumed step: {} {} per step).",
                change.magnitude.trim(),
                if n > 1 { "s" } else { "" },
                fmt_num(per_step),
                mag.unit
            ));
            return n;
        }
    }
    fr.push(format!(
        "Pas de ce réglage inconnu pour cette voiture : « {} » appliqué comme 1 cran.",
        change.magnitude.trim()
    ));
    en.push(format!(
        "Step size unknown for this car: \"{}\" applied as 1 step.",
        change.magnitude.trim()
    ));
    1
}

fn plural(n: i64) -> &'static str {
    if n.abs() > 1 {
        "s"
    } else {
        ""
    }
}

fn fmt_num(v: f64) -> String {
    let s = format!("{v:.4}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    s.to_string()
}

/// Resolve one change against the base. Always returns the change with
/// `resolved` filled in; [`is_kept`] tells whether it stays in the plan.
pub fn resolve_change(k: &Knowledge, change: &SetupChange, input: &ResolveInput) -> SetupChange {
    let mut out = change.clone();
    let mut fr: Vec<String> = Vec::new();
    let mut en: Vec<String> = Vec::new();
    let finish = |out: &mut SetupChange, status, steps, values, fr, en| {
        out.resolved = Some(ResolvedChange {
            status,
            steps,
            values,
            notes_fr: fr,
            notes_en: en,
        });
    };
    let Some(m) = mapping(k, &change.parameter_id) else {
        fr.push(format!(
            "« {} » n'existe pas dans un fichier de réglage : à faire à la main ou à ignorer.",
            change.label_fr
        ));
        en.push(format!(
            "\"{}\" is not a setting of the setup file.",
            change.parameter_id
        ));
        finish(&mut out, ResolveStatus::UnknownKey, 0, Vec::new(), fr, en);
        return out;
    };
    if let Some(note) = &m.translation_note {
        fr.push(note.clone());
        en.push(format!(
            "{} has no key in the file: applied through {}.",
            m.param_id, m.applied_param
        ));
    }
    if let Some(first) = m.targets.first() {
        out.parameter_key = first.key.clone();
        out.section = m
            .targets
            .iter()
            .map(|t| t.section.as_str())
            .collect::<Vec<_>>()
            .join(" / ");
    }

    // Which keys exist and can be changed.
    let mut present = Vec::new();
    let mut missing = 0;
    let mut fixed: Vec<String> = Vec::new();
    for t in &m.targets {
        match input.base.get(&t.section, &t.key) {
            None => missing += 1,
            Some(v) => {
                let bound = input
                    .bounds
                    .and_then(|b| bound_for(b, &t.key, m.rest_key.as_deref()));
                if !is_adjustable(v) || bound.is_some_and(|b| b.is_fixed()) {
                    fixed.push(format!("{} = « {} »", t.key, v.label()));
                } else if v.as_i64().is_none() {
                    fixed.push(format!("{} (valeur non numérique)", t.key));
                } else {
                    present.push((t, v));
                }
            }
        }
    }
    if present.is_empty() {
        let status = if !fixed.is_empty() {
            fr.push(format!(
                "Non réglable sur cette voiture ({}) : écarté.",
                fixed.join(", ")
            ));
            en.push("Not adjustable on this car: left out.".to_string());
            ResolveStatus::NotAdjustable
        } else {
            fr.push(format!(
                "Clé {} absente du fichier : écarté.",
                m.targets
                    .first()
                    .map(|t| t.key.as_str())
                    .unwrap_or_default()
            ));
            en.push("Key absent from the file: left out.".to_string());
            ResolveStatus::UnknownKey
        };
        finish(&mut out, status, 0, Vec::new(), fr, en);
        return out;
    }
    if !fixed.is_empty() || missing > 0 {
        fr.push(format!(
            "Seulement {} clé(s) réglable(s) sur {}.",
            present.len(),
            m.targets.len()
        ));
        en.push(format!(
            "Only {} of {} keys can be changed.",
            present.len(),
            m.targets.len()
        ));
    }

    let book = LabelBook::learn(input.library);
    // Discrete values and unknown signs: by hand.
    let manual = match change.direction {
        ChangeDirection::Set => Some((
            format!(
                "Valeur à choisir à la main dans le garage (« {} »).",
                change.magnitude
            ),
            "Discrete value: to choose by hand in the garage.".to_string(),
        )),
        _ if m.index_sign == 0 => Some((
            "Sens de l'index inconnu pour ce réglage : à faire à la main.".to_string(),
            "Index direction unknown for this setting: by hand.".to_string(),
        )),
        _ => None,
    };
    let mut values = Vec::new();
    if let Some((f, e)) = manual {
        fr.push(f);
        en.push(e);
        for (t, v) in &present {
            let i = v.as_i64().unwrap_or(0) as i32;
            values.push(ResolvedValue {
                section: t.section.clone(),
                key: t.key.clone(),
                wheel: t.wheel.map(str::to_string),
                index_before: i,
                index_after: i,
                label_before: v.label().to_string(),
                label_after: None,
                min: Some(0),
                max: None,
                status: ResolveStatus::Manual,
            });
        }
        finish(&mut out, ResolveStatus::Manual, 0, values, fr, en);
        return out;
    }

    let first_labels = present
        .first()
        .and_then(|(t, _)| book.key(&t.section, &t.key));
    let n = steps_for(change, &m, first_labels, &mut fr, &mut en);
    let mut signed_steps = 0i64;
    let mut worst = ResolveStatus::Applied;
    let mut said_unknown_max = false;
    for (t, v) in &present {
        let before = v.as_i64().unwrap_or(0);
        let labels = book.key(&t.section, &t.key);
        let delta = match change.direction {
            ChangeDirection::Increase => n * m.index_sign as i64,
            ChangeDirection::Decrease => -n * m.index_sign as i64,
            ChangeDirection::TowardZero => {
                let value = labels.and_then(|l| l.value_at(before));
                let slope = labels
                    .and_then(KeyLabels::step)
                    .map(|(b, _)| b.signum())
                    .unwrap_or(m.index_sign as f64);
                match value {
                    Some(x) if x.abs() < 1e-6 => 0,
                    Some(x) => {
                        let dir = if (x > 0.0) == (slope > 0.0) { -1 } else { 1 };
                        // Do not go past zero.
                        let mut steps = n;
                        if let Some((a, b, ..)) = labels.and_then(|l| l.law.as_ref()) {
                            let zero = (-a / b).round() as i64;
                            steps = steps.min((zero - before).abs()).max(1);
                        }
                        dir * steps
                    }
                    None => {
                        fr.push(format!(
                            "{} : valeur actuelle inconnue (libellé « {} ») : sens « vers zéro » impossible à calculer.",
                            t.key,
                            v.label()
                        ));
                        en.push(format!("{}: current value unknown.", t.key));
                        worst = worst_of(worst, ResolveStatus::Manual);
                        values.push(ResolvedValue {
                            section: t.section.clone(),
                            key: t.key.clone(),
                            wheel: t.wheel.map(str::to_string),
                            index_before: before as i32,
                            index_after: before as i32,
                            label_before: v.label().to_string(),
                            label_after: None,
                            min: Some(0),
                            max: None,
                            status: ResolveStatus::Manual,
                        });
                        continue;
                    }
                }
            }
            ChangeDirection::Set => 0,
        };
        signed_steps = delta;
        let bound = input
            .bounds
            .and_then(|b| bound_for(b, &t.key, m.rest_key.as_deref()));
        let min = bound
            .and_then(|b| b.min)
            .map(|x| x.round() as i64)
            .unwrap_or(0)
            .max(0);
        let max = bound.and_then(|b| b.max).map(|x| x.round() as i64);
        let mut status = ResolveStatus::Applied;
        let mut after = before + delta;
        if after < min {
            after = min;
            status = ResolveStatus::ClampedMin;
        }
        match max {
            Some(mx) if after > mx => {
                after = mx.max(min);
                status = ResolveStatus::ClampedMax;
            }
            Some(_) => {}
            None if delta > 0 => {
                if delta > MAX_STEPS_WITHOUT_BOUND {
                    after = before + MAX_STEPS_WITHOUT_BOUND;
                }
                status = ResolveStatus::UnknownBounds;
                if !said_unknown_max {
                    said_unknown_max = true;
                    fr.push(format!(
                        "Maximum inconnu (pas lu dans le garage du jeu) : au plus +{MAX_STEPS_WITHOUT_BOUND} crans, à vérifier dans le garage."
                    ));
                    en.push(format!(
                        "Maximum unknown: at most +{MAX_STEPS_WITHOUT_BOUND} steps, check in the garage."
                    ));
                }
            }
            None => {}
        }
        if delta != 0 && after == before {
            fr.push(format!(
                "{}{} déjà à la limite ({}) : rien à changer.",
                t.key,
                t.wheel.map(|w| format!(" {w}")).unwrap_or_default(),
                before
            ));
            en.push(format!("{} already at its limit.", t.key));
        } else if status == ResolveStatus::ClampedMin || status == ResolveStatus::ClampedMax {
            fr.push(format!(
                "{} arrêté à la limite {} (index {after}).",
                t.key,
                if status == ResolveStatus::ClampedMin {
                    "basse"
                } else {
                    "haute"
                }
            ));
            en.push(format!("{} stopped at its limit (index {after}).", t.key));
        }
        if delta == 0 && change.direction == ChangeDirection::TowardZero {
            fr.push(format!("{} déjà à zéro.", t.key));
            en.push(format!("{} already at zero.", t.key));
        }
        worst = worst_of(worst, status);
        let label_before = v.label().to_string();
        if is_hand_label(&label_before) {
            fr.push(format!(
                "Libellé « {label_before} » écrit à la main dans le fichier : le jeu ne lit que l'index ({before})."
            ));
            en.push(format!(
                "Label \"{label_before}\" was written by hand: the game reads the index only ({before})."
            ));
        }
        let label_after = if after == before {
            Some(label_before.clone()).filter(|l| !is_hand_label(l))
        } else {
            labels.and_then(|l| l.label_for(after)).or_else(|| {
                // TC maps: the label is the index, when this file confirms it.
                (m.label_is_index && label_before.trim() == before.to_string())
                    .then(|| after.to_string())
            })
        };
        values.push(ResolvedValue {
            section: t.section.clone(),
            key: t.key.clone(),
            wheel: t.wheel.map(str::to_string),
            index_before: before as i32,
            index_after: after as i32,
            label_before,
            label_after,
            min: Some(min as i32),
            max: max.map(|x| x as i32),
            status,
        });
    }
    fr.dedup();
    en.dedup();
    finish(&mut out, worst, signed_steps as i32, values, fr, en);
    out
}

fn worst_of(a: ResolveStatus, b: ResolveStatus) -> ResolveStatus {
    if status_rank(b) > status_rank(a) {
        b
    } else {
        a
    }
}

/// Parameters of the catalogue that cannot move on the base (every key
/// absent or not adjustable), translations included.
pub fn unavailable_params(k: &Knowledge, input: &ResolveInput) -> Vec<String> {
    let mut out = Vec::new();
    for p in &k.params.parameters {
        let Some(m) = mapping(k, &p.id) else {
            continue;
        };
        let usable = m.targets.iter().any(|t| {
            input.base.get(&t.section, &t.key).is_some_and(|v| {
                let bound = input
                    .bounds
                    .and_then(|b| bound_for(b, &t.key, m.rest_key.as_deref()));
                is_adjustable(v) && v.as_i64().is_some() && !bound.is_some_and(|b| b.is_fixed())
            })
        });
        if !usable {
            out.push(p.id.clone());
        }
    }
    out
}

/// Resolve a list of changes: `(kept, dropped)`. A second change on keys
/// already changed by an earlier one is a duplicate.
pub fn resolve_changes(
    k: &Knowledge,
    changes: &[SetupChange],
    input: &ResolveInput,
) -> (Vec<SetupChange>, Vec<SetupChange>) {
    let mut kept: Vec<SetupChange> = Vec::new();
    let mut dropped = Vec::new();
    for c in changes {
        let mut r = resolve_change(k, c, input);
        let status = r.resolved.as_ref().map(|x| x.status).unwrap_or_default();
        if is_kept(status) {
            let keys: Vec<(String, String)> = r
                .resolved
                .iter()
                .flat_map(|x| x.values.iter())
                .map(|v| (v.section.clone(), v.key.clone()))
                .collect();
            let clash = kept.iter().find(|o| {
                o.resolved
                    .iter()
                    .flat_map(|x| x.values.iter())
                    .any(|v| keys.iter().any(|(s, kk)| *s == v.section && *kk == v.key))
            });
            if let Some(other) = clash {
                if let Some(res) = r.resolved.as_mut() {
                    res.status = ResolveStatus::Duplicate;
                    res.notes_fr.push(format!(
                        "Même clé que « {} », déjà dans le plan : non appliqué deux fois.",
                        other.label_fr
                    ));
                    res.notes_en
                        .push("Same key as another change of the plan: not applied twice.".into());
                }
                dropped.push(r);
                continue;
            }
            kept.push(r);
        } else {
            dropped.push(r);
        }
    }
    (kept, dropped)
}

fn dropped_warning(c: &SetupChange) -> String {
    let why = c
        .resolved
        .as_ref()
        .and_then(|r| r.notes_fr.last().cloned())
        .unwrap_or_default();
    format!("{} écarté : {why}", c.label_fr)
}

/// Resolve one plan (its non-adjustable changes go to `dropped`, with a
/// warning each).
pub fn resolve_plan(k: &Knowledge, plan: &SetupPlan, input: &ResolveInput) -> SetupPlan {
    let (changes, dropped) = resolve_changes(k, &plan.changes, input);
    let mut warnings = plan.warnings_fr.clone();
    for d in &dropped {
        warnings.push(dropped_warning(d));
    }
    let mut dropped_all = plan.dropped.clone();
    dropped_all.extend(dropped);
    SetupPlan {
        philosophy: plan.philosophy,
        intent_fr: plan.intent_fr.clone(),
        changes,
        warnings_fr: warnings,
        dropped: dropped_all,
    }
}

/// The report's three plans, built again without the parameters the car
/// does not let change (the next best levers take their place), then
/// resolved against the base. `dropped` lists the changes of the original
/// plans that were left out, with their reason.
pub fn resolve_report_plans(
    k: &Knowledge,
    report: &AnalysisReport,
    input: &ResolveInput,
    base: Option<SetupFileInfo>,
) -> SetupPlans {
    let unavailable = unavailable_params(k, input);
    let excluded = |p: &str| unavailable.iter().any(|u| u == p);
    let replanned = rules::build_plans_excluding(
        k,
        &report.findings,
        report.vehicle_class,
        report.data_quality.wet_session,
        MAX_CHANGES_PER_PLAN,
        &excluded,
    );
    let one = |original: &SetupPlan, new: &SetupPlan| {
        let mut plan = resolve_plan(k, new, input);
        let mut extra = Vec::new();
        for c in original
            .changes
            .iter()
            .filter(|c| excluded(&c.parameter_id))
        {
            let r = resolve_change(k, c, input);
            plan.warnings_fr.push(format!(
                "{} — remplacé par le levier suivant.",
                dropped_warning(&r)
            ));
            extra.push(r);
        }
        extra.extend(plan.dropped);
        plan.dropped = extra;
        plan
    };
    SetupPlans {
        current_setup_known: true,
        base,
        aggressive: one(&report.plans.aggressive, &replanned.aggressive),
        safe: one(&report.plans.safe, &replanned.safe),
        wet: one(&report.plans.wet, &replanned.wet),
    }
}

/// An iteration (stored relative) resolved against the base: `changes`
/// (cumulative) and `delta` carry their indices, the non-adjustable ones go
/// to `dropped` with a note.
pub fn resolve_iteration(k: &Knowledge, it: &PlanIteration, input: &ResolveInput) -> PlanIteration {
    let mut out = it.clone();
    let (changes, dropped) = resolve_changes(k, &it.changes, input);
    let (delta, delta_dropped) = resolve_changes(k, &it.delta, input);
    for d in dropped.iter().chain(delta_dropped.iter()) {
        let w = dropped_warning(d);
        if !out.notes_fr.contains(&w) {
            out.notes_fr.push(w);
        }
    }
    let mut all_dropped = dropped;
    for d in delta_dropped {
        if !all_dropped.iter().any(|x| x.parameter_id == d.parameter_id) {
            all_dropped.push(d);
        }
    }
    out.changes = changes;
    out.delta = delta;
    out.dropped = all_dropped;
    out.current_setup_known = true;
    out
}

// ---------------------------------------------------------------------------
// Writing
// ---------------------------------------------------------------------------

/// Printable ASCII only, accents folded, quotes removed.
pub fn ascii_safe(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        let mapped = match c {
            'à' | 'á' | 'â' | 'ä' | 'ã' => "a",
            'À' | 'Á' | 'Â' | 'Ä' => "A",
            'ç' => "c",
            'Ç' => "C",
            'è' | 'é' | 'ê' | 'ë' => "e",
            'È' | 'É' | 'Ê' | 'Ë' => "E",
            'ì' | 'í' | 'î' | 'ï' => "i",
            'ò' | 'ó' | 'ô' | 'ö' | 'õ' => "o",
            'ù' | 'ú' | 'û' | 'ü' => "u",
            'ñ' => "n",
            '—' | '–' | '−' => "-",
            '→' => "->",
            '▲' => "+",
            '▼' => "-",
            '&' => "et",
            '"' | '\\' => "",
            c if c.is_ascii_graphic() || c == ' ' => {
                out.push(c);
                continue;
            }
            _ => "",
        };
        out.push_str(mapped);
    }
    out
}

/// Short name of a change for the notes (`RearAntiSway -1`, `Camber AV -3`).
fn short_change(c: &SetupChange) -> Option<String> {
    let r = c.resolved.as_ref()?;
    let moved: Vec<&ResolvedValue> = r
        .values
        .iter()
        .filter(|v| v.index_after != v.index_before)
        .collect();
    let first = moved.first()?;
    let d = first.index_after - first.index_before;
    let key = first.key.trim_end_matches("Setting");
    let axle = match first.wheel.as_deref() {
        Some("FL" | "FR")
            if moved
                .iter()
                .all(|v| matches!(v.wheel.as_deref(), Some("FL" | "FR"))) =>
        {
            " AV"
        }
        Some("RL" | "RR")
            if moved
                .iter()
                .all(|v| matches!(v.wheel.as_deref(), Some("RL" | "RR"))) =>
        {
            " AR"
        }
        _ => "",
    };
    Some(format!("{key}{axle} {d:+}"))
}

/// `Corde - <plan> - base : <file> - <changes>`, ASCII, at most
/// [`NOTES_MAX_CHARS`] characters.
pub fn notes_for(plan_label: &str, base_file: &str, changes: &[SetupChange]) -> String {
    let parts: Vec<String> = changes.iter().filter_map(short_change).collect();
    let summary = if parts.is_empty() {
        "aucun changement".to_string()
    } else {
        parts.join(", ")
    };
    let text = ascii_safe(&format!(
        "Corde - {plan_label} - base : {base_file} - {summary}"
    ));
    if text.chars().count() <= NOTES_MAX_CHARS {
        text
    } else {
        let cut: String = text.chars().take(NOTES_MAX_CHARS - 3).collect();
        format!("{}...", cut.trim_end())
    }
}

/// Label written after `//` for a changed value.
fn written_label(v: &ResolvedValue) -> String {
    match &v.label_after {
        Some(l) if !l.is_empty() && !l.contains("//") => l.clone(),
        _ => {
            let d = v.index_after - v.index_before;
            format!("Corde : {d:+} cran{}", if d.abs() > 1 { "s" } else { "" })
        }
    }
}

/// The new file: the base with the changed indices (and their labels) and
/// new notes; every other byte as in the base. Returns the setup and the
/// number of `Key=value` lines changed (notes included).
pub fn build_setup(base: &Setup, changes: &[SetupChange], notes: &str) -> (Setup, u32) {
    let mut out = base.clone();
    let mut changed = 0u32;
    for c in changes {
        let Some(r) = &c.resolved else {
            continue;
        };
        if !is_kept(r.status) || r.status == ResolveStatus::Manual {
            continue;
        }
        for v in &r.values {
            if v.index_after == v.index_before || v.status == ResolveStatus::Manual {
                continue;
            }
            let label = written_label(v);
            if let Some(value) = out
                .sections
                .get_mut(&v.section)
                .and_then(|s| s.get_mut(&v.key))
            {
                value.set_index_with_display(v.index_after as i64, Some(label));
                changed += 1;
            }
        }
    }
    if out.notes().as_deref() != Some(notes) {
        out.set_notes(notes);
        changed += 1;
    }
    (out, changed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use corde_protocol::SetupPhilosophy;

    #[test]
    fn labels_are_parsed_only_when_they_are_numbers() {
        let p = |l: &str| parse_label(l).map(|(v, s)| (v, s.render(v, s.decimals)));
        assert_eq!(p("-2.20 deg"), Some((-2.2, "-2.20 deg".into())));
        assert_eq!(p("P2"), Some((2.0, "P2".into())));
        assert_eq!(p("51.5:48.5"), Some((51.5, "51.5:48.5".into())));
        assert_eq!(p("136 kPa"), Some((136.0, "136 kPa".into())));
        assert_eq!(p("33%"), Some((33.0, "33%".into())));
        assert_eq!(p("2.1N/mm"), Some((2.1, "2.1N/mm".into())));
        assert_eq!(p("13"), Some((13.0, "13".into())));
        for bad in [
            "+2 crans",
            "-1 cran",
            "+1 cran AV",
            "120 kgf (100%)",
            "1 (tendre)",
            "Open",
            "96% Medium",
            "13mm Soft (299)",
            "516 deg(19.7 )",
            "8F/60 deg (41.2%)",
            "",
        ] {
            assert_eq!(parse_label(bad), None, "{bad}");
        }
    }

    #[test]
    fn magnitudes_become_quantities() {
        let m = |s: &str| parse_magnitude(s).map(|m| (m.value, m.unit));
        assert_eq!(m("1 cran"), Some((1.0, "steps".into())));
        assert_eq!(m("2 clics"), Some((2.0, "steps".into())));
        assert_eq!(m("−3 kPa"), Some((3.0, "kPa".into())));
        assert_eq!(m("0.2–0.3°"), Some((0.2, "deg".into())));
        assert_eq!(m("0,2°"), Some((0.2, "deg".into())));
        assert_eq!(m("0.5–1 %"), Some((0.5, "%".into())));
        assert_eq!(m("±2 mm"), Some((2.0, "mm".into())));
        assert_eq!(m("+2 mm AR"), Some((2.0, "mm".into())));
        assert_eq!(m("15 Nm"), Some((15.0, "Nm".into())));
        assert_eq!(m("1–2 crans"), Some((1.0, "steps".into())));
        assert_eq!(m("0.5 cm"), Some((5.0, "mm".into())));
        assert_eq!(m("wet (pneus pluie)"), None);
    }

    #[test]
    fn a_linear_law_is_learnt_and_checked() {
        let pairs: Vec<(i64, String)> = vec![
            (22, "-2.20 deg".into()),
            (40, "-1.00 deg".into()),
            (22, "-2.20 deg".into()),
        ];
        let k = KeyLabels::from_pairs(&pairs);
        let (b, unit) = k.step().unwrap();
        assert!((b - 0.0667).abs() < 1e-3, "{b}");
        assert_eq!(unit, "deg");
        assert_eq!(k.label_for(22).as_deref(), Some("-2.20 deg"));
        assert_eq!(k.label_for(19).as_deref(), Some("-2.40 deg"));
        assert_eq!(k.label_for(28).as_deref(), Some("-1.80 deg"));
        // Too far from what was seen: no guess.
        assert_eq!(k.label_for(3), None);
        assert_eq!(k.label_for(59), None);
        // Two close points still label a few steps away (22, 25 → 19).
        let k = KeyLabels::from_pairs(&[(22, "-2.20 deg".into()), (25, "-2.00 deg".into())]);
        assert_eq!(k.label_for(19).as_deref(), Some("-2.40 deg"));
        assert_eq!(k.label_for(18), None);
        // Points that are not on a line give no law.
        let k = KeyLabels::from_pairs(&[(0, "1".into()), (1, "2".into()), (2, "9".into())]);
        assert!(k.step().is_none());
        assert_eq!(k.label_for(1).as_deref(), Some("2"));
        // A decreasing label (Porsche dampers: 5 ↔ 13, 8 ↔ 10).
        let k = KeyLabels::from_pairs(&[(5, "13".into()), (8, "10".into())]);
        assert_eq!(k.label_for(6).as_deref(), Some("12"));
        // Ratio labels keep both halves.
        let k = KeyLabels::from_pairs(&[(22, "51.5:48.5".into()), (34, "48.5:51.5".into())]);
        assert_eq!(k.label_for(26).as_deref(), Some("50.5:49.5"));
        // Percentages stay within 0..100.
        let k = KeyLabels::from_pairs(&[(1, "33%".into()), (2, "67%".into())]);
        assert_eq!(k.label_for(3), None);
    }

    fn change(param: &str, dir: ChangeDirection, magnitude: &str) -> SetupChange {
        SetupChange {
            parameter_id: param.into(),
            parameter_key: String::new(),
            section: String::new(),
            label_fr: param.into(),
            direction: dir,
            magnitude: magnitude.into(),
            steps: None,
            priority: 1,
            rule_ids: vec![],
            rationale_fr: String::new(),
            rationale_en: String::new(),
            side_effects_fr: vec![],
            conflicts_with: vec![],
            confidence: 0.5,
            resolved: None,
        }
    }

    const BASE: &str = "VehicleClassSetting=\"Test_Car GT3\"\r\n[GENERAL]\r\nNotes=\"\"\r\n[SUSPENSION]\r\nRearAntiSwaySetting=2//P2\r\nFrontToeInSetting=7//-0.117 deg\r\n[CONTROLS]\r\nRearBrakeSetting=22//51.5:48.5\r\n[DRIVELINE]\r\nDiffPowerSetting=0//Non-adjustable\r\n[FRONTLEFT]\r\nCamberSetting=22//-2.20 deg\r\nCompoundSetting=0//Medium\r\n[FRONTRIGHT]\r\nCamberSetting=21//-2.27 deg\r\nCompoundSetting=0//Medium\r\n";

    #[test]
    fn resolution_statuses() {
        let k = Knowledge::embedded().unwrap();
        let base = Setup::parse(BASE);
        let lib = [&base];
        let input = ResolveInput {
            base: &base,
            library: &lib,
            bounds: None,
        };
        let r = |c: SetupChange| resolve_change(&k, &c, &input).resolved.unwrap();

        let bar = r(change("susp.arb_rear", ChangeDirection::Increase, "1 cran"));
        assert_eq!(bar.status, ResolveStatus::UnknownBounds);
        assert_eq!(bar.values[0].index_after, 3);
        let bar = r(change(
            "susp.arb_rear",
            ChangeDirection::Increase,
            "5 crans",
        ));
        assert_eq!(bar.values[0].index_after, 4, "+2 at most without a maximum");
        let bar = r(change(
            "susp.arb_rear",
            ChangeDirection::Decrease,
            "5 crans",
        ));
        assert_eq!(bar.status, ResolveStatus::ClampedMin);
        assert_eq!(bar.values[0].index_after, 0);
        let diff = r(change("diff.power", ChangeDirection::Increase, "1 cran"));
        assert_eq!(diff.status, ResolveStatus::NotAdjustable);
        let missing = r(change(
            "susp.third_front",
            ChangeDirection::Increase,
            "1 cran",
        ));
        assert_eq!(missing.status, ResolveStatus::UnknownKey);
        let comp = r(change("tyre.compound", ChangeDirection::Set, "wet"));
        assert_eq!(comp.status, ResolveStatus::Manual);
        // Brake balance: more front = lower index, 0.25 %/step by default.
        let bb = r(change(
            "brake.bias_front",
            ChangeDirection::Increase,
            "0.5–1 %",
        ));
        assert_eq!(bb.values[0].index_after, 20);
        assert_eq!(bb.status, ResolveStatus::Applied);
        // Per wheel: both front wheels, each from its own index.
        let cam = r(change(
            "geo.camber_front",
            ChangeDirection::Increase,
            "1 cran",
        ));
        let idx: Vec<(Option<String>, i32)> = cam
            .values
            .iter()
            .map(|v| (v.wheel.clone(), v.index_after))
            .collect();
        assert_eq!(idx, [(Some("FL".into()), 21), (Some("FR".into()), 20)]);
        // Toe toward zero: −0.117 → index up (no law: one step).
        let toe = r(change(
            "geo.toe_front",
            ChangeDirection::TowardZero,
            "0.05°",
        ));
        assert_eq!(toe.values[0].index_after, 8);
        // Bounds from the garage.
        let mut b = GarageBounds::new();
        b.insert(
            "RearAntiSwaySetting".into(),
            corde_lmu::garage::KeyBound {
                min: Some(0.0),
                max: Some(3.0),
                ..Default::default()
            },
        );
        let input = ResolveInput {
            bounds: Some(&b),
            ..input
        };
        let bar = resolve_change(
            &k,
            &change("susp.arb_rear", ChangeDirection::Increase, "2 crans"),
            &input,
        )
        .resolved
        .unwrap();
        assert_eq!(bar.status, ResolveStatus::ClampedMax);
        assert_eq!(bar.values[0].index_after, 3);
        assert_eq!(bar.values[0].max, Some(3));
    }

    #[test]
    fn rake_goes_through_the_rear_ride_height_and_duplicates_are_dropped() {
        let k = Knowledge::embedded().unwrap();
        let base = Setup::parse(
            "VehicleClassSetting=\"X GT3\"\r\n[REARLEFT]\r\nRideHeightSetting=15//6.5 cm\r\n[REARRIGHT]\r\nRideHeightSetting=15//6.5 cm\r\n",
        );
        let lib = [&base];
        let input = ResolveInput {
            base: &base,
            library: &lib,
            bounds: None,
        };
        let (kept, dropped) = resolve_changes(
            &k,
            &[
                change("aero.rake", ChangeDirection::Increase, "1–2 mm"),
                change("aero.rh_rear", ChangeDirection::Decrease, "2 mm"),
            ],
            &input,
        );
        assert_eq!(kept.len(), 1);
        let r = kept[0].resolved.as_ref().unwrap();
        assert_eq!(r.values.len(), 2);
        assert_eq!(r.values[0].index_after, 16, "1 mm = 1 step by default");
        assert!(r.notes_fr.iter().any(|n| n.contains("hauteur arrière")));
        assert_eq!(dropped.len(), 1);
        assert_eq!(
            dropped[0].resolved.as_ref().unwrap().status,
            ResolveStatus::Duplicate
        );
    }

    #[test]
    fn notes_are_short_ascii() {
        let mut c = change("susp.arb_rear", ChangeDirection::Decrease, "1 cran");
        c.resolved = Some(ResolvedChange {
            status: ResolveStatus::Applied,
            steps: -1,
            values: vec![ResolvedValue {
                section: "SUSPENSION".into(),
                key: "RearAntiSwaySetting".into(),
                index_before: 2,
                index_after: 1,
                ..Default::default()
            }],
            ..Default::default()
        });
        let n = notes_for("Sûr — essai", "BMW_LongBeach.svm", &[c.clone()]);
        assert_eq!(
            n,
            "Corde - Sur - essai - base : BMW_LongBeach.svm - RearAntiSway -1"
        );
        let many = vec![c; 40];
        let n = notes_for("Agressif", "x.svm", &many);
        assert!(n.len() <= NOTES_MAX_CHARS && n.ends_with("..."), "{n}");
        assert!(n.is_ascii());
        assert_eq!(ascii_safe("Détaché \"x\" → é"), "Detache x -> e");
        let _ = SetupPhilosophy::Safe;
    }
}
