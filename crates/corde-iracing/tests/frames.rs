//! The adapter from the mapping to what Corde does with it: the frame, the lap count
//! waited for with its time, and — through the real lap tracker — the laps.

use corde_core::{LapTracker, SourceError, TelemetrySource};
use corde_iracing::fake::{self, FakeSession, PLAYER, RIVAL};
use corde_iracing::reader::{self, MemMap};
use corde_iracing::{Converter, Image, IracingSource, IracingSourceConfig, SessionDesc};
use corde_protocol::{
    CompoundType, Control, GameId, GamePhase, LapRecord, SessionType, TelemetryFrame, VehicleClass,
};
use std::time::Duration;

fn frame_of(conv: &mut Converter, image: &Image) -> TelemetryFrame {
    let desc = SessionDesc::parse(&image.session_text);
    conv.frame(&desc, image.table.row(&image.rows[0].1), "test")
}

/// Drives `seconds` at 60 Hz through the converter and the lap tracker. `fake_at` says
/// what the game publishes at each instant.
fn drive_with(
    seconds: f64,
    fake_at: impl Fn(f64) -> FakeSession,
) -> (Vec<LapRecord>, Vec<TelemetryFrame>) {
    let mut conv = Converter::new();
    let mut tracker = LapTracker::new();
    let mut laps = Vec::new();
    let mut frames = Vec::new();
    // The table and the session text do not change during a drive.
    let table = fake::table();
    let desc = SessionDesc::parse(&fake::session_text(&fake_at(0.0)));
    let steps = (seconds * 60.0) as u64;
    for i in 0..=steps {
        let t = i as f64 / 60.0;
        let f = conv.frame(&desc, table.row(&fake_at(t).row(&table, t)), "test");
        laps.extend(tracker.feed(&f));
        frames.push(f);
    }
    (laps, frames)
}

fn drive(fake: &FakeSession, seconds: f64) -> (Vec<LapRecord>, Vec<TelemetryFrame>) {
    drive_with(seconds, |_| fake.clone())
}

/// The frame at `t` seconds of a drive at 60 Hz.
fn at(frames: &[TelemetryFrame], t: f64) -> &TelemetryFrame {
    &frames[(t * 60.0).round() as usize]
}

