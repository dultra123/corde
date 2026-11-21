//! Assetto Corsa EVO is found like the other games (ADR 0020): a server started with
//! `--source auto` and no game waits; when a (fake) session of the game starts, it is
//! detected, shown, recorded under the game's name and its laps get the times of the game;
//! when the session is left, the recording is closed and Corde looks for a game again.
//!
//! The game is played by `corde_acc::evo::fake`, which writes its three pages as files of
//! a directory — the adapter reads them the way it reads the game's shared memory on
//! Windows.

use corde_acc::evo::fake::FakeEvoSession;
use corde_acc::evo::layout::status;
use corde_acc::reader::EVO_PAGES;
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
        "corde-acevo-e2e-{tag}-{}-{}",
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
async fn assetto_corsa_evo_is_found_recorded_and_left() {
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

    // The game: a short lap (11 s), its pages rewritten 60 times a second, times that
    // differ a little from lap to lap.
    let fake = FakeEvoSession {
        lap_time_s: 11.0,
        length_m: 800.0,
        time_step_ms: 3,
        ..FakeEvoSession::default()
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
                let session = FakeEvoSession {
                    // Back at the menus once the session is left.
                    status: if running.load(Ordering::Relaxed) {
                        status::LIVE
                    } else {
                        status::OFF
                    },
                    ..fake.clone()
                };
                session
                    .at(start.elapsed().as_secs_f64())
                    .write_dir_of(&dir, &EVO_PAGES)
                    .expect("pages");
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
    assert_eq!(st["game"], "ac_evo", "{st}");
    assert_eq!(st["source"], "acevo-shm", "{st}");

    // Recorded like any session, under the game's name.
    let rec = wait_for("recording to start", Duration::from_secs(15), async || {
        let (_, v) = get(format!("{base}/api/recording")).await;
        (v["recording"] == true && v["session_id"].is_string()).then_some(v)
    })
    .await;
    let id = rec["session_id"].as_str().unwrap().to_string();
    assert!(id.contains("brands"), "{id}");
    let detail = wait_for("three laps", Duration::from_secs(60), async || {
        let (code, v) = get(format!("{base}/api/sessions/{id}")).await;
        assert_eq!(code, 200, "{v}");
        (v["laps"].as_array().map(|l| l.len()).unwrap_or(0) >= 3).then_some(v)
    })
    .await;
    assert_eq!(detail["game"], "ac_evo", "{detail}");
    assert_eq!(detail["track_name"], "Brands Hatch — GP");
    assert_eq!(detail["vehicle_model"], "BMW M4 GT3");
    assert_eq!(detail["class_name"], "GT3");
    // The first lap was joined on the way: the laps seen from the line to the line carry
    // the times the game published.
    for lap in &detail["laps"].as_array().unwrap()[1..3] {
        assert_eq!(lap["timed_by_game"], true, "{lap}");
        assert_eq!(lap["valid"], true, "{lap}");
        let t = lap["lap_time_s"].as_f64().unwrap();
        assert!((t - 11.0).abs() < 0.02, "{lap}");
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
    assert_eq!(sessions[0]["game"], "ac_evo");

    // … and finds it when a session starts again.
    running.store(true, Ordering::Relaxed);
    wait_for(
        "the game to be detected again",
        Duration::from_secs(15),
        async || {
            let (_, v) = get(format!("{base}/api/status")).await;
            (v["connected"] == true && v["game"] == "ac_evo").then_some(v)
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
