//! The synthetic world: one player car plus a field of AI cars lapping the
//! [`Track`], producing complete [`TelemetryFrame`]s at a fixed tick rate.
//!
//! Deterministic: every random draw goes through the seeded [`Rng`] in a fixed
//! order, and the wall clock is derived from the simulated clock.

use crate::rng::Rng;
use crate::track::{Envelope, Track};
use crate::SimConfig;
use corde_protocol::*;
use std::f64::consts::TAU;
use std::time::{SystemTime, UNIX_EPOCH};

const G: f64 = 9.81;
const PIT_SPEED_MS: f64 = 80.0 / 3.6;
const PIT_STOP_S: f64 = 28.0;
/// Lap-time factors of the AI classes relative to GT3.
const HYPERCAR_FACTOR: f64 = 0.835;
const LMP2_FACTOR: f64 = 0.87;

/// Player car class (drives the physics envelope and the energy model).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlayerClass {
    #[default]
    Gt3,
    Hypercar,
}

impl PlayerClass {
    pub fn as_str(self) -> &'static str {
        match self {
            PlayerClass::Gt3 => "gt3",
            PlayerClass::Hypercar => "hypercar",
        }
    }
}

impl std::fmt::Display for PlayerClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for PlayerClass {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "gt3" | "lmgt3" => Ok(PlayerClass::Gt3),
            "hypercar" | "hyper" | "lmh" | "lmdh" => Ok(PlayerClass::Hypercar),
            other => Err(format!("unknown class `{other}` (expected gt3 | hypercar)")),
        }
    }
}

#[derive(Debug, Clone)]
struct ClassSpec {
    vehicle_class: VehicleClass,
    class_name: &'static str,
    model: &'static str,
    vehicle_name: &'static str,
    envelope: Envelope,
    gears: u8,
    /// Speed (m/s) reached at `max_rpm` in each gear.
    gear_top_ms: Vec<f64>,
    max_rpm: f64,
    fuel_capacity_l: f64,
    fuel_start_l: f64,
    fuel_per_lap_l: f64,
    /// Virtual energy used per lap (fraction of 1.0), hybrid only.
    ve_per_lap: Option<f64>,
    wheel_range_deg: f64,
    turbo: bool,
}

impl ClassSpec {
    fn for_class(c: PlayerClass) -> Self {
        match c {
            PlayerClass::Gt3 => Self {
                vehicle_class: VehicleClass::Gt3,
                class_name: "GT3",
                model: "Mercedes-AMG GT3",
                vehicle_name: "Corde Racing #88",
                envelope: Envelope::gt3(),
                gears: 6,
                gear_top_ms: [88.0, 130.0, 172.0, 212.0, 258.0, 306.0]
                    .iter()
                    .map(|k| k / 3.6)
                    .collect(),
                max_rpm: 8500.0,
                fuel_capacity_l: 120.0,
                fuel_start_l: 60.0,
                fuel_per_lap_l: 2.6,
                ve_per_lap: None,
                wheel_range_deg: 540.0,
                turbo: false,
            },
            PlayerClass::Hypercar => Self {
                vehicle_class: VehicleClass::Hypercar,
                class_name: "Hyper",
                model: "Porsche 963",
                vehicle_name: "Corde Racing #88",
                envelope: Envelope::hypercar(),
                gears: 7,
                gear_top_ms: [92.0, 134.0, 176.0, 216.0, 256.0, 296.0, 340.0]
                    .iter()
                    .map(|k| k / 3.6)
                    .collect(),
                max_rpm: 8500.0,
                fuel_capacity_l: 90.0,
                fuel_start_l: 60.0,
                fuel_per_lap_l: 2.6,
                ve_per_lap: Some(0.034),
                wheel_range_deg: 480.0,
                turbo: true,
            },
        }
    }
}

