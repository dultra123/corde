//! Home server access end to end (`--role hub`, docs/SECURITY-MODEL.md §7): pairing (no
//! code, wrong code ×3, rate limit), the cookie and the bearer token, missing / fake /
//! revoked tokens on HTTP routes and on the WebSocket, the administrator role, routes
//! refused on the server, the local control routes and commands, and the `Host` / `Origin`
//! guard still in front of everything.

use corde_server::hub::cli;
use corde_server::{Role, Server, ServerConfig};
use futures_util::StreamExt;
use serde_json::Value;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::sync::oneshot;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::tungstenite::Message;

fn temp_dir(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "corde-hub-{tag}-{}-{}",
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
    let cfg = ServerConfig {
        role: Role::Hub,
        port: 0,
        bind: Some([127, 0, 0, 1].into()),
        data_dir: Some(data.to_path_buf()),
        ..Default::default()
    };
    let server = Server::bind(cfg).await.expect("bind hub");
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

struct Reply {
    status: u16,
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
            .find(|l| l.to_ascii_lowercase().starts_with(&name))
            .map(|l| l[name.len()..].trim().to_string())
    }
}

/// A raw HTTP/1.1 request (`Host` set to the bound address unless given).
async fn raw(
    to: SocketAddr,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: &str,
) -> Reply {
    let mut req = format!("{method} {path} HTTP/1.1\r\nConnection: close\r\n");
    if !headers.iter().any(|(k, _)| k.eq_ignore_ascii_case("host")) {
        req.push_str(&format!("Host: {to}\r\n"));
    }
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
        s.write_all(&bytes).unwrap();
        let mut out = Vec::new();
        let _ = s.read_to_end(&mut out);
        let text = String::from_utf8_lossy(&out).into_owned();
        let (head, body) = text.split_once("\r\n\r\n").unwrap_or((&text, ""));
        let status = head
            .split_whitespace()
            .nth(1)
            .and_then(|c| c.parse().ok())
            .unwrap_or(0);
        // Chunked bodies: keep the JSON part only.
        let body = match (body.find('{'), body.rfind('}')) {
            (Some(a), Some(b)) if b >= a && head.to_ascii_lowercase().contains("chunked") => {
                body[a..=b].to_string()
            }
            _ => body.to_string(),
        };
        Reply {
            status,
            head: head.to_string(),
            body,
        }
    })
    .await
    .unwrap()
}

async fn code(data: &Path) -> String {
    let data = data.to_path_buf();
    tokio::task::spawn_blocking(move || cli::pair(&data, false).expect("pair command").code)
        .await
        .unwrap()
}

