//! Turns a stream of [`TelemetryFrame`]s into completed [`LapRecord`]s.
//!
//! # The game is the referee
//!
//! A lap time is only "official" when the game says so. Two facts, both
//! observed on real Le Mans Ultimate sessions, drive the design:
//!
//! * The lap **counter** of the telemetry block (≈100 Hz) moves up to 0.2 s
//!   before the scoring block (≈5 Hz) publishes the official time of the lap
//!   that just ended. Reading `last_lap_s` at the moment the counter moves
//!   returns the time of the *previous* lap.
//! * The game does not time every lap: a lap with a track-limits cut, or a lap
//!   abandoned with Esc, is counted (`laps_completed` goes up) with no time
//!   (`last_lap_s == None`, shown `--.---` in game).
//!
//! So a lap is closed in two steps:
//!
//! 1. **Boundary** — the telemetry lap counter goes up (or, when it does not,
//!    the scoring lap count does). The accumulated statistics are frozen and
//!    the duration between the two line crossings is *measured* on the session
//!    clock.
//! 2. **Confirmation** — the scoring block counts the lap. Its official time is
//!    taken from that very update and cross-checked against the measured
//!    duration. No official time, or one that does not match, means the game
//!    did not time the lap: it is reported with `timed_by_game = false` and
//!    can never be a best lap.
//!
//! A lap that never got near the end of the track before the counter moved
//! (return to garage) is reported as [`LapNote::Aborted`].

use corde_protocol::{GamePhase, LapNote, LapRecord, PlayerTelemetry, SessionType, TelemetryFrame};

/// Largest accepted gap between the official time and the measured duration.
const TIME_TOLERANCE_S: f64 = 0.75;
/// How long (session clock) the scoring block may take to count a lap.
const CONFIRM_TIMEOUT_S: f64 = 3.0;
/// The car must have been seen beyond this fraction of the lap for a counter
/// change to be a line crossing.
const END_OF_LAP_FRACTION: f32 = 0.9;
/// Lap fractions seen during the first seconds of a lap are ignored: the lap
/// distance of the previous lap may linger for a few frames.
const FRACTION_WARMUP_S: f64 = 2.0;
/// Nothing shorter than this is a lap.
const MIN_LAP_S: f64 = 10.0;
/// An abandoned lap is only reported when the car actually went on track.
const ABORTED_MIN_SPEED_KMH: f32 = 30.0;

#[derive(Debug, Clone, Default)]
struct Accumulator {
    lap_number: i32,
    /// Lap start on the session clock.
    start_s: f64,
    /// `start_s` is a real line crossing reported by the game (not the moment
    /// we happened to start looking).
    start_known: bool,
    /// The scoring lap count when the lap started.
    laps_completed_at_start: i32,
    fuel_start_l: f32,
    ve_start: Option<f32>,
    samples: u32,
    sum_temp: [f64; 4],
    sum_press: [f64; 4],
    max_brake: [f32; 4],
    top_speed: f32,
    started_in_pits: bool,
    was_in_pits: bool,
    /// When the car was first seen out of the pits on this lap.
    left_pits_s: Option<f64>,
    max_fraction: f32,
    /// Created from a scoring-only boundary: the telemetry lap counter may
    /// still catch up, which must not open a new lap.
    awaiting_counter: bool,
}

impl Accumulator {
    /// A lap whose start we did not witness: trust the game's lap clock when
    /// it has one.
    fn start(p: &PlayerTelemetry, now: f64) -> Self {
        let game_start = p.lap.lap_start_s;
        let known = game_start > 0.0 && game_start <= now + 0.5;
        Self::start_at(p, now, if known { game_start } else { now }, known)
    }

    /// A lap starting at `start_s` (session clock).
    fn start_at(p: &PlayerTelemetry, now: f64, start_s: f64, start_known: bool) -> Self {
        let in_pits = is_in_pits(p);
        let mut acc = Self {
            lap_number: p.lap.lap_number,
            start_s,
            start_known,
            laps_completed_at_start: p.lap.laps_completed,
            fuel_start_l: p.energy.fuel_l,
            ve_start: p.energy.virtual_energy,
            started_in_pits: in_pits,
            left_pits_s: (!in_pits).then_some(start_s),
            ..Default::default()
        };
        acc.feed(p, now);
        acc
    }

