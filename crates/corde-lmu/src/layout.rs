//! Binary layout of the native LMU shared memory block (`LMU_Data`).
//!
//! These structures are **re-written from a reading of the official Studio 397
//! header** (`Support/SharedMemoryInterface/*.hpp`, not redistributable) and
//! verified against `reference/lmu-shared-memory-layout.txt` (offsets computed
//! with clang in MSVC x64 ABI). Every size and offset is covered by tests
//! (`tests/layout_offsets.rs`, generated from the reference file).
//!
//! ABI rules reproduced here:
//! * `#pragma pack(push, 4)` applies to the structs of `InternalsPlugin.hpp`
//!   (`TelemVect3`, `TelemWheelV01`, `TelemInfoV01`, `VehicleScoringInfoV01`,
//!   `ScoringInfoV01`, `ApplicationStateV01`) → `#[repr(C, packed(4))]`.
//! * The envelope structs of `SharedMemoryInterface.hpp` use the default MSVC
//!   alignment (8) → plain `#[repr(C)]` with explicit padding fields so that
//!   `bytemuck::Pod` can be derived (no implicit padding).
//! * MSVC x64: `long` = `i32`, `unsigned long` = `u32`, `size_t` = `u64`,
//!   pointers (`char*`, `HWND`, `VehicleScoringInfoV01*`) = 8 bytes (stored as
//!   `u64`, meaningless outside the game process), `bool` = 1 byte (stored as
//!   `u8`, see [`as_bool`]), `float_t` = `f32`.
//!
//! Field names keep the header's names in snake_case without the `m` prefix
//! (`mEngineRPM` → `engine_rpm`) so that the generated offset tests can map
//! them mechanically.

#![allow(clippy::too_many_arguments)]

use bytemuck::{Pod, Zeroable};

/// Maximum number of vehicles in the native interface (vs 128 in the legacy plugin).
pub const MAX_VEHICLES: usize = 104;
/// Size of the results stream buffer.
pub const SCORING_STREAM_LEN: usize = 65536;
/// Win32 `MAX_PATH`.
pub const MAX_PATH: usize = 260;

/// Number of useful bytes in the block (`sizeof(SharedMemoryObjectOut)` minus
/// the 4 trailing padding bytes). A mapping of at least this size is accepted.
pub const SHM_USEFUL_SIZE: usize = 324_820;
/// `sizeof(SharedMemoryObjectOut)` / `sizeof(SharedMemoryLayout)`.
pub const SHM_TOTAL_SIZE: usize = 324_824;

/// Win32 names (Windows) — on Linux the bridge mirrors the mapping to
/// `/dev/shm/<name>`.
pub const SHM_FILE_NAME: &str = "LMU_Data";
pub const SHM_HOLD_EVENT_NAME: &str = "LMU_Data_HoldEvent";
pub const SHM_DATA_EVENT_NAME: &str = "LMU_Data_DataEvent";
pub const SHM_LOCK_DATA_NAME: &str = "LMU_SharedMemoryLockData";
pub const SHM_LOCK_EVENT_NAME: &str = "LMU_SharedMemoryLockEvent";

// ---------------------------------------------------------------------------
// Offsets of the blocks inside `SharedMemoryObjectOut` (see reference file).
// ---------------------------------------------------------------------------

pub const OFF_GENERIC: usize = 0;
pub const OFF_PATHS: usize = 332;
pub const OFF_SCORING: usize = 1632;
pub const OFF_SCORING_INFO: usize = OFF_SCORING;
pub const OFF_SCORING_STREAM_SIZE: usize = OFF_SCORING + 552;
pub const OFF_VEH_SCORING_INFO: usize = OFF_SCORING + 560;
pub const OFF_SCORING_STREAM: usize = OFF_SCORING + 61_296;
pub const OFF_TELEMETRY: usize = 128_464;
pub const OFF_ACTIVE_VEHICLES: usize = OFF_TELEMETRY;
pub const OFF_PLAYER_VEHICLE_IDX: usize = OFF_TELEMETRY + 1;
pub const OFF_PLAYER_HAS_VEHICLE: usize = OFF_TELEMETRY + 2;
pub const OFF_TELEM_INFO: usize = OFF_TELEMETRY + 4;

/// `ScoringInfoV01::mCurrentET` inside the whole block (witness for torn reads).
pub const OFF_CURRENT_ET: usize = OFF_SCORING_INFO + 68;
/// `TelemInfoV01::mElapsedTime` relative to the start of a `TelemInfoV01`.
pub const OFF_TELEM_ELAPSED_TIME: usize = 12;

