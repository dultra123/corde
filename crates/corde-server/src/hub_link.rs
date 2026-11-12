//! On the PC: the link to the driver's home server (ADR 0010) and the sending of what it
//! lacks ([`crate::sync_wire`]).
//!
//! * `<data>/hub-link.json` keeps the server's address and, once paired, this PC's device
//!   token (mode 0600 on Unix; under Windows, `%LOCALAPPDATA%` is private to the user). The
//!   token is never shown by the API nor written to the log.
//! * [`run_periodic`] sends at start-up (backlog), after every recording that stops, every
//!   few minutes, and when asked (`POST /api/hub-link/sync`, « Tout renvoyer »).
//! * Routes: `GET /api/hub-link` (state), `PUT /api/hub-link` {url}, `POST
//!   /api/hub-link/pair` {url?, code, name?}, `POST /api/hub-link/sync`, `POST
//!   /api/hub-link/resend`, `DELETE /api/hub-link` — all but the first from this PC only.

use crate::api::{store_of, ApiError};
use crate::state::{AppState, BusMessage, SERVER_VERSION};
use crate::sync_wire::{
    self, FileCommit, Hello, Manifest, CHUNK_DEFAULT, HEADER_SCHEMA_VERSION, HEADER_SYNC_VERSION,
    SYNC_VERSION,
};
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use corde_store::{FeedbackSet, Store};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};
use tokio::sync::Notify;

/// File of the link, in the data folder.
pub const LINK_FILE: &str = "hub-link.json";
/// Cache of the SHA-256 of the local sample files, in the data folder.
pub const HASH_CACHE_FILE: &str = "sync-hashes.json";

#[derive(Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
struct LinkFile {
    url: String,
    #[serde(default)]
    token: Option<String>,
    #[serde(default)]
    device_id: Option<String>,
    #[serde(default)]
    device_name: Option<String>,
    #[serde(default)]
    paired_ms: Option<i64>,
}

impl std::fmt::Debug for LinkFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LinkFile")
            .field("url", &self.url)
            .field("paired", &self.token.is_some())
            .finish()
    }
}

/// Where the link stands (what the settings show).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum LinkState {
    /// No server address, or not paired yet.
    #[default]
    Unpaired,
    /// Paired, nothing to send (or waiting for the next round).
    Connected,
    Syncing,
    /// The server does not answer (switched off, other network, VPN down).
    Unreachable,
    /// The server is older than this PC.
    ServerOutdated,
    /// This PC is older than the server.
    PcOutdated,
    /// The server no longer knows this PC (revoked): pair again.
    Revoked,
    Error,
}

/// What one round sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub struct SyncReport {
    pub results_sent: u32,
    pub sessions_sent: u32,
    pub files_sent: u32,
    pub chunks_sent: u32,
    pub bytes_sent: u64,
    pub feedback_pushed: u32,
    pub feedback_pulled: u32,
}

/// `GET /api/hub-link`.
#[derive(Debug, Clone, PartialEq, Serialize, Default)]
pub struct LinkStatus {
    pub url: Option<String>,
    pub paired: bool,
    pub device_name: Option<String>,
    pub state: LinkState,
    /// French, for the settings.
    pub message: Option<String>,
    pub last_sync_ms: Option<i64>,
    pub last_attempt_ms: Option<i64>,
    /// Sessions / results the server still lacks (`None` until a round has looked).
    pub pending_sessions: Option<u32>,
    pub pending_results: Option<u32>,
    pub server_version: Option<String>,
    pub last_report: Option<SyncReport>,
}

/// Knobs of one round (tests use the last two).
#[derive(Debug, Clone)]
pub struct SyncOptions {
    /// Send every bundle again even when the server already has it.
    pub force: bool,
    pub chunk_size: usize,
    /// Stop with [`SyncError::Interrupted`] after this many chunks (simulated cut).
    pub max_chunks: Option<u32>,
}

impl Default for SyncOptions {
    fn default() -> Self {
        Self {
            force: false,
            chunk_size: CHUNK_DEFAULT,
            max_chunks: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncError {
    NotPaired,
    Unreachable(String),
    Revoked,
    Incompatible { code: String, message: String },
    Http { status: u16, message: String },
    Local(String),
    Interrupted,
}

impl std::fmt::Display for SyncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotPaired => write!(f, "not paired"),
            Self::Unreachable(e) => write!(f, "server unreachable: {e}"),
            Self::Revoked => write!(f, "this PC is no longer paired with the server"),
            Self::Incompatible { code, .. } => write!(f, "incompatible versions ({code})"),
            Self::Http { status, message } => write!(f, "HTTP {status}: {message}"),
            Self::Local(e) => write!(f, "local error: {e}"),
            Self::Interrupted => write!(f, "interrupted"),
        }
    }
}

