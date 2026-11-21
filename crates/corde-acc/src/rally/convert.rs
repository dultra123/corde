//! The pages of Assetto Corsa Rally → a `TelemetryFrame`, each run of a stage as one lap.
//!
//! The pages are Competizione's: [`crate::convert::Converter`] reads them, after the
//! temperatures were brought back from kelvins. What is specific to a rally is then put
//! in the frame:
//!
//! * **A run is a lap.** `lap_number` moves when a run starts (the clock leaves zero) and
//!   when one is given up (the stage restarted before the finish); it is −1 on the start
//!   line before the first run. At the finish `laps_completed` goes up and `last_lap_s`
//!   is the clock where it froze — the time of the game, never one counted here.
//! * **Outside a run** (waiting on the start line, the stop zone after the finish) the
//!   car is reported in the pits, so that the wait is not taken for a lap.
//! * **A run given up** is a lap without a time.
//! * **The finish** is the clock freezing while the car keeps rolling: a pause freezes
//!   both, and is not a finish.
//! * **The length of the stage** is the one the static page gives when it does, else the
//!   one measured on the first run that finished. Until one is known, the stage is taken
//!   to be twice what was driven so far (the car « half-way »): the lap tracker needs a
//!   length to tell a finish from a run given up.
//!
//! Everything here follows what was observed by others; `TODO(verify-in-game)` marks what
//! matters most to confirm.

use crate::convert::Converter;
use crate::layout::{status, Physics};
use crate::reader::Pages;
use corde_protocol::{GameId, GamePhase, TelemetryFrame, YellowFlagState};
use std::time::Duration;

/// Kelvins → °C.
const KELVIN: f32 = 273.15;
/// Above this, a temperature of the pages is in kelvins (no air, road or tyre is at
/// 200 °C when a session starts; in kelvins none is ever below).
const KELVIN_FROM: f32 = 200.0;
/// The car rolled this far past the place where the clock froze: it was the finish line
/// (a pause freezes the car with the clock).
const FINISH_ROLL_M: f32 = 2.0;
/// … and this far is enough to show the car at the end of the stage meanwhile.
const FINISH_HINT_M: f32 = 0.3;
/// The least a stage is taken to be long while its length is not known.
const PROVISIONAL_MIN_M: f32 = 100.0;

/// Are these the pages of Assetto Corsa Rally? Its air is in kelvins. `None` when the
/// page is blank (pause, stop zone, replay): it does not say.
pub fn is_rally(p: &Physics) -> Option<bool> {
    let air = p.air_temp;
    if !air.is_finite() || air == 0.0 {
        return None;
    }
    Some(air > KELVIN_FROM)
}

/// The physics page while the game is not simulating the car.
pub fn is_blank(p: &Physics) -> bool {
    let (air, road, core) = (p.air_temp, p.road_temp, p.tyre_core_temperature);
    air == 0.0 && road == 0.0 && core.iter().all(|t| *t == 0.0)
}

fn celsius(k: f32) -> f32 {
    if k.is_finite() && k > KELVIN_FROM {
        k - KELVIN
    } else {
        k
    }
}

