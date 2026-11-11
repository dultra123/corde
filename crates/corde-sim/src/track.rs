//! Synthetic closed circuit: a Catmull-Rom spline through ~27 control points
//! laid out like Paul Ricard (pit straight, Verrerie S, technical middle
//! section, long Mistral straight with its chicane, fast Signes, Beausset
//! double right, twisty run back to the line), resampled every metre with a
//! precomputed, physically plausible speed profile.
//!
//! Everything here is pure, deterministic and computed once at start-up.

use std::f64::consts::{PI, TAU};

/// Longitudinal / lateral performance envelope used to build the speed profile.
#[derive(Debug, Clone, Copy)]
pub struct Envelope {
    /// Maximum lateral acceleration, m/s².
    pub a_lat: f64,
    /// Maximum braking deceleration, m/s².
    pub a_brake: f64,
    /// Maximum longitudinal acceleration at low speed (traction limited), m/s².
    pub a_acc_max: f64,
    /// Power-to-mass term: available acceleration ≈ `power_accel / v` at speed.
    pub power_accel: f64,
    /// Aerodynamic drag term: `a_drag = drag · v²`.
    pub drag: f64,
    /// Speed floor of the final profile, m/s.
    pub v_min: f64,
    /// Speed ceiling of the final profile, m/s.
    pub v_max: f64,
}

impl Envelope {
    pub fn gt3() -> Self {
        Self {
            a_lat: 1.75 * 9.81,
            a_brake: 1.55 * 9.81,
            a_acc_max: 6.0,
            power_accel: 315.0,
            drag: 0.000_55,
            v_min: 80.0 / 3.6,
            v_max: 300.0 / 3.6,
        }
    }

    pub fn hypercar() -> Self {
        Self {
            a_lat: 2.3 * 9.81,
            a_brake: 1.9 * 9.81,
            a_acc_max: 7.5,
            power_accel: 485.0,
            drag: 0.000_48,
            v_min: 85.0 / 3.6,
            v_max: 335.0 / 3.6,
        }
    }

    /// Full-throttle acceleration available at speed `v` (before drag).
    pub fn a_available(&self, v: f64) -> f64 {
        self.a_acc_max.min(self.power_accel / v.max(5.0))
    }

    /// Throttle and brake (0..1) needed to produce `a_long` at speed `v`.
    pub fn pedals(&self, a_long: f64, v: f64) -> (f64, f64) {
        let a_drag = self.drag * v * v;
        let avail = self.a_available(v);
        if a_long >= -a_drag - 0.25 {
            let throttle = ((a_long + a_drag) / avail).clamp(0.0, 1.0);
            (throttle, 0.0)
        } else {
            let brake = ((-a_long - a_drag) / self.a_brake).clamp(0.0, 1.0);
            (0.0, brake)
        }
    }
}

/// One arc-length sample of the track.
#[derive(Debug, Clone, Copy, Default)]
pub struct Sample {
    pub x: f64,
    pub z: f64,
    /// Heading `atan2(dx, dz)`, radians (same convention as `corde_core::derive`).
    pub heading: f64,
    /// Curvature, 1/m, **positive = turning right**.
    pub curvature: f64,
    /// Nominal speed, m/s.
    pub v: f64,
    /// Nominal longitudinal acceleration `v·dv/ds`, m/s².
    pub a_long: f64,
    /// Nominal time since the start line, seconds.
    pub t: f64,
}

#[derive(Debug, Clone)]
pub struct Track {
    pub length_m: f64,
    ds: f64,
    samples: Vec<Sample>,
    /// Distance at which sectors 1 and 2 end.
    pub sector_ends_m: [f64; 2],
    /// Lap time of the nominal profile (equals the requested lap time).
    pub lap_time_s: f64,
    /// ∫ throttle dt over one nominal lap, seconds (used to scale fuel use).
    pub throttle_integral_s: f64,
    pub envelope: Envelope,
}

