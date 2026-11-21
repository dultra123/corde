//! Plausible pages of the game, for the tests and the development tools: a car lapping a
//! circuit at a constant speed, with the timing the game would publish.
//!
//! The pages are built as bytes, field by field at the offsets of [`crate::layout`] — the
//! way the game writes them — then read back like the real ones.

use crate::layout::{
    status, Game, Graphics, GraphicsAc, Physics, Static, GRAPHICS_SIZE, NO_TIME_MS, PHYSICS_SIZE,
    STATIC_SIZE,
};
use crate::reader::RawPages;
use std::mem::offset_of;

fn put_i32(page: &mut [u8], at: usize, v: i32) {
    page[at..at + 4].copy_from_slice(&v.to_le_bytes());
}

fn put_f32(page: &mut [u8], at: usize, v: f32) {
    page[at..at + 4].copy_from_slice(&v.to_le_bytes());
}

fn put_text(page: &mut [u8], at: usize, units: usize, s: &str) {
    for (i, u) in s.encode_utf16().take(units - 1).enumerate() {
        page[at + 2 * i..at + 2 * i + 2].copy_from_slice(&u.to_le_bytes());
    }
}

/// A car lapping a circuit.
#[derive(Debug, Clone)]
pub struct FakeSession {
    /// Which game writes the pages: its graphics page has its own layout.
    pub game: Game,
    /// Identifier of the circuit, as the game writes it (« monza »).
    pub track: String,
    /// Layout of the circuit, for the first Assetto Corsa (« layout_gp_a »), else empty.
    pub track_configuration: String,
    /// Identifier of the car (« bmw_m4_gt3 »).
    pub car: String,
    /// `Static::sm_version`: « 1.9 » for Competizione, « 1.7 » for the first Assetto Corsa.
    pub sm_version: String,
    pub lap_time_s: f64,
    /// Real length of the lap driven (the game does not publish it).
    pub length_m: f64,
    /// `Graphics::session` and `session_index`.
    pub session: i32,
    pub session_index: i32,
    /// `Graphics::status`.
    pub status: i32,
    /// The game publishes the time of a lap this long after its counter moved.
    pub last_time_delay_s: f64,
    /// By how much the published times differ from lap to lap, ms (0: every lap gets
    /// exactly the same time).
    pub time_step_ms: i32,
}

impl Default for FakeSession {
    fn default() -> Self {
        Self {
            game: Game::Competizione,
            track_configuration: String::new(),
            track: "monza".into(),
            car: "bmw_m4_gt3".into(),
            sm_version: "1.9".into(),
            lap_time_s: 108.0,
            length_m: 5793.0,
            session: crate::layout::session::PRACTICE,
            session_index: 0,
            status: status::LIVE,
            last_time_delay_s: 0.0,
            time_step_ms: 0,
        }
    }
}

impl FakeSession {
    /// A session of the first Assetto Corsa: its interface, its identifiers, a circuit
    /// whose length the game publishes.
    pub fn assetto_corsa() -> Self {
        Self {
            game: Game::AssettoCorsa,
            sm_version: "1.7".into(),
            track: "ks_nurburgring".into(),
            track_configuration: "layout_gp_a".into(),
            car: "ks_ferrari_488_gt3".into(),
            lap_time_s: 118.0,
            length_m: 5137.0,
            ..Self::default()
        }
    }

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
        let lap_ms = (self.lap_time_s * 1000.0).round() as i32;
        // The time of the lap just ended appears `last_time_delay_s` after the line.
        let timed_laps = if into >= self.last_time_delay_s {
            laps
        } else {
            laps - 1.0
        };
        let last_ms = if timed_laps >= 1.0 {
            self.lap_ms(timed_laps as i32)
        } else {
            NO_TIME_MS
        };

