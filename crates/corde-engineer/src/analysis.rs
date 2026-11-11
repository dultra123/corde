//! Orchestration: from the loaded laps to every block of the report except
//! the findings, the plans and the summaries.

use crate::corners::{self, Corner, CornerConfig, MatchedCorner};
use crate::metrics;
use crate::series::{LapSeries, SessionMeta};
use corde_protocol::{
    BalanceAnalysis, BalanceCell, BalanceMatrix, BalanceReference, BalanceRow, BrakeAnalysis,
    CornerPhase, CornerSpeed, CornerStat, DataQuality, DrivingStyle, LapStats, SuspensionAnalysis,
    TurnDirection, TyreAnalysis,
};

/// Fewer clean laps than this halves the confidence of every finding.
pub const MIN_LAPS_FULL_CONFIDENCE: usize = 3;
/// A balance cell built on fewer (corner, lap) passes than this is shown but
/// never becomes a finding: one pass does not make a trend.
pub const MIN_PASSES_FOR_FINDING: usize = 2;

/// Everything computed before the diagnostics run.
#[derive(Debug, Clone)]
pub struct Computed {
    pub data_quality: DataQuality,
    pub lap_stats: LapStats,
    pub style: DrivingStyle,
    pub tyres: TyreAnalysis,
    pub brakes: BrakeAnalysis,
    pub balance: BalanceAnalysis,
    /// `(phase, speed, passes)` of every cell of the balance matrix.
    pub balance_cell_passes: Vec<(CornerPhase, CornerSpeed, usize)>,
    pub suspension: SuspensionAnalysis,
    pub corners_per_lap: Vec<Vec<Corner>>,
    /// Lock-ups per wheel over the analysed laps.
    pub lockups_per_wheel: [u32; 4],
    /// Front lock-ups that happened on the inside wheel of a loaded corner.
    pub lockups_inner_front: u32,
    pub lockup_laps: Vec<i32>,
    pub spins_per_wheel: [u32; 4],
    pub spin_laps: Vec<i32>,
    pub abs_activity: Option<f64>,
    pub tc_exit_activity: Option<f64>,
    pub braking_corrections_per_lap: f64,
    /// `(lap number, bottoming events)`.
    pub bottoming_per_lap: Vec<(i32, usize)>,
    pub wet_session: bool,
    pub n_laps: usize,
    /// Global confidence factor from the coverage (laps, channels).
    pub coverage_factor: f64,
    /// Steering / throttle channels present (style scores are neutral otherwise).
    pub has_steering: bool,
    pub has_throttle: bool,
}

/// Is the session wet? Start weather, lap records or samples.
pub fn is_wet(meta: &SessionMeta, laps: &[LapSeries]) -> bool {
    if meta.weather.rain > 0.2 || meta.weather.wetness_avg > 0.3 {
        return true;
    }
    if laps.iter().any(|l| l.record.rain > 0.2) {
        return true;
    }
    let rain = metrics::mean(laps.iter().filter_map(|l| l.col("rain")).flatten().copied());
    let wet = metrics::mean(
        laps.iter()
            .filter_map(|l| l.col("wetness_avg"))
            .flatten()
            .copied(),
    );
    rain.map(|r| r > 0.2).unwrap_or(false) || wet.map(|w| w > 0.3).unwrap_or(false)
}

fn any_live(laps: &[LapSeries], base: &str) -> bool {
    laps.iter()
        .any(|l| (0..4).any(|w| l.is_live(&crate::series::wheel_channel(base, w))))
}

