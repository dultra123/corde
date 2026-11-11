//! Loads a recorded session from [`corde_store::Store`] into [`LapSeries`].
//!
//! # Which laps are analysed
//!
//! A lap is kept when all of this holds (see [`classify`]):
//!
//! 1. the game timed it (`valid`): not an in-lap, an out-lap, a lap the game
//!    refused to time (track limits) or an abandoned lap;
//! 2. it is not the first lap of a race (standing / rolling start, traffic);
//! 3. it is within [`MAX_PACE_RATIO`] of the best lap of the session (a spin or
//!    heavy traffic would blur the diagnosis);
//! 4. its samples are there: at least [`MIN_COVERAGE`] of what the other laps
//!    of the session hold per second (a lap started before the recorder was
//!    armed is dropped that way).
//!
//! The reasons for skipping laps are reported in `SessionMeta::notes`, in
//! French, with the game's lap numbers (1-based).
use crate::error::{EngineerError, Result};
use crate::series::{LapSeries, SessionMeta, WHEEL_SUFFIXES};
use corde_protocol::LapNote;
use corde_store::{LapRow, Store};

/// Minimum share of the expected samples for a lap to be analysed.
pub const MIN_COVERAGE: f64 = 0.6;

/// Scalar channels read for every lap (`t_s` and `lap_dist_m` always come back
/// as axes and are stored as columns too).
pub const SCALAR_CHANNELS: &[&str] = &[
    "speed_kmh",
    "throttle",
    "brake",
    "throttle_raw",
    "brake_raw",
    "steering",
    "steering_angle_deg",
    "g_lat",
    "g_long",
    "g_vert",
    "yaw_rate_dps",
    "fuel_l",
    "virtual_energy",
    "brake_bias_rear",
    "tc_active",
    "abs_active",
    "front_ride_height_mm",
    "rear_ride_height_mm",
    "rain",
    "wetness_avg",
];

/// Per-wheel channels read for every lap (suffixed `_fl _fr _rl _rr`).
pub const WHEEL_CHANNELS: &[&str] = &[
    "susp_deflection_m",
    "ride_height_m",
    "brake_temp_c",
    "grip_fract",
    "pressure_kpa",
    "temp_l_c",
    "temp_c_c",
    "temp_r_c",
    "carcass_c",
    // Absent from sessions recorded before S2: read as NaN (unknown).
    "optimal_temp_c",
    "wear_remaining",
    "slip_ratio",
    "slip_angle_deg",
    "surface",
    "load_n",
];

/// Every channel name requested from the store.
pub fn channel_names() -> Vec<String> {
    let mut names: Vec<String> = SCALAR_CHANNELS.iter().map(|s| s.to_string()).collect();
    for base in WHEEL_CHANNELS {
        for suffix in WHEEL_SUFFIXES {
            names.push(format!("{base}{suffix}"));
        }
    }
    names
}

/// A lap slower than this × the best lap of the session is not analysed.
pub const MAX_PACE_RATIO: f32 = 1.07;

/// Why a lap is not analysed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Skip {
    /// The game did not time it (track limits, cut).
    NotTimed,
    /// In-lap or out-lap.
    PitLap,
    /// Abandoned (return to the garage).
    Aborted,
    /// First lap of a race.
    StartLap,
    /// More than [`MAX_PACE_RATIO`] × the best lap.
    TooSlow,
    /// Too few samples on disk.
    FewSamples,
}

impl Skip {
    /// French label for `n` laps.
    fn label_fr(self, n: usize) -> String {
        let s = if n > 1 { "s" } else { "" };
        match self {
            Skip::NotTimed => {
                format!("{n} non chronométré{s} par le jeu (limites de piste)")
            }
            Skip::PitLap => format!("{n} tour{s} d'entrée / sortie des stands"),
            Skip::Aborted => format!("{n} abandonné{s} (retour au garage)"),
            Skip::StartLap => format!("{n} tour{s} de départ de course"),
            Skip::TooSlow => format!(
                "{n} à plus de {:.0} % du meilleur tour (trafic, erreur)",
                MAX_PACE_RATIO * 100.0
            ),
            Skip::FewSamples => format!("{n} sans assez de données enregistrées"),
        }
    }
}

