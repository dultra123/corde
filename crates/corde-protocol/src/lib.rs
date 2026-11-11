//! # corde-protocol
//!
//! Single source of truth for the data model shared by every Corde component:
//! the Rust core (game adapters, recorder, analysis), the local server, the
//! desktop/mobile dashboards and the OBS browser source.
//!
//! Design rules:
//! * **Game-agnostic.** Every game adapter (LMU first, then ACC, AC EVO, iRacing…)
//!   converts its native data into these types. Fields a game cannot provide are
//!   `None`, `0` or `Unknown`, never guessed.
//! * **Human units.** °C, km/h and m/s, degrees, kPa, litres, seconds. Raw game
//!   units (Kelvin, radians) are converted by the adapter.
//! * **Serde + ts-rs.** TypeScript definitions are generated from these structs
//!   (`cargo test -p corde-protocol` writes `packages/protocol/src/generated/`).
//!
//! Protocol stability: bump [`PROTOCOL_VERSION`] on any breaking change.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Wire protocol version negotiated in [`ServerMessage::Hello`].
pub const PROTOCOL_VERSION: u32 = 1;

// ---------------------------------------------------------------------------
// Enumerations
// ---------------------------------------------------------------------------

/// Supported (or planned) simulators.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS, Default)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum GameId {
    /// Le Mans Ultimate (Studio 397). First supported game.
    Lmu,
    Rfactor2,
    Acc,
    AcEvo,
    Iracing,
    Ams2,
    /// Built-in simulator used for demos and tests.
    Sim,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, Default)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum SessionType {
    TestDay,
    Practice,
    Qualify,
    Warmup,
    Race,
    #[default]
    Unknown,
}

/// Session phase (mirrors the rFactor-family `mGamePhase`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, Default)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum GamePhase {
    #[default]
    BeforeSession,
    Reconnaissance,
    GridWalk,
    Formation,
    Countdown,
    Green,
    FullCourseYellow,
    SessionStopped,
    SessionOver,
    Paused,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, Default)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum YellowFlagState {
    Invalid,
    #[default]
    None,
    Pending,
    PitsClosed,
    PitLeadLap,
    PitsOpen,
    LastLap,
    Resume,
    RaceHalt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS, Default)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum VehicleClass {
    Hypercar,
    Lmp2,
    Lmp3,
    Gte,
    Gt3,
    PaceCar,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, Default)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum SurfaceType {
    Dry,
    Wet,
    Grass,
    Dirt,
    Gravel,
    Rumble,
    Special,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, Default)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum CompoundType {
    Soft,
    Medium,
    Hard,
    Wet,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, Default)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum PitState {
    #[default]
    None,
    Request,
    Entering,
    Stopped,
    Exiting,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, Default)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum MotorState {
    #[default]
    Unavailable,
    Inactive,
    Propulsion,
    Regeneration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, Default)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum FinishStatus {
    #[default]
    None,
    Finished,
    Dnf,
    Dq,
}

/// Who drives a vehicle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, Default)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum Control {
    Nobody,
    LocalPlayer,
    LocalAi,
    Remote,
    Replay,
    #[default]
    Unknown,
}

// ---------------------------------------------------------------------------
// Session-level data
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, Default)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct Weather {
    pub ambient_c: f32,
    pub track_c: f32,
    /// Rain intensity 0..1.
    pub rain: f32,
    /// Wetness of the racing line, 0..1.
    pub wetness_min: f32,
    pub wetness_max: f32,
    pub wetness_avg: f32,
    /// Cloud darkness 0..1.
    pub cloud_darkness: f32,
    /// Cloud coverage 0..10 (LMU ≥ 1.3) — `None` when the game does not expose it.
    pub cloud_coverage: Option<u8>,
    /// Track rubbering level 0 (green) .. 4 (saturated) — LMU specific.
    pub track_grip_level: Option<u8>,
    /// Wind vector in m/s (x, y, z). Always zero in LMU/rF2.
    pub wind_ms: [f32; 3],
    /// In-game time of day, seconds since midnight.
    pub time_of_day_s: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, Default)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct SessionInfo {
    pub game: GameId,
    pub track_name: String,
    pub track_length_m: f32,
    pub session_type: SessionType,
    /// Raw session index from the game (LMU: 0 test, 1-4 practice, 5-8 qual, 9 warmup, 10-13 race).
    pub session_index: i32,
    pub phase: GamePhase,
    pub yellow_flag: YellowFlagState,
    /// Local yellow per sector, index 0..2 (sector order as reported by the game).
    pub sector_flags: [i8; 3],
    /// Session elapsed time in seconds.
    pub elapsed_s: f64,
    /// Session end time (elapsed) in seconds, 0 when unlimited.
    pub end_s: f64,
    pub time_remaining_s: f32,
    /// Maximum laps, 0 / huge when timed session.
    pub max_laps: i32,
    pub num_vehicles: i32,
    /// `true` when the player is on track (not at the monitor).
    pub in_realtime: bool,
    pub is_fixed_setup: bool,
    pub player_name: String,
    pub server_name: String,
    pub is_multiplayer: bool,
    pub weather: Weather,
}