pub fn compute(meta: &SessionMeta, laps: &[LapSeries], cfg: &CornerConfig) -> Computed {
    let class = meta.vehicle_class;
    let n_laps = laps.len();
    let wet_session = is_wet(meta, laps);

    // --- data quality -------------------------------------------------------
    let has_slip = any_live(laps, "slip_ratio") || any_live(laps, "slip_angle_deg");
    let has_ride_height = any_live(laps, "ride_height_m");
    // Recorded since S2; older sessions lack the column (NaN = unknown).
    let optimal_c: [Option<f64>; 4] = std::array::from_fn(|w| metrics::optimal_temp_c(laps, w));
    let has_optimal_temp = optimal_c.iter().any(Option::is_some);
    let measured_rate = metrics::median(laps.iter().map(|l| 1.0 / l.dt()));
    let mut notes = meta.notes.clone();
    if n_laps < MIN_LAPS_FULL_CONFIDENCE {
        notes.push(format!(
            "{n_laps} tour(s) propre(s) seulement : confiance des constats réduite (≥ 3 tours conseillés)."
        ));
    }
    if !has_slip {
        notes.push(
            "Canaux de glissement (slip_ratio / slip_angle) absents ou plats : pas de détection de blocage / patinage, équilibre estimé par le volant et le lacet seulement."
                .into(),
        );
    }
    if !has_ride_height {
        notes.push("Hauteurs de caisse par roue absentes : talonnage estimé sur la course de suspension seulement.".into());
    }
    if !has_optimal_temp {
        notes.push(
            "Température optimale des pneus absente des échantillons (session enregistrée avant ce canal, ou valeur non fournie par le jeu) : niveau de carcasse non comparé à la fenêtre du pneu."
                .into(),
        );
    }
    let data_quality = DataQuality {
        sample_rate_hz: measured_rate.unwrap_or(meta.record_rate_hz as f64) as f32,
        valid_laps: n_laps as u32,
        total_laps: meta.all_laps.len() as u32,
        has_slip,
        has_ride_height,
        has_optimal_temp,
        wet_session,
        notes,
    };
    let coverage_factor = if n_laps >= MIN_LAPS_FULL_CONFIDENCE {
        1.0
    } else {
        0.5
    };

    // --- corners & balance ----------------------------------------------------
    let corners_per_lap: Vec<Vec<Corner>> = laps
        .iter()
        .map(|l| corners::segment(l, class, cfg))
        .collect();
    let matched = corners::match_corners(
        &corners_per_lap,
        cfg.match_tolerance_m,
        meta.track_length_m as f64,
    );
    let wb = metrics::wheelbase_m(class);
    let ratio = metrics::median(
        laps.iter()
            .filter_map(|l| metrics::estimate_steering_ratio(l, wb)),
    );
    let series: Vec<metrics::BalanceSeries> = laps
        .iter()
        .map(|l| metrics::balance_series(l, class, ratio))
        .collect();
    let (balance, balance_cell_passes) =
        balance_analysis(laps, &corners_per_lap, &matched, &series, n_laps, has_slip);

    // --- lock-ups / wheel-spin -----------------------------------------------
    let mut lockups_per_wheel = [0u32; 4];
    let mut lockups_inner_front = 0u32;
    let mut lockup_laps = Vec::new();
    let mut spins_per_wheel = [0u32; 4];
    let mut spin_laps = Vec::new();
    for lap in laps {
        let g = lap.col("g_lat");
        let mut lap_locks = 0;
        let mut lap_spins = 0;
        for w in 0..4 {
            let locks = metrics::lockups(lap, w);
            lockups_per_wheel[w] += locks.len() as u32;
            lap_locks += locks.len();
            if w < 2 {
                for e in &locks {
                    let gl = g.and_then(|g| g.get(e.start).copied()).unwrap_or(0.0);
                    if gl.abs() > metrics::LOADED_G && !metrics::is_outer(w, gl) {
                        lockups_inner_front += 1;
                    }
                }
            }
            let spins = metrics::wheelspins(lap, w);
            spins_per_wheel[w] += spins.len() as u32;
            lap_spins += spins.len();
        }
        if lap_locks > 0 {
            lockup_laps.push(lap.lap_number);
        }
        if lap_spins > 0 {
            spin_laps.push(lap.lap_number);
        }
    }

    // --- tyres, brakes --------------------------------------------------------
    let tyres = TyreAnalysis {
        per_wheel: std::array::from_fn(|w| metrics::tyre_wheel(laps, w, optimal_c[w])),
    };
    let brakes_per_wheel =
        std::array::from_fn(|w| metrics::brake_wheel(laps, w, class, lockups_per_wheel[w]));
    let bias = metrics::mean(
        laps.iter()
            .filter_map(|l| l.col("brake_bias_rear"))
            .flatten()
            .copied(),
    )
    .filter(|b| *b > 0.0 && *b < 1.0);
    let front_locks = (lockups_per_wheel[0] + lockups_per_wheel[1]) as f64;
    let total_locks = front_locks + (lockups_per_wheel[2] + lockups_per_wheel[3]) as f64;
    let brakes = BrakeAnalysis {
        per_wheel: brakes_per_wheel,
        bias_rear_avg: bias.map(|b| b as f32),
        lockup_front_vs_rear: (total_locks > 0.0).then(|| (front_locks / total_locks) as f32),
    };
    let abs_activity = if class == corde_protocol::VehicleClass::Gt3 {
        metrics::abs_activity(laps)
    } else {
        None
    };
    let tc_exit_activity = metrics::tc_activity_on_exit(laps, &corners_per_lap);
    let braking_corrections_per_lap = metrics::straight_braking_corrections(laps, 10.0);

    // --- suspension -----------------------------------------------------------
    let range = metrics::travel_range(laps);
    let bottoming_per_lap: Vec<(i32, usize)> = laps
        .iter()
        .map(|l| (l.lap_number, metrics::bottoming_events(l, &range).len()))
        .collect();
    let bottoming_total: usize = bottoming_per_lap.iter().map(|x| x.1).sum();
    let min_rh = |wheels: [usize; 2]| -> f32 {
        let mut m = f64::INFINITY;
        for lap in laps {
            for w in wheels {
                if let Some(c) = lap.wheel("ride_height_m", w) {
                    for &v in c {
                        if v.is_finite() && v > 0.0 {
                            m = m.min(v);
                        }
                    }
                }
            }
        }
        if m.is_finite() {
            (m * 1000.0) as f32
        } else {
            0.0
        }
    };
    let suspension = SuspensionAnalysis {
        bottoming_events_per_lap: if n_laps == 0 {
            0.0
        } else {
            bottoming_total as f32 / n_laps as f32
        },
        min_ride_height_mm: [min_rh([0, 1]), min_rh([2, 3])],
        travel_saturation_pct: std::array::from_fn(|w| {
            metrics::travel_saturation_pct(laps, w, range[w]) as f32
        }),
        damper_velocity_hist: std::array::from_fn(|w| {
            metrics::damper_hist(laps, w, metrics::DAMPER_SLOW_MM_S)
        }),
    };

    // --- style & laps ---------------------------------------------------------
    let style = metrics::driving_style(laps, &corners_per_lap);
    let analysed: Vec<_> = laps.iter().map(|l| l.record.clone()).collect();
    let lap_stats = metrics::lap_stats(&meta.all_laps, &analysed);

    Computed {
        data_quality,
        lap_stats,
        style,
        tyres,
        brakes,
        balance,
        balance_cell_passes,
        suspension,
        corners_per_lap,
        lockups_per_wheel,
        lockups_inner_front,
        lockup_laps,
        spins_per_wheel,
        spin_laps,
        abs_activity,
        tc_exit_activity,
        braking_corrections_per_lap,
        bottoming_per_lap,
        wet_session,
        n_laps,
        coverage_factor,
        has_steering: laps.iter().any(|l| l.is_live("steering_angle_deg")),
        has_throttle: laps.iter().any(|l| l.is_live("throttle")),
    }
}

