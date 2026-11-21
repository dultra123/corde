//! Where the car is on the map, worked out from its speed and its heading.
//!
//! The game publishes no coordinates while driving (those of its telemetry files are not
//! in the shared memory). The speed and the heading are exact, though: adding them up
//! draws the circuit. Checked on a real session (Spa, 21 November 2026): a lap closes
//! within 5 to 7 m over 6.9 km, and the outline lies within 17 m of the one Le Mans
//! Ultimate publishes for the same circuit — same orientation, no mirror.
//!
//! * The estimate is put back where it was at each crossing of the line, so nothing
//!   drifts from lap to lap.
//! * What was seen at each place of the lap is remembered: when the car is moved (tow,
//!   back from the garage), it is put where that place was — or nowhere until it reaches
//!   a place already seen.

/// Places of the lap that are remembered (one every ~10 m at Spa).
const BINS: usize = 720;
/// The car moved this much more (or less) than its speed allows: it was moved.
const MOVED_M: f64 = 40.0;
/// Two rows this far apart are not added up (pause, garage).
const MAX_STEP_S: f64 = 0.5;
/// Until the car has covered this much, nothing of the circuit is known: an estimate
/// that is moved to a place never seen simply starts again there. (The game gives a
/// place on the lap of 0 for the first row, before the real one.)
const NOTHING_LEARNT_M: f64 = 50.0;

/// A remembered point, and whether it was seen on the circuit (not in the pit lane).
type Seen = ([f32; 2], bool);

#[derive(Debug, Clone)]
pub struct Odometer {
    /// Metres along the map's two axes (x, z). `None`: not known.
    pos: Option<[f64; 2]>,
    /// Where the line was crossed first.
    line: Option<[f64; 2]>,
    seen: Vec<Option<Seen>>,
    /// Clock and place on the lap of the previous row.
    last: Option<(f64, f32)>,
    started: bool,
    /// Metres added up since the estimate started.
    covered_m: f64,
}

impl Default for Odometer {
    fn default() -> Self {
        Self {
            pos: None,
            line: None,
            seen: vec![None; BINS],
            last: None,
            started: false,
            covered_m: 0.0,
        }
    }
}

/// What one row says of the car.
#[derive(Debug, Clone, Copy)]
pub struct Step {
    /// Session clock, seconds.
    pub now: f64,
    /// Place on the lap, 0..1.
    pub fraction: f32,
    pub speed_ms: f32,
    /// Heading, radians (`None`: the game does not give it).
    pub heading: Option<f32>,
    pub track_m: f32,
    /// In its car, in the world (not the garage screen).
    pub in_world: bool,
    pub in_pits: bool,
}

impl Odometer {
    /// Forgets where the car was a moment ago (another session of the same circuit): what
    /// was learnt of the circuit stays.
    pub fn pause(&mut self) {
        self.last = None;
    }