// ---------------------------------------------------------------------------
// Player telemetry (high rate)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, Default)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct LapTiming {
    /// Lap in progress, **0-based**: the number of laps completed when it
    /// started (the game's own counter). Show `lap_number + 1` to the driver:
    /// that is the lap number the game displays and writes in its results.
    pub lap_number: i32,
    pub laps_completed: i32,
    /// Session time at which the current lap started.
    pub lap_start_s: f64,
    /// Running time of the current lap, derived.
    pub current_lap_s: f32,
    /// Current sector 1..3.
    pub current_sector: u8,
    pub in_pit_lane: bool,
    pub lap_invalidated: bool,
    /// Live delta to personal best lap, seconds (negative = faster).
    pub delta_best_s: f32,
    pub last_lap_s: Option<f32>,
    pub best_lap_s: Option<f32>,
    pub last_s1: Option<f32>,
    pub last_s2: Option<f32>,
    pub last_s3: Option<f32>,
    pub best_s1: Option<f32>,
    pub best_s2: Option<f32>,
    pub best_s3: Option<f32>,
    pub cur_s1: Option<f32>,
    pub cur_s2: Option<f32>,
    /// Sector 1 / 2 times of the best lap (not best sectors).
    pub best_lap_s1: Option<f32>,
    pub best_lap_s2: Option<f32>,
    pub estimated_lap_s: f32,
    pub time_into_lap_s: f32,
    /// Distance travelled around the lap, metres.
    pub lap_dist_m: f32,
    /// `lap_dist_m / track_length_m`, 0..1.
    pub lap_fraction: f32,
    /// Raw "count lap" flag of the game for the lap in progress, when it has
    /// one (rFactor family: 0 = neither lap nor time count, 1 = lap counts but
    /// not its time, 2 = lap and time count). Recorded for diagnosis; lap
    /// validity is decided from the official lap time, not from this flag.
    #[serde(default)]
    pub count_lap_flag: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, Default)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct Motion {
    /// World position, metres. rFactor family: left-handed, +y up.
    pub pos: [f64; 3],
    /// Velocity in local vehicle axes, m/s (rF family: +x left, +y up, +z back).
    pub local_vel: [f32; 3],
    pub local_accel: [f32; 3],
    /// Rotation rates in local axes, rad/s.
    pub local_rot: [f32; 3],
    pub speed_ms: f32,
    pub speed_kmh: f32,
    /// Lateral g (positive = turning right under ISO convention).
    pub g_lat: f32,
    /// Longitudinal g (positive = acceleration).
    pub g_long: f32,
    pub g_vert: f32,
    /// Yaw rate in degrees per second (positive = turning right).
    pub yaw_rate_dps: f32,
    /// Heading in radians, world frame.
    pub heading_rad: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, Default)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct Engine {
    pub rpm: f32,
    pub max_rpm: f32,
    /// -1 reverse, 0 neutral, 1.. forward.
    pub gear: i8,
    pub max_gears: u8,
    pub water_c: f32,
    pub oil_c: f32,
    pub torque_nm: f32,
    pub turbo_boost: f32,
    pub clutch_rpm: f32,
    /// 0 off, 1 ignition, 2 ignition + starter.
    pub ignition_starter: u8,
    pub speed_limiter: bool,
    pub speed_limiter_available: bool,
    pub anti_stall: bool,
    pub overheating: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, Default)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct Inputs {
    /// Filtered (after TC/ABS/limiter) 0..1.
    pub throttle: f32,
    pub brake: f32,
    /// -1 (left) .. 1 (right).
    pub steering: f32,
    pub clutch: f32,
    /// Raw driver inputs 0..1 / -1..1.
    pub throttle_raw: f32,
    pub brake_raw: f32,
    pub steering_raw: f32,
    pub clutch_raw: f32,
    pub steering_torque_nm: f32,
    /// Steering wheel angle in degrees (derived from raw steering × physical range / 2).
    pub steering_angle_deg: f32,
    /// Physical steering wheel range, degrees lock-to-lock.
    pub wheel_range_deg: f32,
    /// Force feedback output torque reported by the game, if any.
    pub ffb_torque: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, Default)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct Aero {
    pub front_ride_height_mm: f32,
    pub rear_ride_height_mm: f32,
    pub front_wing_height_mm: f32,
    pub front_downforce_n: f32,
    pub rear_downforce_n: f32,
    pub drag_n: f32,
    /// Third element (heave spring) deflection, mm.
    pub third_front_mm: f32,
    pub third_rear_mm: f32,
    pub front_flap_active: bool,
    pub rear_flap_active: bool,
    /// 0 disallowed, 1 criteria met, 2 allowed.
    pub rear_flap_legal: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, Default)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct Energy {
    pub fuel_l: f32,
    pub fuel_capacity_l: f32,
    /// `fuel_l / fuel_capacity_l`.
    pub fuel_fraction: f32,
    /// Virtual energy as reported by the game (LMU: fraction 0..1 — to confirm, may be %).
    pub virtual_energy: Option<f32>,
    /// Hybrid battery state of charge 0..1.
    pub battery_soc: Option<f32>,
    /// Regeneration power, kW (LMU ≥ 1.3).
    pub regen_kw: Option<f32>,
    pub motor_torque_nm: f32,
    pub motor_rpm: f32,
    pub motor_temp_c: f32,
    pub motor_water_c: f32,
    pub motor_state: MotorState,
    pub scheduled_stops: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, Default)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct Electronics {
    pub tc: u8,
    pub tc_max: u8,
    pub tc_slip: u8,
    pub tc_slip_max: u8,
    pub tc_cut: u8,
    pub tc_cut_max: u8,
    pub abs: u8,
    pub abs_max: u8,
    pub motor_map: u8,
    pub motor_map_max: u8,
    pub brake_migration: u8,
    pub brake_migration_max: u8,
    pub arb_front: u8,
    pub arb_front_max: u8,
    pub arb_rear: u8,
    pub arb_rear_max: u8,
    /// Fraction of braking on the rear axle, 0..1.
    pub brake_bias_rear: f32,
    pub tc_active: bool,
    pub abs_active: bool,
    /// 0..255 lift-and-coast progress indicator.
    pub lift_and_coast: u8,
    pub wiper: u8,
    pub headlights: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, Default)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct Wheel {
    pub susp_deflection_m: f32,
    pub ride_height_m: f32,
    pub susp_force_n: f32,
    pub brake_temp_c: f32,
    /// 0..1 for now (game promises kPa in the future).
    pub brake_pressure: f32,
    pub rotation_rads: f32,
    pub camber_deg: f32,
    pub toe_deg: f32,
    pub lateral_patch_vel: f32,
    pub longitudinal_patch_vel: f32,
    pub lateral_ground_vel: f32,
    pub longitudinal_ground_vel: f32,
    pub lateral_force_n: f32,
    pub longitudinal_force_n: f32,
    pub load_n: f32,
    /// Fraction of the contact patch that is sliding, 0..1.
    pub grip_fract: f32,
    pub pressure_kpa: f32,
    /// Surface temperature, °C, order **left / centre / right** of the tyre (not inside/outside).
    pub temp_c: [f32; 3],
    /// Inner rubber layer temperature, °C, same order.
    pub temp_inner_c: [f32; 3],
    pub carcass_c: f32,
    /// Optimal temperature for the compound, °C, when known.
    pub optimal_temp_c: Option<f32>,
    /// Remaining tread fraction 0..1 (1.0 = new). Not linear with grip loss.
    pub wear_remaining: f32,
    pub surface: SurfaceType,
    pub terrain: String,
    pub flat: bool,
    pub detached: bool,
    pub compound_index: u8,
    pub compound_type: CompoundType,
    pub radius_m: f32,
    pub vertical_deflection_m: f32,
    /// Derived: longitudinal slip ratio (positive = wheel spin, negative = lock).
    pub slip_ratio: f32,
    /// Derived: slip angle, degrees.
    pub slip_angle_deg: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, Default)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct Damage {
    /// Dent severity at 8 locations around the car (0 none, 1 some, 2 more).
    pub dents: [u8; 8],
    pub detached_parts: bool,
    pub last_impact_s: f64,
    pub last_impact_magnitude: f32,
    pub last_impact_pos: [f32; 3],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, Default)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct Gaps {
    pub car_ahead_s: Option<f32>,
    pub car_behind_s: Option<f32>,
    pub place_ahead_s: Option<f32>,
    pub place_behind_s: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, Default)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct TyreSet {
    pub front_compound: String,
    pub rear_compound: String,
    pub front_compound_index: u8,
    pub rear_compound_index: u8,
}

