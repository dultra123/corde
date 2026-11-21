//! Byte-exact mirror of the graphics and static pages of Assetto Corsa EVO.
//!
//! Source: the shared memory documentation the studio published for the game (interface
//! « 1.x » of the early access), read through two independent open-source readers. The
//! structures below are written from the list of the fields, not copied; their sizes and
//! offsets are checked against a computation made apart (4-byte packing).
//!
//! | page | mapping | size | updated |
//! |---|---|---|---|
//! | physics | `Local\acevo_pmf_physics` | 800 bytes | every physics step |
//! | [`GraphicsEvo`] | `Local\acevo_pmf_graphics` | 4900 bytes | every drawn frame |
//! | [`StaticEvo`] | `Local\acevo_pmf_static` | 208 bytes | once per session |
//!
//! The physics page has the layout of Competizione's ([`crate::layout::Physics`]): the
//! same structure reads both.
//!
//! The game is in early access: its pages may still change. The static page carries the
//! version of the interface (`sm_version`); `acc-probe --evo` prints it. A page is at
//! least one memory page filled with zeros after what the game writes, so a field a
//! build does not have yet reads as zero — the table of the cars' identifiers at the end
//! of the graphics page was added in April 2026, for instance.
//!
//! The game writes booleans as one byte and its texts as ASCII, one byte a character:
//! every field is an integer, a float or a byte, so any bit pattern is valid.

use bytemuck::{AnyBitPattern, Zeroable};

pub use crate::layout::status;

/// `StaticEvo::session`.
pub mod session {
    pub const UNKNOWN: i32 = -1;
    pub const TIME_ATTACK: i32 = 0;
    pub const RACE: i32 = 1;
    pub const HOT_STINT: i32 = 2;
    pub const CRUISE: i32 = 3;
}

/// `GraphicsEvo::flag` and `GraphicsEvo::global_flag`.
pub mod flag {
    pub const NONE: i32 = 0;
    pub const WHITE: i32 = 1;
    pub const GREEN: i32 = 2;
    pub const RED: i32 = 3;
    pub const BLUE: i32 = 4;
    pub const YELLOW: i32 = 5;
    pub const BLACK: i32 = 6;
    pub const BLACK_WHITE: i32 = 7;
    pub const CHECKERED: i32 = 8;
    pub const ORANGE_CIRCLE: i32 = 9;
    pub const RED_YELLOW_STRIPES: i32 = 10;
}

/// `GraphicsEvo::car_location`.
pub mod location {
    pub const UNASSIGNED: i32 = 0;
    pub const PIT_LANE: i32 = 1;
    pub const PIT_ENTRY: i32 = 2;
    pub const PIT_EXIT: i32 = 3;
    pub const TRACK: i32 = 4;
}

/// `GraphicsEvo::engine_type`.
pub mod engine {
    pub const INTERNAL_COMBUSTION: i32 = 0;
    pub const ELECTRIC: i32 = 1;
}

/// Cars the graphics page can place.
pub const MAX_CARS: usize = 60;

/// One corner of the car (256 bytes, four times in the graphics page).
#[repr(C, packed(4))]
#[derive(Clone, Copy, AnyBitPattern)]
pub struct TyreState {
    pub slip: f32,
    /// The wheel is locked under braking.
    pub lock: u8,
    /// psi.
    pub pressure: f32,
    /// °C, one number for the tyre.
    pub temperature_c: f32,
    pub brake_temperature_c: f32,
    pub brake_pressure: f32,
    /// °C. « Left » and « right » are the sides of the car: the left edge of the tread is
    /// the outside of a left wheel and the inside of a right one.
    pub temperature_left: f32,
    pub temperature_center: f32,
    pub temperature_right: f32,
    /// The same two names in the four corners.
    pub compound_front: [u8; 33],
    pub compound_rear: [u8; 33],
    /// 1.0 = the ideal of the compound, for the six below.
    pub normalized_pressure: f32,
    pub normalized_temperature_left: f32,
    pub normalized_temperature_center: f32,
    pub normalized_temperature_right: f32,
    pub brake_normalized_temperature: f32,
    pub normalized_temperature_core: f32,
    pub reserved: [u8; 128],
}

/// Damage by zone and by suspension corner (128 bytes).
#[repr(C, packed(4))]
#[derive(Clone, Copy, AnyBitPattern)]
pub struct DamageState {
    pub front: f32,
    pub rear: f32,
    pub left: f32,
    pub right: f32,
    pub center: f32,
    /// [front left, front right, rear left, rear right].
    pub suspension: [f32; 4],
    pub reserved: [u8; 92],
}

