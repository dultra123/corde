//! End-to-end: start the server in-process with `--source sim` on a free port,
//! talk to it over a real WebSocket (tokio-tungstenite) and over HTTP (ureq).

use corde_server::{Server, ServerConfig, SourceKind};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tokio::sync::oneshot;
use tokio_tungstenite::tungstenite::Message;

struct TestServer {
    addr: SocketAddr,
    data_dir: PathBuf,
    stop: Option<oneshot::Sender<()>>,
    task: tokio::task::JoinHandle<anyhow::Result<()>>,
}

impl TestServer {
    async fn start() -> Self {
        let data_dir = std::env::temp_dir().join(format!(
            "corde-server-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let cfg = ServerConfig {
            port: 0,
            bind: Some(IpAddr::from([127, 0, 0, 1])),
            source: SourceKind::Sim,
            data_dir: Some(data_dir.clone()),
            rate: 20,
            sim_seed: Some(1234),
            sim_rate: Some(100.0),
            ..Default::default()
        };
        let server = Server::bind(cfg).await.expect("bind on a free port");
        assert_eq!(server.source_name(), "sim");
        assert!(server.banner().iter().any(|l| l.contains("127.0.0.1")));
        let addr = server.local_addr();
        assert_ne!(addr.port(), 0);
        let (stop, stop_rx) = oneshot::channel::<()>();
        let task = tokio::spawn(server.run(async move {
            let _ = stop_rx.await;
        }));
        Self {
            addr,
            data_dir,
            stop: Some(stop),
            task,
        }
    }

    fn http(&self, path: &str) -> String {
        format!("http://{}{}", self.addr, path)
    }

    fn ws_url(&self, query: &str) -> String {
        format!("ws://{}/ws{}", self.addr, query)
    }

    async fn shutdown(mut self) {
        let _ = self.stop.take().unwrap().send(());
        let r = tokio::time::timeout(Duration::from_secs(5), self.task)
            .await
            .expect("server stops within 5 s")
            .expect("join");
        r.expect("server run ok");
        let _ = std::fs::remove_dir_all(&self.data_dir);
    }
}

/// Blocking ureq calls run on the blocking pool. Returns `(status, body)`.
async fn http_call(method: &'static str, url: String, body: Option<Value>) -> (u16, Value) {
    tokio::task::spawn_blocking(move || {
        let agent: ureq::Agent = ureq::config::Config::builder()
            .http_status_as_error(false)
            .proxy(None)
            .build()
            .new_agent();
        let mut resp = match (method, body) {
            ("GET", _) => agent.get(&url).call().expect("GET"),
            ("DELETE", _) => agent.delete(&url).call().expect("DELETE"),
            ("PUT", Some(b)) => agent.put(&url).send_json(b).expect("PUT"),
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

async fn next_json(
    ws: &mut (impl StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin),
) -> Value {
    loop {
        let msg = tokio::time::timeout(Duration::from_secs(5), ws.next())
            .await
            .expect("message within 5 s")
            .expect("stream open")
            .expect("ws ok");
        match msg {
            Message::Text(t) => return serde_json::from_str(t.as_str()).expect("valid JSON"),
            Message::Ping(_) | Message::Pong(_) => continue,
            other => panic!("unexpected message {other:?}"),
        }
    }
}

/// Read messages until `n` frames were seen (other message types are returned too).
async fn collect_frames(
    ws: &mut (impl StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin),
    n: usize,
) -> Vec<Value> {
    let mut frames = Vec::new();
    while frames.len() < n {
        let v = next_json(ws).await;
        if v["type"] == "frame" {
            frames.push(v);
        }
    }
    frames
}

/// Count frames received during `window`.
async fn count_frames_for(
    ws: &mut (impl StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin),
    window: Duration,
) -> usize {
    let deadline = Instant::now() + window;
    let mut n = 0;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        match tokio::time::timeout(remaining, ws.next()).await {
            Ok(Some(Ok(Message::Text(t)))) => {
                let v: Value = serde_json::from_str(t.as_str()).unwrap();
                if v["type"] == "frame" {
                    n += 1;
                }
            }
            Ok(Some(Ok(_))) => {}
            Ok(Some(Err(e))) => panic!("ws error {e}"),
            Ok(None) => panic!("ws closed"),
            Err(_) => break,
        }
    }
    n
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn websocket_hello_frames_rate_ping_and_http_api() {
    let server = TestServer::start().await;

    // --- WebSocket --------------------------------------------------------
    let (mut ws, _resp) = tokio_tungstenite::connect_async(server.ws_url(""))
        .await
        .expect("ws connect");
    let hello = next_json(&mut ws).await;
    assert_eq!(hello["type"], "hello");
    assert_eq!(hello["protocol_version"], corde_protocol::PROTOCOL_VERSION);
    assert_eq!(hello["server_version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(hello["status"]["source"], "sim");
    assert_eq!(hello["status"]["output_rate_hz"], 20.0);
    assert_eq!(hello["status"]["clients"], 1);

    let frames = collect_frames(&mut ws, 6).await;
    let seqs: Vec<u64> = frames.iter().map(|f| f["seq"].as_u64().unwrap()).collect();
    assert!(
        seqs.windows(2).all(|w| w[1] > w[0]),
        "seq must be strictly increasing: {seqs:?}"
    );
    for f in &frames {
        assert_eq!(f["v"], corde_protocol::PROTOCOL_VERSION);
        assert_eq!(f["source"], "sim");
        assert!(f["player"].is_object(), "player present");
        assert_eq!(f["session"]["track_name"], "Paul Ricard - 1A");
        assert_eq!(f["standings"].as_array().unwrap().len(), 20);
        let p = &f["player"];
        assert!(p["motion"]["speed_kmh"].as_f64().unwrap() > 70.0);
        assert!(p["engine"]["rpm"].as_f64().unwrap() <= 8500.0);
    }
    // Round-trips through the real protocol types.
    let typed: corde_protocol::ServerMessage =
        serde_json::from_value(frames[0].clone()).expect("frame deserialises as ServerMessage");
    assert!(matches!(typed, corde_protocol::ServerMessage::Frame(_)));

    // Rate change: 50 Hz then 5 Hz (the server answers with a Status carrying the new rate).
    ws.send(Message::text(
        json!({ "type": "set_rate", "hz": 50 }).to_string(),
    ))
    .await
    .unwrap();
    let mut got_status = false;
    for _ in 0..20 {
        let v = next_json(&mut ws).await;
        if v["type"] == "status" && v["output_rate_hz"] == 50.0 {
            got_status = true;
            break;
        }
    }
    assert!(
        got_status,
        "status with output_rate_hz=50 expected after set_rate"
    );
    let fast = count_frames_for(&mut ws, Duration::from_millis(1000)).await;
    assert!(fast >= 30, "≈50 frames/s expected, got {fast}");

    ws.send(Message::text(
        json!({ "type": "set_rate", "hz": 5 }).to_string(),
    ))
    .await
    .unwrap();
    // Drain for a moment so the new ticker is in effect, then count.
    let _ = count_frames_for(&mut ws, Duration::from_millis(300)).await;
    let slow = count_frames_for(&mut ws, Duration::from_millis(1000)).await;
    assert!(slow <= 8, "≈5 frames/s expected, got {slow}");

    // Out-of-range rates are clamped (1..=60).
    ws.send(Message::text(
        json!({ "type": "set_rate", "hz": 200 }).to_string(),
    ))
    .await
    .unwrap();
    let mut clamped = false;
    for _ in 0..20 {
        let v = next_json(&mut ws).await;
        if v["type"] == "status" {
            assert_eq!(v["output_rate_hz"], 60.0);
            clamped = true;
            break;
        }
    }
    assert!(clamped);

    // Ping → Pong with the same t.
    ws.send(Message::text(
        json!({ "type": "ping", "t": 4242 }).to_string(),
    ))
    .await
    .unwrap();
    let mut pong = false;
    for _ in 0..50 {
        let v = next_json(&mut ws).await;
        if v["type"] == "pong" {
            assert_eq!(v["t"], 4242);
            pong = true;
            break;
        }
    }
    assert!(pong, "pong expected");

    // Unknown client messages are ignored (connection stays open).
    ws.send(Message::text("{\"type\":\"whatever\"}"))
        .await
        .unwrap();
    ws.send(Message::text("not json")).await.unwrap();
    let _ = collect_frames(&mut ws, 2).await;

    // --- HTTP -------------------------------------------------------------
    let (code, health) = http_call("GET", server.http("/api/health"), None).await;
    assert_eq!(code, 200);
    assert_eq!(health["ok"], true);

    let (code, status) = http_call("GET", server.http("/api/status"), None).await;
    assert_eq!(code, 200);
    assert_eq!(status["connected"], true);
    assert_eq!(status["source"], "sim");
    assert_eq!(status["game"], "sim");
    assert_eq!(status["clients"], 1);
    assert_eq!(status["in_realtime"], true);
    let typed: corde_protocol::SourceStatus = serde_json::from_value(status.clone()).unwrap();
    assert!(
        typed.input_rate_hz > 50.0,
        "input rate measured ≈100 Hz, got {}",
        typed.input_rate_hz
    );

    let (code, frame) = http_call("GET", server.http("/api/frame"), None).await;
    assert_eq!(code, 200);
    assert!(frame["seq"].as_u64().unwrap() > 0);
    let _typed: corde_protocol::TelemetryFrame = serde_json::from_value(frame).unwrap();

    let (code, laps) = http_call("GET", server.http("/api/laps"), None).await;
    assert_eq!(code, 200);
    assert!(laps.is_array());

    // Layouts CRUD.
    let (code, list) = http_call("GET", server.http("/api/layouts"), None).await;
    assert_eq!(code, 200);
    assert_eq!(list, json!([]));

    let desk = json!({ "id": "ignored", "name": "Bureau", "device": "desktop", "columns": 12,
        "widgets": [{ "id": "w1", "kind": "tyres", "x": 0, "y": 0, "w": 4, "h": 3, "config": {} }], "updated_ms": 10 });
    let (code, stored) = http_call("PUT", server.http("/api/layouts/desk"), Some(desk)).await;
    assert_eq!(code, 200, "{stored}");
    assert_eq!(stored["id"], "desk");
    assert!(
        server.data_dir.join("layouts").join("desk.json").is_file(),
        "persisted on disk"
    );

    let (code, one) = http_call("GET", server.http("/api/layouts/desk"), None).await;
    assert_eq!(code, 200);
    assert_eq!(one["name"], "Bureau");
    let _typed: corde_protocol::DashboardLayout = serde_json::from_value(one).unwrap();

    let (code, _) = http_call("GET", server.http("/api/layouts/nope"), None).await;
    assert_eq!(code, 404);
    let (code, _) = http_call("PUT", server.http("/api/layouts/..%2Fevil"), Some(json!({ "id": "x", "name": "x", "device": "mobile", "columns": 4, "widgets": [], "updated_ms": 1 }))).await;
    assert!(code == 400 || code == 404, "bad id rejected, got {code}");

    // Full replacement through PUT /api/layouts (what the UI does).
    let all = json!([
        { "id": "phone", "name": "Téléphone", "device": "mobile", "columns": 4, "widgets": [], "updated_ms": 20 },
        { "id": "obs", "name": "Overlay", "device": "overlay", "columns": 12, "widgets": [], "updated_ms": 30 }
    ]);
    let (code, stored) = http_call("PUT", server.http("/api/layouts"), Some(all)).await;
    assert_eq!(code, 200);
    assert_eq!(stored.as_array().unwrap().len(), 2);
    assert!(
        !server.data_dir.join("layouts").join("desk.json").exists(),
        "desk removed by full replacement"
    );
    let (code, list) = http_call("GET", server.http("/api/layouts"), None).await;
    assert_eq!(code, 200);
    let ids: Vec<&str> = list
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["obs", "phone"]);

    let (code, _) = http_call("DELETE", server.http("/api/layouts/phone"), None).await;
    assert_eq!(code, 204);
    let (code, _) = http_call("DELETE", server.http("/api/layouts/phone"), None).await;
    assert_eq!(code, 404);
    let (code, deleted) = http_call("DELETE", server.http("/api/layouts"), None).await;
    assert_eq!(code, 200);
    assert_eq!(deleted["deleted"], 1);
    let (_, list) = http_call("GET", server.http("/api/layouts"), None).await;
    assert_eq!(list, json!([]));

    // Landing page when no UI dir, and msgpack not implemented yet.
    let (code, body) = http_call("GET", server.http("/"), None).await;
    assert_eq!(code, 200);
    assert!(body.as_str().unwrap().contains("corde-server"));
    let (code, _) = http_call("GET", server.http("/ws?format=msgpack"), None).await;
    assert_eq!(code, 501);
    assert!(
        tokio_tungstenite::connect_async(server.ws_url("?format=msgpack"))
            .await
            .is_err()
    );

    // Second client is counted; closing it brings the count back.
    let (mut ws2, _) = tokio_tungstenite::connect_async(server.ws_url("?rate=3"))
        .await
        .unwrap();
    let hello2 = next_json(&mut ws2).await;
    assert_eq!(hello2["type"], "hello");
    assert_eq!(hello2["status"]["clients"], 2);
    assert_eq!(hello2["status"]["output_rate_hz"], 3.0);
    ws2.close(None).await.unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let (_, status) = http_call("GET", server.http("/api/status"), None).await;
        if status["clients"] == 1 {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "client count must drop back to 1, got {status}"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    // Shutdown closes the remaining client cleanly.
    server.shutdown().await;
    let closed = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match ws.next().await {
                None | Some(Ok(Message::Close(_))) | Some(Err(_)) => break,
                _ => {}
            }
        }
    })
    .await;
    assert!(closed.is_ok(), "server shutdown must close the websocket");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn lap_and_event_messages_are_broadcast_and_laps_listed() {
    // 115 s laps at ×25 real-time speed: the first line crossing comes after
    // ≈ 4.6 s of real time.
    let data_dir = std::env::temp_dir().join(format!("corde-server-lap-{}", std::process::id()));
    let cfg = ServerConfig {
        port: 0,
        bind: Some(IpAddr::from([127, 0, 0, 1])),
        source: SourceKind::Sim,
        data_dir: Some(data_dir.clone()),
        sim_seed: Some(5),
        sim_ai: Some(3),
        sim_speed: Some(25.0),
        ..Default::default()
    };
    let server = Server::bind(cfg).await.unwrap();
    let addr = server.local_addr();
    let (stop, stop_rx) = oneshot::channel::<()>();
    let task = tokio::spawn(server.run(async move {
        let _ = stop_rx.await;
    }));

    let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://{addr}/ws"))
        .await
        .unwrap();
    let hello = next_json(&mut ws).await;
    assert_eq!(hello["type"], "hello");

    let deadline = Instant::now() + Duration::from_secs(30);
    let mut session_started = false;
    let mut lap_msg: Option<Value> = None;
    while Instant::now() < deadline && lap_msg.is_none() {
        let v = next_json(&mut ws).await;
        match v["type"].as_str() {
            Some("event") if v["kind"] == "session_started" => {
                session_started = true;
                assert_eq!(v["session"]["track_name"], "Paul Ricard - 1A");
            }
            Some("lap") => lap_msg = Some(v),
            _ => {}
        }
    }
    assert!(
        session_started,
        "a session_started event must be broadcast first"
    );
    let lap = lap_msg.expect("a Lap message within 30 s");
    let typed: corde_protocol::ServerMessage = serde_json::from_value(lap.clone()).unwrap();
    let corde_protocol::ServerMessage::Lap(rec) = typed else {
        panic!("not a lap: {lap}")
    };
    assert!(
        (rec.lap_time_s - 115.0).abs() < 1.5,
        "lap time {}",
        rec.lap_time_s
    );
    assert!(rec.valid && rec.s1.is_some() && rec.s2.is_some() && rec.s3.is_some());
    assert!(rec.fuel_used_l > 1.0 && rec.top_speed_kmh > 250.0);

    let (code, laps) = http_call("GET", format!("http://{addr}/api/laps"), None).await;
    assert_eq!(code, 200);
    let listed: Vec<corde_protocol::LapRecord> = serde_json::from_value(laps).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0], rec);

    let _ = stop.send(());
    let _ = tokio::time::timeout(Duration::from_secs(5), task).await;
    let _ = std::fs::remove_dir_all(&data_dir);
}
