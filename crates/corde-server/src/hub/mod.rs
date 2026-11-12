//! The home server (`corde-server --role hub`, ADR 0010).
//!
//! An always-on machine of the house (a Raspberry Pi 5) that keeps what the PC records and
//! serves it to the driver's paired devices, PC switched off. It never reads a game, never
//! records, never touches a game folder, and is never exposed to the Internet (access from
//! outside goes through the driver's own VPN, docs/ACCES-EXTERIEUR.md).
//!
//! * [`auth`] — pairing codes, device tokens, rate limits, revocation.
//! * [`routes`] — `/api/hub/*`: pairing state, typing the code, devices, status, and the
//!   local control routes the `corde-server pair | devices | revoke` commands use.
//! * [`receive`] — `/api/sync/*`: what a paired PC sends ([`crate::sync_wire`]).
//! * [`cli`] — those commands.
//! * [`hub_guard`] — the middleware: every `/api/*` and `/ws` request needs a paired device
//!   except the pairing state and the typed code ([`crate::guard::hub_route_access`]).

pub mod auth;
pub mod cli;
pub mod receive;
pub mod routes;

use crate::guard::{hub_route_access, HubAccess};
use crate::state::AppState;
use auth::{token_of, HubAuth};
use axum::extract::{ConnectInfo, MatchedPath, Request, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// File of the last PC that synchronised, under `<data>/hub/`.
pub const LAST_PC_FILE: &str = "last-pc.json";
/// Where partial sample files wait for their last chunk, under the data folder.
pub const INCOMING_DIR: &str = "sync-incoming";

/// The last PC that synchronised (shown on the server's « Direct » tab).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LastPc {
    pub device_id: String,
    pub device_name: String,
    pub version: String,
    /// Where it serves the live view on the local network (empty: on itself only).
    pub urls: Vec<String>,
    pub synced_ms: i64,
}

/// State of the home server, held by [`AppState::hub`].
pub struct Hub {
    pub auth: HubAuth,
    dir: PathBuf,
    data_dir: PathBuf,
    last_pc: Mutex<Option<LastPc>>,
    /// Serialises the writes of partial sample files.
    pub uploads: tokio::sync::Mutex<()>,
}

impl std::fmt::Debug for Hub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Hub").field("dir", &self.dir).finish()
    }
}

impl Hub {
    pub fn open(data_dir: &Path) -> std::io::Result<Self> {
        let auth = HubAuth::open(data_dir)?;
        let dir = auth.dir().to_path_buf();
        let last_pc = std::fs::read(dir.join(LAST_PC_FILE))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok());
        Ok(Self {
            auth,
            dir,
            data_dir: data_dir.to_path_buf(),
            last_pc: Mutex::new(last_pc),
            uploads: tokio::sync::Mutex::new(()),
        })
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn incoming_dir(&self) -> PathBuf {
        self.data_dir.join(INCOMING_DIR)
    }

    pub fn last_pc(&self) -> Option<LastPc> {
        self.last_pc.lock().ok().and_then(|g| g.clone())
    }

    pub fn set_last_pc(&self, pc: LastPc) {
        if let Ok(json) = serde_json::to_vec_pretty(&pc) {
            if let Err(e) = auth::write_private(&self.dir.join(LAST_PC_FILE), &json) {
                tracing::warn!("cannot save the last PC seen: {e}");
            }
        }
        if let Ok(mut g) = self.last_pc.lock() {
            *g = Some(pc);
        }
    }
}

fn is_api_path(path: &str) -> bool {
    path == "/api" || path.starts_with("/api/") || path == "/ws" || path.starts_with("/ws/")
}

fn refuse(status: StatusCode, code: &str, msg: &str) -> Response {
    let mut r = (status, Json(json!({ "error": msg, "code": code }))).into_response();
    r.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    r
}

/// Message of a 401: the page shows the pairing screen.
pub const UNPAIRED_MSG: &str =
    "cet appareil n'est pas appairé au serveur Corde (ou il a été révoqué) : entre un code d'appairage";

/// Home server only: who may call what ([`hub_route_access`]). Runs inside
/// [`crate::guard::guard`] (`Host`, `Origin`, sizes stay checked first). Inserts the
/// [`auth::DeviceCtx`] of the caller for the handlers.
pub async fn hub_guard(
    State(state): State<Arc<AppState>>,
    mut req: Request,
    next: Next,
) -> Response {
    let path = req.uri().path().to_string();
    let Some(hub) = state.hub.clone() else {
        return next.run(req).await;
    };
    if !is_api_path(&path) {
        return next.run(req).await;
    }
    let route = req
        .extensions()
        .get::<MatchedPath>()
        .map(|m| m.as_str().to_string())
        .unwrap_or(path);
    let access = hub_route_access(req.method(), &route);
    match access {
        HubAccess::LocalControl => {
            let peer_local = req
                .extensions()
                .get::<ConnectInfo<SocketAddr>>()
                .is_some_and(|c| crate::net::peer_is_local(c.0.ip()));
            let no_origin = req.headers().get(header::ORIGIN).is_none();
            let key_ok = req
                .headers()
                .get(header::AUTHORIZATION)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.strip_prefix("Bearer "))
                .is_some_and(|k| hub.auth.control_key_matches(k.trim()));
            if peer_local && no_origin && key_ok {
                next.run(req).await
            } else {
                refuse(
                    StatusCode::FORBIDDEN,
                    "local_only",
                    "réservé aux commandes lancées sur le serveur lui-même (sudo corde-server pair)",
                )
            }
        }
        HubAccess::Public => {
            if let Some(dev) = token_of(req.headers()).and_then(|t| hub.auth.authenticate(&t)) {
                req.extensions_mut().insert(dev);
            }
            next.run(req).await
        }
        HubAccess::Device | HubAccess::Admin | HubAccess::Refused(_) => {
            let Some(dev) = token_of(req.headers()).and_then(|t| hub.auth.authenticate(&t)) else {
                return refuse(StatusCode::UNAUTHORIZED, "unpaired", UNPAIRED_MSG);
            };
            if let HubAccess::Refused(msg) = access {
                return refuse(StatusCode::FORBIDDEN, "not_on_server", msg);
            }
            if access == HubAccess::Admin && !dev.admin {
                return refuse(
                    StatusCode::FORBIDDEN,
                    "admin_only",
                    "action réservée à un appareil administrateur du serveur Corde (page Appareils)",
                );
            }
            req.extensions_mut().insert(dev);
            next.run(req).await
        }
    }
}

/// Every minute: write the devices' « last used » dates (not at every request: the
/// server's storage is often an SD card). Once more on shutdown.
pub async fn run_periodic(state: Arc<AppState>, mut shutdown: tokio::sync::watch::Receiver<bool>) {
    let Some(hub) = state.hub.clone() else {
        return;
    };
    let mut tick = tokio::time::interval(Duration::from_secs(60));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            _ = tick.tick() => hub.auth.flush(),
            _ = shutdown.changed() => {
                hub.auth.flush();
                break;
            }
        }
    }
}
