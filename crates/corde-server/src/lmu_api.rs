//! Keeps a copy of the description of the game's own local API.
//!
//! Le Mans Ultimate serves an OpenAPI description of its REST API at
//! `/swagger-schema.json` (port `WebUI port` of `Settings.JSON`, 6397 by
//! default). It is the only reference for a given game build — the list of
//! what the game agrees to tell a tool like Corde — so a copy is kept in
//! `<data-dir>/diagnostics/lmu-api/`:
//!
//! * `swagger-schema.json` — the description, byte for byte;
//! * `capture.json` — when it was read, from where, and the list of paths
//!   with their methods (quick to read without opening the big file).
//!
//! This is **one read-only `GET` of a documentation file**: no other endpoint
//! of the game is called from here, nothing is ever written to the game. It
//! runs a few seconds after the game is detected, then again every half hour
//! (a game update changes the description), and the files are rewritten only
//! when the content changed.

use crate::state::AppState;
use corde_lmu::rest::PATH_SWAGGER_SCHEMA;
use corde_lmu::{LmuPaths, RestClient, DEFAULT_WEBUI_PORT};
use corde_protocol::GameId;
use serde_json::{json, Value};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

/// Folder of the capture inside the data directory.
pub const CAPTURE_DIR: &str = "diagnostics/lmu-api";
/// The game's description, as served.
pub const SCHEMA_FILE: &str = "swagger-schema.json";
/// What Corde knows about the capture.
pub const META_FILE: &str = "capture.json";

/// First attempt after the game is seen (its web server starts with it).
const FIRST_DELAY: Duration = Duration::from_secs(10);
/// Next attempt after a failure.
const RETRY_DELAY: Duration = Duration::from_secs(60);
/// Next look after a success (picks up a game update without restarting Corde).
const REFRESH_DELAY: Duration = Duration::from_secs(30 * 60);
/// The description is a few hundred kilobytes; the default 1 s of the REST
/// client is too short on a PC busy loading a track.
const TIMEOUT: Duration = Duration::from_secs(5);

/// Outcome of one capture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capture {
    /// `<data-dir>/diagnostics/lmu-api/swagger-schema.json`.
    pub path: PathBuf,
    pub bytes: usize,
    /// Number of paths the game describes.
    pub paths: usize,
    /// `false` when the game served exactly what was already on disk.
    pub changed: bool,
}

#[derive(Debug)]
pub enum CaptureError {
    /// The game's web server did not answer (not started yet, other port…).
    Unreachable(String),
    /// It answered something that is not an API description.
    Unexpected(String),
    Io(io::Error),
}

impl std::fmt::Display for CaptureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unreachable(m) => write!(f, "API du jeu injoignable : {m}"),
            Self::Unexpected(m) => write!(f, "réponse inattendue de l'API du jeu : {m}"),
            Self::Io(e) => write!(f, "écriture impossible : {e}"),
        }
    }
}

impl std::error::Error for CaptureError {}

/// `<data-dir>/diagnostics/lmu-api`.
pub fn capture_dir(data_dir: &Path) -> PathBuf {
    data_dir.join(CAPTURE_DIR)
}

/// Paths of an OpenAPI description with their HTTP methods, sorted by path.
pub fn endpoints(schema: &Value) -> Vec<(String, Vec<String>)> {
    const METHODS: [&str; 7] = ["get", "post", "put", "delete", "patch", "head", "options"];
    let mut out: Vec<(String, Vec<String>)> = schema
        .get("paths")
        .and_then(Value::as_object)
        .map(|paths| {
            paths
                .iter()
                .map(|(path, item)| {
                    let methods = METHODS
                        .iter()
                        .filter(|m| item.get(**m).is_some())
                        .map(|m| m.to_ascii_uppercase())
                        .collect();
                    (path.clone(), methods)
                })
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out
}

fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path)
}

