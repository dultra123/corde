//! Assetto Corsa EVO: from its pages to what Corde does with them — the frame, the
//! session clock, the laps through the real lap tracker, the source.

use corde_acc::evo::fake::{FakeEvoSession, PLAYER_SLOT};
use corde_acc::evo::layout::{session, status, text};
use corde_acc::evo::{EvoConverter, EvoSource, EvoSourceConfig};
use corde_acc::reader::{DirPages, EVO_PAGES};
use corde_acc::{AccSource, AccSourceConfig, PageReader};
use corde_core::{LapTracker, SourceError, TelemetrySource};
use corde_protocol::{
    CompoundType, GameId, GamePhase, LapRecord, SessionType, TelemetryFrame, VehicleClass,
};
use std::time::Duration;

fn at(conv: &mut EvoConverter, fake: &FakeEvoSession, t: f64) -> TelemetryFrame {
    conv.frame(
        &fake.at(t).evo().unwrap(),
        Duration::from_secs_f64(t),
        "test",
    )
}

/// Drives `seconds` at 100 Hz through the converter and the lap tracker.
fn drive(fake: &FakeEvoSession, seconds: f64) -> (Vec<LapRecord>, TelemetryFrame) {
    let mut conv = EvoConverter::new();
    let mut tracker = LapTracker::new();
    let mut laps = Vec::new();
    let mut last = None;
    let steps = (seconds * 100.0) as u64;
    for i in 0..=steps {
        let f = at(&mut conv, fake, i as f64 / 100.0);
        laps.extend(tracker.feed(&f));
        last = Some(f);
    }
    (laps, last.unwrap())
}