/// Everything we know about the player's car, refreshed at telemetry rate (≈100 Hz in LMU).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, Default)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct PlayerTelemetry {
    pub id: i32,
    pub vehicle_name: String,
    pub vehicle_model: String,
    pub vehicle_class: VehicleClass,
    /// Class string as displayed by the game ("Hyper", "LMP2", "GT3"…).
    pub class_name: String,
    pub championship: String,
    pub place: u8,
    pub lap: LapTiming,
    pub motion: Motion,
    pub engine: Engine,
    pub inputs: Inputs,
    pub aero: Aero,
    pub energy: Energy,
    pub electronics: Electronics,
    /// Order: front-left, front-right, rear-left, rear-right.
    pub wheels: [Wheel; 4],
    pub damage: Damage,
    pub gaps: Gaps,
    pub tyres: TyreSet,
    /// Normalised track-limits warning points.
    pub track_limits_steps: u8,
    pub in_pits: bool,
    pub pit_state: PitState,
    pub in_garage: bool,
    /// The player's own finish status (chequered flag taken, DNF…).
    #[serde(default)]
    pub finish_status: FinishStatus,
}

// ---------------------------------------------------------------------------
// Standings (every vehicle, low rate ≈5 Hz)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, Default)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct Standing {
    pub id: i32,
    pub driver_name: String,
    pub vehicle_name: String,
    pub class_name: String,
    pub vehicle_class: VehicleClass,
    pub is_player: bool,
    pub control: Control,
    pub place: u8,
    /// Position within class, derived.
    pub class_place: u8,
    pub laps_completed: i32,
    /// 1..3.
    pub sector: u8,
    pub finish_status: FinishStatus,
    pub lap_dist_m: f32,
    pub lap_fraction: f32,
    pub path_lateral_m: f32,
    pub in_pits: bool,
    pub pit_state: PitState,
    pub in_garage: bool,
    pub num_pitstops: u16,
    pub num_penalties: u16,
    pub time_behind_next_s: f32,
    pub laps_behind_next: i32,
    pub time_behind_leader_s: f32,
    pub laps_behind_leader: i32,
    pub best_lap_s: Option<f32>,
    pub last_lap_s: Option<f32>,
    pub best_s1: Option<f32>,
    pub best_s2: Option<f32>,
    pub last_s1: Option<f32>,
    pub last_s2: Option<f32>,
    pub cur_s1: Option<f32>,
    pub cur_s2: Option<f32>,
    pub time_into_lap_s: f32,
    pub estimated_lap_s: f32,
    pub pos: [f32; 3],
    pub speed_kmh: f32,
    /// 0..1 when known.
    pub fuel_fraction: Option<f32>,
    pub flag_blue: bool,
    pub under_yellow: bool,
    pub headlights: bool,
    pub drs: bool,
    pub qualification: i32,
    pub pit_lap_dist_m: f32,
}

// ---------------------------------------------------------------------------
// Frame & messages
// ---------------------------------------------------------------------------

/// One snapshot of the whole world, as broadcast to dashboards.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, Default)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct TelemetryFrame {
    /// Protocol version.
    pub v: u32,
    /// Monotonic sequence number from the source.
    #[ts(type = "number")]
    pub seq: u64,
    /// Wall-clock timestamp, milliseconds since Unix epoch.
    #[ts(type = "number")]
    pub wall_ms: u64,
    /// Identifier of the source ("lmu-shm", "lmu-shm-linux", "sim", "replay:<file>").
    pub source: String,
    pub session: SessionInfo,
    /// `None` when the player has no vehicle (menus, spectating).
    pub player: Option<PlayerTelemetry>,
    pub standings: Vec<Standing>,
}

/// Per-lap summary emitted when a lap is completed (also what the recorder stores).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct LapRecord {
    /// 0-based, like [`LapTiming::lap_number`]: lap `n` is the game's lap `n + 1`.
    pub lap_number: i32,
    pub lap_time_s: f32,
    pub s1: Option<f32>,
    pub s2: Option<f32>,
    pub s3: Option<f32>,
    pub valid: bool,
    pub in_lap: bool,
    pub out_lap: bool,
    pub fuel_used_l: f32,
    pub virtual_energy_used: Option<f32>,
    pub top_speed_kmh: f32,
    pub front_compound: String,
    pub rear_compound: String,
    /// Average surface temperature per wheel over the lap, °C (FL, FR, RL, RR).
    pub avg_tyre_temp_c: [f32; 4],
    pub avg_tyre_pressure_kpa: [f32; 4],
    pub max_brake_temp_c: [f32; 4],
    pub wear_remaining_end: [f32; 4],
    pub track_c: f32,
    pub ambient_c: f32,
    pub rain: f32,
    pub session_elapsed_end_s: f64,
    /// `true` when `lap_time_s` is the game's official time for this lap.
    /// `false` when the game did not time it (track limits, aborted lap…):
    /// `lap_time_s` is then the measured duration between the two line
    /// crossings and the lap never counts as a best lap.
    #[serde(default = "default_true")]
    pub timed_by_game: bool,
    /// Why the lap is special, if it is. `None` for a normal flying lap.
    #[serde(default)]
    pub note: Option<LapNote>,
}

fn default_true() -> bool {
    true
}