impl SyncError {
    fn state(&self) -> LinkState {
        match self {
            Self::NotPaired => LinkState::Unpaired,
            Self::Unreachable(_) | Self::Interrupted => LinkState::Unreachable,
            Self::Revoked => LinkState::Revoked,
            Self::Incompatible { code, .. } if code == "pc_outdated" => LinkState::PcOutdated,
            Self::Incompatible { .. } => LinkState::ServerOutdated,
            Self::Http { .. } | Self::Local(_) => LinkState::Error,
        }
    }

    fn message_fr(&self) -> String {
        match self {
            Self::NotPaired => "Ce PC n'est pas appairé à un serveur Corde.".into(),
            Self::Unreachable(_) | Self::Interrupted => "Serveur injoignable : est-il allumé, et ce PC est-il sur le même réseau (ou sur le VPN) ? Nouvel essai automatique.".into(),
            Self::Revoked => "Le serveur ne reconnaît plus ce PC (appareil révoqué) : appaire-le de nouveau.".into(),
            Self::Incompatible { message, .. } => message.clone(),
            Self::Http { status, message } => format!("Le serveur a refusé l'envoi ({status}) : {message}"),
            Self::Local(_) => "Erreur sur ce PC pendant l'envoi (détails dans le journal).".into(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct HashEntry {
    size: u64,
    mtime_ms: i64,
    sha256: String,
}

/// The link of this PC, held by [`AppState::link`].
pub struct HubLink {
    data_dir: PathBuf,
    file: Mutex<Option<LinkFile>>,
    status: Mutex<LinkStatus>,
    hashes: Mutex<BTreeMap<String, HashEntry>>,
    force_next: AtomicBool,
    /// Wakes [`run_periodic`] (pairing, « sync now », « send everything again »).
    pub wake: Notify,
    /// One round at a time.
    running: Mutex<()>,
}

impl std::fmt::Debug for HubLink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HubLink")
            .field("data_dir", &self.data_dir)
            .finish()
    }
}

fn now_ms() -> i64 {
    corde_store::db::now_ms()
}

/// `http://<host>[:port]` from what the driver typed (`192.168.1.50`, `corde.local:8471`,
/// `http://…/`). Port 8471 when none. No path, no credentials.
pub fn normalise_url(input: &str) -> Result<String, String> {
    let raw = input.trim().trim_end_matches('/');
    if raw.is_empty() {
        return Err("adresse vide".into());
    }
    let (scheme, rest) = match raw.split_once("://") {
        Some((s, r)) => (s.to_ascii_lowercase(), r),
        None => ("http".to_string(), raw),
    };
    if scheme != "http" && scheme != "https" {
        return Err("l'adresse doit commencer par http://".into());
    }
    if rest.contains(['/', '?', '#', '@', ' ']) || rest.is_empty() {
        return Err("adresse invalide : seulement l'adresse du serveur, par exemple 192.168.1.50 ou http://192.168.1.50:8471".into());
    }
    let Some((host, port)) = crate::net::split_host_port(rest) else {
        return Err("adresse invalide".into());
    };
    let host = if host.contains(':') {
        format!("[{host}]")
    } else {
        host
    };
    let port = port.unwrap_or(crate::config::DEFAULT_PORT);
    Ok(format!("{scheme}://{host}:{port}"))
}

impl HubLink {
    pub fn open(data_dir: &Path) -> Self {
        let file = std::fs::read(data_dir.join(LINK_FILE))
            .ok()
            .and_then(|b| serde_json::from_slice::<LinkFile>(&b).ok());
        let hashes = std::fs::read(data_dir.join(HASH_CACHE_FILE))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        let status = LinkStatus {
            url: file.as_ref().map(|f| f.url.clone()),
            paired: file.as_ref().is_some_and(|f| f.token.is_some()),
            device_name: file.as_ref().and_then(|f| f.device_name.clone()),
            state: if file.as_ref().is_some_and(|f| f.token.is_some()) {
                LinkState::Connected
            } else {
                LinkState::Unpaired
            },
            ..Default::default()
        };
        Self {
            data_dir: data_dir.to_path_buf(),
            file: Mutex::new(file),
            status: Mutex::new(status),
            hashes: Mutex::new(hashes),
            force_next: AtomicBool::new(false),
            wake: Notify::new(),
            running: Mutex::new(()),
        }
    }

    pub fn status(&self) -> LinkStatus {
        self.status.lock().map(|s| s.clone()).unwrap_or_default()
    }

    fn update_status(&self, f: impl FnOnce(&mut LinkStatus)) {
        if let Ok(mut s) = self.status.lock() {
            f(&mut s);
        }
    }

    fn file(&self) -> Option<LinkFile> {
        self.file.lock().ok().and_then(|f| f.clone())
    }

    pub fn is_paired(&self) -> bool {
        self.file().is_some_and(|f| f.token.is_some())
    }

    fn save(&self, file: Option<LinkFile>) -> std::io::Result<()> {
        let path = self.data_dir.join(LINK_FILE);
        match &file {
            Some(f) => {
                let json = serde_json::to_vec_pretty(f).map_err(std::io::Error::other)?;
                crate::hub::auth::write_private(&path, &json)?;
            }
            None => match std::fs::remove_file(&path) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e),
            },
        }
        let paired = file.as_ref().is_some_and(|f| f.token.is_some());
        self.update_status(|s| {
            s.url = file.as_ref().map(|f| f.url.clone());
            s.paired = paired;
            s.device_name = file.as_ref().and_then(|f| f.device_name.clone());
            if !paired {
                *s = LinkStatus {
                    url: s.url.clone(),
                    ..Default::default()
                };
            }
        });
        if let Ok(mut g) = self.file.lock() {
            *g = file;
        }
        Ok(())
    }

