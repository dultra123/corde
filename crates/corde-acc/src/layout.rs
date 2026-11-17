//! Byte-exact mirror of the three shared memory pages of Assetto Corsa Competizione.
//!
//! Source: the « ACC Shared Memory Documentation » shipped with the game (v1.8.12 and
//! later), cross-checked with two independent open-source readers (`simetry` and
//! `acc_shared_memory_rs`, both MIT). The game declares the three structures with
//! `#pragma pack(4)` and `wchar_t` strings (UTF-16, 2 bytes on Windows):
//!
//! | page | mapping | size | updated |
//! |---|---|---|---|
//! | [`Physics`] | `Local\acpmf_physics` | 800 bytes | every physics step |
//! | [`Graphics`] | `Local\acpmf_graphics` | 1588 bytes | every drawn frame |
//! | [`Static`] | `Local\acpmf_static` | 820 bytes | once per session |
//!
//! The sizes are checked by the tests below: a field added or removed by mistake moves
//! them. Every field is a 32-bit integer, a 32-bit float or a UTF-16 unit, so any bit
//! pattern is valid: the pages are read with `bytemuck::pod_read_unaligned`, without
//! `unsafe`.
//!
//! Many fields are documented « not used in ACC » (inherited from the first Assetto
//! Corsa): they are kept to hold the offsets and are only read for that game.
//!
//! ## The first Assetto Corsa
//!
//! It publishes pages of the same names, in an older and shorter layout (interface « 1.7 »
//! at most). Source: the `sim_info.py` the game ships for its Python applications.
//!
//! * physics: the **beginning** of [`Physics`], up to `local_velocity`;
//! * static: the **beginning** of [`Static`], up to `pit_window_end`;
//! * graphics: the same as [`Graphics`] up to `normalized_car_position`, then **different**
//!   — the player's coordinates alone, no list of cars: [`GraphicsAc`].
//!
//! A mapping is at least one memory page (4096 bytes) filled with zeros after what the
//! game writes: reading it with the longer structures gives zeros in the fields this game
//! does not have. [`GraphicsAc::upgrade`] turns its graphics page into a [`Graphics`], so
//! that one conversion serves both games.

use bytemuck::{AnyBitPattern, Zeroable};

/// `Graphics::status`: what the game is doing.
pub mod status {
    pub const OFF: i32 = 0;
    pub const REPLAY: i32 = 1;
    pub const LIVE: i32 = 2;
    pub const PAUSE: i32 = 3;
}

/// `Graphics::session`.
pub mod session {
    pub const UNKNOWN: i32 = -1;
    pub const PRACTICE: i32 = 0;
    pub const QUALIFY: i32 = 1;
    pub const RACE: i32 = 2;
    pub const HOTLAP: i32 = 3;
    pub const TIME_ATTACK: i32 = 4;
    pub const DRIFT: i32 = 5;
    pub const DRAG: i32 = 6;
    pub const HOTSTINT: i32 = 7;
    pub const HOTLAP_SUPERPOLE: i32 = 8;
}

/// `Graphics::flag`.
pub mod flag {
    pub const NONE: i32 = 0;
    pub const BLUE: i32 = 1;
    pub const YELLOW: i32 = 2;
    pub const BLACK: i32 = 3;
    pub const WHITE: i32 = 4;
    pub const CHECKERED: i32 = 5;
    pub const PENALTY: i32 = 6;
    pub const GREEN: i32 = 7;
    pub const ORANGE: i32 = 8;
}

/// Which of the two games publishes the pages (they share the names of the mappings).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Game {
    /// Assetto Corsa Competizione: interface « 1.8 » and later.
    #[default]
    Competizione,
    /// The first Assetto Corsa: interface « 1.7 » at most.
    AssettoCorsa,
}

impl Game {
    /// The game a static page belongs to, from the version of its interface; `None` when
    /// the page holds no version (the game is starting, or these are not its pages).
    pub fn of(statics: &Static) -> Option<Self> {
        let field = statics.sm_version;
        let v = text(&field);
        let mut parts = v.split('.').map(|p| p.trim().parse::<u32>().ok());
        match (parts.next().flatten(), parts.next().flatten()) {
            (Some(1), Some(minor)) if minor <= 7 => Some(Game::AssettoCorsa),
            (Some(major), Some(_)) if major >= 1 => Some(Game::Competizione),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Game::Competizione => "Assetto Corsa Competizione",
            Game::AssettoCorsa => "Assetto Corsa",
        }
    }
}

