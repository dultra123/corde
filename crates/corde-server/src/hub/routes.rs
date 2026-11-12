//! `/api/hub/*` — the home server's own routes.
//!
//! ```text
//! GET    /api/hub/pairing            public: {role:"hub", version, paired, device?}
//! POST   /api/hub/pair               public: {code, name?, kind?: "browser"|"app"} → 201 {device}
//!                                    + cookie (browser) or {token} (app) · 400 wrong / expired /
//!                                    no code · 429 too many attempts
//! GET    /api/hub/status             device: version, counts, last PC that synchronised
//! GET    /api/hub/devices            device: [DeviceInfo + current]
//! PATCH  /api/hub/devices/{id}       device: {name?, admin?} — itself, or any as administrator
//! DELETE /api/hub/devices/{id}       device: itself, or any as administrator (immediate)
//! POST   /api/hub/local/pairing-code local control: {admin?} → {code, expires_in_s, admin}
//! GET    /api/hub/local/devices      local control: [DeviceInfo]
//! DELETE /api/hub/local/devices/{id} local control: revoke
//! ```

use super::auth::{cookie_clear, cookie_set, DeviceCtx, DeviceKind, PairError};
use crate::api::ApiError;
use crate::state::{AppState, SERVER_VERSION};
use axum::extract::{ConnectInfo, Path, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::sync::Arc;

type AppResult = Result<Response, ApiError>;

fn hub_of(state: &AppState) -> Result<Arc<super::Hub>, ApiError> {
    state
        .hub
        .clone()
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "not a home server"))
}

fn no_store(mut r: Response) -> Response {
    r.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    r
}

pub async fn pairing_state(
    State(state): State<Arc<AppState>>,
    dev: Option<Extension<DeviceCtx>>,
) -> AppResult {
    hub_of(&state)?;
    let device = dev.map(|Extension(d)| json!({ "id": d.id, "name": d.name, "admin": d.admin }));
    Ok(no_store(
        Json(json!({
            "role": "hub",
            "version": SERVER_VERSION,
            "paired": device.is_some(),
            "device": device,
        }))
        .into_response(),
    ))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PairBody {
    pub code: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub kind: Option<DeviceKind>,
}

fn pair_error(e: &PairError) -> Response {
    let status = match e {
        PairError::RateLimited { .. } => StatusCode::TOO_MANY_REQUESTS,
        PairError::Io(_) => StatusCode::INTERNAL_SERVER_ERROR,
        PairError::TooManyDevices => StatusCode::CONFLICT,
        _ => StatusCode::BAD_REQUEST,
    };
    let mut r = (
        status,
        Json(json!({ "error": e.message_fr(), "code": e.code() })),
    )
        .into_response();
    if let PairError::RateLimited { retry_after_s } = e {
        if let Ok(v) = HeaderValue::from_str(&retry_after_s.to_string()) {
            r.headers_mut().insert(header::RETRY_AFTER, v);
        }
    }
    no_store(r)
}

pub async fn pair(
    State(state): State<Arc<AppState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    body: axum::body::Bytes,
) -> AppResult {
    let hub = hub_of(&state)?;
    let body: PairBody = serde_json::from_slice(&body)
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "demande d'appairage invalide"))?;
    if body.code.len() > 32 {
        return Err(ApiError::new(StatusCode::BAD_REQUEST, "code invalide"));
    }
    let kind = body.kind.unwrap_or_default();
    let name = body.name.unwrap_or_default();
    let result =
        tokio::task::spawn_blocking(move || hub.auth.pair(peer.ip(), &body.code, &name, kind))
            .await?;
    match result {
        Ok((device, token)) => {
            let body = match kind {
                DeviceKind::App => json!({ "device": device, "token": token }),
                DeviceKind::Browser => json!({ "device": device }),
            };
            let mut r = (StatusCode::CREATED, Json(body)).into_response();
            if kind == DeviceKind::Browser {
                if let Ok(v) = HeaderValue::from_str(&cookie_set(&token)) {
                    r.headers_mut().insert(header::SET_COOKIE, v);
                }
            }
            Ok(no_store(r))
        }
        Err(e) => Ok(pair_error(&e)),
    }
}

/// `GET /api/hub/status`.
#[derive(Debug, Serialize)]
struct HubStatus {
    version: &'static str,
    schema_version: i64,
    sync_version: u32,
    uptime_s: f64,
    sessions: i64,
    results: i64,
    devices: usize,
    last_pc: Option<super::LastPc>,
    you: DeviceCtxOut,
}

#[derive(Debug, Serialize)]
struct DeviceCtxOut {
    id: String,
    name: String,
    admin: bool,
}

