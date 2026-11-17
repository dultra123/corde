//! The adapter from the pages to what Corde does with them: the frame, the session clock,
//! the lap length, and — through the real lap tracker — the laps.

use corde_acc::convert::is_competizione;
use corde_acc::fake::FakeSession;
use corde_acc::layout::{session, status};
use corde_acc::{AccSource, AccSourceConfig, Converter};
use corde_core::{LapTracker, SourceError, TelemetrySource};
use corde_protocol::{
    CompoundType, GameId, GamePhase, LapRecord, SessionType, TelemetryFrame, VehicleClass,
};
use std::time::Duration;

fn at(conv: &mut Converter, fake: &FakeSession, t: f64) -> TelemetryFrame {
    conv.frame(
        &fake.at(t).pages().unwrap(),
        Duration::from_secs_f64(t),
        "test",
    )
}

/// Drives `seconds` at 100 Hz through the converter and the lap tracker.
fn drive(fake: &FakeSession, seconds: f64) -> (Vec<LapRecord>, TelemetryFrame) {
    let mut conv = Converter::new();
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

#[test]
fn a_frame_carries_what_the_game_publishes() {
    let fake = FakeSession::default();
    let mut conv = Converter::new();
    let mut f = at(&mut conv, &fake, 0.0);
    for i in 1..=2700 {
        f = at(&mut conv, &fake, i as f64 / 100.0); // up to a quarter of the lap
    }
    let s = &f.session;
    assert_eq!(s.game, GameId::Acc);
    assert_eq!(s.track_name, "Monza");
    assert_eq!(s.track_length_m, 5793.0);
    assert_eq!(s.session_type, SessionType::Practice);
    assert_eq!(s.phase, GamePhase::Green);
    assert!(s.in_realtime);
    assert!((s.elapsed_s - 27.0).abs() < 1e-6);
    assert_eq!(s.num_vehicles, 3);
    assert_eq!(s.player_name, "Alex Martin");
    assert_eq!(s.weather.ambient_c, 22.0);
    assert_eq!(s.weather.track_c, 31.0);

    let p = f.player.as_ref().unwrap();
    assert_eq!(p.id, 7);
    assert_eq!(p.vehicle_model, "bmw_m4_gt3");
    assert_eq!(p.vehicle_class, VehicleClass::Gt3);
    assert_eq!(p.class_name, "GT3");
    assert_eq!(p.place, 4);
    assert_eq!(
        p.engine.gear, 4,
        "the game counts reverse and neutral first"
    );
    assert_eq!(p.engine.rpm, 6500.0);
    assert_eq!(p.engine.max_rpm, 7500.0);
    assert!((p.motion.speed_kmh - 5793.0 / 108.0 * 3.6).abs() < 0.01);
    assert!((p.lap.lap_fraction - 0.25).abs() < 1e-4);
    assert!((p.lap.lap_dist_m - 0.25 * 5793.0).abs() < 1.0);
    assert!((p.lap.current_lap_s - 27.0).abs() < 0.01);
    assert!(p.lap.lap_start_s.abs() < 0.01);
    assert_eq!(p.lap.current_sector, 1);
    assert_eq!(
        p.lap.last_lap_s, None,
        "no lap yet: the game's « no time » is not a time"
    );
    assert_eq!(p.energy.fuel_capacity_l, 120.0);
    assert!(p.energy.fuel_l > 59.0 && p.energy.fuel_l < 60.0);
    // 27.0 psi.
    assert!((p.wheels[0].pressure_kpa - 186.16).abs() < 0.05);
    assert_eq!(p.wheels[2].carcass_c, 82.0);
    assert_eq!(p.wheels[3].brake_temp_c, 430.0);
    assert_eq!(p.wheels[0].compound_type, CompoundType::Unknown);
    assert_eq!(p.tyres.front_compound, "Dry");
    assert_eq!(p.electronics.tc, 3);
    assert_eq!(p.electronics.abs, 4);
    assert!((p.electronics.brake_bias_rear - 0.42).abs() < 1e-5);
    assert_eq!(p.gaps.car_ahead_s, Some(1.85));
    assert_eq!(p.gaps.car_behind_s, Some(0.64));
    // The player's own coordinates, found by its identifier among the cars on track.
    let radius = 5793.0 / std::f64::consts::TAU;
    assert!(p.motion.pos[0].abs() < 1.0 && (p.motion.pos[2] - radius).abs() < 1.0);
    // The pages name no other driver: the standings hold the player alone.
    assert_eq!(f.standings.len(), 1);
    assert!(f.standings[0].is_player);
    assert_eq!(f.standings[0].place, 4);
}

#[test]
fn laps_are_timed_by_the_game() {
    let (laps, last) = drive(&FakeSession::default(), 3.0 * 108.0 + 20.0);
    assert_eq!(laps.len(), 3, "{laps:#?}");
    for lap in &laps {
        assert!(lap.timed_by_game, "{lap:#?}");
        assert!((lap.lap_time_s - 108.0).abs() < 0.02, "{lap:#?}");
        assert!((lap.top_speed_kmh - 193.1).abs() < 0.5);
    }
    assert_eq!(
        laps.iter().map(|l| l.lap_number).collect::<Vec<_>>(),
        [0, 1, 2]
    );
    let p = last.player.unwrap();
    assert_eq!(p.lap.laps_completed, 3);
    assert_eq!(p.lap.last_lap_s, Some(108.0));
    assert_eq!(p.lap.best_lap_s, Some(108.0));
}

#[test]
fn a_lap_time_published_late_is_still_the_official_one() {
    let fake = FakeSession {
        last_time_delay_s: 0.3,
        ..FakeSession::default()
    };
    // Right after the line: the counter moved, the official count waits for the time.
    let mut conv = Converter::new();
    let mut f = at(&mut conv, &fake, 0.0);
    for i in 1..=10_810 {
        f = at(&mut conv, &fake, i as f64 / 100.0);
    }
    let p = f.player.as_ref().unwrap();
    assert_eq!((p.lap.lap_number, p.lap.laps_completed), (1, 0));
    assert_eq!(p.lap.last_lap_s, None);

    let (laps, _) = drive(&fake, 2.0 * 108.0 + 20.0);
    assert_eq!(laps.len(), 2, "{laps:#?}");
    assert!(laps
        .iter()
        .all(|l| l.timed_by_game && (l.lap_time_s - 108.0).abs() < 0.02));
}

#[test]
fn the_clock_only_runs_while_the_session_is_live() {
    let mut fake = FakeSession::default();
    let mut conv = Converter::new();
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
    fake.session_index = 1;
    let f = at(&mut conv, &fake, 61.0);
    assert_eq!(f.session.session_type, SessionType::Race);
    assert_eq!(f.session.elapsed_s, 0.0);
}

#[test]
fn an_unknown_circuit_gets_its_length_measured() {
    let fake = FakeSession {
        track: "somewhere_new".into(),
        length_m: 4000.0,
        lap_time_s: 90.0,
        ..FakeSession::default()
    };
    let mut conv = Converter::new();
    let mut f = at(&mut conv, &fake, 0.0);
    assert_eq!(f.session.track_name, "Somewhere New");
    assert_eq!(f.session.track_length_m, 0.0);
    // Not known during the first lap seen: the distance covered stands for it…
    for i in 1..=4500 {
        f = at(&mut conv, &fake, i as f64 / 100.0);
    }
    assert_eq!(f.session.track_length_m, 0.0);
    assert!((f.player.as_ref().unwrap().lap.lap_dist_m - 2000.0).abs() < 5.0);
    // … one full lap between two line crossings later, it is.
    for i in 4501..=18_500 {
        f = at(&mut conv, &fake, i as f64 / 100.0);
    }
    assert!(
        (f.session.track_length_m - 4000.0).abs() < 10.0,
        "{}",
        f.session.track_length_m
    );
    // The laps are timed all the same.
    let (laps, _) = drive(&fake, 2.0 * 90.0 + 20.0);
    assert_eq!(laps.len(), 2, "{laps:#?}");
    assert!(laps.iter().all(|l| l.timed_by_game));
}

#[test]
fn the_first_assetto_corsa_is_not_taken_for_competizione() {
    let acc = FakeSession::default();
    assert!(is_competizione(&acc.at(0.0).pages().unwrap().statics));
    let ac1 = FakeSession {
        sm_version: "1.7".into(),
        ..FakeSession::default()
    };
    assert!(!is_competizione(&ac1.at(0.0).pages().unwrap().statics));
    let empty = FakeSession {
        sm_version: String::new(),
        ..FakeSession::default()
    };
    assert!(!is_competizione(&empty.at(0.0).pages().unwrap().statics));
}

#[test]
fn a_diagnostic_copy_keeps_everything_but_the_players_name() {
    let raw = FakeSession::default().at(12.0);
    assert_eq!(raw.pages().unwrap().statics.player_name[0], 'A' as u16);
    let copy = raw.clone().without_names();
    let (before, after) = (raw.pages().unwrap(), copy.pages().unwrap());
    let s = after.statics;
    let (name, surname, nick) = (s.player_name, s.player_surname, s.player_nick);
    assert!(name.iter().chain(&surname).chain(&nick).all(|u| *u == 0));
    // The rest is untouched: the circuit, the car, what follows the names, the other pages.
    let (track, car) = (s.track, s.car_model);
    assert_eq!(corde_acc::layout::text(&track), "monza");
    assert_eq!(corde_acc::layout::text(&car), "bmw_m4_gt3");
    assert_eq!({ s.sector_count }, 3);
    assert_eq!({ s.max_rpm }, { before.statics.max_rpm });
    assert_eq!(copy.physics, raw.physics);
    assert_eq!(copy.graphics, raw.graphics);
    // Written and read back like the game's pages.
    let dir = temp_dir("dump");
    copy.write_dir(&dir).unwrap();
    let mut conv = Converter::new();
    let mut reader = corde_acc::reader::DirPages::open(&dir).unwrap();
    use corde_acc::PageReader;
    let f = conv.frame(&reader.read().unwrap(), Duration::ZERO, "test");
    assert_eq!(f.session.track_name, "Monza");
    assert_eq!(f.session.player_name, "");
    let _ = std::fs::remove_dir_all(&dir);
}

fn temp_dir(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "corde-acc-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

#[test]
fn the_source_follows_the_game_from_the_menus_to_a_session_and_back() {
    let dir = temp_dir("source");
    let mut source = AccSource::with_config(AccSourceConfig {
        dir: Some(dir.clone()),
        stall_timeout: Duration::from_millis(300),
        ..AccSourceConfig::default()
    });
    assert_eq!(source.info().game, GameId::Acc);
    // The game is not there.
    assert!(matches!(
        source.connect(),
        Err(SourceError::NotAvailable(_))
    ));

    // At the menus: the pages exist, no session.
    let mut fake = FakeSession {
        status: status::OFF,
        ..FakeSession::default()
    };
    fake.at(0.0).write_dir(&dir).unwrap();
    assert!(matches!(
        source.connect(),
        Err(SourceError::NotAvailable(_))
    ));

    // The pages of the first Assetto Corsa: not ours.
    fake.status = status::LIVE;
    fake.sm_version = "1.7".into();
    fake.at(0.0).write_dir(&dir).unwrap();
    assert!(matches!(
        source.connect(),
        Err(SourceError::NotAvailable(_))
    ));

    // A session.
    fake.sm_version = "1.9".into();
    fake.at(1.0).write_dir(&dir).unwrap();
    source.connect().unwrap();
    let first = source.poll().unwrap().expect("a frame");
    assert_eq!(first.session.game, GameId::Acc);
    assert_eq!(first.source, "acc-shm");
    // Nothing moved: nothing new.
    assert!(source.poll().unwrap().is_none());
    std::thread::sleep(Duration::from_millis(20));
    fake.at(1.02).write_dir(&dir).unwrap();
    let second = source.poll().unwrap().expect("a frame");
    assert!(second.seq > first.seq);

    // Nothing moves any more while the session is live: the game is gone.
    std::thread::sleep(Duration::from_millis(350));
    assert!(matches!(source.poll(), Err(SourceError::NotAvailable(_))));
    // Back to the menus: not available either, until a session starts again.
    fake.status = status::OFF;
    fake.at(2.0).write_dir(&dir).unwrap();
    assert!(matches!(
        source.connect(),
        Err(SourceError::NotAvailable(_))
    ));
    fake.status = status::LIVE;
    fake.at(3.0).write_dir(&dir).unwrap();
    source.connect().unwrap();
    assert!(source.poll().unwrap().is_some());
    // The session is left.
    fake.status = status::OFF;
    fake.at(3.5).write_dir(&dir).unwrap();
    assert!(matches!(source.poll(), Err(SourceError::NotAvailable(_))));
    let _ = std::fs::remove_dir_all(&dir);
}

// --- the first Assetto Corsa ----------------------------------------------------------

use corde_acc::Game;

/// Drives a session of the first Assetto Corsa through its own decoding.
fn drive_ac(fake: &FakeSession, seconds: f64) -> (Vec<LapRecord>, TelemetryFrame) {
    let mut conv = Converter::for_game(Game::AssettoCorsa);
    let mut tracker = LapTracker::new();
    let mut laps = Vec::new();
    let mut last = None;
    for i in 0..=(seconds * 100.0) as u64 {
        let t = i as f64 / 100.0;
        let pages = fake.at(t).pages_of(Game::AssettoCorsa).unwrap();
        let f = conv.frame(&pages, Duration::from_secs_f64(t), "test");
        laps.extend(tracker.feed(&f));
        last = Some(f);
    }
    (laps, last.unwrap())
}

#[test]
fn the_first_assetto_corsa_is_read_through_its_own_graphics_page() {
    let fake = FakeSession::assetto_corsa();
    let raw = fake.at(0.0);
    assert_eq!(raw.game().unwrap(), Some(Game::AssettoCorsa));
    assert_eq!(
        FakeSession::default().at(0.0).game().unwrap(),
        Some(Game::Competizione)
    );

    let (laps, f) = drive_ac(&fake, 2.0 * 118.0 + 29.5);
    let s = &f.session;
    assert_eq!(s.game, GameId::Ac);
    assert_eq!(s.track_name, "Nurburgring — Layout Gp A");
    // The game publishes the length of the lap: no table, nothing to measure.
    assert_eq!(s.track_length_m, 5137.0);
    assert_eq!(s.session_type, SessionType::Practice);
    assert!(s.in_realtime);
    assert_eq!(s.num_vehicles, 1);

    let p = f.player.as_ref().unwrap();
    assert_eq!(p.vehicle_model, "ks_ferrari_488_gt3");
    assert_eq!(p.vehicle_name, "Ferrari 488 Gt3");
    assert_eq!(p.vehicle_class, VehicleClass::Gt3);
    assert_eq!(p.engine.gear, 4);
    assert!((p.lap.lap_fraction - 0.25).abs() < 1e-3);
    assert!((p.lap.lap_dist_m - 0.25 * 5137.0).abs() < 2.0);
    assert!(!p.lap.lap_invalidated, "the game reports no invalid lap");
    // The player's coordinates: where the game writes them in its own page.
    let radius = 5137.0 / std::f64::consts::TAU;
    assert!(p.motion.pos[0].abs() < 2.0 && (p.motion.pos[2] - radius).abs() < 2.0);
    // Three temperatures per tyre, in the frame's order (left / centre / right of the
    // tyre): the inside of a left wheel is its right side.
    assert_eq!(
        p.wheels[0].temp_c,
        [70.0, 80.0, 90.0],
        "front left: outside first"
    );
    assert_eq!(
        p.wheels[1].temp_c,
        [91.0, 80.0, 71.0],
        "front right: inside first"
    );
    assert!((p.wheels[0].wear_remaining - 0.975).abs() < 1e-6);
    assert_eq!(p.wheels[0].load_n, 3200.0);
    assert!((p.wheels[0].camber_deg + 2.865).abs() < 0.01);
    assert_eq!(p.wheels[0].ride_height_m, 0.055);
    assert_eq!(p.wheels[3].ride_height_m, 0.072);
    assert_eq!(p.wheels[0].compound_type, CompoundType::Soft);
    assert_eq!(p.tyres.front_compound, "Soft (S)");
    // Not published by this game: no gaps, no electronics levels.
    assert_eq!(p.gaps.car_ahead_s, None);
    assert_eq!(p.electronics.tc, 0);

    // Its laps are timed like the others.
    assert_eq!(laps.len(), 2, "{laps:#?}");
    assert!(laps
        .iter()
        .all(|l| l.timed_by_game && (l.lap_time_s - 118.0).abs() < 0.02));
}

#[test]
fn each_game_only_answers_to_its_own_source() {
    let dir = temp_dir("family");
    let mut acc = AccSource::with_config(AccSourceConfig {
        dir: Some(dir.clone()),
        ..AccSourceConfig::default()
    });
    let mut ac = AccSource::with_config(AccSourceConfig {
        game: Game::AssettoCorsa,
        dir: Some(dir.clone()),
        ..AccSourceConfig::default()
    });
    assert_eq!(acc.info().game, GameId::Acc);
    assert_eq!(ac.info().game, GameId::Ac);
    assert_eq!(ac.info().id, "ac-shm");
    assert_eq!(ac.info().label, "Assetto Corsa");

    // The first Assetto Corsa is running.
    FakeSession::assetto_corsa()
        .at(1.0)
        .write_dir(&dir)
        .unwrap();
    assert!(matches!(acc.connect(), Err(SourceError::NotAvailable(_))));
    ac.connect().unwrap();
    let f = ac.poll().unwrap().expect("a frame");
    assert_eq!(f.session.game, GameId::Ac);
    assert_eq!(f.source, "ac-shm");
    ac.disconnect();

    // Competizione is running.
    FakeSession::default().at(1.0).write_dir(&dir).unwrap();
    assert!(matches!(ac.connect(), Err(SourceError::NotAvailable(_))));
    acc.connect().unwrap();
    assert_eq!(
        acc.poll().unwrap().expect("a frame").session.game,
        GameId::Acc
    );
    let _ = std::fs::remove_dir_all(&dir);
}