/// A lap time the game does not have (`i_last_time`, `i_best_time`…).
pub const NO_TIME_MS: i32 = i32::MAX;

/// Cars the graphics page can describe.
pub const MAX_CARS: usize = 60;

/// `Local\acpmf_physics` — the player's car, every physics step.
#[repr(C, packed(4))]
#[derive(Clone, Copy, AnyBitPattern)]
pub struct Physics {
    /// Step counter: changes when the page was rewritten.
    pub packet_id: i32,
    /// 0..1.
    pub gas: f32,
    /// 0..1.
    pub brake: f32,
    /// Fuel left. The documentation says kg; the game shows the same number in litres.
    pub fuel: f32,
    /// 0 = reverse, 1 = neutral, 2 = first…
    pub gear: i32,
    pub rpm: i32,
    /// −1..1.
    pub steer_angle: f32,
    pub speed_kmh: f32,
    /// World frame.
    pub velocity: [f32; 3],
    /// In g: [lateral, vertical, longitudinal].
    pub acc_g: [f32; 3],
    /// [FL, FR, RL, RR], like every per-wheel array below.
    pub wheel_slip: [f32; 4],
    /// Not sent.
    pub wheel_load: [f32; 4],
    /// psi.
    pub wheels_pressure: [f32; 4],
    /// rad/s.
    pub wheel_angular_speed: [f32; 4],
    /// Not sent.
    pub tyre_wear: [f32; 4],
    /// Not sent.
    pub tyre_dirty_level: [f32; 4],
    /// °C.
    pub tyre_core_temperature: [f32; 4],
    /// Not sent.
    pub camber_rad: [f32; 4],
    /// m.
    pub suspension_travel: [f32; 4],
    /// Not sent.
    pub drs: f32,
    /// Traction control at work (> 0).
    pub tc: f32,
    /// rad.
    pub heading: f32,
    pub pitch: f32,
    pub roll: f32,
    /// Not sent.
    pub cg_height: f32,
    /// [front, rear, left, right, centre].
    pub car_damage: [f32; 5],
    /// Not sent.
    pub number_of_tyres_out: i32,
    pub pit_limiter_on: i32,
    /// ABS at work (> 0).
    pub abs: f32,
    pub kers_charge: f32,
    pub kers_input: f32,
    pub auto_shifter_on: i32,
    /// Not sent.
    pub ride_height: [f32; 2],
    pub turbo_boost: f32,
    pub ballast: f32,
    pub air_density: f32,
    /// °C.
    pub air_temp: f32,
    /// °C.
    pub road_temp: f32,
    /// Car frame, rad/s.
    pub local_angular_vel: [f32; 3],
    pub final_ff: f32,
    pub performance_meter: f32,
    pub engine_brake: i32,
    pub ers_recovery_level: i32,
    pub ers_power_level: i32,
    pub ers_heat_charging: i32,
    pub ers_is_charging: i32,
    pub kers_current_kj: f32,
    pub drs_available: i32,
    pub drs_enabled: i32,
    /// °C.
    pub brake_temp: [f32; 4],
    /// 0..1.
    pub clutch: f32,
    /// Not sent.
    pub tyre_temp_i: [f32; 4],
    /// Not sent.
    pub tyre_temp_m: [f32; 4],
    /// Not sent.
    pub tyre_temp_o: [f32; 4],
    pub is_ai_controlled: i32,
    pub tyre_contact_point: [[f32; 3]; 4],
    pub tyre_contact_normal: [[f32; 3]; 4],
    pub tyre_contact_heading: [[f32; 3]; 4],
    /// Front share of the braking, 0..1, **before** the per-car offset of the
    /// documentation's appendix 4 (the number shown in the car differs by a constant).
    pub brake_bias: f32,
    /// Car frame, m/s.
    pub local_velocity: [f32; 3],
    pub p2p_activations: i32,
    pub p2p_status: i32,
    /// Not sent.
    pub current_max_rpm: i32,
    pub mz: [f32; 4],
    pub fx: [f32; 4],
    pub fy: [f32; 4],
    pub slip_ratio: [f32; 4],
    pub slip_angle: [f32; 4],
    /// Not sent (see `tc`).
    pub tc_in_action: i32,
    /// Not sent (see `abs`).
    pub abs_in_action: i32,
    /// Not sent.
    pub suspension_damage: [f32; 4],
    /// Not sent.
    pub tyre_temp: [f32; 4],
    /// °C.
    pub water_temp: f32,
    pub brake_pressure: [f32; 4],
    pub front_brake_compound: i32,
    pub rear_brake_compound: i32,
    pub pad_life: [f32; 4],
    pub disc_life: [f32; 4],
    pub ignition_on: i32,
    pub starter_engine_on: i32,
    pub is_engine_running: i32,
    pub kerb_vibration: f32,
    pub slip_vibrations: f32,
    pub g_vibrations: f32,
    pub abs_vibrations: f32,
}