    /// Keep the address (unpaired until a code is typed).
    pub fn set_url(&self, url: &str) -> Result<String, String> {
        let url = normalise_url(url)?;
        let same = self.file().is_some_and(|f| f.url == url);
        if !same {
            self.save(Some(LinkFile {
                url: url.clone(),
                ..Default::default()
            }))
            .map_err(|e| e.to_string())?;
        }
        Ok(url)
    }

    /// Next round sends every bundle again.
    pub fn force_next_round(&self) {
        self.force_next.store(true, Ordering::Relaxed);
        if let Ok(mut h) = self.hashes.lock() {
            h.clear();
        }
    }

    fn sha_of(&self, path: &Path) -> std::io::Result<(String, u64)> {
        let meta = std::fs::metadata(path)?;
        let size = meta.len();
        let mtime_ms = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        let key = path.to_string_lossy().into_owned();
        if let Some(e) = self.hashes.lock().ok().and_then(|h| h.get(&key).cloned()) {
            if e.size == size && e.mtime_ms == mtime_ms {
                return Ok((e.sha256, size));
            }
        }
        let sha = crate::crypto::sha256_file(path)?;
        if let Ok(mut h) = self.hashes.lock() {
            h.insert(
                key,
                HashEntry {
                    size,
                    mtime_ms,
                    sha256: sha.clone(),
                },
            );
        }
        Ok((sha, size))
    }

    fn save_hashes(&self) {
        let json = match self.hashes.lock() {
            Ok(h) => serde_json::to_vec(&*h).unwrap_or_default(),
            Err(_) => return,
        };
        let _ = std::fs::write(self.data_dir.join(HASH_CACHE_FILE), json);
    }
}

// --- HTTP ------------------------------------------------------------------

struct Client {
    agent: ureq::Agent,
    base: String,
    auth: String,
}

struct Reply {
    status: u16,
    body: Vec<u8>,
}

impl Reply {
    fn json(&self) -> serde_json::Value {
        serde_json::from_slice(&self.body).unwrap_or(serde_json::Value::Null)
    }
    fn error_text(&self) -> String {
        self.json()
            .get("error")
            .and_then(|e| e.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| {
                String::from_utf8_lossy(&self.body)
                    .chars()
                    .take(200)
                    .collect()
            })
    }
}

fn agent(timeout: Duration) -> ureq::Agent {
    ureq::config::Config::builder()
        .http_status_as_error(false)
        // The server is on the home network or the VPN: never through a web proxy.
        .proxy(None)
        .timeout_connect(Some(Duration::from_secs(5)))
        .timeout_global(Some(timeout))
        .build()
        .new_agent()
}

impl Client {
    fn new(base: &str, token: &str) -> Self {
        Self {
            agent: agent(Duration::from_secs(120)),
            base: base.to_string(),
            auth: format!("Bearer {token}"),
        }
    }

    fn send(
        &self,
        method: &str,
        path: &str,
        content_type: Option<&str>,
        body: Option<&[u8]>,
    ) -> Result<Reply, SyncError> {
        let url = format!("{}{path}", self.base);
        let sync = SYNC_VERSION.to_string();
        let schema = corde_store::db::SCHEMA_VERSION.to_string();
        let result = match method {
            "GET" => self
                .agent
                .get(&url)
                .header("Authorization", &self.auth)
                .header(HEADER_SYNC_VERSION, &sync)
                .header(HEADER_SCHEMA_VERSION, &schema)
                .call(),
            "DELETE" => self
                .agent
                .delete(&url)
                .header("Authorization", &self.auth)
                .call(),
            m => {
                let req = match m {
                    "PUT" => self.agent.put(&url),
                    _ => self.agent.post(&url),
                };
                let req = req
                    .header("Authorization", &self.auth)
                    .header(HEADER_SYNC_VERSION, &sync)
                    .header(HEADER_SCHEMA_VERSION, &schema)
                    .header(
                        "Content-Type",
                        content_type.unwrap_or("application/octet-stream"),
                    );
                req.send(body.unwrap_or(&[]))
            }
        };
        let mut resp = result.map_err(|e| SyncError::Unreachable(e.to_string()))?;
        let status = resp.status().as_u16();
        let body = resp
            .body_mut()
            .with_config()
            .limit(64 * 1024 * 1024)
            .read_to_vec()
            .map_err(|e| SyncError::Unreachable(e.to_string()))?;
        let reply = Reply { status, body };
        if status == 401 {
            return Err(SyncError::Revoked);
        }
        if status == 409 {
            let v = reply.json();
            if let Some(code @ ("server_outdated" | "pc_outdated")) =
                v.get("code").and_then(|c| c.as_str())
            {
                return Err(SyncError::Incompatible {
                    code: code.to_string(),
                    message: reply.error_text(),
                });
            }
        }
        Ok(reply)
    }

