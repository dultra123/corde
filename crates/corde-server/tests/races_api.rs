//! The « Courses » routes end to end, on a real server: nothing is fetched
//! while `online_schedule` is off, the pages come back as text once it is on
//! (a stand-in replaces the web: this test never leaves the machine), picks
//! are validated and shared, an unreadable page is kept for diagnosis, and a
//! due reminder reaches the WebSocket.

use corde_server::races::sources::SourceDef;
use corde_server::{Server, ServerConfig, SourceKind};
use futures_util::StreamExt;
use serde_json::{json, Value};
use std::net::IpAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::oneshot;
use tokio_tungstenite::tungstenite::Message;

async fn call(method: &'static str, url: String, body: Option<Value>) -> (u16, Value) {
    tokio::task::spawn_blocking(move || {
        let agent: ureq::Agent = ureq::config::Config::builder()
            .http_status_as_error(false)
            .proxy(None)
            .build()
            .new_agent();
        let mut resp = match (method, body) {
            ("GET", _) => agent.get(&url).call(),
            ("DELETE", _) => agent.delete(&url).call(),
            ("POST", _) => agent.post(&url).send_empty(),
            ("PUT", Some(b)) => agent.put(&url).send_json(&b),
            _ => unreachable!(),
        }
        .expect("request");
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

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn races_routes_end_to_end() {
    let data_dir = std::env::temp_dir().join(format!(
        "corde-races-api-{}-{}",
        std::process::id(),
        now_ms()
    ));
    let cfg = ServerConfig {
        port: 0,
        bind: Some(IpAddr::from([127, 0, 0, 1])),
        source: SourceKind::Sim,
        data_dir: Some(data_dir.clone()),
        no_record: true,
        no_import: true,
        no_game_api: true,
        rate: 1,
        ..Default::default()
    };
    let server = Server::bind(cfg).await.expect("bind");
    let calls = Arc::new(AtomicUsize::new(0));
    let c = Arc::clone(&calls);
    server
        .state()
        .races
        .sources
        .set_fetcher(Arc::new(move |def: &SourceDef| {
            c.fetch_add(1, Ordering::SeqCst);
            Ok(format!(
                "<html><body>{} — maintenance</body></html>",
                def.id
            ))
        }));
    let base = format!("http://{}", server.local_addr());
    let ws_url = format!("ws://{}/ws", server.local_addr());
    let (stop, stop_rx) = oneshot::channel::<()>();
    let task = tokio::spawn(server.run(async move {
        let _ = stop_rx.await;
    }));

    // --- off by default: nothing fetched, no page --------------------------
    let (code, body) = call("GET", format!("{base}/api/races/sources?force=1"), None).await;
    assert_eq!(code, 200, "{body}");
    assert_eq!(body["enabled"], false);
    let sources = body["sources"].as_array().unwrap();
    assert_eq!(sources.len(), 2);
    for s in sources {
        assert_eq!(s["state"], "disabled");
        assert!(s["html"].is_null());
    }
    assert_eq!(sources[0]["url"], "https://racecontrol.gg/");
    assert_eq!(sources[1]["url"], "https://lmu.tools/schedule");
    assert_eq!(calls.load(Ordering::SeqCst), 0);

    // --- on: the pages as text, then from the cache -------------------------
    let (code, _) = call(
        "PUT",
        format!("{base}/api/settings"),
        Some(json!({ "online_schedule": true })),
    )
    .await;
    assert_eq!(code, 200);
    let (code, body) = call("GET", format!("{base}/api/races/sources"), None).await;
    assert_eq!(code, 200);
    assert_eq!(body["enabled"], true);
    assert_eq!(body["sources"][0]["state"], "ok");
    assert_eq!(
        body["sources"][0]["html"],
        "<html><body>racecontrol — maintenance</body></html>"
    );
    assert!(body["sources"][1]["fetched_at_ms"].as_i64().unwrap() > 0);
    call("GET", format!("{base}/api/races/sources?force=1"), None).await;
    assert_eq!(
        calls.load(Ordering::SeqCst),
        2,
        "cached; forced reloads wait 60 s"
    );

    // --- an unreadable page is kept once -------------------------------------
    let (code, body) = call(
        "POST",
        format!("{base}/api/races/sources/racecontrol/unreadable"),
        None,
    )
    .await;
    assert_eq!(code, 200, "{body}");
    assert_eq!(body["kept"], true);
    let kept = data_dir
        .join("diagnostics")
        .join("races")
        .join("racecontrol.html");
    assert!(std::fs::read_to_string(&kept)
        .unwrap()
        .contains("maintenance"));
    let (_, body) = call(
        "POST",
        format!("{base}/api/races/sources/racecontrol/unreadable"),
        None,
    )
    .await;
    assert_eq!(body["kept"], false, "same copy: not written again");
    let (code, _) = call(
        "POST",
        format!("{base}/api/races/sources/raceos/unreadable"),
        None,
    )
    .await;
    assert_eq!(code, 404);

    // --- settings -------------------------------------------------------------
    let (code, body) = call("GET", format!("{base}/api/races/settings"), None).await;
    assert_eq!(code, 200);
    assert_eq!(body["leads"], json!([15, 5]));
    let (code, body) = call(
        "PUT",
        format!("{base}/api/races/settings"),
        Some(json!({ "leads": [5], "profile": { "sr": "S1", "dr": "B2" } })),
    )
    .await;
    assert_eq!(code, 200, "{body}");
    assert_eq!(body["leads"], json!([5]));
    let (code, _) = call(
        "PUT",
        format!("{base}/api/races/settings"),
        Some(json!({ "leads": [7] })),
    )
    .await;
    assert_eq!(code, 400);

    // --- picks ------------------------------------------------------------------
    // Due in about two seconds (5 min before the start).
    let start = now_ms() + 5 * 60_000 + 2_000;
    let id = format!("lmp3-fixed-beginner@{start}");
    let pick = json!({
        "id": id, "race_id": "lmp3-fixed-beginner", "cat": "daily", "src": "rc",
        "name": "LMP3 Fixed", "level": "beginner", "track": "Monza", "duration": "32 min",
        "image": null, "start_ms": start,
    });
    let (mut ws, _) = tokio_tungstenite::connect_async(&ws_url).await.expect("ws");
    let (code, body) = call(
        "PUT",
        format!("{base}/api/races/picks/{id}"),
        Some(pick.clone()),
    )
    .await;
    assert_eq!(code, 201, "{body}");
    let (code, _) = call(
        "PUT",
        format!("{base}/api/races/picks/{id}"),
        Some(pick.clone()),
    )
    .await;
    assert_eq!(code, 200, "same pick: replaced");
    let mut bad = pick.clone();
    bad["image"] = json!("http://cdn.example/a.png");
    let (code, _) = call("PUT", format!("{base}/api/races/picks/{id}"), Some(bad)).await;
    assert_eq!(code, 400);
    let (code, _) = call(
        "PUT",
        format!("{base}/api/races/picks/other@{start}"),
        Some(pick.clone()),
    )
    .await;
    assert_eq!(code, 400, "path and body must agree");
    let (code, body) = call("GET", format!("{base}/api/races/picks"), None).await;
    assert_eq!(code, 200);
    assert_eq!(body.as_array().unwrap().len(), 1);
    assert_eq!(body[0]["name"], "LMP3 Fixed");

    // --- the reminder reaches the WebSocket --------------------------------------
    let deadline = Instant::now() + Duration::from_secs(20);
    let reminder = loop {
        let left = deadline.saturating_duration_since(Instant::now());
        assert!(!left.is_zero(), "no race_reminder within 20 s");
        match tokio::time::timeout(left, ws.next()).await {
            Ok(Some(Ok(Message::Text(t)))) => {
                let v: Value = serde_json::from_str(&t).unwrap();
                if v["type"] == "race_reminder" {
                    break v;
                }
            }
            Ok(Some(Ok(_))) => {}
            other => panic!("websocket: {other:?}"),
        }
    };
    assert_eq!(reminder["pick_id"], id.as_str());
    assert_eq!(reminder["kind"], "lead");
    assert_eq!(reminder["lead_min"], 5);
    assert_eq!(reminder["start_ms"], start);
    assert_eq!(reminder["track"], "Monza");
    let fired: Value = serde_json::from_slice(
        &std::fs::read(data_dir.join("races").join("reminders.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        fired,
        json!([format!("{id}#5")]),
        "remembered across restarts"
    );

    let (code, _) = call("DELETE", format!("{base}/api/races/picks/{id}"), None).await;
    assert_eq!(code, 204);
    let (code, _) = call("DELETE", format!("{base}/api/races/picks/{id}"), None).await;
    assert_eq!(code, 404);
    let (code, _) = call("DELETE", format!("{base}/api/races/picks/..%2Fx"), None).await;
    assert_eq!(code, 400);

    let _ = stop.send(());
    tokio::time::timeout(Duration::from_secs(5), task)
        .await
        .expect("stops")
        .expect("join")
        .expect("run");
    let _ = std::fs::remove_dir_all(&data_dir);
}