impl Default for LapRecord {
    fn default() -> Self {
        Self {
            lap_number: 0,
            lap_time_s: 0.0,
            s1: None,
            s2: None,
            s3: None,
            valid: false,
            in_lap: false,
            out_lap: false,
            fuel_used_l: 0.0,
            virtual_energy_used: None,
            top_speed_kmh: 0.0,
            front_compound: String::new(),
            rear_compound: String::new(),
            avg_tyre_temp_c: [0.0; 4],
            avg_tyre_pressure_kpa: [0.0; 4],
            max_brake_temp_c: [0.0; 4],
            wear_remaining_end: [0.0; 4],
            track_c: 0.0,
            ambient_c: 0.0,
            rain: 0.0,
            session_elapsed_end_s: 0.0,
            // Same default as the wire format: a lap is timed unless said otherwise.
            timed_by_game: true,
            note: None,
        }
    }
}

/// Why a lap is not a normal flying lap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum LapNote {
    /// Started in the pits / garage.
    OutLap,
    /// Ended in the pit lane.
    InLap,
    /// The game did not time the lap (track limits / cut: shown `--.---` in game).
    NotTimed,
    /// The lap was abandoned (return to garage with Esc): not a full lap.
    Aborted,
    /// First lap of a race (standing or rolling start): timed by the game but
    /// not representative of the pace.
    StartLap,
}

/// Game-level events worth surfacing / recording.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum GameEvent {
    SessionStarted {
        session: SessionInfo,
    },
    SessionEnded,
    EnteredRealtime,
    ExitedRealtime,
    PitEntry {
        lap_number: i32,
    },
    PitExit {
        lap_number: i32,
    },
    Impact {
        magnitude: f32,
        lap_number: i32,
    },
    FlagChanged {
        phase: GamePhase,
        yellow: YellowFlagState,
    },
    /// The recorder opened a session on disk (`GET /api/sessions/{session_id}`).
    RecordingStarted {
        session_id: String,
    },
    /// The recorder closed the session: `laps` stored, `samples` rows written.
    RecordingStopped {
        session_id: String,
        laps: u32,
        #[ts(type = "number")]
        samples: u64,
    },
}

/// Health of the acquisition pipeline, pushed on change and every few seconds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, Default)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct SourceStatus {
    /// `true` when the game's shared memory is open and updating.
    pub connected: bool,
    pub game: GameId,
    pub source: String,
    /// Measured input rate from the game, Hz.
    pub input_rate_hz: f32,
    /// Broadcast rate to this client, Hz.
    pub output_rate_hz: f32,
    pub in_realtime: bool,
    pub game_version: Option<String>,
    pub last_error: Option<String>,
    /// Number of dashboards currently connected.
    pub clients: u32,
    pub recording: bool,
}

/// What a [`RaceReminder`] announces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum RaceReminderKind {
    /// `lead_min` minutes before the start of a race (0 = at the start).
    Lead,
    /// The evening before a special event (18:00, local time of the device that chose it).
    Eve,
    /// The day of a special event.
    Day,
}

/// An online race the driver picked on the « Courses » page is about to start (or, for a special
/// event, is tomorrow / today). Sent once to every dashboard when it is due; the UI shows a banner
/// and plays a short sound.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct RaceReminder {
    /// `<race_id>@<start_ms>`.
    pub pick_id: String,
    pub race_id: String,
    pub name: String,
    pub track: String,
    /// `daily` | `weekly` | `special` | `champ`.
    pub cat: String,
    /// `beginner` | `intermediate` | `advanced` (daily races).
    pub level: Option<String>,
    /// Start of the race (ms since the epoch); for a special event, the reminder of the day.
    #[ts(type = "number")]
    pub start_ms: i64,
    pub kind: RaceReminderKind,
    /// Minutes before the start, for [`RaceReminderKind::Lead`].
    pub lead_min: Option<u32>,
    /// When the reminder was due (ms since the epoch).
    #[ts(type = "number")]
    pub due_ms: i64,
}

/// Messages from the server to a dashboard.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[allow(clippy::large_enum_variant)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum ServerMessage {
    Hello {
        server_version: String,
        protocol_version: u32,
        status: SourceStatus,
    },
    Status(SourceStatus),
    Frame(TelemetryFrame),
    Lap(LapRecord),
    Event(GameEvent),
    Pong {
        #[ts(type = "number")]
        t: u64,
    },
    /// A picked online race is due (page « Courses »).
    RaceReminder(RaceReminder),
}

/// Messages from a dashboard to the server.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum ClientMessage {
    /// Ask for a given broadcast rate (1..=60 Hz).
    SetRate { hz: u8 },
    Ping {
        #[ts(type = "number")]
        t: u64,
    },
}

// ---------------------------------------------------------------------------
// Dashboard layouts (persisted by the server)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, Default)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum DeviceKind {
    #[default]
    Desktop,
    Mobile,
    Overlay,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct WidgetPlacement {
    /// Unique instance id.
    pub id: String,
    /// Widget type, e.g. "tyres", "inputs", "laps", "fuel", "map", "session", "standings", "brakes".
    pub kind: String,
    pub x: u16,
    pub y: u16,
    pub w: u16,
    pub h: u16,
    /// Widget-specific settings.
    #[ts(type = "Record<string, unknown>")]
    pub config: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct DashboardLayout {
    pub id: String,
    pub name: String,
    pub device: DeviceKind,
    /// Grid columns (12 desktop, 4 mobile by default).
    pub columns: u8,
    pub widgets: Vec<WidgetPlacement>,
    /// Unix ms.
    #[ts(type = "number")]
    pub updated_ms: u64,
}

// ---------------------------------------------------------------------------
// Post-session analysis (produced by `corde-engineer`, served by
// `GET /api/sessions/{id}/analysis`)
// ---------------------------------------------------------------------------

/// Phase of a corner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum CornerPhase {
    Entry,
    Mid,
    Exit,
}

/// Corner class by minimum speed (thresholds in `corde-engineer`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum CornerSpeed {
    Slow,
    Medium,
    Fast,
}

/// Which way a corner turns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, Default)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum TurnDirection {
    Left,
    Right,
    #[default]
    Unknown,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, TS, Default,
)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum Severity {
    #[default]
    Info,
    Minor,
    Major,
}

/// Camber reading from the inner / outer temperature spread of a tyre.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, Default)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum CamberHint {
    TooMuch,
    TooLittle,
    Ok,
    #[default]
    Unknown,
}

/// Where a brake disc sits relative to its thermal window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, Default)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum BrakeWindow {
    Cold,
    Ok,
    Hot,
    #[default]
    Unknown,
}