/// One (corner, phase) pass after normalisation.
#[derive(Debug, Clone, Copy)]
struct PassValue {
    /// Relative to the session reference.
    index: f64,
    /// Absolute (fixed scales), kept as `raw_index`.
    raw: f64,
    agreement: f64,
    samples: usize,
    speed: CornerSpeed,
    phase: CornerPhase,
}

/// One (matched corner, lap pass, phase) with its raw estimators.
struct RawPass {
    corner: usize,
    phase_idx: usize,
    raw: metrics::PhaseRaw,
    speed: CornerSpeed,
}

fn majority_speed(passes: &[&Corner]) -> CornerSpeed {
    let count = |s: CornerSpeed| passes.iter().filter(|c| c.speed_class == s).count();
    let (mut best, mut n) = (CornerSpeed::Slow, count(CornerSpeed::Slow));
    for s in [CornerSpeed::Medium, CornerSpeed::Fast] {
        let k = count(s);
        if k > n {
            best = s;
            n = k;
        }
    }
    best
}

fn coverage(passes: usize) -> f64 {
    match passes {
        0 => 0.0,
        1 => 0.4,
        2 => 0.6,
        _ => 1.0,
    }
}

fn balance_analysis(
    laps: &[LapSeries],
    corners_per_lap: &[Vec<Corner>],
    matched: &[MatchedCorner],
    series: &[metrics::BalanceSeries],
    n_laps: usize,
    has_slip: bool,
) -> (BalanceAnalysis, Vec<(CornerPhase, CornerSpeed, usize)>) {
    let phases = [CornerPhase::Entry, CornerPhase::Mid, CornerPhase::Exit];
    // 1. Raw estimators of every (corner, lap pass, phase).
    let mut raw_passes: Vec<RawPass> = Vec::new();
    let mut speeds = Vec::with_capacity(matched.len());
    for (k, m) in matched.iter().enumerate() {
        let passes: Vec<&Corner> = m
            .passes
            .iter()
            .map(|&(li, ci)| &corners_per_lap[li][ci])
            .collect();
        let speed = majority_speed(&passes);
        speeds.push(speed);
        for (pi, phase) in phases.iter().enumerate() {
            for &(li, ci) in &m.passes {
                let raw = metrics::corner_phase_raw(&series[li], &corners_per_lap[li][ci], *phase);
                if raw.samples > 0 {
                    raw_passes.push(RawPass {
                        corner: k,
                        phase_idx: pi,
                        raw,
                        speed,
                    });
                }
            }
        }
    }
    // 2. The session reference: what this car usually does in this session.
    let all_raw: Vec<metrics::PhaseRaw> = raw_passes.iter().map(|p| p.raw).collect();
    let refs = metrics::BalanceRefs::of(&all_raw);
    // 3. Every pass relative to it (and its absolute index, for the record).
    let mut values: Vec<PassValue> = Vec::new();
    let mut per_corner: Vec<[Vec<f64>; 3]> = vec![Default::default(); matched.len()];
    let mut raw_all = Vec::new();
    for p in &raw_passes {
        let raw =
            metrics::normalise_balance(p.raw.slip_delta_deg, p.raw.gradient_deg_per_g, p.speed)
                .map(|x| x.0);
        if let Some(r) = raw {
            raw_all.push(r);
        }
        let Some((index, agreement)) = metrics::relative_balance(&p.raw, &refs) else {
            continue;
        };
        per_corner[p.corner][p.phase_idx].push(index);
        values.push(PassValue {
            index,
            raw: raw.unwrap_or(f64::NAN),
            agreement,
            samples: p.raw.samples,
            speed: p.speed,
            phase: phases[p.phase_idx],
        });
    }
    let reference = (!refs.is_empty()).then(|| BalanceReference {
        raw_index: metrics::median(raw_all.iter().copied()).unwrap_or(0.0) as f32,
        slip_balance: refs.slip.map(|r| r.median as f32),
        slip_balance_sigma: refs.slip.map(|r| r.sigma as f32),
        gradient_deg_per_g: refs.gradient.map(|r| r.median as f32),
        gradient_sigma_deg_per_g: refs.gradient.map(|r| r.sigma as f32),
        passes: refs
            .slip
            .map(|r| r.n)
            .max(refs.gradient.map(|r| r.n))
            .unwrap_or(0) as u32,
        trigger_index: metrics::BALANCE_TRIGGER_INDEX as f32,
    });

    let mut stats = Vec::new();
    for (k, m) in matched.iter().enumerate() {
        let passes: Vec<&Corner> = m
            .passes
            .iter()
            .map(|&(li, ci)| &corners_per_lap[li][ci])
            .collect();
        let speed = speeds[k];
        let per_phase: [Option<f32>; 3] = std::array::from_fn(|pi| {
            metrics::median(per_corner[k][pi].iter().copied()).map(|v| v as f32)
        });
        let dir = {
            let right = passes
                .iter()
                .filter(|c| c.direction == TurnDirection::Right)
                .count();
            let left = passes
                .iter()
                .filter(|c| c.direction == TurnDirection::Left)
                .count();
            if right > left {
                TurnDirection::Right
            } else if left > right {
                TurnDirection::Left
            } else {
                TurnDirection::Unknown
            }
        };
        stats.push(CornerStat {
            index: (k + 1) as u32,
            start_dist_m: metrics::mean(passes.iter().map(|c| c.start_dist_m)).unwrap_or(0.0)
                as f32,
            end_dist_m: metrics::mean(passes.iter().map(|c| c.end_dist_m)).unwrap_or(0.0) as f32,
            apex_dist_m: m.apex_dist_m as f32,
            direction: dir,
            speed_class: Some(speed),
            min_speed_kmh: metrics::mean(passes.iter().map(|c| c.min_speed_kmh)).unwrap_or(0.0)
                as f32,
            max_g_lat: metrics::mean(passes.iter().map(|c| c.max_g_lat)).unwrap_or(0.0) as f32,
            passes: passes.len() as u32,
            balance_entry: per_phase[0],
            balance_mid: per_phase[1],
            balance_exit: per_phase[2],
        });
    }
    let cell = |phase: CornerPhase, speed: CornerSpeed| -> BalanceCell {
        let v: Vec<&PassValue> = values
            .iter()
            .filter(|p| p.phase == phase && p.speed == speed)
            .collect();
        if v.is_empty() {
            return BalanceCell::default();
        }
        let index = metrics::median(v.iter().map(|p| p.index)).unwrap_or(0.0);
        let raw_index = metrics::median(v.iter().map(|p| p.raw)).unwrap_or(0.0);
        let agreement = metrics::mean(v.iter().map(|p| p.agreement)).unwrap_or(0.5);
        let samples: usize = v.iter().map(|p| p.samples).sum();
        let laps_factor = if n_laps >= MIN_LAPS_FULL_CONFIDENCE {
            1.0
        } else {
            0.5
        };
        let channel = if has_slip { 1.0 } else { 0.8 };
        BalanceCell {
            understeer_index: index as f32,
            raw_index: raw_index as f32,
            samples: samples as u32,
            confidence: (coverage(v.len()) * agreement * laps_factor * channel).clamp(0.0, 1.0)
                as f32,
        }
    };
    let row = |phase: CornerPhase| BalanceRow {
        slow: cell(phase, CornerSpeed::Slow),
        medium: cell(phase, CornerSpeed::Medium),
        fast: cell(phase, CornerSpeed::Fast),
    };
    let per_phase = BalanceMatrix {
        entry: row(CornerPhase::Entry),
        mid: row(CornerPhase::Mid),
        exit: row(CornerPhase::Exit),
    };
    let cell_passes: Vec<(CornerPhase, CornerSpeed, usize)> = cells(&per_phase)
        .into_iter()
        .map(|(phase, speed, _)| {
            let n = values
                .iter()
                .filter(|p| p.phase == phase && p.speed == speed)
                .count();
            (phase, speed, n)
        })
        .collect();
    let overall_fr = describe_balance(
        &per_phase,
        &cell_passes,
        laps.is_empty(),
        reference.as_ref(),
        raw_passes.len(),
    );
    (
        BalanceAnalysis {
            corners: stats,
            per_phase,
            reference,
            overall_fr,
        },
        cell_passes,
    )
}

