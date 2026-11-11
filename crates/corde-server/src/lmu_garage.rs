//! Read-only capture of the game's garage data, to learn the bounds of the
//! setup values.
//!
//! When the game is detected and the driver is **at the garage or in the
//! pits** (not on track), the answers of a closed list of `GET` requests
//! ([`GARAGE_PATHS`]) are saved under
//! `<data-dir>/diagnostics/lmu-api/garage/<car>/<name>.json` (rewritten only
//! when the content changed). Their exact shape is not known yet: they are
//! kept raw so that they can be read on the driver's PC, and
//! [`corde_lmu::garage::extract_bounds`] reads what it recognises in
//! `getPlayerGarageData.json`.
//!
//! Rules, same spirit as [`crate::lmu_api`]:
//! * `GET` only, these eight paths only — never `/rest/profile/*`,
//!   `getAuthSessionTicket`, `RepairAndRefuel` nor any write;
//! * once per car and session, at most one attempt every 60 s, and only when
//!   the car or the session changed (three failed attempts on the same car
//!   and session and it is left alone);
//! * a sober log: one line per capture;
//! * `--no-game-api` turns it off.

use crate::state::AppState;
use corde_lmu::garage::{extract_bounds, GarageBounds};
use corde_lmu::svm_library::car_dir_name;
use corde_lmu::{LmuPaths, RestClient, DEFAULT_WEBUI_PORT};
use corde_protocol::{GameId, TelemetryFrame};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Folder of the captures inside the data directory.
pub const GARAGE_DIR: &str = "diagnostics/lmu-api/garage";

/// The only requests this module ever sends: `(file name, path)`.
pub const GARAGE_PATHS: [(&str, &str); 8] = [
    ("getPlayerGarageData", "/rest/garage/getPlayerGarageData"),
    ("summary", "/rest/garage/summary"),
    ("CarSetupOverview", "/rest/garage/UIScreen/CarSetupOverview"),
    ("setup", "/rest/garage/setup"),
    ("tireinfo", "/rest/garage/tireinfo"),
    ("brakeinfo", "/rest/garage/brakeinfo"),
    ("race-car", "/rest/race/car"),
    ("race-track", "/rest/race/track"),
];

/// Minimum time between two attempts.
pub const MIN_INTERVAL: Duration = Duration::from_secs(60);
/// Attempts on one car and session before giving up on it.
const MAX_ATTEMPTS: u32 = 3;
/// How often the state of the game is looked at.
const TICK: Duration = Duration::from_secs(5);
/// Per request (the garage answers are small).
const TIMEOUT: Duration = Duration::from_secs(3);

/// `<data-dir>/diagnostics/lmu-api/garage/<car>`.
pub fn garage_dir(data_dir: &Path, vehicle_model: &str) -> PathBuf {
    data_dir.join(GARAGE_DIR).join(car_dir_name(vehicle_model))
}

/// Outcome of one capture.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GarageCapture {
    pub dir: PathBuf,
    /// Answers received (2xx).
    pub received: usize,
    /// Files written because their content changed.
    pub changed: usize,
    /// `name: reason` of the requests that failed.
    pub failed: Vec<String>,
}

fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path)
}

/// Send the eight `GET`s and keep every answer received.
pub fn capture_garage(
    client: &RestClient,
    data_dir: &Path,
    vehicle_model: &str,
) -> io::Result<GarageCapture> {
    let dir = garage_dir(data_dir, vehicle_model);
    let mut out = GarageCapture {
        dir: dir.clone(),
        ..Default::default()
    };
    for (name, path) in GARAGE_PATHS {
        match client.get_text(path) {
            Ok(text) => {
                out.received += 1;
                let file = dir.join(format!("{name}.json"));
                if fs::read(&file).map_or(true, |old| old != text.as_bytes()) {
                    fs::create_dir_all(&dir)?;
                    write_atomic(&file, text.as_bytes())?;
                    out.changed += 1;
                }
            }
            Err(e) => out.failed.push(format!("{name}: {e}")),
        }
    }
    Ok(out)
}

/// Bounds of a car from its last capture (`None`: never captured, or
/// nothing recognised in it).
pub fn load_bounds(data_dir: &Path, vehicle_model: &str) -> Option<GarageBounds> {
    let file = garage_dir(data_dir, vehicle_model).join("getPlayerGarageData.json");
    let text = fs::read_to_string(file).ok()?;
    let doc: serde_json::Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()?;
    let bounds = extract_bounds(&doc);
    (!bounds.is_empty()).then_some(bounds)
}

