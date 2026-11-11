//! The numbers: balance estimators, lock-ups and wheel-spin, tyre / brake /
//! suspension summaries, damper histograms, driving-style indicators and lap
//! statistics. Everything is `f64` inside and becomes `f32` in the protocol
//! types. Formulas and thresholds are documented in `docs/ENGINEER.md`.

use crate::corners::Corner;
use crate::series::LapSeries;
use corde_protocol::{
    BrakeWheelAnalysis, BrakeWindow, CamberHint, CornerPhase, CornerSpeed, DamperHist,
    DrivingStyle, LapRecord, LapStats, SurfaceType, TyreWheelAnalysis, VehicleClass,
};

// ---------------------------------------------------------------------------
// Small statistics helpers
// ---------------------------------------------------------------------------

pub fn mean(it: impl IntoIterator<Item = f64>) -> Option<f64> {
    let (mut s, mut k) = (0.0, 0usize);
    for v in it {
        if v.is_finite() {
            s += v;
            k += 1;
        }
    }
    (k > 0).then(|| s / k as f64)
}

pub fn median(it: impl IntoIterator<Item = f64>) -> Option<f64> {
    let mut v: Vec<f64> = it.into_iter().filter(|x| x.is_finite()).collect();
    if v.is_empty() {
        return None;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let m = v.len() / 2;
    Some(if v.len() % 2 == 0 {
        (v[m - 1] + v[m]) / 2.0
    } else {
        v[m]
    })
}

pub fn std_dev(it: impl IntoIterator<Item = f64>) -> Option<f64> {
    let v: Vec<f64> = it.into_iter().filter(|x| x.is_finite()).collect();
    if v.len() < 2 {
        return None;
    }
    let m = v.iter().sum::<f64>() / v.len() as f64;
    let var = v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (v.len() - 1) as f64;
    Some(var.sqrt())
}

/// Least-squares slope of `y` against `x` (≥ 2 distinct x).
pub fn linear_slope(x: &[f64], y: &[f64]) -> Option<f64> {
    let n = x.len().min(y.len());
    if n < 2 {
        return None;
    }
    let mx = x[..n].iter().sum::<f64>() / n as f64;
    let my = y[..n].iter().sum::<f64>() / n as f64;
    let sxx: f64 = x[..n].iter().map(|v| (v - mx).powi(2)).sum();
    if sxx <= 0.0 {
        return None;
    }
    let sxy: f64 = x[..n]
        .iter()
        .zip(&y[..n])
        .map(|(a, b)| (a - mx) * (b - my))
        .sum();
    Some(sxy / sxx)
}

fn fin(v: f64) -> f64 {
    if v.is_finite() {
        v
    } else {
        0.0
    }
}

fn at(col: Option<&[f64]>, i: usize) -> f64 {
    col.and_then(|c| c.get(i).copied()).map(fin).unwrap_or(0.0)
}

/// Moving average over `w` samples (odd, ≥ 1), NaN-tolerant.
pub fn smooth(x: &[f64], w: usize) -> Vec<f64> {
    let w = w.max(1);
    let h = w / 2;
    (0..x.len())
        .map(|i| {
            let lo = i.saturating_sub(h);
            let hi = (i + h + 1).min(x.len());
            mean(x[lo..hi].iter().copied()).unwrap_or(f64::NAN)
        })
        .collect()
}

/// A sustained condition: `[start, end)` sample ranges where `cond` held for
/// at least `min_s` seconds.
#[derive(Debug, Clone, PartialEq)]
pub struct Event {
    pub start: usize,
    pub end: usize,
}

pub fn sustained(t: &[f64], min_s: f64, cond: impl Fn(usize) -> bool) -> Vec<Event> {
    let mut out = Vec::new();
    let mut start: Option<usize> = None;
    for i in 0..t.len() {
        let on = cond(i);
        match (start, on) {
            (None, true) => start = Some(i),
            (Some(s), false) => {
                if t[i - 1] - t[s] >= min_s {
                    out.push(Event { start: s, end: i });
                }
                start = None;
            }
            _ => {}
        }
    }
    if let Some(s) = start {
        if t.len() > s + 1 && t[t.len() - 1] - t[s] >= min_s {
            out.push(Event {
                start: s,
                end: t.len(),
            });
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Balance (under- / oversteer)
// ---------------------------------------------------------------------------

/// Wheelbase used by the kinematic estimator, metres.
pub fn wheelbase_m(class: VehicleClass) -> f64 {
    match class {
        VehicleClass::Hypercar => 3.1,
        VehicleClass::Lmp2 | VehicleClass::Lmp3 => 3.0,
        VehicleClass::Gt3 | VehicleClass::Gte => 2.7,
        _ => 2.8,
    }
}

/// Lateral g above which a sample counts as "loaded".
pub const LOADED_G: f64 = 0.3;
/// Slip-angle difference (front − rear) read as understeer, degrees.
pub const SLIP_THRESHOLD_DEG: f64 = 1.0;
/// Same in fast corners.
pub const SLIP_THRESHOLD_FAST_DEG: f64 = 0.5;
/// Kinematic understeer gradient read as strong understeer, °/g.
pub const GRADIENT_SCALE_DEG_PER_G: f64 = 3.0;
/// Steering ratio when it cannot be estimated from the data.
pub const DEFAULT_STEERING_RATIO: f64 = 12.0;

/// Smallest `|α_front| + |α_rear|` (degrees) the slip-angle balance ratio is
/// computed on (below, the ratio is noise).
pub const SLIP_BALANCE_MIN_SUM_DEG: f64 = 0.4;
/// Floor of the robust spread of the slip-angle balance ratio across the
/// passes of a session (a perfectly uniform session would otherwise turn
/// noise into findings).
pub const SLIP_BALANCE_SIGMA_FLOOR: f64 = 0.05;
/// Same for the kinematic gradient, °/g.
pub const GRADIENT_SIGMA_FLOOR_DEG_PER_G: f64 = 0.5;
/// Deviation from the session reference, in robust standard deviations,
/// that saturates the relative index at ±1.
pub const BALANCE_SIGMAS_FULL_SCALE: f64 = 5.0;
/// Relative index beyond which a cell is reported (0.3 × 5 = 1.5 σ).
pub const BALANCE_TRIGGER_INDEX: f64 = 0.3;
/// Relative index beyond which a balance finding is major (3 σ).
pub const BALANCE_MAJOR_INDEX: f64 = 0.6;
/// Fewer (corner, phase, lap) passes than this: no session reference.
pub const MIN_REFERENCE_PASSES: usize = 5;
/// `1.4826 × MAD` estimates the standard deviation of a normal distribution.
pub const MAD_TO_SIGMA: f64 = 1.4826;

/// Raw per-sample balance estimators of one lap (NaN where not computable).
#[derive(Debug, Clone, Default)]
pub struct BalanceSeries {
    /// `|α_front| − |α_rear|`, degrees (positive = understeer).
    pub slip_delta_deg: Vec<f64>,
    /// `(|α_front| − |α_rear|) / (|α_front| + |α_rear|)`, −1..1 (positive =
    /// understeer): the slip-angle balance without the load level, which
    /// multiplies both angles (real sessions: `slip_delta_deg` × 5–7 from
    /// 0.3 g to 1.5 g, this ratio flat).
    pub slip_balance: Vec<f64>,
    /// Kinematic understeer gradient `(δ − L·ψ̇/V) / |g_lat|`, °/g (positive = understeer).
    pub gradient_deg_per_g: Vec<f64>,
    pub has_slip: bool,
    pub has_gradient: bool,
    /// Steering ratio used by the gradient estimator.
    pub steering_ratio: f64,
}

/// Steady-state estimate of the steering ratio on gently loaded samples
/// (`0.2 < |g_lat| < 0.5`): `i_s ≈ SWA / (L·ψ̇/V)`.
pub fn estimate_steering_ratio(lap: &LapSeries, wheelbase_m: f64) -> Option<f64> {
    let swa = lap.col("steering_angle_deg")?;
    let yaw = lap.col("yaw_rate_dps")?;
    let speed = lap.col("speed_kmh")?;
    let g = lap.col("g_lat")?;
    let ratios = (0..lap.n).filter_map(|i| {
        let gl = fin(g[i]).abs();
        let v = fin(speed[i]) / 3.6;
        if !(0.2..0.5).contains(&gl) || v < 10.0 {
            return None;
        }
        let ack = (wheelbase_m * fin(yaw[i]).to_radians() / v).to_degrees();
        if ack.abs() < 0.2 {
            return None;
        }
        let r = fin(swa[i]) / ack;
        (r.is_finite() && (3.0..40.0).contains(&r)).then_some(r)
    });
    let v: Vec<f64> = ratios.collect();
    if v.len() < 20 {
        None
    } else {
        median(v)
    }
}

/// Both estimators for one lap.
pub fn balance_series(
    lap: &LapSeries,
    class: VehicleClass,
    steering_ratio: Option<f64>,
) -> BalanceSeries {
    let n = lap.n;
    let g = lap.col("g_lat");
    let mut out = BalanceSeries {
        slip_delta_deg: vec![f64::NAN; n],
        slip_balance: vec![f64::NAN; n],
        gradient_deg_per_g: vec![f64::NAN; n],
        has_slip: false,
        has_gradient: false,
        steering_ratio: steering_ratio.unwrap_or(DEFAULT_STEERING_RATIO),
    };
    let Some(g) = g else { return out };

    let slip: Option<[&[f64]; 4]> = (0..4)
        .map(|w| lap.wheel("slip_angle_deg", w))
        .collect::<Option<Vec<_>>>()
        .and_then(|v| v.try_into().ok());
    if let Some(sa) = slip {
        let live = sa
            .iter()
            .any(|c| c.iter().any(|v| v.is_finite() && v.abs() > 0.05));
        if live {
            out.has_slip = true;
            for i in 0..n {
                if fin(g[i]).abs() < LOADED_G {
                    continue;
                }
                let f = (fin(sa[0][i]).abs() + fin(sa[1][i]).abs()) / 2.0;
                let r = (fin(sa[2][i]).abs() + fin(sa[3][i]).abs()) / 2.0;
                out.slip_delta_deg[i] = f - r;
                if f + r >= SLIP_BALANCE_MIN_SUM_DEG {
                    out.slip_balance[i] = (f - r) / (f + r);
                }
            }
        }
    }

    if let (Some(swa), Some(yaw), Some(speed)) = (
        lap.col("steering_angle_deg"),
        lap.col("yaw_rate_dps"),
        lap.col("speed_kmh"),
    ) {
        let live = swa.iter().any(|v| v.is_finite() && v.abs() > 1.0)
            && yaw.iter().any(|v| v.is_finite() && v.abs() > 1.0);
        if live {
            out.has_gradient = true;
            let wb = wheelbase_m(class);
            for i in 0..n {
                let gl = fin(g[i]);
                let v = fin(speed[i]) / 3.6;
                if gl.abs() < LOADED_G || v < 10.0 {
                    continue;
                }
                let delta = fin(swa[i]) / out.steering_ratio;
                let ack = (wb * fin(yaw[i]).to_radians() / v).to_degrees();
                // Signed so that positive = understeer whichever way the car turns.
                let us = (delta - ack) * gl.signum();
                out.gradient_deg_per_g[i] = us / gl.abs();
            }
        }
    }
    out
}

/// Normalise the raw estimators of one (corner, phase) pass into the
/// **absolute** index in −1..+1 (fixed scales: slip-angle difference over
/// twice [`SLIP_THRESHOLD_DEG`], gradient over [`GRADIENT_SCALE_DEG_PER_G`])
/// and an agreement weight (1.0 both agree, 0.7 one estimator, 0.3
/// contradictory). Only reported as `raw_index`: on real sessions it is
/// positive for every car (front slip angles naturally larger), the
/// diagnosis uses [`relative_balance`].
pub fn normalise_balance(
    slip_delta_deg: Option<f64>,
    gradient_deg_per_g: Option<f64>,
    speed_class: CornerSpeed,
) -> Option<(f64, f64)> {
    let thr = if speed_class == CornerSpeed::Fast {
        SLIP_THRESHOLD_FAST_DEG
    } else {
        SLIP_THRESHOLD_DEG
    };
    let s = slip_delta_deg.map(|d| (d / (2.0 * thr)).clamp(-1.0, 1.0));
    let k = gradient_deg_per_g.map(|k| (k / GRADIENT_SCALE_DEG_PER_G).clamp(-1.0, 1.0));
    match (s, k) {
        (Some(s), Some(k)) => {
            let agree = (s.signum() == k.signum()) || (s.abs() < 0.3 && k.abs() < 0.3);
            Some((0.6 * s + 0.4 * k, if agree { 1.0 } else { 0.3 }))
        }
        (Some(s), None) => Some((s, 0.7)),
        (None, Some(k)) => Some((k, 0.7)),
        (None, None) => None,
    }
}

/// The raw estimators averaged over the samples of one phase of one corner.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct PhaseRaw {
    /// `|α_front| − |α_rear|`, degrees.
    pub slip_delta_deg: Option<f64>,
    /// `(|α_front| − |α_rear|) / (|α_front| + |α_rear|)`.
    pub slip_balance: Option<f64>,
    /// Kinematic gradient, °/g.
    pub gradient_deg_per_g: Option<f64>,
    /// Samples with at least one estimator.
    pub samples: usize,
}

/// Mean of the raw estimators over the samples of `phase` of one corner.
pub fn corner_phase_raw(bal: &BalanceSeries, corner: &Corner, phase: CornerPhase) -> PhaseRaw {
    let idx: Vec<usize> = (corner.start..corner.end)
        .filter(|&i| corner.phase_at(i) == Some(phase))
        .collect();
    let samples = idx
        .iter()
        .filter(|&&i| bal.slip_delta_deg[i].is_finite() || bal.gradient_deg_per_g[i].is_finite())
        .count();
    PhaseRaw {
        slip_delta_deg: mean(idx.iter().map(|&i| bal.slip_delta_deg[i])),
        slip_balance: mean(idx.iter().map(|&i| bal.slip_balance[i])),
        gradient_deg_per_g: mean(idx.iter().map(|&i| bal.gradient_deg_per_g[i])),
        samples,
    }
}

/// Robust centre and spread of one estimator over the passes of a session.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RobustRef {
    pub median: f64,
    /// `max(1.4826 × MAD, floor)`.
    pub sigma: f64,
    pub n: usize,
}

impl RobustRef {
    /// `None` with fewer than [`MIN_REFERENCE_PASSES`] finite values.
    pub fn of(values: impl IntoIterator<Item = f64>, floor: f64) -> Option<Self> {
        let v: Vec<f64> = values.into_iter().filter(|x| x.is_finite()).collect();
        if v.len() < MIN_REFERENCE_PASSES {
            return None;
        }
        let m = median(v.iter().copied())?;
        let mad = median(v.iter().map(|x| (x - m).abs()))?;
        Some(Self {
            median: m,
            sigma: (MAD_TO_SIGMA * mad).max(floor),
            n: v.len(),
        })
    }

    /// Deviation from the centre in robust standard deviations.
    pub fn z(&self, x: f64) -> f64 {
        (x - self.median) / self.sigma
    }
}

/// The session reference of both estimators (each `None` when unavailable).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct BalanceRefs {
    pub slip: Option<RobustRef>,
    pub gradient: Option<RobustRef>,
}

impl BalanceRefs {
    /// References over every pass of the session.
    pub fn of(passes: &[PhaseRaw]) -> Self {
        Self {
            slip: RobustRef::of(
                passes.iter().filter_map(|p| p.slip_balance),
                SLIP_BALANCE_SIGMA_FLOOR,
            ),
            gradient: RobustRef::of(
                passes.iter().filter_map(|p| p.gradient_deg_per_g),
                GRADIENT_SIGMA_FLOOR_DEG_PER_G,
            ),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.slip.is_none() && self.gradient.is_none()
    }
}

/// Balance of one pass **relative to the session reference**: each estimator
/// becomes a deviation in robust standard deviations, the two are fused
/// (`0.6 × slip + 0.4 × gradient`) and scaled so that ±1 =
/// [`BALANCE_SIGMAS_FULL_SCALE`] σ. Returns `(index, agreement)` like
/// [`normalise_balance`]; `None` when no estimator has both a value and a
/// reference.
pub fn relative_balance(raw: &PhaseRaw, refs: &BalanceRefs) -> Option<(f64, f64)> {
    let zs = raw
        .slip_balance
        .zip(refs.slip)
        .map(|(v, r)| r.z(v) / BALANCE_SIGMAS_FULL_SCALE);
    let zk = raw
        .gradient_deg_per_g
        .zip(refs.gradient)
        .map(|(v, r)| r.z(v) / BALANCE_SIGMAS_FULL_SCALE);
    let quiet = |x: f64| x.abs() <= BALANCE_TRIGGER_INDEX;
    match (zs, zk) {
        (Some(s), Some(k)) => {
            let agree = s.signum() == k.signum() || (quiet(s) && quiet(k));
            Some((
                (0.6 * s + 0.4 * k).clamp(-1.0, 1.0),
                if agree { 1.0 } else { 0.3 },
            ))
        }
        (Some(s), None) => Some((s.clamp(-1.0, 1.0), 0.7)),
        (None, Some(k)) => Some((k.clamp(-1.0, 1.0), 0.7)),
        (None, None) => None,
    }
}

// ---------------------------------------------------------------------------
// Lock-ups and wheel-spin
// ---------------------------------------------------------------------------

pub const LOCK_SLIP: f64 = -0.15;
pub const SPIN_SLIP: f64 = 0.15;
pub const LOCK_MIN_S: f64 = 0.06;
pub const LOCK_MIN_BRAKE: f64 = 0.3;
pub const SPIN_MIN_THROTTLE: f64 = 0.5;
pub const LOCK_MIN_SPEED_KMH: f64 = 40.0;

/// Lock-up events of one wheel: `slip_ratio < −0.15` for ≥ 60 ms with the
/// brake above 30 % and more than 40 km/h.
pub fn lockups(lap: &LapSeries, wheel: usize) -> Vec<Event> {
    let Some(slip) = lap.wheel("slip_ratio", wheel) else {
        return Vec::new();
    };
    let brake = lap.col("brake_raw").or(lap.col("brake"));
    let speed = lap.col("speed_kmh");
    sustained(lap.t(), LOCK_MIN_S, |i| {
        fin(slip[i]) < LOCK_SLIP
            && at(brake, i) > LOCK_MIN_BRAKE
            && speed
                .map(|s| fin(s[i]) > LOCK_MIN_SPEED_KMH)
                .unwrap_or(true)
    })
}

/// Wheel-spin events of one wheel: `slip_ratio > 0.15` for ≥ 60 ms with the
/// throttle above 50 %.
pub fn wheelspins(lap: &LapSeries, wheel: usize) -> Vec<Event> {
    let Some(slip) = lap.wheel("slip_ratio", wheel) else {
        return Vec::new();
    };
    let throttle = lap.col("throttle_raw").or(lap.col("throttle"));
    sustained(lap.t(), LOCK_MIN_S, |i| {
        fin(slip[i]) > SPIN_SLIP && at(throttle, i) > SPIN_MIN_THROTTLE
    })
}

// ---------------------------------------------------------------------------
// Tyres
// ---------------------------------------------------------------------------

/// Inner / middle / outer temperatures from the game's left / centre / right
/// order: left wheels (FL, RL) have their inner shoulder on the right.
pub fn inner_middle_outer(wheel: usize, left: f64, centre: f64, right: f64) -> (f64, f64, f64) {
    if wheel % 2 == 0 {
        (right, centre, left)
    } else {
        (left, centre, right)
    }
}

/// `true` when `wheel` is on the outside of the turn for this lateral g
/// (positive g = turning right = left wheels outside).
pub fn is_outer(wheel: usize, g_lat: f64) -> bool {
    let left = wheel % 2 == 0;
    (g_lat > 0.0) == left
}

/// Inner − outer spread read as too much camber, °C.
pub const CAMBER_TOO_MUCH_C: f64 = 12.0;

pub fn camber_hint(spread_c: f64, known: bool) -> CamberHint {
    if !known {
        CamberHint::Unknown
    } else if spread_c > CAMBER_TOO_MUCH_C {
        CamberHint::TooMuch
    } else if spread_c < 0.0 {
        CamberHint::TooLittle
    } else {
        CamberHint::Ok
    }
}

/// The game's optimal tyre temperature of one wheel over the analysed laps
/// (median of the positive samples of `optimal_temp_c_*`). `None` when the
/// column is absent (sessions recorded before it existed) or never filled.
pub fn optimal_temp_c(laps: &[LapSeries], wheel: usize) -> Option<f64> {
    median(
        laps.iter()
            .filter_map(|l| l.wheel("optimal_temp_c", wheel))
            .flatten()
            .copied()
            .filter(|v| v.is_finite() && *v > 0.0),
    )
}

pub fn tyre_wheel(laps: &[LapSeries], wheel: usize, optimal_c: Option<f64>) -> TyreWheelAnalysis {
    let mut surf = Vec::new();
    let mut carcass = Vec::new();
    let mut pressure = Vec::new();
    let mut loaded_outer: Vec<(f64, f64, f64)> = Vec::new();
    let mut loaded_any: Vec<(f64, f64, f64)> = Vec::new();
    let mut all: Vec<(f64, f64, f64)> = Vec::new();
    let mut per_lap_pressure = Vec::new();
    let mut wear_first = None;
    let mut wear_last = None;
    for lap in laps {
        let l = lap.wheel("temp_l_c", wheel);
        let c = lap.wheel("temp_c_c", wheel);
        let r = lap.wheel("temp_r_c", wheel);
        let g = lap.col("g_lat");
        let mut lap_p = Vec::new();
        for i in 0..lap.n {
            let (tl, tc, tr) = (at(l, i), at(c, i), at(r, i));
            if tl > 0.0 || tc > 0.0 || tr > 0.0 {
                surf.push((tl + tc + tr) / 3.0);
                let imo = inner_middle_outer(wheel, tl, tc, tr);
                all.push(imo);
                let gl = at(g, i);
                if gl.abs() > LOADED_G {
                    loaded_any.push(imo);
                    if is_outer(wheel, gl) {
                        loaded_outer.push(imo);
                    }
                }
            }
            let cc = at(lap.wheel("carcass_c", wheel), i);
            if cc > 0.0 {
                carcass.push(cc);
            }
            let p = at(lap.wheel("pressure_kpa", wheel), i);
            if p > 0.0 {
                pressure.push(p);
                lap_p.push(p);
            }
        }
        if let Some(m) = mean(lap_p.iter().copied()) {
            per_lap_pressure.push(m);
        }
        if let Some(w) = lap.wheel("wear_remaining", wheel) {
            let first = w.iter().copied().find(|v| v.is_finite());
            let last = w.iter().rev().copied().find(|v| v.is_finite());
            if wear_first.is_none() {
                wear_first = first;
            }
            if last.is_some() {
                wear_last = last;
            }
        }
    }
    let imo_source = if loaded_outer.len() >= 50 {
        &loaded_outer
    } else if loaded_any.len() >= 20 {
        &loaded_any
    } else {
        &all
    };
    let inner = mean(imo_source.iter().map(|x| x.0)).unwrap_or(0.0);
    let middle = mean(imo_source.iter().map(|x| x.1)).unwrap_or(0.0);
    let outer = mean(imo_source.iter().map(|x| x.2)).unwrap_or(0.0);
    let known = !imo_source.is_empty() && (inner > 0.0 || outer > 0.0);
    let spread = inner - outer;
    let avg_carcass = mean(carcass.iter().copied()).unwrap_or(0.0);
    let drift = match (per_lap_pressure.first(), per_lap_pressure.last()) {
        (Some(a), Some(b)) if per_lap_pressure.len() >= 2 => b - a,
        _ => 0.0,
    };
    let wear_per_lap = match (wear_first, wear_last) {
        (Some(a), Some(b)) if !laps.is_empty() => ((a - b) * 100.0 / laps.len() as f64).max(0.0),
        _ => 0.0,
    };
    TyreWheelAnalysis {
        avg_surface_c: mean(surf.iter().copied()).unwrap_or(0.0) as f32,
        avg_carcass_c: avg_carcass as f32,
        optimal_c: optimal_c.map(|v| v as f32),
        delta_to_optimal_c: optimal_c
            .filter(|_| avg_carcass > 0.0)
            .map(|o| (avg_carcass - o) as f32),
        inner_c: inner as f32,
        middle_c: middle as f32,
        outer_c: outer as f32,
        imo_spread_c: spread as f32,
        camber_hint: camber_hint(spread, known),
        avg_pressure_kpa: mean(pressure.iter().copied()).unwrap_or(0.0) as f32,
        pressure_drift_kpa: drift as f32,
        wear_per_lap_pct: wear_per_lap as f32,
    }
}

// ---------------------------------------------------------------------------
// Brakes
// ---------------------------------------------------------------------------

/// `(cold_below, hot_above)` for the **typical peak** disc temperature
/// (median of the per-lap maxima), °C, by class. Lower bound = bottom of the
/// optimum of `knowledge/setup-parameters.yaml` (`thermal_windows.brakes`),
/// upper bound = its "hot" / "fade" value. Checked on five real GT3 sessions
/// (steel discs, BMW M4 and Porsche 992): typical front peaks 665–805 °C, so
/// the former 650 °C called nearly every session "hot"; 750 °C leaves only the
/// street circuit (Long Beach, 772 / 805 °C). No real prototype session yet:
/// the carbon window is the knowledge base's (see `docs/ENGINEER.md` § 5).
pub fn brake_window_c(class: VehicleClass) -> (f64, f64) {
    match class {
        VehicleClass::Gt3 | VehicleClass::Gte => (350.0, 750.0),
        VehicleClass::Hypercar | VehicleClass::Lmp2 | VehicleClass::Lmp3 => (400.0, 1000.0),
        _ => (350.0, 800.0),
    }
}

pub fn brake_wheel(
    laps: &[LapSeries],
    wheel: usize,
    class: VehicleClass,
    lockups: u32,
) -> BrakeWheelAnalysis {
    let mut all = Vec::new();
    let mut max: f64 = 0.0;
    let mut lap_peaks = Vec::new();
    for lap in laps {
        if let Some(t) = lap.wheel("brake_temp_c", wheel) {
            let mut lap_max: f64 = 0.0;
            for &v in t {
                if v.is_finite() && v > 0.0 {
                    all.push(v);
                    lap_max = lap_max.max(v);
                }
            }
            if lap_max > 0.0 {
                lap_peaks.push(lap_max);
                max = max.max(lap_max);
            }
        }
    }
    let typical = median(lap_peaks.iter().copied()).unwrap_or(0.0);
    let (cold, hot) = brake_window_c(class);
    let window = if all.is_empty() {
        BrakeWindow::Unknown
    } else if typical < cold {
        BrakeWindow::Cold
    } else if typical > hot {
        BrakeWindow::Hot
    } else {
        BrakeWindow::Ok
    };
    BrakeWheelAnalysis {
        avg_c: mean(all.iter().copied()).unwrap_or(0.0) as f32,
        max_c: max as f32,
        typical_peak_c: typical as f32,
        window,
        lockups,
    }
}

/// Share of the braking time (brake > 0.1) with the ABS active, 0..1.
pub fn abs_activity(laps: &[LapSeries]) -> Option<f64> {
    let (mut braking, mut active) = (0usize, 0usize);
    for lap in laps {
        let (Some(b), Some(a)) = (lap.col("brake"), lap.col("abs_active")) else {
            continue;
        };
        for i in 0..lap.n {
            if fin(b[i]) > 0.1 {
                braking += 1;
                if fin(a[i]) > 0.5 {
                    active += 1;
                }
            }
        }
    }
    (braking > 0).then(|| active as f64 / braking as f64)
}

/// Steering corrections larger than `min_deg` while braking in a straight
/// line (`brake > 0.3`, `|g_lat| < 0.2`), per lap.
pub fn straight_braking_corrections(laps: &[LapSeries], min_deg: f64) -> f64 {
    let mut count = 0usize;
    for lap in laps {
        let (Some(swa), Some(b), Some(g)) = (
            lap.col("steering_angle_deg"),
            lap.col("brake"),
            lap.col("g_lat"),
        ) else {
            continue;
        };
        let sm = smooth(swa, 5);
        let mut last_extreme = fin(sm[0]);
        let mut rising: Option<bool> = None;
        for i in 1..lap.n {
            let straight = fin(b[i]) > 0.3 && fin(g[i]).abs() < 0.2;
            if !straight {
                rising = None;
                last_extreme = fin(sm[i]);
                continue;
            }
            let d = fin(sm[i]) - fin(sm[i - 1]);
            let now = d > 0.0;
            if let Some(r) = rising {
                if r != now {
                    // direction change: a correction of |amplitude| since the last extreme
                    if (fin(sm[i - 1]) - last_extreme).abs() > min_deg {
                        count += 1;
                    }
                    last_extreme = fin(sm[i - 1]);
                }
            }
            rising = Some(now);
        }
    }
    if laps.is_empty() {
        0.0
    } else {
        count as f64 / laps.len() as f64
    }
}

// ---------------------------------------------------------------------------
// Suspension & dampers
// ---------------------------------------------------------------------------

pub const BOTTOMING_RIDE_HEIGHT_M: f64 = 0.005;
pub const BOTTOMING_MIN_S: f64 = 0.04;
pub const TRAVEL_SATURATION: f64 = 0.98;
pub const DAMPER_SLOW_MM_S: f64 = 25.0;

/// Observed `(min, max)` of the suspension deflection of every wheel over the
/// session, metres.
pub fn travel_range(laps: &[LapSeries]) -> [(f64, f64); 4] {
    let mut r = [(f64::INFINITY, f64::NEG_INFINITY); 4];
    for lap in laps {
        for (w, range) in r.iter_mut().enumerate() {
            if let Some(d) = lap.wheel("susp_deflection_m", w) {
                for &v in d {
                    if v.is_finite() {
                        range.0 = range.0.min(v);
                        range.1 = range.1.max(v);
                    }
                }
            }
        }
    }
    r
}

/// Bottoming events of one lap (any wheel): ride height under 5 mm for
/// ≥ 40 ms. Without a live ride-height channel, falls back to the suspension
/// sitting at ≥ 98 % of its observed travel for ≥ 40 ms (a weaker criterion:
/// the observed maximum is reached by construction, so it only counts when
/// the travel span is above 5 mm).
pub fn bottoming_events(lap: &LapSeries, range: &[(f64, f64); 4]) -> Vec<Event> {
    let rh: Vec<Option<&[f64]>> = (0..4).map(|w| lap.wheel("ride_height_m", w)).collect();
    let sd: Vec<Option<&[f64]>> = (0..4).map(|w| lap.wheel("susp_deflection_m", w)).collect();
    let has_rh = rh.iter().any(|c| {
        c.map(|c| c.iter().any(|v| v.is_finite() && *v > 0.0))
            .unwrap_or(false)
    });
    sustained(lap.t(), BOTTOMING_MIN_S, |i| {
        (0..4).any(|w| {
            if has_rh {
                rh[w]
                    .map(|c| {
                        let v = fin(c[i]);
                        v > 0.0 && v < BOTTOMING_RIDE_HEIGHT_M
                    })
                    .unwrap_or(false)
            } else {
                let (lo, hi) = range[w];
                let span = hi - lo;
                span.is_finite()
                    && span > 0.005
                    && sd[w]
                        .map(|c| fin(c[i]) >= lo + TRAVEL_SATURATION * span)
                        .unwrap_or(false)
            }
        })
    })
}

/// Share of the time one wheel spends at ≥ 98 % of its observed travel, %.
pub fn travel_saturation_pct(laps: &[LapSeries], wheel: usize, range: (f64, f64)) -> f64 {
    let (lo, hi) = range;
    let span = hi - lo;
    if !span.is_finite() || span <= 0.002 {
        return 0.0;
    }
    let (mut n, mut sat) = (0usize, 0usize);
    for lap in laps {
        if let Some(d) = lap.wheel("susp_deflection_m", wheel) {
            for &v in d {
                if v.is_finite() {
                    n += 1;
                    if v >= lo + TRAVEL_SATURATION * span {
                        sat += 1;
                    }
                }
            }
        }
    }
    if n == 0 {
        0.0
    } else {
        100.0 * sat as f64 / n as f64
    }
}

/// Damper-velocity histogram of one wheel over the laps: central derivative
/// of the deflection (3-point smoothing), + = bump, − = rebound; slow when
/// `|v| < 25 mm/s`.
pub fn damper_hist(laps: &[LapSeries], wheel: usize, slow_mm_s: f64) -> DamperHist {
    let (mut sb, mut sr, mut fb, mut fr, mut n) = (0usize, 0usize, 0usize, 0usize, 0usize);
    for lap in laps {
        let Some(d) = lap.wheel("susp_deflection_m", wheel) else {
            continue;
        };
        let t = lap.t();
        if d.len() < 3 || t.len() != d.len() {
            continue;
        }
        let sm = smooth(d, 3);
        for i in 1..d.len() - 1 {
            let dt = t[i + 1] - t[i - 1];
            if !(dt.is_finite() && dt > 0.0) || !sm[i + 1].is_finite() || !sm[i - 1].is_finite() {
                continue;
            }
            let v = (sm[i + 1] - sm[i - 1]) / dt * 1000.0; // mm/s
            n += 1;
            match (v >= 0.0, v.abs() < slow_mm_s) {
                (true, true) => sb += 1,
                (false, true) => sr += 1,
                (true, false) => fb += 1,
                (false, false) => fr += 1,
            }
        }
    }
    if n == 0 {
        return DamperHist::default();
    }
    let pct = |k: usize| 100.0 * k as f64 / n as f64;
    DamperHist {
        slow_bump_pct: pct(sb) as f32,
        slow_rebound_pct: pct(sr) as f32,
        fast_bump_pct: pct(fb) as f32,
        fast_rebound_pct: pct(fr) as f32,
        skew_slow: (pct(sb) - pct(sr)) as f32,
    }
}

// ---------------------------------------------------------------------------
// Driving style
// ---------------------------------------------------------------------------

/// Brake applications: `(rise time 0→80 % of the peak in ms, peak)`.
pub fn brake_applications(lap: &LapSeries) -> Vec<(f64, f64)> {
    let Some(b) = lap.col("brake_raw").or(lap.col("brake")) else {
        return Vec::new();
    };
    let t = lap.t();
    let mut out = Vec::new();
    let mut i = 0;
    while i + 1 < lap.n {
        if fin(b[i]) < 0.05 && fin(b[i + 1]) >= 0.05 {
            let start = i;
            let mut j = i + 1;
            let mut peak = 0.0f64;
            let mut peak_i = j;
            while j < lap.n && fin(b[j]) >= 0.05 {
                if fin(b[j]) > peak {
                    peak = fin(b[j]);
                    peak_i = j;
                }
                j += 1;
            }
            if peak >= 0.3 {
                let target = 0.8 * peak;
                let k = (start..=peak_i)
                    .find(|&k| fin(b[k]) >= target)
                    .unwrap_or(peak_i);
                out.push(((t[k] - t[start]) * 1000.0, peak));
            }
            i = j;
        } else {
            i += 1;
        }
    }
    out
}

/// 0..100 from the mean rise time (≤ 100 ms → max) and the mean peak.
pub fn brake_aggression(laps: &[LapSeries]) -> f64 {
    let apps: Vec<(f64, f64)> = laps.iter().flat_map(brake_applications).collect();
    if apps.is_empty() {
        return 0.0;
    }
    let rise = mean(apps.iter().map(|a| a.0)).unwrap_or(400.0);
    let peak = mean(apps.iter().map(|a| a.1)).unwrap_or(0.0);
    let speed_score = ((400.0 - rise) / 300.0).clamp(0.0, 1.0);
    let peak_score = ((peak - 0.5) / 0.5).clamp(0.0, 1.0);
    100.0 * (0.5 * speed_score + 0.5 * peak_score)
}

/// Share of the entry-phase samples with brake > 5 % and |g_lat| > 0.3 g, %.
pub fn trail_braking(laps: &[LapSeries], corners: &[Vec<Corner>]) -> f64 {
    let (mut entry, mut trail) = (0usize, 0usize);
    for (lap, cs) in laps.iter().zip(corners) {
        let b = lap.col("brake");
        let g = lap.col("g_lat");
        for c in cs {
            for i in c.start..c.end {
                if c.phase_at(i) == Some(CornerPhase::Entry) {
                    entry += 1;
                    if at(b, i) > 0.05 && at(g, i).abs() > LOADED_G {
                        trail += 1;
                    }
                }
            }
        }
    }
    if entry == 0 {
        0.0
    } else {
        100.0 * trail as f64 / entry as f64
    }
}

/// Smoothness 0..100 of a signal: 1 − (sign inversions per second of its
/// filtered derivative, above `eps` per second) / `ref_per_s`, clamped.
/// `None` when the channel is absent or flat.
pub fn smoothness(
    laps: &[LapSeries],
    channel: &str,
    eps_per_s: f64,
    ref_per_s: f64,
) -> Option<f64> {
    let (mut inversions, mut seconds) = (0usize, 0.0);
    if !laps.iter().any(|l| l.is_live(channel)) {
        return None;
    }
    for lap in laps {
        let Some(x) = lap.col(channel) else { continue };
        let t = lap.t();
        if x.len() < 8 || t.len() != x.len() {
            continue;
        }
        let dt = lap.dt();
        let w = ((0.1 / dt).round() as usize) | 1; // ≈ 0.1 s window, odd
        let sm = smooth(x, w);
        let mut last_sign = 0i8;
        for i in 1..sm.len() {
            let d = (sm[i] - sm[i - 1]) / dt;
            if !d.is_finite() || d.abs() < eps_per_s {
                continue;
            }
            let s = if d > 0.0 { 1 } else { -1 };
            if last_sign != 0 && s != last_sign {
                inversions += 1;
            }
            last_sign = s;
        }
        seconds += t[t.len() - 1] - t[0];
    }
    if seconds <= 0.0 {
        return None;
    }
    let rate = inversions as f64 / seconds;
    Some(100.0 * (1.0 - rate / ref_per_s).clamp(0.0, 1.0))
}

/// Neutral value of a style score whose channel is missing.
pub const NEUTRAL_SCORE: f64 = 50.0;

/// Share of the samples with any wheel on a rumble strip, or a vertical
/// jolt of more than 0.5 g around the lap's resting value, %.
///
/// The resting value is measured (median of the lap): Le Mans Ultimate
/// reports the vertical acceleration without gravity (≈ 0 g at rest), other
/// sources with it (≈ 1 g).
pub fn kerb_usage(laps: &[LapSeries]) -> f64 {
    let (mut n, mut k) = (0usize, 0usize);
    let rumble = SurfaceType::Rumble as u8 as f64;
    for lap in laps {
        let surf: Vec<Option<&[f64]>> = (0..4).map(|w| lap.wheel("surface", w)).collect();
        let gv = lap.col("g_vert");
        let rest = gv.and_then(|g| median(g.iter().copied()));
        for i in 0..lap.n {
            n += 1;
            let on_kerb = surf.iter().any(|s| at(*s, i) == rumble)
                || match (gv, rest) {
                    (Some(g), Some(r)) => g[i].is_finite() && (g[i] - r).abs() > 0.5,
                    _ => false,
                };
            if on_kerb {
                k += 1;
            }
        }
    }
    if n == 0 {
        0.0
    } else {
        100.0 * k as f64 / n as f64
    }
}

/// Share of the samples above 60 km/h with neither pedal, %.
pub fn coasting_pct(laps: &[LapSeries]) -> f64 {
    let (mut n, mut c) = (0usize, 0usize);
    for lap in laps {
        let (Some(th), Some(b), Some(v)) =
            (lap.col("throttle"), lap.col("brake"), lap.col("speed_kmh"))
        else {
            continue;
        };
        for i in 0..lap.n {
            if fin(v[i]) > 60.0 {
                n += 1;
                if fin(th[i]) < 0.05 && fin(b[i]) < 0.05 {
                    c += 1;
                }
            }
        }
    }
    if n == 0 {
        0.0
    } else {
        100.0 * c as f64 / n as f64
    }
}

/// Share of the exit-phase samples with the TC active, 0..1.
pub fn tc_activity_on_exit(laps: &[LapSeries], corners: &[Vec<Corner>]) -> Option<f64> {
    let (mut exit, mut active) = (0usize, 0usize);
    for (lap, cs) in laps.iter().zip(corners) {
        let Some(tc) = lap.col("tc_active") else {
            continue;
        };
        for c in cs {
            for (i, &on) in tc.iter().enumerate().take(c.end).skip(c.start) {
                if c.phase_at(i) == Some(CornerPhase::Exit) {
                    exit += 1;
                    if fin(on) > 0.5 {
                        active += 1;
                    }
                }
            }
        }
    }
    (exit > 0).then(|| active as f64 / exit as f64)
}

/// Assemble the style block (labels are added by the diagnostics).
pub fn driving_style(laps: &[LapSeries], corners: &[Vec<Corner>]) -> DrivingStyle {
    let n_laps = laps.len().max(1) as f64;
    let lockup_total: usize = laps
        .iter()
        .map(|l| (0..4).map(|w| lockups(l, w).len()).sum::<usize>())
        .sum();
    let spin_total: usize = laps
        .iter()
        .map(|l| (0..4).map(|w| wheelspins(l, w).len()).sum::<usize>())
        .sum();
    DrivingStyle {
        brake_aggression: brake_aggression(laps) as f32,
        trail_braking: trail_braking(laps, corners) as f32,
        throttle_smoothness: smoothness(laps, "throttle", 0.5, 4.0).unwrap_or(NEUTRAL_SCORE) as f32,
        steering_smoothness: smoothness(laps, "steering_angle_deg", 20.0, 6.0)
            .unwrap_or(NEUTRAL_SCORE) as f32,
        kerb_usage: kerb_usage(laps) as f32,
        lockup_rate: (lockup_total as f64 / n_laps) as f32,
        wheelspin_rate: (spin_total as f64 / n_laps) as f32,
        coasting_pct: coasting_pct(laps) as f32,
        labels_fr: Vec::new(),
    }
}

// ---------------------------------------------------------------------------
// Lap statistics
// ---------------------------------------------------------------------------

/// Consistency = 100 − 20 × CV(%) (CV = σ / mean of the valid laps), clamped.
pub const CONSISTENCY_K: f64 = 20.0;

pub fn lap_stats(all: &[LapRecord], analysed: &[LapRecord]) -> LapStats {
    let valid: Vec<&LapRecord> = analysed.iter().filter(|l| l.lap_time_s > 0.0).collect();
    let times: Vec<f64> = valid.iter().map(|l| l.lap_time_s as f64).collect();
    let best = times
        .iter()
        .copied()
        .fold(None, |b: Option<f64>, t| Some(b.map_or(t, |b| b.min(t))));
    let mean_t = mean(times.iter().copied());
    let std_t = std_dev(times.iter().copied());
    let consistency = match (mean_t, std_t) {
        (Some(m), Some(s)) if m > 0.0 => (100.0 - CONSISTENCY_K * 100.0 * s / m).clamp(0.0, 100.0),
        (Some(_), None) => 100.0,
        _ => 0.0,
    };
    let degradation = if valid.len() >= 4 {
        let x: Vec<f64> = valid.iter().map(|l| l.lap_number as f64).collect();
        linear_slope(&x, &times)
    } else {
        None
    };
    let fuel = mean(
        valid
            .iter()
            .map(|l| l.fuel_used_l as f64)
            .filter(|f| *f > 0.0),
    );
    let energy = mean(
        valid
            .iter()
            .filter_map(|l| l.virtual_energy_used.map(|e| e as f64))
            .filter(|e| *e > 0.0),
    );
    let top = all
        .iter()
        .map(|l| l.top_speed_kmh as f64)
        .fold(0.0, f64::max);
    LapStats {
        best_s: best.map(|v| v as f32),
        mean_valid_s: mean_t.map(|v| v as f32),
        std_valid_s: std_t.map(|v| v as f32),
        consistency_score: consistency as f32,
        degradation_s_per_lap: degradation.map(|v| v as f32),
        fuel_per_lap_l: fuel.map(|v| v as f32),
        energy_per_lap: energy.map(|v| v as f32),
        top_speed_kmh: top as f32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::corners::{segment, tests::synthetic_lap, CornerConfig};
    use crate::series::wheel_channel;

    #[test]
    fn helpers() {
        assert_eq!(mean([1.0, 2.0, f64::NAN, 3.0]), Some(2.0));
        assert_eq!(median([3.0, 1.0, 2.0]), Some(2.0));
        assert_eq!(median([4.0, 1.0, 2.0, 3.0]), Some(2.5));
        assert!(std_dev([1.0]).is_none());
        assert!((std_dev([2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0]).unwrap() - 2.138).abs() < 0.01);
        assert!((linear_slope(&[1.0, 2.0, 3.0], &[2.0, 4.0, 6.0]).unwrap() - 2.0).abs() < 1e-9);
        let t: Vec<f64> = (0..10).map(|i| i as f64 * 0.02).collect();
        let ev = sustained(&t, 0.05, |i| (3..7).contains(&i));
        assert_eq!(ev, vec![Event { start: 3, end: 7 }]);
        assert!(sustained(&t, 0.05, |i| i == 4).is_empty());
    }

    #[test]
    fn inner_outer_remapping() {
        // Left wheels: inner shoulder is the game's "right" (index 2).
        assert_eq!(inner_middle_outer(0, 80.0, 85.0, 95.0), (95.0, 85.0, 80.0));
        assert_eq!(inner_middle_outer(2, 80.0, 85.0, 95.0), (95.0, 85.0, 80.0));
        // Right wheels: inner shoulder is the game's "left" (index 0).
        assert_eq!(inner_middle_outer(1, 80.0, 85.0, 95.0), (80.0, 85.0, 95.0));
        assert_eq!(inner_middle_outer(3, 80.0, 85.0, 95.0), (80.0, 85.0, 95.0));
        assert!(is_outer(0, 1.0) && !is_outer(1, 1.0) && is_outer(3, -1.0));
        assert_eq!(camber_hint(15.0, true), CamberHint::TooMuch);
        assert_eq!(camber_hint(-3.0, true), CamberHint::TooLittle);
        assert_eq!(camber_hint(6.0, true), CamberHint::Ok);
        assert_eq!(camber_hint(6.0, false), CamberHint::Unknown);
    }

    #[test]
    fn tyre_wheel_uses_loaded_outer_samples() {
        let (mut lap, _, _) = synthetic_lap(50.0, 1.4);
        let n = lap.n;
        let g: Vec<f64> = lap.col("g_lat").unwrap().to_vec();
        // FL (outer in a right-hander): hot inner shoulder (index 2) when loaded.
        let l: Vec<f64> = g
            .iter()
            .map(|g| if g.abs() > 0.3 { 80.0 } else { 70.0 })
            .collect();
        let c: Vec<f64> = g
            .iter()
            .map(|g| if g.abs() > 0.3 { 88.0 } else { 70.0 })
            .collect();
        let r: Vec<f64> = g
            .iter()
            .map(|g| if g.abs() > 0.3 { 100.0 } else { 70.0 })
            .collect();
        lap.set(wheel_channel("temp_l_c", 0), l)
            .set(wheel_channel("temp_c_c", 0), c)
            .set(wheel_channel("temp_r_c", 0), r)
            .set(wheel_channel("carcass_c", 0), vec![85.0; n])
            .set(wheel_channel("pressure_kpa", 0), vec![170.0; n])
            .set(
                wheel_channel("wear_remaining", 0),
                (0..n).map(|i| 1.0 - 0.01 * i as f64 / n as f64).collect(),
            );
        let t = tyre_wheel(&[lap], 0, Some(90.0));
        assert!((t.inner_c - 100.0).abs() < 0.5, "{t:?}");
        assert!((t.outer_c - 80.0).abs() < 0.5, "{t:?}");
        assert!((t.imo_spread_c - 20.0).abs() < 1.0);
        assert_eq!(t.camber_hint, CamberHint::TooMuch);
        assert_eq!(t.delta_to_optimal_c, Some(-5.0));
        assert!(
            (t.wear_per_lap_pct - 1.0).abs() < 0.05,
            "{}",
            t.wear_per_lap_pct
        );
    }

    #[test]
    fn understeer_from_slip_angles_is_positive_and_marked() {
        let (mut lap, _, _) = synthetic_lap(50.0, 1.4);
        let n = lap.n;
        let g: Vec<f64> = lap.col("g_lat").unwrap().to_vec();
        // Front slip angles 4°, rear 1.5° under load: strong understeer.
        for w in 0..4 {
            let a = if w < 2 { 4.0 } else { 1.5 };
            let col: Vec<f64> = g
                .iter()
                .map(|g| if g.abs() > 0.1 { a * g.signum() } else { 0.0 })
                .collect();
            lap.set(wheel_channel("slip_angle_deg", w), col);
        }
        // Neutral kinematics: steering wheel = ratio × Ackermann angle.
        let speed: Vec<f64> = lap.col("speed_kmh").unwrap().to_vec();
        let yaw: Vec<f64> = (0..n)
            .map(|i| {
                let v = speed[i] / 3.6;
                (g[i] * 9.81 / v.max(1.0)).to_degrees()
            })
            .collect();
        let swa: Vec<f64> = (0..n)
            .map(|i| {
                let v = speed[i] / 3.6;
                12.0 * (2.7 * yaw[i].to_radians() / v.max(1.0)).to_degrees()
            })
            .collect();
        lap.set("yaw_rate_dps", yaw).set("steering_angle_deg", swa);
        let corners = segment(&lap, VehicleClass::Gt3, &CornerConfig::default());
        assert_eq!(corners.len(), 1);
        let ratio = estimate_steering_ratio(&lap, 2.7);
        assert!(
            ratio.map(|r| (r - 12.0).abs() < 1.0).unwrap_or(true),
            "{ratio:?}"
        );
        let bal = balance_series(&lap, VehicleClass::Gt3, ratio);
        assert!(bal.has_slip && bal.has_gradient);
        let raw = corner_phase_raw(&bal, &corners[0], CornerPhase::Mid);
        assert!(raw.samples > 10);
        let (s, k) = (raw.slip_delta_deg, raw.gradient_deg_per_g);
        assert!((s.unwrap() - 2.5).abs() < 0.1, "{s:?}");
        assert!(k.unwrap().abs() < 0.5, "{k:?}");
        // Load-free slip balance: (4 − 1.5) / (4 + 1.5).
        assert!(
            (raw.slip_balance.unwrap() - 2.5 / 5.5).abs() < 0.01,
            "{raw:?}"
        );
        let (idx, agree) = normalise_balance(s, k, CornerSpeed::Slow).unwrap();
        assert!(idx > 0.3, "absolute understeer index {idx}");
        assert!(agree >= 0.3);
        // Oversteer: swap the axles.
        let (idx_os, _) = normalise_balance(Some(-2.5), Some(-1.0), CornerSpeed::Slow).unwrap();
        assert!(idx_os < -0.3);
        // Without slip channels only the gradient speaks, with reduced agreement.
        assert_eq!(
            normalise_balance(None, Some(3.0), CornerSpeed::Fast),
            Some((1.0, 0.7))
        );
        assert_eq!(normalise_balance(None, None, CornerSpeed::Fast), None);
    }

    fn pass(slip_balance: f64, gradient: Option<f64>) -> PhaseRaw {
        PhaseRaw {
            slip_delta_deg: Some(slip_balance * 2.0),
            slip_balance: Some(slip_balance),
            gradient_deg_per_g: gradient,
            samples: 20,
        }
    }

    #[test]
    fn robust_reference_and_relative_index() {
        // Too few passes: no reference.
        assert!(RobustRef::of([0.2, 0.3, 0.25], SLIP_BALANCE_SIGMA_FLOOR).is_none());
        // Median 0.25, MAD 0.05 → σ = 0.074; an outlier does not move them.
        let r = RobustRef::of([0.2, 0.25, 0.3, 0.25, 0.2, 0.3, 0.25, 5.0], 0.0).unwrap();
        assert!((r.median - 0.25).abs() < 1e-9, "{r:?}");
        assert!((r.sigma - MAD_TO_SIGMA * 0.05).abs() < 1e-9, "{r:?}");
        assert_eq!(r.n, 8);
        // A perfectly uniform session: the floor keeps σ away from zero.
        let u = RobustRef::of([0.3; 6], SLIP_BALANCE_SIGMA_FLOOR).unwrap();
        assert_eq!(u.sigma, SLIP_BALANCE_SIGMA_FLOOR);

        // Every car of the real sessions sits at a positive absolute balance (+0.2…+0.3 here):
        // relative to its own reference, the usual passes are neutral.
        let passes: Vec<PhaseRaw> = [0.22, 0.25, 0.28, 0.24, 0.26, 0.25, 0.27, 0.23]
            .iter()
            .map(|&s| pass(s, None))
            .collect();
        let refs = BalanceRefs::of(&passes);
        assert!(refs.gradient.is_none());
        let slip = refs.slip.unwrap();
        assert!((slip.median - 0.25).abs() < 0.011, "{slip:?}");
        assert_eq!(slip.sigma, SLIP_BALANCE_SIGMA_FLOOR, "floor");
        let (usual, agree) = relative_balance(&pass(0.26, None), &refs).unwrap();
        assert!(usual.abs() < BALANCE_TRIGGER_INDEX, "{usual}");
        assert_eq!(agree, 0.7);
        // A phase with much more front slip than usual: +0.15 = 3 σ → index +0.6.
        let (us, _) = relative_balance(&pass(0.40, None), &refs).unwrap();
        assert!(
            us > BALANCE_TRIGGER_INDEX && (us - 0.6).abs() < 0.03,
            "{us}"
        );
        // Much less (oversteer relative to the car), saturated at −1.
        let (os, _) = relative_balance(&pass(-0.30, None), &refs).unwrap();
        assert_eq!(os, -1.0);
        // Both estimators: fused deviations, agreement from their signs.
        let both: Vec<PhaseRaw> = (0..8)
            .map(|i| {
                pass(
                    0.25 + 0.01 * (i % 3) as f64,
                    Some(1.0 + 0.1 * (i % 2) as f64),
                )
            })
            .collect();
        let refs = BalanceRefs::of(&both);
        assert!(refs.gradient.is_some());
        let (_, agree) = relative_balance(&pass(0.45, Some(4.0)), &refs).unwrap();
        assert_eq!(agree, 1.0);
        let (_, agree) = relative_balance(&pass(0.45, Some(-3.0)), &refs).unwrap();
        assert_eq!(agree, 0.3);
        // No reference at all: nothing to compare with.
        assert_eq!(
            relative_balance(&pass(0.4, None), &BalanceRefs::default()),
            None
        );
    }

    #[test]
    fn slip_balance_ratio_ignores_the_load_level() {
        // Same 2:1 front / rear ratio at low and high lateral g: same ratio, growing difference.
        let (mut lap, _, _) = synthetic_lap(50.0, 1.4);
        let g: Vec<f64> = lap.col("g_lat").unwrap().to_vec();
        for w in 0..4 {
            let k = if w < 2 { 2.0 } else { 1.0 };
            lap.set(
                wheel_channel("slip_angle_deg", w),
                g.iter().map(|g| k * 2.0 * g).collect::<Vec<_>>(),
            );
        }
        let bal = balance_series(&lap, VehicleClass::Gt3, None);
        let loaded: Vec<usize> = (0..lap.n).filter(|&i| g[i].abs() > LOADED_G).collect();
        let (lo, hi) = loaded.iter().fold((f64::INFINITY, 0.0f64), |(lo, hi), &i| {
            (lo.min(g[i].abs()), hi.max(g[i].abs()))
        });
        assert!(hi > 2.0 * lo, "the lap spans several load levels");
        for &i in &loaded {
            assert!((bal.slip_balance[i] - 1.0 / 3.0).abs() < 1e-9);
            assert!((bal.slip_delta_deg[i] - 2.0 * g[i].abs()).abs() < 1e-9);
        }
    }

    #[test]
    fn front_lockup_is_counted_once() {
        let (mut lap, _, _) = synthetic_lap(50.0, 1.4);
        let n = lap.n;
        let brake: Vec<f64> = lap.col("brake").unwrap().to_vec();
        // FL locks for 0.2 s during braking (t = 3.5..3.7 s), a 20 ms blip is ignored.
        let mut slip = vec![0.0; n];
        for (i, s) in slip.iter_mut().enumerate() {
            let ts = i as f64 / 50.0;
            if (3.5..3.7).contains(&ts) || (4.0..4.02).contains(&ts) {
                *s = -0.6;
            }
        }
        lap.set(wheel_channel("slip_ratio", 0), slip.clone());
        lap.set(wheel_channel("slip_ratio", 1), vec![0.0; n]);
        lap.set(wheel_channel("slip_ratio", 2), vec![0.0; n]);
        lap.set(wheel_channel("slip_ratio", 3), vec![0.0; n]);
        assert_eq!(lockups(&lap, 0).len(), 1);
        assert_eq!(lockups(&lap, 1).len(), 0);
        assert!(brake[(3.6 * 50.0) as usize] > 0.3);
        // Same slip without brake: not a lock-up.
        lap.set("brake", vec![0.0; n]);
        assert_eq!(lockups(&lap, 0).len(), 0);
        // Wheel-spin on the rear under throttle (exit phase, t = 7.5..7.7 s).
        let mut spin = vec![0.0; n];
        for (i, s) in spin.iter_mut().enumerate() {
            let ts = i as f64 / 50.0;
            if (7.5..7.7).contains(&ts) {
                *s = 0.3;
            }
        }
        lap.set(wheel_channel("slip_ratio", 2), spin);
        assert_eq!(wheelspins(&lap, 2).len(), 1);
        assert_eq!(wheelspins(&lap, 3).len(), 0);
    }

    #[test]
    fn brakes_suspension_and_style() {
        let (mut lap, _, _) = synthetic_lap(50.0, 1.4);
        let n = lap.n;
        for w in 0..4 {
            lap.set(
                wheel_channel("brake_temp_c", w),
                (0..n).map(|i| 400.0 + (i % 50) as f64).collect(),
            );
            // Deflection: slow sinus → mostly slow-band, symmetric.
            lap.set(
                wheel_channel("susp_deflection_m", w),
                (0..n)
                    .map(|i| 0.03 + 0.002 * (i as f64 * 0.05).sin())
                    .collect(),
            );
            lap.set(wheel_channel("ride_height_m", w), vec![0.05; n]);
            lap.set(wheel_channel("surface", w), vec![0.0; n]);
        }
        let b = brake_wheel(&[lap.clone()], 0, VehicleClass::Gt3, 2);
        assert_eq!(b.window, BrakeWindow::Ok);
        assert_eq!(b.lockups, 2);
        assert!(b.max_c >= 449.0);
        assert_eq!(b.typical_peak_c, b.max_c);
        assert_eq!(
            brake_wheel(&[], 0, VehicleClass::Gt3, 0).window,
            BrakeWindow::Unknown
        );
        let range = travel_range(&[lap.clone()]);
        assert!(bottoming_events(&lap, &range).is_empty());
        let h = damper_hist(&[lap.clone()], 0, DAMPER_SLOW_MM_S);
        let total = h.slow_bump_pct + h.slow_rebound_pct + h.fast_bump_pct + h.fast_rebound_pct;
        assert!((total - 100.0).abs() < 0.5, "{h:?}");
        assert!(h.skew_slow.abs() < 10.0, "{h:?}");
        // A 1 mm ride height for 0.1 s is a bottoming event.
        let mut rh = vec![0.05; n];
        for v in rh.iter_mut().take(10) {
            *v = 0.001;
        }
        lap.set(wheel_channel("ride_height_m", 1), rh);
        assert_eq!(bottoming_events(&lap, &range).len(), 1);
        let corners = segment(&lap, VehicleClass::Gt3, &CornerConfig::default());
        let style = driving_style(&[lap.clone()], &[corners]);
        assert!(style.brake_aggression > 0.0 && style.brake_aggression <= 100.0);
        assert!(style.trail_braking > 50.0, "{style:?}");
        assert!(style.throttle_smoothness > 50.0, "{style:?}");
        assert!(style.coasting_pct < 20.0, "{style:?}");
        assert_eq!(style.kerb_usage, 0.0);
    }

    #[test]
    fn brake_window_uses_the_typical_peak_per_lap() {
        let lap_with_peak = |peak: f64| {
            let (mut lap, _, _) = synthetic_lap(50.0, 1.4);
            let n = lap.n;
            for w in 0..4 {
                lap.set(
                    wheel_channel("brake_temp_c", w),
                    (0..n)
                        .map(|i| 200.0 + (peak - 200.0) * ((i + 1) as f64 / n as f64))
                        .collect(),
                );
            }
            lap
        };
        // Real GT3 laps peak at 650–720 °C: fine (the former 650 °C bound called them "hot");
        // one 820 °C stop does not make the brakes hot.
        let gt3 = [
            lap_with_peak(690.0),
            lap_with_peak(820.0),
            lap_with_peak(700.0),
        ];
        let b = brake_wheel(&gt3, 0, VehicleClass::Gt3, 0);
        assert_eq!(b.window, BrakeWindow::Ok, "{b:?}");
        assert!((b.typical_peak_c - 700.0).abs() < 1.0, "{b:?}");
        assert!((b.max_c - 820.0).abs() < 1.0, "{b:?}");
        // Every lap above 750 °C on steel discs: hot.
        let hot = [lap_with_peak(790.0), lap_with_peak(810.0)];
        assert_eq!(
            brake_wheel(&hot, 0, VehicleClass::Gt3, 0).window,
            BrakeWindow::Hot
        );
        // The same temperatures are fine on carbon discs.
        assert_eq!(
            brake_wheel(&hot, 0, VehicleClass::Lmp2, 0).window,
            BrakeWindow::Ok
        );
        // Brakes that never reach the bottom of the window: cold.
        let cold = [lap_with_peak(330.0), lap_with_peak(340.0)];
        assert_eq!(
            brake_wheel(&cold, 0, VehicleClass::Gt3, 0).window,
            BrakeWindow::Cold
        );
        assert_eq!(
            brake_wheel(&hot, 0, VehicleClass::Hypercar, 0).window,
            BrakeWindow::Ok
        );
    }

    #[test]
    fn lap_statistics() {
        let mk = |n: i32, t: f32| LapRecord {
            lap_number: n,
            lap_time_s: t,
            valid: true,
            fuel_used_l: 2.0,
            top_speed_kmh: 250.0 + n as f32,
            ..Default::default()
        };
        let laps = vec![mk(1, 100.0), mk(2, 100.5), mk(3, 101.0), mk(4, 101.5)];
        let s = lap_stats(&laps, &laps);
        assert_eq!(s.best_s, Some(100.0));
        assert!((s.degradation_s_per_lap.unwrap() - 0.5).abs() < 1e-5);
        assert_eq!(s.fuel_per_lap_l, Some(2.0));
        assert_eq!(s.top_speed_kmh, 254.0);
        assert!(s.consistency_score > 80.0 && s.consistency_score < 100.0);
        let one = lap_stats(&laps[..1], &laps[..1]);
        assert!(one.degradation_s_per_lap.is_none());
        assert_eq!(one.consistency_score, 100.0);
    }
}