pub const SIZE_TELEM_VECT3: usize = 24;
pub const SIZE_TELEM_WHEEL: usize = 260;
pub const SIZE_TELEM_INFO: usize = 1888;
pub const SIZE_VEHICLE_SCORING_INFO: usize = 584;
pub const SIZE_SCORING_INFO: usize = 548;
pub const SIZE_APPLICATION_STATE: usize = 260;
pub const SIZE_GENERIC: usize = 332;
pub const SIZE_PATHS: usize = 1300;
pub const SIZE_SCORING_DATA: usize = 126_832;
pub const SIZE_TELEMETRY_DATA: usize = 196_356;

/// Byte offset of `telemInfo[idx].mElapsedTime` inside the whole block.
#[inline]
pub const fn telem_elapsed_time_offset(idx: usize) -> usize {
    OFF_TELEM_INFO + idx * SIZE_TELEM_INFO + OFF_TELEM_ELAPSED_TIME
}

// ---------------------------------------------------------------------------
// Enumerations from the header (kept as plain integers in the structs).
// ---------------------------------------------------------------------------

/// `SharedMemoryEvent` (uint32) — values found in `SharedMemoryGeneric::events`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum SharedMemoryEvent {
    Enter = 0,
    Exit = 1,
    Startup = 2,
    Shutdown = 3,
    Load = 4,
    Unload = 5,
    StartSession = 6,
    EndSession = 7,
    EnterRealtime = 8,
    ExitRealtime = 9,
    UpdateScoring = 10,
    UpdateTelemetry = 11,
    InitApplication = 12,
    UninitApplication = 13,
    SetEnvironment = 14,
    Ffb = 15,
}

pub const SME_MAX: usize = 16;

/// `IP_VehicleClass` (uint8) as found in `TelemInfoV01::vehicle_class`.
pub mod vehicle_class {
    pub const HYPERCAR: u8 = 0x00;
    pub const LMP2_ELMS: u8 = 0x02;
    pub const LMP2: u8 = 0x03;
    pub const LMP3: u8 = 0x04;
    pub const GTE: u8 = 0x05;
    pub const GT3: u8 = 0x06;
    pub const PACE_CAR: u8 = 0x08;
    pub const UNKNOWN: u8 = 0xFF;
}

/// `IP_VehicleChampionship` (uint8).
pub mod championship {
    pub const WEC_2023: u8 = 0x00;
    pub const WEC_2024: u8 = 0x01;
    pub const WEC_2025: u8 = 0x02;
    pub const WEC_2026: u8 = 0x03;
    pub const ELMS_2025: u8 = 0x10;
    pub const ELMS_2026: u8 = 0x11;
    pub const UNKNOWN: u8 = 0xFF;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// C `bool` stored as a byte: anything non-zero is `true`.
#[inline]
pub fn as_bool(b: u8) -> bool {
    b != 0
}

/// Decode a fixed `char[N]` buffer: ASCII/Latin-1, stopped at the first NUL,
/// non-UTF-8 bytes replaced (lossy). Trailing whitespace is trimmed.
pub fn cstr(buf: &[u8]) -> String {
    let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    String::from_utf8_lossy(&buf[..end]).trim_end().to_string()
}

/// Write `s` into a fixed `char[N]` buffer (truncated, always NUL-terminated).
pub fn set_cstr(buf: &mut [u8], s: &str) {
    if buf.is_empty() {
        return;
    }
    let n = s.len().min(buf.len() - 1);
    buf[..n].copy_from_slice(&s.as_bytes()[..n]);
    for b in &mut buf[n..] {
        *b = 0;
    }
}

// ---------------------------------------------------------------------------
// InternalsPlugin.hpp structures (pack 4)
// ---------------------------------------------------------------------------

/// `TelemVect3` — three doubles (`x`, `y`, `z`). 24 bytes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Pod, Zeroable)]
#[repr(C, packed(4))]
pub struct TelemVect3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl TelemVect3 {
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }
    #[inline]
    pub fn as_array(&self) -> [f64; 3] {
        [self.x, self.y, self.z]
    }
    #[inline]
    pub fn as_f32(&self) -> [f32; 3] {
        [self.x as f32, self.y as f32, self.z as f32]
    }
    #[inline]
    pub fn norm(&self) -> f64 {
        let (x, y, z) = (self.x, self.y, self.z);
        (x * x + y * y + z * z).sqrt()
    }
}

impl From<[f64; 3]> for TelemVect3 {
    fn from(v: [f64; 3]) -> Self {
        Self::new(v[0], v[1], v[2])
    }
}