/// What a pit stop does (64 bytes): −1 will not be done, 0 done, 1 being done.
#[repr(C, packed(4))]
#[derive(Clone, Copy, AnyBitPattern)]
pub struct PitInfo {
    pub damage: i8,
    pub fuel: i8,
    /// [front left, front right, rear left, rear right].
    pub tyres: [i8; 4],
    pub reserved: [u8; 58],
}

/// The settings the driver changes from the wheel (128 bytes). The page holds four of
/// them: the values, their minimum, their maximum, and which ones can be changed.
#[repr(C, packed(4))]
#[derive(Clone, Copy, AnyBitPattern)]
pub struct ElectronicsState {
    pub tc_level: i8,
    pub tc_cut_level: i8,
    pub abs_level: i8,
    pub esc_level: i8,
    pub ebb_level: i8,
    /// Share of the front axle, 0..1.
    pub brake_bias: f32,
    pub engine_map_level: i8,
    pub turbo_level: f32,
    pub ers_deployment_map: i8,
    pub ers_recharge_map: f32,
    pub is_ers_heat_charging_on: u8,
    pub is_ers_overtake_mode_on: u8,
    pub is_drs_open: u8,
    pub diff_power_level: i8,
    pub diff_coast_level: i8,
    pub front_bump_damper_level: i8,
    pub front_rebound_damper_level: i8,
    pub rear_bump_damper_level: i8,
    pub rear_rebound_damper_level: i8,
    pub is_ignition_on: u8,
    pub is_pitlimiter_on: u8,
    pub active_performance_mode: i8,
    pub reserved: [u8; 88],
}

/// Lights, wipers and displays of the cockpit (128 bytes).
#[repr(C, packed(4))]
#[derive(Clone, Copy, AnyBitPattern)]
pub struct InstrumentationState {
    /// 0 = off.
    pub main_light_stage: i8,
    pub special_light_stage: i8,
    pub cockpit_light_stage: i8,
    /// 0 = off.
    pub wiper_level: i8,
    pub rain_lights: u8,
    pub direction_light_left: u8,
    pub direction_light_right: u8,
    pub flashing_lights: u8,
    pub warning_lights: u8,
    pub selected_display_index: i8,
    pub display_current_page_index: [i8; 16],
    pub are_headlights_visible: u8,
    pub reserved: [u8; 101],
}

/// Where the session stands (256 bytes).
#[repr(C, packed(4))]
#[derive(Clone, Copy, AnyBitPattern)]
pub struct SessionState {
    /// « Race », « Qualify »…
    pub phase_name: [u8; 33],
    /// As shown (HH:MM:SS).
    pub time_left: [u8; 15],
    pub time_left_ms: i32,
    pub wait_time: [u8; 15],
    /// Laps of the session, when it is counted in laps.
    pub total_lap: i32,
    /// The lap being driven.
    pub current_lap: i32,
    pub lights_on: i32,
    pub lights_mode: i32,
    pub lap_length_km: f32,
    pub end_session_flag: i32,
    pub time_to_next_session: [u8; 15],
    pub disconnected_from_server: u8,
    pub restart_season_enabled: u8,
    pub ui_enable_drive: u8,
    pub ui_enable_setup: u8,
    pub is_ready_to_next_blinking: u8,
    pub show_waiting_for_players: u8,
    pub reserved: [u8; 140],
}

/// The times as the game shows them, as texts (256 bytes).
#[repr(C, packed(4))]
#[derive(Clone, Copy, AnyBitPattern)]
pub struct TimingState {
    pub current_laptime: [u8; 15],
    pub delta_current: [u8; 15],
    pub delta_current_p: i32,
    pub last_laptime: [u8; 15],
    pub delta_last: [u8; 15],
    pub delta_last_p: i32,
    pub best_laptime: [u8; 15],
    /// The sum of the best sectors.
    pub ideal_laptime: [u8; 15],
    pub total_time: [u8; 15],
    pub is_invalid: u8,
    pub reserved: [u8; 137],
}

/// The driving aids of the game's options (64 bytes).
#[repr(C, packed(4))]
#[derive(Clone, Copy, AnyBitPattern)]
pub struct AssistsState {
    pub auto_gear: u8,
    pub auto_blip: u8,
    pub auto_clutch: u8,
    pub auto_clutch_on_start: u8,
    pub manual_ignition_e_start: u8,
    pub auto_pit_limiter: u8,
    pub standing_start_assist: u8,
    pub auto_steer: f32,
    pub arcade_stability_control: f32,
    pub reserved: [u8; 48],
}