/// What a lap is compared against.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LapContext {
    /// Samples per second of lap the session really holds (median over its
    /// timed laps), never above the nominal recording rate.
    pub rows_per_s: f64,
    /// Best timed flying lap of the session.
    pub best_s: Option<f32>,
}

impl LapContext {
    pub fn of(laps: &[LapRow], record_rate_hz: f32) -> Self {
        let flying =
            |l: &&LapRow| l.lap.valid && !l.lap.in_lap && !l.lap.out_lap && l.lap.lap_time_s > 0.0;
        let mut rates: Vec<f64> = laps
            .iter()
            .filter(flying)
            .map(|l| {
                (l.sample_row_end - l.sample_row_start).max(0) as f64 / l.lap.lap_time_s as f64
            })
            .filter(|r| *r > 0.0)
            .collect();
        rates.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let nominal = f64::from(record_rate_hz.max(1.0));
        let rows_per_s = match rates.len() {
            0 => nominal,
            n => rates[n / 2].min(nominal),
        };
        let best_s = laps
            .iter()
            .filter(flying)
            .filter(|l| l.lap.note != Some(LapNote::StartLap))
            .map(|l| l.lap.lap_time_s)
            .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        Self { rows_per_s, best_s }
    }
}

/// `None` when the lap is analysed, else why it is not.
pub fn classify(lap: &LapRow, ctx: &LapContext) -> Option<Skip> {
    let r = &lap.lap;
    if r.note == Some(LapNote::Aborted) {
        return Some(Skip::Aborted);
    }
    if r.in_lap || r.out_lap {
        return Some(Skip::PitLap);
    }
    if !r.valid || r.lap_time_s <= 0.0 {
        return Some(Skip::NotTimed);
    }
    if r.note == Some(LapNote::StartLap) {
        return Some(Skip::StartLap);
    }
    if ctx
        .best_s
        .is_some_and(|b| r.lap_time_s > b * MAX_PACE_RATIO)
    {
        return Some(Skip::TooSlow);
    }
    let rows = (lap.sample_row_end - lap.sample_row_start).max(0) as f64;
    if rows < MIN_COVERAGE * r.lap_time_s as f64 * ctx.rows_per_s {
        return Some(Skip::FewSamples);
    }
    None
}