/// `true` when the driver is at the garage, in the pits or in the menus
/// (not driving on track).
pub fn at_garage(frame: &TelemetryFrame) -> bool {
    match &frame.player {
        Some(p) => p.in_garage || p.in_pits || !frame.session.in_realtime,
        None => false,
    }
}

/// The car + session a capture is for.
pub fn capture_key(frame: &TelemetryFrame) -> Option<(String, String)> {
    let p = frame.player.as_ref()?;
    let car = p.vehicle_model.trim();
    if car.is_empty() {
        return None;
    }
    let s = &frame.session;
    Some((
        car.to_string(),
        format!(
            "{}|{}|{:?}|{}",
            car_dir_name(car),
            s.track_name,
            s.session_type,
            s.session_index
        ),
    ))
}

/// When to capture: once per key, at most every [`MIN_INTERVAL`].
#[derive(Debug, Default)]
pub struct CaptureGate {
    done: Option<String>,
    last_attempt: Option<Instant>,
    attempts: (String, u32),
}

impl CaptureGate {
    pub fn should_try(&self, key: &str, now: Instant) -> bool {
        if self.done.as_deref() == Some(key) {
            return false;
        }
        if self.attempts.0 == key && self.attempts.1 >= MAX_ATTEMPTS {
            return false;
        }
        self.last_attempt
            .is_none_or(|t| now.duration_since(t) >= MIN_INTERVAL)
    }

    pub fn attempted(&mut self, key: &str, now: Instant, success: bool) {
        self.last_attempt = Some(now);
        if success {
            self.done = Some(key.to_string());
        } else if self.attempts.0 == key {
            self.attempts.1 += 1;
        } else {
            self.attempts = (key.to_string(), 1);
        }
    }
}

