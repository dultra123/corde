//! # corde-sim
//!
//! A deterministic synthetic world that behaves like a plausible Le Mans
//! Ultimate session, used to develop and demo dashboards without the game:
//!
//! * [`track`] — closed 2D circuit laid out like Paul Ricard 1A with a
//!   physically plausible speed profile (80–300 km/h for a GT3).
//! * [`world`] — [`SimWorld`]: one player car (Mercedes-AMG GT3 by default, or
//!   a hybrid Hypercar) plus a mixed field of AI cars; every tick produces a
//!   complete [`corde_protocol::TelemetryFrame`] (timing, sectors, gearbox,
//!   tyres, brakes, fuel / virtual energy, weather, sorted standings…).
//! * [`source`] — [`SimSource`]: the [`corde_core::TelemetrySource`]
//!   implementation (id `"sim"`) that paces the world on a monotonic clock.
//!
//! Same [`SimConfig::seed`] → byte-identical frames (modulo `wall_ms`, which
//! can be pinned with [`SimConfig::wall_base_ms`]).

pub mod rng;
pub mod source;
pub mod track;
pub mod world;

pub use source::SimSource;
pub use track::{Envelope, Track};
pub use world::{PlayerClass, SimWorld};

use corde_protocol::GameId;

/// Paul Ricard, layout 1A (with the Mistral chicane), metres.
pub const DEFAULT_TRACK_LENGTH_M: f32 = 5698.6;
/// Typical GT3 lap time on that layout, seconds.
pub const DEFAULT_LAP_TIME_S: f32 = 115.0;
/// Default number of AI cars (a 20-car field with the player).
pub const DEFAULT_NUM_AI: usize = 19;
/// Native rate of the simulator (LMU writes its shared memory at ~100 Hz).
pub const DEFAULT_RATE_HZ: f32 = 100.0;

/// Parameters of the synthetic world. All fields are public; start from
/// [`SimConfig::default`] and override what you need.
#[derive(Debug, Clone, PartialEq)]
pub struct SimConfig {
    /// Circuit length, metres (the Paul Ricard-like shape is scaled to it).
    pub track_length_m: f32,
    /// Nominal lap time of the player's car on a dry track, seconds.
    pub lap_time_s: f32,
    /// Number of AI cars (0..=60), classes mixed ≈ 6 Hypercar / 5 LMP2 / 8 GT3 per 19.
    pub num_ai: usize,
    /// Tick rate of the world, Hz (also the source's `native_rate_hz`).
    pub rate_hz: f32,
    /// PRNG seed — same seed, same frames.
    pub seed: u64,
    /// Rain intensity 0..1 (slows the field, wets the track, dims the sky).
    pub rain: f32,
    /// Class of the player's car.
    pub player_class: PlayerClass,
    /// Driver name shown in the standings.
    pub player_name: String,
    /// Reported in `SessionInfo::game` (`Sim` by default).
    pub game: GameId,
    /// Laps already completed by the player when the world starts (gives the
    /// dashboards a last/best lap right away). `0` = fresh out-lap done.
    pub start_laps_completed: i32,
    /// Let the AI cars make pit stops during the run.
    pub ai_pit_stops: bool,
    /// Pin the wall clock of the first frame (ms since Unix epoch) — for
    /// byte-identical frames in tests. `None` = now.
    pub wall_base_ms: Option<u64>,
    /// Real-time pacing of [`SimSource`]: simulated seconds per real second
    /// (1.0 = real time, 10.0 = laps go by ten times faster). Does not change
    /// the frames themselves, only how fast they are produced.
    pub time_scale: f64,
}

impl Default for SimConfig {
    fn default() -> Self {
        Self {
            track_length_m: DEFAULT_TRACK_LENGTH_M,
            lap_time_s: DEFAULT_LAP_TIME_S,
            num_ai: DEFAULT_NUM_AI,
            rate_hz: DEFAULT_RATE_HZ,
            seed: 0x4D55_5245_5400_0001, // "CORDE"
            rain: 0.0,
            player_class: PlayerClass::Gt3,
            player_name: "Pilote".into(),
            game: GameId::Sim,
            start_laps_completed: 2,
            ai_pit_stops: true,
            wall_base_ms: None,
            time_scale: 1.0,
        }
    }
}

impl SimConfig {
    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self
    }

    pub fn with_rate_hz(mut self, rate_hz: f32) -> Self {
        self.rate_hz = rate_hz;
        self
    }

    pub fn with_class(mut self, class: PlayerClass) -> Self {
        self.player_class = class;
        self
    }

    pub fn with_rain(mut self, rain: f32) -> Self {
        self.rain = rain;
        self
    }

    pub fn with_num_ai(mut self, num_ai: usize) -> Self {
        self.num_ai = num_ai;
        self
    }

    pub fn with_time_scale(mut self, time_scale: f64) -> Self {
        self.time_scale = time_scale;
        self
    }

    /// Clamp every field into its supported range.
    pub fn sanitized(mut self) -> Self {
        self.track_length_m = self.track_length_m.clamp(500.0, 30_000.0);
        self.lap_time_s = self.lap_time_s.clamp(30.0, 900.0);
        self.num_ai = self.num_ai.min(60);
        self.rate_hz = if self.rate_hz.is_finite() {
            self.rate_hz.clamp(1.0, 1000.0)
        } else {
            DEFAULT_RATE_HZ
        };
        self.rain = if self.rain.is_finite() {
            self.rain.clamp(0.0, 1.0)
        } else {
            0.0
        };
        self.start_laps_completed = self.start_laps_completed.max(0);
        self.time_scale = if self.time_scale.is_finite() {
            self.time_scale.clamp(0.01, 100.0)
        } else {
            1.0
        };
        self
    }
}
