//! Assembles the [`AnalysisReport`] and writes the two summaries.

use crate::analysis::Computed;
use crate::series::SessionMeta;
use corde_protocol::{AnalysisReport, Finding, SetupPlans, Severity};

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn assemble(
    meta: &SessionMeta,
    laps_analysed: Vec<i32>,
    mut computed: Computed,
    findings: Vec<Finding>,
    labels_fr: Vec<String>,
    plans: SetupPlans,
) -> AnalysisReport {
    computed.style.labels_fr = labels_fr;
    let summary_fr = summary_fr(meta, &computed, &findings, &plans);
    let summary_en = summary_en(meta, &computed, &findings, &plans);
    AnalysisReport {
        engine_version: crate::ENGINE_VERSION,
        session_id: meta.session_id.clone(),
        generated_ms: now_ms(),
        track_name: meta.track_name.clone(),
        vehicle: meta.vehicle_name.clone(),
        vehicle_class: meta.vehicle_class,
        data_quality: computed.data_quality,
        laps_analysed,
        lap_stats: computed.lap_stats,
        driving_style: computed.style,
        tyres: computed.tyres,
        brakes: computed.brakes,
        balance: computed.balance,
        suspension: computed.suspension,
        findings,
        plans,
        summary_fr,
        summary_en,
    }
}

fn fmt_lap(s: Option<f32>) -> String {
    match s {
        Some(t) if t > 0.0 => {
            let m = (t / 60.0).floor();
            format!("{}:{:06.3}", m as u32, t - m * 60.0)
        }
        _ => "—".into(),
    }
}

/// Which orientation to advise: safe when the driver is irregular, nervous or
/// the car locks / spins; aggressive when consistent and the car is calm;
/// wet when the session was wet.
fn advised(c: &Computed, findings: &[Finding]) -> (&'static str, &'static str) {
    if c.wet_session {
        return ("pluie", "wet");
    }
    let has = |sym: &str| {
        findings
            .iter()
            .any(|f| f.severity >= Severity::Minor && f.symptoms.iter().any(|s| s == sym))
    };
    let safe = c.lap_stats.consistency_score < 60.0
        || has("PLT_NERVOUS")
        || has("BRK_LOCK_FRONT")
        || has("BRK_LOCK_REAR")
        || has("BRK_UNSTABLE_STRAIGHT")
        || has("TRC_WHEELSPIN_BOTH");
    if safe {
        ("soft / sûre", "safe")
    } else if c.lap_stats.consistency_score > 85.0 {
        ("agressive", "aggressive")
    } else {
        (
            "soft / sûre, puis agressive une fois la régularité acquise",
            "safe first, aggressive once consistent",
        )
    }
}

fn top_findings(findings: &[Finding], n: usize) -> Vec<&Finding> {
    findings
        .iter()
        .filter(|f| f.severity >= Severity::Minor)
        .take(n)
        .collect()
}

fn summary_fr(
    meta: &SessionMeta,
    c: &Computed,
    findings: &[Finding],
    plans: &SetupPlans,
) -> String {
    let q = &c.data_quality;
    let mut s = Vec::new();
    s.push(format!(
        "{} tour(s) propre(s) sur {} analysé(s) à {:.0} Hz sur {} ({}){} ; meilleur tour {}, régularité {:.0}/100.",
        q.valid_laps,
        q.total_laps,
        q.sample_rate_hz,
        meta.track_name,
        meta.vehicle_name,
        if q.wet_session { ", piste humide" } else { "" },
        fmt_lap(c.lap_stats.best_s),
        c.lap_stats.consistency_score
    ));
    if !q.has_slip {
        s.push("Canaux de glissement absents : blocages et patinage non détectés.".into());
    }
    let labels = &c.style.labels_fr;
    if labels.is_empty() {
        s.push(format!(
            "Style : agressivité au freinage {:.0}/100, trail braking {:.0} %, lissage gaz {:.0}/100, volant {:.0}/100.",
            c.style.brake_aggression, c.style.trail_braking, c.style.throttle_smoothness, c.style.steering_smoothness
        ));
    } else {
        s.push(format!("Style : {}.", labels.join(", ")));
    }
    let top = top_findings(findings, 3);
    if top.is_empty() {
        s.push("Aucun constat marquant : équilibre neutre et pneus / freins dans la norme sur ces tours.".into());
    } else {
        s.push(format!(
            "Constats principaux : {}.",
            top.iter()
                .map(|f| format!(
                    "{} (confiance {:.0} %)",
                    f.title_fr.to_lowercase(),
                    f.confidence * 100.0
                ))
                .collect::<Vec<_>>()
                .join(" ; ")
        ));
    }
    let (adv, _) = advised(c, findings);
    let plan = match adv {
        "pluie" => &plans.wet,
        "agressive" => &plans.aggressive,
        _ => &plans.safe,
    };
    let mut by_priority: Vec<&corde_protocol::SetupChange> = plan.changes.iter().collect();
    by_priority.sort_by_key(|c| c.priority);
    let first: Vec<String> = by_priority
        .iter()
        .take(3)
        .map(|ch| {
            format!(
                "{} {}{}",
                ch.label_fr.to_lowercase(),
                match ch.direction {
                    corde_protocol::ChangeDirection::Increase => "+",
                    corde_protocol::ChangeDirection::Decrease => "−",
                    corde_protocol::ChangeDirection::TowardZero => "→ 0 ",
                    corde_protocol::ChangeDirection::Set => "= ",
                },
                ch.magnitude
            )
        })
        .collect();
    s.push(format!(
        "Orientation conseillée : {adv}{}.",
        if first.is_empty() {
            String::new()
        } else {
            format!(" — d'abord {}", first.join(", "))
        }
    ));
    s.join(" ")
}

fn summary_en(
    meta: &SessionMeta,
    c: &Computed,
    findings: &[Finding],
    plans: &SetupPlans,
) -> String {
    let q = &c.data_quality;
    let mut s = Vec::new();
    s.push(format!(
        "{} clean lap(s) out of {} analysed at {:.0} Hz on {} ({}){}; best lap {}, consistency {:.0}/100.",
        q.valid_laps,
        q.total_laps,
        q.sample_rate_hz,
        meta.track_name,
        meta.vehicle_name,
        if q.wet_session { ", wet track" } else { "" },
        fmt_lap(c.lap_stats.best_s),
        c.lap_stats.consistency_score
    ));
    if !q.has_slip {
        s.push("Slip channels missing: lock-ups and wheel-spin not detected.".into());
    }
    s.push(format!(
        "Style: brake attack {:.0}/100, trail braking {:.0} %, throttle smoothness {:.0}/100, steering {:.0}/100.",
        c.style.brake_aggression, c.style.trail_braking, c.style.throttle_smoothness, c.style.steering_smoothness
    ));
    let top = top_findings(findings, 3);
    if top.is_empty() {
        s.push(
            "No notable finding: neutral balance, tyres and brakes within range on these laps."
                .into(),
        );
    } else {
        s.push(format!(
            "Main findings: {}.",
            top.iter()
                .map(|f| format!(
                    "{} ({:.0} % confidence)",
                    f.title_en.to_lowercase(),
                    f.confidence * 100.0
                ))
                .collect::<Vec<_>>()
                .join("; ")
        ));
    }
    let (_, adv) = advised(c, findings);
    let plan = match adv {
        "wet" => &plans.wet,
        "aggressive" => &plans.aggressive,
        _ => &plans.safe,
    };
    s.push(format!(
        "Advised orientation: {adv} ({} change(s) proposed).",
        plan.changes.len()
    ));
    s.join(" ")
}