/// `TelemWheelV01` — per-wheel telemetry. 260 bytes.
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C, packed(4))]
pub struct TelemWheelV01 {
    /// metres
    pub suspension_deflection: f64,
    /// metres
    pub ride_height: f64,
    /// pushrod load, N
    pub susp_force: f64,
    /// Header says Celsius; the rF2 community reads Kelvin. See convert.rs.
    pub brake_temp: f64,
    /// 0..1 for now (true kPa promised "in future")
    pub brake_pressure: f64,
    /// rad/s
    pub rotation: f64,
    pub lateral_patch_vel: f64,
    pub longitudinal_patch_vel: f64,
    pub lateral_ground_vel: f64,
    pub longitudinal_ground_vel: f64,
    /// radians
    pub camber: f64,
    pub lateral_force: f64,
    pub longitudinal_force: f64,
    pub tire_load: f64,
    /// fraction of the contact patch that is sliding
    pub grip_fract: f64,
    /// kPa
    pub pressure: f64,
    /// Kelvin, left / centre / right
    pub temperature: [f64; 3],
    /// 0..1 fraction of maximum
    pub wear: f64,
    pub terrain_name: [u8; 16],
    /// 0 dry, 1 wet, 2 grass, 3 dirt, 4 gravel, 5 rumble, 6 special
    pub surface_type: u8,
    pub flat: u8,
    pub detached: u8,
    /// centimetres
    pub static_undeflected_radius: u8,
    pub vertical_tire_deflection: f64,
    pub wheel_y_location: f64,
    /// radians
    pub toe: f64,
    /// Kelvin
    pub tire_carcass_temperature: f64,
    /// Kelvin
    pub tire_inner_layer_temperature: [f64; 3],
    pub optimal_temp: f32,
    pub compound_index: u8,
    /// 0 soft, 1 medium, 2 hard, 3 wet
    pub compound_type: u8,
    pub expansion: [u8; 18],
}

impl Default for TelemWheelV01 {
    fn default() -> Self {
        Self::zeroed()
    }
}

/// `TelemInfoV01` — per-vehicle telemetry. 1888 bytes.
#[derive(Clone, Copy, Pod, Zeroable)]
#[repr(C, packed(4))]
pub struct TelemInfoV01 {
    /// slot id (re-used in multiplayer)
    pub id: i32,
    pub delta_time: f64,
    /// game session time, seconds
    pub elapsed_time: f64,
    pub lap_number: i32,
    pub lap_start_et: f64,
    pub vehicle_name: [u8; 64],
    pub track_name: [u8; 64],

    pub pos: TelemVect3,
    /// local axes: +x left, +y up, +z back
    pub local_vel: TelemVect3,
    pub local_accel: TelemVect3,
    pub ori: [TelemVect3; 3],
    /// rad/s, +y = yaw right
    pub local_rot: TelemVect3,
    pub local_rot_accel: TelemVect3,

    /// -1 reverse, 0 neutral, 1.. forward
    pub gear: i32,
    pub engine_rpm: f64,
    /// Celsius
    pub engine_water_temp: f64,
    /// Celsius
    pub engine_oil_temp: f64,
    pub clutch_rpm: f64,

    pub unfiltered_throttle: f64,
    pub unfiltered_brake: f64,
    pub unfiltered_steering: f64,
    pub unfiltered_clutch: f64,
    pub filtered_throttle: f64,
    pub filtered_brake: f64,
    pub filtered_steering: f64,
    pub filtered_clutch: f64,

    pub steering_shaft_torque: f64,
    pub front3rd_deflection: f64,
    pub rear3rd_deflection: f64,

    pub front_wing_height: f64,
    pub front_ride_height: f64,
    pub rear_ride_height: f64,
    pub drag: f64,
    pub front_downforce: f64,
    pub rear_downforce: f64,

    /// litres
    pub fuel: f64,
    pub engine_max_rpm: f64,
    pub scheduled_stops: u8,
    pub overheating: u8,
    pub detached: u8,
    pub headlights: u8,
    pub dent_severity: [u8; 8],
    pub last_impact_et: f64,
    pub last_impact_magnitude: f64,
    pub last_impact_pos: TelemVect3,

    pub engine_torque: f64,
    /// zero-based sector, pit lane in the sign bit
    pub current_sector: i32,
    pub speed_limiter: u8,
    pub max_gears: u8,
    pub front_tire_compound_index: u8,
    pub rear_tire_compound_index: u8,
    pub fuel_capacity: f64,
    pub front_flap_activated: u8,
    pub rear_flap_activated: u8,
    /// 0 disallowed, 1 criteria met, 2 allowed
    pub rear_flap_legal_status: u8,
    /// 0 off, 1 ignition, 2 ignition + starter
    pub ignition_starter: u8,

    pub front_tire_compound_name: [u8; 18],
    pub rear_tire_compound_name: [u8; 18],

    pub speed_limiter_available: u8,
    pub anti_stall_activated: u8,
    pub unused: [u8; 2],
    pub visual_steering_wheel_range: f32,

    /// fraction of braking on the rear axle
    pub rear_brake_bias: f64,
    pub turbo_boost_pressure: f64,
    pub physics_to_graphics_offset: [f32; 3],
    pub physical_steering_wheel_range: f32,

