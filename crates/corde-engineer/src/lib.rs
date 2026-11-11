//! # corde-engineer
//!
//! The race engineer of Corde: turns a recorded session (`corde-store`) into
//! an [`AnalysisReport`] — data quality, lap statistics, driving style,
//! tyres, brakes, balance matrix, platform — a list of [`Finding`]s aligned
//! on `knowledge/diagnostics.yaml`, and three relative setup orientations
//! (aggressive / safe / wet) from `knowledge/setup-rules.yaml`.
//!
//! ```text
//! Store ──load──▶ LapSeries (clean laps) ──corners──▶ phases
//!                     │                                 │
//!                     └──metrics──▶ Computed ◀──────────┘
//!                                      │
//!                            diagnostics ──▶ Findings ──rules──▶ SetupPlans
//!                                      │                              │
//!                                      └──────── report ◀─────────────┘
//! ```
//!
//! The plans stay relative in the report (cached by the server); [`setup`]
//! resolves them against one of the driver's `.svm` files (index before →
//! after, labels, bounds, non-adjustable parameters left out) and writes the
//! new files' content. No UI and no file system access to the game here.
//! Formulas and thresholds: `docs/ENGINEER.md`.

pub mod analysis;
pub mod corners;
pub mod diagnostics;
pub mod error;
pub mod feedback;
pub mod iterate;
pub mod load;
pub mod metrics;
pub mod report;
pub mod rules;
pub mod series;
pub mod setup;

pub use corde_protocol::{AnalysisReport, Finding, SetupPlans};
pub use corners::CornerConfig;
pub use error::{EngineerError, Result};
pub use feedback::{
    interpret, AnthropicExtractor, DictionaryExtractor, FeedbackDictionary, SymptomExtractor,
};
pub use iterate::{iterate_plan, IterationContext};
pub use load::load_session;
pub use rules::{Knowledge, MAX_CHANGES_PER_PLAN};
pub use series::{LapSeries, SessionMeta};

use corde_store::Store;

/// Version of the analysis written into every report
/// (`AnalysisReport::engine_version`). Bump it whenever a report computed by
/// the previous version would mislead: the server then recomputes the cached
/// reports instead of serving them.
///
/// * 0 / absent: reports written before the version was recorded;
/// * 2: balance relative to the session reference, brake window on the
///   typical peak, optimal tyre temperature read from the samples (S2).
pub const ENGINE_VERSION: u32 = 2;

/// Tuning of one analysis.
#[derive(Debug, Clone)]
pub struct AnalyseOptions {
    /// Knowledge base; the embedded one when `None`.
    pub knowledge: Option<Knowledge>,
    /// Changes kept per plan (1..=12).
    pub max_changes_per_plan: usize,
    pub corners: CornerConfig,
}

impl Default for AnalyseOptions {
    fn default() -> Self {
        Self {
            knowledge: None,
            max_changes_per_plan: MAX_CHANGES_PER_PLAN,
            corners: CornerConfig::default(),
        }
    }
}

/// Analyse a recorded session. `NotFound` when the session does not exist,
/// `NotEnoughData` when no clean lap can be analysed.
pub fn analyse_session(
    store: &Store,
    session_id: &str,
    opts: &AnalyseOptions,
) -> Result<AnalysisReport> {
    let loaded = load::load_session(store, session_id)?;
    analyse_samples(&loaded.meta, &loaded.laps, opts)
}

