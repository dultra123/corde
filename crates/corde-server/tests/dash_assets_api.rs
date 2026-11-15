//! The pictures and videos of the race dashboards end to end (ADR 0016, `/api/dash-assets`):
//! sent from this PC, read by type from their first bytes, served with their own type,
//! `nosniff` and byte ranges (a video on an iPhone asks for nothing else), cleaned up when no
//! definition cites them any more.

use corde_server::{Server, ServerConfig, SourceKind};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::oneshot;

fn temp_dir(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "corde-assets-api-{tag}-{}-{}",
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
    async fn start(data: &std::path::Path) -> Self {
        let cfg = ServerConfig {
            port: 0,
            bind: Some(std::net::IpAddr::from([127, 0, 0, 1])),
            source: SourceKind::Sim,
            data_dir: Some(data.to_path_buf()),
            no_record: true,
            no_import: true,
            no_game_api: true,
            ..Default::default()
        };
        let server = Server::bind(cfg).await.expect("bind");
        let bound = server.local_addr();
        let (stop, rx) = oneshot::channel::<()>();
        let task = tokio::spawn(server.run(async move {
            let _ = rx.await;
        }));
        Self { bound, stop, task }
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
    head: String,
    body: Vec<u8>,
}

impl Reply {
    fn header(&self, name: &str) -> Option<String> {
        let name = format!("{}:", name.to_ascii_lowercase());
        self.head
            .lines()
            .find(|l| l.starts_with(&name))
            .map(|l| l[name.len()..].trim().to_string())
    }
    fn json(&self) -> serde_json::Value {
        serde_json::from_slice(&self.body).unwrap_or(serde_json::Value::Null)
    }
}

/// One raw HTTP/1.1 request, binary-safe.
async fn raw(
    to: SocketAddr,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: &[u8],
) -> Reply {
    let mut req = format!("{method} {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n");
    for (k, v) in headers {
        req.push_str(&format!("{k}: {v}\r\n"));
    }
    if !body.is_empty() || matches!(method, "POST" | "PUT") {
        req.push_str(&format!("Content-Length: {}\r\n", body.len()));
    }
    req.push_str("\r\n");
    let mut bytes = req.into_bytes();
    bytes.extend_from_slice(body);
    tokio::task::spawn_blocking(move || {
        let mut s = TcpStream::connect_timeout(&to, Duration::from_secs(5)).expect("connect");
        s.set_read_timeout(Some(Duration::from_secs(30))).unwrap();
        let _ = s.write_all(&bytes);
        let mut out = Vec::new();
        let _ = s.read_to_end(&mut out);
        let split = out
            .windows(4)
            .position(|w| w == b"\r\n\r\n")
            .unwrap_or(out.len());
        let head = String::from_utf8_lossy(&out[..split]).to_ascii_lowercase();
        let body = out.get(split + 4..).unwrap_or_default().to_vec();
        let status = head
            .split_whitespace()
            .nth(1)
            .and_then(|c| c.parse().ok())
            .unwrap_or(0);
        Reply { status, head, body }
    })
    .await
    .unwrap()
}

/// A small PNG made here (1 × 1), varied by `salt` after its end.
fn png(salt: u8) -> Vec<u8> {
    let mut v =
        b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR\0\0\0\x01\0\0\0\x01\x08\x06\0\0\0\x1f\x15\xc4\x89"
            .to_vec();
    v.push(salt);
    v
}

fn mp4(len: usize) -> Vec<u8> {
    let mut v = b"\0\0\0\x14ftypisom\0\0\x02\0isom".to_vec();
    v.extend((0..len.saturating_sub(v.len())).map(|i| (i % 251) as u8));
    v
}

fn dash_citing(id: &str, asset: &str) -> String {
    format!(
        r##"{{"v":1,"id":"{id}","name":"Logo","format":{{"preset":"phone-landscape","width":852,"height":393}},"theme":{{"background":"#000000","panel":"#0d1116","line":"#1f2731","text":"#f2f5f8","dim":"#93a0ae","accent":"#ffb454"}},"pages":[{{"id":"p1","name":"Course","cols":24,"rows":12,"elements":[{{"id":"e1","type":"image","x":0,"y":0,"w":4,"h":3,"props":{{"asset":"{asset}"}}}}]}}],"updated_ms":1}}"##
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn send_read_ranges_and_clean_up() {
    let data = temp_dir("api");
    let server = Running::start(&data).await;
    let to = server.bound;

    // Sent from this PC: stored under the hash of its content.
    let r = raw(to, "POST", "/api/dash-assets", &[], &png(1)).await;
    assert_eq!(r.status, 201, "{}", String::from_utf8_lossy(&r.body));
    let id = r.json()["id"].as_str().unwrap().to_string();
    assert!(id.ends_with(".png") && id.len() == 68, "{id}");
    assert_eq!(r.json()["mime"], "image/png");
    // The announced type does not matter: the first bytes do.
    let r = raw(
        to,
        "POST",
        "/api/dash-assets",
        &[("Content-Type", "image/svg+xml")],
        &png(1),
    )
    .await;
    assert_eq!(
        (r.status, r.json()["id"].as_str()),
        (201, Some(id.as_str()))
    );
    for (body, why) in [
        (
            &b"<svg xmlns='http://www.w3.org/2000/svg' onload='alert(1)'/>"[..],
            "svg",
        ),
        (b"<html><script>alert(1)</script></html>", "html"),
        (b"ID3\x04\0\0\0\0\0\0sound", "mp3"),
        (b"", "empty"),
    ] {
        let r = raw(
            to,
            "POST",
            "/api/dash-assets",
            &[("Content-Type", "image/png")],
            body,
        )
        .await;
        assert_eq!(r.status, 415, "{why}");
        assert_eq!(r.json()["error"], "unsupported_media", "{why}");
    }
    // A picture over 4 MiB.
    let mut big = png(0);
    big.resize(4 * 1024 * 1024 + 1, 0);
    let r = raw(to, "POST", "/api/dash-assets", &[], &big).await;
    assert_eq!(r.status, 413);
    // A video up to 30 MiB passes the size limit of the other routes (1 MiB).
    let video = mp4(3 * 1024 * 1024);
    let r = raw(to, "POST", "/api/dash-assets", &[], &video).await;
    assert_eq!(r.status, 201, "{}", String::from_utf8_lossy(&r.body));
    let vid = r.json()["id"].as_str().unwrap().to_string();
    assert!(vid.ends_with(".mp4"));
    let r = raw(to, "POST", "/api/dash-assets", &[], &mp4(31 * 1024 * 1024)).await;
    assert_eq!(r.status, 413);

    // Read back: its own type, never sniffed, cached for good.
    let r = raw(to, "GET", &format!("/api/dash-assets/{id}"), &[], b"").await;
    assert_eq!(r.status, 200);
    assert_eq!(r.body, png(1));
    assert_eq!(r.header("content-type").as_deref(), Some("image/png"));
    assert_eq!(
        r.header("x-content-type-options").as_deref(),
        Some("nosniff")
    );
    assert_eq!(r.header("accept-ranges").as_deref(), Some("bytes"));
    assert!(r.header("cache-control").unwrap().contains("immutable"));
    assert!(r
        .header("content-security-policy")
        .unwrap()
        .contains("sandbox"));
    let etag = r.header("etag").unwrap();
    let r = raw(
        to,
        "GET",
        &format!("/api/dash-assets/{id}"),
        &[("If-None-Match", &etag)],
        b"",
    )
    .await;
    assert_eq!(r.status, 304);
    assert!(r.body.is_empty());

    // Byte ranges of the video, as an iPhone asks for them.
    let path = format!("/api/dash-assets/{vid}");
    let r = raw(to, "GET", &path, &[("Range", "bytes=0-1")], b"").await;
    assert_eq!(r.status, 206);
    assert_eq!(r.body, &video[0..2]);
    assert_eq!(
        r.header("content-range"),
        Some(format!("bytes 0-1/{}", video.len()))
    );
    assert_eq!(r.header("content-length").as_deref(), Some("2"));
    assert_eq!(r.header("content-type").as_deref(), Some("video/mp4"));
    let r = raw(to, "GET", &path, &[("Range", "bytes=1000-1999")], b"").await;
    assert_eq!((r.status, r.body.as_slice()), (206, &video[1000..2000]));
    let r = raw(to, "GET", &path, &[("Range", "bytes=-16")], b"").await;
    assert_eq!(
        (r.status, r.body.as_slice()),
        (206, &video[video.len() - 16..])
    );
    let r = raw(to, "GET", &path, &[("Range", "bytes=3000000-")], b"").await;
    assert_eq!((r.status, r.body.as_slice()), (206, &video[3_000_000..]));
    let r = raw(
        to,
        "GET",
        &path,
        &[("Range", &format!("bytes={}-", video.len()))],
        b"",
    )
    .await;
    assert_eq!(r.status, 416);
    assert_eq!(
        r.header("content-range"),
        Some(format!("bytes */{}", video.len()))
    );
    let r = raw(to, "HEAD", &path, &[], b"").await;
    assert_eq!(r.status, 200);
    assert_eq!(r.header("content-length"), Some(video.len().to_string()));

    // Unknown, malformed, a path: nothing.
    for bad in [
        format!("/api/dash-assets/{}.png", "0".repeat(64)),
        "/api/dash-assets/..%2F..%2Fsettings.json".to_string(),
        "/api/dash-assets/settings.json".to_string(),
        format!("/api/dash-assets/{}", id.to_uppercase()),
    ] {
        let r = raw(to, "GET", &bad, &[], b"").await;
        assert_eq!(r.status, 404, "{bad}");
    }

    // A definition cites the picture: it cannot be deleted; once no definition cites it, it can.
    let r = raw(
        to,
        "PUT",
        "/api/dashes/logo",
        &[("Content-Type", "application/json")],
        dash_citing("logo", &id).as_bytes(),
    )
    .await;
    assert_eq!(r.status, 200, "{}", String::from_utf8_lossy(&r.body));
    let r = raw(to, "DELETE", &format!("/api/dash-assets/{id}"), &[], b"").await;
    assert_eq!(
        (r.status, r.json()["error"].as_str()),
        (409, Some("media_in_use"))
    );
    // An address in place of an id is refused.
    let r = raw(
        to,
        "PUT",
        "/api/dashes/web",
        &[("Content-Type", "application/json")],
        dash_citing("web", "https://exemple.org/logo.png").as_bytes(),
    )
    .await;
    assert_eq!(r.status, 400);
    let r = raw(to, "DELETE", &format!("/api/dash-assets/{vid}"), &[], b"").await;
    assert_eq!(r.status, 204);
    let r = raw(to, "GET", "/api/dash-assets", &[], b"").await;
    assert_eq!(r.status, 200);
    assert_eq!(r.json()["count"], 1);
    assert_eq!(r.json()["max_count"], 200);

    server.stop().await;

    // At start-up, a media no definition cites and older than a day goes away; a cited one stays.
    let dir = data.join("dash-assets");
    let unused = format!("{}.gif", "ab".repeat(32));
    std::fs::write(dir.join(&unused), b"GIF89a\x01\0\x01\0").unwrap();
    let old = std::time::SystemTime::now() - Duration::from_secs(2 * 24 * 3600);
    for f in [&unused, &id] {
        std::fs::File::options()
            .write(true)
            .open(dir.join(f))
            .unwrap()
            .set_modified(old)
            .unwrap();
    }
    let server = Running::start(&data).await;
    assert!(!dir.join(&unused).exists(), "unused media removed");
    assert!(dir.join(&id).exists(), "cited media kept");
    // The definition deleted: its picture goes at the next clean-up.
    let r = raw(server.bound, "DELETE", "/api/dashes/logo", &[], b"").await;
    assert_eq!(r.status, 204);
    assert!(!dir.join(&id).exists(), "no longer cited");
    server.stop().await;
    let _ = std::fs::remove_dir_all(&data);
}