#[derive(Debug, Clone)]
struct Player {
    s: f64,
    lap_number: i32,
    laps_completed: i32,
    lap_start: f64,
    /// Current-lap pace multiplier (lap time = nominal × lap_scale).
    lap_scale: f64,
    wiggle_phase: f64,
    cur_s1: Option<f64>,
    cur_s2: Option<f64>,
    last_lap: Option<f64>,
    best_lap: Option<f64>,
    last_sectors: [Option<f64>; 3],
    best_sectors: [Option<f64>; 3],
    best_lap_sectors: [Option<f64>; 2],
    gear: i8,
    v: f64,
    a_long: f64,
    curvature: f64,
    throttle: f64,
    brake: f64,
    steering: f64,
    clutch: f64,
    shift_timer: f64,
    tyre_temp: [[f64; 3]; 4],
    tyre_wear: [f64; 4],
    brake_temp: [f64; 4],
    fuel_l: f64,
    ve: f64,
    soc: f64,
    water_c: f64,
    oil_c: f64,
    motor_temp_c: f64,
    tc_active: bool,
    abs_active: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum PitPhase {
    None,
    InLane,
    Stopped(f64),
    Exiting,
}

#[derive(Debug, Clone)]
struct Ai {
    id: i32,
    driver: String,
    vehicle_name: String,
    class: VehicleClass,
    class_name: &'static str,
    class_rank: u8,
    /// Actual nominal lap time of this car, seconds.
    pace_s: f64,
    speed_factor: f64,
    v_cap: f64,
    /// Total race distance, metres.
    d: f64,
    lap_start: f64,
    lap_scale: f64,
    cur_s1: Option<f64>,
    cur_s2: Option<f64>,
    last_s1: Option<f64>,
    last_s2: Option<f64>,
    best_s1: Option<f64>,
    best_s2: Option<f64>,
    last_lap: Option<f64>,
    best_lap: Option<f64>,
    fuel_fraction: f64,
    fuel_per_lap_fraction: f64,
    pit_lap: i32,
    pit_phase: PitPhase,
    num_pitstops: u16,
    v: f64,
    headlights: bool,
    lateral_phase: f64,
    grid: i32,
}

impl Ai {
    fn laps_completed(&self, track: &Track) -> i32 {
        (self.d / track.length_m).floor() as i32
    }
    fn s(&self, track: &Track) -> f64 {
        track.wrap(self.d)
    }
    fn in_pits(&self) -> bool {
        self.pit_phase != PitPhase::None
    }
}

const DRIVERS: [&str; 24] = [
    "Lucas Ferrand",
    "Mika Ollila",
    "Tom Barrow",
    "Diego Salazar",
    "Kenji Arai",
    "Noah Weber",
    "Elias Lindqvist",
    "Marco Bellini",
    "Jordan Price",
    "Yann Lecomte",
    "Felix Brandt",
    "Rafael Souza",
    "Oscar Nyström",
    "Hugo Carvalho",
    "Liam Doyle",
    "Sacha Morel",
    "Matteo Rinaldi",
    "Tomás Herrera",
    "Arthur Duval",
    "Julien Vasseur",
    "Nils Petersen",
    "Pablo Ibáñez",
    "Ewan Mackay",
    "Teo Lindgren",
];

const HYPERCARS: [(&str, &str); 6] = [
    ("Porsche 963", "Meridian Racing #5"),
    ("Toyota GR010 Hybrid", "Nordlicht Motorsport #7"),
    ("Ferrari 499P", "Riviera Autosport #51"),
    ("Cadillac V-Series.R", "Atlas Endurance #2"),
    ("Peugeot 9X8", "Kestrel Racing #93"),
    ("BMW M Hybrid V8", "Solaris Team #15"),
];

const LMP2_TEAMS: [&str; 6] = [
    "Vega Competition #22",
    "Boreal Racing #28",
    "Corsa Blu #37",
    "Helix Motorsport #41",
    "Ardent Racing #65",
    "Tramontane Sport #10",
];

const GT3S: [(&str, &str); 9] = [
    ("Porsche 911 GT3 R LMGT3", "Falcon GT #91"),
    ("Ferrari 296 LMGT3 Evo", "Scuderia Alba #54"),
    ("BMW M4 LMGT3", "Harbour Racing #46"),
    ("Chevrolet Corvette Z06 LMGT3.R", "Ironwood Motorsport #81"),
    ("Aston Martin Vantage AMR LMGT3 Evo", "Greenline Racing #27"),
    ("Lexus RCF LMGT3", "Akira Motorsport #78"),
    ("McLaren 720S LMGT3 Evo", "Orbit Racing #59"),
    ("Ford Mustang LMGT3", "Westgate GT #88"),
    ("Lamborghini Huracan LMGT3 Evo2", "Rosso Vento #60"),
];

/// Interpolated time at which `boundary` was crossed between `s_old` (at
/// `t_old`) and `s_new` (at `t_old + dt`), if it was.
fn crossing(s_old: f64, s_new: f64, boundary: f64, t_old: f64, dt: f64) -> Option<f64> {
    if s_old < boundary && s_new >= boundary {
        let f = ((boundary - s_old) / (s_new - s_old).max(1e-9)).clamp(0.0, 1.0);
        Some(t_old + dt * f)
    } else {
        None
    }
}

/// Lateral acceleration in g for speed `v` on curvature `curvature` (right
/// positive), saturated just above the envelope's grip limit: the nominal
/// profile has a speed floor, so the tightest corners would otherwise report
/// more lateral g than the tyres can deliver.
fn lateral_g(v: f64, curvature: f64, env: &Envelope) -> f64 {
    let limit = env.a_lat / G * 1.04;
    (v * v * curvature / G).clamp(-limit, limit)
}

fn lag(value: &mut f64, target: f64, dt: f64, tau: f64) {
    *value += (target - *value) * (dt / tau).min(1.0);
}

fn min_opt(a: Option<f64>, b: f64) -> Option<f64> {
    Some(match a {
        Some(x) if x <= b => x,
        _ => b,
    })
}

fn f32o(v: Option<f64>) -> Option<f32> {
    v.map(|x| x as f32)
}

/// The simulated world. Call [`SimWorld::tick`] at `rate_hz` to get frames.
#[derive(Debug, Clone)]
pub struct SimWorld {
    cfg: SimConfig,
    track: Track,
    spec: ClassSpec,
    rng: Rng,
    dt: f64,
    seq: u64,
    elapsed: f64,
    start_elapsed: f64,
    wall_base_ms: u64,
    player: Player,
    ais: Vec<Ai>,
    events: Vec<GameEvent>,
    started: bool,
}

impl SimWorld {
    pub fn new(cfg: SimConfig) -> Self {
        let cfg = cfg.sanitized();
        let spec = ClassSpec::for_class(cfg.player_class);
        let rain = cfg.rain.clamp(0.0, 1.0) as f64;
        let rain_slowdown = 1.0 + 0.10 * rain;
        let player_lap = cfg.lap_time_s.max(30.0) as f64 * rain_slowdown;
        let track = Track::paul_ricard_like(
            cfg.track_length_m.max(500.0) as f64,
            player_lap,
            spec.envelope,
        );
        let mut rng = Rng::new(cfg.seed);
        let dt = 1.0 / cfg.rate_hz.max(1.0) as f64;
        let length = track.length_m;

        // GT3 reference lap used to derive the AI classes.
        let gt3_ref = match cfg.player_class {
            PlayerClass::Gt3 => player_lap,
            PlayerClass::Hypercar => player_lap / HYPERCAR_FACTOR,
        };
        let start_laps = cfg.start_laps_completed.max(0);
        let num_ai = cfg.num_ai.min(60);
        // The player sits exactly on the line with `start_laps` done; the AI
        // field has been running for `start_elapsed` minus a per-car "lost"
        // time of ≈ 0.45 s × grid slot, so the player's own "lost" time below
        // (≈ the median grid slot) puts them mid-pack in their class.
        let start_elapsed =
            start_laps as f64 * player_lap + 2.0 + 0.45 * (num_ai as f64 / 2.0 + 1.0);

        // --- AI field -------------------------------------------------------
        let n_hyper = ((num_ai as f64) * 6.0 / 19.0).round() as usize;
        let n_lmp2 = ((num_ai as f64) * 5.0 / 19.0).round() as usize;
        let n_hyper = n_hyper.min(num_ai);
        let n_lmp2 = n_lmp2.min(num_ai - n_hyper);
        let n_gt3 = num_ai - n_hyper - n_lmp2;

        let mut ais = Vec::with_capacity(num_ai);
        let mut next_id = 1;
        let mut names = (0..DRIVERS.len()).collect::<Vec<_>>();
        // Deterministic shuffle of the name list.
        for i in (1..names.len()).rev() {
            let j = rng.below(i + 1);
            names.swap(i, j);
        }
        let mut name_idx = 0usize;
        let mut push_ai = |ais: &mut Vec<Ai>,
                           rng: &mut Rng,
                           class: VehicleClass,
                           class_name: &'static str,
                           class_rank: u8,
                           vehicle_name: String,
                           class_lap: f64,
                           v_cap: f64| {
            let driver = DRIVERS[names[name_idx % names.len()]].to_string();
            name_idx += 1;
            let spread = 1.0 + rng.symmetric(0.012);
            let target = class_lap * spread;
            let speed_factor = track.lap_time_s / target;
            let pace_s = track.lap_time_scaled(speed_factor, v_cap);
            // Race distance so far: started `start_elapsed` ago from a grid
            // slot, with a bit of scatter.
            let grid = ais.len() as i32 + 1;
            let lost = 0.45 * grid as f64 + rng.range(-2.5, 4.0);
            let d = ((start_elapsed - lost).max(0.0) / pace_s * length).max(0.0);
            let laps_done = (d / length).floor() as i32;
            let s = d - laps_done as f64 * length;
            let into_lap = track.time_at(s) * pace_s / track.lap_time_s;
            let best = pace_s * (1.0 - rng.range(0.002, 0.006));
            let best_s1 = track.time_at(track.sector_ends_m[0]) * best / track.lap_time_s;
            let best_s2 = track.time_at(track.sector_ends_m[1]) * best / track.lap_time_s - best_s1;
            let fuel_per_lap_fraction = match class {
                VehicleClass::Hypercar => 0.036,
                VehicleClass::Lmp2 => 0.040,
                _ => 0.026,
            };
            let pit_lap = laps_done + 2 + rng.below(4) as i32;
            let fuel_fraction = (1.0 - fuel_per_lap_fraction * (d / length)).max(0.1);
            ais.push(Ai {
                id: next_id,
                driver,
                vehicle_name,
                class,
                class_name,
                class_rank,
                pace_s,
                speed_factor,
                v_cap,
                d,
                lap_start: start_elapsed - into_lap,
                lap_scale: 1.0 + rng.symmetric(0.6) / pace_s,
                cur_s1: if s > track.sector_ends_m[0] {
                    Some(best_s1 + rng.range(0.0, 0.4))
                } else {
                    None
                },
                cur_s2: if s > track.sector_ends_m[1] {
                    Some(best_s2 + rng.range(0.0, 0.4))
                } else {
                    None
                },
                last_s1: if laps_done > 0 {
                    Some(best_s1 + rng.range(0.0, 0.5))
                } else {
                    None
                },
                last_s2: if laps_done > 0 {
                    Some(best_s2 + rng.range(0.0, 0.5))
                } else {
                    None
                },
                best_s1: if laps_done > 0 { Some(best_s1) } else { None },
                best_s2: if laps_done > 0 { Some(best_s2) } else { None },
                last_lap: if laps_done > 0 {
                    Some(best + rng.range(0.05, 0.9))
                } else {
                    None
                },
                best_lap: if laps_done > 0 { Some(best) } else { None },
                fuel_fraction,
                fuel_per_lap_fraction,
                pit_lap,
                pit_phase: PitPhase::None,
                num_pitstops: 0,
                v: 0.0,
                headlights: class != VehicleClass::Gt3,
                lateral_phase: rng.range(0.0, TAU),
                grid,
            });
            next_id += 1;
        };
        for i in 0..n_hyper {
            let (model, team) = HYPERCARS[i % HYPERCARS.len()];
            push_ai(
                &mut ais,
                &mut rng,
                VehicleClass::Hypercar,
                "Hyper",
                0,
                format!("{team} · {model}"),
                gt3_ref * HYPERCAR_FACTOR,
                340.0 / 3.6,
            );
        }
        for i in 0..n_lmp2 {
            let team = LMP2_TEAMS[i % LMP2_TEAMS.len()];
            push_ai(
                &mut ais,
                &mut rng,
                VehicleClass::Lmp2,
                "LMP2",
                1,
                format!("{team} · Oreca 07"),
                gt3_ref * LMP2_FACTOR,
                318.0 / 3.6,
            );
        }
        for i in 0..n_gt3 {
            let (model, team) = GT3S[i % GT3S.len()];
            push_ai(
                &mut ais,
                &mut rng,
                VehicleClass::Gt3,
                "GT3",
                2,
                format!("{team} · {model}"),
                gt3_ref,
                302.0 / 3.6,
            );
        }

        // --- Player ---------------------------------------------------------
        let nominal = track.lap_time_s;
        let best = nominal * (1.0 - 0.0025);
        let last = nominal + 0.21;
        let t1 = track.time_at(track.sector_ends_m[0]);
        let t2 = track.time_at(track.sector_ends_m[1]);
        let sectors_for = |lap: f64| {
            let k = lap / nominal;
            [t1 * k, (t2 - t1) * k, (nominal - t2) * k]
        };
        let last_sectors = sectors_for(last);
        let best_lap_sectors = sectors_for(best);
        let fresh = start_laps == 0;
        let opt = |x: f64| if fresh { None } else { Some(x) };
        let player = Player {
            s: 0.0,
            // 0-based like the game: the number of laps completed so far.
            lap_number: start_laps,
            laps_completed: start_laps,
            lap_start: start_elapsed,
            lap_scale: 1.0 + rng.symmetric(0.5) / nominal,
            wiggle_phase: rng.range(0.0, TAU),
            cur_s1: None,
            cur_s2: None,
            last_lap: opt(last),
            best_lap: opt(best),
            last_sectors: [
                opt(last_sectors[0]),
                opt(last_sectors[1]),
                opt(last_sectors[2]),
            ],
            best_sectors: [
                opt(best_lap_sectors[0] - 0.08),
                opt(best_lap_sectors[1] - 0.05),
                opt(best_lap_sectors[2] - 0.11),
            ],
            best_lap_sectors: [opt(best_lap_sectors[0]), opt(best_lap_sectors[1])],
            gear: 6,
            v: track.at(0.0).v,
            a_long: 0.0,
            curvature: 0.0,
            throttle: 1.0,
            brake: 0.0,
            steering: 0.0,
            clutch: 0.0,
            shift_timer: 0.0,
            tyre_temp: [
                [86.0, 88.0, 90.0],
                [90.0, 88.0, 86.0],
                [84.0, 87.0, 89.0],
                [89.0, 87.0, 84.0],
            ],
            tyre_wear: {
                let laps = start_laps as f64;
                [
                    1.0 - 0.0055 * laps,
                    1.0 - 0.0045 * laps,
                    1.0 - 0.0048 * laps,
                    1.0 - 0.0040 * laps,
                ]
            },
            brake_temp: [420.0, 415.0, 350.0, 345.0],
            fuel_l: spec.fuel_start_l,
            ve: (1.0 - spec.ve_per_lap.unwrap_or(0.0) * start_laps as f64).max(0.2),
            soc: 0.62,
            water_c: 91.0,
            oil_c: 108.0,
            motor_temp_c: 68.0,
            tc_active: false,
            abs_active: false,
        };

        let wall_base_ms = cfg.wall_base_ms.unwrap_or_else(|| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0)
        });

