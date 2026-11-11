//! A plausible synthetic `LMU_Data` block: 1 player + 3 AI cars lapping a
//! circular "Le Mans" with varying speed / rpm / positions and an advancing
//! session time. Used by the `lmu-shm-fake` tool, the integration tests and
//! `corde-sim`. Deterministic (no randomness).

use crate::layout::{
    set_cstr, SharedMemoryEvent, SharedMemoryObjectOut, TelemInfoV01, TelemVect3,
    VehicleScoringInfoV01, SME_MAX,
};
use std::f64::consts::TAU;

/// One simulated car.
#[derive(Debug, Clone)]
pub struct FakeCar {
    pub id: i32,
    pub driver: String,
    pub vehicle: String,
    pub model: String,
    /// scoring class string ("Hyper", "LMP2", "GT3")
    pub class_name: String,
    /// `IP_VehicleClass` value
    pub class_enum: u8,
    pub is_player: bool,
    /// base speed, m/s
    pub base_speed: f64,
    pub lap_dist: f64,
    pub laps: i32,
    pub lap_start_et: f64,
    pub last_lap: f64,
    pub best_lap: f64,
    pub last_s1: f64,
    pub last_s2: f64,
    pub cur_s1: f64,
    pub cur_s2: f64,
    pub fuel_l: f64,
    pub wear: f64,
    pub speed: f64,
}

/// The simulated world.
#[derive(Debug, Clone)]
pub struct FakeWorld {
    pub track_name: String,
    pub track_length_m: f64,
    pub elapsed_s: f64,
    pub session: i32,
    pub cars: Vec<FakeCar>,
    pub game_version: i32,
    pub user_data_path: String,
}

impl Default for FakeWorld {
    fn default() -> Self {
        Self::new()
    }
}

impl FakeWorld {
    pub fn new() -> Self {
        let mk = |id,
                  driver: &str,
                  vehicle: &str,
                  model: &str,
                  class: &str,
                  class_enum,
                  speed,
                  player| {
            FakeCar {
                id,
                driver: driver.into(),
                vehicle: vehicle.into(),
                model: model.into(),
                class_name: class.into(),
                class_enum,
                is_player: player,
                base_speed: speed,
                lap_dist: 0.0,
                laps: 0,
                lap_start_et: 0.0,
                last_lap: -1.0,
                best_lap: -1.0,
                last_s1: -1.0,
                last_s2: -1.0,
                cur_s1: -1.0,
                cur_s2: -1.0,
                fuel_l: 90.0,
                wear: 1.0,
                speed,
            }
        };
        let cars = vec![
            mk(
                1,
                "Alex Martin",
                "#15 Corde Racing",
                "BMW M Hybrid V8",
                "Hyper",
                0x00,
                68.0,
                true,
            ),
            mk(
                2,
                "AI Alpha",
                "#8 Toyota Gazoo Racing",
                "Toyota GR010",
                "Hyper",
                0x00,
                69.0,
                false,
            ),
            mk(
                3,
                "AI Bravo",
                "#22 United Autosports",
                "Oreca 07",
                "LMP2",
                0x03,
                63.0,
                false,
            ),
            mk(
                4,
                "AI Charlie",
                "#46 Team WRT",
                "BMW M4 LMGT3",
                "GT3",
                0x06,
                56.0,
                false,
            ),
        ];
        // stagger the start
        let mut w = Self {
            track_name: "Le Mans".into(),
            track_length_m: 13_626.0,
            elapsed_s: 0.0,
            session: 10,
            cars,
            game_version: 1430,
            user_data_path:
                r"C:\Program Files (x86)\Steam\steamapps\common\Le Mans Ultimate\UserData".into(),
        };
        for (i, c) in w.cars.iter_mut().enumerate() {
            c.lap_dist = 200.0 * i as f64;
        }
        w
    }

