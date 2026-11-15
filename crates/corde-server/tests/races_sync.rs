//! « Mes courses » shared between the devices (ADR 0019), between real servers: a « PC », a
//! « home server » (`--role hub`) and a « phone » that speaks to them the way the application
//! does (`POST /api/races/picks/sync`). A race picked on the PC reaches the phone through the
//! home server — the path used away from home —, a removal made on the phone comes back to
//! the PC, the phone and the PC also speak directly, and a home server that is older than the
//! sharing is recognised without an error.

use corde_server::hub::cli;
use corde_server::hub_link::{self, PicksExchange};
use corde_server::{AppState, Role, Server, ServerConfig, SourceKind};
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::oneshot;

fn temp_dir(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "corde-picks-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
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

fn pc_config(data: &Path) -> ServerConfig {
    ServerConfig {
        port: 0,
        bind: Some([127, 0, 0, 1].into()),
        source: SourceKind::Sim,
        data_dir: Some(data.to_path_buf()),
        no_record: true,
        no_import: true,
        no_game_api: true,
        rate: 1,
        sync_interval_s: 0,
        ..Default::default()
    }
}

/// `(status, JSON)`; `token` as `Authorization: Bearer`.
async fn http(
    method: &'static str,
    url: String,
    token: Option<String>,
    body: Option<Value>,
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
            ("DELETE", _) => {
                let mut r = agent.delete(&url);
                if let Some(a) = &auth {
                    r = r.header("Authorization", a);
                }
                r.call().expect("DELETE")
            }
            (m, Some(b)) => {
                let mut r = if m == "PUT" {
                    agent.put(&url)
                } else {
                    agent.post(&url)
                };
                if let Some(a) = &auth {
                    r = r.header("Authorization", a);
                }
                r.header("Content-Type", "application/json")
                    .send(b.to_string().as_bytes())
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
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

async fn pairing_code(hub_data: &Path) -> String {
    let d = hub_data.to_path_buf();
    tokio::task::spawn_blocking(move || cli::pair(&d, false).unwrap().code)
        .await
        .unwrap()
}

async fn exchange(state: &Arc<AppState>) -> PicksExchange {
    let st = Arc::clone(state);
    tokio::task::spawn_blocking(move || hub_link::sync_picks_once(&st))
        .await
        .unwrap()
        .expect("exchange")
}

fn pick(race: &str, start: i64) -> Value {
    json!({
        "id": format!("{race}@{start}"),
        "race_id": race,
        "cat": "daily",
        "src": "rc",
        "name": "One Stint Sprint",
        "level": "advanced",
        "track": "Monza",
        "duration": "55 min",
        "image": null,
        "start_ms": start,
        "eve_ms": null
    })
}

fn ids(set: &Value) -> Vec<String> {
    set["picks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["pick"]["id"].as_str().unwrap().to_string())
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_race_picked_on_the_pc_reaches_the_phone_through_the_home_server_and_back() {
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
    let pc = start(pc_config(&pc_data)).await;
    let pc_url = format!("http://{}", pc.bound);
    let sync_hub = format!("{hub_url}/api/races/picks/sync");
    let sync_pc = format!("{pc_url}/api/races/picks/sync");

    // Nothing paired: the PC has nobody to exchange with.
    assert_eq!(exchange(&pc.state).await, PicksExchange::NotPaired);

    // The phone, then the PC, are paired with the home server.
    let code = pairing_code(&hub_data).await;
    let (status, v) = http(
        "POST",
        format!("{hub_url}/api/hub/pair"),
        None,
        Some(json!({ "code": code, "name": "Téléphone", "kind": "app" })),
    )
    .await;
    assert_eq!(status, 201, "{v}");
    let phone = Some(v["token"].as_str().unwrap().to_string());
    let code = pairing_code(&hub_data).await;
    let (status, v) = http(
        "POST",
        format!("{pc_url}/api/hub-link/pair"),
        None,
        Some(json!({ "url": hub_url, "code": code, "name": "PC" })),
    )
    .await;
    assert_eq!(status, 200, "{v}");

    // The home server only answers a paired device.
    let (status, _) = http("POST", sync_hub.clone(), None, Some(json!({}))).await;
    assert_eq!(status, 401);

    // --- a race picked on the PC: the phone finds it on the home server ------------
    let start_a = now_ms() + 2 * 3_600_000;
    let a = pick("one-stint-sprint-advanced", start_a);
    let id_a = a["id"].as_str().unwrap().to_string();
    let (status, _) = http(
        "PUT",
        format!("{pc_url}/api/races/picks/{id_a}"),
        None,
        Some(a.clone()),
    )
    .await;
    assert_eq!(status, 201);
    // The PC sends it by itself, at once (no round of the sessions needed).
    let set = wait_for(
        "the pick on the home server",
        Duration::from_secs(15),
        async || {
            let (status, v) = http("POST", sync_hub.clone(), phone.clone(), Some(json!({}))).await;
            assert_eq!(status, 200, "{v}");
            (ids(&v) == [id_a.as_str()]).then_some(v)
        },
    )
    .await;
    assert_eq!(set["picks"][0]["pick"], a);
    let changed_a = set["picks"][0]["changed_ms"].as_i64().unwrap();
    assert!(changed_a > 0);
    assert_eq!(set["removed"], json!([]));

    // --- removed on the phone: the PC follows ----------------------------------------
    let gone = json!({ "picks": [], "removed": [{ "id": id_a, "removed_ms": changed_a + 1 }] });
    let (status, v) = http("POST", sync_hub.clone(), phone.clone(), Some(gone.clone())).await;
    assert_eq!(status, 200, "{v}");
    assert_eq!(ids(&v), Vec::<String>::new());
    assert_eq!(v["removed"], gone["removed"]);
    assert_eq!(
        exchange(&pc.state).await,
        PicksExchange::Done { changed: true }
    );
    let (_, picks) = http("GET", format!("{pc_url}/api/races/picks"), None, None).await;
    assert_eq!(picks, json!([]), "removed on the PC too");
    // Nothing new: the next exchange changes nothing.
    assert_eq!(
        exchange(&pc.state).await,
        PicksExchange::Done { changed: false }
    );
    // An older copy of the phone (still holding the pick) does not bring it back.
    let stale = json!({ "picks": [{ "pick": a, "changed_ms": changed_a }], "removed": [] });
    let (_, v) = http("POST", sync_hub.clone(), phone.clone(), Some(stale)).await;
    assert_eq!(ids(&v), Vec::<String>::new());

    // --- picked on the phone: the PC shows it and will ring it ------------------------
    let start_b = now_ms() + 3 * 3_600_000;
    let b = pick("weekly-gt3-sprint", start_b);
    let id_b = b["id"].as_str().unwrap().to_string();
    let mine = json!({ "picks": [{ "pick": b, "changed_ms": now_ms() }], "removed": [] });
    let (status, v) = http("POST", sync_hub.clone(), phone.clone(), Some(mine)).await;
    assert_eq!(status, 200, "{v}");
    assert_eq!(ids(&v), [id_b.as_str()]);
    assert_eq!(
        exchange(&pc.state).await,
        PicksExchange::Done { changed: true }
    );
    let (_, picks) = http("GET", format!("{pc_url}/api/races/picks"), None, None).await;
    assert_eq!(picks, json!([b]));

    // --- removed on the PC (its own page): the home server forgets it too --------------
    let (status, _) = http(
        "DELETE",
        format!("{pc_url}/api/races/picks/{id_b}"),
        None,
        None,
    )
    .await;
    assert_eq!(status, 204);
    wait_for(
        "the removal on the home server",
        Duration::from_secs(15),
        async || {
            let (_, v) = http("POST", sync_hub.clone(), phone.clone(), Some(json!({}))).await;
            let removed: Vec<&str> = v["removed"]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| r["id"].as_str().unwrap())
                .collect();
            (ids(&v).is_empty() && removed.contains(&id_b.as_str())).then_some(v.clone())
        },
    )
    .await;

    // --- at home, the phone also speaks to the PC directly (no pairing with a PC) -------
    let start_c = now_ms() + 4 * 3_600_000;
    let c = pick("one-stint-sprint-beginner", start_c);
    let id_c = c["id"].as_str().unwrap().to_string();
    let mine = json!({ "picks": [{ "pick": c, "changed_ms": now_ms() }] });
    let (status, v) = http("POST", sync_pc.clone(), None, Some(mine)).await;
    assert_eq!(status, 200, "{v}");
    assert_eq!(ids(&v), [id_c.as_str()]);
    // … and the PC passes it on to the home server by itself.
    wait_for(
        "the phone's pick on the home server",
        Duration::from_secs(15),
        async || {
            let (_, v) = http("POST", sync_hub.clone(), phone.clone(), Some(json!({}))).await;
            (ids(&v) == [id_c.as_str()]).then_some(v)
        },
    )
    .await;

    // --- what is not a copy of the picks is refused, and changes nothing -----------------
    for bad in [
        json!([]),
        json!({ "picks": {} }),
        json!({ "removed": "all" }),
    ] {
        let (status, v) = http("POST", sync_pc.clone(), None, Some(bad)).await;
        assert_eq!(status, 400, "{v}");
    }
    // Entries that do not pass are left out; the rest is merged.
    let mixed = json!({
        "picks": [
            { "pick": { "id": "x@1", "race_id": "x" }, "changed_ms": 5 },
            { "pick": pick("Not Valid", start_c), "changed_ms": 5 }
        ],
        "removed": [{ "id": "../../etc@1", "removed_ms": 5 }]
    });
    let (status, v) = http("POST", sync_pc.clone(), None, Some(mixed)).await;
    assert_eq!(status, 200, "{v}");
    assert_eq!(ids(&v), [id_c.as_str()]);

    // On disk: the list keeps its shape (an older version still reads it), the instants
    // are apart.
    let on_disk: Value =
        serde_json::from_slice(&std::fs::read(pc_data.join("races/picks.json")).unwrap()).unwrap();
    assert_eq!(on_disk, json!([c]));
    let marks: Value =
        serde_json::from_slice(&std::fs::read(pc_data.join("races/picks-sync.json")).unwrap())
            .unwrap();
    assert!(marks["changed"][&id_c].as_i64().unwrap() > 0);
    assert!(marks["removed"][&id_b].as_i64().unwrap() > 0);

    pc.stop().await;
    hub.stop().await;
    let _ = std::fs::remove_dir_all(&hub_data);
    let _ = std::fs::remove_dir_all(&pc_data);
}

/// A home server installed before the sharing (it only knows `PUT` and `DELETE` of one pick
/// at this address) answers 405: the PC takes note and keeps its picks.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_home_server_older_than_the_sharing_is_recognised() {
    use axum::routing::put;
    let old = axum::Router::new().route(
        "/api/races/picks/{id}",
        put(|| async { "stored" }).delete(|| async { "deleted" }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let old_addr = listener.local_addr().unwrap();
    let old_task = tokio::spawn(async move {
        let _ = axum::serve(listener, old).await;
    });

    let pc_data = temp_dir("pc-old");
    std::fs::create_dir_all(&pc_data).unwrap();
    // The link as a pairing leaves it (a made-up token: this stand-in checks nothing).
    std::fs::write(
        pc_data.join("hub-link.json"),
        json!({ "url": format!("http://{old_addr}"), "token": "a".repeat(64) }).to_string(),
    )
    .unwrap();
    let pc = start(pc_config(&pc_data)).await;
    let pc_url = format!("http://{}", pc.bound);
    let start_a = now_ms() + 2 * 3_600_000;
    let a = pick("one-stint-sprint-advanced", start_a);
    let (status, _) = http(
        "PUT",
        format!("{pc_url}/api/races/picks/{}", a["id"].as_str().unwrap()),
        None,
        Some(a.clone()),
    )
    .await;
    assert_eq!(status, 201);
    assert_eq!(exchange(&pc.state).await, PicksExchange::ServerTooOld);
    let (_, picks) = http("GET", format!("{pc_url}/api/races/picks"), None, None).await;
    assert_eq!(picks, json!([a]), "the PC keeps its picks");

    pc.stop().await;
    old_task.abort();
    let _ = std::fs::remove_dir_all(&pc_data);
}