        Self {
            cfg,
            track,
            spec,
            rng,
            dt,
            seq: 0,
            elapsed: start_elapsed,
            start_elapsed,
            wall_base_ms,
            player,
            ais,
            events: Vec::new(),
            started: false,
        }
    }

    pub fn config(&self) -> &SimConfig {
        &self.cfg
    }

    pub fn track(&self) -> &Track {
        &self.track
    }

    /// Simulated session time, seconds.
    pub fn elapsed_s(&self) -> f64 {
        self.elapsed
    }

    /// Game events produced since the last call (SessionStarted on the first tick).
    pub fn drain_events(&mut self) -> Vec<GameEvent> {
        std::mem::take(&mut self.events)
    }

    /// Advance the world by one tick (`1 / rate_hz`) and return the frame.
    pub fn tick(&mut self) -> TelemetryFrame {
        let dt = self.dt;
        let t_old = self.elapsed;
        self.elapsed += dt;
        self.seq += 1;

        self.step_player(t_old, dt);
        for i in 0..self.ais.len() {
            self.step_ai(i, t_old, dt);
        }

        let session = self.session_info();
        if !self.started {
            self.started = true;
            self.events.push(GameEvent::SessionStarted {
                session: session.clone(),
            });
        }
        let (standings, gaps, place) = self.standings();
        let player = self.player_telemetry(place, gaps);
        let wall_ms = self.wall_base_ms + ((self.elapsed - self.start_elapsed) * 1000.0) as u64;

        TelemetryFrame {
            v: PROTOCOL_VERSION,
            seq: self.seq,
            wall_ms,
            source: "sim".into(),
            session,
            player: Some(player),
            standings,
        }
    }

    // ---------------------------------------------------------------------
    // Player
    // ---------------------------------------------------------------------

    fn step_player(&mut self, t_old: f64, dt: f64) {
        let track = &self.track;
        let length = track.length_m;
        let p = &mut self.player;
        let env = self.spec.envelope;

        let smp = track.at(p.s);
        let wiggle = 1.0 + 0.004 * (3.0 * TAU * p.s / length + p.wiggle_phase).sin();
        let jitter = 1.0 + self.rng.symmetric(0.002);
        let v = smp.v / p.lap_scale * wiggle * jitter;
        let s_new = p.s + v * dt;

        // Sector and lap boundaries (interpolated crossing times).
        if let Some(t) = crossing(p.s, s_new, track.sector_ends_m[0], t_old, dt) {
            p.cur_s1 = Some(t - p.lap_start);
        }
        if let Some(t) = crossing(p.s, s_new, track.sector_ends_m[1], t_old, dt) {
            p.cur_s2 = Some(t - p.lap_start - p.cur_s1.unwrap_or(0.0));
        }
        if let Some(t_cross) = crossing(p.s, s_new, length, t_old, dt) {
            let lap_time = t_cross - p.lap_start;
            let s1 = p.cur_s1;
            let s2 = p.cur_s2;
            let s3 = match (s1, s2) {
                (Some(a), Some(b)) => Some(lap_time - a - b),
                _ => None,
            };
            p.last_lap = Some(lap_time);
            p.last_sectors = [s1, s2, s3];
            for i in 0..3 {
                if let Some(x) = p.last_sectors[i] {
                    p.best_sectors[i] = min_opt(p.best_sectors[i], x);
                }
            }
            if p.best_lap.is_none_or(|b| lap_time < b) {
                p.best_lap = Some(lap_time);
                p.best_lap_sectors = [s1, s2];
            }
            p.lap_number += 1;
            p.laps_completed += 1;
            p.lap_start = t_cross;
            p.cur_s1 = None;
            p.cur_s2 = None;
            p.lap_scale = 1.0 + self.rng.symmetric(0.5) / track.lap_time_s;
            p.wiggle_phase = self.rng.range(0.0, TAU);
        }
        p.s = track.wrap(s_new);
        p.v = v;
        p.curvature = smp.curvature;
        p.a_long = smp.a_long / (p.lap_scale * p.lap_scale);

        // Pedals and steering.
        let (thr, brk) = env.pedals(p.a_long, v);
        let thr = (thr + self.rng.symmetric(0.015)).clamp(0.0, 1.0);
        let brk = if brk > 0.0 {
            (brk + self.rng.symmetric(0.02)).clamp(0.02, 1.0)
        } else {
            0.0
        };
        p.throttle = thr;
        p.brake = brk;
        // Ackermann-ish: road wheel angle ≈ wheelbase × curvature, × steering ratio.
        let wheelbase = 2.7;
        let ratio = 12.0;
        let wheel_deg = (wheelbase * smp.curvature).atan().to_degrees() * ratio;
        let steer = (wheel_deg / (self.spec.wheel_range_deg / 2.0)) + self.rng.symmetric(0.004);
        p.steering = steer.clamp(-1.0, 1.0);

        // Gearbox: rpm proportional to speed within a gear, with hysteresis.
        let tops = &self.spec.gear_top_ms;
        let max_rpm = self.spec.max_rpm;
        let rpm_in = |gear: i8| -> f64 { max_rpm * v / tops[(gear.max(1) - 1) as usize] };
        let mut gear = p.gear.clamp(1, tops.len() as i8);
        let mut shifted = false;
        while gear < tops.len() as i8 && rpm_in(gear) > max_rpm * 0.975 {
            gear += 1;
            shifted = true;
        }
        while gear > 1 && rpm_in(gear) < max_rpm * 0.62 && rpm_in(gear - 1) < max_rpm * 0.97 {
            gear -= 1;
            shifted = true;
        }
        if shifted {
            p.shift_timer = 0.12;
        }
        p.gear = gear;
        p.shift_timer = (p.shift_timer - dt).max(0.0);
        p.clutch = if p.shift_timer > 0.0 { 0.8 } else { 0.0 };

        // Electronics activity.
        let g_lat = lateral_g(v, smp.curvature, &env).abs();
        p.tc_active = thr > 0.75 && g_lat > 0.9 && gear <= 3;
        p.abs_active = brk > 0.85 && v > 28.0;

        // Thermal & wear models.
        let g_lat_r = lateral_g(v, smp.curvature, &env); // right positive
        let speed_norm = (v / env.v_max).clamp(0.0, 1.0);
        for (i, t) in p.tyre_temp.iter_mut().enumerate() {
            let is_left = i % 2 == 0;
            let is_front = i < 2;
            let load = if is_left {
                g_lat_r.max(0.0)
            } else {
                (-g_lat_r).max(0.0)
            };
            let long = if is_front { brk * 4.5 } else { thr * 3.5 };
            let base = 82.0 + 14.0 * load + long + 2.5 * speed_norm;
            // Outer shoulder heats under lateral load, inner shoulder runs warm from camber.
            let (outer, inner) = if is_left { (0usize, 2usize) } else { (2, 0) };
            let mut target = [base; 3];
            target[outer] += 7.0 * load;
            target[inner] += 3.0;
            target[1] += 1.0;
            for z in 0..3 {
                let tau = if target[z] > t[z] { 5.0 } else { 11.0 };
                lag(&mut t[z], target[z], dt, tau);
                t[z] = t[z].clamp(60.0, 115.0);
            }
            let wear_rate = 0.005 / track.lap_time_s
                * (0.55 + 1.1 * load + if is_front { 0.5 * brk } else { 0.5 * thr })
                / 1.0;
            p.tyre_wear[i] = (p.tyre_wear[i] - wear_rate * dt).max(0.0);

            let share = if is_front { 1.0 } else { 0.72 };
            let heat = 2.1 * share * brk * v;
            let cool = (0.018 + 0.000_55 * v) * (p.brake_temp[i] - 180.0);
            p.brake_temp[i] = (p.brake_temp[i] + (heat - cool) * dt).clamp(150.0, 720.0);
        }

        // Fuel / energy scaled so that one nominal lap burns `fuel_per_lap_l`.
        let burn = thr * dt / track.throttle_integral_s;
        p.fuel_l = (p.fuel_l - self.spec.fuel_per_lap_l * burn).max(0.0);
        if let Some(ve) = self.spec.ve_per_lap {
            p.ve = (p.ve - ve * burn).max(0.0);
            // Hybrid: regen under braking, deploy on throttle.
            let d_soc = 0.045 * brk * dt - 0.035 * thr * dt * if p.soc > 0.2 { 1.0 } else { 0.0 };
            p.soc = (p.soc + d_soc).clamp(0.15, 0.95);
            lag(
                &mut p.motor_temp_c,
                62.0 + 22.0 * (thr + brk).min(1.0),
                dt,
                15.0,
            );
        }
        lag(
            &mut p.water_c,
            88.0 + 6.0 * thr + 1.5 * speed_norm,
            dt,
            25.0,
        );
        lag(
            &mut p.oil_c,
            104.0 + 12.0 * thr + 3.0 * speed_norm,
            dt,
            40.0,
        );
    }

    fn player_rpm(&self) -> f64 {
        let p = &self.player;
        let top = self.spec.gear_top_ms[(p.gear.max(1) - 1) as usize];
        (self.spec.max_rpm * p.v / top).clamp(1200.0, self.spec.max_rpm)
    }

    // ---------------------------------------------------------------------
    // AI
    // ---------------------------------------------------------------------

    fn step_ai(&mut self, i: usize, t_old: f64, dt: f64) {
        let track = &self.track;
        let length = track.length_m;
        let a = &mut self.ais[i];
        let s_old = a.s(track);
        let laps = a.laps_completed(track);
        let smp = track.at(s_old);
        let mut v = (smp.v * a.speed_factor / a.lap_scale).min(a.v_cap);

        // Pit stop scenario.
        if self.cfg.ai_pit_stops {
            match a.pit_phase {
                PitPhase::None => {
                    if laps == a.pit_lap && s_old >= length - 300.0 {
                        a.pit_phase = PitPhase::InLane;
                    }
                }
                PitPhase::InLane => {
                    v = v.min(PIT_SPEED_MS);
                    if s_old >= length - 60.0 {
                        a.pit_phase = PitPhase::Stopped(PIT_STOP_S + self.rng.range(-3.0, 4.0));
                    }
                }
                PitPhase::Stopped(rem) => {
                    v = 0.0;
                    let rem = rem - dt;
                    if rem <= 0.0 {
                        a.pit_phase = PitPhase::Exiting;
                        a.fuel_fraction = 0.97;
                    } else {
                        a.pit_phase = PitPhase::Stopped(rem);
                    }
                }
                PitPhase::Exiting => {
                    v = v.min(PIT_SPEED_MS);
                    if laps > a.pit_lap && s_old >= 220.0 {
                        a.pit_phase = PitPhase::None;
                        a.num_pitstops += 1;
                    }
                }
            }
        }

        let s_new_unwrapped = s_old + v * dt;
        if let Some(t) = crossing(s_old, s_new_unwrapped, track.sector_ends_m[0], t_old, dt) {
            a.cur_s1 = Some(t - a.lap_start);
        }
        if let Some(t) = crossing(s_old, s_new_unwrapped, track.sector_ends_m[1], t_old, dt) {
            a.cur_s2 = Some(t - a.lap_start - a.cur_s1.unwrap_or(0.0));
        }
        if let Some(t_cross) = crossing(s_old, s_new_unwrapped, length, t_old, dt) {
            let lap_time = t_cross - a.lap_start;
            a.last_lap = Some(lap_time);
            a.last_s1 = a.cur_s1;
            a.last_s2 = a.cur_s2;
            if let Some(x) = a.cur_s1 {
                a.best_s1 = min_opt(a.best_s1, x);
            }
            if let Some(x) = a.cur_s2 {
                a.best_s2 = min_opt(a.best_s2, x);
            }
            if !a.in_pits() {
                a.best_lap = min_opt(a.best_lap, lap_time);
            }
            a.cur_s1 = None;
            a.cur_s2 = None;
            a.lap_start = t_cross;
            a.lap_scale = 1.0 + self.rng.symmetric(0.6) / a.pace_s;
        }
        a.d += v * dt;
        a.v = v;
        a.fuel_fraction = (a.fuel_fraction - a.fuel_per_lap_fraction * (v * dt / length)).max(0.02);
    }

    // ---------------------------------------------------------------------
    // Frame assembly
    // ---------------------------------------------------------------------

    fn session_info(&self) -> SessionInfo {
        let rain = self.cfg.rain.clamp(0.0, 1.0);
        let drift = (self.elapsed / 600.0 * TAU).sin() as f32;
        let end_s = 3600.0;
        SessionInfo {
            game: self.cfg.game,
            track_name: "Paul Ricard - 1A".into(),
            track_length_m: self.track.length_m as f32,
            session_type: SessionType::Race,
            session_index: 10,
            phase: GamePhase::Green,
            yellow_flag: YellowFlagState::None,
            sector_flags: [0, 0, 0],
            elapsed_s: self.elapsed,
            end_s,
            time_remaining_s: (end_s - self.elapsed).max(0.0) as f32,
            max_laps: 0,
            num_vehicles: (self.ais.len() + 1) as i32,
            in_realtime: true,
            is_fixed_setup: false,
            player_name: self.cfg.player_name.clone(),
            server_name: String::new(),
            is_multiplayer: false,
            weather: Weather {
                ambient_c: 24.0 + 0.4 * drift - 3.0 * rain,
                track_c: 31.0 + 0.8 * drift - 7.0 * rain,
                rain,
                wetness_min: (rain * 0.5).min(1.0),
                wetness_max: (rain * 0.95).min(1.0),
                wetness_avg: (rain * 0.75).min(1.0),
                cloud_darkness: 0.15 + 0.7 * rain,
                cloud_coverage: Some((2.0 + 8.0 * rain).round() as u8),
                track_grip_level: Some(if rain > 0.05 { 1 } else { 3 }),
                wind_ms: [0.0, 0.0, 0.0],
                time_of_day_s: (14.0 * 3600.0 + self.elapsed) as f32,
            },
        }
    }

    /// Sorted standings plus the player's gaps and overall place.
    fn standings(&self) -> (Vec<Standing>, Gaps, u8) {
        let track = &self.track;
        let length = track.length_m;
        let p = &self.player;
        let player_d = p.laps_completed as f64 * length + p.s;
        let player_pace = track.lap_time_s;

        // (index: usize::MAX = player, d, s, pace, class_rank)
        struct Row {
            ai: Option<usize>,
            d: f64,
            s: f64,
            pace: f64,
            class_rank: u8,
        }
        let mut rows: Vec<Row> = Vec::with_capacity(self.ais.len() + 1);
        rows.push(Row {
            ai: None,
            d: player_d,
            s: p.s,
            pace: player_pace,
            class_rank: self.spec_rank(),
        });
        for (i, a) in self.ais.iter().enumerate() {
            rows.push(Row {
                ai: Some(i),
                d: a.d,
                s: a.s(track),
                pace: a.pace_s,
                class_rank: a.class_rank,
            });
        }
        rows.sort_by(|a, b| b.d.partial_cmp(&a.d).unwrap_or(std::cmp::Ordering::Equal));

        let leader_d = rows[0].d;
        let mut class_counts = [0u8; 8];
        let mut out = Vec::with_capacity(rows.len());
        let mut player_place = 1u8;
        let mut player_idx = 0usize;
        for (idx, row) in rows.iter().enumerate() {
            let place = (idx + 1) as u8;
            let v_avg = length / row.pace;
            let (gap_next_m, gap_leader_m) = if idx == 0 {
                (0.0, 0.0)
            } else {
                (rows[idx - 1].d - row.d, leader_d - row.d)
            };
            let laps_next = (gap_next_m / length).floor();
            let laps_leader = (gap_leader_m / length).floor();
            let time_next = (gap_next_m - laps_next * length) / v_avg;
            let time_leader = (gap_leader_m - laps_leader * length) / v_avg;
            // Blue flag: a faster-class car that is ahead on distance is close behind on track.
            let blue = rows.iter().any(|o| {
                o.class_rank < row.class_rank && o.d > row.d && {
                    let behind_m = track.wrap(row.s - o.s);
                    behind_m < 1.2 * v_avg
                }
            });
            let smp = track.at(row.s);
            let sector = track.sector(row.s);
            let laps_completed = (row.d / length).floor() as i32;
            let class_idx = row.class_rank as usize;
            class_counts[class_idx] += 1;
            let class_place = class_counts[class_idx];

            let standing = match row.ai {
                None => {
                    player_place = place;
                    player_idx = idx;
                    Standing {
                        id: 0,
                        driver_name: self.cfg.player_name.clone(),
                        vehicle_name: self.spec.vehicle_name.into(),
                        class_name: self.spec.class_name.into(),
                        vehicle_class: self.spec.vehicle_class,
                        is_player: true,
                        control: Control::LocalPlayer,
                        place,
                        class_place,
                        laps_completed,
                        sector,
                        finish_status: FinishStatus::None,
                        lap_dist_m: row.s as f32,
                        lap_fraction: (row.s / length) as f32,
                        path_lateral_m: (1.5 * (row.s / 41.0).sin()) as f32,
                        in_pits: false,
                        pit_state: PitState::None,
                        in_garage: false,
                        num_pitstops: 0,
                        num_penalties: 0,
                        time_behind_next_s: time_next as f32,
                        laps_behind_next: laps_next as i32,
                        time_behind_leader_s: time_leader as f32,
                        laps_behind_leader: laps_leader as i32,
                        best_lap_s: f32o(p.best_lap),
                        last_lap_s: f32o(p.last_lap),
                        best_s1: f32o(p.best_sectors[0]),
                        best_s2: f32o(p.best_sectors[1]),
                        last_s1: f32o(p.last_sectors[0]),
                        last_s2: f32o(p.last_sectors[1]),
                        cur_s1: f32o(p.cur_s1),
                        cur_s2: f32o(p.cur_s2),
                        time_into_lap_s: (self.elapsed - p.lap_start) as f32,
                        estimated_lap_s: self.estimated_lap() as f32,
                        pos: [smp.x as f32, self.elevation(row.s) as f32, smp.z as f32],
                        speed_kmh: (p.v * 3.6) as f32,
                        fuel_fraction: Some((p.fuel_l / self.spec.fuel_capacity_l) as f32),
                        flag_blue: blue,
                        under_yellow: false,
                        headlights: self.spec.vehicle_class != VehicleClass::Gt3,
                        drs: false,
                        qualification: 12,
                        pit_lap_dist_m: (length - 60.0) as f32,
                    }
                }
                Some(i) => {
                    let a = &self.ais[i];
                    let lateral = 1.8 * (row.s / 37.0 + a.lateral_phase).sin();
                    let (nx, nz) = Track::right_normal(smp.heading);
                    let pit_state = match a.pit_phase {
                        PitPhase::None => PitState::None,
                        PitPhase::InLane => PitState::Entering,
                        PitPhase::Stopped(_) => PitState::Stopped,
                        PitPhase::Exiting => PitState::Exiting,
                    };
                    Standing {
                        id: a.id,
                        driver_name: a.driver.clone(),
                        vehicle_name: a.vehicle_name.clone(),
                        class_name: a.class_name.into(),
                        vehicle_class: a.class,
                        is_player: false,
                        control: Control::LocalAi,
                        place,
                        class_place,
                        laps_completed,
                        sector,
                        finish_status: FinishStatus::None,
                        lap_dist_m: row.s as f32,
                        lap_fraction: (row.s / length) as f32,
                        path_lateral_m: lateral as f32,
                        in_pits: a.in_pits(),
                        pit_state,
                        in_garage: false,
                        num_pitstops: a.num_pitstops,
                        num_penalties: 0,
                        time_behind_next_s: time_next as f32,
                        laps_behind_next: laps_next as i32,
                        time_behind_leader_s: time_leader as f32,
                        laps_behind_leader: laps_leader as i32,
                        best_lap_s: f32o(a.best_lap),
                        last_lap_s: f32o(a.last_lap),
                        best_s1: f32o(a.best_s1),
                        best_s2: f32o(a.best_s2),
                        last_s1: f32o(a.last_s1),
                        last_s2: f32o(a.last_s2),
                        cur_s1: f32o(a.cur_s1),
                        cur_s2: f32o(a.cur_s2),
                        time_into_lap_s: (self.elapsed - a.lap_start) as f32,
                        estimated_lap_s: (a.best_lap.unwrap_or(a.pace_s) + 0.3) as f32,
                        pos: [
                            (smp.x + nx * lateral) as f32,
                            self.elevation(row.s) as f32,
                            (smp.z + nz * lateral) as f32,
                        ],
                        speed_kmh: (a.v * 3.6) as f32,
                        fuel_fraction: Some(a.fuel_fraction as f32),
                        flag_blue: blue,
                        under_yellow: false,
                        headlights: a.headlights,
                        drs: false,
                        qualification: a.grid + 1,
                        pit_lap_dist_m: (length - 60.0) as f32,
                    }
                }
            };
            out.push(standing);
        }

        // Player gaps: by place, and physically on track.
        let v_avg = length / player_pace;
        let place_ahead = if player_idx > 0 {
            Some(out[player_idx].time_behind_next_s)
        } else {
            None
        };
        let place_behind = out.get(player_idx + 1).map(|s| s.time_behind_next_s);
        let mut ahead_m = f64::MAX;
        let mut behind_m = f64::MAX;
        for a in &self.ais {
            let s = a.s(track);
            let fwd = track.wrap(s - p.s);
            let back = track.wrap(p.s - s);
            if fwd > 0.0 && fwd < ahead_m {
                ahead_m = fwd;
            }
            if back > 0.0 && back < behind_m {
                behind_m = back;
            }
        }
        let gaps = Gaps {
            car_ahead_s: if ahead_m < f64::MAX {
                Some((ahead_m / v_avg) as f32)
            } else {
                None
            },
            car_behind_s: if behind_m < f64::MAX {
                Some((behind_m / v_avg) as f32)
            } else {
                None
            },
            place_ahead_s: place_ahead,
            place_behind_s: place_behind,
        };
        (out, gaps, player_place)
    }

    fn spec_rank(&self) -> u8 {
        match self.spec.vehicle_class {
            VehicleClass::Hypercar => 0,
            VehicleClass::Lmp2 => 1,
            _ => 2,
        }
    }

    fn elevation(&self, s: f64) -> f64 {
        2.0 + 4.0 * (TAU * s / self.track.length_m).sin()
            + 1.5 * (3.0 * TAU * s / self.track.length_m + 1.0).sin()
    }

    /// Live delta to the best lap: compares the running lap time with the best
    /// lap's time at the same distance (nominal profile scaled to the best lap).
    fn delta_best(&self) -> f64 {
        let p = &self.player;
        match p.best_lap {
            Some(best) => {
                let reference = self.track.time_at(p.s) * best / self.track.lap_time_s;
                (self.elapsed - p.lap_start) - reference
            }
            None => 0.0,
        }
    }

    fn estimated_lap(&self) -> f64 {
        self.player.best_lap.unwrap_or(self.track.lap_time_s) + self.delta_best()
    }

    fn player_telemetry(&self, place: u8, gaps: Gaps) -> PlayerTelemetry {
        let p = &self.player;
        let track = &self.track;
        let spec = &self.spec;
        let smp = track.at(p.s);
        let v = p.v;
        let rpm = self.player_rpm();
        let g_lat = lateral_g(v, smp.curvature, &spec.envelope);
        let g_long = p.a_long / G;
        let yaw_rate = v * smp.curvature; // rad/s, right positive
        let speed_norm = (v / spec.envelope.v_max).clamp(0.0, 1.0);
        let q = v * v; // for aero terms
        let front_df = 0.75 * q;
        let rear_df = 1.10 * q;
        let drag = 0.65 * q;
        let elev = self.elevation(p.s);
        let is_hybrid = spec.ve_per_lap.is_some();

        let lap = LapTiming {
            lap_number: p.lap_number,
            laps_completed: p.laps_completed,
            lap_start_s: p.lap_start,
            current_lap_s: (self.elapsed - p.lap_start) as f32,
            current_sector: track.sector(p.s),
            in_pit_lane: false,
            lap_invalidated: false,
            delta_best_s: self.delta_best() as f32,
            last_lap_s: f32o(p.last_lap),
            best_lap_s: f32o(p.best_lap),
            last_s1: f32o(p.last_sectors[0]),
            last_s2: f32o(p.last_sectors[1]),
            last_s3: f32o(p.last_sectors[2]),
            best_s1: f32o(p.best_sectors[0]),
            best_s2: f32o(p.best_sectors[1]),
            best_s3: f32o(p.best_sectors[2]),
            cur_s1: f32o(p.cur_s1),
            cur_s2: f32o(p.cur_s2),
            best_lap_s1: f32o(p.best_lap_sectors[0]),
            best_lap_s2: f32o(p.best_lap_sectors[1]),
            estimated_lap_s: self.estimated_lap() as f32,
            time_into_lap_s: (self.elapsed - p.lap_start) as f32,
            lap_dist_m: p.s as f32,
            lap_fraction: (p.s / track.length_m) as f32,
            count_lap_flag: Some(2),
        };

        let motion = Motion {
            pos: [smp.x, elev, smp.z],
            local_vel: [0.0, 0.0, -(v as f32)],
            local_accel: [-(g_lat * G) as f32, 0.0, -(p.a_long as f32)],
            local_rot: [0.0, yaw_rate as f32, 0.0],
            speed_ms: v as f32,
            speed_kmh: (v * 3.6) as f32,
            g_lat: g_lat as f32,
            g_long: g_long as f32,
            g_vert: (1.0 + 0.02 * (p.s / 9.0).sin()) as f32,
            yaw_rate_dps: yaw_rate.to_degrees() as f32,
            heading_rad: smp.heading as f32,
        };

        let torque_curve = (1.0 - ((rpm - 6600.0) / 4200.0).powi(2)).clamp(0.55, 1.0);
        let engine = Engine {
            rpm: rpm as f32,
            max_rpm: spec.max_rpm as f32,
            gear: p.gear,
            max_gears: spec.gears,
            water_c: p.water_c as f32,
            oil_c: p.oil_c as f32,
            torque_nm: (p.throttle * 640.0 * torque_curve) as f32,
            turbo_boost: if spec.turbo {
                (0.4 + 1.3 * p.throttle) as f32
            } else {
                0.0
            },
            clutch_rpm: if p.clutch > 0.0 {
                (rpm * 0.93) as f32
            } else {
                rpm as f32
            },
            ignition_starter: 1,
            speed_limiter: false,
            speed_limiter_available: true,
            anti_stall: false,
            overheating: false,
        };

        let steering_torque = (g_lat * 7.5 + smp.curvature * 40.0) as f32;
        let inputs = Inputs {
            throttle: if p.tc_active {
                (p.throttle * 0.85) as f32
            } else {
                p.throttle as f32
            },
            brake: if p.abs_active {
                (p.brake * 0.9) as f32
            } else {
                p.brake as f32
            },
            steering: p.steering as f32,
            clutch: p.clutch as f32,
            throttle_raw: p.throttle as f32,
            brake_raw: p.brake as f32,
            steering_raw: p.steering as f32,
            clutch_raw: p.clutch as f32,
            steering_torque_nm: steering_torque,
            steering_angle_deg: (p.steering * spec.wheel_range_deg / 2.0) as f32,
            wheel_range_deg: spec.wheel_range_deg as f32,
            ffb_torque: (steering_torque / 20.0).clamp(-1.0, 1.0),
        };

        let aero = Aero {
            front_ride_height_mm: (58.0 - 9.0 * speed_norm * speed_norm) as f32,
            rear_ride_height_mm: (72.0 - 12.0 * speed_norm * speed_norm) as f32,
            front_wing_height_mm: (52.0 - 8.0 * speed_norm * speed_norm) as f32,
            front_downforce_n: front_df as f32,
            rear_downforce_n: rear_df as f32,
            drag_n: drag as f32,
            third_front_mm: (6.0 * speed_norm * speed_norm) as f32,
            third_rear_mm: (9.0 * speed_norm * speed_norm) as f32,
            front_flap_active: false,
            rear_flap_active: false,
            rear_flap_legal: 0,
        };

        let (motor_state, regen_kw, motor_torque) = if is_hybrid {
            if p.brake > 0.05 {
                (
                    MotorState::Regeneration,
                    Some((p.brake * 200.0) as f32),
                    -(p.brake * 180.0) as f32,
                )
            } else if p.throttle > 0.3 && p.soc > 0.2 {
                (
                    MotorState::Propulsion,
                    Some(0.0),
                    (p.throttle * 220.0) as f32,
                )
            } else {
                (MotorState::Inactive, Some(0.0), 0.0)
            }
        } else {
            (MotorState::Unavailable, None, 0.0)
        };
        let energy = Energy {
            fuel_l: p.fuel_l as f32,
            fuel_capacity_l: spec.fuel_capacity_l as f32,
            fuel_fraction: (p.fuel_l / spec.fuel_capacity_l) as f32,
            virtual_energy: if is_hybrid { Some(p.ve as f32) } else { None },
            battery_soc: if is_hybrid { Some(p.soc as f32) } else { None },
            regen_kw,
            motor_torque_nm: motor_torque,
            motor_rpm: if is_hybrid { (v * 180.0) as f32 } else { 0.0 },
            motor_temp_c: if is_hybrid {
                p.motor_temp_c as f32
            } else {
                0.0
            },
            motor_water_c: if is_hybrid {
                (p.motor_temp_c - 12.0) as f32
            } else {
                0.0
            },
            motor_state,
            scheduled_stops: 0,
        };

        let electronics = Electronics {
            tc: 5,
            tc_max: 11,
            tc_slip: 4,
            tc_slip_max: 11,
            tc_cut: 3,
            tc_cut_max: 11,
            abs: 6,
            abs_max: 11,
            motor_map: if is_hybrid { 3 } else { 0 },
            motor_map_max: if is_hybrid { 5 } else { 0 },
            brake_migration: if is_hybrid { 2 } else { 0 },
            brake_migration_max: if is_hybrid { 6 } else { 0 },
            arb_front: 3,
            arb_front_max: 7,
            arb_rear: 2,
            arb_rear_max: 7,
            brake_bias_rear: 0.455,
            tc_active: p.tc_active,
            abs_active: p.abs_active,
            lift_and_coast: 0,
            wiper: if self.cfg.rain > 0.1 { 1 } else { 0 },
            headlights: spec.vehicle_class != VehicleClass::Gt3,
        };

        let static_load = 1300.0 * G / 4.0;
        let mut wheels: [Wheel; 4] = Default::default();
        for (i, w) in wheels.iter_mut().enumerate() {
            let is_left = i % 2 == 0;
            let is_front = i < 2;
            let lat_sign = if is_left { 1.0 } else { -1.0 }; // left tyres loaded in right turns
            let long_sign = if is_front { -1.0 } else { 1.0 }; // fronts loaded under braking
            let df = if is_front { front_df } else { rear_df } / 2.0;
            let load = static_load
                + df
                + 0.35 * static_load * (lat_sign * g_lat)
                + 0.25 * static_load * (long_sign * g_long);
            let load = load.max(200.0);
            let t = p.tyre_temp[i];
            let t_avg = (t[0] + t[1] + t[2]) / 3.0;
            let pressure = 136.0 + 0.3 * t_avg + [1.5, -1.0, 0.5, -1.5][i];
            let radius = if is_front { 0.330 } else { 0.342 };
            let slip_angle = (smp.curvature * 55.0 * (1.0 + v / 60.0)).clamp(-6.0, 6.0)
                * if is_front { 1.0 } else { 0.7 };
            let slip_ratio = if is_front {
                -0.02 * p.brake
            } else {
                0.03 * p.throttle - 0.015 * p.brake
            } * (1.0 + 0.3 * (p.s / 13.0).sin());
            let grip_fract =
                (0.04 + 0.22 * (g_lat.abs() / 1.8).min(1.0) + 0.12 * p.brake + 0.06 * p.throttle)
                    .min(0.6);
            let lat_force = -load * g_lat * 1.0;
            let long_force = load * g_long * if is_front { 0.6 } else { 1.0 };
            let susp = 0.028 + 0.012 * (load - static_load) / static_load;
            *w = Wheel {
                susp_deflection_m: susp as f32,
                ride_height_m: (if is_front {
                    aero.front_ride_height_mm
                } else {
                    aero.rear_ride_height_mm
                }) / 1000.0,
                susp_force_n: load as f32,
                brake_temp_c: p.brake_temp[i] as f32,
                brake_pressure: (p.brake * if is_front { 1.0 } else { 0.83 }) as f32,
                rotation_rads: (v / radius) as f32,
                camber_deg: if is_front { -3.3 } else { -2.6 },
                toe_deg: if is_front { -0.10 } else { 0.15 },
                lateral_patch_vel: (v * (slip_angle.to_radians()).sin()) as f32,
                longitudinal_patch_vel: (v * slip_ratio) as f32,
                lateral_ground_vel: (v * (slip_angle.to_radians()).sin() * 0.9) as f32,
                longitudinal_ground_vel: v as f32,
                lateral_force_n: lat_force as f32,
                longitudinal_force_n: long_force as f32,
                load_n: load as f32,
                grip_fract: grip_fract as f32,
                pressure_kpa: pressure as f32,
                temp_c: [t[0] as f32, t[1] as f32, t[2] as f32],
                temp_inner_c: [
                    (t[0] - 7.0) as f32,
                    (t[1] - 6.0) as f32,
                    (t[2] - 7.0) as f32,
                ],
                carcass_c: (t_avg - 4.0) as f32,
                optimal_temp_c: Some(90.0),
                wear_remaining: p.tyre_wear[i] as f32,
                surface: if self.cfg.rain > 0.3 {
                    SurfaceType::Wet
                } else {
                    SurfaceType::Dry
                },
                terrain: "ROAD".into(),
                flat: false,
                detached: false,
                compound_index: 0,
                compound_type: CompoundType::Medium,
                radius_m: radius as f32,
                vertical_deflection_m: (0.010 + 0.006 * (load - static_load) / static_load) as f32,
                slip_ratio: slip_ratio as f32,
                slip_angle_deg: slip_angle as f32,
            };
        }

        PlayerTelemetry {
            id: 0,
            vehicle_name: spec.vehicle_name.into(),
            vehicle_model: spec.model.into(),
            vehicle_class: spec.vehicle_class,
            class_name: spec.class_name.into(),
            championship: "WEC".into(),
            place,
            lap,
            motion,
            engine,
            inputs,
            aero,
            energy,
            electronics,
            wheels,
            damage: Damage::default(),
            gaps,
            tyres: TyreSet {
                front_compound: "Medium".into(),
                rear_compound: "Medium".into(),
                front_compound_index: 0,
                rear_compound_index: 0,
            },
            track_limits_steps: 0,
            in_pits: false,
            pit_state: PitState::None,
            in_garage: false,
            finish_status: FinishStatus::None,
        }
    }
}