    fn feed(&mut self, p: &PlayerTelemetry, now: f64) {
        self.samples += 1;
        for i in 0..4 {
            let w = &p.wheels[i];
            let t = (w.temp_c[0] + w.temp_c[1] + w.temp_c[2]) / 3.0;
            self.sum_temp[i] += t as f64;
            self.sum_press[i] += w.pressure_kpa as f64;
            if w.brake_temp_c > self.max_brake[i] {
                self.max_brake[i] = w.brake_temp_c;
            }
        }
        if p.motion.speed_kmh > self.top_speed {
            self.top_speed = p.motion.speed_kmh;
        }
        if is_in_pits(p) {
            if self.left_pits_s.is_some() {
                self.was_in_pits = true;
            }
        } else if self.left_pits_s.is_none() {
            self.left_pits_s = Some(now);
        }
        if now - self.start_s >= FRACTION_WARMUP_S && p.lap.lap_fraction > self.max_fraction {
            self.max_fraction = p.lap.lap_fraction;
        }
    }

    /// The car has been seen near the end of the lap.
    fn reached_end(&self, track_length_m: f32) -> bool {
        // A source without lap distance cannot be checked.
        track_length_m <= 1.0 || self.max_fraction >= END_OF_LAP_FRACTION
    }

    /// Everything but the timing, which the caller decides.
    fn finish(&self, end: &PlayerTelemetry, frame: &TelemetryFrame, end_s: f64) -> LapRecord {
        let n = self.samples.max(1) as f64;
        let mut avg_t = [0f32; 4];
        let mut avg_p = [0f32; 4];
        let mut wear = [0f32; 4];
        for i in 0..4 {
            avg_t[i] = (self.sum_temp[i] / n) as f32;
            avg_p[i] = (self.sum_press[i] / n) as f32;
            wear[i] = end.wheels[i].wear_remaining;
        }
        let ended_in_pits = is_in_pits(end);
        LapRecord {
            lap_number: self.lap_number,
            in_lap: !self.started_in_pits && (self.was_in_pits || ended_in_pits),
            out_lap: self.started_in_pits,
            fuel_used_l: (self.fuel_start_l - end.energy.fuel_l).max(0.0),
            virtual_energy_used: match (self.ve_start, end.energy.virtual_energy) {
                (Some(a), Some(b)) => Some((a - b).max(0.0)),
                _ => None,
            },
            top_speed_kmh: self.top_speed,
            front_compound: end.tyres.front_compound.clone(),
            rear_compound: end.tyres.rear_compound.clone(),
            avg_tyre_temp_c: avg_t,
            avg_tyre_pressure_kpa: avg_p,
            max_brake_temp_c: self.max_brake,
            wear_remaining_end: wear,
            track_c: frame.session.weather.track_c,
            ambient_c: frame.session.weather.ambient_c,
            rain: frame.session.weather.rain,
            session_elapsed_end_s: end_s,
            ..Default::default()
        }
    }
}

fn is_in_pits(p: &PlayerTelemetry) -> bool {
    p.in_pits || p.lap.in_pit_lane || p.in_garage
}

/// A lap that crossed the line and waits for the game's verdict.
#[derive(Debug, Clone)]
struct Pending {
    record: LapRecord,
    /// Duration between the two line crossings, seconds.
    measured_s: f64,
    /// `measured_s` spans two real crossings (false when we joined mid-lap).
    measured_ok: bool,
    /// What to show as duration when the game gives no time: the measured
    /// duration, or the time on track for a lap that started in the pits.
    untimed_s: f64,
    laps_completed_before: i32,
    boundary_s: f64,
    start_lap: bool,
}

impl Pending {
    /// Freezes `cur`, ended at `end_s` on the frame `frame` (the first frame
    /// of the next lap). The caller sets `laps_completed_before` / `boundary_s`.
    fn new(
        cur: &Accumulator,
        p: &PlayerTelemetry,
        frame: &TelemetryFrame,
        end_s: f64,
        crossing_known: bool,
        is_race: bool,
    ) -> Self {
        let measured_s = end_s - cur.start_s;
        Self {
            record: cur.finish(p, frame, end_s),
            measured_s,
            measured_ok: cur.start_known && crossing_known,
            untimed_s: match cur.left_pits_s {
                Some(t) if cur.started_in_pits => end_s - t.max(cur.start_s),
                _ => measured_s,
            },
            laps_completed_before: p.lap.laps_completed,
            boundary_s: end_s,
            start_lap: is_race && cur.laps_completed_at_start == 0 && !cur.started_in_pits,
        }
    }
}