        let mut physics = vec![0u8; PHYSICS_SIZE];
        put_i32(
            &mut physics,
            offset_of!(Physics, packet_id),
            (t * 333.0) as i32 + 1,
        );
        put_f32(&mut physics, offset_of!(Physics, gas), 0.8);
        put_f32(&mut physics, offset_of!(Physics, brake), 0.0);
        put_f32(
            &mut physics,
            offset_of!(Physics, fuel),
            (60.0 - t / self.lap_time_s * 2.9) as f32,
        );
        put_i32(&mut physics, offset_of!(Physics, gear), 5); // fourth
        put_i32(&mut physics, offset_of!(Physics, rpm), 6500);
        put_f32(&mut physics, offset_of!(Physics, steer_angle), 0.05);
        // As the game writes them in a right-hand bend, clutch pedal released.
        put_f32(&mut physics, offset_of!(Physics, acc_g), -1.2);
        put_f32(&mut physics, offset_of!(Physics, acc_g) + 8, 0.3);
        put_f32(
            &mut physics,
            offset_of!(Physics, local_angular_vel) + 4,
            -0.2,
        );
        put_f32(&mut physics, offset_of!(Physics, clutch), 1.0);
        put_f32(&mut physics, offset_of!(Physics, speed_kmh), speed_ms * 3.6);
        for i in 0..4 {
            put_f32(
                &mut physics,
                offset_of!(Physics, wheels_pressure) + 4 * i,
                27.0 + 0.1 * i as f32,
            );
            put_f32(
                &mut physics,
                offset_of!(Physics, tyre_core_temperature) + 4 * i,
                80.0 + i as f32,
            );
            put_f32(
                &mut physics,
                offset_of!(Physics, brake_temp) + 4 * i,
                400.0 + 10.0 * i as f32,
            );
        }
        put_f32(&mut physics, offset_of!(Physics, air_temp), 22.0);
        put_f32(&mut physics, offset_of!(Physics, road_temp), 31.0);
        put_f32(&mut physics, offset_of!(Physics, brake_bias), 0.58);
        put_f32(&mut physics, offset_of!(Physics, water_temp), 88.0);
        put_f32(
            &mut physics,
            offset_of!(Physics, local_velocity) + 8,
            speed_ms,
        );

        let ac = self.game == Game::AssettoCorsa;
        if ac {
            // What this game publishes and Competizione does not.
            for i in 0..4 {
                let (inner, outer) = (90.0 + i as f32, 70.0 + i as f32);
                put_f32(
                    &mut physics,
                    offset_of!(Physics, tyre_temp_i) + 4 * i,
                    inner,
                );
                put_f32(&mut physics, offset_of!(Physics, tyre_temp_m) + 4 * i, 80.0);
                put_f32(
                    &mut physics,
                    offset_of!(Physics, tyre_temp_o) + 4 * i,
                    outer,
                );
                put_f32(&mut physics, offset_of!(Physics, tyre_wear) + 4 * i, 97.5);
                put_f32(
                    &mut physics,
                    offset_of!(Physics, wheel_load) + 4 * i,
                    3200.0,
                );
                put_f32(&mut physics, offset_of!(Physics, camber_rad) + 4 * i, -0.05);
            }
            put_f32(&mut physics, offset_of!(Physics, ride_height), 0.055);
            put_f32(&mut physics, offset_of!(Physics, ride_height) + 4, 0.072);
        }

