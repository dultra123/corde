//! rFactor 2 is found like the other games (ADR 0020): a server started with
//! `--source auto` and no game waits; when a (fake) session of the game starts, it is
//! detected, shown, recorded under the game's name and its laps get the times of the game;
//! when the session is left, the recording is closed and Corde looks for a game again.
//!
//! The game is played by `corde_lmu::rf2::fake`, which writes the two buffers of the
//! game's plugin as files of a directory — the adapter reads them the way it reads the
//! plugin's shared memory on Windows.

use corde_lmu::rf2::fake::FakeRf2;
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
        "corde-rf2-e2e-{tag}-{}-{}",
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
async fn rfactor2_is_found_recorded_and_left() {
    let data_dir = temp_dir("data");
    let game_dir = temp_dir("game");
    let server = Server::bind(ServerConfig {
        port: 0,
        bind: Some(IpAddr::from([127, 0, 0, 1])),
        source: SourceKind::Auto,
        // No other game here: only the buffers of this one exist.
        lmu_shm: Some(game_dir.join("no-lmu")),
        acc_shm: Some(game_dir.join("no-acc")),
        iracing_shm: Some(game_dir.join("no-iracing")),
        rf2_shm: Some(game_dir.clone()),
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

    // The game: a short circuit (a lap in about 12 s), the plugin writing 60 times a
    // second. When the session is left, the plugin clears its buffers.
    let running = Arc::new(AtomicBool::new(true));
    let quit = Arc::new(AtomicBool::new(false));
    let writer = {
        let (running, quit, dir) = (Arc::clone(&running), Arc::clone(&quit), game_dir.clone());
        std::thread::spawn(move || {
            let new_game = || {
                let mut game = FakeRf2::new();
                game.world.track_length_m = 800.0;
                game
            };
            let mut game = new_game();
            let mut live = false;
            let mut last = Instant::now();
            while !quit.load(Ordering::Relaxed) {
                let now = Instant::now();
                if running.load(Ordering::Relaxed) {
                    if !live {
                        game = new_game();
                        live = true;
                    }
                    game.step(now.duration_since(last).as_secs_f64())
                        .write_dir(&dir)
                        .expect("buffers");
                } else if live {
                    game.ended().write_dir(&dir).expect("buffers");
                    live = false;
                }
                last = now;
                std::thread::sleep(Duration::from_millis(16));
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
    assert_eq!(st["game"], "rfactor2", "{st}");
    assert_eq!(st["source"], "rf2-shm", "{st}");

    // Recorded like any session, under the game's name.
    let rec = wait_for("recording to start", Duration::from_secs(15), async || {
        let (_, v) = get(format!("{base}/api/recording")).await;
        (v["recording"] == true && v["session_id"].is_string()).then_some(v)
    })
    .await;
    let id = rec["session_id"].as_str().unwrap().to_string();
    let detail = wait_for("three laps", Duration::from_secs(90), async || {
        let (code, v) = get(format!("{base}/api/sessions/{id}")).await;
        assert_eq!(code, 200, "{v}");
        (v["laps"].as_array().map(|l| l.len()).unwrap_or(0) >= 3).then_some(v)
    })
    .await;
    assert_eq!(detail["game"], "rfactor2", "{detail}");
    assert!(
        detail["track_name"].as_str().is_some_and(|t| !t.is_empty()),
        "{detail}"
    );
    assert_eq!(detail["class_name"], "Hyper", "{detail}");
    // The first lap was joined on the way: the laps seen from the line to the line carry
    // the times the game published.
    for lap in &detail["laps"].as_array().unwrap()[1..3] {
        assert_eq!(lap["timed_by_game"], true, "{lap}");
        assert_eq!(lap["valid"], true, "{lap}");
        let t = lap["lap_time_s"].as_f64().unwrap();
        assert!((10.0..14.0).contains(&t), "{lap}");
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
    assert_eq!(sessions[0]["game"], "rfactor2");

    // … and finds it when a session starts again.
    running.store(true, Ordering::Relaxed);
    wait_for(
        "the game to be detected again",
        Duration::from_secs(15),
        async || {
            let (_, v) = get(format!("{base}/api/status")).await;
            (v["connected"] == true && v["game"] == "rfactor2").then_some(v)
        },
    )
    .await;

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