/// The three setup orientations Corde proposes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum SetupPhilosophy {
    Aggressive,
    Safe,
    Wet,
}

/// Direction of a relative setup change (the sign convention of each parameter
/// is the one of `knowledge/setup-rules.yaml` → `params[].sign`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum ChangeDirection {
    Increase,
    Decrease,
    /// Reduce the absolute value (toe toward zero).
    TowardZero,
    /// Discrete value given by `magnitude` (compound "wet", regen "max"…).
    Set,
}

/// What the analysis could rely on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, Default)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct DataQuality {
    /// Recording rate of the samples on disk.
    pub sample_rate_hz: f32,
    /// Laps kept for the analysis (valid, not in/out-lap, enough samples).
    pub valid_laps: u32,
    pub total_laps: u32,
    /// Slip ratio / slip angle channels present and not flat.
    pub has_slip: bool,
    pub has_ride_height: bool,
    /// Optimal tyre temperature known (needed for the cold/hot findings).
    pub has_optimal_temp: bool,
    /// Rain or wet track during the analysed laps.
    pub wet_session: bool,
    /// Human notes (French): missing channels, few laps, assumptions.
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, Default)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct LapStats {
    pub best_s: Option<f32>,
    pub mean_valid_s: Option<f32>,
    pub std_valid_s: Option<f32>,
    /// 0 (erratic) .. 100 (metronome).
    pub consistency_score: f32,
    /// Slope of the valid lap time against the lap number, s/lap (≥ 4 laps).
    pub degradation_s_per_lap: Option<f32>,
    pub fuel_per_lap_l: Option<f32>,
    /// Virtual energy per lap, in the game's unit (fraction or %).
    pub energy_per_lap: Option<f32>,
    pub top_speed_kmh: f32,
}

/// Driving-style indicators, all 0..100 unless stated.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, Default)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct DrivingStyle {
    /// Brake attack: rise time 0→80 % and peak pressure.
    pub brake_aggression: f32,
    /// Share of corner entries driven with brake and lateral g together.
    pub trail_braking: f32,
    pub throttle_smoothness: f32,
    pub steering_smoothness: f32,
    /// Share of the lap spent on kerbs (rumble strips).
    pub kerb_usage: f32,
    /// Wheel lock-ups per lap (all wheels).
    pub lockup_rate: f32,
    /// Wheel-spin events per lap (all wheels).
    pub wheelspin_rate: f32,
    /// Share of the lap with neither throttle nor brake above 60 km/h, %.
    pub coasting_pct: f32,
    /// Short French labels ("freinage agressif", "trail braking marqué"…).
    pub labels_fr: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, Default)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct TyreWheelAnalysis {
    pub avg_surface_c: f32,
    pub avg_carcass_c: f32,
    pub optimal_c: Option<f32>,
    /// `avg_carcass_c − optimal_c`.
    pub delta_to_optimal_c: Option<f32>,
    /// Inner / middle / outer shoulder temperatures under lateral load, °C
    /// (already remapped from the game's left / centre / right order).
    pub inner_c: f32,
    pub middle_c: f32,
    pub outer_c: f32,
    /// `inner_c − outer_c`.
    pub imo_spread_c: f32,
    pub camber_hint: CamberHint,
    pub avg_pressure_kpa: f32,
    /// Pressure at the end of the stint minus at the start, kPa.
    pub pressure_drift_kpa: f32,
    /// Tread lost per lap, percentage points.
    pub wear_per_lap_pct: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, Default)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct TyreAnalysis {
    /// FL, FR, RL, RR.
    pub per_wheel: [TyreWheelAnalysis; 4],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, Default)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct BrakeWheelAnalysis {
    pub avg_c: f32,
    /// Highest temperature over the analysed laps.
    pub max_c: f32,
    /// Typical peak: median over the analysed laps of each lap's highest
    /// temperature. `window` is judged on it (one long stop does not make the
    /// brakes "hot"). 0 in reports written before it existed.
    #[serde(default)]
    pub typical_peak_c: f32,
    pub window: BrakeWindow,
    /// Lock-up events over the analysed laps.
    pub lockups: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, Default)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct BrakeAnalysis {
    /// FL, FR, RL, RR.
    pub per_wheel: [BrakeWheelAnalysis; 4],
    /// Average rear brake bias, 0..1.
    pub bias_rear_avg: Option<f32>,
    /// Share of the lock-ups on the front axle, 0..1 (`None` without lock-ups).
    pub lockup_front_vs_rear: Option<f32>,
}

/// One corner of the track, matched across laps by distance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, Default)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct CornerStat {
    /// 1-based, in track order.
    pub index: u32,
    pub start_dist_m: f32,
    pub end_dist_m: f32,
    pub apex_dist_m: f32,
    pub direction: TurnDirection,
    pub speed_class: Option<CornerSpeed>,
    pub min_speed_kmh: f32,
    pub max_g_lat: f32,
    /// Laps in which the corner was found.
    pub passes: u32,
    /// Understeer index per phase, relative to the session reference like
    /// [`BalanceCell::understeer_index`] (−1 more oversteer … +1 more
    /// understeer than usual), `None` when the phase had no usable sample.
    pub balance_entry: Option<f32>,
    pub balance_mid: Option<f32>,
    pub balance_exit: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, Default)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct BalanceCell {
    /// Balance **relative to the session reference** of this car
    /// ([`BalanceAnalysis::reference`]): −1 much more oversteer … +1 much more
    /// understeer than usual in this session (±1 = 5 robust standard
    /// deviations, see `docs/ENGINEER.md` § 3); 0 when `samples == 0`.
    /// Reports written before the reference existed hold the absolute index
    /// here (`reference` is then absent).
    pub understeer_index: f32,
    /// Absolute index (fixed slip-angle / gradient scales, before the session
    /// reference is taken out), kept for comparison: it is not comparable from
    /// one car to another.
    #[serde(default)]
    pub raw_index: f32,
    pub samples: u32,
    /// 0..1: coverage (laps, passes) × estimator agreement × channels.
    pub confidence: f32,
}