    /// The position after this row: `[x, 0, z]` in metres, zeros while it is not known.
    pub fn step(&mut self, s: Step) -> [f64; 3] {
        let Some(heading) = s.heading.filter(|_| s.in_world) else {
            self.last = None;
            return [0.0; 3];
        };
        let bin = ((s.fraction.clamp(0.0, 1.0) * BINS as f32) as usize).min(BINS - 1);
        let remembered = self.seen[bin].map(|(p, _)| [f64::from(p[0]), f64::from(p[1])]);
        match self.last {
            Some((t0, f0)) if s.now > t0 && s.now - t0 < MAX_STEP_S => {
                let travelled = f64::from(s.speed_ms.max(0.0)) * (s.now - t0);
                let mut lap = f64::from(s.fraction - f0);
                if lap > 0.5 {
                    lap -= 1.0;
                } else if lap < -0.5 {
                    lap += 1.0;
                }
                let moved = s.track_m > 0.0
                    && (lap * f64::from(s.track_m) - travelled).abs() > MOVED_M + travelled;
                if moved {
                    self.pos = remembered;
                } else if let Some(p) = self.pos.as_mut() {
                    self.covered_m += travelled;
                    p[0] += travelled * f64::from(heading.cos());
                    p[1] += travelled * f64::from(heading.sin());
                    if f0 > 0.9 && s.fraction < 0.1 {
                        // The line: back where it was first crossed.
                        match self.line {
                            Some(line) => *p = line,
                            None => self.line = Some(*p),
                        }
                    }
                } else {
                    // Not known since the car was moved: until a place already seen.
                    self.pos = remembered;
                }
            }
            _ if !self.started => {
                self.started = true;
                self.pos = Some([0.0; 2]);
            }
            // After a pause: where this place of the lap was seen, if it was.
            _ => self.pos = remembered,
        }
        self.last = Some((s.now, s.fraction));
        if self.pos.is_none() && self.covered_m < NOTHING_LEARNT_M {
            // Nothing learnt yet: the estimate starts again from here.
            *self = Self {
                pos: Some([0.0; 2]),
                last: self.last,
                started: true,
                ..Self::default()
            };
        }
        let Some(p) = self.pos else {
            return [0.0; 3];
        };
        let on_track = !s.in_pits;
        // What was seen on the circuit is not replaced by what is seen from the pit lane.
        if on_track || !matches!(self.seen[bin], Some((_, true))) {
            self.seen[bin] = Some(([p[0] as f32, p[1] as f32], on_track));
        }
        [p[0], 0.0, p[1]]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A round circuit of 1 km driven at 50 m/s, 60 rows a second: 20 s a lap.
    const TRACK_M: f32 = 1000.0;
    const SPEED: f32 = 50.0;

    fn row(t: f64, start: f64) -> Step {
        let fraction = (start + t * f64::from(SPEED) / f64::from(TRACK_M)).fract();
        Step {
            now: 100.0 + t,
            fraction: fraction as f32,
            speed_ms: SPEED,
            // Turning right all the way round.
            heading: Some((fraction * std::f64::consts::TAU) as f32),
            track_m: TRACK_M,
            in_world: true,
            in_pits: false,
        }
    }

    fn drive(odo: &mut Odometer, from: f64, to: f64, start: f64) -> Vec<[f64; 3]> {
        let (a, b) = ((from * 60.0) as u64, (to * 60.0) as u64);
        (a..=b)
            .map(|i| odo.step(row(i as f64 / 60.0, start)))
            .collect()
    }

    fn dist(a: [f64; 3], b: [f64; 3]) -> f64 {
        (a[0] - b[0]).hypot(a[2] - b[2])
    }

    #[test]
    fn the_circuit_is_drawn_and_does_not_drift() {
        let mut odo = Odometer::default();
        // From the middle of the lap: the line at 10 s, then every 20 s.
        let path = drive(&mut odo, 0.0, 130.0, 0.5);
        let at = |t: f64| path[(t * 60.0).round() as usize];
        // A circle of 1 km: 318 m across.
        let (lo, hi) = path.iter().fold((f64::MAX, f64::MIN), |(lo, hi), p| {
            (lo.min(p[0]), hi.max(p[0]))
        });
        assert!((hi - lo - 318.3).abs() < 2.0, "{}", hi - lo);
        // The same place a lap later, five laps later: the same point.
        assert!(
            dist(at(15.0), at(35.0)) < 1.0,
            "{}",
            dist(at(15.0), at(35.0))
        );
        assert!(dist(at(15.0), at(115.0)) < 1.0);
        assert!(dist(at(15.0), at(25.0)) > 300.0, "half a lap apart");
        // The line is where it was first crossed.
        assert!(dist(at(10.02), at(110.02)) < 0.5);
    }

    #[test]
    fn a_car_that_is_moved_is_put_where_that_place_was() {
        let mut odo = Odometer::default();
        // From 30 % of the lap (the pit stall, say), two laps.
        let path = drive(&mut odo, 0.0, 44.0, 0.3);
        let stall = path[1];
        let far = path[(34.0 * 60.0) as usize];
        assert!(dist(stall, far) > 100.0);
        // Towed back to the stall while driving elsewhere: the next row is 30 % again.
        let towed = odo.step(Step {
            now: 100.0 + 44.02,
            fraction: 0.3,
            ..row(0.0, 0.3)
        });
        assert!(dist(towed, stall) < 3.0, "{}", dist(towed, stall));
        // The garage: no position; back in the car at the stall: the stall.
        let out = odo.step(Step {
            now: 150.0,
            in_world: false,
            ..row(0.0, 0.3)
        });
        assert_eq!(out, [0.0; 3]);
        let back = odo.step(Step {
            now: 190.0,
            fraction: 0.3,
            ..row(0.0, 0.3)
        });
        assert!(dist(back, stall) < 3.0);

        // Moved to a place never seen: nowhere, until a place that was.
        let mut odo = Odometer::default();
        drive(&mut odo, 0.0, 4.0, 0.0); // the first 20 % of the lap
        let lost = odo.step(Step {
            now: 100.0 + 4.02,
            fraction: 0.6,
            ..row(0.0, 0.6)
        });
        assert_eq!(lost, [0.0; 3]);
        // … it drives on from 60 %, crosses the line, and is found at the start of the lap.
        let found = (1..=600)
            .map(|i| {
                let t = f64::from(i) / 60.0;
                odo.step(Step {
                    now: 100.0 + 4.02 + t,
                    ..row(t, 0.6)
                })
            })
            .find(|p| *p != [0.0; 3]);
        assert!(found.is_some());

        // No heading: no position, and no panic on odd values.
        let mut odo = Odometer::default();
        let none = odo.step(Step {
            heading: None,
            ..row(0.0, 0.0)
        });
        assert_eq!(none, [0.0; 3]);
        for fraction in [f32::NAN, -3.0, 7.0] {
            odo.step(Step {
                fraction,
                ..row(1.0, 0.0)
            });
        }
    }

    #[test]
    fn a_first_row_with_a_wrong_place_does_not_hide_the_first_lap() {
        // What the game does when the driver gets in the car: one row with the place on
        // the lap at 0, then the real one (the pit stall, 30 % here), standing still.
        let mut odo = Odometer::default();
        let still = |now: f64, fraction: f32| Step {
            now,
            fraction,
            speed_ms: 0.0,
            ..row(0.0, 0.3)
        };
        odo.step(still(100.0, 0.0));
        for i in 1..120 {
            odo.step(still(100.0 + f64::from(i) / 60.0, 0.3));
        }
        // Then it drives: a position from the first metres, and the same point a lap on.
        let from = 102.0;
        let path: Vec<[f64; 3]> = (0..=(45 * 60))
            .map(|i| {
                let t = f64::from(i) / 60.0;
                odo.step(Step {
                    now: from + t,
                    ..row(t, 0.3)
                })
            })
            .collect();
        assert!(path[120] != [0.0; 3], "known from the start");
        assert!(
            dist(path[5 * 60], path[25 * 60]) < 2.0,
            "the same place a lap later"
        );
        assert!(dist(path[5 * 60], path[15 * 60]) > 300.0);
    }

    #[test]
    fn the_pit_lane_does_not_replace_the_circuit() {
        let mut odo = Odometer::default();
        drive(&mut odo, 0.0, 25.0, 0.0);
        let on_track = odo.seen[BINS / 2].unwrap();
        assert!(on_track.1);
        // The same place of the lap seen again from the pit lane, 30 m aside.
        odo.pos = odo.pos.map(|p| [p[0] + 30.0, p[1]]);
        odo.step(Step {
            in_pits: true,
            ..row(30.0, 0.0)
        });
        assert_eq!(odo.seen[BINS / 2], Some(on_track));
    }
}