    fn expect_ok(&self, reply: Reply) -> Result<Reply, SyncError> {
        if (200..300).contains(&reply.status) {
            Ok(reply)
        } else {
            Err(SyncError::Http {
                status: reply.status,
                message: reply.error_text(),
            })
        }
    }
}

// --- one round ---------------------------------------------------------------

fn local(e: impl std::fmt::Display) -> SyncError {
    SyncError::Local(e.to_string())
}

/// What the server lacks for one session.
#[derive(Debug, Default)]
struct SessionWork {
    id: String,
    bundle: Option<(Vec<u8>, String)>,
    files: Vec<(String, PathBuf, String, u64)>,
    push_feedback: bool,
    pull_feedback: bool,
}

fn plan_sessions(
    link: &HubLink,
    store: &Store,
    manifest: &Manifest,
    force: bool,
) -> Result<Vec<SessionWork>, SyncError> {
    let ids = store.db().closed_session_ids().map_err(local)?;
    let uids = store.db().feedback_uids_by_session().map_err(local)?;
    let mut work = Vec::new();
    for id in ids {
        if store.is_active(&id) || manifest.tombstones.contains(&id) {
            continue;
        }
        let Some(bundle) = store.db().export_session(&id).map_err(local)? else {
            continue;
        };
        let remote = manifest.sessions.get(&id);
        let (packed, fp) = sync_wire::compress_json(&bundle).map_err(local)?;
        let mut w = SessionWork {
            id: id.clone(),
            ..Default::default()
        };
        if force || remote.and_then(|r| r.fingerprint.as_deref()) != Some(fp.as_str()) {
            w.bundle = Some((packed, fp));
        }
        let dir = store.session_dir(&id).map_err(local)?;
        for f in &bundle.sample_files {
            let path = dir.join(&f.name);
            if !path.is_file() {
                tracing::warn!("sync: {id}/{} missing on this PC, not sent", f.name);
                continue;
            }
            let (sha, size) = link.sha_of(&path).map_err(local)?;
            let have = remote.and_then(|r| r.files.get(&f.name));
            if have != Some(&sha) {
                w.files.push((f.name.clone(), path, sha, size));
            }
        }
        let mine = uids.get(&id).cloned().unwrap_or_default();
        let theirs = remote.map(|r| (r.feedback.clone(), r.iterations.clone()));
        let (tf, ti) = theirs.unwrap_or_default();
        w.push_feedback = mine.feedback.iter().any(|u| !tf.contains(u))
            || mine.iterations.iter().any(|u| !ti.contains(u));
        w.pull_feedback = tf.iter().any(|u| !mine.feedback.contains(u))
            || ti.iter().any(|u| !mine.iterations.contains(u));
        if w.bundle.is_some() || !w.files.is_empty() || w.push_feedback || w.pull_feedback {
            work.push(w);
        }
    }
    Ok(work)
}

/// One local sample file to send: name, path, SHA-256, size.
type LocalFile = (String, PathBuf, String, u64);

