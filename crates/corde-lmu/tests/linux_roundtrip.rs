//! Linux round-trip: write a block through the `layout` structs into a
//! temporary file (not `/dev/shm`, to leave other readers alone), read it back
//! through [`LmuSource`] and check the converted values.
#![cfg(target_os = "linux")]

use corde_core::{SourceError, TelemetrySource};
use corde_lmu::fake::FakeWorld;
use corde_lmu::layout::{
    set_cstr, SharedMemoryObjectOut, TelemVect3, SHM_TOTAL_SIZE, SHM_USEFUL_SIZE,
};
use corde_lmu::source::{LmuSource, LmuSourceConfig};
use corde_protocol::{GameId, GamePhase, SessionType, VehicleClass};
use std::fs::OpenOptions;
use std::io::{Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::time::Duration;

fn temp_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("corde-roundtrip-{}-{name}", std::process::id()))
}

fn write_block(path: &PathBuf, raw: &SharedMemoryObjectOut, len: usize) {
    let mut f = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(path)
        .unwrap();
    f.seek(SeekFrom::Start(0)).unwrap();
    f.write_all(&raw.as_bytes()[..len]).unwrap();
    f.flush().unwrap();
}

#[test]
fn hand_written_block_reads_back_through_the_source() {
    let path = temp_path("hand");
    let _ = std::fs::remove_file(&path);

    let mut raw = SharedMemoryObjectOut::boxed();
    raw.generic.game_version = 1430;
    raw.generic.ffb_torque = 0.25;
    set_cstr(&mut raw.paths.user_data, r"C:\LMU\UserData");

    let s = &mut raw.scoring.scoring_info;
    set_cstr(&mut s.track_name, "Circuit de la Sarthe");
    s.session = 11; // race
    s.current_et = 1234.5;
    s.end_et = 7200.0;
    s.max_laps = 0;
    s.lap_dist = 13_626.0;
    s.num_vehicles = 2;
    s.game_phase = 5;
    s.yellow_flag_state = 0;
    s.in_realtime = 1;
    set_cstr(&mut s.player_name, "Alex Martin");
    s.ambient_temp = 18.5;
    s.track_temp = 27.25;
    s.raining = 0.1;
    s.track_grip_level = 3;
    s.cloud_coverage = 7;
    s.game_mode = 1;

    let v = &mut raw.scoring.veh_scoring_info[0];
    v.id = 42;
    set_cstr(&mut v.driver_name, "Alex Martin");
    set_cstr(&mut v.vehicle_name, "#15 Corde Racing");
    set_cstr(&mut v.vehicle_class, "Hyper");
    v.is_player = 1;
    v.place = 2;
    v.total_laps = 5;
    v.sector = 2; // S2
    v.best_lap_time = 215.123;
    v.last_lap_time = 216.5;
    v.last_sector1 = 60.0;
    v.last_sector2 = 130.0; // cumulative
    v.lap_dist = 6_813.0;
    v.fuel_fraction = 128;
    v.pit_state = 0;
    let v2 = &mut raw.scoring.veh_scoring_info[1];
    v2.id = 7;
    set_cstr(&mut v2.driver_name, "AI");
    set_cstr(&mut v2.vehicle_class, "LMP2");
    v2.place = 1;
    v2.control = 1;

    raw.telemetry.active_vehicles = 2;
    raw.telemetry.player_has_vehicle = 1;
    raw.telemetry.player_vehicle_idx = 1; // the player is the 2nd telemetry entry
    let t = &mut raw.telemetry.telem_info[1];
    t.id = 42;
    t.elapsed_time = 1234.5;
    t.lap_number = 6;
    t.lap_start_et = 1200.0;
    set_cstr(&mut t.vehicle_name, "#15 Corde Racing");
    set_cstr(&mut t.vehicle_model, "BMW M Hybrid V8");
    t.vehicle_class = 0x00; // Hypercar
    t.vehicle_championship = 0x02;
    t.local_vel = TelemVect3::new(0.0, 0.0, -75.0); // 270 km/h forward
    t.local_accel = TelemVect3::new(-19.6133, 9.80665, 4.903_325);
    t.local_rot = TelemVect3::new(0.0, 0.1, 0.0);
    t.gear = 6;
    t.engine_rpm = 7_250.0;
    t.engine_max_rpm = 8_500.0;
    t.engine_water_temp = 91.0;
    t.fuel = 43.21;
    t.fuel_capacity = 90.0;
    t.current_sector = 1; // zero-based S2, not in pits
    t.rear_brake_bias = 0.46;
    t.tc = 5;
    t.tc_max = 10;
    t.virtual_energy = 0.62;
    t.battery_charge_fraction = 0.8;
    t.electric_boost_motor_state = 2;
    t.physical_steering_wheel_range = 540.0;
    t.unfiltered_steering = 0.5;
    t.front_ride_height = 0.045;
    set_cstr(&mut t.front_tire_compound_name, "Medium");
    for (i, w) in t.wheel.iter_mut().enumerate() {
        w.temperature = [273.15 + 80.0 + i as f64, 273.15 + 90.0, 273.15 + 85.0];
        w.pressure = 150.0 + i as f64;
        w.wear = 0.9;
        w.brake_temp = 273.15 + 400.0;
        w.longitudinal_ground_vel = -75.0;
        w.longitudinal_patch_vel = 7.5; // 10 % spin
        w.camber = -0.0523599; // -3°
        set_cstr(&mut w.terrain_name, "ROAD");
    }
    raw.telemetry.telem_info[0].id = 7;
    raw.telemetry.telem_info[0].elapsed_time = 1234.5;
    raw.telemetry.telem_info[0].vehicle_class = 0x03;

    // Only the useful part is written: the source must accept 324 820 bytes.
    write_block(&path, &raw, SHM_USEFUL_SIZE);

    let mut src = LmuSource::with_config(LmuSourceConfig {
        shm_path: Some(path.clone()),
        stall_timeout: Duration::from_millis(100),
        ..Default::default()
    });
    assert_eq!(src.info().game, GameId::Lmu);
    assert_eq!(src.info().id, "lmu-shm-linux");
    assert_eq!(src.info().native_rate_hz, 100.0);
    src.connect().expect("connect");

    let f = src.poll().expect("poll").expect("frame");
    assert_eq!(f.source, "lmu-shm-linux");
    assert_eq!(f.seq, 1);
    assert!(f.wall_ms > 1_600_000_000_000);
    assert_eq!(f.session.game, GameId::Lmu);
    assert_eq!(f.session.track_name, "Circuit de la Sarthe");
    assert_eq!(f.session.session_type, SessionType::Race);
    assert_eq!(f.session.session_index, 11);
    assert_eq!(f.session.phase, GamePhase::Green);
    assert_eq!(f.session.elapsed_s, 1234.5);
    assert_eq!(f.session.num_vehicles, 2);
    assert!(f.session.in_realtime);
    assert!(!f.session.is_multiplayer);
    assert_eq!(f.session.player_name, "Alex Martin");
    assert!((f.session.weather.track_c - 27.25).abs() < 1e-5);
    assert_eq!(f.session.weather.track_grip_level, Some(3));
    assert_eq!(f.session.weather.cloud_coverage, Some(7));

    let p = f.player.as_ref().expect("player");
    assert_eq!(p.id, 42);
    assert_eq!(p.vehicle_name, "#15 Corde Racing");
    assert_eq!(p.vehicle_model, "BMW M Hybrid V8");
    assert_eq!(p.vehicle_class, VehicleClass::Hypercar);
    assert_eq!(p.class_name, "Hyper");
    assert_eq!(p.championship, "WEC 2025");
    assert_eq!(p.place, 2);
    assert_eq!(p.lap.lap_number, 6);
    assert_eq!(p.lap.laps_completed, 5);
    assert_eq!(p.lap.current_sector, 2);
    assert!(!p.lap.in_pit_lane);
    assert!((p.lap.current_lap_s - 34.5).abs() < 1e-4);
    assert_eq!(p.lap.best_lap_s, Some(215.123));
    assert_eq!(p.lap.last_lap_s, Some(216.5));
    assert_eq!(p.lap.last_s1, Some(60.0));
    assert_eq!(p.lap.last_s2, Some(70.0));
    assert_eq!(p.lap.last_s3, Some(86.5));
    assert_eq!(p.lap.best_s3, None);
    assert!((p.lap.lap_fraction - 0.5).abs() < 1e-3);
    assert!((p.motion.speed_kmh - 270.0).abs() < 1e-3);
    assert!(
        (p.motion.g_lat - 2.0).abs() < 1e-4,
        "right-hand turn → +2 g"
    );
    assert!((p.motion.g_long + 0.5).abs() < 1e-4, "braking → -0.5 g");
    assert!((p.motion.yaw_rate_dps - 5.729_578).abs() < 1e-3);
    assert_eq!(p.engine.gear, 6);
    assert_eq!(p.engine.rpm, 7_250.0);
    assert_eq!(p.engine.water_c, 91.0);
    assert!((p.energy.fuel_l - 43.21).abs() < 1e-5);
    assert!((p.energy.fuel_fraction - 0.480_111).abs() < 1e-5);
    assert_eq!(p.energy.virtual_energy, Some(0.62));
    assert_eq!(p.energy.battery_soc, Some(0.8));
    assert_eq!(p.electronics.tc, 5);
    assert_eq!(p.electronics.tc_max, 10);
    assert!((p.electronics.brake_bias_rear - 0.46).abs() < 1e-6);
    assert!((p.inputs.steering_angle_deg - 135.0).abs() < 1e-4);
    assert!((p.aero.front_ride_height_mm - 45.0).abs() < 1e-3);
    assert_eq!(p.tyres.front_compound, "Medium");
    for (i, w) in p.wheels.iter().enumerate() {
        assert!((w.temp_c[0] - (80.0 + i as f32)).abs() < 1e-3);
        assert!((w.temp_c[1] - 90.0).abs() < 1e-3);
        assert!((w.pressure_kpa - (150.0 + i as f32)).abs() < 1e-3);
        assert!((w.wear_remaining - 0.9).abs() < 1e-6);
        assert!((w.brake_temp_c - 400.0).abs() < 1e-3);
        assert!((w.slip_ratio - 0.1).abs() < 1e-6);
        assert!((w.camber_deg + 3.0).abs() < 1e-3);
        assert_eq!(w.terrain, "ROAD");
    }

    assert_eq!(f.standings.len(), 2);
    assert_eq!(f.standings[0].place, 1);
    assert_eq!(f.standings[0].driver_name, "AI");
    assert_eq!(f.standings[0].vehicle_class, VehicleClass::Lmp2);
    assert_eq!(f.standings[0].class_place, 1);
    assert_eq!(f.standings[1].place, 2);
    assert!(f.standings[1].is_player);
    assert_eq!(f.standings[1].class_place, 1, "first of its own class");
    assert_eq!(f.standings[1].sector, 2);
    assert!((f.standings[1].fuel_fraction.unwrap() - 128.0 / 255.0).abs() < 1e-6);

    // Unchanged block → no frame.
    assert!(src.poll().unwrap().is_none());

    // The clocks move → a new frame with the next sequence number.
    raw.scoring.scoring_info.current_et = 1234.51;
    raw.telemetry.telem_info[1].elapsed_time = 1234.51;
    raw.telemetry.telem_info[1].gear = 7;
    write_block(&path, &raw, SHM_USEFUL_SIZE);
    let f2 = src.poll().unwrap().expect("second frame");
    assert_eq!(f2.seq, 2);
    assert_eq!(f2.player.unwrap().engine.gear, 7);
    assert_eq!(src.status().frames, 2);
    assert!(src.status().connected);

    // Mirror removed → not available once the stall timeout elapsed.
    std::fs::remove_file(&path).unwrap();
    std::thread::sleep(Duration::from_millis(150));
    assert!(matches!(src.poll(), Err(SourceError::NotAvailable(_))));
    assert!(!src.is_connected());
}