/// `Local\acpmf_graphics` — session, timing and the cars on track, every drawn frame.
#[repr(C, packed(4))]
#[derive(Clone, Copy, AnyBitPattern)]
pub struct Graphics {
    pub packet_id: i32,
    /// See [`status`].
    pub status: i32,
    /// See [`session`].
    pub session: i32,
    pub current_time: [u16; 15],
    pub last_time: [u16; 15],
    pub best_time: [u16; 15],
    pub split: [u16; 15],
    pub completed_laps: i32,
    /// The player's place, 1 = leader.
    pub position: i32,
    /// ms.
    pub i_current_time: i32,
    /// ms, [`NO_TIME_MS`] when there is none.
    pub i_last_time: i32,
    /// ms, [`NO_TIME_MS`] when there is none.
    pub i_best_time: i32,
    /// ms.
    pub session_time_left: f32,
    pub distance_traveled: f32,
    /// In the pit box.
    pub is_in_pit: i32,
    /// 0-based.
    pub current_sector_index: i32,
    /// ms.
    pub last_sector_time: i32,
    pub number_of_laps: i32,
    pub tyre_compound: [u16; 33],
    pub replay_time_multiplier: f32,
    /// 0..1 along the lap.
    pub normalized_car_position: f32,
    pub active_cars: i32,
    /// World frame, one entry per car on track (see `car_id`).
    pub car_coordinates: [[f32; 3]; MAX_CARS],
    pub car_id: [i32; MAX_CARS],
    pub player_car_id: i32,
    pub penalty_time: f32,
    /// See [`flag`].
    pub flag: i32,
    pub penalty: i32,
    pub ideal_line_on: i32,
    pub is_in_pit_lane: i32,
    pub surface_grip: f32,
    pub mandatory_pit_done: i32,
    /// m/s.
    pub wind_speed: f32,
    /// rad.
    pub wind_direction: f32,
    pub is_setup_menu_visible: i32,
    pub main_display_index: i32,
    pub secondary_display_index: i32,
    /// Traction control level.
    pub tc: i32,
    pub tc_cut: i32,
    pub engine_map: i32,
    /// ABS level.
    pub abs: i32,
    /// Litres per lap.
    pub fuel_used_per_lap: f32,
    pub rain_lights: i32,
    pub flashing_lights: i32,
    pub lights_stage: i32,
    pub exhaust_temperature: f32,
    pub wiper_lv: i32,
    pub driver_stint_total_time_left: i32,
    pub driver_stint_time_left: i32,
    pub rain_tyres: i32,
    pub session_index: i32,
    pub used_fuel: f32,
    pub delta_lap_time: [u16; 15],
    /// ms.
    pub i_delta_lap_time: i32,
    pub estimated_lap_time: [u16; 15],
    /// ms.
    pub i_estimated_lap_time: i32,
    pub is_delta_positive: i32,
    pub i_split: i32,
    pub is_valid_lap: i32,
    pub fuel_estimated_laps: f32,
    pub track_status: [u16; 33],
    pub missing_mandatory_pits: i32,
    /// Time of day, s.
    pub clock: f32,
    pub direction_lights_left: i32,
    pub direction_lights_right: i32,
    pub global_yellow: i32,
    pub global_yellow1: i32,
    pub global_yellow2: i32,
    pub global_yellow3: i32,
    pub global_white: i32,
    pub global_green: i32,
    pub global_chequered: i32,
    pub global_red: i32,
    pub mfd_tyre_set: i32,
    pub mfd_fuel_to_add: f32,
    pub mfd_tyre_pressure_lf: f32,
    pub mfd_tyre_pressure_rf: f32,
    pub mfd_tyre_pressure_lr: f32,
    pub mfd_tyre_pressure_rr: f32,
    /// 0 green, 1 fast, 2 optimum, 3 greasy, 4 damp, 5 wet, 6 flooded.
    pub track_grip_status: i32,
    /// 0 none, 1 drizzle, 2 light, 3 medium, 4 heavy, 5 thunderstorm.
    pub rain_intensity: i32,
    pub rain_intensity_in_10m: i32,
    pub rain_intensity_in_30m: i32,
    pub current_tyre_set: i32,
    pub strategy_tyre_set: i32,
    /// ms to the car ahead.
    pub gap_ahead: i32,
    /// ms to the car behind.
    pub gap_behind: i32,
}