/// How a pending lap gets its verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Verdict {
    /// The scoring block counted the lap: its `last_lap_s` belongs to it.
    Counted,
    /// The scoring block never counted it.
    TimedOut,
}

/// Stateful lap detector. Feed every frame; collect the returned records.
#[derive(Debug, Default)]
pub struct LapTracker {
    cur: Option<Accumulator>,
    pending: Option<Pending>,
    last_track: String,
    last_session_index: i32,
    last_now: f64,
    prev_laps_completed: Option<i32>,
    prev_phase: Option<GamePhase>,
    /// The source publishes a lap count (seen above zero or changing).
    scoring_alive: bool,
}

impl LapTracker {
    pub fn new() -> Self {
        Self::default()
    }

    fn reset(&mut self) {
        self.cur = None;
        self.pending = None;
        self.prev_laps_completed = None;
        self.prev_phase = None;
        self.scoring_alive = false;
    }

    /// Feeds one frame. Returns the laps this frame completes (almost always
    /// none, sometimes one, exceptionally two).
    pub fn feed(&mut self, frame: &TelemetryFrame) -> Vec<LapRecord> {
        let mut out = Vec::new();
        let Some(p) = frame.player.as_ref() else {
            return out;
        };
        let now = frame.session.elapsed_s;

        // Another session, or the clock went back (restart): forget everything.
        if frame.session.track_name != self.last_track
            || frame.session.session_index != self.last_session_index
            || now < self.last_now - 1.0
        {
            self.last_track = frame.session.track_name.clone();
            self.last_session_index = frame.session.session_index;
            self.reset();
        }
        self.last_now = now;

        let lc = p.lap.laps_completed;
        let counted_now = self.prev_laps_completed.is_some_and(|prev| lc > prev);
        if lc > 0 || counted_now {
            self.scoring_alive = true;
        }
        let is_race = frame.session.session_type == SessionType::Race;
        let track_len = frame.session.track_length_m;

        // 1. A lap waiting for the game's verdict.
        if let Some(pd) = self.pending.take() {
            if lc > pd.laps_completed_before {
                out.push(self.resolve(pd, p, Verdict::Counted));
            } else if now - pd.boundary_s > CONFIRM_TIMEOUT_S {
                out.push(self.resolve(pd, p, Verdict::TimedOut));
            } else {
                self.pending = Some(pd);
            }
        }
        let resolved_now = !out.is_empty();

        // 2. Lap boundary?
        // Start of a race: the lap clock starts at the green flag, wherever
        // the car is (grid, rolling start), without a new lap number.
        let green_now = is_race
            && lc == 0
            && frame.session.phase == GamePhase::Green
            && self.prev_phase.is_some_and(|ph| {
                matches!(
                    ph,
                    GamePhase::BeforeSession
                        | GamePhase::Reconnaissance
                        | GamePhase::GridWalk
                        | GamePhase::Formation
                        | GamePhase::Countdown
                )
            });
        match self.cur.take() {
            None => self.cur = Some(Accumulator::start(p, now)),
            Some(cur) if p.lap.lap_number < cur.lap_number => {
                // The counter went back (restart, driver swap): start over.
                self.cur = Some(Accumulator::start(p, now));
            }
            Some(mut cur) if p.lap.lap_number > cur.lap_number => {
                let game_start = p.lap.lap_start_s;
                let crossing_known = game_start > cur.start_s && game_start <= now + 0.5;
                let end_s = if crossing_known { game_start } else { now };
                if cur.awaiting_counter && now - cur.start_s < MIN_LAP_S {
                    // The telemetry counter catches up with a lap already
                    // closed from the scoring block: same lap, new number.
                    cur.lap_number = p.lap.lap_number;
                    cur.awaiting_counter = false;
                    if game_start > cur.start_s - 1.0 && game_start <= now + 0.5 {
                        cur.start_s = game_start;
                        cur.start_known = true;
                    }
                    cur.feed(p, now);
                    self.cur = Some(cur);
                } else if cur.reached_end(track_len) && end_s - cur.start_s >= MIN_LAP_S {
                    let pd = Pending::new(&cur, p, frame, end_s, crossing_known, is_race);
                    let pd = Pending {
                        laps_completed_before: if counted_now { lc - 1 } else { lc },
                        boundary_s: now,
                        ..pd
                    };
                    if let Some(old) = self.pending.take() {
                        // Cannot happen with real laps; never lose one anyway.
                        out.push(self.resolve(old, p, Verdict::TimedOut));
                    }
                    if counted_now && !resolved_now {
                        // Telemetry and scoring moved on the same frame.
                        out.push(self.resolve(pd, p, Verdict::Counted));
                    } else {
                        self.pending = Some(pd);
                    }
                    self.cur = Some(Accumulator::start_at(p, now, end_s, crossing_known));
                } else {
                    // The counter moved far from the line: the lap was
                    // abandoned (Esc to the garage). Report it only when the
                    // car really drove.
                    if cur.left_pits_s.is_some() && cur.top_speed >= ABORTED_MIN_SPEED_KMH {
                        let mut rec = cur.finish(p, frame, now);
                        rec.lap_time_s = (now
                            - cur.left_pits_s.unwrap_or(cur.start_s).max(cur.start_s))
                        .max(0.0) as f32;
                        rec.timed_by_game = false;
                        rec.valid = false;
                        rec.in_lap = false;
                        rec.out_lap = cur.started_in_pits;
                        rec.note = Some(LapNote::Aborted);
                        out.push(rec);
                    }
                    self.cur = Some(Accumulator::start_at(p, now, now, false));
                }
            }
            Some(mut cur) => {
                // Same lap number.
                let game_start = p.lap.lap_start_s;
                if counted_now
                    && !resolved_now
                    && self.pending.is_none()
                    && cur.reached_end(track_len)
                    && now - cur.start_s >= MIN_LAP_S
                {
                    // The scoring block counted a lap the telemetry counter
                    // has not (yet) shown — typically the chequered flag.
                    let pd = Pending::new(&cur, p, frame, now, false, is_race);
                    let pd = Pending {
                        // Up to one scoring period late: still within tolerance.
                        measured_ok: cur.start_known,
                        laps_completed_before: lc - 1,
                        boundary_s: now,
                        ..pd
                    };
                    out.push(self.resolve(pd, p, Verdict::Counted));
                    let mut acc = Accumulator::start_at(p, now, now, false);
                    acc.awaiting_counter = true;
                    self.cur = Some(acc);
                } else if green_now {
                    // The first lap of the race starts here. Use the game's
                    // lap clock when it has just been restarted, else "now"
                    // (the official time will be trusted as is).
                    let known = (game_start - now).abs() <= 1.0;
                    self.cur = Some(Accumulator::start_at(
                        p,
                        now,
                        if known { game_start } else { now },
                        known,
                    ));
                } else {
                    cur.feed(p, now);
                    self.cur = Some(cur);
                }
            }
        }

        self.prev_phase = Some(frame.session.phase);
        self.prev_laps_completed = Some(lc);
        out
    }

