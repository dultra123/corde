//! `/api/sync/*` — what a paired PC sends the home server (protocol: [`crate::sync_wire`]).
//!
//! Everything is written inside the data folder: the database, `sessions/<id>/` and
//! `sync-incoming/<id>/<name>.part` for a file not complete yet. Identifiers and file names
//! are checked before any disk access.

// The handlers answer `Result<Response, Response>` (an early refusal is a full response):
// the size of `Response` does not matter here.
#![allow(clippy::result_large_err)]

use super::auth::DeviceCtx;
use super::LastPc;
use crate::api::{store_of, ApiError};
use crate::state::{AppState, SERVER_VERSION};
use crate::sync_wire::{
    self, FileCommit, Hello, Manifest, SessionEntry, CHUNK_MAX, FILE_MAX, HEADER_SCHEMA_VERSION,
    HEADER_SYNC_VERSION, MIN_SYNC_VERSION, SYNC_VERSION,
};
use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use corde_store::sync::{valid_sample_file_name, KIND_RESULT, KIND_SESSION};
use corde_store::{FeedbackSet, ResultBundle, SessionBundle};
use serde::Deserialize;
use serde_json::json;
use std::io::Write;
use std::sync::Arc;

fn conflict(code: &str, msg: String) -> Response {
    (
        StatusCode::CONFLICT,
        Json(json!({ "error": msg, "code": code })),
    )
        .into_response()
}

/// The two version headers: a PC newer than this server is refused with a message that says
/// what to update.
fn check_versions(headers: &HeaderMap) -> Result<(), Response> {
    let read = |name: &str| {
        headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.trim().parse::<i64>().ok())
    };
    let (Some(sync), Some(schema)) = (read(HEADER_SYNC_VERSION), read(HEADER_SCHEMA_VERSION))
    else {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "en-têtes de version de synchronisation absents",
                "code": "bad_request",
            })),
        )
            .into_response());
    };
    let sync = u32::try_from(sync).unwrap_or(0);
    match sync_wire::incompatibility(sync, schema, SERVER_VERSION) {
        Some((code, msg)) => Err(conflict(code, msg)),
        None => Ok(()),
    }
}

fn valid_session_id(id: &str) -> Result<(), ApiError> {
    if corde_store::valid_id(id) {
        Ok(())
    } else {
        Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "identifiant de session invalide",
        ))
    }
}

fn valid_file(name: &str) -> Result<(), ApiError> {
    if valid_sample_file_name(name) {
        Ok(())
    } else {
        Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "nom de fichier invalide",
        ))
    }
}

/// Addresses the PC says it serves the live view at: `http://host[:port]` only, few, short.
fn clean_urls(urls: &[String]) -> Vec<String> {
    urls.iter()
        .filter(|u| {
            u.len() <= 100
                && u.starts_with("http://")
                && crate::net::split_host_port(&u["http://".len()..]).is_some()
        })
        .take(8)
        .cloned()
        .collect()
}