    /// Advance the world by `dt` seconds.
    pub fn step(&mut self, dt: f64) {
        self.elapsed_s += dt;
        let et = self.elapsed_s;
        let len = self.track_length_m;
        for c in self.cars.iter_mut() {
            // speed oscillates ±25 % along the lap (corners / straights)
            let phase = c.lap_dist / len * TAU * 7.0;
            c.speed = c.base_speed * (1.0 + 0.25 * phase.sin());
            c.lap_dist += c.speed * dt;
            let s1_end = len / 3.0;
            let s2_end = 2.0 * len / 3.0;
            if c.cur_s1 < 0.0 && c.lap_dist >= s1_end {
                c.cur_s1 = et - c.lap_start_et;
            }
            if c.cur_s2 < 0.0 && c.lap_dist >= s2_end {
                c.cur_s2 = et - c.lap_start_et;
            }
            if c.lap_dist >= len {
                c.lap_dist -= len;
                c.laps += 1;
                c.last_lap = et - c.lap_start_et;
                if c.best_lap < 0.0 || c.last_lap < c.best_lap {
                    c.best_lap = c.last_lap;
                }
                c.last_s1 = c.cur_s1;
                c.last_s2 = c.cur_s2;
                c.cur_s1 = -1.0;
                c.cur_s2 = -1.0;
                c.lap_start_et = et;
            }
            c.fuel_l = (c.fuel_l - 0.0035 * dt * c.speed / 60.0).max(0.0);
            c.wear = (c.wear - 0.000_004 * dt * c.speed).max(0.0);
        }
    }

    /// Current sector (0..2) of a car.
    fn sector0(&self, c: &FakeCar) -> i32 {
        let f = c.lap_dist / self.track_length_m;
        if f < 1.0 / 3.0 {
            0
        } else if f < 2.0 / 3.0 {
            1
        } else {
            2
        }
    }