/// `Local\acevo_pmf_graphics` — what the game's own displays show, every drawn frame.
#[repr(C, packed(4))]
#[derive(Clone, Copy, AnyBitPattern)]
pub struct GraphicsEvo {
    /// Step counter: changes when the page was rewritten.
    pub packet_id: i32,
    /// See [`status`].
    pub status: i32,
    /// The car the camera shows (two halves of one identifier).
    pub focused_car_id: [u64; 2],
    /// The player's own car.
    pub player_car_id: [u64; 2],
    pub rpm: u16,
    pub is_rpm_limiter_on: u8,
    pub is_change_up_rpm: u8,
    pub is_change_down_rpm: u8,
    pub tc_active: u8,
    pub abs_active: u8,
    pub esc_active: u8,
    pub launch_active: u8,
    pub is_ignition_on: u8,
    pub is_engine_running: u8,
    pub kers_is_charging: u8,
    pub is_wrong_way: u8,
    pub is_drs_available: u8,
    pub battery_is_charging: u8,
    pub is_max_kj_per_lap_reached: u8,
    pub is_max_charge_kj_per_lap_reached: u8,
    pub display_speed_kmh: i16,
    pub display_speed_mph: i16,
    pub display_speed_ms: i16,
    pub pitspeeding_delta: f32,
    /// As the physics page: 0 = reverse, 1 = neutral, 2 = first…
    pub gear_int: i16,
    /// 0..1 of the red line.
    pub rpm_percent: f32,
    /// 0..1, like the three below.
    pub gas_percent: f32,
    pub brake_percent: f32,
    pub handbrake_percent: f32,
    pub clutch_percent: f32,
    /// −1..1.
    pub steering_percent: f32,
    pub ffb_strength: f32,
    pub car_ffb_multiplier: f32,
    pub water_temperature_percent: f32,
    pub water_pressure_bar: f32,
    pub fuel_pressure_bar: f32,
    pub water_temperature_c: i8,
    pub air_temperature_c: i8,
    pub oil_temperature_c: f32,
    pub oil_pressure_bar: f32,
    pub exhaust_temperature_c: f32,
    pub g_forces_x: f32,
    pub g_forces_y: f32,
    pub g_forces_z: f32,
    pub turbo_boost: f32,
    pub turbo_boost_level: f32,
    pub turbo_boost_perc: f32,
    /// Rotation of the wheel from its centre, degrees.
    pub steer_degrees: i32,
    pub current_km: f32,
    pub total_km: u32,
    pub total_driving_time_s: u32,
    pub time_of_day_hours: i32,
    pub time_of_day_minutes: i32,
    pub time_of_day_seconds: i32,
    /// To the reference lap, ms.
    pub delta_time_ms: i32,
    pub current_lap_time_ms: i32,
    pub predicted_lap_time_ms: i32,
    pub fuel_liter_current_quantity: f32,
    pub fuel_liter_current_quantity_percent: f32,
    pub fuel_liter_per_km: f32,
    pub km_per_fuel_liter: f32,
    pub current_torque: f32,
    pub current_bhp: i32,
    /// The four corners: front left, front right, rear left, rear right (see
    /// [`GraphicsEvo::tyres`]).
    pub tyre_lf: TyreState,
    pub tyre_rf: TyreState,
    pub tyre_lr: TyreState,
    pub tyre_rr: TyreState,
    /// Place on the lap, 0..1 from the line.
    pub npos: f32,
    pub kers_charge_perc: f32,
    pub kers_current_perc: f32,
    pub control_lock_time: f32,
    pub car_damage: DamageState,
    /// See [`location`].
    pub car_location: i32,
    pub pit_info: PitInfo,
    pub fuel_liter_used: f32,
    pub fuel_liter_per_lap: f32,
    pub laps_possible_with_fuel: f32,
    pub battery_temperature: f32,
    pub battery_voltage: f32,
    pub instantaneous_fuel_liter_per_km: f32,
    pub instantaneous_km_per_fuel_liter: f32,
    pub gear_rpm_window: f32,
    pub instrumentation: InstrumentationState,
    pub instrumentation_min_limit: InstrumentationState,
    pub instrumentation_max_limit: InstrumentationState,
    pub electronics: ElectronicsState,
    pub electronics_min_limit: ElectronicsState,
    pub electronics_max_limit: ElectronicsState,
    pub electronics_is_modifiable: ElectronicsState,
    /// Laps completed in the session.
    pub total_lap_count: i32,
    /// 1 = the leader.
    pub current_pos: u32,
    pub total_drivers: u32,
    pub last_laptime_ms: i32,
    pub best_laptime_ms: i32,
    /// Shown to the driver, see [`flag`].
    pub flag: i32,
    /// Of the whole session.
    pub global_flag: i32,
    /// Forward gears.
    pub max_gears: u32,
    /// See [`engine`].
    pub engine_type: i32,
    pub has_kers: u8,
    pub is_last_lap: u8,
    pub performance_mode_name: [u8; 33],
    pub diff_coast_raw_value: f32,
    pub diff_power_raw_value: f32,
    pub race_cut_gained_time_ms: i32,
    pub distance_to_deadline: i32,
    pub race_cut_current_delta: f32,
    pub session_state: SessionState,
    pub timing_state: TimingState,
    pub player_ping: i32,
    pub player_latency: i32,
    pub player_cpu_usage: i32,
    pub player_cpu_usage_avg: i32,
    pub player_qos: i32,
    pub player_qos_avg: i32,
    pub player_fps: i32,
    pub player_fps_avg: i32,
    pub driver_name: [u8; 33],
    pub driver_surname: [u8; 33],
    /// The game's identifier of the car.
    pub car_model: [u8; 33],
    /// Standing in its box.
    pub is_in_pit_box: u8,
    pub is_in_pit_lane: u8,
    pub is_valid_lap: u8,
    /// World positions, by slot.
    pub car_coordinates: [[f32; 3]; MAX_CARS],
    /// Seconds.
    pub gap_ahead: f32,
    pub gap_behind: f32,
    pub active_cars: u8,
    pub fuel_per_lap: f32,
    pub fuel_estimated_laps: f32,
    pub assists_state: AssistsState,
    /// Litres.
    pub max_fuel: f32,
    pub max_turbo_boost: f32,
    pub use_single_compound: u8,
    /// The car of each slot of `car_coordinates` (the two halves of its identifier).
    pub car_ids: [[u64; 2]; MAX_CARS],
}