/// `Local\acpmf_static` — what does not change during a session.
#[repr(C, packed(4))]
#[derive(Clone, Copy, AnyBitPattern)]
pub struct Static {
    /// Version of this interface: « 1.8 » and later for Competizione; the first
    /// Assetto Corsa stops at « 1.7 ».
    pub sm_version: [u16; 15],
    pub ac_version: [u16; 15],
    pub number_of_sessions: i32,
    pub num_cars: i32,
    pub car_model: [u16; 33],
    pub track: [u16; 33],
    pub player_name: [u16; 33],
    pub player_surname: [u16; 33],
    pub player_nick: [u16; 33],
    pub sector_count: i32,
    pub max_torque: f32,
    pub max_power: f32,
    pub max_rpm: i32,
    /// Litres.
    pub max_fuel: f32,
    pub suspension_max_travel: [f32; 4],
    pub tyre_radius: [f32; 4],
    pub max_turbo_boost: f32,
    pub deprecated_1: f32,
    pub deprecated_2: f32,
    pub penalties_enabled: i32,
    pub aid_fuel_rate: f32,
    pub aid_tyre_rate: f32,
    pub aid_mechanical_damage: f32,
    pub aid_allow_tyre_blankets: f32,
    pub aid_stability: f32,
    pub aid_auto_clutch: i32,
    pub aid_auto_blip: i32,
    pub has_drs: i32,
    pub has_ers: i32,
    pub has_kers: i32,
    pub kers_max_j: f32,
    pub engine_brake_settings_count: i32,
    pub ers_power_controller_count: i32,
    /// Not used in ACC (0).
    pub track_spline_length: f32,
    pub track_configuration: [u16; 33],
    pub ers_max_j: f32,
    pub is_timed_race: i32,
    pub has_extra_lap: i32,
    pub car_skin: [u16; 33],
    pub reversed_grid_positions: i32,
    pub pit_window_start: i32,
    pub pit_window_end: i32,
    pub is_online: i32,
    pub dry_tyres_name: [u16; 33],
    pub wet_tyres_name: [u16; 33],
}

/// `Local\acpmf_graphics` of the **first Assetto Corsa** (see the module documentation).
#[repr(C, packed(4))]
#[derive(Clone, Copy, AnyBitPattern)]
pub struct GraphicsAc {
    pub packet_id: i32,
    pub status: i32,
    pub session: i32,
    pub current_time: [u16; 15],
    pub last_time: [u16; 15],
    pub best_time: [u16; 15],
    pub split: [u16; 15],
    pub completed_laps: i32,
    pub position: i32,
    pub i_current_time: i32,
    pub i_last_time: i32,
    pub i_best_time: i32,
    pub session_time_left: f32,
    pub distance_traveled: f32,
    pub is_in_pit: i32,
    pub current_sector_index: i32,
    pub last_sector_time: i32,
    pub number_of_laps: i32,
    pub tyre_compound: [u16; 33],
    pub replay_time_multiplier: f32,
    pub normalized_car_position: f32,
    /// The player's car, world frame.
    pub car_coordinates: [f32; 3],
    pub penalty_time: f32,
    pub flag: i32,
    pub ideal_line_on: i32,
    pub is_in_pit_lane: i32,
    pub surface_grip: f32,
    pub mandatory_pit_done: i32,
    /// Added by later versions of the game (zeros before).
    pub wind_speed: f32,
    pub wind_direction: f32,
}

/// Identifier given to the player's car when the game names none (the first Assetto Corsa).
pub const LONE_CAR_ID: i32 = 0;

