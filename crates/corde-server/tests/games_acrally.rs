//! Assetto Corsa Rally is found like the other games (ADR 0020): a server started with
//! `--source auto` and no game waits; when a (fake) stage is loaded, the game is detected
//! — its pages have the names of Competizione's, its temperatures tell it apart —, shown,
//! recorded under its name, and each run of the stage is a lap with the time of the game;
//! back in the menus, the recording is closed and Corde looks for a game again.
//!
//! The game is played by `corde_acc::rally::fake`, which writes its three pages as files
//! of a directory — the adapter reads them the way it reads the game's shared memory on
//! Windows.

use corde_acc::layout::status;
use corde_acc::rally::fake::FakeRallyStage;
use corde_server::{Server, ServerConfig, SourceKind};
use serde_json::Value;
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::oneshot;

fn temp_dir(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "corde-acrally-e2e-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

async fn get(url: String) -> (u16, Value) {
    tokio::task::spawn_blocking(move || {
        let agent: ureq::Agent = ureq::config::Config::builder()
            .http_status_as_error(false)
            .proxy(None)
            .build()
            .new_agent();
        let mut resp = agent.get(&url).call().expect("GET");
        let status = resp.status().as_u16();
        let text = resp.body_mut().read_to_string().unwrap_or_default();
        (status, serde_json::from_str(&text).unwrap_or(Value::Null))
    })
    .await
    .unwrap()
}

async fn wait_for(
    what: &str,
    timeout: Duration,
    mut check: impl AsyncFnMut() -> Option<Value>,
) -> Value {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(v) = check().await {
            return v;
        }
        assert!(Instant::now() < deadline, "timeout waiting for {what}");
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn assetto_corsa_rally_is_found_and_its_runs_are_laps() {
    let data_dir = temp_dir("data");
    let game_dir = temp_dir("game");
    let server = Server::bind(ServerConfig {
        port: 0,
        bind: Some(IpAddr::from([127, 0, 0, 1])),
        source: SourceKind::Auto,
        // No other game here: the directory only ever holds the pages of this one.
        lmu_shm: Some(game_dir.join("no-lmu")),
        acc_shm: Some(game_dir.clone()),
        iracing_shm: Some(game_dir.join("no-iracing")),
        data_dir: Some(data_dir.clone()),
        record_rate: 20.0,
        no_import: true,
        no_game_api: true,
        sync_interval_s: 0,
        ..Default::default()
    })
    .await
    .expect("bind");
    let addr: SocketAddr = server.local_addr();
    let base = format!("http://{addr}");
    let (stop, stop_rx) = oneshot::channel::<()>();
    let task = tokio::spawn(server.run(async move {
        let _ = stop_rx.await;
    }));

    // No game: Corde waits.
    tokio::time::sleep(Duration::from_millis(500)).await;
    let (_, st) = get(format!("{base}/api/status")).await;
    assert_eq!(st["connected"], false, "{st}");
    assert_eq!(st["game"], "unknown", "{st}");

    // The game: a short stage (12 s), 3 s on the start line, 5 s in the stop zone, then
    // the stage is restarted; its static page comes a second after the stage loaded.
    let fake = FakeRallyStage {
        wait_s: 3.0,
        run_s: 12.0,
        roll_s: 2.0,
        rest_s: 3.0,
        length_m: 400.0,
        static_after_s: 1.0,
        ..FakeRallyStage::default()
    };
    let running = Arc::new(AtomicBool::new(true));
    let quit = Arc::new(AtomicBool::new(false));
    let writer = {
        let (running, quit, dir, fake) = (
            Arc::clone(&running),
            Arc::clone(&quit),
            game_dir.clone(),
            fake.clone(),
        );
        std::thread::spawn(move || {
            let start = Instant::now();
            while !quit.load(Ordering::Relaxed) {
                let stage = FakeRallyStage {
                    // Back in the menus once the stage is left.
                    status: if running.load(Ordering::Relaxed) {
                        status::LIVE
                    } else {
                        status::OFF
                    },
                    ..fake.clone()
                };
                stage
                    .at(start.elapsed().as_secs_f64())
                    .write_dir(&dir)
                    .expect("pages");
                std::thread::sleep(Duration::from_millis(16));
            }
        })
    };

    // Detected, without anything being configured — and not taken for Competizione.
    let st = wait_for(
        "the game to be detected",
        Duration::from_secs(15),
        async || {
            let (_, v) = get(format!("{base}/api/status")).await;
            (v["connected"] == true).then_some(v)
        },
    )
    .await;
    assert_eq!(st["game"], "ac_rally", "{st}");
    assert_eq!(st["source"], "acrally-shm", "{st}");

    // Recorded like any session, under the game's name; two runs, two laps.
    let rec = wait_for("recording to start", Duration::from_secs(20), async || {
        let (_, v) = get(format!("{base}/api/recording")).await;
        (v["recording"] == true && v["session_id"].is_string()).then_some(v)
    })
    .await;
    let id = rec["session_id"].as_str().unwrap().to_string();
    assert!(id.contains("alsace"), "{id}");
    let detail = wait_for("two runs", Duration::from_secs(90), async || {
        let (code, v) = get(format!("{base}/api/sessions/{id}")).await;
        assert_eq!(code, 200, "{v}");
        (v["laps"].as_array().map(|l| l.len()).unwrap_or(0) >= 2).then_some(v)
    })
    .await;
    assert_eq!(detail["game"], "ac_rally", "{detail}");
    assert_eq!(detail["track_name"], "Alsace Stage 02");
    assert_eq!(detail["vehicle_model"], "Alpine A110 Rally");
    let laps = detail["laps"].as_array().unwrap();
    // Every lap of the session is a run that finished, with the clock of the game: the
    // waits on the start line and in the stop zone are not laps.
    for lap in laps {
        assert_eq!(lap["timed_by_game"], true, "{lap}");
        assert_eq!(lap["valid"], true, "{lap}");
        assert_eq!(lap["lap_time_s"].as_f64().unwrap(), 12.0, "{lap}");
    }

    // Back in the menus: the recording is closed, Corde looks for a game again.
    running.store(false, Ordering::Relaxed);
    let st = wait_for("the game to be left", Duration::from_secs(15), async || {
        let (_, v) = get(format!("{base}/api/status")).await;
        (v["connected"] == false && v["recording"] == false).then_some(v)
    })
    .await;
    assert_eq!(st["in_realtime"], false, "{st}");
    let (_, list) = get(format!("{base}/api/sessions")).await;
    let sessions = list["sessions"]
        .as_array()
        .or(list.as_array())
        .unwrap()
        .clone();
    assert_eq!(sessions.len(), 1, "{list}");
    assert_eq!(sessions[0]["game"], "ac_rally");

    quit.store(true, Ordering::Relaxed);
    writer.join().unwrap();
    let _ = stop.send(());
    tokio::time::timeout(Duration::from_secs(15), task)
        .await
        .expect("server stops")
        .expect("join")
        .expect("run ok");
    let _ = std::fs::remove_dir_all(&data_dir);
    let _ = std::fs::remove_dir_all(&game_dir);
}
