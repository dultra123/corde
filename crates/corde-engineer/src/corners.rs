//! Corner segmentation of one lap from `|g_lat|`, phase tagging (entry / mid /
//! exit) and matching of the corners of several laps by distance.
//!
//! Thresholds (see `docs/ENGINEER.md`): a corner starts when `|g_lat|` rises
//! above `g_enter` (0.5 g) and ends when it drops under `g_exit` (0.35 g,
//! hysteresis); segments of the same direction closer than `merge_gap_s`
//! (0.5 s) are merged; segments shorter than `min_duration_s` (0.4 s) are
//! dropped. The apex is the minimum of speed. Corner speed classes: slow
//! ≤ 90 km/h, medium 90–160, fast > 160 (prototypes: +20 km/h).

use crate::series::LapSeries;
use corde_protocol::{CornerPhase, CornerSpeed, TurnDirection, VehicleClass};

#[derive(Debug, Clone)]
pub struct CornerConfig {
    pub g_enter: f64,
    pub g_exit: f64,
    pub merge_gap_s: f64,
    pub min_duration_s: f64,
    /// Half-width of the "mid" window around the apex, seconds.
    pub mid_window_s: f64,
    /// Corners of different laps closer than this are the same corner.
    pub match_tolerance_m: f64,
}

impl Default for CornerConfig {
    fn default() -> Self {
        Self {
            g_enter: 0.5,
            g_exit: 0.35,
            merge_gap_s: 0.5,
            min_duration_s: 0.4,
            mid_window_s: 0.25,
            match_tolerance_m: 40.0,
        }
    }
}

/// One corner of one lap, as sample indices into the lap.
#[derive(Debug, Clone)]
pub struct Corner {
    /// `[start, end)` sample range.
    pub start: usize,
    pub end: usize,
    /// Index of the minimum speed.
    pub apex: usize,
    pub direction: TurnDirection,
    pub min_speed_kmh: f64,
    pub max_g_lat: f64,
    pub start_dist_m: f64,
    pub end_dist_m: f64,
    pub apex_dist_m: f64,
    pub speed_class: CornerSpeed,
    /// Phase of each sample of `[start, end)`.
    pub phases: Vec<Option<CornerPhase>>,
}

impl Corner {
    pub fn len(&self) -> usize {
        self.end - self.start
    }

    pub fn is_empty(&self) -> bool {
        self.end <= self.start
    }

    /// Phase of the sample at absolute index `i`.
    pub fn phase_at(&self, i: usize) -> Option<CornerPhase> {
        if i < self.start || i >= self.end {
            None
        } else {
            self.phases[i - self.start]
        }
    }
}

/// Corner speed class from the minimum speed; prototypes shift the limits.
pub fn classify_speed(min_speed_kmh: f64, class: VehicleClass) -> CornerSpeed {
    let shift = match class {
        VehicleClass::Hypercar | VehicleClass::Lmp2 | VehicleClass::Lmp3 => 20.0,
        _ => 0.0,
    };
    if min_speed_kmh <= 90.0 + shift {
        CornerSpeed::Slow
    } else if min_speed_kmh <= 160.0 + shift {
        CornerSpeed::Medium
    } else {
        CornerSpeed::Fast
    }
}

fn finite_or(v: f64, d: f64) -> f64 {
    if v.is_finite() {
        v
    } else {
        d
    }
}