fn temp_dir(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "corde-acevo-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

#[test]
fn a_frame_carries_what_the_game_publishes() {
    let fake = FakeEvoSession::default();
    let mut conv = EvoConverter::new();
    let mut f = at(&mut conv, &fake, 0.0);
    for i in 1..=2100 {
        f = at(&mut conv, &fake, i as f64 / 100.0); // up to a quarter of the lap
    }
    let s = &f.session;
    assert_eq!(s.game, GameId::AcEvo);
    assert_eq!(s.track_name, "Brands Hatch — GP");
    assert_eq!(s.track_length_m, 3908.0);
    assert_eq!(s.session_type, SessionType::Practice);
    assert_eq!(s.phase, GamePhase::Green);
    assert!(s.in_realtime);
    assert!((s.elapsed_s - 21.0).abs() < 1e-6);
    assert_eq!(s.num_vehicles, 3);
    assert_eq!(s.player_name, "Alex Martin");
    assert_eq!(s.weather.ambient_c, 21.0);
    assert_eq!(s.weather.track_c, 29.0);
    assert_eq!(s.weather.time_of_day_s, 14.5 * 3600.0);
    assert!((s.time_remaining_s - (3600.0 - 21.0)).abs() < 0.01);

    let p = f.player.as_ref().unwrap();
    assert_eq!(p.id, PLAYER_SLOT as i32);
    assert_eq!(p.vehicle_model, "BMW M4 GT3");
    assert_eq!(p.vehicle_class, VehicleClass::Gt3);
    assert_eq!(p.class_name, "GT3");
    assert_eq!(p.place, 4);
    assert_eq!(
        p.engine.gear, 4,
        "the game counts reverse and neutral first"
    );
    assert_eq!(p.engine.rpm, 6500.0);
    assert_eq!(
        p.engine.max_rpm, 7400.0,
        "the red line is in the physics page"
    );
    assert_eq!(p.engine.max_gears, 6);
    assert_eq!(p.engine.oil_c, 104.0);
    assert_eq!(p.engine.water_c, 88.0);
    assert!((p.motion.speed_kmh - 3908.0 / 84.0 * 3.6).abs() < 0.01);
    assert!((p.lap.lap_fraction - 0.25).abs() < 1e-4);
    assert!((p.lap.lap_dist_m - 0.25 * 3908.0).abs() < 1.0);
    assert!((p.lap.current_lap_s - 21.0).abs() < 0.01);
    assert_eq!(p.lap.current_sector, 1);
    assert_eq!(p.lap.last_lap_s, None, "no lap yet: zero is not a time");
    assert!(!p.lap.lap_invalidated);
    assert!((p.lap.delta_best_s + 0.12).abs() < 1e-6);
    assert_eq!(p.lap.estimated_lap_s, 84.0);
    assert_eq!(
        p.energy.fuel_capacity_l, 120.0,
        "the tank is in the graphics page"
    );
    assert!(p.energy.fuel_l > 59.0 && p.energy.fuel_l < 60.0);
    assert_eq!(p.energy.battery_soc, None);
    // 26.0 psi.
    assert!((p.wheels[0].pressure_kpa - 179.26).abs() < 0.05);
    assert_eq!(p.wheels[2].carcass_c, 82.0);
    assert_eq!(p.wheels[3].brake_temp_c, 430.0);
    // Left, centre, right of the tread, as the car sees them.
    assert_eq!(p.wheels[1].temp_c, [71.0, 81.0, 91.0]);
    assert_eq!(
        p.wheels[0].wear_remaining, 1.0,
        "the game leaves the wear at zero"
    );
    assert_eq!(p.wheels[1].load_n, 3300.0);
    // Metres at the front; the millimetres of the rear axle are brought back to metres.
    assert!((p.wheels[0].ride_height_m - 0.058).abs() < 1e-6);
    assert!((p.wheels[3].ride_height_m - 0.071).abs() < 1e-6);
    // The camber of a right wheel is brought back to the sense of the left ones.
    assert!((p.wheels[0].camber_deg + 2.865).abs() < 1e-2);
    assert!((p.wheels[1].camber_deg + 2.865).abs() < 1e-2);
    assert_eq!(p.wheels[0].compound_type, CompoundType::Medium);
    assert_eq!(p.wheels[2].compound_type, CompoundType::Soft);
    assert_eq!(
        (
            p.tyres.front_compound.as_str(),
            p.tyres.rear_compound.as_str()
        ),
        ("Medium", "Soft")
    );
    // The levels of the wheel, each with its maximum.
    let e = &p.electronics;
    assert_eq!((e.tc, e.tc_max), (3, 11));
    assert_eq!((e.tc_cut, e.tc_cut_max), (2, 6));
    assert_eq!((e.abs, e.abs_max), (4, 9));
    assert_eq!((e.motor_map, e.motor_map_max), (1, 5));
    assert!((e.brake_bias_rear - 0.43).abs() < 1e-5);
    assert!(e.headlights);
    assert_eq!(e.wiper, 2);
    assert_eq!(p.gaps.car_ahead_s, Some(1.85));
    assert_eq!(p.gaps.car_behind_s, Some(0.64));
    // The player's own coordinates, found by its identifier among the cars. The third
    // axis is turned round, as for Competizione.
    let radius = 3908.0 / std::f64::consts::TAU;
    assert!(p.motion.pos[0].abs() < 1.0 && (p.motion.pos[2] + radius).abs() < 1.0);
    // A right-hand bend, accelerating: positive sideways and in yaw, as with the other
    // games; this game orders [lateral, longitudinal, vertical].
    assert!((p.motion.g_lat - 1.2).abs() < 1e-6);
    assert!((p.motion.g_long - 0.3).abs() < 1e-6);
    assert!((p.motion.g_vert - 0.02).abs() < 1e-6);
    assert!((p.motion.yaw_rate_dps - 11.459).abs() < 1e-2);
    assert_eq!(p.inputs.throttle, 0.8);
    assert_eq!(p.inputs.clutch, 0.0);
    assert_eq!(p.inputs.steering, 0.05);
    assert_eq!(p.inputs.steering_angle_deg, 14.0);
    assert!(!p.in_pits && !p.in_garage);
    // The pages name no other driver: the standings hold the player alone.
    assert_eq!(f.standings.len(), 1);
    assert!(f.standings[0].is_player);

    // A circuit with one layout, and one the game does not prefix.
    let plain = FakeEvoSession {
        track: "imola".into(),
        track_configuration: String::new(),
        ..FakeEvoSession::default()
    };
    assert_eq!(
        at(&mut EvoConverter::new(), &plain, 5.0).session.track_name,
        "Imola"
    );
}

#[test]
fn the_player_is_found_without_the_table_of_identifiers() {
    // A build from before April 2026 does not say which car sits in which slot: the
    // first slot is taken, which is the player's when alone on track.
    let old = FakeEvoSession {
        car_ids: false,
        ..FakeEvoSession::default()
    };
    let f = at(&mut EvoConverter::new(), &old, 21.0);
    let p = f.player.unwrap();
    assert_eq!(p.id, 0);
    assert!(p.motion.pos[0].is_finite());
}

#[test]
fn laps_are_timed_by_the_game() {
    let (laps, last) = drive(&FakeEvoSession::default(), 3.0 * 84.0 + 20.0);
    assert_eq!(laps.len(), 3, "{laps:#?}");
    for lap in &laps {
        assert!(lap.timed_by_game && lap.valid, "{lap:#?}");
        assert!((lap.lap_time_s - 84.0).abs() < 0.02, "{lap:#?}");
    }
    let p = last.player.unwrap();
    assert_eq!(p.lap.laps_completed, 3);
    assert_eq!(p.lap.last_lap_s, Some(84.0));
    assert_eq!(p.lap.best_lap_s, Some(84.0));
}

#[test]
fn a_lap_time_published_late_is_still_the_official_one_and_never_borrowed() {
    let late = FakeEvoSession {
        last_time_delay_s: 0.3,
        time_step_ms: 3,
        ..FakeEvoSession::default()
    };
    // Right after the line: the counter moved, the official count waits for the time.
    let mut conv = EvoConverter::new();
    let mut f = at(&mut conv, &late, 0.0);
    for i in 1..=8_410 {
        f = at(&mut conv, &late, i as f64 / 100.0);
    }
    let p = f.player.as_ref().unwrap();
    assert_eq!((p.lap.lap_number, p.lap.laps_completed), (1, 0));
    assert_eq!(p.lap.last_lap_s, None);
    let (laps, _) = drive(&late, 2.0 * 84.0 + 20.0);
    assert_eq!(laps.len(), 2, "{laps:#?}");
    assert!(laps.iter().all(|l| l.timed_by_game), "{laps:#?}");

    // A time that takes ten seconds to come: Corde stops waiting after one, and the time
    // of the lap before is withdrawn rather than given to the new lap.
    let never = FakeEvoSession {
        last_time_delay_s: 10.0,
        time_step_ms: 150,
        ..FakeEvoSession::default()
    };
    let mut conv = EvoConverter::new();
    let mut seen = Vec::new();
    for i in 0..=(2 * 84 + 20) * 100 {
        let f = at(&mut conv, &never, i as f64 / 100.0);
        let lap = &f.player.as_ref().unwrap().lap;
        seen.push((lap.laps_completed, lap.last_lap_s));
    }
    let lap = |t: f64| seen[(t * 100.0).round() as usize];
    let official = |k: i32| Some(never.lap_ms(k) as f32 / 1000.0);
    assert_eq!(lap(168.5), (1, official(1)));
    assert_eq!(lap(169.5), (2, None));
    assert_eq!(lap(177.0), (2, None));
    assert_eq!(lap(179.0), (2, official(2)));
}

#[test]
fn the_clock_only_runs_while_the_session_is_live() {
    let mut fake = FakeEvoSession::default();
    let mut conv = EvoConverter::new();
    at(&mut conv, &fake, 0.0);
    at(&mut conv, &fake, 0.4);
    fake.status = status::PAUSE;
    let paused = at(&mut conv, &fake, 0.8);
    assert_eq!(paused.session.phase, GamePhase::Paused);
    assert!((paused.session.elapsed_s - 0.4).abs() < 1e-9);
    fake.status = status::LIVE;
    // A reader that stalled for a minute does not make the clock jump.
    let f = at(&mut conv, &fake, 60.0);
    assert!((f.session.elapsed_s - 0.9).abs() < 1e-9);
    // Another session: the clock starts again.
    fake.session = session::RACE;
    fake.session_id = 1;
    let f = at(&mut conv, &fake, 61.0);
    assert_eq!(f.session.session_type, SessionType::Race);
    assert_eq!(f.session.elapsed_s, 0.0);
}

#[test]
fn a_diagnostic_copy_names_nobody_and_still_finds_the_player() {
    let raw = FakeEvoSession::default().at(21.0);
    let before = raw.evo().unwrap().graphics;
    let (name, id) = (before.driver_name, before.player_car_id);
    assert_eq!(text(&name), "Alex");
    assert_ne!(id, [0, 0]);

    let copy = raw.clone().evo_without_names();
    let g = copy.evo().unwrap().graphics;
    let (name, surname, car) = (g.driver_name, g.driver_surname, g.car_model);
    assert_eq!(
        (text(&name), text(&surname)),
        (String::new(), String::new())
    );
    assert_eq!(text(&car), "ks_bmw_m4_gt3");
    // The identifiers are replaced by small numbers; the same car keeps the same one.
    let (me, focus, ids) = (g.player_car_id, g.focused_car_id, g.car_ids);
    assert_eq!(me, [1, 0]);
    assert_eq!(focus, me);
    assert_eq!(ids[PLAYER_SLOT], me);
    assert_eq!([ids[0], ids[2]], [[2, 0], [3, 0]]);
    assert_eq!(ids[3], [0, 0], "an empty slot stays empty");
    // Nothing else changed.
    assert_eq!(copy.physics, raw.physics);
    assert_eq!(copy.statics, raw.statics);
    let changed = copy
        .graphics
        .iter()
        .zip(&raw.graphics)
        .filter(|(a, b)| a != b)
        .count();
    assert!(changed <= 66 + 5 * 16, "{changed} bytes changed");

    // Written and read back like the game's pages.
    let dir = temp_dir("dump");
    copy.write_dir_of(&dir, &EVO_PAGES).unwrap();
    let mut reader = DirPages::open_of(&dir, &EVO_PAGES).unwrap();
    let f = EvoConverter::new().frame(
        &reader.read_raw().unwrap().evo().unwrap(),
        Duration::ZERO,
        "test",
    );
    assert_eq!(f.session.track_name, "Brands Hatch — GP");
    assert_eq!(f.session.player_name, "");
    let p = f.player.unwrap();
    assert_eq!(p.id, PLAYER_SLOT as i32);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_source_follows_the_game_from_the_menus_to_a_session_and_back() {
    let dir = temp_dir("source");
    let mut source = EvoSource::with_config(EvoSourceConfig {
        dir: Some(dir.clone()),
        stall_timeout: Duration::from_millis(300),
        ..EvoSourceConfig::default()
    });
    assert_eq!(source.info().game, GameId::AcEvo);
    assert_eq!(source.info().label, "Assetto Corsa EVO");
    // The game is not there.
    assert!(matches!(
        source.connect(),
        Err(SourceError::NotAvailable(_))
    ));

    // At the menus: the pages exist, no session.
    let mut fake = FakeEvoSession {
        status: status::OFF,
        ..FakeEvoSession::default()
    };
    fake.at(0.0).write_dir_of(&dir, &EVO_PAGES).unwrap();
    assert!(matches!(
        source.connect(),
        Err(SourceError::NotAvailable(_))
    ));

    // A session.
    fake.status = status::LIVE;
    fake.at(1.0).write_dir_of(&dir, &EVO_PAGES).unwrap();
    source.connect().unwrap();
    let first = source.poll().unwrap().expect("a frame");
    assert_eq!(first.session.game, GameId::AcEvo);
    assert_eq!(first.source, "acevo-shm");
    // Nothing moved: nothing new.
    assert!(source.poll().unwrap().is_none());
    std::thread::sleep(Duration::from_millis(20));
    fake.at(1.02).write_dir_of(&dir, &EVO_PAGES).unwrap();
    let second = source.poll().unwrap().expect("a frame");
    assert!(second.seq > first.seq);

    // Nothing moves any more while the session is live: the game is gone.
    std::thread::sleep(Duration::from_millis(350));
    assert!(matches!(source.poll(), Err(SourceError::NotAvailable(_))));
    fake.at(3.0).write_dir_of(&dir, &EVO_PAGES).unwrap();
    source.connect().unwrap();
    assert!(source.poll().unwrap().is_some());
    // The session is left.
    fake.status = status::OFF;
    fake.at(3.5).write_dir_of(&dir, &EVO_PAGES).unwrap();
    assert!(matches!(source.poll(), Err(SourceError::NotAvailable(_))));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_pages_of_this_game_are_not_those_of_the_others() {
    // Its pages have their own names: the sources of Competizione and of the first
    // Assetto Corsa do not see them, and its own source does not see theirs.
    let dir = temp_dir("names");
    FakeEvoSession::default()
        .at(1.0)
        .write_dir_of(&dir, &EVO_PAGES)
        .unwrap();
    for mut other in [
        AccSource::with_config(AccSourceConfig {
            dir: Some(dir.clone()),
            ..AccSourceConfig::default()
        }),
        AccSource::with_config(AccSourceConfig {
            game: corde_acc::Game::AssettoCorsa,
            dir: Some(dir.clone()),
            ..AccSourceConfig::default()
        }),
    ] {
        assert!(matches!(other.connect(), Err(SourceError::NotAvailable(_))));
    }
    let theirs = temp_dir("names-acc");
    corde_acc::fake::FakeSession::default()
        .at(1.0)
        .write_dir(&theirs)
        .unwrap();
    let mut evo = EvoSource::with_config(EvoSourceConfig {
        dir: Some(theirs.clone()),
        ..EvoSourceConfig::default()
    });
    assert!(matches!(evo.connect(), Err(SourceError::NotAvailable(_))));
    for d in [dir, theirs] {
        let _ = std::fs::remove_dir_all(d);
    }
}

#[test]
fn pages_cut_short_are_refused_not_read() {
    let mut raw = FakeEvoSession::default().at(1.0);
    raw.graphics.truncate(1588); // the size of Competizione's page
    assert!(matches!(raw.evo(), Err(SourceError::Invalid(_))));

    // The page of a build from before the table of identifiers: read, the table at zero.
    let mut old = FakeEvoSession::default().at(21.0);
    old.graphics
        .truncate(corde_acc::evo::layout::GRAPHICS_EVO_MIN_SIZE);
    let g = old.evo().unwrap().graphics;
    let (ids, speed) = (g.car_ids, g.npos);
    assert!(ids.iter().all(|id| *id == [0, 0]));
    assert!((speed - 0.25).abs() < 1e-4);
    let f = EvoConverter::new().frame(&old.evo().unwrap(), Duration::ZERO, "test");
    assert_eq!(f.player.unwrap().id, 0);
}