/// The usual balance of this car in this session, what every cell of the
/// matrix is compared with: robust median and spread (1.4826 × MAD, with a
/// floor) of each estimator over every (corner, phase, lap) pass.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, Default)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct BalanceReference {
    /// Median of the absolute index over the passes (what the engine reported
    /// as the balance before the reference existed).
    pub raw_index: f32,
    /// Slip-angle balance `(|α_front| − |α_rear|) / (|α_front| + |α_rear|)`:
    /// session median and spread (`None` without slip-angle channels).
    pub slip_balance: Option<f32>,
    pub slip_balance_sigma: Option<f32>,
    /// Kinematic understeer gradient, °/g: session median and spread (`None`
    /// without a live steering angle).
    pub gradient_deg_per_g: Option<f32>,
    pub gradient_sigma_deg_per_g: Option<f32>,
    /// (corner, phase, lap) passes the reference was taken on.
    pub passes: u32,
    /// Relative index beyond which a cell becomes a finding (0.3 = 1.5 σ).
    pub trigger_index: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, Default)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct BalanceRow {
    pub slow: BalanceCell,
    pub medium: BalanceCell,
    pub fast: BalanceCell,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, Default)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct BalanceMatrix {
    pub entry: BalanceRow,
    pub mid: BalanceRow,
    pub exit: BalanceRow,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, Default)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct BalanceAnalysis {
    pub corners: Vec<CornerStat>,
    /// Phase × speed matrix, relative to `reference`.
    pub per_phase: BalanceMatrix,
    /// The session reference the matrix and the corners are relative to
    /// (`None` in reports written before it existed, or with too few passes).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub reference: Option<BalanceReference>,
    /// One-line French reading of the matrix.
    pub overall_fr: String,
}

/// Damper-velocity histogram of one wheel, % of the analysed time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, Default)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct DamperHist {
    /// Compression below the slow/fast threshold (25 mm/s by default).
    pub slow_bump_pct: f32,
    pub slow_rebound_pct: f32,
    pub fast_bump_pct: f32,
    pub fast_rebound_pct: f32,
    /// `slow_bump_pct − slow_rebound_pct` (target |skew| < 5 points).
    pub skew_slow: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, Default)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct SuspensionAnalysis {
    pub bottoming_events_per_lap: f32,
    /// Minimum ride height seen, mm: front, rear.
    pub min_ride_height_mm: [f32; 2],
    /// Share of the time at ≥ 98 % of the observed travel, % per wheel.
    pub travel_saturation_pct: [f32; 4],
    pub damper_velocity_hist: [DamperHist; 4],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, Default)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct Evidence {
    pub label: String,
    pub value: f32,
    pub unit: String,
}

/// One diagnostic that fired, with its numbers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, Default)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct Finding {
    /// Unique within the report (`D-BRK-03/front`).
    pub id: String,
    /// `knowledge/diagnostics.yaml` id (`D-BRK-03`, `S-01`…).
    pub diagnostic_id: String,
    /// Symptoms of `knowledge/setup-rules.yaml` this finding triggers.
    pub symptoms: Vec<String>,
    pub severity: Severity,
    /// 0..1.
    pub confidence: f32,
    pub phase: Option<CornerPhase>,
    pub corner_speed: Option<CornerSpeed>,
    pub title_fr: String,
    pub title_en: String,
    pub detail_fr: String,
    pub detail_en: String,
    pub evidence: Vec<Evidence>,
    /// Laps where the symptom was seen (empty = whole session).
    pub lap_numbers: Vec<i32>,
}

/// One relative setup change.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct SetupChange {
    /// Logical parameter id of `knowledge/setup-parameters.yaml` (`aero.rear_wing`).
    pub parameter_id: String,
    /// `.svm` key when known (`RWSetting`), else the logical id.
    pub parameter_key: String,
    /// `.svm` section (`REARWING`, `FRONTLEFT / FRONTRIGHT`…) or "".
    pub section: String,
    pub label_fr: String,
    pub direction: ChangeDirection,
    /// Relative magnitude as written in the rule ("1 cran", "3 kPa", "0.5–1 %").
    pub magnitude: String,
    /// Whole number of engineer steps parsed from `magnitude`, when it is one.
    pub steps: Option<i32>,
    /// 1 (first thing to try) .. 5.
    pub priority: u8,
    pub rule_ids: Vec<String>,
    pub rationale_fr: String,
    pub rationale_en: String,
    pub side_effects_fr: Vec<String>,
    /// Rule ids whose changes oppose this one.
    pub conflicts_with: Vec<String>,
    /// 0..1, from the rule and the findings behind it.
    pub confidence: f32,
    /// The change applied to the real setup (keys, wheels, index before →
    /// after, labels), when the plan was resolved against a `.svm` file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub resolved: Option<ResolvedChange>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct SetupPlan {
    pub philosophy: SetupPhilosophy,
    pub intent_fr: String,
    pub changes: Vec<SetupChange>,
    pub warnings_fr: Vec<String>,
    /// Changes left out once the plan was resolved against the real setup
    /// (parameter not adjustable on this car, key absent…), each with its
    /// `resolved.status` and notes. Empty when the plan is not resolved.
    #[serde(default)]
    pub dropped: Vec<SetupChange>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct SetupPlans {
    /// `true` when the plans were resolved against a real setup (`.svm` file
    /// of the driver's library, see `base`): every change then carries its
    /// `resolved` index before → after.
    pub current_setup_known: bool,
    /// The setup file the plans were resolved against.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub base: Option<SetupFileInfo>,
    pub aggressive: SetupPlan,
    pub safe: SetupPlan,
    pub wet: SetupPlan,
}

/// The post-session report of the race engineer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct AnalysisReport {
    /// Version of the analysis engine that wrote the report (0 before it was
    /// recorded); the server recomputes a cached report from an older engine.
    #[serde(default)]
    pub engine_version: u32,
    pub session_id: String,
    /// Unix ms.
    #[ts(type = "number")]
    pub generated_ms: u64,
    pub track_name: String,
    pub vehicle: String,
    pub vehicle_class: VehicleClass,
    pub data_quality: DataQuality,
    pub laps_analysed: Vec<i32>,
    pub lap_stats: LapStats,
    pub driving_style: DrivingStyle,
    pub tyres: TyreAnalysis,
    pub brakes: BrakeAnalysis,
    pub balance: BalanceAnalysis,
    pub suspension: SuspensionAnalysis,
    pub findings: Vec<Finding>,
    pub plans: SetupPlans,
    pub summary_fr: String,
    pub summary_en: String,
}

