//! Corde finds the game by itself (ADR 0020): a server started with `--source auto` and no
//! game waits; when a (fake) Assetto Corsa Competizione session starts, it is detected,
//! shown, recorded and its laps are timed like any other; when the session is left, the
//! recording is closed and Corde looks for a game again — and finds the first Assetto Corsa
//! when that one is started instead, although it publishes pages of the same names.
//!
//! The game is played by `corde_acc::fake`, which writes the three pages as files — the
//! adapter reads them the way it reads the game's shared memory on Windows.

use corde_acc::fake::FakeSession;
use corde_acc::layout::status;
use corde_server::{Server, ServerConfig, SourceKind};
use serde_json::Value;
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::oneshot;

fn temp_dir(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "corde-games-{tag}-{}-{}",
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
async fn a_game_is_found_recorded_and_left_without_being_named() {
    let data_dir = temp_dir("data");
    let game_dir = temp_dir("acc");
    let server = Server::bind(ServerConfig {
        port: 0,
        bind: Some(IpAddr::from([127, 0, 0, 1])),
        source: SourceKind::Auto,
        // No Le Mans Ultimate here: its mirror does not exist.
        lmu_shm: Some(game_dir.join("no-lmu")),
        acc_shm: Some(game_dir.clone()),
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

    // No game: Corde waits, and says no game's name.
    tokio::time::sleep(Duration::from_millis(500)).await;
    let (code, st) = get(format!("{base}/api/status")).await;
    assert_eq!(code, 200);
    assert_eq!(st["connected"], false, "{st}");
    assert_eq!(st["game"], "unknown", "{st}");
    assert_eq!(st["source"], "auto", "{st}");

    // The game: a short lap (11 s), written 100 times a second.
    let fake = FakeSession {
        lap_time_s: 11.0,
        length_m: 5793.0,
        ..FakeSession::default()
    };
    let running = Arc::new(AtomicBool::new(true));
    let quit = Arc::new(AtomicBool::new(false));
    // The game being played (changed further down).
    let playing = Arc::new(Mutex::new(fake.clone()));
    let writer = {
        let (running, quit, dir, playing) = (
            Arc::clone(&running),
            Arc::clone(&quit),
            game_dir.clone(),
            Arc::clone(&playing),
        );
        std::thread::spawn(move || {
            let start = Instant::now();
            while !quit.load(Ordering::Relaxed) {
                let mut session = playing.lock().unwrap().clone();
                if !running.load(Ordering::Relaxed) {
                    // Back to the menus.
                    session.status = status::OFF;
                }
                session
                    .at(start.elapsed().as_secs_f64())
                    .write_dir(&dir)
                    .expect("pages");
                std::thread::sleep(Duration::from_millis(10));
            }
        })
    };

    // Detected, without anything being configured.
    let st = wait_for(
        "the game to be detected",
        Duration::from_secs(15),
        async || {
            let (_, v) = get(format!("{base}/api/status")).await;
            (v["connected"] == true).then_some(v)
        },
    )
    .await;
    assert_eq!(st["game"], "acc", "{st}");
    assert_eq!(st["source"], "acc-shm", "{st}");

    // Recorded like any session, under the game's name.
    let rec = wait_for("recording to start", Duration::from_secs(15), async || {
        let (_, v) = get(format!("{base}/api/recording")).await;
        (v["recording"] == true && v["session_id"].is_string()).then_some(v)
    })
    .await;
    let id = rec["session_id"].as_str().unwrap().to_string();
    assert!(id.contains("monza"), "{id}");
    let detail = wait_for("two timed laps", Duration::from_secs(45), async || {
        let (code, v) = get(format!("{base}/api/sessions/{id}")).await;
        assert_eq!(code, 200, "{v}");
        (v["laps"].as_array().map(|l| l.len()).unwrap_or(0) >= 2).then_some(v)
    })
    .await;
    assert_eq!(detail["game"], "acc", "{detail}");
    assert_eq!(detail["track_name"], "Monza");
    assert_eq!(detail["vehicle_model"], "bmw_m4_gt3");
    assert_eq!(detail["class_name"], "GT3");
    for lap in detail["laps"].as_array().unwrap().iter().take(2) {
        assert_eq!(lap["timed_by_game"], true, "{lap}");
        let t = lap["lap_time_s"].as_f64().unwrap();
        assert!((t - 11.0).abs() < 0.05, "{lap}");
    }

    // The session is left: the recording is closed, Corde looks for a game again.
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
    assert_eq!(sessions[0]["game"], "acc");

    // … and finds it when a session starts again.
    running.store(true, Ordering::Relaxed);
    wait_for(
        "the game to be detected again",
        Duration::from_secs(15),
        async || {
            let (_, v) = get(format!("{base}/api/status")).await;
            (v["connected"] == true && v["game"] == "acc").then_some(v)
        },
    )
    .await;

    // Another evening, another game: the first Assetto Corsa, whose pages have the same
    // names. Competizione is left first.
    running.store(false, Ordering::Relaxed);
    wait_for(
        "Competizione to be left",
        Duration::from_secs(15),
        async || {
            let (_, v) = get(format!("{base}/api/status")).await;
            (v["connected"] == false && v["recording"] == false).then_some(v)
        },
    )
    .await;
    *playing.lock().unwrap() = FakeSession {
        lap_time_s: 11.0,
        ..FakeSession::assetto_corsa()
    };
    running.store(true, Ordering::Relaxed);
    let st = wait_for(
        "Assetto Corsa to be detected",
        Duration::from_secs(15),
        async || {
            let (_, v) = get(format!("{base}/api/status")).await;
            (v["connected"] == true).then_some(v)
        },
    )
    .await;
    assert_eq!(st["game"], "ac", "{st}");
    assert_eq!(st["source"], "ac-shm", "{st}");
    let rec = wait_for(
        "its recording to start",
        Duration::from_secs(15),
        async || {
            let (_, v) = get(format!("{base}/api/recording")).await;
            (v["recording"] == true && v["session_id"].is_string()).then_some(v)
        },
    )
    .await;
    let ac_id = rec["session_id"].as_str().unwrap().to_string();
    let detail = wait_for(
        "two laps of Assetto Corsa",
        Duration::from_secs(45),
        async || {
            let (_, v) = get(format!("{base}/api/sessions/{ac_id}")).await;
            (v["laps"].as_array().map(|l| l.len()).unwrap_or(0) >= 2).then_some(v)
        },
    )
    .await;
    assert_eq!(detail["game"], "ac", "{detail}");
    assert_eq!(detail["track_name"], "Nurburgring — Layout Gp A");
    assert_eq!(detail["vehicle_model"], "ks_ferrari_488_gt3");
    // The first lap was joined on the way (the game was already lapping): only a lap seen
    // from the line to the line is checked against the game's time.
    let lap = &detail["laps"][1];
    assert_eq!(lap["timed_by_game"], true, "{lap}");
    assert!(
        (lap["lap_time_s"].as_f64().unwrap() - 11.0).abs() < 0.05,
        "{lap}"
    );

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