    fn resolve(&self, pd: Pending, p: &PlayerTelemetry, verdict: Verdict) -> LapRecord {
        let mut rec = pd.record;
        let official = p.lap.last_lap_s.map(f64::from);
        let matches = |t: f64| !pd.measured_ok || (t - pd.measured_s).abs() <= TIME_TOLERANCE_S;
        let timed = match verdict {
            Verdict::Counted => official.is_some_and(matches),
            // No lap count from this source: fall back on our own stopwatch.
            Verdict::TimedOut => !self.scoring_alive,
        };
        if timed {
            match official.filter(|&t| verdict == Verdict::Counted || matches(t)) {
                Some(t) => {
                    rec.lap_time_s = t as f32;
                    rec.s1 = p.lap.last_s1;
                    rec.s2 = p.lap.last_s2;
                    rec.s3 = match (rec.s1, rec.s2) {
                        (Some(a), Some(b)) => Some((t as f32 - a - b).max(0.0)),
                        _ => p.lap.last_s3,
                    };
                }
                None => rec.lap_time_s = pd.measured_s.max(0.0) as f32,
            }
        } else {
            rec.lap_time_s = pd.untimed_s.max(0.0) as f32;
            // The game still reports the sectors it could time on that lap.
            if verdict == Verdict::Counted && official.is_none() {
                rec.s1 = p.lap.last_s1;
                rec.s2 = p.lap.last_s2;
            }
        }
        rec.timed_by_game = timed;
        rec.valid = timed && !rec.in_lap && !rec.out_lap && rec.lap_time_s > 0.0;
        rec.note = if rec.out_lap {
            Some(LapNote::OutLap)
        } else if rec.in_lap {
            Some(LapNote::InLap)
        } else if !timed {
            Some(LapNote::NotTimed)
        } else if pd.start_lap {
            Some(LapNote::StartLap)
        } else {
            None
        };
        rec
    }