// ---------------------------------------------------------------------------
// Driver feedback: stored, interpreted into symptoms, and turned into plan
// iterations (`corde-engineer::feedback` / `::iterate`)
// ---------------------------------------------------------------------------

/// What the driver felt in a session, in their own words
/// (`POST /api/sessions/{id}/feedback`). `plan` is the orientation they
/// tried, when it was one of the engineer's plans.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, Default)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct NewFeedback {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub plan: Option<SetupPhilosophy>,
    /// 1..=2000 characters once trimmed.
    pub text: String,
}

/// A stored [`NewFeedback`] (`GET /api/sessions/{id}/feedback`), with the
/// engineer's reading of it when it has been interpreted.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct Feedback {
    #[ts(type = "number")]
    pub id: i64,
    pub session_id: String,
    /// Unix ms.
    #[ts(type = "number")]
    pub created_ms: i64,
    pub plan: Option<SetupPhilosophy>,
    pub text: String,
    /// Symptoms read from `text` (`None` for feedback stored before the
    /// interpreter existed).
    #[serde(default)]
    pub interpretation: Option<FeedbackInterpretation>,
}

/// Which end of the car a symptom concerns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum Axle {
    Front,
    Rear,
}

/// How strongly the driver worded a symptom ("un peu" / "beaucoup" / "trop").
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum SymptomIntensity {
    Mild,
    Marked,
    Severe,
}

/// What produced a [`FeedbackInterpretation`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS, Default)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum ExtractorKind {
    /// `knowledge/feedback-dictionary.yaml` + keyword / regex matching (offline, default).
    #[default]
    Dictionary,
    /// A language model (Anthropic Messages API), opt-in.
    Llm,
}

/// One symptom read from the driver's text.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, Default)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct DetectedSymptom {
    /// A `symptom` of `knowledge/setup-rules.yaml` (`US_ENTRY_SLOW`,
    /// `BRK_LOCK_FRONT`…) or a generic family when the text does not say
    /// more (`US_ENTRY`, `US`, `OS_EXIT`, `BRK_LOCK`, `TRC_WHEELSPIN`,
    /// `TYR_HOT`, `UNSTABLE`, `NO_GRIP`, `JUMPS`).
    pub symptom_id: String,
    pub label_fr: String,
    pub label_en: String,
    /// Braking counts as entry.
    pub phase: Option<CornerPhase>,
    pub corner_speed: Option<CornerSpeed>,
    pub axle: Option<Axle>,
    /// Rain / wet track mentioned with the symptom.
    pub wet: bool,
    pub intensity: Option<SymptomIntensity>,
    /// "pas de sous-virage", "ne glisse plus": the driver says the symptom is absent.
    pub negated: bool,
    /// The words of the driver's text that matched.
    pub matched_text: String,
    /// 0..1.
    pub confidence: f32,
}

/// The engineer's reading of one feedback: symptoms, what to ask back, and
/// what could not be understood.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, Default)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct FeedbackInterpretation {
    /// `Feedback::id` (0 before the feedback is stored).
    #[ts(type = "number")]
    pub feedback_id: i64,
    pub symptoms: Vec<DetectedSymptom>,
    /// Clarification questions, French.
    pub questions_fr: Vec<String>,
    pub questions_en: Vec<String>,
    /// Pieces of the text no pattern recognised.
    pub unmatched_fragments: Vec<String>,
    pub extractor: ExtractorKind,
    /// 0..1: how much of the text was understood, and how surely.
    pub confidence: f32,
    /// French remarks (e.g. the language model was unavailable).
    #[serde(default)]
    pub notes_fr: Vec<String>,
}

/// One turn of the "feedback → interpretation → adjusted plan" loop
/// (`GET /api/sessions/{id}/iterations`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct PlanIteration {
    #[ts(type = "number")]
    pub id: i64,
    pub session_id: String,
    /// Unix ms.
    #[ts(type = "number")]
    pub created_ms: i64,
    /// 1 for the first iteration on this session and plan.
    pub iteration: u32,
    /// The plan the loop started from.
    pub base_plan: SetupPhilosophy,
    #[ts(type = "number | null")]
    pub feedback_id: Option<i64>,
    pub interpretation: Option<FeedbackInterpretation>,
    /// `Finding::id`s of the latest analysis that took part in the merge.
    pub data_findings_used: Vec<String>,
    /// The whole cumulative plan after this iteration.
    pub changes: Vec<SetupChange>,
    /// What this iteration adds or reverses compared to the previous one.
    pub delta: Vec<SetupChange>,
    /// Where the data and the feeling say the same thing.
    pub agreements_fr: Vec<String>,
    /// Where they contradict each other, and which side was followed.
    pub disagreements_fr: Vec<String>,
    pub notes_fr: Vec<String>,
    /// `true` when `changes` / `delta` were resolved against a real setup
    /// (not stored: computed when the iteration is served).
    #[serde(default)]
    pub current_setup_known: bool,
    /// Changes of the cumulative plan left out by the resolution (not
    /// adjustable on this car…), with their reason.
    #[serde(default)]
    pub dropped: Vec<SetupChange>,
}

// ---------------------------------------------------------------------------
// Real setups: the driver's `.svm` files, plans resolved against one of them,
// and the files Corde writes (`GET /api/setups`, `…/setup-preview`,
// `POST …/setup-files`)
// ---------------------------------------------------------------------------

/// One `.svm` file of the driver's library
/// (`UserData/player/Settings/<folder>/<file>`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, Default)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct SetupFileInfo {
    /// Track folder as the game names it (`Longbeach`, `Road Atlanta`).
    pub folder: String,
    /// File name with its extension (`BMW_LongBeach.svm`).
    pub file: String,
    /// `folder/file`: what the API takes back as `base`.
    pub path: String,
    /// `VehicleClassSetting` of the file (`BMW_M4_LMGT3 GT3 WEC2026`).
    pub vehicle: String,
    /// Last modification, Unix ms.
    #[ts(type = "number")]
    pub mtime_ms: i64,
    /// `Notes="…"` of the file.
    pub notes: String,
}