pub async fn status(
    State(state): State<Arc<AppState>>,
    Extension(dev): Extension<DeviceCtx>,
) -> AppResult {
    let hub = hub_of(&state)?;
    let store = crate::api::store_of(&state)?;
    let (sessions, results) = tokio::task::spawn_blocking(move || -> Result<_, ApiError> {
        let db = store.db();
        Ok((db.session_count()?, db.result_count()?))
    })
    .await??;
    Ok(Json(HubStatus {
        version: SERVER_VERSION,
        schema_version: corde_store::db::SCHEMA_VERSION,
        sync_version: crate::sync_wire::SYNC_VERSION,
        uptime_s: state.uptime_s(),
        sessions,
        results,
        devices: hub.auth.list().len(),
        last_pc: hub.last_pc(),
        you: DeviceCtxOut {
            id: dev.id,
            name: dev.name,
            admin: dev.admin,
        },
    })
    .into_response())
}

pub async fn list_devices(
    State(state): State<Arc<AppState>>,
    Extension(dev): Extension<DeviceCtx>,
) -> AppResult {
    let hub = hub_of(&state)?;
    let list: Vec<Value> = hub
        .auth
        .list()
        .into_iter()
        .map(|d| {
            let current = d.id == dev.id;
            let mut v = serde_json::to_value(d).unwrap_or(Value::Null);
            if let Value::Object(m) = &mut v {
                m.insert("current".into(), Value::Bool(current));
            }
            v
        })
        .collect();
    Ok(no_store(Json(json!({ "devices": list })).into_response()))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DevicePatch {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub admin: Option<bool>,
}

pub async fn patch_device(
    State(state): State<Arc<AppState>>,
    Extension(dev): Extension<DeviceCtx>,
    Path(id): Path<String>,
    body: axum::body::Bytes,
) -> AppResult {
    let hub = hub_of(&state)?;
    let patch: DevicePatch = serde_json::from_slice(&body).map_err(|_| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "modification invalide : {\"name\": \"…\"} ou {\"admin\": true|false}",
        )
    })?;
    if id != dev.id && !dev.admin {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "seul un appareil administrateur peut modifier un autre appareil",
        ));
    }
    if patch.admin.is_some() && !dev.admin {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "seul un appareil administrateur peut changer les droits",
        ));
    }
    if patch.admin == Some(false) {
        let admins = hub.auth.list().iter().filter(|d| d.admin).count();
        let target_is_admin = hub.auth.get(&id).is_some_and(|d| d.admin);
        if target_is_admin && admins <= 1 {
            return Err(ApiError::new(
                StatusCode::CONFLICT,
                "il faut garder au moins un appareil administrateur",
            ));
        }
    }
    let updated = tokio::task::spawn_blocking(move || {
        hub.auth.update(&id, patch.name.as_deref(), patch.admin)
    })
    .await??;
    match updated {
        Some(d) => Ok(Json(d).into_response()),
        None => Err(ApiError::new(StatusCode::NOT_FOUND, "appareil inconnu")),
    }
}

pub async fn delete_device(
    State(state): State<Arc<AppState>>,
    Extension(dev): Extension<DeviceCtx>,
    Path(id): Path<String>,
) -> AppResult {
    let hub = hub_of(&state)?;
    if id != dev.id && !dev.admin {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "seul un appareil administrateur peut révoquer un autre appareil",
        ));
    }
    let own = id == dev.id;
    let removed = tokio::task::spawn_blocking(move || hub.auth.revoke(&id)).await??;
    if !removed {
        return Err(ApiError::new(StatusCode::NOT_FOUND, "appareil inconnu"));
    }
    let mut r = StatusCode::NO_CONTENT.into_response();
    if own {
        if let Ok(v) = HeaderValue::from_str(&cookie_clear()) {
            r.headers_mut().insert(header::SET_COOKIE, v);
        }
    }
    Ok(no_store(r))
}

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct CodeBody {
    #[serde(default)]
    pub admin: bool,
}

pub async fn local_pairing_code(
    State(state): State<Arc<AppState>>,
    body: axum::body::Bytes,
) -> AppResult {
    let hub = hub_of(&state)?;
    let body: CodeBody = if body.is_empty() {
        CodeBody::default()
    } else {
        serde_json::from_slice(&body)
            .map_err(|e| ApiError::new(StatusCode::BAD_REQUEST, e.to_string()))?
    };
    let issued = hub.auth.issue_code(body.admin);
    Ok(no_store(Json(issued).into_response()))
}

pub async fn local_devices(State(state): State<Arc<AppState>>) -> AppResult {
    let hub = hub_of(&state)?;
    Ok(no_store(Json(hub.auth.list()).into_response()))
}

pub async fn local_revoke(State(state): State<Arc<AppState>>, Path(id): Path<String>) -> AppResult {
    let hub = hub_of(&state)?;
    let removed = tokio::task::spawn_blocking(move || hub.auth.revoke(&id)).await??;
    if removed {
        Ok(StatusCode::NO_CONTENT.into_response())
    } else {
        Err(ApiError::new(StatusCode::NOT_FOUND, "appareil inconnu"))
    }
}