    pub delta_best: f64,
    /// 0..1
    pub battery_charge_fraction: f64,
    pub electric_boost_motor_torque: f64,
    pub electric_boost_motor_rpm: f64,
    pub electric_boost_motor_temperature: f64,
    pub electric_boost_water_temperature: f64,
    /// 0 unavailable, 1 inactive, 2 propulsion, 3 regeneration
    pub electric_boost_motor_state: u8,
    pub lap_invalidated: u8,
    pub abs_active: u8,
    pub tc_active: u8,
    pub speed_limiter_active: u8,
    pub wiper_state: u8,
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
    pub migration: u8,
    pub migration_max: u8,
    pub front_anti_sway: u8,
    pub front_anti_sway_max: u8,
    pub rear_anti_sway: u8,
    pub rear_anti_sway_max: u8,
    pub lift_and_coast_progress: u8,
    pub track_limits_steps: u8,
    /// kW
    pub regen: f32,
    pub soc: f32,
    pub virtual_energy: f32,
    pub time_gap_car_ahead: f32,
    pub time_gap_car_behind: f32,
    pub time_gap_place_ahead: f32,
    pub time_gap_place_behind: f32,
    pub vehicle_model: [u8; 30],
    /// `IP_VehicleClass`, see [`vehicle_class`]
    pub vehicle_class: u8,
    /// `IP_VehicleChampionship`, see [`championship`]
    pub vehicle_championship: u8,
    pub expansion: [u8; 20],

    /// FL, FR, RL, RR
    pub wheel: [TelemWheelV01; 4],
}

impl Default for TelemInfoV01 {
    fn default() -> Self {
        Self::zeroed()
    }
}

impl TelemInfoV01 {
    pub fn vehicle_name(&self) -> String {
        cstr(&self.vehicle_name)
    }
    pub fn track_name(&self) -> String {
        cstr(&self.track_name)
    }
    pub fn vehicle_model(&self) -> String {
        cstr(&self.vehicle_model)
    }
    pub fn front_tire_compound_name(&self) -> String {
        cstr(&self.front_tire_compound_name)
    }
    pub fn rear_tire_compound_name(&self) -> String {
        cstr(&self.rear_tire_compound_name)
    }
    /// `true` when the sign bit of `current_sector` is set (vehicle in the pit lane).
    #[inline]
    pub fn in_pit_lane(&self) -> bool {
        self.current_sector < 0
    }
    /// Zero-based sector (0..2) with the pit-lane bit removed.
    #[inline]
    pub fn sector_index(&self) -> i32 {
        self.current_sector & 0x7FFF_FFFF
    }
}

impl std::fmt::Debug for TelemInfoV01 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (id, et, lap, gear, rpm) = (
            self.id,
            self.elapsed_time,
            self.lap_number,
            self.gear,
            self.engine_rpm,
        );
        f.debug_struct("TelemInfoV01")
            .field("id", &id)
            .field("elapsed_time", &et)
            .field("lap_number", &lap)
            .field("vehicle_name", &self.vehicle_name())
            .field("track_name", &self.track_name())
            .field("gear", &gear)
            .field("engine_rpm", &rpm)
            .finish_non_exhaustive()
    }
}

/// `VehicleScoringInfoV01` — per-vehicle scoring. 584 bytes.
#[derive(Clone, Copy, Pod, Zeroable)]
#[repr(C, packed(4))]
pub struct VehicleScoringInfoV01 {
    pub id: i32,
    pub driver_name: [u8; 32],
    pub vehicle_name: [u8; 64],
    /// laps completed
    pub total_laps: i16,
    /// 0 = sector 3, 1 = sector 1, 2 = sector 2
    pub sector: i8,
    /// 0 none, 1 finished, 2 dnf, 3 dq
    pub finish_status: i8,
    pub lap_dist: f64,
    pub path_lateral: f64,
    pub track_edge: f64,

    pub best_sector1: f64,
    /// cumulative (S1 + S2)
    pub best_sector2: f64,
    pub best_lap_time: f64,
    pub last_sector1: f64,
    /// cumulative (S1 + S2)
    pub last_sector2: f64,
    pub last_lap_time: f64,
    pub cur_sector1: f64,
    /// cumulative (S1 + S2)
    pub cur_sector2: f64,

    pub num_pitstops: i16,
    pub num_penalties: i16,
    pub is_player: u8,
    /// -1 nobody, 0 local player, 1 local AI, 2 remote, 3 replay
    pub control: i8,
    pub in_pits: u8,
    /// 1-based
    pub place: u8,
    pub vehicle_class: [u8; 32],

    pub time_behind_next: f64,
    pub laps_behind_next: i32,
    pub time_behind_leader: f64,
    pub laps_behind_leader: i32,
    pub lap_start_et: f64,

