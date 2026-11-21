//! Plausible pages of Assetto Corsa Rally, for the tests and the development tools: a car
//! that waits on the start line, drives the stage at a constant speed, rolls into the
//! stop zone, stands there, and is put back on the start line — again and again.
//!
//! The pages are Competizione's, built as bytes field by field, with what this game does
//! differently: temperatures in kelvins, a static page written late, a clock that reads
//! zero on the start line and freezes on the finish line, a blank physics page once the
//! car stands in the stop zone.

use crate::layout::{
    session, status, Graphics, Physics, Static, GRAPHICS_SIZE, PHYSICS_SIZE, STATIC_SIZE,
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

/// The physics page as the game shows it while it is not simulating the car: everything
/// at zero but the counter.
pub fn blank_physics(mut raw: RawPages) -> RawPages {
    let id = offset_of!(Physics, packet_id);
    for (i, b) in raw.physics.iter_mut().enumerate() {
        if !(id..id + 4).contains(&i) {
            *b = 0;
        }
    }
    raw
}

/// A stage driven again and again.
#[derive(Debug, Clone)]
pub struct FakeRallyStage {
    /// Identifier of the stage and of the car, as the game writes them.
    pub track: String,
    pub car: String,
    /// Seconds on the start line before each run.
    pub wait_s: f64,
    /// Time of a run.
    pub run_s: f64,
    /// Seconds rolling after the finish line, then standing in the stop zone.
    pub roll_s: f64,
    pub rest_s: f64,
    /// From the start line to the finish line.
    pub length_m: f64,
    /// What the game's distance reads on the start line.
    pub start_m: f64,
    /// The static page is written this long after the stage loaded (zeros before).
    pub static_after_s: f64,
    /// The static page gives the length of the stage.
    pub publishes_length: bool,
    /// The first run is given up this long after its start (the stage is restarted).
    pub gives_up_after_s: Option<f64>,
    /// `Graphics::status`.
    pub status: i32,
}

impl Default for FakeRallyStage {
    fn default() -> Self {
        Self {
            track: "alsace_stage_02".into(),
            car: "alpine_a110_rally".into(),
            wait_s: 6.0,
            run_s: 95.0,
            roll_s: 4.0,
            rest_s: 8.0,
            length_m: 2850.0,
            start_m: 12.5,
            static_after_s: 2.0,
            publishes_length: true,
            gives_up_after_s: None,
            status: status::LIVE,
        }
    }
}

/// Where the car is in the cycle.
enum Phase {
    /// On the start line.
    Line,
    /// Driving: seconds since the start.
    Run(f64),
    /// Past the finish line, rolling: seconds since the finish.
    Roll(f64),
    /// Standing in the stop zone.
    Stop,
}

impl FakeRallyStage {
    /// The time the game gives a run that finishes, ms.
    pub fn run_ms(&self) -> i32 {
        (self.run_s * 1000.0).round() as i32
    }

    /// How long one cycle lasts: start line, run, stop zone.
    pub fn cycle_s(&self) -> f64 {
        self.wait_s + self.run_s + self.roll_s + self.rest_s
    }

    fn phase(&self, t: f64) -> Phase {
        let mut t = t.max(0.0);
        if let Some(gave_up) = self.gives_up_after_s {
            // The first attempt: the start line, then the run until it is given up.
            if t < self.wait_s {
                return Phase::Line;
            }
            if t < self.wait_s + gave_up {
                return Phase::Run(t - self.wait_s);
            }
            t -= self.wait_s + gave_up;
        }
        let into = t % self.cycle_s();
        if into < self.wait_s {
            Phase::Line
        } else if into < self.wait_s + self.run_s {
            Phase::Run(into - self.wait_s)
        } else if into < self.wait_s + self.run_s + self.roll_s {
            Phase::Roll(into - self.wait_s - self.run_s)
        } else {
            Phase::Stop
        }
    }

    /// The pages `t_s` seconds after the stage was loaded.
    pub fn at(&self, t_s: f64) -> RawPages {
        let t = t_s.max(0.0);
        let speed = self.length_m / self.run_s;
        // After the line the car rolls on at half its speed.
        let (clock_ms, along, speed_ms, simulated) = match self.phase(t) {
            Phase::Line => (0, 0.0, 0.0, true),
            Phase::Run(s) => ((s * 1000.0) as i32, speed * s, speed, true),
            Phase::Roll(s) => (
                self.run_ms(),
                self.length_m + 0.5 * speed * s,
                0.5 * speed,
                true,
            ),
            Phase::Stop => (
                self.run_ms(),
                self.length_m + 0.5 * speed * self.roll_s,
                0.0,
                false,
            ),
        };
        let dist = (self.start_m + along) as f32;

        let mut physics = vec![0u8; PHYSICS_SIZE];
        let p = &mut physics;
        put_i32(p, offset_of!(Physics, packet_id), (t * 333.0) as i32 + 1);
        if simulated {
            put_f32(
                p,
                offset_of!(Physics, gas),
                if speed_ms > 0.0 { 0.9 } else { 0.0 },
            );
            put_i32(p, offset_of!(Physics, gear), 4); // third
            put_i32(p, offset_of!(Physics, rpm), 5200);
            put_f32(p, offset_of!(Physics, speed_kmh), speed_ms as f32 * 3.6);
            put_f32(p, offset_of!(Physics, fuel), 38.0);
            put_f32(p, offset_of!(Physics, clutch), 1.0);
            // Kelvins: 22 °C in the air, 27 °C on the road, 70.45 °C in the engine.
            put_f32(p, offset_of!(Physics, air_temp), 295.15);
            put_f32(p, offset_of!(Physics, road_temp), 300.15);
            put_f32(p, offset_of!(Physics, water_temp), 343.6);
            for i in 0..4 {
                let f = i as f32;
                put_f32(
                    p,
                    offset_of!(Physics, tyre_core_temperature) + 4 * i,
                    363.15 + f,
                );
                put_f32(p, offset_of!(Physics, brake_temp) + 4 * i, 322.0 + 10.0 * f);
                put_f32(p, offset_of!(Physics, wheels_pressure) + 4 * i, 24.0);
                put_f32(p, offset_of!(Physics, wheel_load) + 4 * i, 3400.0);
            }
        }

        let mut graphics = vec![0u8; GRAPHICS_SIZE];
        let g = &mut graphics;
        put_i32(g, offset_of!(Graphics, packet_id), (t * 60.0) as i32 + 1);
        put_i32(g, offset_of!(Graphics, status), self.status);
        put_i32(g, offset_of!(Graphics, session), session::HOTLAP);
        put_i32(g, offset_of!(Graphics, position), 1);
        put_i32(g, offset_of!(Graphics, i_current_time), clock_ms);
        put_f32(g, offset_of!(Graphics, distance_traveled), dist);
        put_i32(g, offset_of!(Graphics, active_cars), 1);
        // A stage that winds from west to east.
        let at = offset_of!(Graphics, car_coordinates);
        put_f32(g, at, dist);
        put_f32(g, at + 4, 310.0);
        put_f32(g, at + 8, 60.0 * (dist / 180.0).sin());

        let mut statics = vec![0u8; STATIC_SIZE];
        if t >= self.static_after_s {
            let s = &mut statics;
            put_text(s, offset_of!(Static, sm_version), 15, "1.7");
            put_i32(s, offset_of!(Static, number_of_sessions), 1);
            put_i32(s, offset_of!(Static, num_cars), 1);
            put_text(s, offset_of!(Static, car_model), 33, &self.car);
            put_text(s, offset_of!(Static, track), 33, &self.track);
            put_text(s, offset_of!(Static, player_name), 33, "Alex");
            put_text(s, offset_of!(Static, player_surname), 33, "Martin");
            put_i32(s, offset_of!(Static, max_rpm), 7000);
            put_f32(s, offset_of!(Static, max_fuel), 60.0);
            if self.publishes_length {
                put_f32(
                    s,
                    offset_of!(Static, track_spline_length),
                    self.length_m as f32,
                );
            }
        }
        RawPages {
            physics,
            graphics,
            statics,
        }
    }
}
