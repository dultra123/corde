//! Access policy end to end (`corde_server::guard`, docs/SECURITY-MODEL.md):
//! `Host` and `Origin` checks on a real socket, CORS headers, the WebSocket
//! upgrade, body size, static files outside the UI folder, and — when this
//! machine has a LAN address — the routes kept for this PC.

use corde_server::{Server, ServerConfig, SourceKind};
use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::sync::oneshot;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::HeaderValue;

fn temp_dir(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "corde-access-{tag}-{}-{}",
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

impl Running {
    async fn start(data_dir: &Path, tweak: impl FnOnce(&mut ServerConfig)) -> Self {
        let mut cfg = ServerConfig {
            port: 0,
            source: SourceKind::Sim,
            data_dir: Some(data_dir.to_path_buf()),
            no_record: true,
            no_import: true,
            no_game_api: true,
            ..Default::default()
        };
        tweak(&mut cfg);
        let server = Server::bind(cfg).await.expect("bind");
        let bound = server.local_addr();
        let (stop, stop_rx) = oneshot::channel::<()>();
        let task = tokio::spawn(server.run(async move {
            let _ = stop_rx.await;
        }));
        Self { bound, stop, task }
    }

    fn port(&self) -> u16 {
        self.bound.port()
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

/// One raw HTTP/1.1 request (so that `Host` can be anything) → `(status,
/// headers in lower case, body)`.
struct Reply {
    status: u16,
    head: String,
    body: String,
}

async fn raw(
    to: SocketAddr,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: &[u8],
) -> Reply {
    let mut req = format!("{method} {path} HTTP/1.1\r\nConnection: close\r\n");
    for (k, v) in headers {
        req.push_str(&format!("{k}: {v}\r\n"));
    }
    if !body.is_empty() || matches!(method, "POST" | "PUT") {
        req.push_str(&format!(
            "Content-Type: application/json\r\nContent-Length: {}\r\n",
            body.len()
        ));
    }
    req.push_str("\r\n");
    let mut bytes = req.into_bytes();
    bytes.extend_from_slice(body);
    tokio::task::spawn_blocking(move || {
        let mut s = TcpStream::connect_timeout(&to, Duration::from_secs(5)).expect("connect");
        s.set_read_timeout(Some(Duration::from_secs(30))).unwrap();
        // The server may answer (413) before reading the whole body.
        let _ = s.write_all(&bytes);
        let mut out = Vec::new();
        let _ = s.read_to_end(&mut out);
        let text = String::from_utf8_lossy(&out).into_owned();
        let (head, body) = text.split_once("\r\n\r\n").unwrap_or((&text, ""));
        let status = head
            .split_whitespace()
            .nth(1)
            .and_then(|c| c.parse().ok())
            .unwrap_or(0);
        Reply {
            status,
            head: head.to_ascii_lowercase(),
            body: body.to_string(),
        }
    })
    .await
    .unwrap()
}

/// Opens the WebSocket with these `Host` / `Origin`; `Ok` = upgraded.
async fn ws_open(to: SocketAddr, host: &str, origin: Option<&str>) -> Result<(), u16> {
    let mut req = format!("ws://{to}/ws").into_client_request().unwrap();
    req.headers_mut()
        .insert("host", HeaderValue::from_str(host).unwrap());
    if let Some(o) = origin {
        req.headers_mut()
            .insert("origin", HeaderValue::from_str(o).unwrap());
    }
    match tokio_tungstenite::connect_async(req).await {
        Ok(_) => Ok(()),
        Err(tokio_tungstenite::tungstenite::Error::Http(resp)) => Err(resp.status().as_u16()),
        Err(e) => panic!("ws: {e}"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn host_and_origin_are_checked_on_this_pc() {
    let data = temp_dir("local");
    let server = Running::start(&data, |_| {}).await;
    let to = server.bound;
    let port = server.port();
    let host = format!("127.0.0.1:{port}");
    let page = format!("http://127.0.0.1:{port}");

    // --- Host (DNS rebinding) -------------------------------------------------
    for ok in [host.clone(), format!("localhost:{port}"), "[::1]".into()] {
        let r = raw(to, "GET", "/api/health", &[("Host", &ok)], b"").await;
        assert_eq!(r.status, 200, "{ok}: {}", r.body);
    }
    for bad in [
        format!("evil.example:{port}"),
        "127.0.0.1.nip.io".into(),
        format!("192.168.1.20:{port}"),
    ] {
        for path in ["/api/health", "/", "/ws"] {
            let r = raw(to, "GET", path, &[("Host", &bad)], b"").await;
            assert_eq!(r.status, 403, "{bad} {path}: {}", r.body);
        }
    }
    assert_eq!(ws_open(to, "evil.example", None).await, Err(403));

    // --- Origin -----------------------------------------------------------------
    // The UI of this server (desktop shell, browser, OBS), a dev server or a
    // capture script on another local port, the shell's own pages.
    for origin in [
        page.as_str(),
        "http://127.0.0.1:5173",
        "http://localhost:4173",
        "http://[::1]:5173",
        "tauri://localhost",
        "http://tauri.localhost",
    ] {
        let r = raw(
            to,
            "GET",
            "/api/sessions",
            &[("Host", &host), ("Origin", origin)],
            b"",
        )
        .await;
        assert_eq!(r.status, 200, "{origin}: {}", r.body);
        assert!(
            r.head
                .contains(&format!("access-control-allow-origin: {origin}")),
            "{origin}: {}",
            r.head
        );
        assert_eq!(ws_open(to, &host, Some(origin)).await, Ok(()), "{origin}");
    }
    // Any other web site: refused, reads included, and no CORS header.
    for origin in [
        "https://evil.example",
        "http://192.168.1.20:8471",
        "null",
        "https://localhost:8471",
    ] {
        let r = raw(
            to,
            "GET",
            "/api/sessions",
            &[("Host", &host), ("Origin", origin)],
            b"",
        )
        .await;
        assert_eq!(r.status, 403, "{origin}: {}", r.body);
        assert!(
            !r.head.contains("access-control-allow-origin"),
            "{}",
            r.head
        );
        let r = raw(
            to,
            "POST",
            "/api/recording/start",
            &[("Host", &host), ("Origin", origin)],
            b"",
        )
        .await;
        assert_eq!(r.status, 403, "{origin}");
        let pre = raw(
            to,
            "OPTIONS",
            "/api/layouts",
            &[
                ("Host", &host),
                ("Origin", origin),
                ("Access-Control-Request-Method", "PUT"),
            ],
            b"",
        )
        .await;
        assert_eq!(pre.status, 403, "preflight {origin}");
        assert_eq!(ws_open(to, &host, Some(origin)).await, Err(403), "{origin}");
    }
    // An allowed preflight.
    let pre = raw(
        to,
        "OPTIONS",
        "/api/layouts",
        &[
            ("Host", &host),
            ("Origin", "http://127.0.0.1:5173"),
            ("Access-Control-Request-Method", "PUT"),
            ("Access-Control-Request-Headers", "content-type"),
        ],
        b"",
    )
    .await;
    assert_eq!(pre.status, 200, "{}", pre.head);
    assert!(pre
        .head
        .contains("access-control-allow-origin: http://127.0.0.1:5173"));
    // No Origin (curl, a direct visit): accepted.
    let r = raw(to, "GET", "/api/health", &[("Host", &host)], b"").await;
    assert_eq!(r.status, 200);
    assert!(!r.head.contains("access-control-allow-origin"));
    assert_eq!(ws_open(to, &host, None).await, Ok(()));

    // --- routes kept for this PC: open to this PC -------------------------------
    let r = raw(
        to,
        "DELETE",
        "/api/sessions/20260101-000000-nowhere",
        &[("Host", &host), ("Origin", &page)],
        b"",
    )
    .await;
    assert_eq!(r.status, 404, "{}", r.body);

    // --- body size ---------------------------------------------------------------
    let big = vec![b' '; 2 * 1024 * 1024];
    let r = raw(to, "PUT", "/api/layouts", &[("Host", &host)], &big).await;
    assert_eq!(r.status, 413, "{}", r.body);

    server.stop().await;
    let _ = std::fs::remove_dir_all(&data);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn static_files_stay_inside_the_ui_folder() {
    let root = temp_dir("static");
    let ui = root.join("ui");
    std::fs::create_dir_all(&ui).unwrap();
    std::fs::write(ui.join("index.html"), "<!doctype html>corde-index").unwrap();
    std::fs::write(root.join("secret.txt"), "top-secret").unwrap();
    let data = root.join("data");
    let ui_dir = ui.clone();
    let server = Running::start(&data, move |c| c.ui_dir = Some(ui_dir)).await;
    let host = format!("127.0.0.1:{}", server.port());
    for path in [
        "/../secret.txt",
        "/%2e%2e/secret.txt",
        "/%2E%2E%2Fsecret.txt",
        "/..%2fsecret.txt",
        "/..\\secret.txt",
        "/ui/../../secret.txt",
        "//secret.txt",
    ] {
        let r = raw(server.bound, "GET", path, &[("Host", &host)], b"").await;
        assert!(!r.body.contains("top-secret"), "{path} → {}", r.status);
    }
    let r = raw(server.bound, "GET", "/", &[("Host", &host)], b"").await;
    assert_eq!(r.status, 200);
    assert!(r.body.contains("corde-index"));
    server.stop().await;
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_phone_of_the_lan_keeps_the_dashboard_not_the_pc_only_routes() {
    // Needs an address of this machine other than the loopback: the request
    // must come from "another device" as the server sees it.
    let Some(lan_ip) = corde_server::net::lan_ipv4().first().map(|(_, ip)| *ip) else {
        eprintln!("no LAN address on this machine: test skipped");
        return;
    };
    let data = temp_dir("lan");
    let server = Running::start(&data, |c| c.lan = true).await;
    assert!(server.bound.ip().is_unspecified());
    let to = SocketAddr::new(IpAddr::V4(lan_ip), server.port());
    let host = to.to_string();
    let page = format!("http://{to}");
    let h = [("Host", host.as_str()), ("Origin", page.as_str())];

    // Reads and dashboard writes.
    let r = raw(to, "GET", "/api/sessions", &h, b"").await;
    assert_eq!(r.status, 200, "{}", r.body);
    let r = raw(to, "GET", "/api/network", &h[..1], b"").await;
    assert_eq!(r.status, 200);
    let layout = br#"{"id":"phone","name":"Phone","device":"mobile","columns":4,"widgets":[],"updated_ms":0}"#;
    let r = raw(to, "PUT", "/api/layouts", &h, layout).await;
    assert_eq!(r.status, 200, "{}", r.body);
    let r = raw(to, "POST", "/api/recording/start", &h, b"").await;
    assert_ne!(r.status, 403, "{}", r.body);
    let r = raw(
        to,
        "POST",
        "/api/sessions/x/feedback",
        &h,
        br#"{"text":"ok"}"#,
    )
    .await;
    assert_ne!(r.status, 403, "{}", r.body);
    assert_eq!(ws_open(to, &host, Some(&page)).await, Ok(()));

    // Kept for the PC.
    for (method, path, body) in [
        ("DELETE", "/api/sessions/20260101-000000-nowhere", &b""[..]),
        ("DELETE", "/api/layouts/phone", b""),
        ("DELETE", "/api/layouts", b""),
        (
            "POST",
            "/api/sessions/20260101-000000-nowhere/analysis/recompute",
            b"",
        ),
        ("POST", "/api/profile/import", b""),
        (
            "POST",
            "/api/sessions/20260101-000000-nowhere/setup-files",
            br#"{"base":"A/b.svm","plans":["safe"]}"#,
        ),
        ("PUT", "/api/settings", br#"{"lan_access":false}"#),
    ] {
        for headers in [&h[..], &h[..1]] {
            let r = raw(to, method, path, headers, body).await;
            assert_eq!(r.status, 403, "{method} {path}: {}", r.body);
            assert!(r.body.contains("depuis le PC"), "{}", r.body);
        }
    }
    // The layout is still there.
    let r = raw(to, "GET", "/api/layouts/phone", &h[..1], b"").await;
    assert_eq!(r.status, 200);

    // A name nobody gave this machine.
    let r = raw(to, "GET", "/api/health", &[("Host", "evil.example")], b"").await;
    assert_eq!(r.status, 403);

    server.stop().await;
    let _ = std::fs::remove_dir_all(&data);
}