    pub pos: TelemVect3,
    pub local_vel: TelemVect3,
    pub local_accel: TelemVect3,
    pub ori: [TelemVect3; 3],
    pub local_rot: TelemVect3,
    pub local_rot_accel: TelemVect3,

    pub headlights: u8,
    /// 0 none, 1 request, 2 entering, 3 stopped, 4 exiting
    pub pit_state: u8,
    pub server_scored: u8,
    pub individual_phase: u8,

    /// 1-based, -1 when invalid
    pub qualification: i32,
    pub time_into_lap: f64,
    pub estimated_lap_time: f64,

    pub pit_group: [u8; 24],
    /// 0 green, 6 blue
    pub flag: u8,
    pub under_yellow: u8,
    pub count_lap_flag: u8,
    pub in_garage_stall: u8,

    pub upgrade_pack: [u8; 16],
    pub pit_lap_dist: f32,
    pub best_lap_sector1: f32,
    /// cumulative (S1 + S2) of the best lap
    pub best_lap_sector2: f32,
    pub steam_id: u64,
    pub veh_filename: [u8; 32],
    pub attack_mode: i16,
    /// 0x00 = 0 %, 0xFF = 100 %
    pub fuel_fraction: u8,
    pub drs_state: u8,
    pub expansion: [u8; 4],
}

impl Default for VehicleScoringInfoV01 {
    fn default() -> Self {
        Self::zeroed()
    }
}

impl VehicleScoringInfoV01 {
    pub fn driver_name(&self) -> String {
        cstr(&self.driver_name)
    }
    pub fn vehicle_name(&self) -> String {
        cstr(&self.vehicle_name)
    }
    pub fn vehicle_class(&self) -> String {
        cstr(&self.vehicle_class)
    }
    pub fn pit_group(&self) -> String {
        cstr(&self.pit_group)
    }
    pub fn veh_filename(&self) -> String {
        cstr(&self.veh_filename)
    }
}

impl std::fmt::Debug for VehicleScoringInfoV01 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (id, place, laps) = (self.id, self.place, self.total_laps);
        f.debug_struct("VehicleScoringInfoV01")
            .field("id", &id)
            .field("driver_name", &self.driver_name())
            .field("vehicle_name", &self.vehicle_name())
            .field("vehicle_class", &self.vehicle_class())
            .field("place", &place)
            .field("total_laps", &laps)
            .finish_non_exhaustive()
    }
}

/// `ScoringInfoV01` — session scoring. 548 bytes.
#[derive(Clone, Copy, Pod, Zeroable)]
#[repr(C, packed(4))]
pub struct ScoringInfoV01 {
    pub track_name: [u8; 64],
    /// 0 test day, 1-4 practice, 5-8 qualify, 9 warmup, 10-13 race
    pub session: i32,
    pub current_et: f64,
    pub end_et: f64,
    pub max_laps: i32,
    /// track length, metres
    pub lap_dist: f64,
    /// `char*` in the game process — meaningless here
    pub results_stream: u64,
    pub num_vehicles: i32,
    /// 0..9, see `GamePhase`
    pub game_phase: u8,
    /// -1..7, see `YellowFlagState`
    pub yellow_flag_state: i8,
    pub sector_flag: [i8; 3],
    pub start_light: u8,
    pub num_red_lights: u8,
    pub in_realtime: u8,
    pub player_name: [u8; 32],
    pub plr_file_name: [u8; 64],

    pub dark_cloud: f64,
    pub raining: f64,
    /// Celsius
    pub ambient_temp: f64,
    /// Celsius
    pub track_temp: f64,
    pub wind: TelemVect3,
    pub min_path_wetness: f64,
    pub max_path_wetness: f64,

    /// 1 server, 2 client, 3 server and client
    pub game_mode: u8,
    pub is_password_protected: u8,
    pub server_port: u16,
    pub server_public_ip: u32,
    pub max_players: i32,
    pub server_name: [u8; 32],
    /// seconds since midnight
    pub start_et: f32,

    pub avg_path_wetness: f64,
    pub session_time_remaining: f32,
    pub time_of_day: f32,
    pub is_fixed_setup: u8,
    /// 0 green .. 4 saturated
    pub track_grip_level: u8,
    /// 0..10
    pub cloud_coverage: u8,
    pub track_limits_steps_per_penalty: u8,
    pub track_limits_steps_per_point: u8,
    pub expansion: [u8; 187],
    /// `VehicleScoringInfoV01*` in the game process — meaningless here
    pub vehicle: u64,
}

impl Default for ScoringInfoV01 {
    fn default() -> Self {
        Self::zeroed()
    }
}