/// The physics page with its temperatures in °C.
fn in_celsius(mut p: Physics) -> Physics {
    p.air_temp = celsius(p.air_temp);
    p.road_temp = celsius(p.road_temp);
    p.water_temp = celsius(p.water_temp);
    let (mut core, mut brake, mut tyre) = (p.tyre_core_temperature, p.brake_temp, p.tyre_temp);
    for i in 0..4 {
        core[i] = celsius(core[i]);
        brake[i] = celsius(brake[i]);
        tyre[i] = celsius(tyre[i]);
    }
    p.tyre_core_temperature = core;
    p.brake_temp = brake;
    p.tyre_temp = tyre;
    p
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Run {
    /// Not in a run: on the start line, in the stop zone, or a stage joined on the way.
    #[default]
    Out,
    Running,
}

/// Keeps what has to be remembered from one frame to the next. One per connection to the
/// game.
#[derive(Debug, Default)]
pub struct RallyConverter {
    inner: Converter,
    /// What the runs belong to: they are forgotten when it changes.
    stage: Option<(String, i32)>,
    run: Run,
    /// The clock was seen at zero: a run may start.
    armed: bool,
    /// Runs started and runs given up so far.
    boundaries: i32,
    finished: i32,
    last_ms: Option<i32>,
    best_ms: Option<i32>,
    /// Distance along the stage on the start line.
    start_m: f32,
    /// Counter of the graphics page, its clock and its distance when it was last rewritten.
    seen: Option<(i32, i32, f32)>,
    /// Where the car was when the clock stopped moving.
    frozen_at_m: Option<f32>,
    /// The finish line was crossed: the time, and where the clock froze. Counted on the
    /// next frame.
    crossed: Option<(i32, f32)>,
    /// Length of the stage measured on a run that finished.
    measured_m: Option<f32>,
}

impl RallyConverter {
    pub fn new() -> Self {
        Self::default()
    }

    /// The session clock, seconds.
    pub fn elapsed_s(&self) -> f64 {
        self.inner.elapsed_s()
    }

    /// One frame from one copy of the pages. `now` is a monotonic clock (any origin).
    pub fn frame(&mut self, pages: &Pages, now: Duration, source_id: &str) -> TelemetryFrame {
        let blank = is_blank(&pages.physics);
        let pages = Pages {
            physics: in_celsius(pages.physics),
            ..*pages
        };
        let mut f = self.inner.frame(&pages, now, source_id);
        let g = &pages.graphics;
        let elapsed = self.inner.elapsed_s();

        // Another stage, or another kind of session: other runs. The name of the stage
        // arriving late (the static page is written after the stage loaded) is the same
        // stage: its runs are kept.
        let stage = (f.session.track_name.clone(), g.session);
        if self.stage.as_ref() != Some(&stage) {
            let named_late = self
                .stage
                .as_ref()
                .is_some_and(|(name, kind)| name.is_empty() && *kind == stage.1);
            if named_late {
                self.stage = Some(stage);
            } else {
                *self = Self {
                    inner: std::mem::take(&mut self.inner),
                    stage: Some(stage),
                    ..Self::default()
                };
            }
        }

        // --- the runs, from the clock of the game.
        let clock = g.i_current_time.max(0);
        let dist = if g.distance_traveled.is_finite() {
            g.distance_traveled
        } else {
            0.0
        };
        // The frame on which the finish is counted still belongs to the run. It is the
        // one after the frame that showed the car at the end of the stage, so that the
        // lap tracker has seen the end before the count moves.
        let mut finished_now = false;
        if let Some((time_ms, at)) = self.crossed.take() {
            self.finished += 1;
            self.last_ms = Some(time_ms);
            self.best_ms = Some(self.best_ms.map_or(time_ms, |b| b.min(time_ms)));
            if at - self.start_m > PROVISIONAL_MIN_M {
                self.measured_m = Some(at - self.start_m);
            }
            self.run = Run::Out;
            self.armed = false;
            self.frozen_at_m = None;
            finished_now = true;
        }
        let rewritten = self.seen.is_none_or(|(id, _, _)| id != g.packet_id);
        if rewritten && !finished_now && g.status != status::REPLAY {
            let before = self.seen.map(|(_, c, _)| c);
            if clock == 0 {
                // On the start line (for the first time, or the stage was restarted).
                if self.run == Run::Running {
                    // The run in progress was given up: it ends here.
                    self.boundaries += 1;
                }
                self.run = Run::Out;
                self.armed = true;
                self.start_m = dist;
                self.frozen_at_m = None;
            } else if self.run == Run::Out {
                // The clock left zero: a run starts — when the game is driving the car
                // (a replay runs the clock too, on a blank physics page).
                if self.armed && before == Some(0) && !blank {
                    self.begin();
                }
            } else if before.is_some_and(|b| clock < b) {
                // A restart of which not one page at zero was seen: the clock reads less
                // than it did. The run given up ends here, another starts.
                self.begin();
            } else if before == Some(clock) {
                // The clock stopped. The finish line, when the car rolls on.
                let at = *self.frozen_at_m.get_or_insert(dist);
                if dist - at >= FINISH_ROLL_M {
                    self.crossed = Some((clock, at));
                }
            } else {
                self.frozen_at_m = None;
            }
        }
        if rewritten {
            self.seen = Some((g.packet_id, clock, dist));
        }

        // --- the frame.
        let running = self.run == Run::Running || finished_now;
        let along = if running {
            (dist - self.start_m).max(0.0)
        } else {
            0.0
        };
        // The clock stopped and the car rolls on: the end of the stage, whatever the
        // length says.
        let at_the_end = finished_now
            || self.crossed.is_some()
            || self
                .frozen_at_m
                .is_some_and(|at| self.run == Run::Running && dist - at >= FINISH_HINT_M);
        let published = pages.statics.track_spline_length;
        let known = if published.is_finite() && published > 100.0 {
            Some(published)
        } else {
            self.measured_m
        };
        let (length, fraction) = match known {
            _ if at_the_end => (known.unwrap_or(along.max(PROVISIONAL_MIN_M)), 1.0),
            Some(len) => (len, (along / len).clamp(0.0, 1.0)),
            None if running => (2.0 * along.max(PROVISIONAL_MIN_M), 0.5),
            None => (0.0, 0.0),
        };
        let fraction = if running { fraction } else { 0.0 };
        let s = &mut f.session;
        s.game = GameId::AcRally;
        s.track_length_m = length;
        s.phase = if g.status == status::PAUSE {
            GamePhase::Paused
        } else if g.status == status::LIVE {
            GamePhase::Green
        } else {
            GamePhase::Unknown
        };
        s.yellow_flag = YellowFlagState::None;
        s.sector_flags = [0; 3];
        s.max_laps = 0;
        s.num_vehicles = 1;
        s.in_realtime = matches!(g.status, status::LIVE | status::PAUSE);

        let time_s = |ms: Option<i32>| ms.filter(|ms| *ms > 0).map(|ms| ms as f32 / 1000.0);
        let (number, completed) = (self.boundaries - 1, self.finished);
        let current_s = if running { clock as f32 / 1000.0 } else { 0.0 };
        let (last, best) = (time_s(self.last_ms), time_s(self.best_ms));
        if let Some(p) = f.player.as_mut() {
            let lap = &mut p.lap;
            lap.lap_number = number;
            lap.laps_completed = completed;
            // From the clock of the game: the session clock starts again when the name
            // of the stage arrives.
            lap.lap_start_s = elapsed - f64::from(current_s);
            lap.current_lap_s = current_s;
            lap.time_into_lap_s = current_s;
            lap.current_sector = ((fraction * 3.0) as u8).min(2) + 1;
            lap.in_pit_lane = false;
            lap.lap_invalidated = false;
            lap.delta_best_s = 0.0;
            lap.last_lap_s = last;
            lap.best_lap_s = best;
            lap.estimated_lap_s = 0.0;
            lap.lap_dist_m = along;
            lap.lap_fraction = fraction;
            // The game counts the runs itself: a run given up near the end is never
            // timed by Corde's own stopwatch.
            lap.count_lap_flag = Some(if running { 2 } else { 0 });
            // Outside a run the car is « in the pits »: the wait is not a lap.
            p.in_pits = !running;
            p.in_garage = false;
            p.place = 1;
            p.gaps = Default::default();
            if let Some(line) = f.standings.first_mut() {
                line.place = 1;
                line.class_place = 1;
                line.laps_completed = completed;
                line.sector = p.lap.current_sector;
                line.lap_dist_m = along;
                line.lap_fraction = fraction;
                line.in_pits = p.in_pits;
                line.in_garage = false;
                line.best_lap_s = best;
                line.last_lap_s = last;
                line.time_into_lap_s = current_s;
                line.estimated_lap_s = 0.0;
            }
        }
        f
    }

    /// A run starts.
    fn begin(&mut self) {
        self.boundaries += 1;
        self.run = Run::Running;
        self.armed = false;
        self.frozen_at_m = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytemuck::Zeroable;

    #[test]
    fn kelvins_tell_the_game_and_become_celsius() {
        let mut p: Physics = Zeroable::zeroed();
        assert_eq!(is_rally(&p), None, "a blank page does not say");
        assert!(is_blank(&p));
        p.air_temp = 22.0;
        assert_eq!(is_rally(&p), Some(false));
        p.air_temp = 295.15;
        p.road_temp = 300.15;
        p.water_temp = 343.6;
        p.tyre_core_temperature = [363.15; 4];
        p.brake_temp = [322.0; 4];
        assert_eq!(is_rally(&p), Some(true));
        assert!(!is_blank(&p));
        let c = in_celsius(p);
        let (air, water, core, brake) = (
            c.air_temp,
            c.water_temp,
            c.tyre_core_temperature,
            c.brake_temp,
        );
        assert!((air - 22.0).abs() < 1e-3);
        assert!((water - 70.45).abs() < 1e-3);
        assert!((core[0] - 90.0).abs() < 1e-3);
        assert!((brake[3] - 48.85).abs() < 1e-3);
        // Already in °C (a later build of the game): left alone.
        assert_eq!(celsius(85.0), 85.0);
        assert_eq!(celsius(f32::NAN).to_bits(), f32::NAN.to_bits());
    }
}
