//! The iPhone application (`apps/mobile`, Capacitor) end to end, docs/SECURITY-MODEL.md §8:
//! its pages come from `capacitor://localhost`, so the server must accept that origin on the
//! API, in CORS (with `Authorization`) and on the WebSocket — on the PC with the network access
//! open, and on the home server with a device token (`Authorization: Bearer` on HTTP, a
//! `corde.token.<token>` subprotocol entry on the WebSocket, never echoed back). The routes
//! kept for the PC stay closed to it.

use corde_server::hub::cli;
use corde_server::{Role, Server, ServerConfig, SourceKind};
use serde_json::Value;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::oneshot;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::HeaderValue;

const APP: &str = "capacitor://localhost";

fn temp_dir(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "corde-mobile-{tag}-{}-{}",
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
    async fn start(cfg: ServerConfig) -> Self {
        let server = Server::bind(cfg).await.expect("bind");
        let bound = server.local_addr();
        let (stop, rx) = oneshot::channel::<()>();
        let task = tokio::spawn(server.run(async move {
            let _ = rx.await;
        }));
        Self { bound, stop, task }
    }

    /// The loopback address of the server (it may listen on every interface).
    fn to(&self) -> SocketAddr {
        SocketAddr::from(([127, 0, 0, 1], self.bound.port()))
    }

    async fn stop(self) {
        let _ = self.stop.send(());
        tokio::time::timeout(Duration::from_secs(10), self.task)
            .await
            .expect("server stops")
            .expect("join")
            .expect("run ok");
    }
}

struct Reply {
    status: u16,
    /// Lower case.
    head: String,
    body: String,
}

impl Reply {
    fn json(&self) -> Value {
        serde_json::from_str(&self.body).unwrap_or(Value::Null)
    }
    fn header(&self, name: &str) -> Option<String> {
        let name = format!("{}:", name.to_ascii_lowercase());
        self.head
            .lines()
            .find(|l| l.starts_with(&name))
            .map(|l| l[name.len()..].trim().to_string())
    }
}

/// One raw HTTP/1.1 request, `Host` as given.
async fn raw(
    to: SocketAddr,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: &str,
) -> Reply {
    let mut req = format!("{method} {path} HTTP/1.1\r\nConnection: close\r\n");
    for (k, v) in headers {
        req.push_str(&format!("{k}: {v}\r\n"));
    }
    if !body.is_empty() || matches!(method, "POST" | "PUT" | "PATCH") {
        req.push_str(&format!(
            "Content-Type: application/json\r\nContent-Length: {}\r\n",
            body.len()
        ));
    }
    req.push_str("\r\n");
    req.push_str(body);
    let bytes = req.into_bytes();
    tokio::task::spawn_blocking(move || {
        let mut s = TcpStream::connect_timeout(&to, Duration::from_secs(5)).expect("connect");
        s.set_read_timeout(Some(Duration::from_secs(30))).unwrap();
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
        let body = match (body.find('{'), body.rfind('}')) {
            (Some(a), Some(b)) if b >= a && head.to_ascii_lowercase().contains("chunked") => {
                body[a..=b].to_string()
            }
            _ => body.to_string(),
        };
        Reply {
            status,
            head: head.to_ascii_lowercase(),
            body,
        }
    })
    .await
    .unwrap()
}