fn upload_file(
    client: &Client,
    session: &str,
    file: &LocalFile,
    opts: &SyncOptions,
    report: &mut SyncReport,
) -> Result<(), SyncError> {
    use std::io::{Read, Seek, SeekFrom};
    let (name, path, sha, size) = (file.0.as_str(), file.1.as_path(), file.2.as_str(), file.3);
    let chunk = opts.chunk_size.clamp(1024, sync_wire::CHUNK_MAX);
    for attempt in 0..2 {
        let mut f = std::fs::File::open(path).map_err(local)?;
        let mut offset: u64 = 0;
        let mut reset = attempt > 0;
        let mut sent_once = false;
        let mut conflicts = 0;
        let mut buf = vec![0u8; chunk];
        while offset < size || (size == 0 && !sent_once) {
            f.seek(SeekFrom::Start(offset)).map_err(local)?;
            let want = ((size - offset) as usize).min(chunk);
            f.read_exact(&mut buf[..want]).map_err(local)?;
            if let Some(max) = opts.max_chunks {
                if report.chunks_sent >= max {
                    return Err(SyncError::Interrupted);
                }
            }
            let reply = client.send(
                "PUT",
                &format!(
                    "/api/sync/files/{session}/{name}?offset={offset}&total={size}{}",
                    if reset { "&reset=1" } else { "" }
                ),
                None,
                Some(&buf[..want]),
            )?;
            reset = false;
            if reply.status == 409 {
                conflicts += 1;
                if conflicts > 4 {
                    return Err(SyncError::Http {
                        status: 409,
                        message: "reprise du fichier impossible".into(),
                    });
                }
                // The server already holds part of it (an earlier cut): go on from there.
                let expected = reply.json()["expected_offset"].as_u64().unwrap_or(u64::MAX);
                if expected <= size && expected != offset {
                    offset = expected;
                } else {
                    reset = true;
                    offset = 0;
                }
                continue;
            }
            client.expect_ok(reply)?;
            report.chunks_sent += 1;
            report.bytes_sent += want as u64;
            offset += want as u64;
            sent_once = true;
        }
        let commit = serde_json::to_vec(&FileCommit {
            sha256: sha.to_string(),
            size,
        })
        .map_err(local)?;
        let reply = client.send(
            "POST",
            &format!("/api/sync/files/{session}/{name}/commit"),
            Some("application/json"),
            Some(&commit),
        )?;
        if reply.status == 422 && attempt == 0 {
            // The partial file was of another version of this file: once more from zero.
            tracing::info!("sync: {session}/{name} sent again from the start (checksum)");
            continue;
        }
        client.expect_ok(reply)?;
        report.files_sent += 1;
        return Ok(());
    }
    Err(SyncError::Http {
        status: 422,
        message: "somme de contrôle différente".into(),
    })
}

/// One round: what the server lacks, sent; what it holds and this PC lacks (feedback typed
/// on the server), fetched. Blocking (call from a blocking thread).
pub fn sync_once(state: &AppState, opts: &SyncOptions) -> Result<SyncReport, SyncError> {
    let link = state.link.clone().ok_or(SyncError::NotPaired)?;
    let _running = link.running.lock().map_err(|_| local("lock"))?;
    let file = link.file().ok_or(SyncError::NotPaired)?;
    let token = file.token.clone().ok_or(SyncError::NotPaired)?;
    let store = store_of(state).map_err(|e| local(format!("{e:?}")))?;
    let force = opts.force || link.force_next.swap(false, Ordering::Relaxed);
    link.update_status(|s| {
        s.state = LinkState::Syncing;
        s.last_attempt_ms = Some(now_ms());
        s.message = None;
    });
    let client = Client::new(&file.url, &token);
    let result = run_round(state, &link, &client, &store, opts, force);
    link.save_hashes();
    match &result {
        Ok(report) => {
            if *report != SyncReport::default() {
                tracing::info!(
                    "sync: sent {} session(s), {} file(s) ({} KiB), {} result(s); feedback {} sent, {} received",
                    report.sessions_sent,
                    report.files_sent,
                    report.bytes_sent / 1024,
                    report.results_sent,
                    report.feedback_pushed,
                    report.feedback_pulled
                );
            }
            link.update_status(|s| {
                s.state = LinkState::Connected;
                s.last_sync_ms = Some(now_ms());
                s.pending_sessions = Some(0);
                s.pending_results = Some(0);
                s.last_report = Some(*report);
                s.message = None;
            });
        }
        Err(e) => {
            if force {
                link.force_next.store(true, Ordering::Relaxed);
            }
            match e {
                SyncError::Unreachable(detail) => {
                    tracing::info!("sync: server unreachable ({detail})")
                }
                other => tracing::warn!("sync: {other}"),
            }
            link.update_status(|s| {
                s.state = e.state();
                s.message = Some(e.message_fr());
            });
        }
    }
    result
}