impl GraphicsAc {
    /// The same page as a [`Graphics`]: what both games publish is copied, the player's
    /// car becomes the only car of the list, and what this game does not publish stays at
    /// zero — except the validity of the lap, which it never reports: every lap is valid.
    pub fn upgrade(&self) -> Graphics {
        let mut g = Graphics::zeroed();
        g.packet_id = self.packet_id;
        g.status = self.status;
        g.session = self.session;
        g.current_time = self.current_time;
        g.last_time = self.last_time;
        g.best_time = self.best_time;
        g.split = self.split;
        g.completed_laps = self.completed_laps;
        g.position = self.position;
        g.i_current_time = self.i_current_time;
        g.i_last_time = self.i_last_time;
        g.i_best_time = self.i_best_time;
        g.session_time_left = self.session_time_left;
        g.distance_traveled = self.distance_traveled;
        g.is_in_pit = self.is_in_pit;
        g.current_sector_index = self.current_sector_index;
        g.last_sector_time = self.last_sector_time;
        g.number_of_laps = self.number_of_laps;
        g.tyre_compound = self.tyre_compound;
        g.replay_time_multiplier = self.replay_time_multiplier;
        g.normalized_car_position = self.normalized_car_position;
        g.active_cars = 1;
        let mut cars = [[0.0f32; 3]; MAX_CARS];
        cars[0] = self.car_coordinates;
        g.car_coordinates = cars;
        let mut ids = [0i32; MAX_CARS];
        ids[0] = LONE_CAR_ID;
        g.car_id = ids;
        g.player_car_id = LONE_CAR_ID;
        g.penalty_time = self.penalty_time;
        g.flag = self.flag;
        g.ideal_line_on = self.ideal_line_on;
        g.is_in_pit_lane = self.is_in_pit_lane;
        g.surface_grip = self.surface_grip;
        g.mandatory_pit_done = self.mandatory_pit_done;
        g.wind_speed = self.wind_speed;
        g.wind_direction = self.wind_direction;
        g.is_valid_lap = 1;
        g
    }
}

pub const PHYSICS_SIZE: usize = std::mem::size_of::<Physics>();
pub const GRAPHICS_AC_SIZE: usize = std::mem::size_of::<GraphicsAc>();
pub const GRAPHICS_SIZE: usize = std::mem::size_of::<Graphics>();
pub const STATIC_SIZE: usize = std::mem::size_of::<Static>();

impl Physics {
    pub fn zeroed() -> Self {
        Zeroable::zeroed()
    }
}
impl Graphics {
    pub fn zeroed() -> Self {
        Zeroable::zeroed()
    }
}
impl Static {
    pub fn zeroed() -> Self {
        Zeroable::zeroed()
    }
}

/// A fixed UTF-16 field of the game, up to its first NUL.
pub fn text(units: &[u16]) -> String {
    let end = units.iter().position(|u| *u == 0).unwrap_or(units.len());
    String::from_utf16_lossy(&units[..end]).trim().to_string()
}