/// Opens the WebSocket as the application does: `Host`, `Origin`, and the subprotocols it
/// asks for (`None`: none). `Ok(subprotocol chosen by the server)` when upgraded.
async fn ws_open(
    to: SocketAddr,
    host: &str,
    path: &str,
    protocols: Option<&str>,
) -> Result<Option<String>, u16> {
    let mut req = format!("ws://{to}{path}").into_client_request().unwrap();
    let h = req.headers_mut();
    h.insert("host", HeaderValue::from_str(host).unwrap());
    h.insert("origin", HeaderValue::from_static(APP));
    if let Some(p) = protocols {
        h.insert("sec-websocket-protocol", HeaderValue::from_str(p).unwrap());
    }
    match tokio_tungstenite::connect_async(req).await {
        Ok((_, resp)) => Ok(resp
            .headers()
            .get("sec-websocket-protocol")
            .map(|v| v.to_str().unwrap().to_string())),
        Err(tokio_tungstenite::tungstenite::Error::Http(resp)) => Err(resp.status().as_u16()),
        Err(e) => panic!("ws: {e}"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_application_reaches_a_pc_with_the_network_access_open() {
    let data = temp_dir("pc");
    let server = Running::start(ServerConfig {
        port: 0,
        source: SourceKind::Sim,
        data_dir: Some(data.clone()),
        no_record: true,
        no_import: true,
        no_game_api: true,
        lan: true,
        ..Default::default()
    })
    .await;
    let to = server.to();
    // The phone types the PC's address: `Host` is that IP literal (any one passes with the
    // network access open). The connection itself comes from the loopback here, which is
    // the strictest case for the PC-only routes: only the origin keeps them closed.
    let host = format!("192.168.1.20:{}", server.bound.port());
    let h = [("Host", host.as_str()), ("Origin", APP)];

    // `/api/health` tells the application which kind of server it reached.
    let r = raw(to, "GET", "/api/health", &h, "").await;
    assert_eq!(r.status, 200, "{}", r.body);
    assert_eq!(
        r.header("access-control-allow-origin").as_deref(),
        Some(APP)
    );
    assert_eq!(r.json()["role"], "desktop");
    let r = raw(to, "GET", "/api/sessions", &h, "").await;
    assert_eq!(r.status, 200, "{}", r.body);
    let layout = r#"{"id":"phone","name":"iPhone","device":"mobile","columns":4,"widgets":[],"updated_ms":0}"#;
    let r = raw(to, "PUT", "/api/layouts", &h, layout).await;
    assert_eq!(r.status, 200, "{}", r.body);

    // Preflight of a JSON request carrying a token.
    let pre = [
        ("Host", host.as_str()),
        ("Origin", APP),
        ("Access-Control-Request-Method", "PUT"),
        (
            "Access-Control-Request-Headers",
            "authorization, content-type",
        ),
    ];
    let r = raw(to, "OPTIONS", "/api/layouts", &pre, "").await;
    assert!((200..300).contains(&r.status), "{} {}", r.status, r.head);
    assert_eq!(
        r.header("access-control-allow-origin").as_deref(),
        Some(APP)
    );
    assert!(r
        .header("access-control-allow-headers")
        .unwrap_or_default()
        .contains("authorization"));

    // WebSocket: without subprotocol, and asking for `corde.v1` (echoed).
    assert_eq!(ws_open(to, &host, "/ws", None).await, Ok(None));
    assert_eq!(
        ws_open(to, &host, "/ws", Some("corde.v1")).await,
        Ok(Some("corde.v1".to_string()))
    );

    // The routes kept for the PC stay closed, even over the loopback.
    for (method, path, body) in [
        ("PUT", "/api/settings", r#"{"lan_access":false}"#),
        ("DELETE", "/api/sessions/20260101-000000-nowhere", ""),
        ("DELETE", "/api/layouts/phone", ""),
        ("POST", "/api/profile/import", ""),
        ("POST", "/api/hub-link/sync", ""),
    ] {
        let r = raw(to, method, path, &h, body).await;
        assert_eq!(r.status, 403, "{method} {path}: {}", r.body);
    }

    // A browser page of another site is still refused, and so are look-alike origins.
    for bad in [
        "https://evil.example",
        "capacitor://evil.example",
        "ionic://localhost",
    ] {
        let r = raw(
            to,
            "GET",
            "/api/health",
            &[("Host", host.as_str()), ("Origin", bad)],
            "",
        )
        .await;
        assert_eq!(r.status, 403, "{bad}");
    }

    server.stop().await;
    let _ = std::fs::remove_dir_all(&data);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn with_the_network_access_closed_the_phone_is_refused() {
    let data = temp_dir("closed");
    let server = Running::start(ServerConfig {
        port: 0,
        source: SourceKind::Sim,
        data_dir: Some(data.clone()),
        no_record: true,
        no_import: true,
        no_game_api: true,
        ..Default::default()
    })
    .await;
    let host = format!("192.168.1.20:{}", server.bound.port());
    let r = raw(
        server.to(),
        "GET",
        "/api/health",
        &[("Host", host.as_str()), ("Origin", APP)],
        "",
    )
    .await;
    assert_eq!(r.status, 403, "Host refused: {}", r.body);
    server.stop().await;
    let _ = std::fs::remove_dir_all(&data);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_application_pairs_with_the_home_server_and_uses_its_token() {
    let data = temp_dir("hub");
    let server = Running::start(ServerConfig {
        role: Role::Hub,
        port: 0,
        bind: Some([127, 0, 0, 1].into()),
        data_dir: Some(data.clone()),
        ..Default::default()
    })
    .await;
    let to = server.to();
    let host = to.to_string();
    let h = [("Host", host.as_str()), ("Origin", APP)];

    // Unpaired: the pairing state only.
    let r = raw(to, "GET", "/api/hub/pairing", &h, "").await;
    assert_eq!(r.status, 200);
    assert_eq!(r.json()["paired"], false);
    assert_eq!(
        r.header("access-control-allow-origin").as_deref(),
        Some(APP)
    );
    assert_eq!(raw(to, "GET", "/api/health", &h, "").await.status, 401);
    assert_eq!(
        ws_open(to, &host, "/ws", Some("corde.v1")).await,
        Err(401),
        "no token"
    );

    // Pairing as an application: the token comes in the body, no cookie.
    let data2 = data.clone();
    let code = tokio::task::spawn_blocking(move || cli::pair(&data2, false).unwrap().code)
        .await
        .unwrap();
    let r = raw(
        to,
        "POST",
        "/api/hub/pair",
        &h,
        &format!(r#"{{"code":"{code}","name":"iPhone","kind":"app"}}"#),
    )
    .await;
    assert_eq!(r.status, 201, "{}", r.body);
    assert!(r.header("set-cookie").is_none(), "{}", r.head);
    let token = r.json()["token"].as_str().unwrap().to_string();
    assert_eq!(token.len(), 64);

    // HTTP: `Authorization: Bearer`, CORS answered to the application.
    let bearer = format!("Bearer {token}");
    let auth = [
        ("Host", host.as_str()),
        ("Origin", APP),
        ("Authorization", bearer.as_str()),
    ];
    let r = raw(to, "GET", "/api/health", &auth, "").await;
    assert_eq!(r.status, 200, "{}", r.body);
    assert_eq!(
        r.header("access-control-allow-origin").as_deref(),
        Some(APP)
    );
    let r = raw(to, "GET", "/api/hub/pairing", &auth, "").await;
    assert_eq!(r.json()["paired"], true);
    assert_eq!(r.json()["device"]["name"], "iPhone");
    // The preflight is answered before the device check (no token on a preflight).
    let pre = [
        ("Host", host.as_str()),
        ("Origin", APP),
        ("Access-Control-Request-Method", "GET"),
        ("Access-Control-Request-Headers", "authorization"),
    ];
    let r = raw(to, "OPTIONS", "/api/sessions", &pre, "").await;
    assert!((200..300).contains(&r.status), "{} {}", r.status, r.head);

    // WebSocket: the token as a subprotocol entry; the answer names `corde.v1` only.
    let protocols = format!("corde.v1, corde.token.{token}");
    assert_eq!(
        ws_open(to, &host, "/ws", Some(&protocols)).await,
        Ok(Some("corde.v1".to_string()))
    );
    let wrong = format!("corde.v1, corde.token.{}", "0".repeat(64));
    assert_eq!(ws_open(to, &host, "/ws", Some(&wrong)).await, Err(401));
    // A token in the address is not a way in (it would end up in request logs).
    assert_eq!(
        ws_open(to, &host, &format!("/ws?token={token}"), Some("corde.v1")).await,
        Err(401)
    );

    // Revoked: refused at once, on both ways.
    let r = raw(to, "GET", "/api/hub/devices", &auth, "").await;
    let id = r.json()["devices"][0]["id"].as_str().unwrap().to_string();
    let r = raw(to, "DELETE", &format!("/api/hub/devices/{id}"), &auth, "").await;
    assert_eq!(r.status, 204, "{}", r.body);
    assert_eq!(raw(to, "GET", "/api/health", &auth, "").await.status, 401);
    assert_eq!(ws_open(to, &host, "/ws", Some(&protocols)).await, Err(401));

    server.stop().await;
    let _ = std::fs::remove_dir_all(&data);
}
