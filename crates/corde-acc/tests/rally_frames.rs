//! Assetto Corsa Rally: from its pages to what Corde does with them — each run of a stage
//! as one lap, timed by the clock of the game, through the real lap tracker.

use corde_acc::fake::FakeSession;
use corde_acc::layout::status;
use corde_acc::rally::fake::{blank_physics, FakeRallyStage};
use corde_acc::rally::{RallyConverter, RallySource, RallySourceConfig};
use corde_acc::reader::RawPages;
use corde_acc::{AccSource, AccSourceConfig, Game};
use corde_core::{LapTracker, SourceError, TelemetrySource};
use corde_protocol::{GameId, GamePhase, LapNote, LapRecord, TelemetryFrame};
use std::time::Duration;

fn frame(conv: &mut RallyConverter, raw: &RawPages, t: f64) -> TelemetryFrame {
    conv.frame(&raw.pages().unwrap(), Duration::from_secs_f64(t), "test")
}

/// Drives `seconds` at 100 Hz through the converter and the lap tracker; `pages` may
/// change what the game writes at a given time.
fn drive_with(
    fake: &FakeRallyStage,
    from: f64,
    seconds: f64,
    pages: impl Fn(f64, RawPages) -> RawPages,
) -> (Vec<LapRecord>, Vec<TelemetryFrame>) {
    let mut conv = RallyConverter::new();
    let mut tracker = LapTracker::new();
    let mut laps = Vec::new();
    let mut frames = Vec::new();
    for i in 0..=(seconds * 100.0) as u64 {
        let t = from + i as f64 / 100.0;
        let f = frame(&mut conv, &pages(t, fake.at(t)), t);
        laps.extend(tracker.feed(&f));
        frames.push(f);
    }
    (laps, frames)
}

fn drive(fake: &FakeRallyStage, seconds: f64) -> (Vec<LapRecord>, Vec<TelemetryFrame>) {
    drive_with(fake, 0.0, seconds, |_, raw| raw)
}

