//! End-to-end recording: server in-process with `--source sim` (1.5 km track,
//! 30 s laps played ×3 → 10 s per lap, 20 Hz on disk), wait for three laps, then check
//! the session through the HTTP API (list, detail, decimated samples, stop,
//! delete) and the clean close on shutdown.

use corde_server::{Server, ServerConfig, SourceKind};
use futures_util::StreamExt;
use serde_json::Value;
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tokio::sync::oneshot;
use tokio_tungstenite::tungstenite::Message;

fn temp_data_dir(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "corde-record-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

async fn http(method: &'static str, url: String) -> (u16, Value) {
    tokio::task::spawn_blocking(move || {
        let agent: ureq::Agent = ureq::config::Config::builder()
            .http_status_as_error(false)
            .proxy(None)
            .build()
            .new_agent();
        let mut resp = match method {
            "GET" => agent.get(&url).call().expect("GET"),
            "DELETE" => agent.delete(&url).call().expect("DELETE"),
            "POST" => agent.post(&url).send_empty().expect("POST"),
            _ => unreachable!(),
        };
        let status = resp.status().as_u16();
        let text = resp.body_mut().read_to_string().unwrap_or_default();
        let value = if text.is_empty() {
            Value::Null
        } else {
            serde_json::from_str(&text).unwrap_or(Value::String(text))
        };
        (status, value)
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn records_three_sim_laps_and_serves_them() {
    let data_dir = temp_data_dir("laps");
    let cfg = ServerConfig {
        port: 0,
        bind: Some(IpAddr::from([127, 0, 0, 1])),
        source: SourceKind::Sim,
        data_dir: Some(data_dir.clone()),
        sim_seed: Some(7),
        sim_ai: Some(3),
        sim_lap_time: Some(30.0),
        sim_track_length: Some(1500.0),
        sim_speed: Some(3.0),
        sim_rate: Some(100.0),
        record_rate: 20.0,
        ..Default::default()
    };
    let server = Server::bind(cfg).await.expect("bind");
    let addr: SocketAddr = server.local_addr();
    let base = format!("http://{addr}");
    let (stop, stop_rx) = oneshot::channel::<()>();
    let task = tokio::spawn(server.run(async move {
        let _ = stop_rx.await;
    }));

    // A dashboard listening for the recording events.
    let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://{addr}/ws?rate=1"))
        .await
        .expect("ws");
    let ws_events = tokio::spawn(async move {
        let mut kinds = Vec::new();
        let deadline = Instant::now() + Duration::from_secs(90);
        while Instant::now() < deadline {
            let remaining = deadline.saturating_duration_since(Instant::now());
            match tokio::time::timeout(remaining, ws.next()).await {
                Ok(Some(Ok(Message::Text(t)))) => {
                    let v: Value = serde_json::from_str(t.as_str()).unwrap();
                    if v["type"] == "event" {
                        kinds.push((v["kind"].as_str().unwrap_or("").to_string(), v.clone()));
                        if v["kind"] == "recording_stopped" {
                            break;
                        }
                    }
                }
                Ok(Some(Ok(_))) => {}
                _ => break,
            }
        }
        kinds
    });

    // Recording starts on the first frames.
    let rec = wait_for("recording to start", Duration::from_secs(15), async || {
        let (code, v) = http("GET", format!("{base}/api/recording")).await;
        assert_eq!(code, 200);
        (v["recording"] == true && v["session_id"].is_string()).then_some(v)
    })
    .await;
    let id = rec["session_id"].as_str().unwrap().to_string();
    assert!(id.ends_with("-paul-ricard-1a-race"), "{id}");
    assert_eq!(rec["record_rate_hz"], 20.0);
    assert_eq!(rec["auto"], true);

    let (code, status) = http("GET", format!("{base}/api/status")).await;
    assert_eq!(code, 200);
    assert_eq!(status["recording"], true, "{status}");

    // Deleting the live session is refused.
    let (code, _) = http("DELETE", format!("{base}/api/sessions/{id}")).await;
    assert_eq!(code, 409);

    // Three laps (10 s each in real time).
    let detail = wait_for("3 laps", Duration::from_secs(60), async || {
        let (code, v) = http("GET", format!("{base}/api/sessions/{id}")).await;
        assert_eq!(code, 200, "{v}");
        (v["laps"].as_array().map(|l| l.len()).unwrap_or(0) >= 3).then_some(v)
    })
    .await;
    assert_eq!(detail["id"], id);
    assert_eq!(detail["track_name"], "Paul Ricard - 1A");
    assert_eq!(detail["session_type"], "race");
    assert_eq!(detail["game"], "sim");
    assert_eq!(detail["vehicle_class"], "gt3");
    assert!(detail["ended_wall_ms"].is_null());
    assert_eq!(detail["events"][0]["kind"], "session_started");
    let laps = detail["laps"].as_array().unwrap();
    for lap in laps.iter().take(3) {
        assert!(lap["lap_time_s"].as_f64().unwrap() > 25.0, "{lap}");
        let start = lap["sample_row_start"].as_i64().unwrap();
        let end = lap["sample_row_end"].as_i64().unwrap();
        // 30 s at 20 Hz ≈ 600 rows for a full lap; the first lap may be partial.
        assert!(end > start, "{lap}");
        assert!(end - start <= 640, "{lap}");
        assert_eq!(lap["sample_file"], "samples-000.parquet");
    }
    let full = &laps[1];
    let rows =
        full["sample_row_end"].as_i64().unwrap() - full["sample_row_start"].as_i64().unwrap();
    assert!(
        (560..=640).contains(&rows),
        "a full lap at 20 Hz: {rows} rows"
    );
    let lap_no = full["lap_number"].as_i64().unwrap();

    // Listed, with filters.
    let (code, list) = http(
        "GET",
        format!("{base}/api/sessions?limit=5&track=ricard&type=race"),
    )
    .await;
    assert_eq!(code, 200);
    let list = list.as_array().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0]["id"], id);
    assert!(list[0]["lap_count"].as_i64().unwrap() >= 3);
    let (code, empty) = http("GET", format!("{base}/api/sessions?type=practice")).await;
    assert_eq!(code, 200);
    assert_eq!(empty.as_array().unwrap().len(), 0);
    let (code, _) = http("GET", format!("{base}/api/sessions?type=bogus")).await;
    assert_eq!(code, 400);

    // Decimated samples of a full lap.
    let (code, s) = http(
        "GET",
        format!("{base}/api/sessions/{id}/laps/{lap_no}/samples?channels=speed_kmh,throttle,brake&points=200"),
    )
    .await;
    assert_eq!(code, 200, "{s}");
    assert_eq!(s["lap_number"], lap_no);
    assert_eq!(s["total_rows"].as_i64().unwrap(), rows);
    let points = s["points"].as_u64().unwrap() as usize;
    assert!((100..=202).contains(&points), "points {points}");
    let t = s["t_s"].as_array().unwrap();
    let d = s["lap_dist_m"].as_array().unwrap();
    assert_eq!(t.len(), points);
    assert_eq!(d.len(), points);
    assert!(
        t.windows(2).all(|w| w[1].as_f64() > w[0].as_f64()),
        "t_s increasing"
    );
    let ch = s["channels"].as_array().unwrap();
    assert_eq!(ch.len(), 3);
    assert_eq!(ch[0]["name"], "speed_kmh");
    assert_eq!(ch[0]["unit"], "km/h");
    assert_eq!(ch[0]["values"].as_array().unwrap().len(), points);
    let speeds: Vec<f64> = ch[0]["values"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_f64)
        .collect();
    let vmax = speeds.iter().cloned().fold(0.0, f64::max);
    let vmin = speeds.iter().cloned().fold(f64::MAX, f64::min);
    assert!(vmax > 200.0 && vmin < 150.0, "speed range {vmin}..{vmax}");
    assert!(ch[1]["values"].as_array().unwrap().iter().all(|v| {
        let v = v.as_f64().unwrap();
        (0.0..=1.0).contains(&v)
    }));
    // Full resolution when points is large; unknown channel → 400; unknown lap → 404.
    let (code, s_full) = http(
        "GET",
        format!("{base}/api/sessions/{id}/laps/{lap_no}/samples?channels=gear,tc_active,virtual_energy&points=100000"),
    )
    .await;
    assert_eq!(code, 200);
    assert_eq!(s_full["points"].as_i64().unwrap(), rows);
    let (code, _) = http(
        "GET",
        format!("{base}/api/sessions/{id}/laps/{lap_no}/samples?channels=nope"),
    )
    .await;
    assert_eq!(code, 400);
    let (code, _) = http("GET", format!("{base}/api/sessions/{id}/laps/9999/samples")).await;
    assert_eq!(code, 404);
    let (code, _) = http("GET", format!("{base}/api/sessions/..%2Fx")).await;
    assert!(code == 400 || code == 404, "{code}");

    // Channels catalogue and storage.
    let (code, channels) = http("GET", format!("{base}/api/channels")).await;
    assert_eq!(code, 200);
    assert_eq!(
        channels.as_array().unwrap().len(),
        corde_store::CHANNELS.len()
    );
    let (code, storage) = http("GET", format!("{base}/api/storage")).await;
    assert_eq!(code, 200, "{storage}");
    assert_eq!(storage["sessions"], 1);
    assert!(
        storage["samples_bytes"].as_u64().unwrap() > 10_000,
        "{storage}"
    );
    assert!(storage["db_bytes"].as_u64().unwrap() > 0);
    assert_eq!(storage["path"], data_dir.display().to_string());

    // Manual stop → closed session, recording false, WS event.
    let (code, st) = http("POST", format!("{base}/api/recording/stop")).await;
    assert_eq!(code, 202, "{st}");
    let closed = wait_for("session to close", Duration::from_secs(10), async || {
        let (_, v) = http("GET", format!("{base}/api/sessions/{id}")).await;
        v["ended_wall_ms"].is_number().then_some(v)
    })
    .await;
    assert!(closed["sample_count"].as_i64().unwrap() >= rows);
    assert!(closed["lap_count"].as_i64().unwrap() >= 3);
    assert!(closed["best_lap_s"].as_f64().unwrap() > 25.0);
    assert_eq!(closed["files"][0]["closed"], true);
    assert!(
        closed["notes"].is_null(),
        "clean close: {}",
        closed["notes"]
    );
    let (_, rec) = http("GET", format!("{base}/api/recording")).await;
    assert_eq!(rec["recording"], false);
    assert_eq!(rec["last_stop_reason"], "stop requested");
    let (_, status) = http("GET", format!("{base}/api/status")).await;
    assert_eq!(status["recording"], false);
    // No auto restart after a manual stop (same session).
    tokio::time::sleep(Duration::from_millis(500)).await;
    let (_, rec) = http("GET", format!("{base}/api/recording")).await;
    assert_eq!(rec["recording"], false);

    let kinds = ws_events.await.unwrap();
    let names: Vec<&str> = kinds.iter().map(|(k, _)| k.as_str()).collect();
    assert!(names.contains(&"recording_started"), "{names:?}");
    let stopped = kinds
        .iter()
        .find(|(k, _)| k == "recording_stopped")
        .map(|(_, v)| v)
        .expect("recording_stopped event");
    assert_eq!(stopped["session_id"], id);
    assert!(stopped["laps"].as_i64().unwrap() >= 3);
    assert!(stopped["samples"].as_i64().unwrap() >= rows);

    // Delete.
    let (code, _) = http("DELETE", format!("{base}/api/sessions/{id}")).await;
    assert_eq!(code, 204);
    assert!(!data_dir.join("sessions").join(&id).exists());
    let (code, _) = http("DELETE", format!("{base}/api/sessions/{id}")).await;
    assert_eq!(code, 404);
    let (_, list) = http("GET", format!("{base}/api/sessions")).await;
    assert_eq!(list.as_array().unwrap().len(), 0);

    // Manual start then server shutdown: the new session is closed cleanly.
    let (code, st) = http("POST", format!("{base}/api/recording/start")).await;
    assert_eq!(code, 202, "{st}");
    let rec2 = wait_for("forced recording", Duration::from_secs(10), async || {
        let (_, v) = http("GET", format!("{base}/api/recording")).await;
        (v["recording"] == true).then_some(v)
    })
    .await;
    let id2 = rec2["session_id"].as_str().unwrap().to_string();
    tokio::time::sleep(Duration::from_secs(2)).await;
    let _ = stop.send(());
    tokio::time::timeout(Duration::from_secs(10), task)
        .await
        .expect("server stops")
        .expect("join")
        .expect("run ok");

    let store = corde_store::Store::open(&data_dir).expect("reopen store");
    let detail = store.get_session(&id2).expect("session kept");
    assert!(detail.session.ended_wall_ms.is_some(), "closed on shutdown");
    assert!(detail.session.notes.is_none(), "not a crash recovery");
    assert!(
        detail.session.sample_count > 20,
        "{}",
        detail.session.sample_count
    );
    assert!(detail.files.iter().all(|f| f.closed));
    drop(store);
    let _ = std::fs::remove_dir_all(&data_dir);
}