pub async fn hello(
    State(state): State<Arc<AppState>>,
    Extension(dev): Extension<DeviceCtx>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, Response> {
    check_versions(&headers)?;
    let hello: Hello = serde_json::from_slice(&body).map_err(|e| {
        ApiError::new(StatusCode::BAD_REQUEST, format!("hello invalide : {e}")).into_response()
    })?;
    let hub = state
        .hub
        .clone()
        .ok_or_else(|| StatusCode::NOT_FOUND.into_response())?;
    let store = store_of(&state).map_err(IntoResponse::into_response)?;
    hub.set_last_pc(LastPc {
        device_id: dev.id.clone(),
        device_name: dev.name.clone(),
        version: hello.version.chars().take(32).collect(),
        urls: clean_urls(&hello.urls),
        synced_ms: corde_store::db::now_ms(),
    });
    let manifest = tokio::task::spawn_blocking(move || -> Result<Manifest, ApiError> {
        let db = store.db();
        let fingerprints = db.sync_received(KIND_SESSION)?;
        let mut files = db.sync_files()?;
        let mut uids = db.feedback_uids_by_session()?;
        let mut sessions = std::collections::BTreeMap::new();
        let mut ids: std::collections::BTreeSet<String> = fingerprints.keys().cloned().collect();
        ids.extend(files.keys().cloned());
        for id in ids {
            let u = uids.remove(&id).unwrap_or_default();
            sessions.insert(
                id.clone(),
                SessionEntry {
                    fingerprint: fingerprints.get(&id).cloned(),
                    files: files.remove(&id).unwrap_or_default(),
                    feedback: u.feedback,
                    iterations: u.iterations,
                },
            );
        }
        Ok(Manifest {
            server_version: SERVER_VERSION.to_string(),
            sync_version: SYNC_VERSION,
            min_sync_version: MIN_SYNC_VERSION,
            schema_version: corde_store::db::SCHEMA_VERSION,
            sessions,
            results: db.sync_received(KIND_RESULT)?,
            tombstones: db.tombstones()?.into_iter().collect(),
        })
    })
    .await
    .map_err(|e| ApiError::from(e).into_response())?
    .map_err(IntoResponse::into_response)?;
    tracing::info!(
        "sync: hello from {} ({}), Corde {}",
        dev.id,
        dev.name,
        hello.version
    );
    Ok(Json(manifest).into_response())
}

pub async fn put_result(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, Response> {
    check_versions(&headers)?;
    let store = store_of(&state).map_err(IntoResponse::into_response)?;
    let r = tokio::task::spawn_blocking(move || -> Result<String, ApiError> {
        let (bundle, fp): (ResultBundle, String) =
            sync_wire::decompress_json(&body).map_err(|e| {
                ApiError::new(StatusCode::BAD_REQUEST, format!("résultat illisible : {e}"))
            })?;
        let db = store.db();
        db.import_result(&bundle)?;
        db.set_sync_received(KIND_RESULT, &bundle.row.file_name, &fp)?;
        Ok(fp)
    })
    .await
    .map_err(|e| ApiError::from(e).into_response())?
    .map_err(IntoResponse::into_response)?;
    Ok(Json(json!({ "fingerprint": r })).into_response())
}

#[derive(Debug, Deserialize)]
pub struct ChunkQuery {
    pub offset: u64,
    pub total: u64,
    /// Start the file again (the partial one is dropped).
    #[serde(default)]
    pub reset: Option<String>,
}

fn part_path(state: &AppState, id: &str, name: &str) -> Result<std::path::PathBuf, ApiError> {
    let hub = state
        .hub
        .clone()
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "not a home server"))?;
    Ok(hub.incoming_dir().join(id).join(format!("{name}.part")))
}

pub async fn put_chunk(
    State(state): State<Arc<AppState>>,
    Path((id, name)): Path<(String, String)>,
    Query(q): Query<ChunkQuery>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, Response> {
    check_versions(&headers)?;
    valid_session_id(&id).map_err(IntoResponse::into_response)?;
    valid_file(&name).map_err(IntoResponse::into_response)?;
    if body.len() > CHUNK_MAX || q.total > FILE_MAX || q.offset + body.len() as u64 > q.total {
        return Err(
            ApiError::new(StatusCode::BAD_REQUEST, "morceau de fichier invalide").into_response(),
        );
    }
    let hub = state
        .hub
        .clone()
        .ok_or_else(|| StatusCode::NOT_FOUND.into_response())?;
    let part = part_path(&state, &id, &name).map_err(IntoResponse::into_response)?;
    let _lock = hub.uploads.lock().await;
    let reset = matches!(q.reset.as_deref(), Some("1" | "true"));
    let written = tokio::task::spawn_blocking(move || -> Result<Result<u64, u64>, ApiError> {
        if let Some(dir) = part.parent() {
            std::fs::create_dir_all(dir)?;
        }
        if reset {
            let _ = std::fs::remove_file(&part);
        }
        let have = std::fs::metadata(&part).map(|m| m.len()).unwrap_or(0);
        if q.offset != have {
            return Ok(Err(have));
        }
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&part)?;
        f.write_all(&body)?;
        Ok(Ok(have + body.len() as u64))
    })
    .await
    .map_err(|e| ApiError::from(e).into_response())?
    .map_err(IntoResponse::into_response)?;
    match written {
        Ok(received) => Ok(Json(json!({ "received": received })).into_response()),
        Err(expected) => Ok((
            StatusCode::CONFLICT,
            Json(json!({
                "error": "reprise du fichier à un autre endroit",
                "code": "offset",
                "expected_offset": expected,
            })),
        )
            .into_response()),
    }
}