/// Background task until `shutdown` flips.
pub async fn run_periodic(
    state: Arc<AppState>,
    data_dir: PathBuf,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) {
    let mut gate = CaptureGate::default();
    loop {
        tokio::select! {
            _ = tokio::time::sleep(TICK) => {}
            _ = shutdown.changed() => {
                if *shutdown.borrow() {
                    break;
                }
            }
        }
        let status = state.status(0.0);
        if !(status.connected && status.game == GameId::Lmu && status.source.starts_with("lmu")) {
            continue;
        }
        let Some(slot) = state.latest_frame() else {
            continue;
        };
        if !at_garage(&slot.frame) {
            continue;
        }
        let Some((car, key)) = capture_key(&slot.frame) else {
            continue;
        };
        let now = Instant::now();
        if !gate.should_try(&key, now) {
            continue;
        }
        let port = state
            .game_user_data()
            .map(|dir| LmuPaths::from_user_data(dir).webui_port())
            .unwrap_or(DEFAULT_WEBUI_PORT);
        let dir = data_dir.clone();
        let car2 = car.clone();
        let result = tokio::task::spawn_blocking(move || {
            let client = RestClient::with_host(corde_lmu::rest::DEFAULT_HOST, port, TIMEOUT);
            capture_garage(&client, &dir, &car2)
        })
        .await;
        match result {
            Ok(Ok(c)) => {
                let ok = c.received > 0;
                gate.attempted(&key, now, ok);
                if ok {
                    tracing::info!(
                        "garage du jeu ({car}) : {}/{} réponses lues, {} fichier(s) mis à jour → {}",
                        c.received,
                        GARAGE_PATHS.len(),
                        c.changed,
                        c.dir.display()
                    );
                    for f in &c.failed {
                        tracing::debug!("garage du jeu : {f}");
                    }
                } else {
                    tracing::debug!("garage du jeu injoignable ({car}) : {:?}", c.failed);
                }
            }
            Ok(Err(e)) => {
                gate.attempted(&key, now, false);
                tracing::warn!("garage du jeu : écriture impossible ({e})");
            }
            Err(e) => {
                gate.attempted(&key, now, false);
                tracing::error!("garage du jeu : tâche interrompue ({e})");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use corde_protocol::{PlayerTelemetry, SessionInfo};
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::Mutex;

    fn tmp_dir(name: &str) -> PathBuf {
        // Digits only: the debug form of a `SystemTime` holds `:` and `{`, which Windows
        // refuses in a file name.
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let d = std::env::temp_dir().join(format!(
            "corde-garage-{name}-{}-{nanos}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&d);
        d
    }

    /// Answers `body_for(path)` to the next `requests` requests and records
    /// the request lines (same pattern as `lmu_api`'s tests).
    fn serve(
        requests: usize,
        body_for: fn(&str) -> (u16, &'static str),
    ) -> (u16, Arc<Mutex<Vec<String>>>) {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&seen);
        std::thread::spawn(move || {
            for stream in listener.incoming().take(requests) {
                let Ok(mut stream) = stream else { continue };
                let mut buf = [0u8; 2048];
                let n = stream.read(&mut buf).unwrap_or(0);
                let head = String::from_utf8_lossy(&buf[..n]).to_string();
                let line = head.lines().next().unwrap_or("").to_string();
                let path = line.split(' ').nth(1).unwrap_or("/").to_string();
                log.lock().unwrap().push(line);
                let (status, body) = body_for(&path);
                let reason = if status == 200 { "OK" } else { "Not Found" };
                let resp = format!(
                    "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(resp.as_bytes());
            }
        });
        (port, seen)
    }

    fn router(path: &str) -> (u16, &'static str) {
        match path {
            "/rest/garage/getPlayerGarageData" => (
                200,
                r#"{"VM_REAR_WING":{"minValue":0,"maxValue":15,"value":8,"stringValue":"P9"}}"#,
            ),
            "/rest/race/track" => (404, r#"{"error":"no"}"#),
            _ => (200, r#"{"ok":true}"#),
        }
    }

    fn client(port: u16) -> RestClient {
        RestClient::with_host("127.0.0.1", port, Duration::from_secs(2))
    }

    #[test]
    fn sends_the_eight_gets_only_and_keeps_the_answers() {
        let dir = tmp_dir("capture");
        let (port, seen) = serve(16, router);
        let c = capture_garage(&client(port), &dir, "BMW M4 LMGT3").unwrap();
        assert_eq!(c.received, 7);
        assert_eq!(c.changed, 7);
        assert_eq!(c.failed.len(), 1);
        assert!(c.failed[0].starts_with("race-track"));
        assert_eq!(c.dir, dir.join(GARAGE_DIR).join("bmw-m4-lmgt3"));
        assert!(c.dir.join("getPlayerGarageData.json").exists());
        assert!(c.dir.join("CarSetupOverview.json").exists());
        assert!(!c.dir.join("race-track.json").exists());

        // Second time: same answers, nothing rewritten.
        let again = capture_garage(&client(port), &dir, "BMW M4 LMGT3").unwrap();
        assert_eq!(again.changed, 0);

        let lines = seen.lock().unwrap().clone();
        assert_eq!(lines.len(), 16);
        for l in &lines {
            assert!(l.starts_with("GET "), "{l}");
            let path = l.split(' ').nth(1).unwrap();
            assert!(
                GARAGE_PATHS.iter().any(|(_, p)| *p == path),
                "unexpected request {l}"
            );
            assert!(!path.contains("profile") && !path.contains("RepairAndRefuel"));
        }

        let b = load_bounds(&dir, "BMW M4 LMGT3").unwrap();
        assert_eq!(b["VM_REAR_WING"].max, Some(15.0));
        assert!(load_bounds(&dir, "Porsche 911 GT3 R LMGT3").is_none());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn once_per_car_and_session_at_most_every_minute() {
        let mut g = CaptureGate::default();
        let t0 = Instant::now();
        assert!(g.should_try("a", t0));
        g.attempted("a", t0, true);
        assert!(!g.should_try("a", t0 + Duration::from_secs(3600)), "done");
        assert!(
            !g.should_try("b", t0 + Duration::from_secs(10)),
            "60 s apart"
        );
        assert!(g.should_try("b", t0 + MIN_INTERVAL));
        // Failures: three tries, then left alone.
        let mut t = t0 + MIN_INTERVAL;
        for _ in 0..3 {
            assert!(g.should_try("b", t));
            g.attempted("b", t, false);
            t += MIN_INTERVAL;
        }
        assert!(!g.should_try("b", t));
        assert!(g.should_try("c", t), "another session");
    }

    #[test]
    fn garage_means_not_driving() {
        let mut f = TelemetryFrame {
            player: Some(PlayerTelemetry {
                vehicle_model: "BMW M4 LMGT3".into(),
                ..Default::default()
            }),
            session: SessionInfo {
                in_realtime: true,
                track_name: "Long Beach".into(),
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(!at_garage(&f), "on track");
        f.player.as_mut().unwrap().in_garage = true;
        assert!(at_garage(&f));
        f.player.as_mut().unwrap().in_garage = false;
        f.session.in_realtime = false;
        assert!(at_garage(&f), "monitor / menus");
        let (car, key) = capture_key(&f).unwrap();
        assert_eq!(car, "BMW M4 LMGT3");
        assert!(key.starts_with("bmw-m4-lmgt3|Long Beach|"));
        f.player = None;
        assert!(!at_garage(&f));
        assert!(capture_key(&f).is_none());
    }
}