        // As mapped: the size of Competizione's page, zeros after what the game writes.
        let mut graphics = vec![0u8; GRAPHICS_SIZE];
        put_i32(
            &mut graphics,
            offset_of!(Graphics, packet_id),
            (t * 60.0) as i32 + 1,
        );
        put_i32(&mut graphics, offset_of!(Graphics, status), self.status);
        put_i32(&mut graphics, offset_of!(Graphics, session), self.session);
        if !ac {
            put_i32(
                &mut graphics,
                offset_of!(Graphics, session_index),
                self.session_index,
            );
        }
        put_i32(
            &mut graphics,
            offset_of!(Graphics, completed_laps),
            laps as i32,
        );
        put_i32(&mut graphics, offset_of!(Graphics, position), 4);
        put_i32(
            &mut graphics,
            offset_of!(Graphics, i_current_time),
            (into * 1000.0) as i32,
        );
        put_i32(&mut graphics, offset_of!(Graphics, i_last_time), last_ms);
        put_i32(&mut graphics, offset_of!(Graphics, i_best_time), last_ms);
        put_f32(
            &mut graphics,
            offset_of!(Graphics, session_time_left),
            ((3600.0 - t) * 1000.0) as f32,
        );
        put_i32(
            &mut graphics,
            offset_of!(Graphics, current_sector_index),
            (fraction * 3.0) as i32,
        );
        put_text(
            &mut graphics,
            offset_of!(Graphics, tyre_compound),
            33,
            if ac { "Soft (S)" } else { "dry_compound" },
        );
        put_f32(
            &mut graphics,
            offset_of!(Graphics, normalized_car_position),
            fraction,
        );
        let radius = (self.length_m / std::f64::consts::TAU) as f32;
        let angle = fraction * std::f32::consts::TAU;
        if ac {
            // The first Assetto Corsa: the page is the same up to here, then holds the
            // player's coordinates alone and stops after a few fields.
            let at = offset_of!(GraphicsAc, car_coordinates);
            put_f32(&mut graphics, at, radius * angle.cos());
            put_f32(&mut graphics, at + 8, radius * angle.sin());
            put_i32(&mut graphics, offset_of!(GraphicsAc, is_in_pit_lane), 0);
            put_f32(&mut graphics, offset_of!(GraphicsAc, surface_grip), 0.98);
            put_f32(&mut graphics, offset_of!(GraphicsAc, wind_speed), 3.0);
            return self.finish(physics, graphics);
        }
        put_i32(&mut graphics, offset_of!(Graphics, active_cars), 3);
        for (slot, (id, lead)) in [(3, 0.4f32), (7, 0.0), (9, -0.7)].into_iter().enumerate() {
            let a = angle + lead;
            let at = offset_of!(Graphics, car_coordinates) + 12 * slot;
            put_f32(&mut graphics, at, radius * a.cos());
            put_f32(&mut graphics, at + 8, radius * a.sin());
            put_i32(&mut graphics, offset_of!(Graphics, car_id) + 4 * slot, id);
        }
        put_i32(&mut graphics, offset_of!(Graphics, player_car_id), 7);
        put_i32(&mut graphics, offset_of!(Graphics, tc), 3);
        put_i32(&mut graphics, offset_of!(Graphics, tc_cut), 2);
        put_i32(&mut graphics, offset_of!(Graphics, engine_map), 1);
        put_i32(&mut graphics, offset_of!(Graphics, abs), 4);
        put_i32(&mut graphics, offset_of!(Graphics, is_valid_lap), 1);
        put_i32(
            &mut graphics,
            offset_of!(Graphics, i_estimated_lap_time),
            lap_ms,
        );
        put_f32(
            &mut graphics,
            offset_of!(Graphics, clock),
            14.0 * 3600.0 + t as f32,
        );
        put_i32(&mut graphics, offset_of!(Graphics, track_grip_status), 2);
        put_i32(&mut graphics, offset_of!(Graphics, gap_ahead), 1850);
        put_i32(&mut graphics, offset_of!(Graphics, gap_behind), 640);
        self.finish(physics, graphics)
    }

    /// The static page, and the three together.
    fn finish(&self, physics: Vec<u8>, graphics: Vec<u8>) -> RawPages {
        let mut statics = vec![0u8; STATIC_SIZE];
        put_text(
            &mut statics,
            offset_of!(Static, sm_version),
            15,
            &self.sm_version,
        );
        put_text(&mut statics, offset_of!(Static, ac_version), 15, "1.10.2");
        put_i32(&mut statics, offset_of!(Static, number_of_sessions), 1);
        put_i32(&mut statics, offset_of!(Static, num_cars), 3);
        put_text(&mut statics, offset_of!(Static, car_model), 33, &self.car);
        put_text(&mut statics, offset_of!(Static, track), 33, &self.track);
        put_text(&mut statics, offset_of!(Static, player_name), 33, "Alex");
        put_text(
            &mut statics,
            offset_of!(Static, player_surname),
            33,
            "Martin",
        );
        put_i32(&mut statics, offset_of!(Static, sector_count), 3);
        put_i32(&mut statics, offset_of!(Static, max_rpm), 7500);
        put_f32(&mut statics, offset_of!(Static, max_fuel), 120.0);
        for i in 0..4 {
            put_f32(&mut statics, offset_of!(Static, tyre_radius) + 4 * i, 0.34);
        }
        if self.game == Game::AssettoCorsa {
            // This game publishes the length of the lap and the layout of the circuit.
            put_f32(
                &mut statics,
                offset_of!(Static, track_spline_length),
                self.length_m as f32,
            );
            put_text(
                &mut statics,
                offset_of!(Static, track_configuration),
                33,
                &self.track_configuration,
            );
        }
        RawPages {
            physics,
            graphics,
            statics,
        }
    }
}
