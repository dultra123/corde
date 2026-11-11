//! Minimal **synchronous, GET-only** client for the local REST API of the game
//! (`http://127.0.0.1:<WebUI port>`, default 6397).
//!
//! The API is the backend of the in-game web UI: it is up whenever the game
//! runs, needs no plugin, and answers JSON. Its paths and schemas change with
//! game updates, so everything is returned as `serde_json::Value`; typed
//! wrappers belong to the consumers (and to a per-version adaptation layer).
//!
//! Rules (see `docs/research/01-telemetrie-lmu.md` §2):
//! * host forced to `127.0.0.1` — since LMU 1.3.3 `localhost` may resolve to
//!   `::1` while the server only listens on IPv4;
//! * 1 s timeout on every call, the game must never stall the pipeline;
//! * `/rest/garage/UIScreen/RepairAndRefuel` is polled at most every
//!   [`REPAIR_REFUEL_MIN_INTERVAL`]: faster polling makes the in-game pit MFD
//!   flicker (known, unfixed bug) — [`RestClient::repair_and_refuel`] serves a
//!   cached value in between;
//! * **no write endpoint**, ever, in this module.

use serde_json::Value;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use thiserror::Error;

/// Host the game's REST server listens on.
pub const DEFAULT_HOST: &str = "127.0.0.1";
/// Per-request timeout (connect + response).
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(1);
/// Minimum interval between two real calls of `RepairAndRefuel`.
pub const REPAIR_REFUEL_MIN_INTERVAL: Duration = Duration::from_millis(500);

pub const PATH_PLAYER_GARAGE_DATA: &str = "/rest/garage/getPlayerGarageData";
pub const PATH_WEATHER: &str = "/rest/sessions/weather";
pub const PATH_SESSION_INFO: &str = "/rest/watch/sessionInfo";
pub const PATH_VEHICLE_CONDITION: &str = "/rest/garage/getVehicleCondition";
pub const PATH_REPAIR_AND_REFUEL: &str = "/rest/garage/UIScreen/RepairAndRefuel";
pub const PATH_STANDINGS: &str = "/rest/watch/standings";
pub const PATH_GAME_STATE: &str = "/rest/sessions/GetGameState";
pub const PATH_PITSTOP_ESTIMATE: &str = "/rest/strategy/pitstop-estimate";
pub const PATH_SWAGGER_SCHEMA: &str = "/swagger-schema.json";

#[derive(Debug, Error)]
pub enum RestError {
    /// Connection refused / reset: the game is not running (or the port is wrong).
    #[error("LMU REST API unreachable at {url}: {source}")]
    Unreachable {
        url: String,
        #[source]
        source: ureq::Error,
    },
    #[error("LMU REST API timed out on {0}")]
    Timeout(String),
    #[error("LMU REST API answered HTTP {status} on {path}")]
    Status { path: String, status: u16 },
    #[error("LMU REST API returned invalid JSON on {path}: {source}")]
    InvalidJson {
        path: String,
        #[source]
        source: ureq::Error,
    },
    #[error("LMU REST API request failed on {path}: {source}")]
    Other {
        path: String,
        #[source]
        source: ureq::Error,
    },
}

struct Cached {
    at: Instant,
    value: Value,
}

/// GET-only client bound to one base URL.
pub struct RestClient {
    base: String,
    agent: ureq::Agent,
    timeout: Duration,
    repair_refuel_cache: Mutex<Option<Cached>>,
}

impl std::fmt::Debug for RestClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RestClient")
            .field("base", &self.base)
            .field("timeout", &self.timeout)
            .finish()
    }
}

impl RestClient {
    /// Client for `http://127.0.0.1:<port>` with the default timeout.
    pub fn new(port: u16) -> Self {
        Self::with_host(DEFAULT_HOST, port, DEFAULT_TIMEOUT)
    }

    /// Client for an arbitrary host (tests, remote PC on the LAN).
    pub fn with_host(host: &str, port: u16, timeout: Duration) -> Self {
        let config = ureq::Agent::config_builder()
            .timeout_global(Some(timeout))
            .http_status_as_error(false)
            .user_agent("corde-lmu")
            .build();
        Self {
            base: format!("http://{host}:{port}"),
            agent: ureq::Agent::new_with_config(config),
            timeout,
            repair_refuel_cache: Mutex::new(None),
        }
    }

