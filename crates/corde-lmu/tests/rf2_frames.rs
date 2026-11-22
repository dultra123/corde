//! rFactor 2 end to end, against the fake game: the two buffers of the plugin, read from
//! a folder, give the frames Le Mans Ultimate's converter gives for the same world —
//! minus what only Le Mans Ultimate publishes.

use corde_core::{SourceError, TelemetrySource};
use corde_lmu::convert::to_frame;
use corde_lmu::fake::FakeWorld;
use corde_lmu::layout::{SharedMemoryObjectOut, SIZE_TELEM_INFO, SIZE_VEHICLE_SCORING_INFO};
use corde_lmu::rf2::fake::{buffers_from_block, FakeRf2};
use corde_lmu::rf2::layout::{
    MAX_VEHICLES, OFF_SCORING_INFO, OFF_SCORING_VEHICLES, OFF_TELEM_NUM, OFF_TELEM_VEHICLES,
    OFF_VERSION_BEGIN,
};
use corde_lmu::rf2::{Rf2Converter, Rf2Raw, Rf2Source, Rf2SourceConfig};
use corde_protocol::{CompoundType, GameId, VehicleClass};
use std::path::PathBuf;
use std::time::Duration;

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("corde-rf2-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// The world after `steps` steps of 10 ms, as Le Mans Ultimate's block.
fn world_block(steps: usize) -> (FakeWorld, Box<SharedMemoryObjectOut>) {
    let mut world = FakeWorld::new();
    let mut block = SharedMemoryObjectOut::boxed();
    for _ in 0..steps {
        world.step(0.01);
    }
    world.write_into(&mut block);
    (world, block)
}

#[test]
fn the_same_world_gives_the_same_frame_as_le_mans_ultimate_minus_what_is_its_own() {
    let (_, block) = world_block(900);
    let lmu = to_frame(&block, None, 1, "lmu");
    let raw = buffers_from_block(&block, 1, 1);
    let rf2 = Rf2Converter::new().frame(&raw, "rf2-shm");

    assert_eq!(rf2.session.game, GameId::Rfactor2);
    assert_eq!(rf2.session.track_name, lmu.session.track_name);
    assert_eq!(rf2.session.track_length_m, lmu.session.track_length_m);
    assert_eq!(rf2.session.session_type, lmu.session.session_type);
    assert_eq!(rf2.session.elapsed_s, lmu.session.elapsed_s);
    assert_eq!(rf2.session.num_vehicles, lmu.session.num_vehicles);
    assert_eq!(rf2.session.in_realtime, lmu.session.in_realtime);

    let (a, b) = (rf2.player.as_ref().unwrap(), lmu.player.as_ref().unwrap());
    assert_eq!(a.id, b.id);
    assert_eq!(a.vehicle_name, b.vehicle_name);
    assert_eq!(rf2.session.player_name, lmu.session.player_name);
    assert_eq!(a.place, b.place);
    // Everything measured on the car: the same numbers, field for field.
    assert_eq!(a.motion, b.motion);
    assert_eq!(a.engine, b.engine);
    assert_eq!(a.inputs, b.inputs);
    assert_eq!(a.lap.lap_number, b.lap.lap_number);
    assert_eq!(a.lap.last_lap_s, b.lap.last_lap_s);
    assert_eq!(a.lap.best_lap_s, b.lap.best_lap_s);
    assert_eq!(a.lap.current_lap_s, b.lap.current_lap_s);
    assert_eq!(a.energy.fuel_l, b.energy.fuel_l);
    assert_eq!(a.energy.fuel_capacity_l, b.energy.fuel_capacity_l);
    // The battery and the electric motor sit elsewhere in rFactor 2: read at the right place.
    assert_eq!(a.energy.battery_soc, b.energy.battery_soc);
    assert_eq!(a.energy.motor_rpm, b.energy.motor_rpm);
    assert_eq!(a.energy.motor_torque_nm, b.energy.motor_torque_nm);
    assert_eq!(a.electronics.brake_bias_rear, b.electronics.brake_bias_rear);
    for (wa, wb) in a.wheels.iter().zip(b.wheels.iter()) {
        assert_eq!(wa.brake_temp_c, wb.brake_temp_c);
        assert_eq!(wa.pressure_kpa, wb.pressure_kpa);
        assert_eq!(wa.temp_c, wb.temp_c);
        assert_eq!(wa.wear_remaining, wb.wear_remaining);
        assert_eq!(wa.carcass_c, wb.carcass_c);
        assert_eq!(wa.optimal_temp_c, None);
        // Not « soft »: rFactor 2 does not say.
        assert_eq!(wa.compound_type, CompoundType::Unknown);
    }

    // What only Le Mans Ultimate publishes is empty, not invented.
    assert_eq!(a.vehicle_model, "");
    assert_eq!(a.electronics.tc, 0);
    assert_eq!(a.electronics.tc_max, 0);
    assert_eq!(a.electronics.abs_max, 0);
    assert_eq!(a.energy.virtual_energy, None);
    assert!(!a.lap.lap_invalidated);
    assert_eq!(a.lap.delta_best_s, 0.0);
    assert_eq!(rf2.session.weather.track_grip_level, None);
    // The class comes from its name.
    assert_eq!(a.class_name, b.class_name);
    assert_ne!(a.vehicle_class, VehicleClass::Unknown);

    // The standings: same cars, same order, same times.
    assert_eq!(rf2.standings.len(), lmu.standings.len());
    for (x, y) in rf2.standings.iter().zip(lmu.standings.iter()) {
        assert_eq!(x.id, y.id);
        assert_eq!(x.place, y.place);
        assert_eq!(x.driver_name, y.driver_name);
        assert_eq!(x.best_lap_s, y.best_lap_s);
        assert_eq!(x.laps_completed, y.laps_completed);
        assert_eq!(x.fuel_fraction, None);
    }
}

#[test]
fn the_time_left_and_the_hour_are_worked_out_from_what_the_game_publishes() {
    let (_, mut block) = world_block(300);
    block.scoring.scoring_info.current_et = 600.0;
    block.scoring.scoring_info.end_et = 3600.0;
    block.scoring.scoring_info.start_et = 14.0 * 3600.0;
    block.scoring.scoring_info.dark_cloud = 0.42;
    let raw = buffers_from_block(&block, 1, 1);
    let frame = Rf2Converter::new().frame(&raw, "rf2-shm");
    assert_eq!(frame.session.time_remaining_s, 3000.0);
    assert_eq!(frame.session.weather.time_of_day_s, 14.0 * 3600.0 + 600.0);
    assert_eq!(frame.session.weather.cloud_coverage, Some(4));
}

/// The plugin lists up to 128 vehicles, in the order the game hands them over: the
/// player can be anywhere, and further than the 104 vehicles the converter holds.
#[test]
fn the_player_is_found_wherever_the_plugin_put_it() {
    let (_, block) = world_block(500);
    let base = buffers_from_block(&block, 1, 1);
    let want = Rf2Converter::new().frame(&base, "rf2-shm");
    let player = want.player.as_ref().unwrap();

    // The same session with 120 vehicles: the player last in both buffers.
    let mut raw = Rf2Raw::new();
    raw.telemetry[..OFF_TELEM_VEHICLES].copy_from_slice(&base.telemetry[..OFF_TELEM_VEHICLES]);
    raw.scoring[..OFF_SCORING_VEHICLES].copy_from_slice(&base.scoring[..OFF_SCORING_VEHICLES]);
    let n = 120usize;
    assert!(n <= MAX_VEHICLES);
    let tel = |i: usize| OFF_TELEM_VEHICLES + i * SIZE_TELEM_INFO;
    let veh = |i: usize| OFF_SCORING_VEHICLES + i * SIZE_VEHICLE_SCORING_INFO;
    // Which entries of the base are the player's (slot id at offset 0, « is player » at 196).
    let base_n = base.scoring_vehicles();
    let p_score = (0..base_n)
        .find(|&i| base.scoring[veh(i) + 196] != 0)
        .unwrap();
    let p_id = &base.scoring[veh(p_score)..veh(p_score) + 4];
    let p_tel = (0..base.telemetry_vehicles())
        .find(|&i| &base.telemetry[tel(i)..tel(i) + 4] == p_id)
        .unwrap();
    let other_score = (0..base_n).find(|&i| i != p_score).unwrap();
    let other_tel = (0..base.telemetry_vehicles())
        .find(|&i| i != p_tel)
        .unwrap();
    for i in 0..n {
        let (s, t) = if i == n - 1 {
            (p_score, p_tel)
        } else {
            (other_score, other_tel)
        };
        raw.scoring[veh(i)..veh(i) + SIZE_VEHICLE_SCORING_INFO]
            .copy_from_slice(&base.scoring[veh(s)..veh(s) + SIZE_VEHICLE_SCORING_INFO]);
        raw.telemetry[tel(i)..tel(i) + SIZE_TELEM_INFO]
            .copy_from_slice(&base.telemetry[tel(t)..tel(t) + SIZE_TELEM_INFO]);
        if i != n - 1 {
            // Each its own slot id, none the player's.
            let id = (1000 + i as i32).to_le_bytes();
            raw.scoring[veh(i)..veh(i) + 4].copy_from_slice(&id);
            raw.telemetry[tel(i)..tel(i) + 4].copy_from_slice(&id);
        }
    }
    raw.telemetry[OFF_TELEM_NUM..OFF_TELEM_NUM + 4].copy_from_slice(&(n as i32).to_le_bytes());
    let num = OFF_SCORING_INFO + 104;
    raw.scoring[num..num + 4].copy_from_slice(&(n as i32).to_le_bytes());

    let frame = Rf2Converter::new().frame(&raw, "rf2-shm");
    let got = frame.player.as_ref().expect("the player's car");
    assert_eq!(got.id, player.id);
    assert_eq!(got.vehicle_name, player.vehicle_name);
    assert_eq!(got.motion, player.motion);
    assert_eq!(got.lap.last_lap_s, player.lap.last_lap_s);
    // Le Mans Ultimate's block holds 104 vehicles: the first ones, and the player.
    assert_eq!(frame.session.num_vehicles, 104);
    assert!(frame.standings.iter().any(|s| s.id == player.id));
}

#[test]
fn the_source_waits_for_a_sign_of_life_then_follows_the_session_until_it_ends() {
    let dir = temp_dir("source");
    let mut game = FakeRf2::new();
    let mut source = Rf2Source::with_config(Rf2SourceConfig {
        dir: Some(dir.clone()),
        stall_timeout: Duration::from_millis(200),
    });
    assert_eq!(source.info().game, GameId::Rfactor2);

    // No game: no folder.
    assert!(matches!(
        source.connect(),
        Err(SourceError::NotAvailable(_))
    ));

    // A session, seen once: it could be buffers left behind. Seen again unchanged: still
    // not taken. Only buffers that moved are a game.
    game.step(0.01).write_dir(&dir).unwrap();
    assert!(matches!(
        source.connect(),
        Err(SourceError::NotAvailable(_))
    ));
    assert!(matches!(
        source.connect(),
        Err(SourceError::NotAvailable(_))
    ));
    game.step(0.01).write_dir(&dir).unwrap();
    source.connect().expect("a live session");

    // Frames follow the writes; nothing new, nothing sent.
    let mut frames = Vec::new();
    for _ in 0..40 {
        game.step(0.02).write_dir(&dir).unwrap();
        std::thread::sleep(Duration::from_millis(12));
        if let Some(f) = source.poll().unwrap() {
            frames.push(f);
        }
        assert!(source.poll().unwrap().is_none());
    }
    assert!(frames.len() >= 30, "{} frames", frames.len());
    let last = frames.last().unwrap();
    assert_eq!(last.session.game, GameId::Rfactor2);
    assert_eq!(last.source, "rf2-shm");
    let player = last.player.as_ref().unwrap();
    assert!(player.motion.speed_kmh > 10.0);
    assert!(frames.windows(2).all(|w| w[1].seq > w[0].seq));
    assert!(frames
        .windows(2)
        .all(|w| w[1].session.elapsed_s >= w[0].session.elapsed_s));

    // A pause: nothing moves, the game is still there. The source stays, silent.
    std::thread::sleep(Duration::from_millis(260));
    assert!(source.poll().unwrap().is_none());
    game.step(0.02).write_dir(&dir).unwrap();
    std::thread::sleep(Duration::from_millis(12));
    assert!(source.poll().unwrap().is_some());

    // The session ends: the plugin clears its buffers.
    game.ended().write_dir(&dir).unwrap();
    std::thread::sleep(Duration::from_millis(12));
    assert!(matches!(source.poll(), Err(SourceError::NotAvailable(_))));
    // … and blank buffers are not a session, however often they are looked at.
    assert!(matches!(
        source.connect(),
        Err(SourceError::NotAvailable(_))
    ));
    assert!(matches!(
        source.connect(),
        Err(SourceError::NotAvailable(_))
    ));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_game_that_vanishes_without_clearing_its_buffers_is_lost() {
    let dir = temp_dir("vanish");
    let mut game = FakeRf2::new();
    let mut source = Rf2Source::with_config(Rf2SourceConfig {
        dir: Some(dir.clone()),
        stall_timeout: Duration::from_millis(150),
    });
    game.step(0.01).write_dir(&dir).unwrap();
    assert!(source.connect().is_err());
    game.step(0.01).write_dir(&dir).unwrap();
    source.connect().unwrap();
    std::thread::sleep(Duration::from_millis(12));
    assert!(source.poll().unwrap().is_some());

    // The mappings are gone with the game.
    std::fs::remove_dir_all(&dir).unwrap();
    std::thread::sleep(Duration::from_millis(200));
    assert!(matches!(source.poll(), Err(SourceError::NotAvailable(_))));
}

#[test]
fn a_buffer_caught_in_the_middle_of_a_write_is_skipped_not_read() {
    let dir = temp_dir("torn");
    let mut game = FakeRf2::new();
    let mut source = Rf2Source::with_config(Rf2SourceConfig {
        dir: Some(dir.clone()),
        stall_timeout: Duration::from_secs(5),
    });
    game.step(0.01).write_dir(&dir).unwrap();
    assert!(source.connect().is_err());
    game.step(0.01).write_dir(&dir).unwrap();
    source.connect().unwrap();

    // The plugin bumped the first counter and has not finished writing.
    let mut raw = game.step(0.02);
    let v = u32::from_le_bytes(raw.telemetry[..4].try_into().unwrap());
    raw.telemetry[OFF_VERSION_BEGIN..OFF_VERSION_BEGIN + 4].copy_from_slice(&(v + 1).to_le_bytes());
    raw.write_dir(&dir).unwrap();
    std::thread::sleep(Duration::from_millis(12));
    assert!(source.poll().unwrap().is_none());

    // Once whole, it is read.
    game.step(0.02).write_dir(&dir).unwrap();
    std::thread::sleep(Duration::from_millis(12));
    assert!(source.poll().unwrap().is_some());
    let _ = std::fs::remove_dir_all(&dir);
}