#[test]
fn a_frame_carries_what_the_game_publishes() {
    let fake = FakeSession::default();
    let mut conv = Converter::new();
    frame_of(&mut conv, &fake.at(0.0));
    let f = frame_of(&mut conv, &fake.at(10.0));
    assert_eq!(f.seq, 2);
    let s = &f.session;
    assert_eq!(s.game, GameId::Iracing);
    assert_eq!(
        s.track_name,
        "Circuit de Spa-Francorchamps — Grand Prix Pits"
    );
    assert!((s.track_length_m - 6930.0).abs() < 1.0);
    assert_eq!(s.session_type, SessionType::Practice);
    assert_eq!(s.phase, GamePhase::Green);
    assert!(s.in_realtime);
    assert_eq!(s.elapsed_s, 510.0);
    // A practice has neither an end nor a number of laps.
    assert_eq!((s.end_s, s.time_remaining_s, s.max_laps), (0.0, 0.0, 0));
    assert_eq!(s.player_name, "Pilote Essai");
    assert_eq!(s.weather.ambient_c, 21.0);
    assert_eq!(s.weather.track_c, 29.5);
    assert_eq!(s.weather.wetness_avg, 0.0, "1 is « dry »");

    let p = f.player.as_ref().expect("the player's car");
    assert_eq!(p.id, PLAYER as i32);
    assert_eq!(p.vehicle_name, "BMW M4 GT3");
    assert_eq!(p.vehicle_model, "bmwm4gt3");
    assert_eq!(p.vehicle_class, VehicleClass::Gt3);
    assert_eq!(p.place, 2);
    assert!((p.motion.speed_kmh - 249.48).abs() < 0.01);
    // The game: positive to the left. Corde: positive to the right.
    assert!((p.motion.g_lat + 1.0).abs() < 1e-4);
    assert!((p.motion.g_long + 0.5).abs() < 1e-4, "braking");
    assert_eq!(p.engine.gear, 4);
    assert_eq!(p.engine.rpm, 6200.0);
    assert_eq!(p.engine.max_rpm, 7500.0);
    assert_eq!(p.engine.max_gears, 6);
    assert_eq!(p.inputs.throttle, 0.8);
    assert_eq!(
        p.inputs.clutch, 0.0,
        "the game says « engaged », Corde the pedal"
    );
    assert!((p.inputs.steering + 0.125).abs() < 1e-6, "left of centre");
    assert!((p.energy.fuel_l - 59.8).abs() < 1e-3);
    assert_eq!(p.energy.fuel_capacity_l, 120.0);
    assert_eq!((p.electronics.tc, p.electronics.abs), (4, 3));
    assert!((p.electronics.brake_bias_rear - 0.455).abs() < 1e-4);
    assert_eq!(p.wheels[0].temp_c, [80.0, 85.0, 90.0]);
    assert_eq!(p.wheels[3].temp_c, [83.0, 88.0, 93.0]);
    assert!((p.wheels[1].wear_remaining - 0.9).abs() < 1e-6);
    assert_eq!(p.wheels[2].pressure_kpa, 174.0);
    assert_eq!(p.wheels[0].compound_type, CompoundType::Hard);
    assert_eq!(p.tyres.front_compound, "Hard");
    assert_eq!(p.lap.current_sector, 2, "60 % of the lap");
    assert!((p.lap.lap_fraction - 0.6).abs() < 1e-4);
    assert!((p.lap.lap_dist_m - 4158.0).abs() < 1.0);
    assert!(!p.lap.lap_invalidated);
    // Metres from where the session was first seen.
    assert!(
        p.motion.pos[0] > 1.0 && p.motion.pos[2] < -1.0,
        "{:?}",
        p.motion.pos
    );
    assert_eq!(p.motion.pos[1], 410.0);

    // The cars: the rival then the player; neither the pace car (not out) nor the
    // spectator.
    assert_eq!(f.standings.len(), 2);
    assert_eq!(s.num_vehicles, 2);
    let (first, second) = (&f.standings[0], &f.standings[1]);
    assert_eq!(
        (first.id, first.place, first.is_player),
        (RIVAL as i32, 1, false)
    );
    assert_eq!(first.control, Control::Remote);
    assert_eq!(first.vehicle_name, "Ferrari 296 GT3");
    assert!((first.lap_fraction - 0.63).abs() < 1e-4);
    assert_eq!(
        (second.id, second.place, second.is_player),
        (PLAYER as i32, 2, true)
    );
    assert_eq!(second.control, Control::LocalPlayer);
    // Three seconds behind the car ahead on the road, nobody behind.
    assert!((p.gaps.car_ahead_s.unwrap() - 3.0).abs() < 1e-3);
    assert_eq!(p.gaps.car_behind_s, None);
    assert_eq!(p.gaps.place_ahead_s, None, "not a race");

    // The race: an end, a number of laps, gaps between places.
    let race = FakeSession::race();
    let f = frame_of(&mut Converter::new(), &race.at(10.0));
    assert_eq!(f.session.session_type, SessionType::Race);
    assert_eq!(f.session.max_laps, 12);
    assert_eq!(f.session.time_remaining_s, 1790.0);
    assert_eq!(f.session.end_s, 510.0 + 1790.0);
    let p = f.player.as_ref().unwrap();
    assert_eq!(p.gaps.place_ahead_s, Some(3.0));
    assert_eq!(f.standings[1].time_behind_next_s, 3.0);
}