    /// Places (1-based) by laps then distance.
    fn places(&self) -> Vec<u8> {
        let mut order: Vec<usize> = (0..self.cars.len()).collect();
        order.sort_by(|&a, &b| {
            let ca = &self.cars[a];
            let cb = &self.cars[b];
            cb.laps.cmp(&ca.laps).then(
                cb.lap_dist
                    .partial_cmp(&ca.lap_dist)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
        });
        let mut places = vec![0u8; self.cars.len()];
        for (rank, idx) in order.iter().enumerate() {
            places[*idx] = rank as u8 + 1;
        }
        places
    }

    /// World position on a circle of circumference `track_length_m`, and the
    /// orientation matrix rows (forward = -z local).
    fn pose(&self, c: &FakeCar) -> (TelemVect3, [TelemVect3; 3], f64) {
        let r = self.track_length_m / TAU;
        let a = c.lap_dist / self.track_length_m * TAU;
        let pos = TelemVect3::new(r * a.cos(), 2.0, r * a.sin());
        // forward direction (tangent) in world X/Z
        let fwd = (-a.sin(), a.cos());
        // local +z points to the back → row 2 = -forward ; +x left ; +y up
        let heading = fwd.0.atan2(fwd.1);
        let ori = [
            TelemVect3::new(-fwd.1, 0.0, fwd.0),
            TelemVect3::new(0.0, 1.0, 0.0),
            TelemVect3::new(-fwd.0, 0.0, -fwd.1),
        ];
        (pos, ori, heading)
    }

    fn fill_scoring(&self, c: &FakeCar, place: u8, v: &mut VehicleScoringInfoV01) {
        v.id = c.id;
        set_cstr(&mut v.driver_name, &c.driver);
        set_cstr(&mut v.vehicle_name, &c.vehicle);
        v.total_laps = c.laps as i16;
        // 0 = S3, 1 = S1, 2 = S2
        v.sector = match self.sector0(c) {
            0 => 1,
            1 => 2,
            _ => 0,
        };
        v.finish_status = 0;
        v.lap_dist = c.lap_dist;
        v.path_lateral = 0.5 * (c.lap_dist / 97.0).sin();
        v.track_edge = 6.0;
        v.best_sector1 = c.last_s1;
        v.best_sector2 = c.last_s2;
        v.best_lap_time = c.best_lap;
        v.last_sector1 = c.last_s1;
        v.last_sector2 = c.last_s2;
        v.last_lap_time = c.last_lap;
        v.cur_sector1 = c.cur_s1;
        v.cur_sector2 = c.cur_s2;
        v.num_pitstops = 0;
        v.num_penalties = 0;
        v.is_player = c.is_player as u8;
        v.control = if c.is_player { 0 } else { 1 };
        v.in_pits = 0;
        v.place = place;
        set_cstr(&mut v.vehicle_class, &c.class_name);
        v.time_behind_next = if place > 1 { 1.5 * place as f64 } else { 0.0 };
        v.time_behind_leader = if place > 1 { 3.0 * place as f64 } else { 0.0 };
        v.lap_start_et = c.lap_start_et;
        let (pos, ori, _) = self.pose(c);
        v.pos = pos;
        v.local_vel = TelemVect3::new(0.0, 0.0, -c.speed);
        v.ori = ori;
        v.headlights = 0;
        v.pit_state = 0;
        v.server_scored = 1;
        v.qualification = place as i32;
        v.time_into_lap = self.elapsed_s - c.lap_start_et;
        v.estimated_lap_time = self.track_length_m / c.base_speed;
        set_cstr(&mut v.pit_group, &c.vehicle);
        v.flag = 0;
        v.count_lap_flag = 2;
        v.pit_lap_dist = 100.0;
        v.best_lap_sector1 = c.last_s1 as f32;
        v.best_lap_sector2 = c.last_s2 as f32;
        v.steam_id = 76_561_198_000_000_000 + c.id as u64;
        set_cstr(
            &mut v.veh_filename,
            &format!("{}.VEH", c.model.replace(' ', "_")),
        );
        v.fuel_fraction = (c.fuel_l / 90.0 * 255.0) as u8;
    }

    fn fill_telemetry(&self, c: &FakeCar, t: &mut TelemInfoV01) {
        t.id = c.id;
        t.delta_time = 0.01;
        t.elapsed_time = self.elapsed_s;
        t.lap_number = c.laps;
        t.lap_start_et = c.lap_start_et;
        set_cstr(&mut t.vehicle_name, &c.vehicle);
        set_cstr(&mut t.track_name, &self.track_name);
        let (pos, ori, _) = self.pose(c);
        t.pos = pos;
        t.local_vel = TelemVect3::new(0.0, 0.0, -c.speed);
        let phase = c.lap_dist / self.track_length_m * TAU * 7.0;
        // decelerating in corners (positive z accel = backwards), lateral load
        t.local_accel = TelemVect3::new(-12.0 * phase.sin(), 9.81, -6.0 * phase.cos());
        t.ori = ori;
        t.local_rot = TelemVect3::new(0.0, 0.3 * phase.sin(), 0.0);
        let gear = ((c.speed / 12.0) as i32).clamp(1, 7);
        t.gear = gear;
        t.engine_max_rpm = 8_500.0;
        t.engine_rpm = (c.speed / (gear as f64 * 12.0) * 7_800.0).clamp(2_000.0, 8_400.0);
        t.engine_water_temp = 88.0;
        t.engine_oil_temp = 105.0;
        t.clutch_rpm = t.engine_rpm;
        let throttle = (0.5 + 0.5 * phase.cos()).clamp(0.0, 1.0);
        let brake = if phase.sin() > 0.6 { phase.sin() } else { 0.0 };
        t.unfiltered_throttle = throttle;
        t.unfiltered_brake = brake;
        t.unfiltered_steering = 0.3 * phase.sin();
        t.filtered_throttle = throttle;
        t.filtered_brake = brake;
        t.filtered_steering = 0.3 * phase.sin();
        t.steering_shaft_torque = 8.0 * phase.sin();
        t.front_ride_height = 0.045;
        t.rear_ride_height = 0.060;
        t.front_wing_height = 0.040;
        t.drag = 2_000.0 * (c.speed / 70.0).powi(2);
        t.front_downforce = 4_000.0 * (c.speed / 70.0).powi(2);
        t.rear_downforce = 6_000.0 * (c.speed / 70.0).powi(2);
        t.fuel = c.fuel_l;
        t.fuel_capacity = 90.0;
        t.scheduled_stops = 0;
        t.engine_torque = 450.0 * throttle;
        let mut sector = self.sector0(c);
        if c.lap_dist < 50.0 && c.laps == 0 {
            // leaving the pits on the first metres
            sector |= i32::MIN;
        }
        t.current_sector = sector;
        t.max_gears = 7;
        t.front_tire_compound_index = 1;
        t.rear_tire_compound_index = 1;
        set_cstr(&mut t.front_tire_compound_name, "Medium");
        set_cstr(&mut t.rear_tire_compound_name, "Medium");
        t.speed_limiter_available = 1;
        t.visual_steering_wheel_range = 540.0;
        t.physical_steering_wheel_range = 540.0;
        t.rear_brake_bias = 0.47;
        t.delta_best = 0.25 * phase.sin();
        t.battery_charge_fraction = 0.6 + 0.3 * phase.cos();
        t.electric_boost_motor_state = if throttle > 0.8 { 2 } else { 1 };
        t.electric_boost_motor_torque = if throttle > 0.8 { 150.0 } else { -40.0 };
        t.electric_boost_motor_rpm = t.engine_rpm;
        t.electric_boost_motor_temperature = 70.0;
        t.electric_boost_water_temperature = 60.0;
        t.tc = 4;
        t.tc_max = 10;
        t.abs = 3;
        t.abs_max = 10;
        t.motor_map = 2;
        t.motor_map_max = 5;
        t.migration = 1;
        t.migration_max = 6;
        t.front_anti_sway = 3;
        t.front_anti_sway_max = 6;
        t.rear_anti_sway = 2;
        t.rear_anti_sway_max = 6;
        t.regen = if throttle < 0.2 { 120.0 } else { 0.0 };
        t.soc = (t.battery_charge_fraction * 100.0) as f32;
        t.virtual_energy = (c.fuel_l / 90.0) as f32;
        t.time_gap_car_ahead = 1.2;
        t.time_gap_car_behind = 0.8;
        t.time_gap_place_ahead = 1.2;
        t.time_gap_place_behind = 0.8;
        set_cstr(&mut t.vehicle_model, &c.model);
        t.vehicle_class = c.class_enum;
        t.vehicle_championship = 0x02; // WEC 2025
        for (i, w) in t.wheel.iter_mut().enumerate() {
            let front = i < 2;
            let left = i % 2 == 0;
            w.suspension_deflection = 0.02 + 0.005 * phase.sin();
            w.ride_height = if front { 0.045 } else { 0.060 };
            w.susp_force = 3_500.0 + 500.0 * phase.sin() * if left { 1.0 } else { -1.0 };
            w.brake_temp = 273.15 + 350.0 + 200.0 * brake;
            w.brake_pressure = brake;
            w.rotation = c.speed / 0.33;
            w.lateral_patch_vel = 0.4 * phase.sin();
            w.longitudinal_patch_vel = 0.02 * c.speed * (throttle - brake);
            w.lateral_ground_vel = 0.4 * phase.sin();
            w.longitudinal_ground_vel = -c.speed;
            w.camber = -0.052; // ≈ -3°
            w.lateral_force = 2_500.0 * phase.sin();
            w.longitudinal_force = 1_500.0 * (throttle - brake);
            w.tire_load = 3_800.0;
            w.grip_fract = 0.1 + 0.2 * brake;
            w.pressure = if front { 152.0 } else { 148.0 };
            w.temperature = [273.15 + 88.0, 273.15 + 92.0, 273.15 + 90.0];
            w.wear = c.wear;
            set_cstr(&mut w.terrain_name, "ROAD");
            w.surface_type = 0;
            w.static_undeflected_radius = 33;
            w.vertical_tire_deflection = 0.012;
            w.toe = 0.002;
            w.tire_carcass_temperature = 273.15 + 85.0;
            w.tire_inner_layer_temperature = [273.15 + 86.0, 273.15 + 89.0, 273.15 + 87.0];
            w.optimal_temp = 90.0;
            w.compound_index = 1;
            w.compound_type = 1;
        }
    }

    /// Write the whole world into `raw` (all four blocks).
    pub fn write_into(&self, raw: &mut SharedMemoryObjectOut) {
        // generic
        let mut events = [SharedMemoryEvent::UpdateTelemetry as u32; SME_MAX];
        events[0] = SharedMemoryEvent::UpdateScoring as u32;
        raw.generic.events = events;
        raw.generic.game_version = self.game_version;
        raw.generic.ffb_torque = 0.0;
        raw.generic.app_info.width = 2560;
        raw.generic.app_info.height = 1440;
        raw.generic.app_info.refresh_rate = 144;
        raw.generic.app_info.options_location = 3;
        set_cstr(&mut raw.generic.app_info.options_page, "");

        // paths
        set_cstr(&mut raw.paths.user_data, &self.user_data_path);
        set_cstr(
            &mut raw.paths.custom_variables,
            &format!(r"{}\player\CustomPluginVariables.JSON", self.user_data_path),
        );
        set_cstr(
            &mut raw.paths.player_profile,
            &format!(r"{}\player", self.user_data_path),
        );

        // scoring
        let s = &mut raw.scoring.scoring_info;
        set_cstr(&mut s.track_name, &self.track_name);
        s.session = self.session;
        s.current_et = self.elapsed_s;
        s.end_et = 3_600.0;
        s.max_laps = i32::MAX;
        s.lap_dist = self.track_length_m;
        s.num_vehicles = self.cars.len() as i32;
        s.game_phase = 5;
        s.yellow_flag_state = 0;
        s.sector_flag = [0, 0, 0];
        s.in_realtime = 1;
        set_cstr(&mut s.player_name, "Alex Martin");
        set_cstr(&mut s.plr_file_name, "Alex Martin");
        s.dark_cloud = 0.2;
        s.raining = 0.0;
        s.ambient_temp = 21.0;
        s.track_temp = 29.5;
        s.min_path_wetness = 0.0;
        s.max_path_wetness = 0.0;
        s.avg_path_wetness = 0.0;
        s.game_mode = 1;
        s.max_players = 104;
        set_cstr(&mut s.server_name, "");
        s.start_et = 14.0 * 3600.0;
        s.session_time_remaining = (3_600.0 - self.elapsed_s).max(0.0) as f32;
        s.time_of_day = (14.0 * 3600.0 + self.elapsed_s) as f32;
        s.is_fixed_setup = 0;
        s.track_grip_level = 2;
        s.cloud_coverage = 3;
        s.track_limits_steps_per_penalty = 12;
        s.track_limits_steps_per_point = 4;
        raw.scoring.scoring_stream_size = 0;

        let places = self.places();
        for (i, c) in self.cars.iter().enumerate() {
            self.fill_scoring(c, places[i], &mut raw.scoring.veh_scoring_info[i]);
        }
        for v in raw.scoring.veh_scoring_info[self.cars.len()..].iter_mut() {
            *v = VehicleScoringInfoV01::default();
        }

        // telemetry
        raw.telemetry.active_vehicles = self.cars.len() as u8;
        let player_idx = self.cars.iter().position(|c| c.is_player);
        raw.telemetry.player_has_vehicle = player_idx.is_some() as u8;
        raw.telemetry.player_vehicle_idx = player_idx.unwrap_or(0) as u8;
        for (i, c) in self.cars.iter().enumerate() {
            self.fill_telemetry(c, &mut raw.telemetry.telem_info[i]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_advances_and_laps() {
        let mut w = FakeWorld::new();
        let mut raw = SharedMemoryObjectOut::boxed();
        for _ in 0..(260 * 100) {
            w.step(0.01);
        }
        w.write_into(&mut raw);
        assert!(w.elapsed_s > 259.0);
        // the fastest car has completed at least one lap after 260 s (~200 s/lap)
        assert!(w.cars.iter().any(|c| c.laps >= 1));
        assert_eq!(raw.scoring.scoring_info.num_vehicles, 4);
        assert_eq!(raw.telemetry.active_vehicles, 4);
        assert!(raw.player_scoring().is_some());
        assert_eq!(raw.player_telemetry().map(|t| t.id), Some(1));
        let places: Vec<u8> = raw.scoring.vehicles().iter().map(|v| v.place).collect();
        let mut sorted = places.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, vec![1, 2, 3, 4]);
    }
}