/// Analyse already-loaded laps (what the tests feed without a database).
/// Deterministic: the same input gives the same report (except `generated_ms`).
pub fn analyse_samples(
    meta: &SessionMeta,
    laps: &[LapSeries],
    opts: &AnalyseOptions,
) -> Result<AnalysisReport> {
    if laps.is_empty() {
        return Err(EngineerError::NotEnoughData(
            "aucun tour exploitable : il faut au moins un tour valide complet".into(),
        ));
    }
    let knowledge = match &opts.knowledge {
        Some(k) => k.clone(),
        None => Knowledge::embedded()?,
    };
    let computed = analysis::compute(meta, laps, &opts.corners);
    let (findings, labels) = diagnostics::findings(&computed, meta.vehicle_class);
    let plans = rules::build_plans(
        &knowledge,
        &findings,
        meta.vehicle_class,
        computed.wet_session,
        meta.setup_known,
        opts.max_changes_per_plan,
    );
    let laps_analysed = laps.iter().map(|l| l.lap_number).collect();
    Ok(report::assemble(
        meta,
        laps_analysed,
        computed,
        findings,
        labels,
        plans,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::series::wheel_channel;
    use corde_protocol::{LapRecord, Severity, VehicleClass};

    pub(super) fn session_for_dump() -> (SessionMeta, Vec<LapSeries>) {
        session()
    }

    /// Three synthetic laps with one corner whose middle understeers markedly
    /// more than its entry and exit, and a front lock-up.
    fn session() -> (SessionMeta, Vec<LapSeries>) {
        session_with(|ts| if (5.0..7.0).contains(&ts) { 4.0 } else { 2.0 })
    }

    /// Same laps, front slip angle given by `front_slip(t)` (rear 1.5°).
    fn session_with(front_slip: impl Fn(f64) -> f64) -> (SessionMeta, Vec<LapSeries>) {
        let mut laps = Vec::new();
        for k in 0..3 {
            let (mut lap, _, _) = crate::corners::tests::synthetic_lap(50.0, 1.4);
            let n = lap.n;
            let g: Vec<f64> = lap.col("g_lat").unwrap().to_vec();
            for w in 0..4 {
                lap.set(
                    wheel_channel("slip_angle_deg", w),
                    g.iter()
                        .enumerate()
                        .map(|(i, g)| {
                            let a = if w < 2 {
                                front_slip(i as f64 / 50.0)
                            } else {
                                1.5
                            };
                            if g.abs() > 0.1 {
                                a * g.signum()
                            } else {
                                0.0
                            }
                        })
                        .collect::<Vec<_>>(),
                );
                let mut slip = vec![0.0; n];
                if w == 0 {
                    for (i, s) in slip.iter_mut().enumerate() {
                        let ts = i as f64 / 50.0;
                        if (3.5..3.7).contains(&ts) {
                            *s = -0.6;
                        }
                    }
                }
                lap.set(wheel_channel("slip_ratio", w), slip);
                lap.set(wheel_channel("brake_temp_c", w), vec![450.0; n]);
                lap.set(wheel_channel("temp_l_c", w), vec![85.0; n]);
                lap.set(wheel_channel("temp_c_c", w), vec![88.0; n]);
                lap.set(wheel_channel("temp_r_c", w), vec![90.0; n]);
                lap.set(wheel_channel("carcass_c", w), vec![84.0; n]);
                lap.set(wheel_channel("pressure_kpa", w), vec![172.0; n]);
                lap.set(
                    wheel_channel("susp_deflection_m", w),
                    (0..n)
                        .map(|i| 0.03 + 0.002 * (i as f64 * 0.05).sin())
                        .collect::<Vec<_>>(),
                );
                lap.set(wheel_channel("ride_height_m", w), vec![0.05; n]);
                lap.set(
                    wheel_channel("wear_remaining", w),
                    vec![1.0 - 0.003 * k as f64; n],
                );
                lap.set(wheel_channel("surface", w), vec![0.0; n]);
            }
            lap.set("brake_bias_rear", vec![0.46; n]);
            lap.set("tc_active", vec![0.0; n]);
            lap.set("rain", vec![0.0; n]);
            lap.lap_number = k + 1;
            lap.record = LapRecord {
                lap_number: k + 1,
                lap_time_s: 12.0 + 0.05 * k as f32,
                valid: true,
                fuel_used_l: 0.4,
                top_speed_kmh: 220.0,
                ..Default::default()
            };
            laps.push(lap);
        }
        let meta = SessionMeta {
            session_id: "test-session".into(),
            track_name: "Synthetic".into(),
            track_length_m: 0.0,
            vehicle_name: "GT3 test".into(),
            vehicle_class: VehicleClass::Gt3,
            record_rate_hz: 50.0,
            setup_known: false,
            all_laps: laps.iter().map(|l| l.record.clone()).collect(),
            ..Default::default()
        };
        (meta, laps)
    }

    #[test]
    fn end_to_end_on_synthetic_laps() {
        let (meta, laps) = session();
        let report = analyse_samples(&meta, &laps, &AnalyseOptions::default()).unwrap();
        assert_eq!(report.laps_analysed, vec![1, 2, 3]);
        assert_eq!(report.data_quality.valid_laps, 3);
        assert!(report.data_quality.has_slip);
        assert!(report.data_quality.has_ride_height);
        assert!(!report.data_quality.wet_session);
        assert_eq!(report.balance.corners.len(), 1);
        assert_eq!(report.balance.corners[0].passes, 3);
        assert_eq!(report.engine_version, ENGINE_VERSION);
        let mid = &report.balance.per_phase.mid.slow;
        assert!(mid.understeer_index > 0.3, "{mid:?}");
        assert!(mid.confidence > 0.5, "{mid:?}");
        // Entry and exit are this car's usual balance: the reference.
        let reference = report.balance.reference.as_ref().expect("reference");
        assert!(
            (reference.slip_balance.unwrap() - 0.5 / 3.5).abs() < 0.01,
            "{reference:?}"
        );
        assert_eq!(reference.passes, 9);
        for c in [
            &report.balance.per_phase.entry.slow,
            &report.balance.per_phase.exit.slow,
        ] {
            assert!(c.understeer_index.abs() < 0.3, "{c:?}");
            // The absolute index leans to understeer everywhere, as on real sessions.
            assert!(c.raw_index > 0.2, "{c:?}");
        }
        assert!(
            report
                .balance
                .overall_fr
                .contains("rapport à ta référence de la session")
                && report
                    .balance
                    .overall_fr
                    .contains("milieu de virages lents"),
            "{}",
            report.balance.overall_fr
        );
        let us = report
            .findings
            .iter()
            .find(|f| f.symptoms.iter().any(|s| s == "US_MID_SLOW"))
            .expect("understeer finding");
        assert!(us.severity >= Severity::Minor);
        assert!(report
            .findings
            .iter()
            .any(|f| f.symptoms.iter().any(|s| s == "BRK_LOCK_FRONT")));
        assert_eq!(report.brakes.per_wheel[0].lockups, 3);
        assert_eq!(report.brakes.lockup_front_vs_rear, Some(1.0));
        assert!(!report.plans.aggressive.changes.is_empty());
        assert!(!report.plans.safe.changes.is_empty());
        assert!(!report.plans.wet.changes.is_empty());
        assert!(report.plans.aggressive.changes.len() <= MAX_CHANGES_PER_PLAN);
        // The understeer rule shows up in the plans.
        assert!(report
            .plans
            .aggressive
            .changes
            .iter()
            .any(|c| c.rule_ids.iter().any(|r| r == "US-MID-SLOW-01")));
        assert!(report.summary_fr.contains("Orientation conseillée"));
        assert!(report.summary_en.contains("Advised orientation"));
        // Serialisable, and deterministic.
        let json = serde_json::to_string(&report).unwrap();
        let back: AnalysisReport = serde_json::from_str(&json).unwrap();
        assert_eq!(back.findings.len(), report.findings.len());
        let again = analyse_samples(&meta, &laps, &AnalyseOptions::default()).unwrap();
        assert_eq!(again.findings, report.findings);
        assert_eq!(again.plans, report.plans);
    }

    #[test]
    fn a_uniformly_understeering_car_has_no_balance_finding() {
        // Front slip angles 4° against 1.5° everywhere: the absolute index is high in every
        // cell (what every real session showed), but nothing stands out from the car's usual
        // behaviour, so there is nothing to fix from the data alone.
        let (meta, laps) = session_with(|_| 4.0);
        let report = analyse_samples(&meta, &laps, &AnalyseOptions::default()).unwrap();
        let reference = report.balance.reference.as_ref().expect("reference");
        assert!(reference.raw_index > 0.6, "{reference:?}");
        for (_, _, c) in analysis::cells(&report.balance.per_phase) {
            if c.samples > 0 {
                assert!(c.understeer_index.abs() < 0.3, "{c:?}");
            }
        }
        assert!(
            report
                .findings
                .iter()
                .all(|f| f.diagnostic_id != "D-BAL-01"),
            "{:?}",
            report.findings.iter().map(|f| &f.id).collect::<Vec<_>>()
        );
        assert!(
            report.balance.overall_fr.contains("ne se détache"),
            "{}",
            report.balance.overall_fr
        );
        // Oversteer on exit only: found, with the right sign.
        let (meta, laps) = session_with(|ts| if ts >= 7.0 { 1.2 } else { 3.0 });
        let report = analyse_samples(&meta, &laps, &AnalyseOptions::default()).unwrap();
        assert!(report.balance.per_phase.exit.slow.understeer_index < -0.3);
        assert!(report
            .findings
            .iter()
            .any(|f| f.symptoms.iter().any(|s| s == "OS_EXIT_SLOW")));
        assert!(report
            .findings
            .iter()
            .all(|f| !f.symptoms.iter().any(|s| s.starts_with("US_"))));
    }

    #[test]
    fn optimal_tyre_temperature_is_used_when_recorded() {
        // Older sessions: no column → unknown, as before.
        let (meta, laps) = session();
        let report = analyse_samples(&meta, &laps, &AnalyseOptions::default()).unwrap();
        assert!(!report.data_quality.has_optimal_temp);
        assert!(report.tyres.per_wheel.iter().all(|w| w.optimal_c.is_none()));
        assert!(report
            .findings
            .iter()
            .all(|f| f.diagnostic_id != "D-TYR-03"));
        // Recorded: carcass 84 °C against an optimum of 95 °C → cold tyres on both axles.
        let (meta, mut laps) = session();
        for lap in &mut laps {
            let n = lap.n;
            for w in 0..4 {
                let mut v = vec![95.0; n];
                v[0] = f64::NAN; // a null sample does not matter
                lap.set(wheel_channel("optimal_temp_c", w), v);
            }
        }
        let report = analyse_samples(&meta, &laps, &AnalyseOptions::default()).unwrap();
        assert!(report.data_quality.has_optimal_temp);
        let fl = &report.tyres.per_wheel[0];
        assert_eq!(fl.optimal_c, Some(95.0));
        assert_eq!(fl.delta_to_optimal_c, Some(-11.0));
        let cold: Vec<&Finding> = report
            .findings
            .iter()
            .filter(|f| f.diagnostic_id == "D-TYR-03")
            .collect();
        assert_eq!(cold.len(), 2, "{cold:?}");
        assert!(cold
            .iter()
            .all(|f| f.symptoms == vec!["TYR_COLD".to_string()]));
        assert!(!report
            .data_quality
            .notes
            .iter()
            .any(|n| n.contains("Température optimale")));
    }

    #[test]
    fn empty_input_is_not_enough_data() {
        let (meta, _) = session();
        let err = analyse_samples(&meta, &[], &AnalyseOptions::default()).unwrap_err();
        assert!(err.is_not_enough_data());
    }
}

#[cfg(test)]
mod dump {
    #[test]
    #[ignore]
    fn dump_synthetic_report() {
        let (meta, laps) = super::tests::session_for_dump();
        let report =
            super::analyse_samples(&meta, &laps, &super::AnalyseOptions::default()).unwrap();
        println!("{}", serde_json::to_string_pretty(&report).unwrap());
    }
}