/// Writes `s` into a fixed UTF-16 field (NUL-terminated, cut to fit). For the fakes.
pub fn put_text<const N: usize>(s: &str) -> [u16; N] {
    let mut out = [0u16; N];
    for (slot, unit) in out.iter_mut().take(N - 1).zip(s.encode_utf16()) {
        *slot = unit;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::offset_of;

    /// The sizes of the documented structures. 800 and 1588 are also the sizes an
    /// independent reader maps; 820 follows from the documented fields of the static page
    /// (the same reader parses it up to offset 820).
    #[test]
    fn the_pages_have_the_documented_sizes() {
        assert_eq!(PHYSICS_SIZE, 800);
        assert_eq!(GRAPHICS_SIZE, 1588);
        assert_eq!(STATIC_SIZE, 820);
    }

    /// A few offsets worked out by hand from the documentation (4-byte packing: a
    /// UTF-16 field of an odd length is followed by 2 bytes of padding).
    #[test]
    fn known_fields_sit_at_their_offsets() {
        assert_eq!(offset_of!(Physics, packet_id), 0);
        assert_eq!(offset_of!(Physics, gear), 16);
        assert_eq!(offset_of!(Physics, speed_kmh), 28);
        assert_eq!(offset_of!(Physics, velocity), 32);
        assert_eq!(offset_of!(Physics, wheel_slip), 56);
        // 4 × 15 UTF-16 units = 120 bytes after the three first integers.
        assert_eq!(offset_of!(Graphics, current_time), 12);
        assert_eq!(offset_of!(Graphics, completed_laps), 132);
        assert_eq!(offset_of!(Graphics, i_current_time), 140);
        assert_eq!(offset_of!(Graphics, number_of_laps), 172);
        // 33 units = 66 bytes, then 2 of padding before the float.
        assert_eq!(offset_of!(Graphics, tyre_compound), 176);
        assert_eq!(offset_of!(Graphics, replay_time_multiplier), 244);
        assert_eq!(offset_of!(Graphics, car_coordinates), 256);
        assert_eq!(offset_of!(Graphics, car_id), 256 + 60 * 12);
        assert_eq!(offset_of!(Graphics, player_car_id), 256 + 60 * 12 + 60 * 4);
        assert_eq!(offset_of!(Graphics, gap_behind), 1584);
        assert_eq!(offset_of!(Static, sm_version), 0);
        assert_eq!(offset_of!(Static, number_of_sessions), 60);
        assert_eq!(offset_of!(Static, car_model), 68);
        assert_eq!(offset_of!(Static, track), 134);
        // 5 × 66 bytes from 68 = 398, padded to 400.
        assert_eq!(offset_of!(Static, sector_count), 400);
        assert_eq!(offset_of!(Static, track_configuration), 524);
        assert_eq!(offset_of!(Static, car_skin), 604);
        assert_eq!(offset_of!(Static, is_online), 684);
        assert_eq!(offset_of!(Static, dry_tyres_name), 688);
        assert_eq!(offset_of!(Static, wet_tyres_name), 754);
    }

    /// The first Assetto Corsa: the sizes of the structures of its `sim_info.py`, worked
    /// out by hand (4-byte packing), and where its pages stop inside ours.
    #[test]
    fn the_first_assetto_corsa_fits_where_its_reference_says() {
        // graphics: identical up to the position on the lap, then 3 + 1 floats, 3 ints,
        // 1 float, 1 int (296 bytes), and the two wind floats of later versions.
        assert_eq!(offset_of!(GraphicsAc, normalized_car_position), 248);
        assert_eq!(
            offset_of!(GraphicsAc, normalized_car_position),
            offset_of!(Graphics, normalized_car_position)
        );
        assert_eq!(offset_of!(GraphicsAc, car_coordinates), 252);
        assert_eq!(offset_of!(GraphicsAc, penalty_time), 264);
        assert_eq!(offset_of!(GraphicsAc, mandatory_pit_done), 284);
        assert_eq!(GRAPHICS_AC_SIZE, 296);
        // physics: its 60 fields end with the local velocity, 580 bytes into ours.
        assert_eq!(offset_of!(Physics, local_velocity) + 12, 580);
        assert_eq!(offset_of!(Physics, p2p_activations), 580);
        // static: its 42 fields end with the pit window, 684 bytes into ours.
        assert_eq!(offset_of!(Static, pit_window_end) + 4, 684);
    }

    #[test]
    fn a_page_of_the_first_assetto_corsa_becomes_one_of_ours() {
        let mut ac = <GraphicsAc as Zeroable>::zeroed();
        ac.packet_id = 12;
        ac.status = status::LIVE;
        ac.session = session::RACE;
        ac.completed_laps = 3;
        ac.position = 2;
        ac.i_current_time = 41_500;
        ac.i_last_time = 95_210;
        ac.i_best_time = 94_870;
        ac.current_sector_index = 1;
        ac.normalized_car_position = 0.43;
        ac.car_coordinates = [10.0, 2.0, -30.0];
        ac.is_in_pit_lane = 1;
        ac.wind_speed = 4.0;
        let g = ac.upgrade();
        assert_eq!({ g.packet_id }, 12);
        assert_eq!({ g.status }, status::LIVE);
        assert_eq!({ g.session }, session::RACE);
        assert_eq!({ g.completed_laps }, 3);
        assert_eq!({ g.position }, 2);
        assert_eq!({ g.i_current_time }, 41_500);
        assert_eq!({ g.i_last_time }, 95_210);
        assert_eq!({ g.i_best_time }, 94_870);
        assert_eq!({ g.current_sector_index }, 1);
        assert_eq!({ g.normalized_car_position }, 0.43);
        // The player's car is the only one of the list.
        assert_eq!({ g.active_cars }, 1);
        assert_eq!({ g.player_car_id }, LONE_CAR_ID);
        let cars = g.car_coordinates;
        assert_eq!(cars[0], [10.0, 2.0, -30.0]);
        assert_eq!({ g.is_in_pit_lane }, 1);
        assert_eq!({ g.wind_speed }, 4.0);
        // Not published by this game.
        assert_eq!({ g.is_valid_lap }, 1);
        assert_eq!({ g.session_index }, 0);
        assert_eq!({ g.gap_ahead }, 0);
        assert_eq!({ g.is_setup_menu_visible }, 0);
    }

    #[test]
    fn texts_stop_at_the_first_nul() {
        let field: [u16; 33] = put_text("monza");
        assert_eq!(text(&field), "monza");
        let long: [u16; 15] = put_text("a text that is far too long");
        assert_eq!(text(&long), "a text that is");
        assert_eq!(text(&[0u16; 15]), "");
    }
}