fn temp_dir(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "corde-acrally-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

#[test]
fn a_frame_carries_what_the_game_publishes_in_celsius() {
    let fake = FakeRallyStage::default();
    let (_, frames) = drive(&fake, 6.0 + 23.75);
    // On the start line: no run yet, the car « in the pits ».
    let line = frames[400].player.as_ref().unwrap();
    assert_eq!(line.lap.lap_number, -1);
    assert!(line.in_pits);
    assert_eq!(line.lap.current_lap_s, 0.0);
    assert_eq!(line.lap.lap_fraction, 0.0);

    // A quarter of the stage.
    let f = frames.last().unwrap();
    let s = &f.session;
    assert_eq!(s.game, GameId::AcRally);
    assert_eq!(s.track_name, "Alsace Stage 02");
    assert_eq!(s.track_length_m, 2850.0);
    assert_eq!(s.phase, GamePhase::Green);
    assert!(s.in_realtime);
    assert_eq!(s.num_vehicles, 1);
    assert!((s.weather.ambient_c - 22.0).abs() < 1e-3, "kelvins → °C");
    assert!((s.weather.track_c - 27.0).abs() < 1e-3);
    let p = f.player.as_ref().unwrap();
    assert_eq!(p.vehicle_model, "Alpine A110 Rally");
    assert_eq!(p.lap.lap_number, 0);
    assert_eq!(p.lap.laps_completed, 0);
    assert!(!p.in_pits);
    assert!((p.lap.current_lap_s - 23.75).abs() < 0.02);
    assert!((p.lap.lap_fraction - 0.25).abs() < 1e-3);
    assert!((p.lap.lap_dist_m - 712.5).abs() < 1.0);
    // The session clock started again when the name of the stage arrived (2 s in).
    assert!((p.lap.lap_start_s - 4.0).abs() < 0.05);
    assert!((s.elapsed_s - 27.75).abs() < 0.02);
    assert_eq!(p.lap.last_lap_s, None);
    assert_eq!(p.engine.gear, 3);
    assert!((p.engine.water_c - 70.45).abs() < 1e-3);
    assert!((p.wheels[1].carcass_c - 91.0).abs() < 1e-3);
    assert!((p.wheels[3].brake_temp_c - 78.85).abs() < 1e-3);
    assert_eq!(p.place, 1);
    assert_eq!(p.gaps.car_ahead_s, None);
    assert_eq!(f.standings.len(), 1);
    assert_eq!(f.standings[0].lap_fraction, p.lap.lap_fraction);
}

#[test]
fn each_run_is_a_lap_with_the_time_of_the_game() {
    let fake = FakeRallyStage::default();
    let (laps, frames) = drive(&fake, 2.0 * fake.cycle_s() + 30.0);
    assert_eq!(laps.len(), 2, "{laps:#?}");
    for lap in &laps {
        assert!(lap.timed_by_game && lap.valid, "{lap:#?}");
        assert_eq!(lap.lap_time_s, 95.0, "the clock where it froze");
        assert!(!lap.in_lap && !lap.out_lap, "{lap:#?}");
        assert_eq!(lap.note, None);
        assert!((lap.top_speed_kmh - 108.0).abs() < 0.5);
    }
    assert_eq!(
        laps.iter().map(|l| l.lap_number).collect::<Vec<_>>(),
        [0, 1]
    );
    // In the stop zone and back on the line: the last time stays, nothing is running.
    let at = |t: f64| frames[(t * 100.0).round() as usize].player.clone().unwrap();
    let stopped = at(6.0 + 95.0 + 6.0);
    assert!(stopped.in_pits);
    assert_eq!(stopped.lap.laps_completed, 1);
    assert_eq!(stopped.lap.last_lap_s, Some(95.0));
    assert_eq!(stopped.lap.best_lap_s, Some(95.0));
    assert_eq!(stopped.lap.current_lap_s, 0.0);
    // The second run.
    let second = at(fake.cycle_s() + 6.0 + 40.0);
    assert_eq!(second.lap.lap_number, 1);
    assert!((second.lap.current_lap_s - 40.0).abs() < 0.02);
    assert!(!second.in_pits);
}

#[test]
fn a_stage_whose_length_the_game_does_not_give_is_measured() {
    let fake = FakeRallyStage {
        publishes_length: false,
        ..FakeRallyStage::default()
    };
    let (laps, frames) = drive(&fake, 2.0 * fake.cycle_s() + 30.0);
    assert_eq!(laps.len(), 2, "{laps:#?}");
    assert!(laps.iter().all(|l| l.timed_by_game && l.valid), "{laps:#?}");
    // Nothing known on the start line; « half-way » while the first run goes on.
    let at = |t: f64| &frames[(t * 100.0).round() as usize];
    assert_eq!(at(3.0).session.track_length_m, 0.0);
    let first = at(6.0 + 50.0);
    assert_eq!(first.player.as_ref().unwrap().lap.lap_fraction, 0.5);
    assert!((first.session.track_length_m - 2.0 * 1500.0).abs() < 2.0);
    // The second run knows the length the first one measured.
    let second = at(fake.cycle_s() + 6.0 + 47.5);
    assert!((second.session.track_length_m - 2850.0).abs() < 2.0);
    assert!((second.player.as_ref().unwrap().lap.lap_fraction - 0.5).abs() < 0.01);
}

#[test]
fn a_run_given_up_is_a_lap_without_a_time() {
    let fake = FakeRallyStage {
        gives_up_after_s: Some(40.0),
        ..FakeRallyStage::default()
    };
    let (laps, frames) = drive(&fake, 6.0 + 40.0 + fake.cycle_s() + 30.0);
    assert_eq!(laps.len(), 2, "{laps:#?}");
    let (gave_up, finished) = (&laps[0], &laps[1]);
    assert_eq!(gave_up.note, Some(LapNote::Aborted));
    assert!(!gave_up.valid && !gave_up.timed_by_game);
    assert!((gave_up.lap_time_s - 40.0).abs() < 0.1, "{gave_up:#?}");
    assert!(finished.valid && finished.timed_by_game);
    assert_eq!(finished.lap_time_s, 95.0);
    // The run given up never counted.
    let last = frames.last().unwrap().player.as_ref().unwrap();
    assert_eq!(last.lap.laps_completed, 1);
    assert_eq!(last.lap.best_lap_s, Some(95.0));
}

#[test]
fn a_pause_is_not_a_finish_and_a_replay_is_not_a_run() {
    let fake = FakeRallyStage::default();
    // A pause from 30 s to 50 s of the game: its pages are frozen (the same as at 30 s)
    // and the physics page is blank; the clock of the game then goes on from there.
    let (laps, frames) = drive_with(&fake, 0.0, fake.cycle_s() + 20.0 + 20.0, |t, raw| {
        if (30.0..50.0).contains(&t) {
            let mut frozen = blank_physics(fake.at(30.0));
            // The counter of the physics page keeps running.
            frozen.physics[..4].copy_from_slice(&raw.physics[..4]);
            frozen.graphics[4..8].copy_from_slice(&status::PAUSE.to_le_bytes());
            frozen
        } else if t >= 50.0 {
            fake.at(t - 20.0)
        } else {
            raw
        }
    });
    let paused = &frames[4000];
    assert_eq!(paused.session.phase, GamePhase::Paused);
    assert_eq!(paused.player.as_ref().unwrap().lap.laps_completed, 0);
    assert_eq!(laps.len(), 1, "{laps:#?}");
    assert!(laps[0].timed_by_game && laps[0].valid, "{laps:#?}");
    assert_eq!(laps[0].lap_time_s, 95.0);

    // A replay: the clock runs from zero and the car moves, on a blank physics page.
    let (laps, frames) = drive_with(&fake, 0.0, fake.cycle_s() + 20.0, |_, raw| {
        let mut replay = blank_physics(raw);
        replay.graphics[4..8].copy_from_slice(&status::REPLAY.to_le_bytes());
        replay
    });
    assert!(laps.is_empty(), "{laps:#?}");
    let p = frames.last().unwrap().player.as_ref().unwrap();
    assert_eq!((p.lap.lap_number, p.lap.laps_completed), (-1, 0));
    assert!(p.in_pits);
}

#[test]
fn the_name_of_the_stage_arriving_during_a_run_does_not_lose_the_run() {
    // The static page is only written 30 s after the start of the first run.
    let fake = FakeRallyStage {
        static_after_s: 36.0,
        ..FakeRallyStage::default()
    };
    let (laps, frames) = drive(&fake, fake.cycle_s() + 20.0);
    assert_eq!(frames[3000].session.track_name, "");
    assert_eq!(frames[4000].session.track_name, "Alsace Stage 02");
    assert_eq!(laps.len(), 1, "{laps:#?}");
    assert!(laps[0].timed_by_game && laps[0].valid, "{laps:#?}");
    assert_eq!(laps[0].lap_time_s, 95.0);
}

#[test]
fn a_stage_joined_on_the_way_is_not_timed_the_next_one_is() {
    let fake = FakeRallyStage::default();
    // Corde starts 40 s into the first run: it never saw the start line.
    let (laps, frames) = drive_with(&fake, 46.0, 2.0 * fake.cycle_s(), |_, raw| raw);
    assert_eq!(laps.len(), 1, "{laps:#?}");
    assert!(laps[0].timed_by_game && laps[0].valid);
    assert_eq!(laps[0].lap_time_s, 95.0);
    let first = frames[100].player.as_ref().unwrap();
    assert!(first.in_pits, "not in a run Corde saw start");
    assert_eq!(first.lap.lap_number, -1);
}

#[test]
fn the_source_tells_this_game_from_the_two_others() {
    let dir = temp_dir("source");
    let mut source = RallySource::with_config(RallySourceConfig {
        dir: Some(dir.clone()),
        stall_timeout: Duration::from_millis(300),
    });
    assert_eq!(source.info().game, GameId::AcRally);
    assert_eq!(source.info().label, "Assetto Corsa Rally");
    let others = |dir: &std::path::Path| {
        [Game::Competizione, Game::AssettoCorsa].map(|game| {
            AccSource::with_config(AccSourceConfig {
                game,
                dir: Some(dir.to_path_buf()),
                ..AccSourceConfig::default()
            })
        })
    };
    // The game is not there.
    assert!(matches!(
        source.connect(),
        Err(SourceError::NotAvailable(_))
    ));

    // In the menus: the graphics page is all zeros.
    let mut fake = FakeRallyStage {
        status: status::OFF,
        ..FakeRallyStage::default()
    };
    fake.at(0.0).write_dir(&dir).unwrap();
    assert!(matches!(
        source.connect(),
        Err(SourceError::NotAvailable(_))
    ));

    // A stage, its static page still empty: this game's, and nobody else's.
    fake.status = status::LIVE;
    fake.at(1.0).write_dir(&dir).unwrap();
    for mut other in others(&dir) {
        assert!(matches!(other.connect(), Err(SourceError::NotAvailable(_))));
    }
    source.connect().unwrap();
    let first = source.poll().unwrap().expect("a frame");
    assert_eq!(first.session.game, GameId::AcRally);
    assert_eq!(first.source, "acrally-shm");
    assert!(source.poll().unwrap().is_none(), "nothing moved");
    // The static page written: still not the others' (their version number is there now).
    std::thread::sleep(Duration::from_millis(20));
    fake.at(3.0).write_dir(&dir).unwrap();
    assert!(source.poll().unwrap().is_some());
    for mut other in others(&dir) {
        assert!(matches!(other.connect(), Err(SourceError::NotAvailable(_))));
    }
    // A blank physics page with that static page: nobody connects on it.
    blank_physics(fake.at(3.0)).write_dir(&dir).unwrap();
    for mut other in others(&dir) {
        assert!(matches!(other.connect(), Err(SourceError::NotAvailable(_))));
    }
    let mut again = RallySource::with_config(RallySourceConfig {
        dir: Some(dir.clone()),
        ..RallySourceConfig::default()
    });
    assert!(matches!(again.connect(), Err(SourceError::NotAvailable(_))));

    // Nothing moves any more: the game is gone.
    std::thread::sleep(Duration::from_millis(350));
    assert!(matches!(source.poll(), Err(SourceError::NotAvailable(_))));
    // Back in the menus after a stage.
    fake.at(4.0).write_dir(&dir).unwrap();
    source.connect().unwrap();
    fake.status = status::OFF;
    fake.at(5.0).write_dir(&dir).unwrap();
    assert!(matches!(source.poll(), Err(SourceError::NotAvailable(_))));

    // The pages of Competizione, of the first Assetto Corsa: not this game's.
    for theirs in [FakeSession::default(), FakeSession::assetto_corsa()] {
        theirs.at(1.0).write_dir(&dir).unwrap();
        assert!(matches!(
            source.connect(),
            Err(SourceError::NotAvailable(_))
        ));
    }
    // … and another game taking the names while connected ends the session.
    FakeRallyStage::default().at(1.0).write_dir(&dir).unwrap();
    source.connect().unwrap();
    FakeSession::default().at(1.0).write_dir(&dir).unwrap();
    assert!(matches!(source.poll(), Err(SourceError::NotAvailable(_))));
    let _ = std::fs::remove_dir_all(&dir);
}