/// `Local\acevo_pmf_static` — the session and the circuit, written once per session.
/// The game publishes no specification of the car here (red line, tank, power): those
/// are in the two other pages.
#[repr(C, packed(4))]
#[derive(Clone, Copy, AnyBitPattern)]
pub struct StaticEvo {
    /// Version of this interface (« 1.0 »…).
    pub sm_version: [u8; 15],
    /// Build of the game.
    pub ac_evo_version: [u8; 15],
    /// See [`session`].
    pub session: i32,
    /// « Race 1 »…
    pub session_name: [u8; 33],
    pub event_id: u8,
    pub session_id: u8,
    pub starting_grip: i32,
    pub starting_ambient_temperature_c: f32,
    pub starting_ground_temperature_c: f32,
    pub is_static_weather: u8,
    pub is_timed_race: u8,
    pub is_online: u8,
    pub number_of_sessions: i32,
    pub nation: [u8; 33],
    pub longitude: f32,
    pub latitude: f32,
    pub track: [u8; 33],
    pub track_configuration: [u8; 33],
    pub track_length_m: f32,
}

pub const GRAPHICS_EVO_SIZE: usize = std::mem::size_of::<GraphicsEvo>();
/// What the graphics page held before the table of the cars' identifiers was added.
pub const GRAPHICS_EVO_MIN_SIZE: usize = std::mem::offset_of!(GraphicsEvo, car_ids);
pub const STATIC_EVO_SIZE: usize = std::mem::size_of::<StaticEvo>();

impl GraphicsEvo {
    pub fn zeroed() -> Self {
        Zeroable::zeroed()
    }

    /// The four corners in the order of every per-wheel array: front left, front right,
    /// rear left, rear right.
    pub fn tyres(&self) -> [TyreState; 4] {
        [self.tyre_lf, self.tyre_rf, self.tyre_lr, self.tyre_rr]
    }
}
impl StaticEvo {
    pub fn zeroed() -> Self {
        Zeroable::zeroed()
    }
}

/// A text of the game: bytes up to the first NUL.
pub fn text(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).trim().to_string()
}