/// Control points (metres) of a clockwise loop: pit straight on top heading
/// east, technical section down the east side, long back straight heading
/// west with a chicane, fast right-hander and twisty return up the west side.
const CONTROL_POINTS: [(f64, f64); 27] = [
    (0.0, 500.0),      // start / finish
    (350.0, 500.0),    // pit straight
    (700.0, 500.0),    // end of pit straight
    (880.0, 440.0),    // T1 (Verrerie) right
    (930.0, 330.0),    // S left
    (860.0, 240.0),    // S right
    (900.0, 120.0),    // Camp
    (820.0, 20.0),     // Hotel
    (640.0, -40.0),    // Hotel exit
    (560.0, -130.0),   // approach Sainte-Baume
    (700.0, -300.0),   // Sainte-Baume (tight right)
    (600.0, -330.0),   // onto the back straight
    (300.0, -330.0),   // Mistral
    (-50.0, -330.0),   // Mistral before chicane
    (-130.0, -290.0),  // chicane right
    (-230.0, -290.0),  // chicane
    (-310.0, -330.0),  // chicane left
    (-700.0, -330.0),  // Mistral
    (-1050.0, -330.0), // Mistral end
    (-1250.0, -280.0), // Signes (very fast right)
    (-1330.0, -150.0), // Signes exit
    (-1300.0, 0.0),    // Beausset 1
    (-1180.0, 80.0),   // Beausset 2
    (-1150.0, 200.0),  // Bendor
    (-1000.0, 300.0),  // Village
    (-900.0, 420.0),   // La Tour
    (-700.0, 480.0),   // Le Pont
];

fn catmull_rom(
    p0: (f64, f64),
    p1: (f64, f64),
    p2: (f64, f64),
    p3: (f64, f64),
    t: f64,
) -> (f64, f64) {
    let t2 = t * t;
    let t3 = t2 * t;
    let f = |a: f64, b: f64, c: f64, d: f64| {
        0.5 * ((2.0 * b)
            + (-a + c) * t
            + (2.0 * a - 5.0 * b + 4.0 * c - d) * t2
            + (-a + 3.0 * b - 3.0 * c + d) * t3)
    };
    (f(p0.0, p1.0, p2.0, p3.0), f(p0.1, p1.1, p2.1, p3.1))
}

fn wrap_angle(a: f64) -> f64 {
    let mut a = a % TAU;
    if a > PI {
        a -= TAU;
    } else if a < -PI {
        a += TAU;
    }
    a
}