    /// Lap number being accumulated, if any.
    pub fn current_lap(&self) -> Option<i32> {
        self.cur.as_ref().map(|a| a.lap_number)
    }

    /// A lap has crossed the line and waits for the game's official time.
    pub fn has_pending(&self) -> bool {
        self.pending.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use corde_protocol::{PlayerTelemetry, TelemetryFrame};

    const TRACK: f32 = 4000.0;

    /// A tiny stand-in for the game: telemetry at 100 Hz, scoring every 0.2 s.
    struct Game {
        t: f64,
        /// Telemetry side.
        lap_number: i32,
        lap_start: f64,
        dist: f32,
        speed: f32,
        in_pits: bool,
        in_garage: bool,
        /// Scoring side, as last published.
        laps_completed: i32,
        last_lap: Option<f32>,
        last_s1: Option<f32>,
        last_s2: Option<f32>,
        /// Scoring side, waiting for the next publication.
        next: Option<(i32, Option<f32>)>,
        publish_at: f64,
        session_type: SessionType,
        phase: GamePhase,
        has_scoring: bool,
    }

    impl Game {
        fn new() -> Self {
            Self {
                t: 100.0,
                lap_number: 0,
                lap_start: 100.0,
                dist: 0.0,
                speed: 50.0,
                in_pits: false,
                in_garage: false,
                laps_completed: 0,
                last_lap: None,
                last_s1: None,
                last_s2: None,
                next: None,
                publish_at: 0.0,
                session_type: SessionType::Practice,
                phase: GamePhase::Green,
                has_scoring: true,
            }
        }

        fn frame(&self) -> TelemetryFrame {
            let mut p = PlayerTelemetry::default();
            p.lap.lap_number = self.lap_number;
            p.lap.lap_start_s = self.lap_start;
            p.lap.laps_completed = self.laps_completed;
            p.lap.last_lap_s = self.last_lap;
            p.lap.last_s1 = self.last_s1;
            p.lap.last_s2 = self.last_s2;
            p.lap.lap_dist_m = self.dist;
            p.lap.lap_fraction = (self.dist / TRACK).clamp(0.0, 1.0);
            p.in_pits = self.in_pits;
            p.in_garage = self.in_garage;
            p.motion.speed_kmh = self.speed * 3.6;
            p.energy.fuel_l = 60.0 - (self.t as f32 - 100.0) * 0.03;
            for w in p.wheels.iter_mut() {
                w.temp_c = [80.0, 85.0, 90.0];
                w.pressure_kpa = 150.0;
                w.brake_temp_c = 400.0;
                w.wear_remaining = 0.9;
            }
            let mut f = TelemetryFrame::default();
            f.session.track_name = "Test".into();
            f.session.track_length_m = TRACK;
            f.session.session_type = self.session_type;
            f.session.phase = self.phase;
            f.session.elapsed_s = self.t;
            f.player = Some(p);
            f
        }

        /// Advances 10 ms; returns the laps the tracker reports.
        fn step(&mut self, tr: &mut LapTracker) -> Vec<LapRecord> {
            self.t += 0.01;
            if !self.in_garage {
                self.dist += self.speed * 0.01;
            }
            if let Some((laps, last)) = self.next {
                if self.t >= self.publish_at {
                    self.laps_completed = laps;
                    self.last_lap = last;
                    self.next = None;
                }
            }
            tr.feed(&self.frame())
        }

        /// Crosses the line now: the telemetry counter moves immediately, the
        /// scoring block `lag` seconds later with `official` as last lap.
        fn cross(&mut self, official: Option<f32>, lag: f64) {
            self.lap_number += 1;
            self.lap_start = self.t;
            self.dist = 0.0;
            if self.has_scoring {
                self.next = Some((self.laps_completed + 1, official));
                self.publish_at = self.t + lag;
            }
        }

        /// Drives until the end of the lap at 50 m/s (80 s for 4000 m).
        fn drive_lap(&mut self, tr: &mut LapTracker) -> Vec<LapRecord> {
            let mut out = Vec::new();
            while self.dist < TRACK {
                out.extend(self.step(tr));
            }
            out
        }

        fn run(&mut self, tr: &mut LapTracker, seconds: f64) -> Vec<LapRecord> {
            let mut out = Vec::new();
            let end = self.t + seconds;
            while self.t < end {
                out.extend(self.step(tr));
            }
            out
        }
    }

    #[test]
    fn official_time_is_taken_when_the_scoring_catches_up() {
        let mut g = Game::new();
        let mut tr = LapTracker::new();
        // Lap 0, then lap 1 a little faster. The scoring block lags 0.18 s.
        assert!(g.drive_lap(&mut tr).is_empty());
        g.cross(Some(80.01), 0.18);
        let laps = g.run(&mut tr, 1.0);
        assert_eq!(laps.len(), 1, "{laps:?}");
        assert_eq!(laps[0].lap_number, 0);
        assert!((laps[0].lap_time_s - 80.01).abs() < 1e-3);
        assert!(laps[0].valid && laps[0].timed_by_game);
        assert_eq!(laps[0].note, None);

        // Second lap: 2 s quicker (speed up). The stale `last_lap` (80.01)
        // must not be used even though it sits there when the counter moves.
        g.speed = 51.3;
        let laps = g.drive_lap(&mut tr);
        assert!(laps.is_empty());
        let real = (g.t - g.lap_start) as f32;
        g.cross(Some(real), 0.18);
        let laps = g.run(&mut tr, 1.0);
        assert_eq!(laps.len(), 1);
        assert_eq!(laps[0].lap_number, 1);
        assert!(
            (laps[0].lap_time_s - real).abs() < 1e-3,
            "{} vs {real}",
            laps[0].lap_time_s
        );
        assert!(laps[0].lap_time_s < 79.0);
        assert!(laps[0].valid);
    }

    #[test]
    fn a_lap_the_game_did_not_time_is_not_valid() {
        let mut g = Game::new();
        let mut tr = LapTracker::new();
        g.drive_lap(&mut tr);
        g.cross(Some(80.0), 0.1);
        assert_eq!(g.run(&mut tr, 1.0).len(), 1);
        // Track limits on the next lap: counted, not timed.
        g.drive_lap(&mut tr);
        g.cross(None, 0.15);
        let laps = g.run(&mut tr, 1.0);
        assert_eq!(laps.len(), 1);
        let l = &laps[0];
        assert!(!l.valid && !l.timed_by_game);
        assert_eq!(l.note, Some(LapNote::NotTimed));
        assert!(
            (l.lap_time_s - 79.0).abs() < 1.5,
            "measured duration, got {}",
            l.lap_time_s
        );
    }

    #[test]
    fn an_official_time_that_does_not_match_is_rejected() {
        let mut g = Game::new();
        let mut tr = LapTracker::new();
        g.drive_lap(&mut tr);
        g.cross(Some(80.0), 0.1);
        g.run(&mut tr, 1.0);
        g.drive_lap(&mut tr);
        // The game keeps the old time instead of publishing a new one.
        g.speed = 40.0; // irrelevant now
        g.cross(Some(60.0), 0.1);
        let laps = g.run(&mut tr, 1.0);
        assert_eq!(laps.len(), 1);
        assert!(!laps[0].timed_by_game);
        assert!(laps[0].lap_time_s > 70.0);
    }

    #[test]
    fn escape_to_the_garage_is_an_aborted_lap_not_a_record() {
        let mut g = Game::new();
        let mut tr = LapTracker::new();
        g.drive_lap(&mut tr);
        g.cross(Some(80.0), 0.1);
        g.run(&mut tr, 1.0);
        // Ten seconds into the next lap: Esc. The game bumps both counters,
        // gives no time, and teleports the car to the garage.
        g.run(&mut tr, 9.0);
        g.lap_number += 1;
        g.next = Some((g.laps_completed + 1, None));
        g.publish_at = g.t;
        g.in_garage = true;
        g.in_pits = true;
        g.speed = 0.0;
        g.dist = 150.0;
        let laps = g.run(&mut tr, 5.0);
        assert_eq!(laps.len(), 1, "{laps:?}");
        let l = &laps[0];
        assert_eq!(l.note, Some(LapNote::Aborted));
        assert!(!l.valid && !l.timed_by_game);
        assert!(l.lap_time_s < 12.0);
        assert!(!tr.has_pending());
    }

    #[test]
    fn sitting_in_the_garage_reports_nothing() {
        let mut g = Game::new();
        g.in_garage = true;
        g.in_pits = true;
        g.speed = 0.0;
        let mut tr = LapTracker::new();
        g.run(&mut tr, 3.0);
        g.lap_number += 1;
        g.laps_completed += 1;
        assert!(g.run(&mut tr, 3.0).is_empty());
    }

    #[test]
    fn chequered_flag_without_a_telemetry_counter_change() {
        let mut g = Game::new();
        g.session_type = SessionType::Race;
        let mut tr = LapTracker::new();
        g.drive_lap(&mut tr);
        g.cross(Some(80.0), 0.1);
        g.run(&mut tr, 1.0);
        // Last lap: only the scoring block counts it.
        g.drive_lap(&mut tr);
        let real = (g.t - g.lap_start) as f32;
        g.dist = 0.0;
        g.next = Some((g.laps_completed + 1, Some(real)));
        g.publish_at = g.t + 0.1;
        let laps = g.run(&mut tr, 1.0);
        assert_eq!(laps.len(), 1, "{laps:?}");
        assert_eq!(laps[0].lap_number, 1);
        assert!((laps[0].lap_time_s - real).abs() < 1e-3);
        assert!(laps[0].valid);
        // The telemetry counter catching up later does not create a lap.
        g.lap_number += 1;
        g.lap_start = g.t - 1.0;
        assert!(g.run(&mut tr, 5.0).is_empty());
    }

    #[test]
    fn telemetry_and_scoring_on_the_same_frame() {
        let mut g = Game::new();
        let mut tr = LapTracker::new();
        g.drive_lap(&mut tr);
        g.cross(Some(80.0), 0.0);
        let laps = g.run(&mut tr, 0.05);
        assert_eq!(laps.len(), 1);
        assert!((laps[0].lap_time_s - 80.0).abs() < 1e-3);
        assert!(!tr.has_pending());
    }

    #[test]
    fn race_start_lap_is_measured_from_the_green_flag() {
        let mut g = Game::new();
        g.session_type = SessionType::Race;
        g.phase = GamePhase::Formation;
        g.lap_start = 0.0; // nothing meaningful before the start
        g.speed = 15.0; // rolling start, far behind the line
        g.dist = 3400.0;
        let mut tr = LapTracker::new();
        g.run(&mut tr, 30.0);
        assert!(
            g.dist > 0.9 * TRACK,
            "the car is near the end of the 'previous' lap"
        );
        // Green flag: the game starts the lap clock, same lap number.
        g.phase = GamePhase::Green;
        g.lap_start = g.t;
        g.speed = 50.0;
        g.dist = -100.0;
        g.drive_lap(&mut tr);
        let real = (g.t - g.lap_start) as f32;
        g.cross(Some(real), 0.15);
        let laps = g.run(&mut tr, 1.0);
        assert_eq!(laps.len(), 1);
        let l = &laps[0];
        assert!((l.lap_time_s - real).abs() < 1e-3);
        assert!(l.timed_by_game && l.valid);
        assert_eq!(l.note, Some(LapNote::StartLap));
        assert!(!l.out_lap);

        // Same start, but the game never restarts its lap clock: the official
        // time is trusted as is, and a caution on lap 1 changes nothing.
        let mut g = Game::new();
        g.session_type = SessionType::Race;
        g.phase = GamePhase::Countdown;
        g.lap_start = 0.0;
        g.speed = 0.0;
        g.dist = 3900.0;
        let mut tr = LapTracker::new();
        g.run(&mut tr, 30.0);
        g.phase = GamePhase::Green;
        g.speed = 50.0;
        g.dist = -100.0;
        let green_at = g.t;
        g.run(&mut tr, 20.0);
        g.phase = GamePhase::FullCourseYellow;
        g.run(&mut tr, 5.0);
        g.phase = GamePhase::Green;
        g.drive_lap(&mut tr);
        let real = (g.t - green_at) as f32;
        g.cross(Some(real), 0.15);
        let laps = g.run(&mut tr, 1.0);
        assert_eq!(laps.len(), 1);
        assert!((laps[0].lap_time_s - real).abs() < 1e-3);
        assert!(laps[0].valid);
        assert_eq!(laps[0].note, Some(LapNote::StartLap));
    }

    #[test]
    fn lap_clock_published_one_frame_before_the_counter_loses_nothing() {
        let mut g = Game::new();
        let mut tr = LapTracker::new();
        g.drive_lap(&mut tr);
        g.cross(Some(80.0), 0.1);
        assert_eq!(g.run(&mut tr, 1.0).len(), 1);
        // End of the next lap: the lap clock moves first, the counter a frame later.
        g.drive_lap(&mut tr);
        let real = (g.t - g.lap_start) as f32;
        g.lap_start = g.t;
        g.dist = 0.0;
        assert!(g.step(&mut tr).is_empty());
        g.lap_number += 1;
        g.next = Some((g.laps_completed + 1, Some(real)));
        g.publish_at = g.t + 0.1;
        let laps = g.run(&mut tr, 1.0);
        assert_eq!(laps.len(), 1, "{laps:?}");
        assert!((laps[0].lap_time_s - real).abs() < 1e-3);
        assert!(laps[0].valid);
    }

    #[test]
    fn out_lap_and_in_lap() {
        let mut g = Game::new();
        g.in_pits = true;
        let mut tr = LapTracker::new();
        g.run(&mut tr, 5.0);
        g.in_pits = false;
        g.drive_lap(&mut tr);
        // Out-lap from the garage: the game counts it without a time.
        g.cross(None, 0.1);
        let laps = g.run(&mut tr, 1.0);
        assert_eq!(laps.len(), 1);
        assert_eq!(laps[0].note, Some(LapNote::OutLap));
        assert!(laps[0].out_lap && !laps[0].valid);

        // A flying lap, then an in-lap ending in the pit lane (timed by the game).
        g.drive_lap(&mut tr);
        g.cross(Some(80.0), 0.1);
        assert!(g.run(&mut tr, 1.0)[0].valid);
        while g.dist < TRACK - 300.0 {
            g.step(&mut tr);
        }
        g.in_pits = true;
        g.drive_lap(&mut tr);
        let real = (g.t - g.lap_start) as f32;
        g.cross(Some(real), 0.1);
        let laps = g.run(&mut tr, 1.0);
        assert_eq!(laps.len(), 1);
        assert!(laps[0].in_lap && !laps[0].valid && laps[0].timed_by_game);
        assert_eq!(laps[0].note, Some(LapNote::InLap));
        // And the lap that starts in the pit lane is an out-lap.
        g.run(&mut tr, 20.0);
        g.in_pits = false;
        g.drive_lap(&mut tr);
        let real = (g.t - g.lap_start) as f32;
        g.cross(Some(real), 0.1);
        let laps = g.run(&mut tr, 1.0);
        assert!(laps[0].out_lap && laps[0].timed_by_game && !laps[0].valid);
    }

    #[test]
    fn a_source_without_lap_count_uses_the_stopwatch() {
        let mut g = Game::new();
        g.has_scoring = false;
        let mut tr = LapTracker::new();
        g.drive_lap(&mut tr);
        g.cross(None, 0.0);
        let laps = g.run(&mut tr, 4.0);
        assert_eq!(laps.len(), 1);
        assert!(laps[0].valid && laps[0].timed_by_game);
        assert!((laps[0].lap_time_s - 80.0).abs() < 0.1);
    }

    #[test]
    fn session_change_forgets_the_lap_in_progress() {
        let mut g = Game::new();
        let mut tr = LapTracker::new();
        g.run(&mut tr, 30.0);
        let mut f = g.frame();
        f.session.track_name = "Other".into();
        assert!(tr.feed(&f).is_empty());
        assert_eq!(tr.current_lap(), Some(0));
        // Restart of the same session (clock goes back).
        g.t = 5.0;
        g.lap_start = 0.0;
        assert!(g.step(&mut tr).is_empty());
    }

    #[test]
    fn lap_statistics_are_accumulated() {
        let mut g = Game::new();
        let mut tr = LapTracker::new();
        g.drive_lap(&mut tr);
        g.cross(Some(80.0), 0.1);
        let l = g.run(&mut tr, 1.0).remove(0);
        assert!((l.avg_tyre_temp_c[0] - 85.0).abs() < 1e-3);
        assert!((l.avg_tyre_pressure_kpa[3] - 150.0).abs() < 1e-3);
        assert!((l.fuel_used_l - 2.4).abs() < 0.05, "{}", l.fuel_used_l);
        assert!((l.top_speed_kmh - 180.0).abs() < 0.5);
        assert!(
            (l.session_elapsed_end_s - 180.0).abs() < 0.05,
            "{}",
            l.session_elapsed_end_s
        );
    }
}