    /// Base URL (`http://127.0.0.1:6397`).
    pub fn base_url(&self) -> &str {
        &self.base
    }

    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    /// `GET <base><path>` and parse the body as JSON.
    pub fn get_json(&self, path: &str) -> Result<Value, RestError> {
        let url = format!("{}{}", self.base, path);
        let mut resp = self
            .agent
            .get(&url)
            .call()
            .map_err(|e| map_error(e, &url, path))?;
        let status = resp.status().as_u16();
        if !(200..300).contains(&status) {
            return Err(RestError::Status {
                path: path.to_string(),
                status,
            });
        }
        resp.body_mut().read_json::<Value>().map_err(|e| match e {
            ureq::Error::Timeout(_) => RestError::Timeout(path.to_string()),
            other => RestError::InvalidJson {
                path: path.to_string(),
                source: other,
            },
        })
    }

    /// `GET <base><path>` and return the raw body (for `/swagger-schema.json`,
    /// debugging dumps…).
    pub fn get_text(&self, path: &str) -> Result<String, RestError> {
        let url = format!("{}{}", self.base, path);
        let mut resp = self
            .agent
            .get(&url)
            .call()
            .map_err(|e| map_error(e, &url, path))?;
        let status = resp.status().as_u16();
        if !(200..300).contains(&status) {
            return Err(RestError::Status {
                path: path.to_string(),
                status,
            });
        }
        resp.body_mut()
            .read_to_string()
            .map_err(|e| RestError::Other {
                path: path.to_string(),
                source: e,
            })
    }

    /// `true` when the server answers at all (any HTTP status).
    pub fn is_reachable(&self) -> bool {
        !matches!(
            self.get_text(PATH_GAME_STATE),
            Err(RestError::Unreachable { .. } | RestError::Timeout(_))
        )
    }

    /// Current garage setup: `VM_*` objects `{value, stringValue,
    /// lastSavedStringValue, minValue, maxValue}`. Frozen while on track.
    pub fn player_garage_data(&self) -> Result<Value, RestError> {
        self.get_json(PATH_PLAYER_GARAGE_DATA)
    }

    /// Weather and per-session forecast.
    pub fn weather(&self) -> Result<Value, RestError> {
        self.get_json(PATH_WEATHER)
    }

    /// Session summary: temperatures, phase, flags, time remaining…
    pub fn session_info(&self) -> Result<Value, RestError> {
        self.get_json(PATH_SESSION_INFO)
    }

    /// Fuel, tyre / brake condition, suspension and body damage.
    pub fn vehicle_condition(&self) -> Result<Value, RestError> {
        self.get_json(PATH_VEHICLE_CONDITION)
    }

    /// Pit-stop screen: virtual energy max, wearables (aero, brakes,
    /// suspension), pit menu and times.
    ///
    /// Rate-limited to one real request per [`REPAIR_REFUEL_MIN_INTERVAL`];
    /// in between the previous answer is returned (`cached == true`).
    pub fn repair_and_refuel(&self) -> Result<(Value, bool), RestError> {
        if let Ok(guard) = self.repair_refuel_cache.lock() {
            if let Some(c) = guard.as_ref() {
                if c.at.elapsed() < REPAIR_REFUEL_MIN_INTERVAL {
                    return Ok((c.value.clone(), true));
                }
            }
        }
        let value = self.get_json(PATH_REPAIR_AND_REFUEL)?;
        if let Ok(mut guard) = self.repair_refuel_cache.lock() {
            *guard = Some(Cached {
                at: Instant::now(),
                value: value.clone(),
            });
        }
        Ok((value, false))
    }

    /// Forget the cached `RepairAndRefuel` answer (e.g. after a pit stop).
    pub fn invalidate_repair_and_refuel(&self) {
        if let Ok(mut guard) = self.repair_refuel_cache.lock() {
            *guard = None;
        }
    }
}

fn map_error(e: ureq::Error, url: &str, path: &str) -> RestError {
    match e {
        ureq::Error::Timeout(_) => RestError::Timeout(path.to_string()),
        ureq::Error::Io(_) | ureq::Error::ConnectionFailed | ureq::Error::HostNotFound => {
            RestError::Unreachable {
                url: url.to_string(),
                source: e,
            }
        }
        ureq::Error::StatusCode(code) => RestError::Status {
            path: path.to_string(),
            status: code,
        },
        other => RestError::Other {
            path: path.to_string(),
            source: other,
        },
    }
}