/// `s` as a text field of `N` bytes, cut to leave the final NUL.
pub fn put_text<const N: usize>(s: &str) -> [u8; N] {
    let mut out = [0u8; N];
    for (slot, byte) in out.iter_mut().zip(s.bytes()).take(N.saturating_sub(1)) {
        *slot = byte;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{offset_of, size_of};

    /// The sizes the game's documentation states for the embedded structures, and those
    /// of the two pages worked out apart from the list of the fields.
    #[test]
    fn the_structures_have_the_documented_sizes() {
        assert_eq!(size_of::<TyreState>(), 256);
        assert_eq!(size_of::<DamageState>(), 128);
        assert_eq!(size_of::<PitInfo>(), 64);
        assert_eq!(size_of::<ElectronicsState>(), 128);
        assert_eq!(size_of::<InstrumentationState>(), 128);
        assert_eq!(size_of::<SessionState>(), 256);
        assert_eq!(size_of::<TimingState>(), 256);
        assert_eq!(size_of::<AssistsState>(), 64);
        assert_eq!(GRAPHICS_EVO_SIZE, 4900);
        assert_eq!(GRAPHICS_EVO_MIN_SIZE, 3940);
        assert_eq!(STATIC_EVO_SIZE, 208);
        // The physics page is Competizione's.
        assert_eq!(crate::layout::PHYSICS_SIZE, 800);
    }

    /// Offsets computed apart (4-byte packing: a 64-bit integer sits on a multiple of 4,
    /// a one-byte field anywhere, a float after a text of 33 bytes on the next multiple
    /// of 4).
    #[test]
    fn known_fields_sit_at_their_offsets() {
        assert_eq!(offset_of!(GraphicsEvo, focused_car_id), 8);
        assert_eq!(offset_of!(GraphicsEvo, rpm), 40);
        assert_eq!(offset_of!(GraphicsEvo, display_speed_kmh), 58);
        assert_eq!(offset_of!(GraphicsEvo, pitspeeding_delta), 64);
        assert_eq!(offset_of!(GraphicsEvo, gear_int), 68);
        assert_eq!(offset_of!(GraphicsEvo, rpm_percent), 72);
        assert_eq!(offset_of!(GraphicsEvo, water_temperature_c), 116);
        assert_eq!(offset_of!(GraphicsEvo, oil_temperature_c), 120);
        assert_eq!(offset_of!(GraphicsEvo, steer_degrees), 156);
        assert_eq!(offset_of!(GraphicsEvo, current_lap_time_ms), 188);
        assert_eq!(offset_of!(GraphicsEvo, fuel_liter_current_quantity), 196);
        assert_eq!(offset_of!(GraphicsEvo, tyre_lf), 220);
        assert_eq!(offset_of!(GraphicsEvo, tyre_rr), 988);
        assert_eq!(offset_of!(GraphicsEvo, npos), 1244);
        assert_eq!(offset_of!(GraphicsEvo, car_damage), 1260);
        assert_eq!(offset_of!(GraphicsEvo, car_location), 1388);
        assert_eq!(offset_of!(GraphicsEvo, pit_info), 1392);
        assert_eq!(offset_of!(GraphicsEvo, fuel_liter_used), 1456);
        assert_eq!(offset_of!(GraphicsEvo, instrumentation), 1488);
        assert_eq!(offset_of!(GraphicsEvo, electronics), 1872);
        assert_eq!(offset_of!(GraphicsEvo, electronics_max_limit), 2128);
        assert_eq!(offset_of!(GraphicsEvo, total_lap_count), 2384);
        assert_eq!(offset_of!(GraphicsEvo, last_laptime_ms), 2396);
        assert_eq!(offset_of!(GraphicsEvo, flag), 2404);
        assert_eq!(offset_of!(GraphicsEvo, max_gears), 2412);
        assert_eq!(offset_of!(GraphicsEvo, has_kers), 2420);
        assert_eq!(offset_of!(GraphicsEvo, performance_mode_name), 2422);
        assert_eq!(offset_of!(GraphicsEvo, diff_coast_raw_value), 2456);
        assert_eq!(offset_of!(GraphicsEvo, session_state), 2476);
        assert_eq!(offset_of!(GraphicsEvo, timing_state), 2732);
        assert_eq!(offset_of!(GraphicsEvo, player_ping), 2988);
        assert_eq!(offset_of!(GraphicsEvo, driver_name), 3020);
        assert_eq!(offset_of!(GraphicsEvo, car_model), 3086);
        assert_eq!(offset_of!(GraphicsEvo, is_in_pit_box), 3119);
        assert_eq!(offset_of!(GraphicsEvo, is_valid_lap), 3121);
        assert_eq!(offset_of!(GraphicsEvo, car_coordinates), 3124);
        assert_eq!(offset_of!(GraphicsEvo, gap_ahead), 3844);
        assert_eq!(offset_of!(GraphicsEvo, active_cars), 3852);
        assert_eq!(offset_of!(GraphicsEvo, fuel_per_lap), 3856);
        assert_eq!(offset_of!(GraphicsEvo, assists_state), 3864);
        assert_eq!(offset_of!(GraphicsEvo, max_fuel), 3928);
        assert_eq!(offset_of!(GraphicsEvo, use_single_compound), 3936);
        assert_eq!(offset_of!(GraphicsEvo, car_ids), 3940);

        assert_eq!(offset_of!(TyreState, pressure), 8);
        assert_eq!(offset_of!(TyreState, temperature_left), 24);
        assert_eq!(offset_of!(TyreState, compound_front), 36);
        assert_eq!(offset_of!(TyreState, compound_rear), 69);
        assert_eq!(offset_of!(TyreState, normalized_pressure), 104);
        assert_eq!(offset_of!(TyreState, reserved), 128);

        assert_eq!(offset_of!(ElectronicsState, brake_bias), 8);
        assert_eq!(offset_of!(ElectronicsState, engine_map_level), 12);
        assert_eq!(offset_of!(ElectronicsState, turbo_level), 16);
        assert_eq!(offset_of!(ElectronicsState, ers_recharge_map), 24);
        assert_eq!(offset_of!(ElectronicsState, diff_power_level), 31);
        assert_eq!(offset_of!(ElectronicsState, active_performance_mode), 39);

        assert_eq!(offset_of!(SessionState, time_left), 33);
        assert_eq!(offset_of!(SessionState, time_left_ms), 48);
        assert_eq!(offset_of!(SessionState, total_lap), 68);
        assert_eq!(offset_of!(SessionState, current_lap), 72);
        assert_eq!(offset_of!(SessionState, lap_length_km), 84);
        assert_eq!(offset_of!(SessionState, disconnected_from_server), 107);
        assert_eq!(offset_of!(SessionState, reserved), 113);

        assert_eq!(offset_of!(TimingState, delta_current_p), 32);
        assert_eq!(offset_of!(TimingState, last_laptime), 36);
        assert_eq!(offset_of!(TimingState, best_laptime), 72);
        assert_eq!(offset_of!(TimingState, ideal_laptime), 87);
        assert_eq!(offset_of!(TimingState, is_invalid), 117);

        assert_eq!(
            offset_of!(InstrumentationState, display_current_page_index),
            10
        );
        assert_eq!(offset_of!(InstrumentationState, are_headlights_visible), 26);
        assert_eq!(offset_of!(AssistsState, auto_steer), 8);

        assert_eq!(offset_of!(StaticEvo, ac_evo_version), 15);
        assert_eq!(offset_of!(StaticEvo, session), 32);
        assert_eq!(offset_of!(StaticEvo, session_name), 36);
        assert_eq!(offset_of!(StaticEvo, event_id), 69);
        assert_eq!(offset_of!(StaticEvo, starting_grip), 72);
        assert_eq!(offset_of!(StaticEvo, is_static_weather), 84);
        assert_eq!(offset_of!(StaticEvo, number_of_sessions), 88);
        assert_eq!(offset_of!(StaticEvo, nation), 92);
        assert_eq!(offset_of!(StaticEvo, longitude), 128);
        assert_eq!(offset_of!(StaticEvo, track), 136);
        assert_eq!(offset_of!(StaticEvo, track_configuration), 169);
        assert_eq!(offset_of!(StaticEvo, track_length_m), 204);
    }

    #[test]
    fn texts_stop_at_the_first_nul() {
        let field: [u8; 33] = put_text("ks_brands_hatch");
        assert_eq!(text(&field), "ks_brands_hatch");
        let long: [u8; 5] = put_text("abcdefgh");
        assert_eq!(text(&long), "abcd");
        assert_eq!(text(&[0u8; 15]), "");
        // Bytes that are not text do not make the reading fail.
        assert_eq!(text(&[0xff, 0xfe, b'a', 0, b'z']), "\u{fffd}\u{fffd}a");
    }
}