/// Segment the lap into corners.
pub fn segment(lap: &LapSeries, class: VehicleClass, cfg: &CornerConfig) -> Vec<Corner> {
    let n = lap.n;
    let Some(g_lat) = lap.col("g_lat") else {
        return Vec::new();
    };
    let t = lap.t();
    let dist = lap.dist();
    let speed = lap.col("speed_kmh").unwrap_or(&[]);
    let brake = lap.col("brake");
    let throttle = lap.col("throttle");
    let g_long = lap.col("g_long");
    if n < 3 || t.len() != n {
        return Vec::new();
    }
    // Light smoothing of |g_lat| (3 samples) against the game's noise.
    let g: Vec<f64> = (0..n)
        .map(|i| {
            let a = finite_or(g_lat[i.saturating_sub(1)], 0.0).abs();
            let b = finite_or(g_lat[i], 0.0).abs();
            let c = finite_or(g_lat[(i + 1).min(n - 1)], 0.0).abs();
            (a + b + c) / 3.0
        })
        .collect();

    // Raw segments with hysteresis.
    let mut raw: Vec<(usize, usize)> = Vec::new();
    let mut start: Option<usize> = None;
    for (i, &gi) in g.iter().enumerate() {
        match start {
            None if gi > cfg.g_enter => start = Some(i),
            Some(s) if gi < cfg.g_exit => {
                raw.push((s, i));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(s) = start {
        raw.push((s, n));
    }

    let sign_of = |s: usize, e: usize| -> f64 {
        let sum: f64 = g_lat[s..e].iter().filter(|v| v.is_finite()).sum();
        sum.signum()
    };

    // Merge close segments of the same direction.
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for (s, e) in raw {
        if let Some(last) = merged.last_mut() {
            let gap = t[s] - t[last.1.min(n - 1)];
            if gap < cfg.merge_gap_s && sign_of(last.0, last.1) == sign_of(s, e) {
                last.1 = e;
                continue;
            }
        }
        merged.push((s, e));
    }

    let mut corners = Vec::new();
    for (s, e) in merged {
        if e <= s + 1 {
            continue;
        }
        let duration = t[e - 1] - t[s];
        if duration < cfg.min_duration_s {
            continue;
        }
        let mut apex = s;
        let mut min_speed = f64::INFINITY;
        let mut max_g: f64 = 0.0;
        for (i, &gi) in g.iter().enumerate().take(e).skip(s) {
            let v = speed.get(i).copied().unwrap_or(f64::NAN);
            if v.is_finite() && v < min_speed {
                min_speed = v;
                apex = i;
            }
            max_g = max_g.max(gi);
        }
        if !min_speed.is_finite() {
            min_speed = 0.0;
        }
        let direction = match sign_of(s, e) {
            x if x > 0.0 => TurnDirection::Right,
            x if x < 0.0 => TurnDirection::Left,
            _ => TurnDirection::Unknown,
        };
        let phases = (s..e)
            .map(|i| {
                let near_apex = (t[i] - t[apex]).abs() <= cfg.mid_window_s;
                let steady = g[i] >= 0.9 * max_g;
                if near_apex {
                    Some(CornerPhase::Mid)
                } else if i < apex {
                    let braking = brake.map(|b| finite_or(b[i], 0.0) > 0.05).unwrap_or(false)
                        || g_long
                            .map(|a| finite_or(a[i], 0.0) < -0.05)
                            .unwrap_or(false);
                    if braking {
                        Some(CornerPhase::Entry)
                    } else if steady {
                        Some(CornerPhase::Mid)
                    } else {
                        None
                    }
                } else {
                    let on_throttle = throttle
                        .map(|th| finite_or(th[i], 0.0) > 0.10)
                        .unwrap_or(false);
                    if on_throttle {
                        Some(CornerPhase::Exit)
                    } else if steady {
                        Some(CornerPhase::Mid)
                    } else {
                        None
                    }
                }
            })
            .collect();
        corners.push(Corner {
            start: s,
            end: e,
            apex,
            direction,
            min_speed_kmh: min_speed,
            max_g_lat: max_g,
            start_dist_m: finite_or(dist.get(s).copied().unwrap_or(f64::NAN), 0.0),
            end_dist_m: finite_or(dist.get(e - 1).copied().unwrap_or(f64::NAN), 0.0),
            apex_dist_m: finite_or(dist.get(apex).copied().unwrap_or(f64::NAN), 0.0),
            speed_class: classify_speed(min_speed, class),
            phases,
        });
    }
    corners
}

/// The same corner seen on several laps.
#[derive(Debug, Clone)]
pub struct MatchedCorner {
    /// Mean apex distance of the passes, metres.
    pub apex_dist_m: f64,
    /// `(lap index, corner index in that lap)`.
    pub passes: Vec<(usize, usize)>,
}

/// Group the corners of every lap by apex distance (± `tol_m`, wrapping
/// around the lap when `track_length_m > 0`) and direction; a group holds at
/// most one corner per lap. Sorted by distance.
pub fn match_corners(
    per_lap: &[Vec<Corner>],
    tol_m: f64,
    track_length_m: f64,
) -> Vec<MatchedCorner> {
    let mut seeds: Vec<MatchedCorner> = Vec::new();
    let mut seed_dirs: Vec<TurnDirection> = Vec::new();
    let distance = |a: f64, b: f64| -> f64 {
        let d = (a - b).abs();
        if track_length_m > 0.0 {
            d.min((track_length_m - d).abs())
        } else {
            d
        }
    };
    for (li, corners) in per_lap.iter().enumerate() {
        for (ci, c) in corners.iter().enumerate() {
            let best = seeds
                .iter_mut()
                .zip(seed_dirs.iter())
                .map(|(s, d)| (distance(s.apex_dist_m, c.apex_dist_m), s, *d))
                .filter(|(dist, s, d)| {
                    *dist <= tol_m
                        && (*d == TurnDirection::Unknown
                            || c.direction == TurnDirection::Unknown
                            || *d == c.direction)
                        && !s.passes.iter().any(|(l, _)| *l == li)
                })
                .min_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
            match best {
                Some((_, seed, _)) => {
                    let k = seed.passes.len() as f64;
                    seed.apex_dist_m = (seed.apex_dist_m * k + c.apex_dist_m) / (k + 1.0);
                    seed.passes.push((li, ci));
                }
                None => {
                    seeds.push(MatchedCorner {
                        apex_dist_m: c.apex_dist_m,
                        passes: vec![(li, ci)],
                    });
                    seed_dirs.push(c.direction);
                }
            }
        }
    }
    seeds.sort_by(|a, b| {
        a.apex_dist_m
            .partial_cmp(&b.apex_dist_m)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    seeds
}

#[cfg(test)]
mod match_tests {
    use super::*;
    use corde_protocol::LapRecord;

    #[test]
    fn one_corner_per_lap_and_same_direction() {
        let (lap, _, _) = tests::synthetic_lap(50.0, 1.4);
        let base = segment(&lap, VehicleClass::Gt3, &CornerConfig::default());
        // Lap A: a right-hander at 300 m and a left-hander at 320 m (chicane).
        let mut a = base.clone();
        a[0].apex_dist_m = 300.0;
        let mut left = base[0].clone();
        left.apex_dist_m = 320.0;
        left.direction = TurnDirection::Left;
        a.push(left);
        let b = a.clone();
        let matched = match_corners(&[a, b], 40.0, 0.0);
        assert_eq!(matched.len(), 2, "{matched:?}");
        assert!(matched.iter().all(|m| m.passes.len() == 2));
        let _ = LapRecord::default();
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use corde_protocol::LapRecord;

    /// A synthetic lap at `hz` Hz: straight, braking into a right-hander,
    /// steady state, throttle out, straight again. Returns the lap and the
    /// sample index range of the corner.
    pub(crate) fn synthetic_lap(hz: f64, g_peak: f64) -> (LapSeries, usize, usize) {
        let dt = 1.0 / hz;
        let n = (12.0 * hz) as usize; // 12 s
        let mut t = Vec::with_capacity(n);
        let mut dist = Vec::with_capacity(n);
        let mut speed = Vec::with_capacity(n);
        let mut brake = Vec::with_capacity(n);
        let mut throttle = Vec::with_capacity(n);
        let mut g_lat = Vec::with_capacity(n);
        let mut g_long = Vec::with_capacity(n);
        let mut d = 0.0;
        for i in 0..n {
            let ts = i as f64 * dt;
            // 0–3 s straight at 220, 3–5 s braking to 90, 5–7 s steady, 7–9 s accelerating, 9–12 s straight.
            let (v, b, th, gl) = if ts < 3.0 {
                (220.0, 0.0, 1.0, 0.0)
            } else if ts < 5.0 {
                let f = (ts - 3.0) / 2.0;
                (220.0 - 130.0 * f, 0.8 - 0.6 * f, 0.0, -1.2 + 1.0 * f)
            } else if ts < 7.0 {
                (90.0, 0.0, 0.1, 0.0)
            } else if ts < 9.0 {
                let f = (ts - 7.0) / 2.0;
                (90.0 + 100.0 * f, 0.0, 0.3 + 0.7 * f, 0.5)
            } else {
                (200.0, 0.0, 1.0, 0.0)
            };
            // Lateral g: ramps up during braking, peaks at the apex, fades on exit.
            let g = if ts < 3.5 {
                0.0
            } else if ts < 5.5 {
                g_peak * (ts - 3.5) / 2.0
            } else if ts < 7.0 {
                g_peak
            } else if ts < 9.0 {
                g_peak * (1.0 - (ts - 7.0) / 2.0)
            } else {
                0.0
            };
            t.push(ts);
            d += v / 3.6 * dt;
            dist.push(d);
            speed.push(v);
            brake.push(b);
            throttle.push(th);
            g_lat.push(g);
            g_long.push(gl);
        }
        let mut lap = LapSeries::new(1, LapRecord::default());
        lap.set("t_s", t)
            .set("lap_dist_m", dist)
            .set("speed_kmh", speed)
            .set("brake", brake)
            .set("throttle", throttle)
            .set("g_lat", g_lat)
            .set("g_long", g_long);
        let s = (3.5 * hz) as usize;
        let e = (9.0 * hz) as usize;
        (lap, s, e)
    }

    #[test]
    fn finds_one_right_hander_with_three_phases() {
        let (lap, s, e) = synthetic_lap(50.0, 1.4);
        let corners = segment(&lap, VehicleClass::Gt3, &CornerConfig::default());
        assert_eq!(corners.len(), 1, "{corners:?}");
        let c = &corners[0];
        assert!(c.start >= s && c.start < s + 60, "start {} vs {s}", c.start);
        assert!(c.end <= e + 2 && c.end > e - 60, "end {} vs {e}", c.end);
        assert_eq!(c.direction, TurnDirection::Right);
        assert!((c.min_speed_kmh - 90.0).abs() < 1.0);
        assert_eq!(c.speed_class, CornerSpeed::Slow);
        assert!((c.max_g_lat - 1.4).abs() < 0.05);
        let count = |p: CornerPhase| c.phases.iter().filter(|x| **x == Some(p)).count();
        assert!(
            count(CornerPhase::Entry) > 20,
            "entry {}",
            count(CornerPhase::Entry)
        );
        assert!(
            count(CornerPhase::Mid) > 20,
            "mid {}",
            count(CornerPhase::Mid)
        );
        assert!(
            count(CornerPhase::Exit) > 20,
            "exit {}",
            count(CornerPhase::Exit)
        );
        // Entry comes before mid, mid before exit.
        let first = |p: CornerPhase| c.phases.iter().position(|x| *x == Some(p)).unwrap();
        let last = |p: CornerPhase| c.phases.iter().rposition(|x| *x == Some(p)).unwrap();
        assert!(last(CornerPhase::Entry) < first(CornerPhase::Exit));
        assert!(first(CornerPhase::Mid) > first(CornerPhase::Entry));
    }

    #[test]
    fn classes_and_matching() {
        assert_eq!(classify_speed(80.0, VehicleClass::Gt3), CornerSpeed::Slow);
        assert_eq!(
            classify_speed(100.0, VehicleClass::Gt3),
            CornerSpeed::Medium
        );
        assert_eq!(
            classify_speed(100.0, VehicleClass::Hypercar),
            CornerSpeed::Slow
        );
        assert_eq!(classify_speed(200.0, VehicleClass::Lmp2), CornerSpeed::Fast);
        let (lap, _, _) = synthetic_lap(50.0, 1.4);
        let a = segment(&lap, VehicleClass::Gt3, &CornerConfig::default());
        let mut b = a.clone();
        b[0].apex_dist_m += 25.0;
        let mut c = a.clone();
        c[0].apex_dist_m += 300.0;
        let matched = match_corners(&[a, b, c], 40.0, 0.0);
        assert_eq!(matched.len(), 2);
        assert_eq!(matched[0].passes.len(), 2);
        assert_eq!(matched[1].passes.len(), 1);
        // Wrap-around: 10 m and 1490 m on a 1500 m track are the same corner.
        let (l2, _, _) = synthetic_lap(50.0, 1.4);
        let mut d = segment(&l2, VehicleClass::Gt3, &CornerConfig::default());
        d[0].apex_dist_m = 10.0;
        let mut e2 = d.clone();
        e2[0].apex_dist_m = 1490.0;
        assert_eq!(match_corners(&[d, e2], 40.0, 1500.0).len(), 1);
    }

    #[test]
    fn no_corner_without_g() {
        let mut lap = LapSeries::new(1, LapRecord::default());
        lap.set("t_s", vec![0.0, 0.02, 0.04]);
        assert!(segment(&lap, VehicleClass::Gt3, &CornerConfig::default()).is_empty());
    }
}
