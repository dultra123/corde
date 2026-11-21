//! Plausible pages of Assetto Corsa EVO, for the tests and the development tools: a car
//! lapping a circuit at a constant speed, with the timing the game would publish.
//!
//! The pages are built as bytes, field by field at the offsets of [`super::layout`] and of
//! the physics page — the way the game writes them — then read back like the real ones.

use super::layout::{
    session, status, ElectronicsState, GraphicsEvo, InstrumentationState, SessionState, StaticEvo,
    TimingState, TyreState, GRAPHICS_EVO_SIZE, STATIC_EVO_SIZE,
};
use crate::layout::{Physics, PHYSICS_SIZE};
use crate::reader::RawPages;
use std::mem::{offset_of, size_of};

fn put_i32(page: &mut [u8], at: usize, v: i32) {
    page[at..at + 4].copy_from_slice(&v.to_le_bytes());
}

fn put_f32(page: &mut [u8], at: usize, v: f32) {
    page[at..at + 4].copy_from_slice(&v.to_le_bytes());
}

fn put_u64(page: &mut [u8], at: usize, v: u64) {
    page[at..at + 8].copy_from_slice(&v.to_le_bytes());
}

/// A text of one byte a character, cut to leave the final NUL of a field of `len` bytes.
fn put_text(page: &mut [u8], at: usize, len: usize, s: &str) {
    for (i, b) in s.bytes().take(len - 1).enumerate() {
        page[at + i] = b;
    }
}

/// The identifiers of the cars of the fake session: slot → the two halves.
const CARS: [(u64, u64, f32); 3] = [
    (0x1111_2222_3333_4444, 0xaaaa, 0.4),
    (0x5555_6666_7777_8888, 0xbbbb, 0.0),
    (0x9999_aaaa_bbbb_cccc, 0xcccc, -0.7),
];
/// The slot of the player in [`CARS`].
pub const PLAYER_SLOT: usize = 1;

/// A car lapping a circuit.
#[derive(Debug, Clone)]
pub struct FakeEvoSession {
    /// Identifier of the circuit, as the game writes it.
    pub track: String,
    /// Layout of the circuit, empty when it has one.
    pub track_configuration: String,
    /// Identifier of the car.
    pub car: String,
    /// `StaticEvo::sm_version`.
    pub sm_version: String,
    pub lap_time_s: f64,
    pub length_m: f64,
    /// `StaticEvo::session` and `session_id`.
    pub session: i32,
    pub session_id: u8,
    /// `GraphicsEvo::status`.
    pub status: i32,
    /// The game publishes the time of a lap this long after its counter moved.
    pub last_time_delay_s: f64,
    /// By how much the published times differ from lap to lap, ms.
    pub time_step_ms: i32,
    /// The game writes which car sits in which slot (builds since April 2026).
    pub car_ids: bool,
}

impl Default for FakeEvoSession {
    fn default() -> Self {
        Self {
            track: "ks_brands_hatch".into(),
            track_configuration: "gp".into(),
            car: "ks_bmw_m4_gt3".into(),
            sm_version: "1.1".into(),
            lap_time_s: 84.0,
            length_m: 3908.0,
            session: session::HOT_STINT,
            session_id: 0,
            status: status::LIVE,
            last_time_delay_s: 0.0,
            time_step_ms: 0,
            car_ids: true,
        }
    }
}

impl FakeEvoSession {
    /// The time the game publishes for the `k`-th lap (1: the first), ms.
    pub fn lap_ms(&self, k: i32) -> i32 {
        (self.lap_time_s * 1000.0).round() as i32 + self.time_step_ms * (k % 5)
    }