#[test]
fn laps_get_the_times_of_the_game() {
    // The line at 50, 150, 250 and 350 s; each time published half a second later.
    let fake = FakeSession::default();
    let (laps, frames) = drive(&fake, 400.0);
    assert_eq!(laps.len(), 4, "{laps:#?}");
    // The lap joined on the way: the game publishes no time for it.
    assert_eq!(laps[0].lap_number, 0);
    for (lap, k) in laps[1..].iter().zip(2..) {
        assert_eq!(lap.lap_number, k - 1);
        assert!(lap.timed_by_game && lap.valid, "{lap:#?}");
        assert_eq!(
            lap.lap_time_s,
            fake.lap_time(k),
            "the official time, not ours"
        );
    }
    // The boundary at once; the official count with the time of the lap.
    let lap = |t: f64| at(&frames, t).player.as_ref().unwrap().lap.clone();
    assert_eq!((lap(149.9).lap_number, lap(149.9).laps_completed), (1, 0));
    assert_eq!((lap(150.1).lap_number, lap(150.1).laps_completed), (2, 0));
    assert_eq!(lap(150.1).last_lap_s, None);
    assert_eq!((lap(150.6).lap_number, lap(150.6).laps_completed), (2, 1));
    assert_eq!(lap(150.6).last_lap_s, Some(fake.lap_time(2)));
    assert_eq!(lap(399.0).best_lap_s, Some(fake.lap_time(2)));
    assert_eq!(lap(399.0).laps_completed, 3);

    // The other way the game may count (a lap completed at each crossing): same laps.
    let other = FakeSession {
        completed_offset: 0,
        ..FakeSession::default()
    };
    let (laps, _) = drive(&other, 400.0);
    assert_eq!(laps.len(), 4, "{laps:#?}");
    assert!(!laps[0].timed_by_game, "joined on the way: no time");
    for (lap, k) in laps[1..].iter().zip(2..) {
        assert!(lap.timed_by_game && lap.valid, "{lap:#?}");
        assert_eq!(lap.lap_time_s, other.lap_time(k));
    }
}

#[test]
fn a_lap_the_game_refuses_has_no_time() {
    // The lap that ends at the third crossing (250 s) is refused: the game publishes −1.
    let fake = FakeSession {
        refused: vec![3],
        ..FakeSession::default()
    };
    let (laps, frames) = drive(&fake, 400.0);
    assert_eq!(laps.len(), 4);
    assert!(laps[1].timed_by_game);
    assert!(!laps[2].timed_by_game && !laps[2].valid, "{:#?}", laps[2]);
    assert!(laps[3].timed_by_game && laps[3].valid);
    assert_eq!(laps[3].lap_time_s, fake.lap_time(4));
    let lap = at(&frames, 251.0).player.as_ref().unwrap().lap.clone();
    assert_eq!(
        (lap.laps_completed, lap.last_lap_s),
        (2, None),
        "counted, without a time"
    );
}

#[test]
fn a_time_that_never_comes_is_not_borrowed_from_the_lap_before() {
    // The game takes 10 s to publish a time: Corde stops waiting after 2 s.
    let fake = FakeSession {
        last_time_delay_s: 10.0,
        time_step_s: 0.15,
        ..FakeSession::default()
    };
    let (laps, frames) = drive(&fake, 400.0);
    let lap = |t: f64| at(&frames, t).player.as_ref().unwrap().lap.clone();
    // Third crossing at 250 s: until 260 s the game still shows the time of the lap
    // before. It is shown while waiting, then withdrawn — never given to the new lap.
    assert_eq!(lap(251.0).last_lap_s, Some(fake.lap_time(2)));
    assert_eq!(lap(251.0).laps_completed, 1);
    assert_eq!(lap(252.5).laps_completed, 2);
    assert_eq!(lap(252.5).last_lap_s, None);
    assert_eq!(lap(259.0).last_lap_s, None);
    assert_eq!(lap(261.0).last_lap_s, Some(fake.lap_time(3)));
    assert!(!laps[2].timed_by_game, "{:#?}", laps[2]);

    // Two laps of exactly the same time: the time does not change, and it is the right one.
    let same = FakeSession {
        time_step_s: 0.0,
        ..FakeSession::default()
    };
    let (laps, frames) = drive(&same, 400.0);
    assert_eq!(laps.len(), 4);
    assert!(laps[2].timed_by_game && laps[2].valid, "{:#?}", laps[2]);
    assert_eq!(laps[2].lap_time_s, 100.0);
    assert_eq!(
        at(&frames, 253.0).player.as_ref().unwrap().lap.last_lap_s,
        Some(100.0)
    );
}