/// French summary of the laps kept and skipped, with the game's lap numbers.
pub fn selection_note(laps: &[LapRow], ctx: &LapContext) -> String {
    let mut skipped: std::collections::BTreeMap<Skip, Vec<i32>> = Default::default();
    let mut kept = 0usize;
    for l in laps {
        match classify(l, ctx) {
            Some(why) => skipped.entry(why).or_default().push(l.lap.lap_number + 1),
            None => kept += 1,
        }
    }
    let mut note = format!("Tours retenus pour l'analyse : {kept} sur {}.", laps.len());
    if !skipped.is_empty() {
        let parts: Vec<String> = skipped
            .iter()
            .map(|(why, nums)| {
                let list = nums
                    .iter()
                    .map(|n| n.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                format!(
                    "{} — tour{} {list}",
                    why.label_fr(nums.len()),
                    if nums.len() > 1 { "s" } else { "" }
                )
            })
            .collect();
        note.push_str(" Écartés : ");
        note.push_str(&parts.join(" ; "));
        note.push('.');
    }
    note
}

/// Channels that hold codes or flags: never interpolated.
fn is_discrete(name: &str) -> bool {
    name.starts_with("surface") || name == "tc_active" || name == "abs_active"
}

/// `true` when the time axis has holes or bursts (sessions recorded with the
/// old sample clock, once repaired by the store).
pub fn is_uneven(t: &[f64]) -> bool {
    let mut d: Vec<f64> = t
        .windows(2)
        .map(|w| w[1] - w[0])
        .filter(|d| d.is_finite() && *d > 0.0)
        .collect();
    if d.len() < 10 {
        return false;
    }
    d.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median = d[d.len() / 2];
    let p95 = d[(d.len() * 95) / 100];
    p95 > 2.5 * median
}

/// Resamples every column of `lap` on a regular time grid of `step_s`
/// (linear interpolation; previous value for flags and codes).
pub fn resample_uniform(lap: &mut LapSeries, step_s: f64) {
    let t: Vec<f64> = lap.t().to_vec();
    let n = t.len();
    if n < 2 || step_s <= 0.0 || !t.iter().all(|v| v.is_finite()) {
        return;
    }
    let (t0, t1) = (t[0], t[n - 1]);
    let m = ((t1 - t0) / step_s).floor() as usize + 1;
    if m < 2 {
        return;
    }
    // For each grid point: index `j` with t[j] <= x < t[j + 1], and the weight.
    let mut at: Vec<(usize, f64)> = Vec::with_capacity(m);
    let mut j = 0usize;
    for k in 0..m {
        let x = t0 + step_s * k as f64;
        while j + 2 < n && t[j + 1] <= x {
            j += 1;
        }
        let span = t[j + 1] - t[j];
        let w = if span > 0.0 {
            ((x - t[j]) / span).clamp(0.0, 1.0)
        } else {
            0.0
        };
        at.push((j, w));
    }
    let names: Vec<String> = lap.names().map(str::to_string).collect();
    let mut resampled: Vec<(String, Vec<f64>)> = Vec::with_capacity(names.len());
    for name in names {
        let Some(col) = lap.col(&name).map(<[f64]>::to_vec) else {
            continue;
        };
        if col.len() != n {
            continue;
        }
        let out: Vec<f64> = if name == "t_s" {
            (0..m).map(|k| t0 + step_s * k as f64).collect()
        } else if is_discrete(&name) {
            at.iter()
                .map(|&(j, w)| if w >= 1.0 { col[j + 1] } else { col[j] })
                .collect()
        } else {
            at.iter()
                .map(
                    |&(j, w)| match (col[j].is_finite(), col[j + 1].is_finite()) {
                        (true, true) => col[j] + (col[j + 1] - col[j]) * w,
                        (true, false) => col[j],
                        (false, true) => col[j + 1],
                        (false, false) => f64::NAN,
                    },
                )
                .collect()
        };
        resampled.push((name, out));
    }
    lap.replace_columns(m, resampled);
}
/// What [`load_session`] returns.
#[derive(Debug)]
pub struct Loaded {
    /// Session facts; `meta.notes` says what was skipped.
    pub meta: SessionMeta,
    /// Clean laps with their samples, in lap order.
    pub laps: Vec<LapSeries>,
}

/// Read the session, keep the clean laps and load their samples.
pub fn load_session(store: &Store, session_id: &str) -> Result<Loaded> {
    let detail = store.get_session(session_id)?;
    let s = &detail.session;
    let mut meta = SessionMeta {
        session_id: s.id.clone(),
        track_name: s.track_name.clone(),
        track_length_m: s.track_length_m,
        vehicle_name: s.vehicle_name.clone(),
        vehicle_class: s.vehicle_class,
        record_rate_hz: s.record_rate_hz,
        setup_known: !detail.setups.is_empty(),
        weather: s.weather.clone(),
        all_laps: detail.laps.iter().map(|l| l.lap.clone()).collect(),
        notes: Vec::new(),
    };
    let ctx = LapContext::of(&detail.laps, s.record_rate_hz);
    let mut notes = vec![selection_note(&detail.laps, &ctx)];
    let mut resampled = false;
    let names = channel_names();
    let refs: Vec<&str> = names.iter().map(String::as_str).collect();
    let mut laps = Vec::new();
    for row in detail.laps.iter().filter(|l| classify(l, &ctx).is_none()) {
        let samples = match store.lap_samples(
            session_id,
            row.lap.lap_number,
            &refs,
            corde_store::query::MAX_MAX_POINTS,
        ) {
            Ok(s) => s,
            Err(e) => {
                notes.push(format!(
                    "Tour {} : échantillons illisibles ({e}), ignoré.",
                    row.lap.lap_number + 1
                ));
                continue;
            }
        };
        if samples.points < 10 {
            notes.push(format!(
                "Tour {} : {} échantillons seulement, ignoré.",
                row.lap.lap_number + 1,
                samples.points
            ));
            continue;
        }
        let mut lap = LapSeries::new(row.lap.lap_number, row.lap.clone());
        lap.set("t_s", samples.t_s);
        lap.set("lap_dist_m", samples.lap_dist_m);
        for ch in samples.channels {
            let values: Vec<f64> = ch
                .values
                .into_iter()
                .map(|v| v.unwrap_or(f64::NAN))
                .collect();
            lap.set(ch.name, values);
        }
        if is_uneven(lap.t()) {
            // Recorded with the old sample clock: put the lap on a regular grid.
            resample_uniform(
                &mut lap,
                1.0 / f64::from(s.record_rate_hz.clamp(10.0, 200.0)),
            );
            resampled = true;
        }
        laps.push(lap);
    }
    if resampled {
        notes.push(
            "Session enregistrée avec l'ancienne horloge d'échantillonnage (version alpha 1) : \
             les données ont été recalées sur une grille régulière, précision un peu moindre."
                .to_string(),
        );
    }
    if laps.is_empty() {
        return Err(EngineerError::NotEnoughData(format!(
            "aucun tour exploitable dans la session {session_id} — {}",
            selection_note(&detail.laps, &ctx)
        )));
    }
    meta.notes = notes;
    Ok(Loaded { meta, laps })
}

#[cfg(test)]
mod tests {
    use super::*;
    use corde_protocol::LapRecord;

    fn row(valid: bool, in_lap: bool, out_lap: bool, time: f32, rows: i64) -> LapRow {
        LapRow {
            session_id: "s".into(),
            lap: LapRecord {
                lap_number: 1,
                lap_time_s: time,
                valid,
                in_lap,
                out_lap,
                ..Default::default()
            },
            sample_file: "samples-000.parquet".into(),
            sample_row_start: 0,
            sample_row_end: rows,
            time_source: "live".into(),
        }
    }

    #[test]
    fn lap_selection_and_reasons() {
        let ctx = LapContext {
            rows_per_s: 50.0,
            best_s: Some(100.0),
        };
        assert_eq!(classify(&row(true, false, false, 100.0, 5000), &ctx), None);
        // 40 % of the samples.
        assert_eq!(
            classify(&row(true, false, false, 100.0, 2000), &ctx),
            Some(Skip::FewSamples)
        );
        assert_eq!(
            classify(&row(false, false, false, 100.0, 5000), &ctx),
            Some(Skip::NotTimed)
        );
        assert_eq!(
            classify(&row(false, true, false, 100.0, 5000), &ctx),
            Some(Skip::PitLap)
        );
        assert_eq!(
            classify(&row(false, false, true, 100.0, 5000), &ctx),
            Some(Skip::PitLap)
        );
        assert_eq!(
            classify(&row(true, false, false, 0.0, 5000), &ctx),
            Some(Skip::NotTimed)
        );
        // A spin: 8 % off the best lap.
        assert_eq!(
            classify(&row(true, false, false, 108.0, 5400), &ctx),
            Some(Skip::TooSlow)
        );
        assert_eq!(classify(&row(true, false, false, 106.9, 5300), &ctx), None);
        let mut start = row(true, false, false, 104.0, 5200);
        start.lap.note = Some(LapNote::StartLap);
        assert_eq!(classify(&start, &ctx), Some(Skip::StartLap));
        let mut aborted = row(false, false, false, 12.0, 600);
        aborted.lap.note = Some(LapNote::Aborted);
        assert_eq!(classify(&aborted, &ctx), Some(Skip::Aborted));
    }

    #[test]
    fn coverage_is_judged_against_what_the_session_really_holds() {
        // A session recorded at an effective 30 Hz (old sample clock) for a
        // nominal 50 Hz: every lap holds 60 % of the nominal count and must
        // still be analysed — this used to reject clean laps at random.
        let mut laps: Vec<LapRow> = (0..6)
            .map(|i| {
                let mut r = row(true, false, false, 83.0 + i as f32 * 0.2, 2490 + i * 6);
                r.lap.lap_number = i as i32;
                r
            })
            .collect();
        // One lap really is short of samples (recorder armed mid-lap).
        laps[5].sample_row_end = 900;
        let ctx = LapContext::of(&laps, 50.0);
        assert!((ctx.rows_per_s - 30.0).abs() < 0.5, "{}", ctx.rows_per_s);
        assert_eq!(ctx.best_s, Some(83.0));
        let kept: Vec<i32> = laps
            .iter()
            .filter(|l| classify(l, &ctx).is_none())
            .map(|l| l.lap.lap_number)
            .collect();
        assert_eq!(kept, vec![0, 1, 2, 3, 4]);
        let note = selection_note(&laps, &ctx);
        assert!(
            note.starts_with("Tours retenus pour l'analyse : 5 sur 6."),
            "{note}"
        );
        assert!(note.contains("tour 6"), "game numbering is 1-based: {note}");
    }

    #[test]
    fn uneven_laps_are_resampled_on_a_regular_grid() {
        // Bursts of 5 samples 10 ms apart, then a 60 ms hole.
        let mut t = Vec::new();
        for burst in 0..40 {
            for j in 0..5 {
                t.push(burst as f64 * 0.1 + j as f64 * 0.01);
            }
        }
        assert!(is_uneven(&t));
        let regular: Vec<f64> = (0..200).map(|i| i as f64 * 0.02).collect();
        assert!(!is_uneven(&regular));

        let mut lap = LapSeries::new(1, LapRecord::default());
        lap.set("speed_kmh", t.iter().map(|x| 100.0 + 10.0 * x).collect());
        lap.set(
            "abs_active",
            t.iter()
                .map(|x| if *x >= 2.0 { 1.0 } else { 0.0 })
                .collect(),
        );
        lap.set("t_s", t.clone());
        resample_uniform(&mut lap, 0.02);
        let rt = lap.t().to_vec();
        assert!(!is_uneven(&rt));
        assert!((rt[1] - rt[0] - 0.02).abs() < 1e-12);
        assert!((lap.dt() - 0.02).abs() < 1e-9);
        let v = lap.col("speed_kmh").unwrap();
        assert_eq!(v.len(), rt.len());
        for (x, y) in rt.iter().zip(v) {
            assert!(
                (y - (100.0 + 10.0 * x)).abs() < 1e-6,
                "linear data must survive: {x} {y}"
            );
        }
        let flags = lap.col("abs_active").unwrap();
        assert!(
            flags.iter().all(|f| *f == 0.0 || *f == 1.0),
            "flags are never interpolated"
        );
        assert_eq!(flags[0], 0.0);
        assert_eq!(*flags.last().unwrap(), 1.0);
    }

    #[test]
    fn channel_names_are_real_store_channels() {
        for n in channel_names() {
            assert!(
                corde_store::channels::channel(&n).is_some(),
                "unknown channel {n}"
            );
        }
    }
}