fn run_round(
    state: &AppState,
    link: &HubLink,
    client: &Client,
    store: &Store,
    opts: &SyncOptions,
    force: bool,
) -> Result<SyncReport, SyncError> {
    let mut report = SyncReport::default();
    let hello = Hello {
        version: SERVER_VERSION.to_string(),
        sync_version: SYNC_VERSION,
        schema_version: corde_store::db::SCHEMA_VERSION,
        urls: crate::net::lan_urls(state.listen),
    };
    let body = serde_json::to_vec(&hello).map_err(local)?;
    let reply = client.send(
        "POST",
        "/api/sync/hello",
        Some("application/json"),
        Some(&body),
    )?;
    let reply = client.expect_ok(reply)?;
    let manifest: Manifest = serde_json::from_slice(&reply.body).map_err(|e| SyncError::Http {
        status: 200,
        message: format!("manifest illisible : {e}"),
    })?;
    // The server checks the PC; the PC checks the server too.
    if manifest.schema_version < corde_store::db::SCHEMA_VERSION
        || manifest.sync_version < SYNC_VERSION
    {
        return Err(SyncError::Incompatible {
            code: "server_outdated".into(),
            message: format!(
                "Le serveur Corde (version {}) est plus ancien que Corde sur ton PC : mets à jour le serveur (docs/SERVEUR.md, « Mettre à jour »).",
                manifest.server_version
            ),
        });
    }
    if SYNC_VERSION < manifest.min_sync_version {
        return Err(SyncError::Incompatible {
            code: "pc_outdated".into(),
            message: format!(
                "Corde sur ton PC est trop ancien pour ce serveur (version {}) : mets à jour Corde sur ton PC.",
                manifest.server_version
            ),
        });
    }
    link.update_status(|s| s.server_version = Some(manifest.server_version.clone()));

    // Results first: a session names the result it was reconciled against.
    let mut results = Vec::new();
    for name in store.db().result_file_names().map_err(local)? {
        let Some(bundle) = store.db().export_result(&name).map_err(local)? else {
            continue;
        };
        let (packed, fp) = sync_wire::compress_json(&bundle).map_err(local)?;
        if force || manifest.results.get(&name) != Some(&fp) {
            results.push(packed);
        }
    }
    let sessions = plan_sessions(link, store, &manifest, force)?;
    let mut pending_results = results.len() as u32;
    let mut pending_sessions = sessions.len() as u32;
    link.update_status(|s| {
        s.pending_results = Some(pending_results);
        s.pending_sessions = Some(pending_sessions);
    });
    for packed in results {
        let reply = client.send(
            "POST",
            "/api/sync/results",
            Some("application/zstd"),
            Some(&packed),
        )?;
        client.expect_ok(reply)?;
        report.results_sent += 1;
        report.bytes_sent += packed.len() as u64;
        pending_results -= 1;
        link.update_status(|s| s.pending_results = Some(pending_results));
    }
    for w in sessions {
        for file in &w.files {
            upload_file(client, &w.id, file, opts, &mut report)?;
        }
        if let Some((packed, _fp)) = &w.bundle {
            let reply = client.send(
                "PUT",
                &format!("/api/sync/sessions/{}", w.id),
                Some("application/zstd"),
                Some(packed),
            )?;
            if reply.status == 410 {
                tracing::info!("sync: session {} deleted on the server, not sent", w.id);
                pending_sessions -= 1;
                continue;
            }
            client.expect_ok(reply)?;
            report.sessions_sent += 1;
            report.bytes_sent += packed.len() as u64;
        }
        if w.pull_feedback {
            let reply = client.send(
                "GET",
                &format!("/api/sync/sessions/{}/feedback", w.id),
                None,
                None,
            )?;
            let reply = client.expect_ok(reply)?;
            let set: FeedbackSet = serde_json::from_slice(&reply.body).map_err(local)?;
            let merged = store.db().merge_feedback(&w.id, &set).map_err(local)?;
            report.feedback_pulled += (merged.feedback_added + merged.iterations_added) as u32;
        }
        if w.push_feedback || w.bundle.is_some() {
            let set = store.db().export_feedback(&w.id).map_err(local)?;
            if !set.feedback.is_empty() || !set.iterations.is_empty() {
                let body = serde_json::to_vec(&set).map_err(local)?;
                let reply = client.send(
                    "POST",
                    &format!("/api/sync/sessions/{}/feedback", w.id),
                    Some("application/json"),
                    Some(&body),
                )?;
                let reply = client.expect_ok(reply)?;
                let merged: corde_store::MergeReport =
                    serde_json::from_slice(&reply.body).map_err(local)?;
                report.feedback_pushed += (merged.feedback_added + merged.iterations_added) as u32;
            }
        }
        pending_sessions -= 1;
        link.update_status(|s| s.pending_sessions = Some(pending_sessions));
    }
    Ok(report)
}

// --- pairing ---------------------------------------------------------------