#[test]
fn leaving_the_car_does_not_restart_the_count() {
    // Out of the world between 180 and 200 s (the counters of the game read 0 then).
    let fake = FakeSession::default();
    let (laps, frames) = drive_with(400.0, |t| FakeSession {
        in_world: !(180.0..200.0).contains(&t),
        ..fake.clone()
    });
    let lap = |t: f64| at(&frames, t).player.as_ref().unwrap().lap.clone();
    assert_eq!((lap(170.0).lap_number, lap(170.0).laps_completed), (2, 1));
    assert_eq!(
        (lap(190.0).lap_number, lap(190.0).laps_completed),
        (2, 1),
        "held"
    );
    assert!(!at(&frames, 190.0).session.in_realtime);
    assert_eq!((lap(210.0).lap_number, lap(210.0).laps_completed), (2, 1));
    // The laps before and after are those of the game.
    assert!(laps.iter().any(|l| l.lap_number == 1 && l.timed_by_game));
    assert!(laps
        .iter()
        .any(|l| l.lap_number == 3 && l.timed_by_game && l.lap_time_s == fake.lap_time(4)));

    // Another session of the event: the count starts again from what the game says.
    let mut conv = Converter::new();
    let f = frame_of(&mut conv, &fake.at(160.0));
    assert_eq!(f.player.as_ref().unwrap().lap.lap_number, 2);
    let race = FakeSession {
        first_lap: 0,
        start_fraction: 0.0,
        clock_at_start: 0.0,
        ..FakeSession::race()
    };
    let f = frame_of(&mut conv, &race.at(1.0));
    let p = f.player.as_ref().unwrap();
    assert_eq!((p.lap.lap_number, p.lap.laps_completed), (0, 0));
    assert_eq!(f.session.session_index, 1);

    // An incident during the lap: the lap is marked.
    let mut conv = Converter::new();
    frame_of(&mut conv, &fake.at(60.0));
    let hit = FakeSession {
        incidents: 2,
        ..fake.clone()
    };
    let f = frame_of(&mut conv, &hit.at(70.0));
    assert!(f.player.as_ref().unwrap().lap.lap_invalidated);
}