impl ScoringInfoV01 {
    pub fn track_name(&self) -> String {
        cstr(&self.track_name)
    }
    pub fn player_name(&self) -> String {
        cstr(&self.player_name)
    }
    pub fn plr_file_name(&self) -> String {
        cstr(&self.plr_file_name)
    }
    pub fn server_name(&self) -> String {
        cstr(&self.server_name)
    }
}

impl std::fmt::Debug for ScoringInfoV01 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (session, et, n, phase) = (
            self.session,
            self.current_et,
            self.num_vehicles,
            self.game_phase,
        );
        f.debug_struct("ScoringInfoV01")
            .field("track_name", &self.track_name())
            .field("session", &session)
            .field("current_et", &et)
            .field("num_vehicles", &n)
            .field("game_phase", &phase)
            .finish_non_exhaustive()
    }
}

/// `ApplicationStateV01`. 260 bytes.
#[derive(Clone, Copy, Pod, Zeroable)]
#[repr(C, packed(4))]
pub struct ApplicationStateV01 {
    /// `HWND` — meaningless outside the game process
    pub app_window: u64,
    pub width: u32,
    pub height: u32,
    pub refresh_rate: u32,
    pub windowed: u32,
    /// 0 main UI, 1 track loading, 2 monitor, 3 on track
    pub options_location: u8,
    pub options_page: [u8; 31],
    pub expansion: [u8; 204],
}

impl Default for ApplicationStateV01 {
    fn default() -> Self {
        Self::zeroed()
    }
}

impl ApplicationStateV01 {
    pub fn options_page(&self) -> String {
        cstr(&self.options_page)
    }
}

impl std::fmt::Debug for ApplicationStateV01 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (w, h, loc) = (self.width, self.height, self.options_location);
        f.debug_struct("ApplicationStateV01")
            .field("width", &w)
            .field("height", &h)
            .field("options_location", &loc)
            .field("options_page", &self.options_page())
            .finish_non_exhaustive()
    }
}

// ---------------------------------------------------------------------------
// SharedMemoryInterface.hpp envelopes (default alignment)
// ---------------------------------------------------------------------------

/// `SharedMemoryGeneric`. 332 bytes, alignment 4.
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
#[repr(C)]
pub struct SharedMemoryGeneric {
    /// "queue of events": `SharedMemoryEvent` values (see [`SharedMemoryEvent`]).
    pub events: [u32; SME_MAX],
    pub game_version: i32,
    pub ffb_torque: f32,
    pub app_info: ApplicationStateV01,
}

impl Default for SharedMemoryGeneric {
    fn default() -> Self {
        Self::zeroed()
    }
}

impl SharedMemoryGeneric {
    /// `true` when the event queue contains `ev` (mirrors `CopySharedMemoryObj`).
    pub fn has_event(&self, ev: SharedMemoryEvent) -> bool {
        self.events.contains(&(ev as u32))
    }
}

/// `SharedMemoryPathData`: 5 × `char[MAX_PATH]`. 1300 bytes, alignment 1.
#[derive(Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct SharedMemoryPathData {
    pub user_data: [u8; MAX_PATH],
    pub custom_variables: [u8; MAX_PATH],
    pub steward_results: [u8; MAX_PATH],
    pub player_profile: [u8; MAX_PATH],
    pub plugins_folder: [u8; MAX_PATH],
}

impl Default for SharedMemoryPathData {
    fn default() -> Self {
        Self::zeroed()
    }
}

impl SharedMemoryPathData {
    pub fn user_data(&self) -> String {
        cstr(&self.user_data)
    }
    pub fn custom_variables(&self) -> String {
        cstr(&self.custom_variables)
    }
    pub fn steward_results(&self) -> String {
        cstr(&self.steward_results)
    }
    pub fn player_profile(&self) -> String {
        cstr(&self.player_profile)
    }
    pub fn plugins_folder(&self) -> String {
        cstr(&self.plugins_folder)
    }
}

impl std::fmt::Debug for SharedMemoryPathData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SharedMemoryPathData")
            .field("user_data", &self.user_data())
            .field("custom_variables", &self.custom_variables())
            .field("steward_results", &self.steward_results())
            .field("player_profile", &self.player_profile())
            .field("plugins_folder", &self.plugins_folder())
            .finish()
    }
}

/// `SharedMemoryScoringData`. 126 832 bytes, alignment 8.
#[derive(Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct SharedMemoryScoringData {
    pub scoring_info: ScoringInfoV01,
    /// MSVC inserts 4 bytes so that the following `size_t` is 8-aligned.
    pub _pad0: [u8; 4],
    pub scoring_stream_size: u64,
    pub veh_scoring_info: [VehicleScoringInfoV01; MAX_VEHICLES],
    pub scoring_stream: [u8; SCORING_STREAM_LEN],
}

impl Default for SharedMemoryScoringData {
    fn default() -> Self {
        Self::zeroed()
    }
}

