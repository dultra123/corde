//! Repair of the time and distance axes of sessions recorded before the
//! sample clock was fixed (≤ v0.1.0-alpha.1).
//!
//! Those sessions stamped every sample with the game's *scoring* clock, which
//! only moves every 0.2 s, and stored the scoring lap distance as is: both
//! axes are staircases (ten samples share one timestamp, the distance jumps
//! 14 m at a time at 250 km/h). The wall clock of each sample (`wall_ms`) and
//! the speed were recorded correctly, so the axes can be rebuilt:
//!
//! * time — the scoring clock is right at the start of each step; inside a
//!   step the wall clock gives the offset;
//! * distance — the scoring distance is right at the start of each step;
//!   inside a step the car moves at its recorded speed.
//!
//! Sessions recorded with the fixed clock are left untouched.

/// Share of samples repeating the previous timestamp above which the time
/// axis is considered a staircase.
const QUANTIZED_SHARE: f64 = 0.3;

/// Smallest time step kept between two repaired samples, seconds.
const EPS: f64 = 1e-4;

/// `true` when `t_s` is a staircase (many samples share a timestamp).
pub fn is_quantized(t_s: &[f64]) -> bool {
    if t_s.len() < 20 {
        return false;
    }
    let repeats = t_s.windows(2).filter(|w| w[1] == w[0]).count();
    repeats as f64 > QUANTIZED_SHARE * (t_s.len() - 1) as f64
}