impl Track {
    /// Build the circuit scaled to `length_m`, with a speed profile whose lap
    /// time equals `lap_time_s` once clamped into `[env.v_min, env.v_max]`.
    pub fn paul_ricard_like(length_m: f64, lap_time_s: f64, env: Envelope) -> Self {
        // 1. Dense polyline from the closed spline.
        let n_ctrl = CONTROL_POINTS.len();
        const SUB: usize = 64;
        let mut poly: Vec<(f64, f64)> = Vec::with_capacity(n_ctrl * SUB);
        for i in 0..n_ctrl {
            let p0 = CONTROL_POINTS[(i + n_ctrl - 1) % n_ctrl];
            let p1 = CONTROL_POINTS[i];
            let p2 = CONTROL_POINTS[(i + 1) % n_ctrl];
            let p3 = CONTROL_POINTS[(i + 2) % n_ctrl];
            for k in 0..SUB {
                poly.push(catmull_rom(p0, p1, p2, p3, k as f64 / SUB as f64));
            }
        }
        // Cumulative raw length (closed).
        let m = poly.len();
        let mut cum = Vec::with_capacity(m + 1);
        cum.push(0.0);
        for i in 0..m {
            let (x0, z0) = poly[i];
            let (x1, z1) = poly[(i + 1) % m];
            cum.push(cum[i] + ((x1 - x0).powi(2) + (z1 - z0).powi(2)).sqrt());
        }
        let raw_len = cum[m];
        let scale = length_m / raw_len;

        // 2. Uniform resampling (≈ 1 m).
        let n = length_m.round().max(100.0) as usize;
        let ds = length_m / n as f64;
        let mut samples = vec![Sample::default(); n];
        let mut j = 0usize;
        for (k, smp) in samples.iter_mut().enumerate() {
            let s_raw = (k as f64 * ds) / scale;
            while j + 1 < m && cum[j + 1] < s_raw {
                j += 1;
            }
            let seg = (cum[j + 1] - cum[j]).max(1e-9);
            let f = ((s_raw - cum[j]) / seg).clamp(0.0, 1.0);
            let (x0, z0) = poly[j];
            let (x1, z1) = poly[(j + 1) % m];
            smp.x = (x0 + (x1 - x0) * f) * scale;
            smp.z = (z0 + (z1 - z0) * f) * scale;
        }

        // 3. Heading and curvature (central differences, circular).
        let heading: Vec<f64> = (0..n)
            .map(|k| {
                let a = samples[(k + n - 1) % n];
                let b = samples[(k + 1) % n];
                (b.x - a.x).atan2(b.z - a.z)
            })
            .collect();
        let raw_curv: Vec<f64> = (0..n)
            .map(|k| wrap_angle(heading[(k + 1) % n] - heading[(k + n - 1) % n]) / (2.0 * ds))
            .collect();
        const HALF_WIN: usize = 12;
        let curvature: Vec<f64> = (0..n)
            .map(|k| {
                let mut acc = 0.0;
                for d in 0..=(2 * HALF_WIN) {
                    acc += raw_curv[(k + n + d - HALF_WIN) % n];
                }
                acc / (2 * HALF_WIN + 1) as f64
            })
            .collect();
        for (k, smp) in samples.iter_mut().enumerate() {
            smp.heading = heading[k];
            smp.curvature = curvature[k];
        }

        // 4. Speed profile: corner limit, then forward (traction/power) and
        //    backward (braking) passes, run twice around the loop for closure.
        let v_cap_raw = env.v_max * 1.6;
        let mut v: Vec<f64> = curvature
            .iter()
            .map(|c| (env.a_lat / c.abs().max(1e-6)).sqrt().min(v_cap_raw))
            .collect();
        for _ in 0..2 {
            for i in 0..(2 * n) {
                let k = i % n;
                let nk = (k + 1) % n;
                let a = (env.a_available(v[k]) - env.drag * v[k] * v[k]).max(0.3);
                let reach = (v[k] * v[k] + 2.0 * a * ds).sqrt();
                if reach < v[nk] {
                    v[nk] = reach;
                }
            }
            for i in (0..(2 * n)).rev() {
                let k = i % n;
                let nk = (k + 1) % n;
                let reach = (v[nk] * v[nk] + 2.0 * env.a_brake * ds).sqrt();
                if reach < v[k] {
                    v[k] = reach;
                }
            }
        }

        // 5. Global scale so that the clamped profile hits the requested lap time.
        let lap_time_for = |scale: f64| -> f64 {
            let mut t = 0.0;
            for k in 0..n {
                let a = (v[k] * scale).clamp(env.v_min, env.v_max);
                let b = (v[(k + 1) % n] * scale).clamp(env.v_min, env.v_max);
                t += ds / (0.5 * (a + b));
            }
            t
        };
        let (mut lo, mut hi) = (0.2, 4.0);
        for _ in 0..80 {
            let mid = 0.5 * (lo + hi);
            if lap_time_for(mid) > lap_time_s {
                lo = mid; // too slow → scale up
            } else {
                hi = mid;
            }
        }
        let scale_v = 0.5 * (lo + hi);
        let v: Vec<f64> = v
            .iter()
            .map(|x| (x * scale_v).clamp(env.v_min, env.v_max))
            .collect();

        // 6. Times and longitudinal acceleration.
        let mut t = 0.0;
        let mut throttle_integral = 0.0;
        for k in 0..n {
            let prev = v[(k + n - 1) % n];
            let next = v[(k + 1) % n];
            samples[k].v = v[k];
            samples[k].a_long = (next * next - prev * prev) / (4.0 * ds);
            samples[k].t = t;
            let dt = ds / (0.5 * (v[k] + next));
            t += dt;
            throttle_integral += env.pedals(samples[k].a_long, v[k]).0 * dt;
        }
        let total = t;

        // 7. Sector boundaries at 27 % and 54 % of the nominal lap time
        //    (ratios observed on real Paul Ricard 1A results).
        let find_t = |target: f64| -> f64 {
            samples
                .iter()
                .position(|s| s.t >= target)
                .map(|k| k as f64 * ds)
                .unwrap_or(length_m * 0.5)
        };
        let sector_ends_m = [find_t(0.27 * total), find_t(0.54 * total)];

        Self {
            length_m,
            ds,
            samples,
            sector_ends_m,
            lap_time_s: total,
            throttle_integral_s: throttle_integral,
            envelope: env,
        }
    }