/// Pair this PC with the server at `url` using a code read on the server. The token is
/// kept in `<data>/hub-link.json`. Blocking.
pub fn pair_blocking(
    state: &AppState,
    url: &str,
    code: &str,
    name: &str,
) -> Result<LinkStatus, (StatusCode, String)> {
    let link = state.link.clone().ok_or((
        StatusCode::NOT_FOUND,
        "pas de lien serveur sur ce rôle".to_string(),
    ))?;
    let url = normalise_url(url).map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    let body = json!({ "code": code, "name": name, "kind": "app" }).to_string();
    let resp = agent(Duration::from_secs(15))
        .post(&format!("{url}/api/hub/pair"))
        .header("Content-Type", "application/json")
        .send(body.as_bytes());
    let mut resp = resp.map_err(|_| {
        (
            StatusCode::BAD_GATEWAY,
            "Serveur injoignable à cette adresse : vérifie qu'il est allumé et que l'adresse est la bonne.".to_string(),
        )
    })?;
    let status = resp.status().as_u16();
    let text = resp.body_mut().read_to_string().unwrap_or_default();
    let v: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
    if status != 201 {
        let msg = v
            .get("error")
            .and_then(|e| e.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| {
                if status == 404 {
                    "Ce n'est pas un serveur Corde (ou il n'est pas en mode serveur).".to_string()
                } else {
                    format!("Appairage refusé ({status}).")
                }
            });
        let code = if status == 429 {
            StatusCode::TOO_MANY_REQUESTS
        } else {
            StatusCode::BAD_REQUEST
        };
        return Err((code, msg));
    }
    let token = v["token"].as_str().unwrap_or("").to_string();
    if token.len() != 64 {
        return Err((
            StatusCode::BAD_GATEWAY,
            "réponse d'appairage inattendue".to_string(),
        ));
    }
    let file = LinkFile {
        url,
        token: Some(token),
        device_id: v["device"]["id"].as_str().map(str::to_string),
        device_name: v["device"]["name"].as_str().map(str::to_string),
        paired_ms: Some(now_ms()),
    };
    link.save(Some(file))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    link.update_status(|s| {
        s.state = LinkState::Connected;
        s.message = None;
    });
    tracing::info!("paired with the home server");
    Ok(link.status())
}

/// Forget the server (asking it first to revoke this PC, best effort). Blocking.
pub fn unpair_blocking(state: &AppState) -> Result<bool, String> {
    let Some(link) = state.link.clone() else {
        return Ok(false);
    };
    let mut revoked = false;
    if let Some(f) = link.file() {
        if let (Some(token), Some(id)) = (&f.token, &f.device_id) {
            let client = Client {
                agent: agent(Duration::from_secs(5)),
                base: f.url.clone(),
                auth: format!("Bearer {token}"),
            };
            revoked = client
                .send("DELETE", &format!("/api/hub/devices/{id}"), None, None)
                .is_ok_and(|r| r.status == 204);
        }
    }
    link.save(None).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(link.data_dir.join(HASH_CACHE_FILE));
    tracing::info!(
        "unpaired from the home server{}",
        if revoked {
            ""
        } else {
            " (the server could not be told)"
        }
    );
    Ok(revoked)
}

// --- background task ---------------------------------------------------------

/// Sends at start-up, after every recording that stops, every `interval` (0 = only when
/// woken), and when [`HubLink::wake`] is notified. Backs off to a minute after a failure.
pub async fn run_periodic(
    state: Arc<AppState>,
    interval: Duration,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) {
    let Some(link) = state.link.clone() else {
        return;
    };
    let mut bus = state.bus.subscribe();
    let auto = !interval.is_zero();
    let mut next = if auto {
        Duration::from_secs(15).min(interval)
    } else {
        Duration::MAX
    };
    loop {
        let sleep = tokio::time::sleep(next.min(Duration::from_secs(365 * 24 * 3600)));
        tokio::pin!(sleep);
        let mut woken = false;
        loop {
            tokio::select! {
                _ = &mut sleep => break,
                _ = link.wake.notified() => { woken = true; break; }
                msg = bus.recv() => {
                    if let Ok(BusMessage::Json(j)) = msg {
                        if j.contains("\"recording_stopped\"") {
                            // Let the recorder close its files first.
                            tokio::time::sleep(Duration::from_secs(3)).await;
                            woken = true;
                            break;
                        }
                    }
                }
                _ = shutdown.changed() => return,
            }
        }
        if !link.is_paired() {
            next = if auto { interval } else { Duration::MAX };
            continue;
        }
        let st = Arc::clone(&state);
        let result =
            tokio::task::spawn_blocking(move || sync_once(&st, &SyncOptions::default())).await;
        let ok = matches!(result, Ok(Ok(_)));
        next = match (ok, auto) {
            (true, true) => interval,
            (true, false) => Duration::MAX,
            (false, _) => Duration::from_secs(60),
        };
        let _ = woken;
    }
}

// --- routes ------------------------------------------------------------------

fn link_of(state: &AppState) -> Result<Arc<HubLink>, ApiError> {
    state
        .link
        .clone()
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "pas de lien serveur sur ce rôle"))
}