/// Convenience: `value["a"]["b"]…` as `f64` when present.
pub fn number_at(value: &Value, path: &[&str]) -> Option<f64> {
    let mut cur = value;
    for key in path {
        cur = cur.get(key)?;
    }
    cur.as_f64()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    /// Tiny HTTP/1.1 server answering `n` requests on a background thread.
    /// Returns the port and the list of request lines received.
    fn serve(
        n: usize,
        body_for: fn(&str) -> (u16, String),
    ) -> (u16, std::thread::JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let handle = std::thread::spawn(move || {
            let mut seen = Vec::new();
            for _ in 0..n {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut buf = Vec::new();
                let mut chunk = [0u8; 1024];
                loop {
                    let k = match stream.read(&mut chunk) {
                        Ok(0) | Err(_) => break,
                        Ok(k) => k,
                    };
                    buf.extend_from_slice(&chunk[..k]);
                    if buf.windows(4).any(|w| w == b"\r\n\r\n") {
                        break;
                    }
                }
                let text = String::from_utf8_lossy(&buf).to_string();
                let request_line = text.lines().next().unwrap_or("").to_string();
                let path = request_line.split(' ').nth(1).unwrap_or("/").to_string();
                let (status, body) = body_for(&path);
                let reason = if status == 200 { "OK" } else { "Error" };
                let resp = format!(
                    "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.write_all(resp.as_bytes());
                let _ = stream.flush();
                seen.push(request_line);
            }
            seen
        });
        (port, handle)
    }

    fn router(path: &str) -> (u16, String) {
        match path {
            PATH_WEATHER => (200, r#"{"RACE":{"ambient":21.5}}"#.into()),
            PATH_SESSION_INFO => (200, r#"{"gamePhase":5,"trackTemp":29.5}"#.into()),
            PATH_REPAIR_AND_REFUEL => (200, r#"{"fuelInfo":{"maxVirtualEnergy":100}}"#.into()),
            PATH_PLAYER_GARAGE_DATA => (
                200,
                r#"{"VM_STEER_LOCK":{"value":3,"stringValue":"540"}}"#.into(),
            ),
            "/broken" => (200, "{not json".into()),
            _ => (404, r#"{"error":"not found"}"#.into()),
        }
    }

    #[test]
    fn typed_getters_and_rate_limit() {
        // weather, sessionInfo, garage, RepairAndRefuel (once), 404, broken
        let (port, handle) = serve(6, router);
        let c = RestClient::with_host("127.0.0.1", port, Duration::from_secs(2));
        assert_eq!(c.base_url(), format!("http://127.0.0.1:{port}"));

        let w = c.weather().unwrap();
        assert_eq!(number_at(&w, &["RACE", "ambient"]), Some(21.5));
        let s = c.session_info().unwrap();
        assert_eq!(number_at(&s, &["trackTemp"]), Some(29.5));
        let g = c.player_garage_data().unwrap();
        assert_eq!(g["VM_STEER_LOCK"]["value"], 3);

        let (r1, cached1) = c.repair_and_refuel().unwrap();
        assert!(!cached1);
        assert_eq!(
            number_at(&r1, &["fuelInfo", "maxVirtualEnergy"]),
            Some(100.0)
        );
        // Immediately again: served from cache, no request hits the server.
        let (r2, cached2) = c.repair_and_refuel().unwrap();
        assert!(cached2);
        assert_eq!(r1, r2);

        match c.get_json("/nope") {
            Err(RestError::Status { status: 404, .. }) => {}
            other => panic!("expected 404, got {other:?}"),
        }
        match c.get_json("/broken") {
            Err(RestError::InvalidJson { .. }) => {}
            other => panic!("expected invalid json, got {other:?}"),
        }

        let seen = handle.join().unwrap();
        assert_eq!(seen.len(), 6);
        assert_eq!(
            seen.iter()
                .filter(|l| l.contains(PATH_REPAIR_AND_REFUEL))
                .count(),
            1,
            "RepairAndRefuel must be requested once: {seen:?}"
        );
        assert!(seen.iter().all(|l| l.starts_with("GET ")), "{seen:?}");
    }

    #[test]
    fn unreachable_port_is_reported() {
        // Bind then drop: the port is free and refuses connections.
        let port = {
            let l = TcpListener::bind("127.0.0.1:0").unwrap();
            l.local_addr().unwrap().port()
        };
        let c = RestClient::with_host("127.0.0.1", port, Duration::from_millis(500));
        match c.vehicle_condition() {
            Err(RestError::Unreachable { .. }) | Err(RestError::Timeout(_)) => {}
            other => panic!("expected unreachable, got {other:?}"),
        }
        assert!(!c.is_reachable());
    }
}
