//! PC → home server synchronisation between two real servers (ADR 0010): a « PC » that
//! records a short simulator session, a « server » with `--role hub`. Pairing, a round cut
//! in the middle of a file, the next round resuming it, a third round sending nothing, the
//! session and its analysis identical on both sides, feedback typed on the server coming
//! down to the PC (and back up), a server too old refused with a clear message, and the
//! states shown when the server is unreachable.

use corde_server::hub::cli;
use corde_server::hub_link::{self, LinkState, SyncError, SyncOptions, SyncReport};
use corde_server::sync_wire::{HEADER_SCHEMA_VERSION, HEADER_SYNC_VERSION, SYNC_VERSION};
use corde_server::{AppState, Role, Server, ServerConfig, SourceKind};
use serde_json::Value;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::oneshot;

fn temp_dir(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "corde-sync-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

struct Running {
    bound: SocketAddr,
    state: Arc<AppState>,
    stop: oneshot::Sender<()>,
    task: tokio::task::JoinHandle<anyhow::Result<()>>,
}

async fn start(cfg: ServerConfig) -> Running {
    let server = Server::bind(cfg).await.expect("bind");
    let bound = server.local_addr();
    let state = Arc::clone(server.state());
    let (stop, rx) = oneshot::channel::<()>();
    let task = tokio::spawn(server.run(async move {
        let _ = rx.await;
    }));
    Running {
        bound,
        state,
        stop,
        task,
    }
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

/// `(status, JSON)`; `token` as `Authorization: Bearer`.
async fn http(
    method: &'static str,
    url: String,
    token: Option<String>,
    body: Option<(String, Vec<(String, String)>)>,
) -> (u16, Value) {
    tokio::task::spawn_blocking(move || {
        let agent: ureq::Agent = ureq::config::Config::builder()
            .http_status_as_error(false)
            .proxy(None)
            .build()
            .new_agent();
        let auth = token.map(|t| format!("Bearer {t}"));
        let mut resp = match (method, body) {
            ("GET", _) => {
                let mut r = agent.get(&url);
                if let Some(a) = &auth {
                    r = r.header("Authorization", a);
                }
                r.call().expect("GET")
            }
            ("DELETE", _) => agent.delete(&url).call().expect("DELETE"),
            ("POST", None) => {
                let mut r = agent.post(&url);
                if let Some(a) = &auth {
                    r = r.header("Authorization", a);
                }
                r.send_empty().expect("POST")
            }
            (m, Some((b, headers))) => {
                let mut r = if m == "PUT" {
                    agent.put(&url)
                } else {
                    agent.post(&url)
                };
                if let Some(a) = &auth {
                    r = r.header("Authorization", a);
                }
                for (k, v) in &headers {
                    r = r.header(k, v);
                }
                r.header("Content-Type", "application/json")
                    .send(b.as_bytes())
                    .expect("send")
            }
            _ => unreachable!(),
        };
        let status = resp.status().as_u16();
        let text = resp.body_mut().read_to_string().unwrap_or_default();
        (
            status,
            serde_json::from_str(&text).unwrap_or(Value::String(text)),
        )
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
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

async fn pairing_code(hub_data: &Path) -> String {
    let d = hub_data.to_path_buf();
    tokio::task::spawn_blocking(move || cli::pair(&d, false).unwrap().code)
        .await
        .unwrap()
}

async fn sync(state: &Arc<AppState>, opts: SyncOptions) -> Result<SyncReport, SyncError> {
    let st = Arc::clone(state);
    tokio::task::spawn_blocking(move || hub_link::sync_once(&st, &opts))
        .await
        .unwrap()
}

/// The analysis without what legitimately differs (when it was computed).
fn comparable(mut v: Value) -> Value {
    if let Value::Object(m) = &mut v {
        m.remove("generated_ms");
    }
    v
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_pc_session_reaches_the_home_server_once_and_identical() {
    // --- the home server and the PC ------------------------------------------------
    let hub_data = temp_dir("hub");
    let pc_data = temp_dir("pc");
    let hub = start(ServerConfig {
        role: Role::Hub,
        port: 0,
        bind: Some([127, 0, 0, 1].into()),
        data_dir: Some(hub_data.clone()),
        ..Default::default()
    })
    .await;
    let hub_url = format!("http://{}", hub.bound);
    let pc = start(ServerConfig {
        port: 0,
        bind: Some([127, 0, 0, 1].into()),
        source: SourceKind::Sim,
        data_dir: Some(pc_data.clone()),
        sim_seed: Some(11),
        sim_ai: Some(2),
        sim_lap_time: Some(30.0),
        sim_track_length: Some(1500.0),
        sim_speed: Some(3.0),
        sim_rate: Some(100.0),
        record_rate: 20.0,
        no_import: true,
        no_game_api: true,
        sync_interval_s: 0,
        ..Default::default()
    })
    .await;
    let pc_base = format!("http://{}", pc.bound);

    // Not paired yet: a round has nothing to do, the settings say so.
    assert_eq!(
        sync(&pc.state, SyncOptions::default()).await,
        Err(SyncError::NotPaired)
    );
    let (code, link) = http("GET", format!("{pc_base}/api/hub-link"), None, None).await;
    assert_eq!(code, 200);
    assert_eq!(link["paired"], false);

    // --- a short session recorded on the PC ---------------------------------------
    let rec = wait_for("recording", Duration::from_secs(20), async || {
        let (_, v) = http("GET", format!("{pc_base}/api/recording"), None, None).await;
        (v["recording"] == true).then_some(v)
    })
    .await;
    let id = rec["session_id"].as_str().unwrap().to_string();
    wait_for("4 laps", Duration::from_secs(120), async || {
        let (_, v) = http("GET", format!("{pc_base}/api/sessions/{id}"), None, None).await;
        (v["laps"].as_array().map_or(0, |l| l.len()) >= 4).then_some(v)
    })
    .await;
    let (code, _) = http("POST", format!("{pc_base}/api/recording/stop"), None, None).await;
    assert_eq!(code, 202);
    let pc_session = wait_for("session closed", Duration::from_secs(20), async || {
        let (_, v) = http("GET", format!("{pc_base}/api/sessions/{id}"), None, None).await;
        v["ended_wall_ms"].is_number().then_some(v)
    })
    .await;
    // Feedback typed on the PC before the first sync.
    let (code, _) = http(
        "POST",
        format!("{pc_base}/api/sessions/{id}/feedback"),
        None,
        Some((
            r#"{"plan":"safe","text":"sous-vire en entrée de virage lent"}"#.into(),
            vec![],
        )),
    )
    .await;
    assert_eq!(code, 201);
    let (code, pc_report) = http(
        "GET",
        format!("{pc_base}/api/sessions/{id}/analysis"),
        None,
        None,
    )
    .await;
    assert_eq!(code, 200, "{pc_report}");

    // --- pairing --------------------------------------------------------------------
    // A wrong code through the PC's route: a clear refusal, nothing stored.
    let (code, v) = http(
        "POST",
        format!("{pc_base}/api/hub-link/pair"),
        None,
        Some((
            format!(r#"{{"url":"{hub_url}","code":"00000000"}}"#),
            vec![],
        )),
    )
    .await;
    assert_eq!(code, 400, "{v}");
    assert!(v["error"].as_str().unwrap().contains("code"), "{v}");
    let c = pairing_code(&hub_data).await;
    let st = Arc::clone(&pc.state);
    let url = hub_url.clone();
    let status =
        tokio::task::spawn_blocking(move || hub_link::pair_blocking(&st, &url, &c, "PC de test"))
            .await
            .unwrap()
            .expect("paired");
    assert!(status.paired);
    let link_file = std::fs::read_to_string(pc_data.join(hub_link::LINK_FILE)).unwrap();
    let (_, shown) = http("GET", format!("{pc_base}/api/hub-link"), None, None).await;
    let token = serde_json::from_str::<Value>(&link_file).unwrap()["token"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(
        !shown.to_string().contains(&token),
        "the token is never shown: {shown}"
    );

    // A device to read the server with (as the phone would).
    let c = pairing_code(&hub_data).await;
    let (code, v) = http(
        "POST",
        format!("{hub_url}/api/hub/pair"),
        None,
        Some((
            format!(r#"{{"code":"{c}","name":"lecteur","kind":"app"}}"#),
            vec![],
        )),
    )
    .await;
    assert_eq!(code, 201, "{v}");
    let reader = Some(v["token"].as_str().unwrap().to_string());

    // --- a round cut in the middle of the Parquet file -------------------------------
    let chunk = 16 * 1024;
    let cut = sync(
        &pc.state,
        SyncOptions {
            chunk_size: chunk,
            max_chunks: Some(3),
            ..Default::default()
        },
    )
    .await;
    assert_eq!(cut, Err(SyncError::Interrupted));
    let (_, list) = http(
        "GET",
        format!("{hub_url}/api/sessions"),
        reader.clone(),
        None,
    )
    .await;
    assert_eq!(
        list.as_array().unwrap().len(),
        0,
        "nothing shown before the session is complete"
    );
    let parquet = pc_data
        .join("sessions")
        .join(&id)
        .join("samples-000.parquet");
    let size = std::fs::metadata(&parquet).unwrap().len();
    let part = hub_data
        .join("sync-incoming")
        .join(&id)
        .join("samples-000.parquet.part");
    assert_eq!(
        std::fs::metadata(&part).unwrap().len(),
        3 * chunk as u64,
        "3 chunks kept"
    );
    let (_, link) = http("GET", format!("{pc_base}/api/hub-link"), None, None).await;
    assert_eq!(link["state"], "unreachable", "{link}");
    assert_eq!(link["pending_sessions"], 1, "{link}");

    // --- the next round resumes where the cut left it ---------------------------------
    let report = sync(
        &pc.state,
        SyncOptions {
            chunk_size: chunk,
            ..Default::default()
        },
    )
    .await
    .expect("second round");
    let total_chunks = size.div_ceil(chunk as u64) as u32;
    assert_eq!(report.sessions_sent, 1, "{report:?}");
    assert_eq!(report.files_sent, 1, "{report:?}");
    assert_eq!(
        report.chunks_sent,
        total_chunks - 3,
        "resumed, not restarted: {report:?}"
    );
    assert!(
        report.feedback_pushed >= 2,
        "feedback + iteration: {report:?}"
    );
    assert!(!part.exists());
    let on_hub = hub_data
        .join("sessions")
        .join(&id)
        .join("samples-000.parquet");
    assert_eq!(
        std::fs::read(&on_hub).unwrap(),
        std::fs::read(&parquet).unwrap()
    );
    let (_, link) = http("GET", format!("{pc_base}/api/hub-link"), None, None).await;
    assert_eq!(link["state"], "connected", "{link}");
    assert_eq!(link["pending_sessions"], 0);
    assert!(link["last_sync_ms"].is_number());

    // --- a third round sends nothing --------------------------------------------------
    let report = sync(&pc.state, SyncOptions::default())
        .await
        .expect("third round");
    assert_eq!(report, SyncReport::default(), "nothing twice");

    // --- the same session and the same analysis on the server -----------------------------
    let (code, hub_session) = http(
        "GET",
        format!("{hub_url}/api/sessions/{id}"),
        reader.clone(),
        None,
    )
    .await;
    assert_eq!(code, 200, "{hub_session}");
    for key in [
        "track_name",
        "started_wall_ms",
        "ended_wall_ms",
        "lap_count",
        "best_lap_s",
        "laps",
        "files",
    ] {
        assert_eq!(
            hub_session[key],
            pc_session_now(&pc_base, &id).await[key],
            "{key}"
        );
    }
    let (code, hub_report) = http(
        "GET",
        format!("{hub_url}/api/sessions/{id}/analysis"),
        reader.clone(),
        None,
    )
    .await;
    assert_eq!(code, 200, "{hub_report}");
    assert_eq!(
        comparable(hub_report),
        comparable(pc_report),
        "analysis computed on the server = the PC's"
    );
    let lap = pc_session["laps"][1]["lap_number"].as_i64().unwrap();
    let (code, samples) = http(
        "GET",
        format!("{hub_url}/api/sessions/{id}/laps/{lap}/samples?channels=speed_kmh&points=100"),
        reader.clone(),
        None,
    )
    .await;
    assert_eq!(code, 200, "{samples}");
    let (_, pc_samples) = http(
        "GET",
        format!("{pc_base}/api/sessions/{id}/laps/{lap}/samples?channels=speed_kmh&points=100"),
        None,
        None,
    )
    .await;
    assert_eq!(samples, pc_samples);
    let (_, fb) = http(
        "GET",
        format!("{hub_url}/api/sessions/{id}/feedback"),
        reader.clone(),
        None,
    )
    .await;
    assert_eq!(fb.as_array().unwrap().len(), 1, "{fb}");
    // The server knows where the PC serves the live view (here: nowhere on the LAN).
    let (_, st) = http(
        "GET",
        format!("{hub_url}/api/hub/status"),
        reader.clone(),
        None,
    )
    .await;
    assert_eq!(st["last_pc"]["device_name"], "PC de test", "{st}");
    assert_eq!(st["sessions"], 1);

    // --- feedback typed on the server (the phone, PC off) comes down ------------------------
    let (code, v) = http(
        "POST",
        format!("{hub_url}/api/sessions/{id}/feedback"),
        reader.clone(),
        Some((
            r#"{"plan":"safe","text":"survire en sortie de virage rapide"}"#.into(),
            vec![],
        )),
    )
    .await;
    assert_eq!(code, 201, "{v}");
    let report = sync(&pc.state, SyncOptions::default()).await.unwrap();
    assert!(report.feedback_pulled >= 2, "{report:?}");
    assert_eq!(report.sessions_sent, 0);
    let (_, fb) = http(
        "GET",
        format!("{pc_base}/api/sessions/{id}/feedback"),
        None,
        None,
    )
    .await;
    let texts: Vec<&str> = fb
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["text"].as_str().unwrap())
        .collect();
    assert_eq!(texts.len(), 2, "{texts:?}");
    assert!(texts.contains(&"survire en sortie de virage rapide"));
    let (_, it_pc) = http(
        "GET",
        format!("{pc_base}/api/sessions/{id}/iterations"),
        None,
        None,
    )
    .await;
    let (_, it_hub) = http(
        "GET",
        format!("{hub_url}/api/sessions/{id}/iterations"),
        reader.clone(),
        None,
    )
    .await;
    let numbers = |v: &Value| -> Vec<(i64, i64)> {
        v.as_array()
            .unwrap()
            .iter()
            .map(|i| {
                (
                    i["iteration"].as_i64().unwrap(),
                    i["created_ms"].as_i64().unwrap(),
                )
            })
            .collect()
    };
    assert_eq!(
        numbers(&it_pc),
        numbers(&it_hub),
        "same iterations, same numbers"
    );
    assert_eq!(numbers(&it_pc).len(), 2);
    let report = sync(&pc.state, SyncOptions::default()).await.unwrap();
    assert_eq!(report, SyncReport::default(), "converged");

    // --- « send everything again »: bundles again, nothing duplicated -------------------------
    let report = sync(
        &pc.state,
        SyncOptions {
            force: true,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(report.sessions_sent, 1);
    assert_eq!(
        report.files_sent, 0,
        "files are identified by their checksum"
    );
    let (_, list) = http(
        "GET",
        format!("{hub_url}/api/sessions"),
        reader.clone(),
        None,
    )
    .await;
    assert_eq!(list.as_array().unwrap().len(), 1);
    let (_, fb) = http(
        "GET",
        format!("{hub_url}/api/sessions/{id}/feedback"),
        reader.clone(),
        None,
    )
    .await;
    assert_eq!(
        fb.as_array().unwrap().len(),
        2,
        "feedback kept, not doubled"
    );

    // --- a PC newer than the server is refused with a message that says what to do ---------
    for (sync_v, schema_v) in [(SYNC_VERSION, 999), (SYNC_VERSION + 1, 1)] {
        let (code, v) = http(
            "POST",
            format!("{hub_url}/api/sync/hello"),
            reader.clone(),
            Some((
                r#"{"version":"9.9.9","sync_version":1,"schema_version":999}"#.into(),
                vec![
                    (HEADER_SYNC_VERSION.to_string(), sync_v.to_string()),
                    (HEADER_SCHEMA_VERSION.to_string(), schema_v.to_string()),
                ],
            )),
        )
        .await;
        assert_eq!(code, 409, "{v}");
        assert_eq!(v["code"], "server_outdated");
        assert!(
            v["error"]
                .as_str()
                .unwrap()
                .contains("mets à jour le serveur"),
            "{v}"
        );
    }
    // Without the headers, or without a device: refused too.
    let (code, _) = http(
        "POST",
        format!("{hub_url}/api/sync/hello"),
        reader.clone(),
        Some(("{}".into(), vec![])),
    )
    .await;
    assert_eq!(code, 400);
    let (code, _) = http(
        "POST",
        format!("{hub_url}/api/sync/hello"),
        None,
        Some(("{}".into(), vec![])),
    )
    .await;
    assert_eq!(code, 401);

    // --- the server switched off: « unreachable », the backlog waits --------------------------
    hub.stop().await;
    let r = sync(&pc.state, SyncOptions::default()).await;
    assert!(matches!(r, Err(SyncError::Unreachable(_))), "{r:?}");
    assert_eq!(
        pc.state.link.as_ref().unwrap().status().state,
        LinkState::Unreachable
    );
    let (_, link) = http("GET", format!("{pc_base}/api/hub-link"), None, None).await;
    assert!(
        link["message"].as_str().unwrap().contains("injoignable"),
        "{link}"
    );

    // Unpairing forgets the token (the server could not be told: it is off).
    let (code, v) = http("DELETE", format!("{pc_base}/api/hub-link"), None, None).await;
    assert_eq!(code, 200, "{v}");
    assert_eq!(v["revoked_on_server"], false);
    assert!(!pc_data.join(hub_link::LINK_FILE).exists());

    pc.stop().await;
    let _ = std::fs::remove_dir_all(&hub_data);
    let _ = std::fs::remove_dir_all(&pc_data);
}

async fn pc_session_now(pc_base: &str, id: &str) -> Value {
    http("GET", format!("{pc_base}/api/sessions/{id}"), None, None)
        .await
        .1
}

fn quick_pc(data: &Path, record: bool, interval_s: u64) -> ServerConfig {
    ServerConfig {
        port: 0,
        bind: Some([127, 0, 0, 1].into()),
        source: SourceKind::Sim,
        data_dir: Some(data.to_path_buf()),
        sim_seed: Some(3),
        sim_ai: Some(1),
        sim_lap_time: Some(30.0),
        sim_track_length: Some(1500.0),
        sim_speed: Some(3.0),
        record_rate: 10.0,
        no_record: !record,
        no_import: true,
        no_game_api: true,
        sync_interval_s: interval_s,
        ..Default::default()
    }
}

async fn hub_sessions(hub_url: &str, reader: &Option<String>) -> usize {
    let (_, list) = http(
        "GET",
        format!("{hub_url}/api/sessions"),
        reader.clone(),
        None,
    )
    .await;
    list.as_array().map_or(0, |l| l.len())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_backlog_goes_at_start_up_and_each_recording_when_it_ends() {
    let hub_data = temp_dir("hub2");
    let pc_data = temp_dir("pc2");
    let hub = start(ServerConfig {
        role: Role::Hub,
        port: 0,
        bind: Some([127, 0, 0, 1].into()),
        data_dir: Some(hub_data.clone()),
        ..Default::default()
    })
    .await;
    let hub_url = format!("http://{}", hub.bound);
    let c = pairing_code(&hub_data).await;
    let (_, v) = http(
        "POST",
        format!("{hub_url}/api/hub/pair"),
        None,
        Some((format!(r#"{{"code":"{c}","kind":"app"}}"#), vec![])),
    )
    .await;
    let reader = Some(v["token"].as_str().unwrap().to_string());

    // A session recorded while the PC was not paired yet: the backlog.
    let pc = start(quick_pc(&pc_data, true, 0)).await;
    let base = format!("http://{}", pc.bound);
    wait_for("recording", Duration::from_secs(20), async || {
        let (_, v) = http("GET", format!("{base}/api/recording"), None, None).await;
        (v["recording"] == true).then_some(v)
    })
    .await;
    tokio::time::sleep(Duration::from_secs(2)).await;
    http("POST", format!("{base}/api/recording/stop"), None, None).await;
    wait_for("closed", Duration::from_secs(20), async || {
        let (_, v) = http("GET", format!("{base}/api/recording"), None, None).await;
        (v["recording"] == false).then_some(v)
    })
    .await;
    let c = pairing_code(&hub_data).await;
    let (st, url) = (Arc::clone(&pc.state), hub_url.clone());
    tokio::task::spawn_blocking(move || hub_link::pair_blocking(&st, &url, &c, "PC"))
        .await
        .unwrap()
        .unwrap();
    pc.stop().await;
    assert_eq!(hub_sessions(&hub_url, &reader).await, 0);

    // Corde starts again: the backlog goes without being asked (round ~15 s after start-up).
    let pc = start(quick_pc(&pc_data, false, 3600)).await;
    let base = format!("http://{}", pc.bound);
    wait_for(
        "backlog on the server",
        Duration::from_secs(45),
        async || (hub_sessions(&hub_url, &reader).await == 1).then_some(Value::Null),
    )
    .await;

    // A new recording: sent as soon as it stops (the next periodic round is an hour away).
    tokio::time::sleep(Duration::from_millis(1100)).await;
    let (code, _) = http("POST", format!("{base}/api/recording/start"), None, None).await;
    assert_eq!(code, 202);
    wait_for("recording", Duration::from_secs(20), async || {
        let (_, v) = http("GET", format!("{base}/api/recording"), None, None).await;
        (v["recording"] == true).then_some(v)
    })
    .await;
    tokio::time::sleep(Duration::from_secs(2)).await;
    http("POST", format!("{base}/api/recording/stop"), None, None).await;
    wait_for(
        "the new session on the server",
        Duration::from_secs(30),
        async || (hub_sessions(&hub_url, &reader).await == 2).then_some(Value::Null),
    )
    .await;

    pc.stop().await;
    hub.stop().await;
    let _ = std::fs::remove_dir_all(&hub_data);
    let _ = std::fs::remove_dir_all(&pc_data);
}