#[test]
fn fake_world_streams_through_the_source() {
    let path = temp_path("fake");
    let _ = std::fs::remove_file(&path);
    let mut world = FakeWorld::new();
    let mut raw = SharedMemoryObjectOut::boxed();
    world.write_into(&mut raw);
    write_block(&path, &raw, SHM_TOTAL_SIZE);

    let mut src = LmuSource::with_config(LmuSourceConfig {
        shm_path: Some(path.clone()),
        ..Default::default()
    });
    src.connect().unwrap();
    let mut frames = 0;
    let mut last_lap = 0;
    for _ in 0..300 {
        world.step(0.01);
        world.write_into(&mut raw);
        write_block(&path, &raw, SHM_TOTAL_SIZE);
        if let Some(f) = src.poll().unwrap() {
            frames += 1;
            let p = f.player.expect("player always present in the fake world");
            assert!(p.lap.lap_number >= last_lap);
            last_lap = p.lap.lap_number;
            assert_eq!(f.standings.len(), 4);
            assert!(f.standings.iter().all(|s| s.place >= 1 && s.place <= 4));
            assert!(p.motion.speed_kmh > 100.0 && p.motion.speed_kmh < 400.0);
        }
    }
    assert_eq!(frames, 300, "every write must produce exactly one frame");
    assert_eq!(src.status().frames, 300);
    assert_eq!(src.status().torn_reads, 0);
    let _ = std::fs::remove_file(&path);
}