pub async fn commit_file(
    State(state): State<Arc<AppState>>,
    Path((id, name)): Path<(String, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, Response> {
    check_versions(&headers)?;
    valid_session_id(&id).map_err(IntoResponse::into_response)?;
    valid_file(&name).map_err(IntoResponse::into_response)?;
    let commit: FileCommit = serde_json::from_slice(&body).map_err(|_| {
        ApiError::new(StatusCode::BAD_REQUEST, "validation de fichier invalide").into_response()
    })?;
    let hub = state
        .hub
        .clone()
        .ok_or_else(|| StatusCode::NOT_FOUND.into_response())?;
    let store = store_of(&state).map_err(IntoResponse::into_response)?;
    let part = part_path(&state, &id, &name).map_err(IntoResponse::into_response)?;
    let _lock = hub.uploads.lock().await;
    let result = tokio::task::spawn_blocking(move || -> Result<Option<String>, ApiError> {
        let size = std::fs::metadata(&part).map(|m| m.len()).unwrap_or(0);
        let sha = if size == commit.size {
            crate::crypto::sha256_file(&part).ok()
        } else {
            None
        };
        if sha.as_deref() != Some(commit.sha256.as_str()) {
            let _ = std::fs::remove_file(&part);
            tracing::warn!("sync: {id}/{name} — checksum or size mismatch, partial file dropped");
            return Ok(None);
        }
        let dir = store.session_dir(&id)?;
        std::fs::create_dir_all(&dir)?;
        std::fs::rename(&part, dir.join(&name))?;
        if let Some(parent) = part.parent() {
            let _ = std::fs::remove_dir(parent);
        }
        store
            .db()
            .set_sync_file(&id, &name, &commit.sha256, commit.size as i64)?;
        Ok(Some(commit.sha256))
    })
    .await
    .map_err(|e| ApiError::from(e).into_response())?
    .map_err(IntoResponse::into_response)?;
    match result {
        Some(sha) => Ok(Json(json!({ "sha256": sha })).into_response()),
        None => Ok((
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "error": "somme de contrôle différente : fichier à renvoyer",
                "code": "checksum",
            })),
        )
            .into_response()),
    }
}

pub async fn put_session(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, Response> {
    check_versions(&headers)?;
    valid_session_id(&id).map_err(IntoResponse::into_response)?;
    let store = store_of(&state).map_err(IntoResponse::into_response)?;
    enum Outcome {
        Stored(String),
        Missing(Vec<String>),
        Deleted,
    }
    let outcome = tokio::task::spawn_blocking(move || -> Result<Outcome, ApiError> {
        let (bundle, fp): (SessionBundle, String) =
            sync_wire::decompress_json(&body).map_err(|e| {
                ApiError::new(StatusCode::BAD_REQUEST, format!("session illisible : {e}"))
            })?;
        if bundle.session.id != id {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "identifiant de session différent",
            ));
        }
        if store.db().is_tombstoned(&id)? {
            return Ok(Outcome::Deleted);
        }
        let dir = store.session_dir(&id)?;
        let missing: Vec<String> = bundle
            .sample_files
            .iter()
            .filter(|f| !dir.join(&f.name).is_file())
            .map(|f| f.name.clone())
            .collect();
        if !missing.is_empty() {
            return Ok(Outcome::Missing(missing));
        }
        let db = store.db();
        db.import_session(&bundle)?;
        db.set_sync_received(KIND_SESSION, &id, &fp)?;
        Ok(Outcome::Stored(fp))
    })
    .await
    .map_err(|e| ApiError::from(e).into_response())?
    .map_err(IntoResponse::into_response)?;
    match outcome {
        Outcome::Stored(fp) => Ok(Json(json!({ "fingerprint": fp })).into_response()),
        Outcome::Missing(names) => Ok((
            StatusCode::CONFLICT,
            Json(json!({
                "error": "fichiers d'échantillons absents",
                "code": "missing_files",
                "missing": names,
            })),
        )
            .into_response()),
        Outcome::Deleted => Ok((
            StatusCode::GONE,
            Json(json!({
                "error": "session supprimée sur le serveur : elle n'est plus reçue",
                "code": "deleted_on_server",
            })),
        )
            .into_response()),
    }
}

pub async fn get_feedback(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, Response> {
    check_versions(&headers)?;
    valid_session_id(&id).map_err(IntoResponse::into_response)?;
    let store = store_of(&state).map_err(IntoResponse::into_response)?;
    let set = tokio::task::spawn_blocking(move || store.db().export_feedback(&id))
        .await
        .map_err(|e| ApiError::from(e).into_response())?
        .map_err(|e| ApiError::from(e).into_response())?;
    Ok(Json(set).into_response())
}

pub async fn post_feedback(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, Response> {
    check_versions(&headers)?;
    valid_session_id(&id).map_err(IntoResponse::into_response)?;
    let set: FeedbackSet = serde_json::from_slice(&body).map_err(|e| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            format!("ressentis illisibles : {e}"),
        )
        .into_response()
    })?;
    let store = store_of(&state).map_err(IntoResponse::into_response)?;
    let report = tokio::task::spawn_blocking(move || store.db().merge_feedback(&id, &set))
        .await
        .map_err(|e| ApiError::from(e).into_response())?
        .map_err(|e| ApiError::from(e).into_response())?;
    Ok(Json(report).into_response())
}