/// Read the game's API description and keep it in the data directory.
pub fn capture_schema(
    client: &RestClient,
    data_dir: &Path,
    now_ms: u64,
) -> Result<Capture, CaptureError> {
    let text = client
        .get_text(PATH_SWAGGER_SCHEMA)
        .map_err(|e| CaptureError::Unreachable(e.to_string()))?;
    let schema: Value = serde_json::from_str(text.trim_start_matches('\u{feff}'))
        .map_err(|e| CaptureError::Unexpected(format!("pas du JSON ({e})")))?;
    let list = endpoints(&schema);
    if list.is_empty() {
        return Err(CaptureError::Unexpected(
            "aucun chemin (`paths`) dans la description".to_string(),
        ));
    }

    let dir = capture_dir(data_dir);
    let path = dir.join(SCHEMA_FILE);
    let changed = fs::read(&path).map_or(true, |old| old != text.as_bytes());
    if changed {
        fs::create_dir_all(&dir).map_err(CaptureError::Io)?;
        write_atomic(&path, text.as_bytes()).map_err(CaptureError::Io)?;
        let info = schema.get("info");
        let meta = json!({
            "captured_ms": now_ms,
            "source": format!("{}{PATH_SWAGGER_SCHEMA}", client.base_url()),
            "bytes": text.len(),
            "title": info.and_then(|i| i.get("title")).cloned().unwrap_or(Value::Null),
            "version": info.and_then(|i| i.get("version")).cloned().unwrap_or(Value::Null),
            "paths": list.len(),
            "endpoints": list
                .iter()
                .map(|(p, m)| json!({ "path": p, "methods": m }))
                .collect::<Vec<_>>(),
        });
        let mut body = serde_json::to_vec_pretty(&meta)
            .map_err(io::Error::other)
            .map_err(CaptureError::Io)?;
        body.push(b'\n');
        write_atomic(&dir.join(META_FILE), &body).map_err(CaptureError::Io)?;
    }
    Ok(Capture {
        path,
        bytes: text.len(),
        paths: list.len(),
        changed,
    })
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Background task: capture once the game is there, until `shutdown` flips.
pub async fn run_periodic(
    state: Arc<AppState>,
    data_dir: PathBuf,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) {
    let mut delay = FIRST_DELAY;
    let mut warned = false;
    loop {
        tokio::select! {
            _ = tokio::time::sleep(delay) => {}
            _ = shutdown.changed() => {
                if *shutdown.borrow() {
                    break;
                }
            }
        }
        // Only the real game has this API (not the simulator, not a replay).
        let status = state.status(0.0);
        if !(status.connected && status.game == GameId::Lmu && status.source.starts_with("lmu")) {
            delay = FIRST_DELAY;
            continue;
        }
        let port = state
            .game_user_data()
            .map(|dir| LmuPaths::from_user_data(dir).webui_port())
            .unwrap_or(DEFAULT_WEBUI_PORT);
        let dir = data_dir.clone();
        let result = tokio::task::spawn_blocking(move || {
            let client = RestClient::with_host(corde_lmu::rest::DEFAULT_HOST, port, TIMEOUT);
            capture_schema(&client, &dir, now_ms())
        })
        .await;
        delay = match result {
            Ok(Ok(c)) => {
                if c.changed {
                    tracing::info!(
                        "API du jeu : description enregistrée ({} chemins, {} ko) → {}",
                        c.paths,
                        c.bytes / 1024,
                        c.path.display()
                    );
                }
                warned = false;
                REFRESH_DELAY
            }
            Ok(Err(e)) => {
                // Once is enough: the game may simply not have started its web server yet.
                if !warned {
                    tracing::warn!("{e} (port {port}) — nouvel essai toutes les minutes");
                    warned = true;
                }
                RETRY_DELAY
            }
            Err(e) => {
                tracing::error!("capture de l'API du jeu : tâche interrompue ({e})");
                RETRY_DELAY
            }
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    fn tmp_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "corde-lmu-api-{name}-{}-{}",
            std::process::id(),
            now_ms()
        ));
        let _ = fs::remove_dir_all(&d);
        d
    }

    /// A one-file web server: answers `body` to the next `requests` requests.
    fn serve(body: &'static str, requests: usize) -> u16 {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for stream in listener.incoming().take(requests) {
                let Ok(mut stream) = stream else { continue };
                let mut buf = [0u8; 2048];
                let _ = stream.read(&mut buf);
                let head = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(head.as_bytes());
                let _ = stream.write_all(body.as_bytes());
            }
        });
        port
    }

    fn client(port: u16) -> RestClient {
        RestClient::with_host("127.0.0.1", port, Duration::from_secs(2))
    }

    const SCHEMA: &str = r#"{"swagger":"2.0","info":{"title":"LMU","version":"1.0"},"paths":{
        "/rest/watch/standings":{"get":{}},
        "/rest/garage/setup":{"post":{},"delete":{},"get":{}},
        "/rest/sessions":{"get":{}}}}"#;

    #[test]
    fn lists_paths_with_their_methods() {
        let schema: Value = serde_json::from_str(SCHEMA).unwrap();
        let list = endpoints(&schema);
        assert_eq!(list.len(), 3);
        assert_eq!(list[0].0, "/rest/garage/setup");
        assert_eq!(list[0].1, ["GET", "POST", "DELETE"]);
        assert_eq!(
            list[2],
            ("/rest/watch/standings".to_string(), vec!["GET".to_string()])
        );
        assert!(endpoints(&json!({"paths": 3})).is_empty());
    }

    #[test]
    fn writes_the_description_once_and_only_rewrites_a_change() {
        let dir = tmp_dir("capture");
        let port = serve(SCHEMA, 2);

        let first = capture_schema(&client(port), &dir, 1_000).unwrap();
        assert!(first.changed);
        assert_eq!(first.paths, 3);
        assert_eq!(fs::read_to_string(&first.path).unwrap(), SCHEMA);
        let meta: Value =
            serde_json::from_slice(&fs::read(capture_dir(&dir).join(META_FILE)).unwrap()).unwrap();
        assert_eq!(meta["captured_ms"], 1_000);
        assert_eq!(meta["title"], "LMU");
        assert_eq!(meta["paths"], 3);
        assert_eq!(meta["endpoints"][0]["path"], "/rest/garage/setup");
        assert_eq!(meta["endpoints"][0]["methods"][1], "POST");

        // Same content: nothing is rewritten, the first capture date stays.
        let second = capture_schema(&client(port), &dir, 2_000).unwrap();
        assert!(!second.changed);
        let meta: Value =
            serde_json::from_slice(&fs::read(capture_dir(&dir).join(META_FILE)).unwrap()).unwrap();
        assert_eq!(meta["captured_ms"], 1_000);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn refuses_what_is_not_a_description_and_writes_nothing() {
        let dir = tmp_dir("bad");
        for body in [
            "<html>404</html>",
            r#"{"paths":{}}"#,
            r#"{"hello":"world"}"#,
        ] {
            let port = serve(body, 1);
            let err = capture_schema(&client(port), &dir, 1).unwrap_err();
            assert!(matches!(err, CaptureError::Unexpected(_)), "{body}: {err}");
        }
        assert!(!capture_dir(&dir).exists());
    }

    #[test]
    fn a_closed_port_is_reported_as_unreachable() {
        let dir = tmp_dir("closed");
        // Bind then drop: the port is free again and nothing listens on it.
        let port = TcpListener::bind(("127.0.0.1", 0))
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let err = capture_schema(&client(port), &dir, 1).unwrap_err();
        assert!(matches!(err, CaptureError::Unreachable(_)), "{err}");
        assert!(!capture_dir(&dir).exists());
    }
}