pub async fn get_link(State(state): State<Arc<AppState>>) -> Result<Response, ApiError> {
    let link = link_of(&state)?;
    Ok(Json(link.status()).into_response())
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UrlBody {
    pub url: String,
}

pub async fn put_link(
    State(state): State<Arc<AppState>>,
    body: axum::body::Bytes,
) -> Result<Response, ApiError> {
    let link = link_of(&state)?;
    let body: UrlBody = serde_json::from_slice(&body).map_err(|_| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "adresse attendue : {\"url\": \"…\"}",
        )
    })?;
    let l = Arc::clone(&link);
    tokio::task::spawn_blocking(move || l.set_url(&body.url))
        .await?
        .map_err(|e| ApiError::new(StatusCode::BAD_REQUEST, e))?;
    Ok(Json(link.status()).into_response())
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PairBody {
    #[serde(default)]
    pub url: Option<String>,
    pub code: String,
    #[serde(default)]
    pub name: Option<String>,
}

/// Default name of this PC on the server.
fn default_pc_name() -> String {
    crate::guard::machine_names()
        .into_iter()
        .next()
        .map(|n| crate::hub::auth::clean_name(&n, "PC"))
        .unwrap_or_else(|| "PC".to_string())
}

pub async fn pair_route(
    State(state): State<Arc<AppState>>,
    body: axum::body::Bytes,
) -> Result<Response, ApiError> {
    let link = link_of(&state)?;
    let body: PairBody = serde_json::from_slice(&body).map_err(|_| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "code attendu : {\"url\": \"…\", \"code\": \"…\"}",
        )
    })?;
    let url = match body.url.or_else(|| link.file().map(|f| f.url)) {
        Some(u) => u,
        None => {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "indique d'abord l'adresse du serveur",
            ))
        }
    };
    let name = body.name.unwrap_or_else(default_pc_name);
    let st = Arc::clone(&state);
    let status = tokio::task::spawn_blocking(move || pair_blocking(&st, &url, &body.code, &name))
        .await?
        .map_err(|(code, msg)| ApiError::new(code, msg))?;
    link.wake.notify_one();
    Ok(Json(status).into_response())
}

pub async fn sync_route(State(state): State<Arc<AppState>>) -> Result<Response, ApiError> {
    let link = link_of(&state)?;
    if !link.is_paired() {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "ce PC n'est pas appairé",
        ));
    }
    link.wake.notify_one();
    Ok((StatusCode::ACCEPTED, Json(link.status())).into_response())
}

pub async fn resend_route(State(state): State<Arc<AppState>>) -> Result<Response, ApiError> {
    let link = link_of(&state)?;
    if !link.is_paired() {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "ce PC n'est pas appairé",
        ));
    }
    link.force_next_round();
    link.wake.notify_one();
    Ok((StatusCode::ACCEPTED, Json(link.status())).into_response())
}

pub async fn delete_link(State(state): State<Arc<AppState>>) -> Result<Response, ApiError> {
    link_of(&state)?;
    let st = Arc::clone(&state);
    let revoked = tokio::task::spawn_blocking(move || unpair_blocking(&st))
        .await?
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(json!({ "revoked_on_server": revoked })).into_response())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_typed_by_the_driver() {
        assert_eq!(
            normalise_url("192.168.1.50").unwrap(),
            "http://192.168.1.50:8471"
        );
        assert_eq!(
            normalise_url(" http://192.168.1.50:9000/ ").unwrap(),
            "http://192.168.1.50:9000"
        );
        assert_eq!(
            normalise_url("corde.local").unwrap(),
            "http://corde.local:8471"
        );
        assert_eq!(
            normalise_url("https://pi.home:443").unwrap(),
            "https://pi.home:443"
        );
        assert_eq!(
            normalise_url("[fd00::5]:8471").unwrap(),
            "http://[fd00::5]:8471"
        );
        for bad in [
            "",
            "ftp://x",
            "http://x/api",
            "http://u:p@x",
            "http://x?y",
            "a b",
        ] {
            assert!(normalise_url(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn the_link_file_never_shows_the_token() {
        let f = LinkFile {
            url: "http://192.168.1.50:8471".into(),
            token: Some("f".repeat(64)),
            ..Default::default()
        };
        let dbg = format!("{f:?}");
        assert!(!dbg.contains("ffff"), "{dbg}");
        let dir =
            std::env::temp_dir().join(format!("corde-link-{}-{}", std::process::id(), now_ms()));
        std::fs::create_dir_all(&dir).unwrap();
        let link = HubLink::open(&dir);
        assert!(!link.is_paired());
        link.save(Some(f)).unwrap();
        let st = link.status();
        assert!(st.paired);
        let json = serde_json::to_string(&st).unwrap();
        assert!(!json.contains("ffff"), "{json}");
        let again = HubLink::open(&dir);
        assert!(again.is_paired());
        assert_eq!(again.status().state, LinkState::Connected);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(dir.join(LINK_FILE))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o077, 0);
        }
        // A new address forgets the old pairing.
        again.set_url("192.168.1.51").unwrap();
        assert!(!again.is_paired());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