impl SharedMemoryScoringData {
    /// The vehicles announced by `scoring_info.num_vehicles` (clamped to the array).
    pub fn vehicles(&self) -> &[VehicleScoringInfoV01] {
        let n = self.scoring_info.num_vehicles.clamp(0, MAX_VEHICLES as i32) as usize;
        &self.veh_scoring_info[..n]
    }
    /// The results stream (newline-delimited text), bounded by `scoring_stream_size`.
    pub fn scoring_stream(&self) -> String {
        let n = (self.scoring_stream_size as usize).min(SCORING_STREAM_LEN);
        cstr(&self.scoring_stream[..n])
    }
}

/// `SharedMemoryTelemetryData`. 196 356 bytes, alignment 4.
#[derive(Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct SharedMemoryTelemetryData {
    pub active_vehicles: u8,
    pub player_vehicle_idx: u8,
    pub player_has_vehicle: u8,
    /// alignment padding before the `TelemInfoV01` array
    pub _pad0: u8,
    pub telem_info: [TelemInfoV01; MAX_VEHICLES],
}

impl Default for SharedMemoryTelemetryData {
    fn default() -> Self {
        Self::zeroed()
    }
}

impl SharedMemoryTelemetryData {
    /// The vehicles announced by `active_vehicles` (clamped to the array).
    pub fn vehicles(&self) -> &[TelemInfoV01] {
        let n = (self.active_vehicles as usize).min(MAX_VEHICLES);
        &self.telem_info[..n]
    }
    /// The player's telemetry, when `player_has_vehicle` and the index is valid.
    pub fn player(&self) -> Option<&TelemInfoV01> {
        if !as_bool(self.player_has_vehicle) {
            return None;
        }
        self.telem_info.get(self.player_vehicle_idx as usize)
    }
}

/// `SharedMemoryObjectOut` — the whole `LMU_Data` block. 324 824 bytes
/// (324 820 useful + 4 trailing padding bytes), alignment 8.
#[derive(Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct SharedMemoryObjectOut {
    pub generic: SharedMemoryGeneric,
    pub paths: SharedMemoryPathData,
    pub scoring: SharedMemoryScoringData,
    pub telemetry: SharedMemoryTelemetryData,
    /// trailing padding so that `size_of` is a multiple of 8
    pub _pad_end: [u8; 4],
}

impl Default for SharedMemoryObjectOut {
    fn default() -> Self {
        Self::zeroed()
    }
}

impl SharedMemoryObjectOut {
    /// Heap-allocated zeroed block (the struct is ~317 KiB: never put it on the stack).
    pub fn boxed() -> Box<Self> {
        bytemuck::zeroed_box()
    }

    /// Raw bytes of the block.
    pub fn as_bytes(&self) -> &[u8] {
        bytemuck::bytes_of(self)
    }