/// Rebuilds `t_s` and `lap_dist_m` in place when `t_s` is a staircase.
/// Returns `true` when the axes were repaired.
///
/// `wall_ms` and `speed_kmh` are the matching samples (NaN where missing).
pub fn repair_axes(
    t_s: &mut [f64],
    lap_dist_m: &mut [f64],
    wall_ms: &[f64],
    speed_kmh: &[f64],
) -> bool {
    let n = t_s.len();
    if lap_dist_m.len() != n || wall_ms.len() != n || speed_kmh.len() != n || !is_quantized(t_s) {
        return false;
    }
    // Steps of the scoring clock: [start, end) index ranges sharing a timestamp.
    let mut steps: Vec<(usize, usize)> = Vec::new();
    let mut start = 0;
    for i in 1..=n {
        if i == n || t_s[i] != t_s[start] {
            steps.push((start, i));
            start = i;
        }
    }
    let raw_t: Vec<f64> = t_s.to_vec();
    let raw_d: Vec<f64> = lap_dist_m.to_vec();
    let max_d = raw_d
        .iter()
        .copied()
        .filter(|d| d.is_finite())
        .fold(0.0, f64::max);

    for (k, &(a, b)) in steps.iter().enumerate() {
        let next = steps.get(k + 1).map(|&(na, _)| na);
        let t0 = raw_t[a];
        let w0 = wall_ms[a];
        // Upper bound of the step: the next scoring timestamp.
        let t_next = next.map(|na| raw_t[na]).filter(|tn| *tn > t0);
        for i in a..b {
            let mut off = if w0.is_finite() && wall_ms[i].is_finite() {
                ((wall_ms[i] - w0) / 1000.0).max(0.0)
            } else {
                0.0
            };
            // Keep the axis strictly increasing whatever the wall clock did:
            // each sample at least EPS after the previous one, the last one
            // at least EPS before the next step.
            let j = (i - a) as f64;
            off = off.max(j * EPS);
            if let Some(tn) = t_next {
                off = off.min((tn - t0) - ((b - i) as f64) * EPS).max(j * EPS);
            }
            t_s[i] = t0 + off;
        }

        // Distance.
        let d0 = raw_d[a];
        let d_next = next.map(|na| raw_d[na]);
        // First step of a lap still showing the end of the previous lap: the
        // line was crossed during this step. Count back from the next step.
        let stale_start =
            k == 0 && max_d > 0.0 && d0 > 0.9 * max_d && d_next.is_some_and(|dn| dn < 0.1 * max_d);
        for i in a..b {
            let v = if speed_kmh[i].is_finite() {
                speed_kmh[i] / 3.6
            } else {
                0.0
            };
            if stale_start {
                let (dn, tn) = (d_next.unwrap_or(0.0), t_next.unwrap_or(t_s[i]));
                lap_dist_m[i] = (dn - v * (tn - t_s[i])).max(0.0);
                continue;
            }
            let mut d = d0 + v * (t_s[i] - t0);
            if let Some(dn) = d_next {
                if dn >= d0 {
                    // Never ahead of the next published distance.
                    d = d.min(dn);
                }
            }
            lap_dist_m[i] = d;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A car at 60 m/s sampled the way old sessions were: scoring clock and
    /// distance in 0.2 s steps, bursts of 6 samples 10 ms apart per step.
    fn staircase(steps: usize, d_start: f64) -> (Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>) {
        let (mut t, mut d, mut w, mut v) = (vec![], vec![], vec![], vec![]);
        for s in 0..steps {
            let t0 = 100.0 + 0.2 * s as f64;
            for j in 0..6 {
                t.push(t0);
                d.push(d_start + 60.0 * 0.2 * s as f64);
                w.push(1_000_000.0 + (0.2 * s as f64 + 0.01 * j as f64) * 1000.0);
                v.push(216.0);
            }
        }
        (t, d, w, v)
    }

    #[test]
    fn detects_a_staircase() {
        let (t, ..) = staircase(10, 0.0);
        assert!(is_quantized(&t));
        let uniform: Vec<f64> = (0..100).map(|i| 100.0 + 0.02 * i as f64).collect();
        assert!(!is_quantized(&uniform));
        assert!(!is_quantized(&[1.0, 1.0, 1.0]));
    }

    #[test]
    fn rebuilds_time_and_distance() {
        let (mut t, mut d, w, v) = staircase(10, 500.0);
        assert!(repair_axes(&mut t, &mut d, &w, &v));
        for i in 1..t.len() {
            assert!(
                t[i] > t[i - 1],
                "time must increase at {i}: {} {}",
                t[i - 1],
                t[i]
            );
            assert!(d[i] >= d[i - 1], "distance must not go back at {i}");
        }
        // Inside a step: 10 ms and 0.6 m per sample.
        assert!((t[1] - t[0] - 0.01).abs() < 1e-9);
        assert!((d[1] - d[0] - 0.6).abs() < 1e-6);
        assert!((t[6] - 100.2).abs() < 1e-9);
        assert!((d[6] - 512.0).abs() < 1e-9);
        // The true position 50 ms into the third step.
        let i = 2 * 6 + 5;
        assert!((d[i] - (500.0 + 60.0 * 0.45)).abs() < 1e-6, "{}", d[i]);
    }

    #[test]
    fn untouched_when_the_clock_is_precise() {
        let mut t: Vec<f64> = (0..100).map(|i| 100.0 + 0.02 * i as f64).collect();
        let mut d: Vec<f64> = (0..100).map(|i| 1.2 * i as f64).collect();
        let (t0, d0) = (t.clone(), d.clone());
        let w: Vec<f64> = (0..100).map(|i| 20.0 * i as f64).collect();
        let v = vec![216.0; 100];
        assert!(!repair_axes(&mut t, &mut d, &w, &v));
        assert_eq!(t, t0);
        assert_eq!(d, d0);
    }

    #[test]
    fn a_lap_starting_on_the_previous_lap_distance_is_counted_back() {
        // First step still shows 4080 m (end of the previous lap), the next 11 m.
        let (mut t, mut d, w, v) = staircase(10, 0.0);
        for x in d.iter_mut().take(6) {
            *x = 4080.0;
        }
        for (s, chunk) in d.chunks_mut(6).enumerate().skip(1) {
            for x in chunk {
                *x = 11.0 + 12.0 * (s - 1) as f64;
            }
        }
        // Make the lap long enough for the "end of lap" test to mean something.
        let (mut t2, mut d2, w2, v2) = staircase(10, 3000.0);
        let off = t.len();
        for x in t2.iter_mut() {
            *x += 2.0;
        }
        t.append(&mut t2);
        d.append(&mut d2);
        let mut w = w;
        w.extend(w2.iter().map(|x| x + 2000.0));
        let mut v = v;
        v.extend(v2);
        assert!(repair_axes(&mut t, &mut d, &w, &v));
        // 11 m at the next step, 60 m/s: the line was crossed ≈ 0.18 s before.
        assert_eq!(d[0], 0.0, "{:?}", &d[..7]);
        assert!((d[5] - 2.0).abs() < 1e-6, "{:?}", &d[..7]);
        assert!((d[6] - 11.0).abs() < 1e-9);
        assert!(d[off] >= 3000.0);
    }

    #[test]
    fn mismatched_lengths_are_left_alone() {
        let (mut t, mut d, w, _) = staircase(10, 0.0);
        assert!(!repair_axes(&mut t, &mut d, &w, &[1.0]));
    }
}