/// Passes of one cell of the matrix (0 when absent).
pub fn passes_of(
    cell_passes: &[(CornerPhase, CornerSpeed, usize)],
    phase: CornerPhase,
    speed: CornerSpeed,
) -> usize {
    cell_passes
        .iter()
        .find(|(p, s, _)| *p == phase && *s == speed)
        .map_or(0, |x| x.2)
}

/// A cell can become a finding: enough passes, beyond the trigger.
pub fn cell_stands_out(cell: &BalanceCell, passes: usize) -> bool {
    cell.samples > 0
        && passes >= MIN_PASSES_FOR_FINDING
        && cell.understeer_index.abs() as f64 > metrics::BALANCE_TRIGGER_INDEX
}

pub fn phase_fr(p: CornerPhase) -> &'static str {
    match p {
        CornerPhase::Entry => "entrée",
        CornerPhase::Mid => "milieu",
        CornerPhase::Exit => "sortie",
    }
}

pub fn phase_en(p: CornerPhase) -> &'static str {
    match p {
        CornerPhase::Entry => "entry",
        CornerPhase::Mid => "mid-corner",
        CornerPhase::Exit => "exit",
    }
}

pub fn speed_fr(s: CornerSpeed) -> &'static str {
    match s {
        CornerSpeed::Slow => "lents",
        CornerSpeed::Medium => "moyens",
        CornerSpeed::Fast => "rapides",
    }
}