/// Opens the WebSocket with these headers; `Ok(stream)` = upgraded.
async fn ws(
    to: SocketAddr,
    headers: &[(&str, &str)],
) -> Result<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    u16,
> {
    let mut req = format!("ws://{to}/ws?rate=1")
        .into_client_request()
        .unwrap();
    for (k, v) in headers {
        req.headers_mut().insert(
            tokio_tungstenite::tungstenite::http::HeaderName::from_bytes(k.as_bytes()).unwrap(),
            HeaderValue::from_str(v).unwrap(),
        );
    }
    match tokio_tungstenite::connect_async(req).await {
        Ok((s, _)) => Ok(s),
        Err(tokio_tungstenite::tungstenite::Error::Http(resp)) => Err(resp.status().as_u16()),
        Err(e) => panic!("ws: {e}"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pairing_cookie_tokens_admin_and_revocation() {
    let data = temp_dir("access");
    let hub = start_hub(&data).await;
    let to = hub.bound;

    // Nothing but the pairing state answers an unpaired device; the UI files are served.
    let r = raw(to, "GET", "/api/health", &[], "").await;
    assert_eq!(r.status, 401, "{}", r.body);
    assert_eq!(r.json()["code"], "unpaired");
    for path in [
        "/api/sessions",
        "/api/profile",
        "/api/hub/devices",
        "/api/nope",
    ] {
        assert_eq!(raw(to, "GET", path, &[], "").await.status, 401, "{path}");
    }
    assert_eq!(
        raw(to, "GET", "/", &[], "").await.status,
        200,
        "UI / landing page"
    );
    let r = raw(to, "GET", "/api/hub/pairing", &[], "").await;
    assert_eq!(r.status, 200);
    assert_eq!(r.json()["role"], "hub");
    assert_eq!(r.json()["paired"], false);
    assert_eq!(
        ws(to, &[]).await.err(),
        Some(401),
        "WebSocket without a token"
    );

    // The local control routes: loopback, no Origin, the key of the control file.
    let r = raw(to, "POST", "/api/hub/local/pairing-code", &[], "{}").await;
    assert_eq!(r.status, 403);
    let r = raw(
        to,
        "POST",
        "/api/hub/local/pairing-code",
        &[("Authorization", &format!("Bearer {}", "0".repeat(64)))],
        "{}",
    )
    .await;
    assert_eq!(r.status, 403, "wrong control key");

    // No code issued yet.
    let r = raw(to, "POST", "/api/hub/pair", &[], r#"{"code":"12345678"}"#).await;
    assert_eq!(r.status, 400);
    assert_eq!(r.json()["code"], "no_code");

    // A browser pairs: cookie HttpOnly, SameSite=Strict, Path=/, not Secure; no token in body.
    let c = code(&data).await;
    assert_eq!(c.len(), 8);
    let r = raw(
        to,
        "POST",
        "/api/hub/pair",
        &[],
        &format!(r#"{{"code":"{} {}","name":"Téléphone"}}"#, &c[..4], &c[4..]),
    )
    .await;
    assert_eq!(r.status, 201, "{}", r.body);
    assert!(r.json().get("token").is_none(), "{}", r.body);
    assert_eq!(r.json()["device"]["admin"], true, "first device");
    let set_cookie = r.header("set-cookie").expect("cookie");
    for part in ["HttpOnly", "SameSite=Strict", "Path=/"] {
        assert!(set_cookie.contains(part), "{set_cookie}");
    }
    assert!(!set_cookie.contains("Secure"), "{set_cookie}");
    let token = set_cookie
        .split(';')
        .next()
        .unwrap()
        .trim()
        .strip_prefix("corde_device=")
        .unwrap()
        .to_string();
    assert_eq!(token.len(), 64);
    let cookie = format!("corde_device={token}");
    let phone = [("Cookie", cookie.as_str())];
    let r = raw(to, "GET", "/api/health", &phone, "").await;
    assert_eq!(r.status, 200, "{}", r.body);
    assert_eq!(r.json()["role"], "hub");
    let r = raw(to, "GET", "/api/hub/pairing", &phone, "").await;
    assert_eq!(r.json()["paired"], true);
    assert_eq!(r.json()["device"]["admin"], true);
    // The token never shows in the devices list.
    let r = raw(to, "GET", "/api/hub/devices", &phone, "").await;
    assert_eq!(r.status, 200);
    assert!(!r.body.contains(&token));
    assert_eq!(r.json()["devices"][0]["current"], true);
    // Not in the files either.
    let stored = std::fs::read_to_string(data.join("hub").join("devices.json")).unwrap();
    assert!(!stored.contains(&token));

    // The PC pairs as an app: token in the body, no cookie, not administrator.
    let c = code(&data).await;
    let r = raw(
        to,
        "POST",
        "/api/hub/pair",
        &[],
        &format!(r#"{{"code":"{c}","name":"PC de jeu","kind":"app"}}"#),
    )
    .await;
    assert_eq!(r.status, 201, "{}", r.body);
    assert!(r.header("set-cookie").is_none());
    let pc_token = r.json()["token"].as_str().unwrap().to_string();
    let pc_id = r.json()["device"]["id"].as_str().unwrap().to_string();
    assert_eq!(r.json()["device"]["admin"], false);
    let bearer = format!("Bearer {pc_token}");
    let pc = [("Authorization", bearer.as_str())];
    assert_eq!(raw(to, "GET", "/api/sessions", &pc, "").await.status, 200);

    // A fake token, a token from the wrong place.
    let fake = format!("corde_device={}", "a".repeat(64));
    assert_eq!(
        raw(to, "GET", "/api/sessions", &[("Cookie", &fake)], "")
            .await
            .status,
        401
    );
    let wrong = format!("Bearer {}", "a".repeat(64));
    assert_eq!(
        raw(to, "GET", "/api/sessions", &[("Authorization", &wrong)], "")
            .await
            .status,
        401
    );
    assert_eq!(ws(to, &[("Cookie", &fake)]).await.err(), Some(401));

    // Administrator only.
    let r = raw(to, "DELETE", "/api/sessions/nope-123", &pc, "").await;
    assert_eq!(r.status, 403);
    assert_eq!(r.json()["code"], "admin_only");
    let r = raw(to, "DELETE", "/api/sessions/nope-123", &phone, "").await;
    assert_eq!(r.status, 404, "the administrator gets through: {}", r.body);
    let r = raw(
        to,
        "PUT",
        "/api/settings",
        &pc,
        r#"{"online_schedule":true}"#,
    )
    .await;
    assert_eq!(r.status, 403);
    let r = raw(
        to,
        "PUT",
        "/api/settings",
        &phone,
        r#"{"online_schedule":true}"#,
    )
    .await;
    assert_eq!(r.status, 200, "{}", r.body);
    let r = raw(to, "PUT", "/api/settings", &phone, r#"{"lan_access":true}"#).await;
    assert_eq!(r.status, 400, "no LAN switch on the server");
    // Never on the server.
    for (m, p) in [
        ("POST", "/api/profile/import"),
        ("POST", "/api/recording/start"),
        ("POST", "/api/sessions/x/setup-files"),
    ] {
        let r = raw(to, m, p, &phone, "{}").await;
        assert_eq!(r.status, 403, "{m} {p}");
        assert_eq!(r.json()["code"], "not_on_server", "{m} {p}");
    }
    let r = raw(to, "GET", "/api/profile", &phone, "").await;
    assert_eq!(r.status, 200, "profile without importer: {}", r.body);

    // Devices: rename self; a non-administrator cannot touch another device.
    let r = raw(
        to,
        "PATCH",
        &format!("/api/hub/devices/{pc_id}"),
        &pc,
        r#"{"name":"PC salon"}"#,
    )
    .await;
    assert_eq!(r.status, 200, "{}", r.body);
    assert_eq!(r.json()["name"], "PC salon");
    let phone_id = raw(to, "GET", "/api/hub/pairing", &phone, "").await.json()["device"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let r = raw(
        to,
        "PATCH",
        &format!("/api/hub/devices/{phone_id}"),
        &pc,
        r#"{"name":"x"}"#,
    )
    .await;
    assert_eq!(r.status, 403);
    let r = raw(
        to,
        "PATCH",
        &format!("/api/hub/devices/{pc_id}"),
        &pc,
        r#"{"admin":true}"#,
    )
    .await;
    assert_eq!(r.status, 403, "no self-promotion");
    let r = raw(
        to,
        "PATCH",
        &format!("/api/hub/devices/{phone_id}"),
        &phone,
        r#"{"admin":false}"#,
    )
    .await;
    assert_eq!(r.status, 409, "the last administrator stays");
    let r = raw(
        to,
        "DELETE",
        &format!("/api/hub/devices/{phone_id}"),
        &pc,
        "",
    )
    .await;
    assert_eq!(r.status, 403);

    // The guard is still in front: Host and Origin, even with a valid token.
    let r = raw(
        to,
        "GET",
        "/api/sessions",
        &[("Host", "evil.example:8471"), phone[0]],
        "",
    )
    .await;
    assert_eq!(r.status, 403);
    let r = raw(
        to,
        "GET",
        "/api/sessions",
        &[phone[0], ("Origin", "https://evil.example")],
        "",
    )
    .await;
    assert_eq!(r.status, 403);
    assert_eq!(
        ws(to, &[phone[0], ("Origin", "https://evil.example")])
            .await
            .err(),
        Some(403)
    );

    // WebSocket with the cookie, then the device is revoked: the socket closes at once.
    let mut socket = ws(to, &phone).await.expect("ws with cookie");
    let first = tokio::time::timeout(Duration::from_secs(5), socket.next())
        .await
        .unwrap();
    assert!(
        matches!(first, Some(Ok(Message::Text(_)))),
        "hello: {first:?}"
    );
    let mut pc_socket = ws(to, &pc).await.expect("ws with bearer");
    let data2 = data.clone();
    let phone_id2 = phone_id.clone();
    let listed = tokio::task::spawn_blocking(move || {
        let list = cli::devices(&data2).unwrap();
        let removed = cli::revoke(&data2, &phone_id2).unwrap();
        (list, removed)
    })
    .await
    .unwrap();
    assert_eq!(listed.0.len(), 2);
    assert!(listed.1);
    let closed = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match socket.next().await {
                Some(Ok(Message::Close(_))) | None | Some(Err(_)) => return true,
                Some(Ok(_)) => continue,
            }
        }
    })
    .await
    .expect("socket closed within 5 s");
    assert!(closed);
    assert_eq!(
        raw(to, "GET", "/api/sessions", &phone, "").await.status,
        401
    );
    // The other device's socket stays open.
    let still = tokio::time::timeout(Duration::from_secs(7), pc_socket.next()).await;
    assert!(matches!(still, Ok(Some(Ok(Message::Text(_))))), "{still:?}");
    // A device revokes itself: the cookie / token stops working.
    let r = raw(to, "DELETE", &format!("/api/hub/devices/{pc_id}"), &pc, "").await;
    assert_eq!(r.status, 204);
    assert_eq!(raw(to, "GET", "/api/sessions", &pc, "").await.status, 401);

    hub.stop().await;
    let _ = std::fs::remove_dir_all(&data);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn three_wrong_codes_and_the_rate_limit() {
    let data = temp_dir("wrong");
    let hub = start_hub(&data).await;
    let to = hub.bound;
    let c = code(&data).await;
    let wrong = if c == "00000000" {
        "11111111"
    } else {
        "00000000"
    };
    for left in [2, 1] {
        let r = raw(
            to,
            "POST",
            "/api/hub/pair",
            &[],
            &format!(r#"{{"code":"{wrong}"}}"#),
        )
        .await;
        assert_eq!(r.status, 400);
        assert_eq!(r.json()["code"], "wrong_code");
        assert!(r.body.contains(&format!("Encore {left}")), "{}", r.body);
        assert!(!r.body.contains(&c), "the code is never repeated");
    }
    let r = raw(
        to,
        "POST",
        "/api/hub/pair",
        &[],
        &format!(r#"{{"code":"{wrong}"}}"#),
    )
    .await;
    assert_eq!(r.json()["code"], "code_invalidated");
    // The right code is dead now.
    let r = raw(
        to,
        "POST",
        "/api/hub/pair",
        &[],
        &format!(r#"{{"code":"{c}"}}"#),
    )
    .await;
    assert_eq!(r.json()["code"], "no_code");
    // Fifth attempt from this address in the window… then refused, even with a fresh code.
    let r = raw(to, "POST", "/api/hub/pair", &[], r#"{"code":"1"}"#).await;
    assert_eq!(r.json()["code"], "no_code");
    let c = code(&data).await;
    let r = raw(
        to,
        "POST",
        "/api/hub/pair",
        &[],
        &format!(r#"{{"code":"{c}"}}"#),
    )
    .await;
    assert_eq!(r.status, 429, "{}", r.body);
    assert_eq!(r.json()["code"], "rate_limited");
    assert!(r.header("retry-after").is_some());
    // The log never holds a code: checked on the issued one's absence from the files too.
    for entry in std::fs::read_dir(data.join("hub")).unwrap() {
        let p = entry.unwrap().path();
        let text = std::fs::read_to_string(&p).unwrap_or_default();
        assert!(!text.contains(&c), "{}", p.display());
    }
    hub.stop().await;
    let _ = std::fs::remove_dir_all(&data);
}