fn temp_file(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir()
        .join(format!(
            "corde-iracing-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
        .join("iracing.map")
}

#[test]
fn the_source_follows_the_game() {
    let path = temp_file("source");
    let mut source = IracingSource::with_config(IracingSourceConfig {
        file: Some(path.clone()),
        stall_timeout: Duration::from_millis(80),
        connect_wait: Duration::ZERO,
    });
    assert_eq!(source.info().game, GameId::Iracing);
    // No game.
    assert!(matches!(
        source.connect(),
        Err(SourceError::NotAvailable(_))
    ));
    // The game is there without a session (what it leaves behind when it closes).
    let fake = FakeSession::default();
    FakeSession {
        connected: false,
        ..fake.clone()
    }
    .at(0.0)
    .write(&path)
    .unwrap();
    assert!(matches!(
        source.connect(),
        Err(SourceError::NotAvailable(_))
    ));
    assert!(matches!(source.poll(), Err(SourceError::NotAvailable(_))));

    // A session: one frame per row written, nothing in between.
    fake.at(1.0).write(&path).unwrap();
    source.connect().unwrap();
    let f = source.poll().unwrap().expect("a frame");
    assert_eq!(f.source, "iracing-shm");
    assert_eq!(f.session.game, GameId::Iracing);
    assert_eq!(f.player.as_ref().unwrap().vehicle_name, "BMW M4 GT3");
    assert!(source.poll().unwrap().is_none());
    fake.at(1.1).write(&path).unwrap();
    assert!(source.poll().unwrap().is_some());

    // The game rewrites its session text (here: laps of another duration).
    let longer = FakeSession {
        lap_s: 120.0,
        info_update: 2,
        ..fake.clone()
    };
    longer.at(1.2).write(&path).unwrap();
    let f = source.poll().unwrap().unwrap();
    assert_eq!(f.player.as_ref().unwrap().lap.estimated_lap_s, 120.0);

    // Nothing written for a while: the game is gone.
    std::thread::sleep(Duration::from_millis(120));
    assert!(matches!(source.poll(), Err(SourceError::NotAvailable(_))));
    assert!(
        matches!(source.poll(), Err(SourceError::NotAvailable(_))),
        "closed"
    );

    // Back; then the session ends.
    fake.at(2.0).write(&path).unwrap();
    source.connect().unwrap();
    assert!(source.poll().unwrap().is_some());
    FakeSession {
        connected: false,
        ..fake.clone()
    }
    .at(2.1)
    .write(&path)
    .unwrap();
    assert!(matches!(source.poll(), Err(SourceError::NotAvailable(_))));

    // What a crashed game leaves behind: « connected », and nothing written any more.
    fake.at(3.0).write(&path).unwrap();
    let mut careful = IracingSource::with_config(IracingSourceConfig {
        file: Some(path.clone()),
        connect_wait: Duration::from_millis(40),
        ..IracingSourceConfig::default()
    });
    assert!(matches!(
        careful.connect(),
        Err(SourceError::NotAvailable(_))
    ));
    // The same with a game that writes.
    let writing = {
        let (fake, path) = (fake.clone(), path.clone());
        std::thread::spawn(move || {
            for i in 0..40 {
                fake.at(3.0 + f64::from(i) / 60.0).write(&path).unwrap();
                std::thread::sleep(Duration::from_millis(5));
            }
        })
    };
    let mut patient = IracingSource::with_config(IracingSourceConfig {
        file: Some(path.clone()),
        connect_wait: Duration::from_millis(150),
        ..IracingSourceConfig::default()
    });
    patient.connect().unwrap();
    writing.join().unwrap();

    // A file that is not a mapping of the game: refused, not read.
    std::fs::write(&path, vec![0xFFu8; 4096]).unwrap();
    assert!(source.connect().is_err());
    std::fs::write(&path, b"short").unwrap();
    assert!(source.connect().is_err());
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn a_diagnostic_copy_names_nobody_and_still_reads() {
    let fake = FakeSession::default();
    let mut map = MemMap(fake.at(10.0).to_bytes());
    let copy = Image::of(&mut map).unwrap().for_diagnostic();
    let bytes = copy.to_bytes();
    let text: String = bytes.iter().map(|b| *b as char).collect();
    for secret in [
        "Pilote",
        "Essai",
        "Rival",
        "rapide",
        "curie",
        "Spectateur",
        "424242",
        "111111",
        "333333",
        "pluie",
    ] {
        assert!(!text.contains(secret), "{secret} is still in the copy");
    }
    // What describes the circuit, the session and the cars is kept: the copy reads like
    // the game, without the names.
    let mut map = MemMap(bytes);
    let h = reader::header(&mut map).unwrap();
    let desc = SessionDesc::parse(&reader::session_text(&mut map, &h).unwrap());
    assert_eq!(desc.track_id, "spa up");
    assert_eq!(desc.player().unwrap().car, "BMW M4 GT3");
    assert_eq!(desc.player().unwrap().name, "");
    let table = reader::var_table(&mut map, &h).unwrap();
    assert_eq!(table.len(), fake::table().len());
    let tick = reader::latest_row(&mut map).unwrap();
    let f = Converter::new().frame(&desc, table.row(&tick.row), "test");
    assert_eq!(f.session.player_name, "");
    assert_eq!(f.standings.len(), 2);
    assert!((f.player.unwrap().motion.speed_kmh - 249.48).abs() < 0.01);
}