pub fn speed_en(s: CornerSpeed) -> &'static str {
    match s {
        CornerSpeed::Slow => "slow",
        CornerSpeed::Medium => "medium",
        CornerSpeed::Fast => "fast",
    }
}

/// Every cell of the matrix as `(phase, speed, cell)`.
pub fn cells(m: &BalanceMatrix) -> Vec<(CornerPhase, CornerSpeed, &BalanceCell)> {
    let mut out = Vec::with_capacity(9);
    for (phase, row) in [
        (CornerPhase::Entry, &m.entry),
        (CornerPhase::Mid, &m.mid),
        (CornerPhase::Exit, &m.exit),
    ] {
        out.push((phase, CornerSpeed::Slow, &row.slow));
        out.push((phase, CornerSpeed::Medium, &row.medium));
        out.push((phase, CornerSpeed::Fast, &row.fast));
    }
    out
}

fn describe_balance(
    m: &BalanceMatrix,
    cell_passes: &[(CornerPhase, CornerSpeed, usize)],
    empty: bool,
    reference: Option<&BalanceReference>,
    raw_passes: usize,
) -> String {
    if empty {
        return "Pas de données d'équilibre.".into();
    }
    if reference.is_none() {
        return if raw_passes == 0 {
            "Aucun virage détecté : équilibre non évalué.".into()
        } else {
            format!(
                "Trop peu de passages en virage ({raw_passes}, il en faut {}) pour établir ta référence de la session : équilibre non évalué.",
                metrics::MIN_REFERENCE_PASSES
            )
        };
    }
    let mut marked: Vec<String> = cells(m)
        .into_iter()
        .filter(|(p, s, c)| cell_stands_out(c, passes_of(cell_passes, *p, *s)))
        .map(|(p, s, c)| {
            format!(
                "plus {} que d'habitude en {} de virages {} ({:+.2})",
                if c.understeer_index > 0.0 {
                    "sous-vireuse"
                } else {
                    "survireuse"
                },
                phase_fr(p),
                speed_fr(s),
                c.understeer_index
            )
        })
        .collect();
    if marked.is_empty() {
        if cells(m).iter().all(|(_, _, c)| c.samples == 0) {
            "Aucun virage détecté : équilibre non évalué.".into()
        } else {
            "Par rapport à ta référence de la session, aucune phase ni plage de vitesse ne se détache : équilibre homogène sur les virages détectés.".into()
        }
    } else {
        marked.truncate(4);
        format!(
            "Par rapport à ta référence de la session, la voiture est {}.",
            marked.join(" ; ")
        )
    }
}