    /// The pages `t_s` seconds after the car crossed the line for the first time.
    pub fn at(&self, t_s: f64) -> RawPages {
        let t = t_s.max(0.0);
        let laps = (t / self.lap_time_s).floor();
        let into = t - laps * self.lap_time_s;
        let fraction = (into / self.lap_time_s) as f32;
        let speed_ms = (self.length_m / self.lap_time_s) as f32;
        // The time of the lap just ended appears `last_time_delay_s` after the line.
        let timed_laps = if into >= self.last_time_delay_s {
            laps
        } else {
            laps - 1.0
        };
        let last_ms = if timed_laps >= 1.0 {
            self.lap_ms(timed_laps as i32)
        } else {
            0
        };

        let mut physics = vec![0u8; PHYSICS_SIZE];
        let p = &mut physics;
        put_i32(p, offset_of!(Physics, packet_id), (t * 333.0) as i32 + 1);
        put_f32(p, offset_of!(Physics, gas), 0.8);
        put_f32(p, offset_of!(Physics, brake), 0.0);
        put_i32(p, offset_of!(Physics, gear), 5); // fourth
        put_i32(p, offset_of!(Physics, rpm), 6500);
        put_f32(p, offset_of!(Physics, speed_kmh), speed_ms * 3.6);
        // As the game is taken to write them in a right-hand bend, accelerating:
        // [lateral, longitudinal, vertical without gravity].
        put_f32(p, offset_of!(Physics, acc_g), -1.2);
        put_f32(p, offset_of!(Physics, acc_g) + 4, 0.3);
        put_f32(p, offset_of!(Physics, acc_g) + 8, 0.02);
        put_f32(p, offset_of!(Physics, local_angular_vel) + 4, -0.2);
        put_f32(p, offset_of!(Physics, local_velocity) + 8, speed_ms);
        put_f32(p, offset_of!(Physics, heading), 0.5);
        put_f32(p, offset_of!(Physics, air_temp), 21.0);
        put_f32(p, offset_of!(Physics, road_temp), 29.0);
        put_f32(p, offset_of!(Physics, water_temp), 88.0);
        put_f32(p, offset_of!(Physics, brake_bias), 0.57);
        put_i32(p, offset_of!(Physics, current_max_rpm), 7400);
        // Metres for the front axle, millimetres for the rear one: both are seen.
        put_f32(p, offset_of!(Physics, ride_height), 0.058);
        put_f32(p, offset_of!(Physics, ride_height) + 4, 71.0);
        for i in 0..4 {
            let f = i as f32;
            put_f32(
                p,
                offset_of!(Physics, tyre_core_temperature) + 4 * i,
                80.0 + f,
            );
            put_f32(
                p,
                offset_of!(Physics, wheel_load) + 4 * i,
                3200.0 + 100.0 * f,
            );
            put_f32(p, offset_of!(Physics, suspension_travel) + 4 * i, 0.03);
            // The game gives a right wheel the other sign.
            let camber = if i % 2 == 0 { -0.05 } else { 0.05 };
            put_f32(p, offset_of!(Physics, camber_rad) + 4 * i, camber);
            put_f32(p, offset_of!(Physics, wheel_angular_speed) + 4 * i, 130.0);
            put_f32(p, offset_of!(Physics, slip_ratio) + 4 * i, 0.02);
        }

        let mut graphics = vec![0u8; GRAPHICS_EVO_SIZE];
        let g = &mut graphics;
        put_i32(g, offset_of!(GraphicsEvo, packet_id), (t * 60.0) as i32 + 1);
        put_i32(g, offset_of!(GraphicsEvo, status), self.status);
        let (me_a, me_b, _) = CARS[PLAYER_SLOT];
        for at in [
            offset_of!(GraphicsEvo, focused_car_id),
            offset_of!(GraphicsEvo, player_car_id),
        ] {
            put_u64(g, at, me_a);
            put_u64(g, at + 8, me_b);
        }
        g[offset_of!(GraphicsEvo, rpm)..][..2].copy_from_slice(&6500u16.to_le_bytes());
        g[offset_of!(GraphicsEvo, is_ignition_on)] = 1;
        g[offset_of!(GraphicsEvo, is_engine_running)] = 1;
        g[offset_of!(GraphicsEvo, gear_int)..][..2].copy_from_slice(&5i16.to_le_bytes());
        put_f32(g, offset_of!(GraphicsEvo, gas_percent), 0.8);
        put_f32(g, offset_of!(GraphicsEvo, clutch_percent), 0.0);
        put_f32(g, offset_of!(GraphicsEvo, steering_percent), 0.05);
        put_f32(g, offset_of!(GraphicsEvo, oil_temperature_c), 104.0);
        put_i32(g, offset_of!(GraphicsEvo, steer_degrees), 14);
        put_i32(g, offset_of!(GraphicsEvo, time_of_day_hours), 14);
        put_i32(g, offset_of!(GraphicsEvo, time_of_day_minutes), 30);
        put_i32(g, offset_of!(GraphicsEvo, delta_time_ms), -120);
        put_i32(
            g,
            offset_of!(GraphicsEvo, current_lap_time_ms),
            (into * 1000.0) as i32,
        );
        put_i32(
            g,
            offset_of!(GraphicsEvo, predicted_lap_time_ms),
            (self.lap_time_s * 1000.0).round() as i32,
        );
        put_f32(
            g,
            offset_of!(GraphicsEvo, fuel_liter_current_quantity),
            (60.0 - t / self.lap_time_s * 2.4) as f32,
        );
        put_f32(g, offset_of!(GraphicsEvo, current_torque), 410.0);
        for i in 0..4 {
            let f = i as f32;
            let at = offset_of!(GraphicsEvo, tyre_lf) + i * size_of::<TyreState>();
            put_f32(g, at + offset_of!(TyreState, pressure), 26.0 + 0.1 * f);
            put_f32(g, at + offset_of!(TyreState, temperature_c), 80.0 + f);
            put_f32(
                g,
                at + offset_of!(TyreState, brake_temperature_c),
                400.0 + 10.0 * f,
            );
            put_f32(g, at + offset_of!(TyreState, brake_pressure), 12.0);
            put_f32(g, at + offset_of!(TyreState, temperature_left), 70.0 + f);
            put_f32(g, at + offset_of!(TyreState, temperature_center), 80.0 + f);
            put_f32(g, at + offset_of!(TyreState, temperature_right), 90.0 + f);
            put_text(g, at + offset_of!(TyreState, compound_front), 33, "Medium");
            put_text(g, at + offset_of!(TyreState, compound_rear), 33, "Soft");
        }
        put_f32(g, offset_of!(GraphicsEvo, npos), fraction);
        put_i32(
            g,
            offset_of!(GraphicsEvo, car_location),
            super::layout::location::TRACK,
        );
        // Lights and wipers.
        let lights = offset_of!(GraphicsEvo, instrumentation);
        g[lights + offset_of!(InstrumentationState, main_light_stage)] = 1;
        g[lights + offset_of!(InstrumentationState, wiper_level)] = 2;
        // The settings of the wheel, and their maximum.
        for (at, tc, cut, abs, map) in [
            (offset_of!(GraphicsEvo, electronics), 3u8, 2u8, 4u8, 1u8),
            (offset_of!(GraphicsEvo, electronics_max_limit), 11, 6, 9, 5),
        ] {
            g[at + offset_of!(ElectronicsState, tc_level)] = tc;
            g[at + offset_of!(ElectronicsState, tc_cut_level)] = cut;
            g[at + offset_of!(ElectronicsState, abs_level)] = abs;
            g[at + offset_of!(ElectronicsState, engine_map_level)] = map;
        }
        put_f32(
            g,
            offset_of!(GraphicsEvo, electronics) + offset_of!(ElectronicsState, brake_bias),
            0.57,
        );
        put_i32(g, offset_of!(GraphicsEvo, total_lap_count), laps as i32);
        put_i32(g, offset_of!(GraphicsEvo, current_pos), 4);
        put_i32(g, offset_of!(GraphicsEvo, total_drivers), CARS.len() as i32);
        put_i32(g, offset_of!(GraphicsEvo, last_laptime_ms), last_ms);
        put_i32(g, offset_of!(GraphicsEvo, best_laptime_ms), last_ms);
        put_i32(g, offset_of!(GraphicsEvo, max_gears), 6);
        let state = offset_of!(GraphicsEvo, session_state);
        put_text(
            g,
            state + offset_of!(SessionState, phase_name),
            33,
            "Session",
        );
        put_i32(
            g,
            state + offset_of!(SessionState, time_left_ms),
            ((3600.0 - t) * 1000.0) as i32,
        );
        put_i32(
            g,
            state + offset_of!(SessionState, current_lap),
            laps as i32 + 1,
        );
        put_f32(
            g,
            state + offset_of!(SessionState, lap_length_km),
            (self.length_m / 1000.0) as f32,
        );
        g[offset_of!(GraphicsEvo, timing_state) + offset_of!(TimingState, is_invalid)] = 0;
        put_text(g, offset_of!(GraphicsEvo, driver_name), 33, "Alex");
        put_text(g, offset_of!(GraphicsEvo, driver_surname), 33, "Martin");
        put_text(g, offset_of!(GraphicsEvo, car_model), 33, &self.car);
        g[offset_of!(GraphicsEvo, is_valid_lap)] = 1;
        let radius = (self.length_m / std::f64::consts::TAU) as f32;
        let angle = fraction * std::f32::consts::TAU;
        for (slot, (a, b, lead)) in CARS.into_iter().enumerate() {
            let at = offset_of!(GraphicsEvo, car_coordinates) + 12 * slot;
            put_f32(g, at, radius * (angle + lead).cos());
            put_f32(g, at + 8, radius * (angle + lead).sin());
            if self.car_ids {
                let at = offset_of!(GraphicsEvo, car_ids) + 16 * slot;
                put_u64(g, at, a);
                put_u64(g, at + 8, b);
            }
        }
        put_f32(g, offset_of!(GraphicsEvo, gap_ahead), 1.85);
        put_f32(g, offset_of!(GraphicsEvo, gap_behind), 0.64);
        g[offset_of!(GraphicsEvo, active_cars)] = CARS.len() as u8;
        put_f32(g, offset_of!(GraphicsEvo, max_fuel), 120.0);

        let mut statics = vec![0u8; STATIC_EVO_SIZE];
        let s = &mut statics;
        put_text(s, offset_of!(StaticEvo, sm_version), 15, &self.sm_version);
        put_text(s, offset_of!(StaticEvo, ac_evo_version), 15, "0.4.2");
        put_i32(s, offset_of!(StaticEvo, session), self.session);
        put_text(s, offset_of!(StaticEvo, session_name), 33, "Hot stint");
        s[offset_of!(StaticEvo, session_id)] = self.session_id;
        put_i32(s, offset_of!(StaticEvo, starting_grip), 2);
        put_f32(
            s,
            offset_of!(StaticEvo, starting_ambient_temperature_c),
            20.0,
        );
        put_f32(
            s,
            offset_of!(StaticEvo, starting_ground_temperature_c),
            27.0,
        );
        put_i32(s, offset_of!(StaticEvo, number_of_sessions), 1);
        put_text(s, offset_of!(StaticEvo, nation), 33, "GBR");
        put_text(s, offset_of!(StaticEvo, track), 33, &self.track);
        put_text(
            s,
            offset_of!(StaticEvo, track_configuration),
            33,
            &self.track_configuration,
        );
        put_f32(
            s,
            offset_of!(StaticEvo, track_length_m),
            self.length_m as f32,
        );
        RawPages {
            physics,
            graphics,
            statics,
        }
    }
}