    /// Mutable raw bytes of the block.
    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        bytemuck::bytes_of_mut(self)
    }

    /// The player's telemetry (if any).
    pub fn player_telemetry(&self) -> Option<&TelemInfoV01> {
        self.telemetry.player()
    }

    /// The player's scoring entry: matched by slot `id` on the telemetry
    /// entry, else by `is_player`.
    pub fn player_scoring(&self) -> Option<&VehicleScoringInfoV01> {
        let vehicles = self.scoring.vehicles();
        if let Some(t) = self.player_telemetry() {
            let id = t.id;
            if let Some(v) = vehicles.iter().find(|v| v.id == id) {
                return Some(v);
            }
        }
        vehicles.iter().find(|v| as_bool(v.is_player))
    }

    /// Session elapsed time (`ScoringInfoV01::mCurrentET`).
    #[inline]
    pub fn current_et(&self) -> f64 {
        self.scoring.scoring_info.current_et
    }

    /// Player's `mElapsedTime`, 0 when there is no player vehicle.
    #[inline]
    pub fn player_elapsed_time(&self) -> f64 {
        self.player_telemetry()
            .map(|t| t.elapsed_time)
            .unwrap_or(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{align_of, offset_of, size_of};

    #[test]
    fn sizes() {
        assert_eq!(size_of::<TelemVect3>(), SIZE_TELEM_VECT3);
        assert_eq!(size_of::<TelemWheelV01>(), SIZE_TELEM_WHEEL);
        assert_eq!(size_of::<TelemInfoV01>(), SIZE_TELEM_INFO);
        assert_eq!(
            size_of::<VehicleScoringInfoV01>(),
            SIZE_VEHICLE_SCORING_INFO
        );
        assert_eq!(size_of::<ScoringInfoV01>(), SIZE_SCORING_INFO);
        assert_eq!(size_of::<ApplicationStateV01>(), SIZE_APPLICATION_STATE);
        assert_eq!(size_of::<SharedMemoryGeneric>(), SIZE_GENERIC);
        assert_eq!(size_of::<SharedMemoryPathData>(), SIZE_PATHS);
        assert_eq!(size_of::<SharedMemoryScoringData>(), SIZE_SCORING_DATA);
        assert_eq!(size_of::<SharedMemoryTelemetryData>(), SIZE_TELEMETRY_DATA);
        assert_eq!(size_of::<SharedMemoryObjectOut>(), SHM_TOTAL_SIZE);
        assert_eq!(align_of::<SharedMemoryObjectOut>(), 8);
        assert_eq!(align_of::<SharedMemoryScoringData>(), 8);
        assert_eq!(align_of::<SharedMemoryTelemetryData>(), 4);
        assert_eq!(align_of::<SharedMemoryPathData>(), 1);
    }

    #[test]
    fn block_offsets() {
        assert_eq!(offset_of!(SharedMemoryObjectOut, generic), OFF_GENERIC);
        assert_eq!(offset_of!(SharedMemoryObjectOut, paths), OFF_PATHS);
        assert_eq!(offset_of!(SharedMemoryObjectOut, scoring), OFF_SCORING);
        assert_eq!(offset_of!(SharedMemoryObjectOut, telemetry), OFF_TELEMETRY);
        assert_eq!(
            OFF_SCORING + offset_of!(SharedMemoryScoringData, scoring_stream_size),
            OFF_SCORING_STREAM_SIZE
        );
        assert_eq!(
            OFF_SCORING + offset_of!(SharedMemoryScoringData, veh_scoring_info),
            OFF_VEH_SCORING_INFO
        );
        assert_eq!(
            OFF_SCORING + offset_of!(SharedMemoryScoringData, scoring_stream),
            OFF_SCORING_STREAM
        );
        assert_eq!(
            OFF_TELEMETRY + offset_of!(SharedMemoryTelemetryData, telem_info),
            OFF_TELEM_INFO
        );
        assert_eq!(
            OFF_SCORING + offset_of!(ScoringInfoV01, current_et),
            OFF_CURRENT_ET
        );
        assert_eq!(
            offset_of!(TelemInfoV01, elapsed_time),
            OFF_TELEM_ELAPSED_TIME
        );
        assert_eq!(telem_elapsed_time_offset(1), 128_468 + 1888 + 12);
        assert_eq!(offset_of!(TelemInfoV01, wheel), 848);
        assert_eq!(offset_of!(TelemInfoV01, vehicle_class), 826);
        assert_eq!(offset_of!(VehicleScoringInfoV01, steam_id), 536);
        assert_eq!(offset_of!(ScoringInfoV01, vehicle), 540);
    }

    #[test]
    fn strings() {
        let mut buf = [0u8; 8];
        set_cstr(&mut buf, "Le Mans Ultimate");
        assert_eq!(buf, *b"Le Mans\0");
        assert_eq!(cstr(&buf), "Le Mans");
        assert_eq!(cstr(b"abc\0def"), "abc");
        assert_eq!(cstr(b"no-nul"), "no-nul");
        assert_eq!(cstr(b"caf\xe9\0"), "caf\u{fffd}");
        assert_eq!(cstr(b""), "");
    }

    #[test]
    fn boxed_block_is_zeroed_and_sized() {
        let b = SharedMemoryObjectOut::boxed();
        assert_eq!(b.as_bytes().len(), SHM_TOTAL_SIZE);
        assert!(b.as_bytes().iter().all(|&x| x == 0));
        assert!(b.player_telemetry().is_none());
        assert!(b.scoring.vehicles().is_empty());
    }

    #[test]
    fn player_lookup() {
        let mut b = SharedMemoryObjectOut::boxed();
        b.telemetry.player_has_vehicle = 1;
        b.telemetry.player_vehicle_idx = 2;
        b.telemetry.active_vehicles = 3;
        b.telemetry.telem_info[2].id = 42;
        b.scoring.scoring_info.num_vehicles = 3;
        b.scoring.veh_scoring_info[0].id = 42;
        b.scoring.veh_scoring_info[1].is_player = 1;
        b.scoring.veh_scoring_info[1].id = 7;
        // matched by id first
        assert_eq!(b.player_scoring().map(|v| v.id), Some(42));
        // falls back to is_player
        b.scoring.veh_scoring_info[0].id = 99;
        assert_eq!(b.player_scoring().map(|v| v.id), Some(7));
        assert!(b.generic.has_event(SharedMemoryEvent::Enter)); // zeroed queue == SME_ENTER
        b.generic.events = [11; SME_MAX];
        assert!(b.generic.has_event(SharedMemoryEvent::UpdateTelemetry));
        assert!(!b.generic.has_event(SharedMemoryEvent::UpdateScoring));
    }
}
