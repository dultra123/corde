//! What a paired device cannot do to the home server (docs/SECURITY-MODEL.md §7.4): ask
//! whether a file of the server exists, wrap an offset around, fill the disk with files it
//! never completes — and what nobody can do to any server: hold its connections for ever.

use corde_protocol::{GameId, SessionType, VehicleClass, Weather};
use corde_server::hub::{self, cli};
use corde_server::sync_wire::{self, HEADER_SCHEMA_VERSION, HEADER_SYNC_VERSION, SYNC_VERSION};
use corde_server::{Role, Server, ServerConfig};
use corde_store::db::{SampleFileRow, SessionRow};
use corde_store::SessionBundle;
use serde_json::Value;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::sync::oneshot;

fn temp_dir(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "corde-hub-hard-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

struct Running {
    bound: SocketAddr,
    stop: oneshot::Sender<()>,
    task: tokio::task::JoinHandle<anyhow::Result<()>>,
}

async fn start_hub(data: &Path) -> Running {
    let server = Server::bind(ServerConfig {
        role: Role::Hub,
        port: 0,
        bind: Some([127, 0, 0, 1].into()),
        data_dir: Some(data.to_path_buf()),
        ..Default::default()
    })
    .await
    .expect("bind hub");
    let bound = server.local_addr();
    let (stop, rx) = oneshot::channel::<()>();
    let task = tokio::spawn(server.run(async move {
        let _ = rx.await;
    }));
    Running { bound, stop, task }
}

impl Running {
    async fn stop(self) {
        let _ = self.stop.send(());
        tokio::time::timeout(Duration::from_secs(10), self.task)
            .await
            .expect("server stops")
            .expect("join")
            .expect("run ok");
    }
}

/// One request of a paired PC: `(status, JSON body)`.
async fn sync(
    base: &str,
    token: &str,
    method: &'static str,
    path: &str,
    body: Vec<u8>,
) -> (u16, Value) {
    let url = format!("{base}{path}");
    let auth = format!("Bearer {token}");
    tokio::task::spawn_blocking(move || {
        let agent: ureq::Agent = ureq::config::Config::builder()
            .http_status_as_error(false)
            .proxy(None)
            .build()
            .new_agent();
        let req = match method {
            "PUT" => agent.put(&url),
            _ => agent.post(&url),
        };
        let mut resp = req
            .header("Authorization", &auth)
            .header(HEADER_SYNC_VERSION, &SYNC_VERSION.to_string())
            .header(
                HEADER_SCHEMA_VERSION,
                &corde_store::db::SCHEMA_VERSION.to_string(),
            )
            .header("Content-Type", "application/octet-stream")
            .send(&body[..])
            .expect("request");
        let status = resp.status().as_u16();
        let text = resp.body_mut().read_to_string().unwrap_or_default();
        (status, serde_json::from_str(&text).unwrap_or(Value::Null))
    })
    .await
    .unwrap()
}

async fn pair(base: &str, data: &Path) -> String {
    let data = data.to_path_buf();
    let code = tokio::task::spawn_blocking(move || cli::pair(&data, false).unwrap().code)
        .await
        .unwrap();
    let url = format!("{base}/api/hub/pair");
    tokio::task::spawn_blocking(move || {
        let agent: ureq::Agent = ureq::config::Config::builder()
            .http_status_as_error(false)
            .proxy(None)
            .build()
            .new_agent();
        let mut resp = agent
            .post(&url)
            .header("Content-Type", "application/json")
            .send(format!(r#"{{"code":"{code}","name":"PC","kind":"app"}}"#).as_bytes())
            .expect("pair");
        assert_eq!(resp.status().as_u16(), 201);
        let v: Value = serde_json::from_str(&resp.body_mut().read_to_string().unwrap()).unwrap();
        v["token"].as_str().unwrap().to_string()
    })
    .await
    .unwrap()
}

fn session(id: &str) -> SessionRow {
    SessionRow {
        id: id.into(),
        game: GameId::Lmu,
        track_name: "Monza".into(),
        track_length_m: 5793.0,
        session_type: SessionType::Practice,
        session_index: 0,
        started_wall_ms: 1_700_000_000_000,
        ended_wall_ms: Some(1_700_000_600_000),
        player_name: "Pilote".into(),
        vehicle_name: "Voiture".into(),
        vehicle_model: "voiture".into(),
        vehicle_class: VehicleClass::Gt3,
        class_name: "GT3".into(),
        server_name: String::new(),
        is_multiplayer: false,
        game_version: None,
        source: "test".into(),
        record_rate_hz: 50.0,
        sample_count: 0,
        lap_count: 0,
        best_lap_s: None,
        weather: Weather::default(),
        notes: None,
    }
}

fn bundle(id: &str, file: &str) -> Vec<u8> {
    let b = SessionBundle {
        session: session(id),
        laps: vec![],
        events: vec![],
        setups: vec![],
        sample_files: vec![SampleFileRow {
            session_id: id.into(),
            file_index: 0,
            name: file.into(),
            row_start: 0,
            row_end: 0,
            bytes: 0,
            closed: true,
        }],
        reconciled_result_file: None,
    };
    sync_wire::compress_json(&b).unwrap().0
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_paired_device_cannot_probe_fill_or_wrap() {
    let data = temp_dir("sync");
    let hub = start_hub(&data).await;
    let base = format!("http://{}", hub.bound);
    let token = pair(&base, &data).await;
    let id = "20261103-213000-monza-practice";

    // --- no way to ask whether a file of the server exists ---------------------------
    // A file that exists for sure, and one that does not: the same refusal, before any
    // look at the disk (it used to be 400 for the first — « name refused » after it was
    // found — and 409 « missing » for the second).
    let exists = std::env::current_exe().unwrap();
    let exists = exists.to_str().unwrap();
    for name in [
        exists,
        "/definitely/not/there",
        "../../../etc/passwd",
        "../corde.db",
        "samples-000.parquet/../../x",
        "..",
        "",
    ] {
        let (status, body) = sync(
            &base,
            &token,
            "PUT",
            &format!("/api/sync/sessions/{id}"),
            bundle(id, name),
        )
        .await;
        assert_eq!(status, 400, "{name:?}: {body}");
        assert!(body["missing"].is_null(), "{name:?}: {body}");
    }
    // A real sample file that is not there yet: said missing, as before.
    let (status, body) = sync(
        &base,
        &token,
        "PUT",
        &format!("/api/sync/sessions/{id}"),
        bundle(id, "samples-000.parquet"),
    )
    .await;
    assert_eq!(status, 409, "{body}");
    assert_eq!(body["missing"][0], "samples-000.parquet");

    // --- an offset cannot wrap around --------------------------------------------------
    let (status, body) = sync(
        &base,
        &token,
        "PUT",
        &format!("/api/sync/files/{id}/samples-000.parquet?offset=18446744073709551615&total=10"),
        vec![1, 2],
    )
    .await;
    assert_eq!(status, 400, "{body}");
    // A chunk beyond the announced size neither.
    let (status, _) = sync(
        &base,
        &token,
        "PUT",
        &format!("/api/sync/files/{id}/samples-000.parquet?offset=9&total=10"),
        vec![1, 2],
    )
    .await;
    assert_eq!(status, 400);

    // --- files nobody completes do not pile up ---------------------------------------
    // A PC sends one file at a time; here a device starts as many as it can.
    let mut refused_at = None;
    for i in 0..hub::INCOMING_MAX_FILES + 2 {
        let (status, body) = sync(
            &base,
            &token,
            "PUT",
            &format!("/api/sync/files/s{i}/samples-000.parquet?offset=0&total=100"),
            vec![7; 10],
        )
        .await;
        if status != 200 {
            assert_eq!(status, 507, "{body}");
            refused_at = Some(i);
            break;
        }
    }
    assert_eq!(refused_at, Some(hub::INCOMING_MAX_FILES));
    let (files, bytes) = hub::incoming_usage(&data.join(hub::INCOMING_DIR));
    assert_eq!(files, hub::INCOMING_MAX_FILES);
    assert_eq!(bytes, 10 * hub::INCOMING_MAX_FILES as u64);
    // One already started can still go on.
    let (status, body) = sync(
        &base,
        &token,
        "PUT",
        "/api/sync/files/s0/samples-000.parquet?offset=10&total=100",
        vec![7; 10],
    )
    .await;
    assert_eq!(status, 200, "{body}");

    // Those nobody completed are removed once old (here: at once), with their folders.
    let incoming = data.join(hub::INCOMING_DIR);
    assert_eq!(hub::prune_incoming(&incoming, hub::INCOMING_MAX_AGE), 0);
    assert_eq!(
        hub::prune_incoming(&incoming, Duration::ZERO),
        hub::INCOMING_MAX_FILES
    );
    assert_eq!(hub::incoming_usage(&incoming), (0, 0));
    assert_eq!(std::fs::read_dir(&incoming).unwrap().count(), 0);

    hub.stop().await;
    let _ = std::fs::remove_dir_all(&data);
}

/// Before any pairing, anything that reaches the port can open connections: they are
/// answered or closed, never kept for ever, and the answers carry the browser protections.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_unpaired_visitor_gets_refusals_with_the_browser_protections() {
    use std::io::{Read, Write};
    let data = temp_dir("visitor");
    let hub = start_hub(&data).await;
    let to = hub.bound;
    let answer = tokio::task::spawn_blocking(move || {
        let mut s = std::net::TcpStream::connect_timeout(&to, Duration::from_secs(5)).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
        s.write_all(
            format!("GET /api/sessions HTTP/1.1\r\nHost: {to}\r\nConnection: close\r\n\r\n")
                .as_bytes(),
        )
        .unwrap();
        let mut out = String::new();
        let _ = s.read_to_string(&mut out);
        out.to_ascii_lowercase()
    })
    .await
    .unwrap();
    assert!(answer.starts_with("http/1.1 401"), "{answer}");
    for header in [
        "x-content-type-options: nosniff",
        "x-frame-options: sameorigin",
        "content-security-policy: default-src 'none'; frame-ancestors 'none'",
        "cross-origin-resource-policy: same-origin",
        "referrer-policy: no-referrer",
        "cache-control: no-store",
    ] {
        assert!(answer.contains(header), "{header} missing in {answer}");
    }
    hub.stop().await;
    let _ = std::fs::remove_dir_all(&data);
}