    /// Wrap a distance into `[0, length)`.
    pub fn wrap(&self, s: f64) -> f64 {
        let r = s % self.length_m;
        if r < 0.0 {
            r + self.length_m
        } else {
            r
        }
    }

    /// Interpolated sample at distance `s` (any value, wrapped).
    pub fn at(&self, s: f64) -> Sample {
        let n = self.samples.len();
        let s = self.wrap(s);
        let pos = s / self.ds;
        let i = (pos.floor() as usize).min(n - 1);
        let f = pos - i as f64;
        let a = self.samples[i];
        let b = self.samples[(i + 1) % n];
        let lerp = |x: f64, y: f64| x + (y - x) * f;
        let t_b = if i + 1 == n { self.lap_time_s } else { b.t };
        Sample {
            x: lerp(a.x, b.x),
            z: lerp(a.z, b.z),
            heading: a.heading + wrap_angle(b.heading - a.heading) * f,
            curvature: lerp(a.curvature, b.curvature),
            v: lerp(a.v, b.v),
            a_long: lerp(a.a_long, b.a_long),
            t: lerp(a.t, t_b),
        }
    }

    /// Nominal time since the line at distance `s`.
    pub fn time_at(&self, s: f64) -> f64 {
        self.at(s).t
    }

    /// Sector number 1..=3 at distance `s`.
    pub fn sector(&self, s: f64) -> u8 {
        let s = self.wrap(s);
        if s < self.sector_ends_m[0] {
            1
        } else if s < self.sector_ends_m[1] {
            2
        } else {
            3
        }
    }

    /// Lap time obtained when driving the nominal profile multiplied by
    /// `factor` and capped at `v_cap` (used for the AI classes).
    pub fn lap_time_scaled(&self, factor: f64, v_cap: f64) -> f64 {
        let n = self.samples.len();
        let mut t = 0.0;
        for k in 0..n {
            let a = (self.samples[k].v * factor).min(v_cap);
            let b = (self.samples[(k + 1) % n].v * factor).min(v_cap);
            t += self.ds / (0.5 * (a + b));
        }
        t
    }

    /// Unit vector perpendicular to the heading (to the right of the car) in
    /// the (x, z) plane — used for lateral offsets of the racing line.
    pub fn right_normal(heading: f64) -> (f64, f64) {
        // forward = (sin h, cos h); right = rotate forward by -90° in (x,z)
        (heading.cos(), -heading.sin())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_matches_requested_lap_time_and_bounds() {
        let env = Envelope::gt3();
        let track = Track::paul_ricard_like(5698.6, 115.0, env);
        assert!(
            (track.lap_time_s - 115.0).abs() < 0.05,
            "lap time {}",
            track.lap_time_s
        );
        assert!((track.at(0.0).t).abs() < 1e-9);
        let mut vmin = f64::MAX;
        let mut vmax = 0.0f64;
        let mut s = 0.0;
        while s < track.length_m {
            let smp = track.at(s);
            vmin = vmin.min(smp.v);
            vmax = vmax.max(smp.v);
            s += 7.0;
        }
        assert!(
            vmin >= env.v_min - 1e-6 && vmin < 30.0,
            "vmin {}",
            vmin * 3.6
        );
        assert!(
            vmax <= env.v_max + 1e-6 && vmax > 75.0,
            "vmax {}",
            vmax * 3.6
        );
        assert!(track.sector_ends_m[0] > 500.0 && track.sector_ends_m[0] < track.sector_ends_m[1]);
        assert!(track.sector_ends_m[1] < track.length_m - 500.0);
        assert!(track.throttle_integral_s > 30.0 && track.throttle_integral_s < 110.0);
    }

    #[test]
    fn mostly_right_handers_and_closed() {
        let track = Track::paul_ricard_like(5698.6, 115.0, Envelope::gt3());
        let mut right = 0;
        let mut left = 0;
        let mut s = 0.0;
        while s < track.length_m {
            let c = track.at(s).curvature;
            if c > 0.002 {
                right += 1;
            } else if c < -0.002 {
                left += 1;
            }
            s += 5.0;
        }
        assert!(right > left, "right {right} left {left}");
        let a = track.at(0.0);
        let b = track.at(track.length_m - 0.001);
        assert!(((a.x - b.x).powi(2) + (a.z - b.z).powi(2)).sqrt() < 1.0);
    }
}
