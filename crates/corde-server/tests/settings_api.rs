//! Settings and network end to end: the default listening address is the
//! loopback, `PUT /api/settings` writes `<data-dir>/settings.json` (the file
//! shared with the desktop shell) and the next start follows it.

use corde_server::{resolve_bind, Server, ServerConfig, Settings, SourceKind};
use serde_json::{json, Value};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::sync::oneshot;

fn temp_data_dir(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "corde-settings-api-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

/// `(status, body)` of one request sent from this machine. `body` is sent as
/// it is (so that broken JSON can be tried), `origin` adds the header a
/// browser would send.
async fn call(
    method: &'static str,
    url: String,
    body: Option<&'static str>,
    origin: Option<&'static str>,
) -> (u16, Value) {
    tokio::task::spawn_blocking(move || {
        let agent: ureq::Agent = ureq::config::Config::builder()
            .http_status_as_error(false)
            .proxy(None)
            .build()
            .new_agent();
        let mut resp = match method {
            "GET" => agent.get(&url).call().expect("GET"),
            "PUT" => {
                let mut req = agent.put(&url).header("Content-Type", "application/json");
                if let Some(o) = origin {
                    req = req.header("Origin", o);
                }
                req.send(body.unwrap_or("")).expect("PUT")
            }
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

struct Running {
    /// Where the server listens (may be 0.0.0.0).
    bound: SocketAddr,
    /// How the test reaches it.
    base: String,
    stop: oneshot::Sender<()>,
    task: tokio::task::JoinHandle<anyhow::Result<()>>,
}

impl Running {
    /// A simulator server on a free port, with nothing said about the network
    /// unless `tweak` does.
    async fn start(data_dir: &Path, tweak: impl FnOnce(&mut ServerConfig)) -> Self {
        let mut cfg = ServerConfig {
            port: 0,
            source: SourceKind::Sim,
            data_dir: Some(data_dir.to_path_buf()),
            no_record: true,
            no_import: true,
            ..Default::default()
        };
        tweak(&mut cfg);
        let server = Server::bind(cfg).await.expect("bind");
        let bound = server.local_addr();
        assert_eq!(
            server.listens_on_lan(),
            !bound.ip().is_loopback(),
            "{bound}"
        );
        let banner = server.banner().join("\n");
        assert!(
            banner.contains(if server.listens_on_lan() {
                "OUVERT"
            } else {
                "FERMÉ"
            }),
            "{banner}"
        );
        let (stop, stop_rx) = oneshot::channel::<()>();
        let task = tokio::spawn(server.run(async move {
            let _ = stop_rx.await;
        }));
        Self {
            bound,
            base: format!("http://127.0.0.1:{}", bound.port()),
            stop,
            task,
        }
    }

    async fn stop(self) {
        let _ = self.stop.send(());
        tokio::time::timeout(Duration::from_secs(5), self.task)
            .await
            .expect("server stops within 5 s")
            .expect("join")
            .expect("server run ok");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn lan_access_is_off_by_default_saved_by_put_and_applied_at_the_next_start() {
    let data_dir = temp_data_dir("lan");
    let settings_file = data_dir.join("settings.json");

    // --- first start: nothing configured → this PC only ---------------------
    let server = Running::start(&data_dir, |_| {}).await;
    assert_eq!(
        server.bound.ip(),
        IpAddr::V4(Ipv4Addr::LOCALHOST),
        "the default must not expose the server (no firewall prompt)"
    );
    let base = server.base.clone();
    let port = server.bound.port();

    let (code, settings) = call("GET", format!("{base}/api/settings"), None, None).await;
    assert_eq!(code, 200);
    assert_eq!(settings, json!({ "lan_access": false }));
    assert!(
        !settings_file.exists(),
        "reading the settings must not create the file"
    );

    let (code, net) = call("GET", format!("{base}/api/network"), None, None).await;
    assert_eq!(code, 200);
    assert_eq!(
        net,
        json!({ "lan_access_setting": false, "listening_on_lan": false, "port": port, "urls": [] })
    );

    // --- validation ---------------------------------------------------------
    for bad in [
        "",
        "oui",
        "[true]",
        r#"{"lan_access": "yes"}"#,
        r#"{"lan_access": 1}"#,
        r#"{"lan_acces": true}"#,
    ] {
        let (code, body) = call("PUT", format!("{base}/api/settings"), Some(bad), None).await;
        assert_eq!(code, 400, "{bad:?} → {body}");
        assert!(body["error"].is_string(), "{bad:?} → {body}");
    }
    assert!(
        !settings_file.exists(),
        "a rejected body must not write anything"
    );

    // A web page from somewhere else cannot flip the setting (CORS lets the
    // request through, the server refuses it).
    for origin in ["https://evil.example", "http://192.168.1.20:8471"] {
        let (code, body) = call(
            "PUT",
            format!("{base}/api/settings"),
            Some(r#"{"lan_access": true}"#),
            Some(origin),
        )
        .await;
        assert_eq!(code, 403, "{origin} → {body}");
    }
    assert!(!settings_file.exists());

    // An empty object changes nothing and still answers with the settings.
    let (code, body) = call("PUT", format!("{base}/api/settings"), Some("{}"), None).await;
    assert_eq!(code, 200, "{body}");
    assert_eq!(
        body,
        json!({ "lan_access": false, "restart_required": false })
    );

    // --- turning it on: saved, but this process keeps its address -----------
    let (code, body) = call(
        "PUT",
        format!("{base}/api/settings"),
        Some(r#"{"lan_access": true}"#),
        Some("http://127.0.0.1:8471"),
    )
    .await;
    assert_eq!(code, 200, "{body}");
    assert_eq!(
        body,
        json!({ "lan_access": true, "restart_required": true })
    );
    let on_disk: Value = serde_json::from_slice(&std::fs::read(&settings_file).unwrap()).unwrap();
    assert_eq!(on_disk, json!({ "lan_access": true }));
    assert!(Settings::load(&data_dir).lan_access);

    let (_, settings) = call("GET", format!("{base}/api/settings"), None, None).await;
    assert_eq!(settings, json!({ "lan_access": true }));
    let (_, net) = call("GET", format!("{base}/api/network"), None, None).await;
    assert_eq!(net["lan_access_setting"], true);
    assert_eq!(
        net["listening_on_lan"], false,
        "applied at the next start only"
    );
    assert_eq!(net["urls"], json!([]));

    // The desktop shell may keep its own keys in the same file: they survive.
    std::fs::write(
        &settings_file,
        r#"{ "lan_access": true, "shell": { "minimized": true } }"#,
    )
    .unwrap();
    let (code, body) = call(
        "PUT",
        format!("{base}/api/settings"),
        Some(r#"{"lan_access": true}"#),
        None,
    )
    .await;
    assert_eq!(code, 200, "{body}");
    let on_disk: Value = serde_json::from_slice(&std::fs::read(&settings_file).unwrap()).unwrap();
    assert_eq!(
        on_disk,
        json!({ "lan_access": true, "shell": { "minimized": true } })
    );
    server.stop().await;

    // --- second start: the setting opens the LAN ----------------------------
    let server = Running::start(&data_dir, |_| {}).await;
    assert!(
        server.bound.ip().is_unspecified(),
        "lan_access → 0.0.0.0, got {}",
        server.bound
    );
    let base = server.base.clone();
    let port = server.bound.port();
    let (code, net) = call("GET", format!("{base}/api/network"), None, None).await;
    assert_eq!(code, 200);
    assert_eq!(net["lan_access_setting"], true);
    assert_eq!(net["listening_on_lan"], true);
    assert_eq!(net["port"], port);
    for url in net["urls"].as_array().expect("urls") {
        let url = url.as_str().unwrap();
        assert!(
            url.starts_with("http://") && url.ends_with(&format!(":{port}")),
            "{url}"
        );
    }

    // Turning it off again asks for a restart too.
    let (code, body) = call(
        "PUT",
        format!("{base}/api/settings"),
        Some(r#"{"lan_access": false}"#),
        None,
    )
    .await;
    assert_eq!(code, 200, "{body}");
    assert_eq!(
        body,
        json!({ "lan_access": false, "restart_required": true })
    );
    let (_, net) = call("GET", format!("{base}/api/network"), None, None).await;
    assert_eq!(net["lan_access_setting"], false);
    assert_eq!(net["listening_on_lan"], true);
    server.stop().await;

    // --- third start: back to this PC only ----------------------------------
    let server = Running::start(&data_dir, |_| {}).await;
    assert!(server.bound.ip().is_loopback());
    server.stop().await;
    let _ = std::fs::remove_dir_all(&data_dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn command_line_wins_over_the_saved_setting() {
    let data_dir = temp_data_dir("cli");
    Settings {
        lan_access: true,
        ..Default::default()
    }
    .save(&data_dir)
    .expect("write settings.json");

    // What the desktop shell computes before starting the server.
    let cfg = ServerConfig {
        data_dir: Some(data_dir.clone()),
        ..Default::default()
    };
    assert!(resolve_bind(&cfg, &Settings::load(&cfg.resolve_data_dir())).is_unspecified());

    // --bind 127.0.0.1 keeps the server private although the setting is on.
    let server = Running::start(&data_dir, |c| {
        c.bind = Some(IpAddr::V4(Ipv4Addr::LOCALHOST));
    })
    .await;
    assert!(server.bound.ip().is_loopback());
    let (_, net) = call("GET", format!("{}/api/network", server.base), None, None).await;
    assert_eq!(net["lan_access_setting"], true);
    assert_eq!(net["listening_on_lan"], false);
    assert_eq!(net["urls"], json!([]));
    server.stop().await;

    // --lan opens it although the setting is off.
    Settings::default().save(&data_dir).unwrap();
    let server = Running::start(&data_dir, |c| c.lan = true).await;
    assert!(server.bound.ip().is_unspecified());
    let (_, net) = call("GET", format!("{}/api/network", server.base), None, None).await;
    assert_eq!(net["lan_access_setting"], false);
    assert_eq!(net["listening_on_lan"], true);
    server.stop().await;
    let _ = std::fs::remove_dir_all(&data_dir);
}