/// What happened to one change once applied to the real setup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS, Default)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum ResolveStatus {
    /// Applied as asked (within the known bounds).
    #[default]
    Applied,
    /// Stopped at the minimum index (0 or the game's minimum).
    ClampedMin,
    /// Stopped at the maximum the game gave.
    ClampedMax,
    /// The game marks the parameter `Non-adjustable` / `N/A` / `Fixed` /
    /// `Détaché` on this car: left out.
    NotAdjustable,
    /// No such key in the file (or no `.svm` key for this parameter): left out.
    UnknownKey,
    /// Applied, but the maximum is unknown: at most 2 steps up, to check in
    /// the garage.
    UnknownBounds,
    /// Cannot be computed (discrete value such as the compound, unknown sign):
    /// to do by hand.
    Manual,
    /// Already covered by another change of the plan on the same key.
    Duplicate,
}

/// One key of the file touched by a change (one per wheel for a per-wheel
/// parameter).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, Default)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct ResolvedValue {
    /// `.svm` section (`SUSPENSION`, `FRONTLEFT`…).
    pub section: String,
    /// `.svm` key (`RearAntiSwaySetting`).
    pub key: String,
    /// `FL` / `FR` / `RL` / `RR` for a per-wheel key.
    pub wheel: Option<String>,
    pub index_before: i32,
    pub index_after: i32,
    /// Label after `//` in the base file (as written there).
    pub label_before: String,
    /// Label for the new index when Corde can tell it (seen in another file
    /// of the same car, or interpolated from two numeric labels).
    pub label_after: Option<String>,
    /// Bounds of the index, when known (minimum 0 when the game says nothing).
    pub min: Option<i32>,
    pub max: Option<i32>,
    pub status: ResolveStatus,
}

/// A [`SetupChange`] applied to a real setup.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, Default)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct ResolvedChange {
    /// Worst status of `values` (or why there is none).
    pub status: ResolveStatus,
    /// Index steps asked for (signed, in index direction) before clamping.
    pub steps: i32,
    pub values: Vec<ResolvedValue>,
    /// How the magnitude became steps, limits met, assumptions.
    pub notes_fr: Vec<String>,
    pub notes_en: Vec<String>,
}

/// `GET /api/setups?session=<id>`: the driver's library and the base
/// proposed for the session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, Default)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct SetupLibrary {
    /// `false` while the game has never been seen (its `UserData` folder is
    /// unknown): nothing can be read or written.
    pub user_data_known: bool,
    /// `…/UserData/player/Settings` on this PC.
    pub settings_dir: Option<String>,
    /// Car of the session (`vehicle_model`).
    pub vehicle: String,
    pub track_name: String,
    /// Setups folder matching the session's track.
    pub track_folder: Option<String>,
    /// The base Corde proposes (also what the plans are resolved against by
    /// default).
    pub proposed: Option<SetupFileInfo>,
    /// Why this one, in French.
    pub proposed_reason_fr: Option<String>,
    /// Files for the session's car (every file without `session`), track
    /// folder first, then most recent first.
    pub candidates: Vec<SetupFileInfo>,
    /// Number of `.svm` files in the library, all cars.
    pub total_files: u32,
    /// `folder/file` of the files that could not be read.
    pub unreadable: Vec<String>,
    /// `true` when the game's garage gave the bounds of this car.
    pub bounds_known: bool,
}

/// `GET /api/sessions/{id}/setup-preview?base=&plan=`: a plan resolved
/// against a base file, and the file Corde would write (nothing written).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct SetupPreview {
    pub base: SetupFileInfo,
    /// `aggressive` | `safe` | `wet` | `iteration:<n>`.
    pub plan: String,
    /// `Agressif`, `Soft & sûr`, `Pluie`, `Itération 2 (Soft & sûr)`.
    pub plan_label_fr: String,
    /// Name the file would get (`Corde_Safe_BMW_LongBeach.svm`, before the
    /// `_2`, `_3`… suffix).
    pub file_name: String,
    /// `Notes="…"` it would carry.
    pub notes: String,
    pub changes: Vec<SetupChange>,
    pub dropped: Vec<SetupChange>,
    pub warnings_fr: Vec<String>,
    /// Number of `Key=value` lines that would change.
    pub changed_lines: u32,
}

/// `POST /api/sessions/{id}/setup-files` body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, Default)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct NewSetupFiles {
    /// `folder/file` of the base, as listed by `GET /api/setups`.
    pub base: String,
    /// Plans to write: `aggressive`, `safe`, `wet`, `iteration:<n>`.
    pub plans: Vec<String>,
}

/// One file written by `POST …/setup-files`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, Default)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct CreatedSetupFile {
    pub plan: String,
    pub plan_label_fr: String,
    /// `folder/file` of the new file.
    pub path: String,
    pub file: String,
    /// Number of `Key=value` lines changed from the base.
    pub changed_lines: u32,
}

/// `POST /api/sessions/{id}/setup-files` answer (201).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, Default)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct SetupFilesCreated {
    pub folder: String,
    pub files: Vec<CreatedSetupFile>,
    /// What to do in the game, French / English.
    pub instructions_fr: String,
    pub instructions_en: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_roundtrip_json() {
        let f = TelemetryFrame {
            v: PROTOCOL_VERSION,
            seq: 1,
            source: "test".into(),
            player: Some(PlayerTelemetry::default()),
            ..Default::default()
        };
        let s = serde_json::to_string(&f).unwrap();
        let back: TelemetryFrame = serde_json::from_str(&s).unwrap();
        assert_eq!(f, back);
    }

    #[test]
    fn server_message_is_tagged() {
        let m = ServerMessage::Pong { t: 42 };
        let s = serde_json::to_string(&m).unwrap();
        assert_eq!(s, r#"{"type":"pong","t":42}"#);
    }

    #[test]
    fn race_reminder_is_flat_and_tagged() {
        let m = ServerMessage::RaceReminder(RaceReminder {
            pick_id: "one-stint-sprint-advanced@1762892700000".into(),
            race_id: "one-stint-sprint-advanced".into(),
            name: "One Stint Sprint".into(),
            track: "Monza".into(),
            cat: "daily".into(),
            level: Some("advanced".into()),
            start_ms: 1_762_892_700_000,
            kind: RaceReminderKind::Lead,
            lead_min: Some(5),
            due_ms: 1_762_892_400_000,
        });
        let v: serde_json::Value = serde_json::to_value(&m).unwrap();
        assert_eq!(v["type"], "race_reminder");
        assert_eq!(v["kind"], "lead");
        assert_eq!(v["lead_min"], 5);
        assert_eq!(v["start_ms"], 1_762_892_700_000_i64);
        let back: ServerMessage = serde_json::from_value(v).unwrap();
        assert_eq!(back, m);
    }
}
