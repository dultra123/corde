//! The « Courses » page: the online race schedule of Le Mans Ultimate, the
//! races the driver picked and their reminders.
//!
//! ```text
//! GET    /api/races/sources?force=1             {enabled, sources: [SourceSnapshot]} — the two public pages
//!                                               (racecontrol.gg, lmu.tools) as text; nothing fetched while
//!                                               the `online_schedule` setting is off (see `sources`)
//! POST   /api/races/sources/{source}/unreadable the UI found no race in that page: the server keeps the copy
//!                                               it served in diagnostics/races/<source>.html → {kept}
//! GET    /api/races/picks                       Pick[] (started less than 3 h ago), by start
//! PUT    /api/races/picks/{id}                  body Pick → 201 new / 200 replaced · 400 invalid · 409 > 50
//! DELETE /api/races/picks/{id}                  204 / 404
//! POST   /api/races/picks/sync                  body PickSet (this device's copy: picks and removals,
//!                                               each with the instant of its change) → the merged
//!                                               PickSet · 400 not an object, a list too long, > 256 KiB
//! GET    /api/races/settings                    RaceSettings
//! PUT    /api/races/settings                    body: some keys of RaceSettings → RaceSettings · 400 invalid
//! ```
//!
//! The parsing of the pages is done by the UI (`packages/ui/src/races/`); the
//! server only fetches them (closed list) and keeps the picks. A task sends
//! `race_reminder` on the WebSocket when a reminder is due ([`reminders`]).
//!
//! « Mes courses » are shared between the devices (ADR 0019): the phone application sends its
//! copy to every server it knows, the PC sends its own to the home server
//! (`hub_link::run_picks_periodic`), and each one keeps the merge ([`merge`]). The settings
//! of the page are not shared.
//! The game itself is never contacted for this, and nothing is sent to the
//! sites but a plain `GET`: registration happens in the game.

pub mod merge;
pub mod reminders;
pub mod sources;
pub mod store;

use crate::api::ApiError;
use crate::state::AppState;
use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use corde_protocol::ServerMessage;
use merge::PickSet;
use serde::Deserialize;
use serde_json::json;
use sources::RaceSources;
use std::path::Path as FsPath;
use std::sync::Arc;
use std::time::Duration;
use store::{Pick, RaceStore};

/// How often the reminder task looks at the picks.
pub const REMINDER_TICK: Duration = Duration::from_secs(5);

/// Everything the races routes need, held by [`AppState`].
pub struct Races {
    pub sources: RaceSources,
    pub store: RaceStore,
    /// Notified when the picks changed on this server: the PC then sends its copy to the
    /// home server without waiting (`hub_link::run_picks_periodic`).
    pub changed: tokio::sync::Notify,
}

impl Races {
    pub fn open(data_dir: &FsPath) -> Self {
        Self {
            sources: RaceSources::default(),
            store: RaceStore::open(data_dir),
            changed: tokio::sync::Notify::new(),
        }
    }
}

/// Milliseconds since the epoch, now.
pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn store_error(e: std::io::Error) -> ApiError {
    match e.kind() {
        std::io::ErrorKind::InvalidInput => ApiError::new(StatusCode::BAD_REQUEST, e.to_string()),
        std::io::ErrorKind::StorageFull => ApiError::new(StatusCode::CONFLICT, e.to_string()),
        _ => ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

#[derive(Debug, Deserialize)]
pub struct SourcesQuery {
    #[serde(default)]
    force: Option<String>,
}

pub async fn get_sources(
    State(state): State<Arc<AppState>>,
    Query(q): Query<SourcesQuery>,
) -> Result<Response, ApiError> {
    let force = matches!(q.force.as_deref(), Some("1" | "true"));
    let st = Arc::clone(&state);
    let enabled = tokio::task::spawn_blocking(move || st.settings.get().online_schedule).await?;
    let sources = state.races.sources.snapshot(enabled, force, now_ms()).await;
    Ok(Json(json!({ "enabled": enabled, "sources": sources })).into_response())
}

pub async fn post_unreadable(
    State(state): State<Arc<AppState>>,
    Path(source): Path<String>,
) -> Result<Response, ApiError> {
    if sources::source(&source).is_none() {
        return Err(ApiError::new(StatusCode::NOT_FOUND, "source inconnue"));
    }
    let kept = state
        .races
        .sources
        .keep_unreadable(&source, &state.data_dir)
        .await
        .map_err(store_error)?;
    Ok(Json(json!({
        "kept": kept.is_some(),
        "file": format!("{}/{source}.html", sources::DIAGNOSTICS_DIR),
    }))
    .into_response())
}

pub async fn list_picks(State(state): State<Arc<AppState>>) -> Result<Response, ApiError> {
    let picks = tokio::task::spawn_blocking(move || state.races.store.picks(now_ms())).await?;
    Ok(Json(picks).into_response())
}

pub async fn put_pick(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    body: Bytes,
) -> Result<Response, ApiError> {
    if !store::valid_pick_id(&id) {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "identifiant de choix invalide",
        ));
    }
    let pick = Pick::from_request(&id, &body).map_err(store_error)?;
    let saved = pick.clone();
    let st = Arc::clone(&state);
    let new = tokio::task::spawn_blocking(move || st.races.store.put_pick(pick, now_ms()))
        .await?
        .map_err(store_error)?;
    state.races.changed.notify_one();
    let code = if new {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };
    Ok((code, Json(saved)).into_response())
}

pub async fn delete_pick(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    if !store::valid_pick_id(&id) {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "identifiant de choix invalide",
        ));
    }
    let st = Arc::clone(&state);
    let removed = tokio::task::spawn_blocking(move || st.races.store.delete_pick(&id, now_ms()))
        .await?
        .map_err(store_error)?;
    if removed {
        state.races.changed.notify_one();
        Ok(StatusCode::NO_CONTENT.into_response())
    } else {
        Err(ApiError::new(StatusCode::NOT_FOUND, "choix inconnu"))
    }
}

/// `POST /api/races/picks/sync`: another device (the phone application, the PC) sends its
/// copy of « Mes courses »; this server merges it into its own and answers the result.
pub async fn sync_picks(
    State(state): State<Arc<AppState>>,
    body: Bytes,
) -> Result<Response, ApiError> {
    let (remote, skipped) = PickSet::from_wire(&body).map_err(store_error)?;
    if skipped > 0 {
        tracing::info!(
            "courses : {skipped} élément(s) non reconnu(s) d'un autre appareil, laissés de côté"
        );
    }
    let st = Arc::clone(&state);
    let (merged, changed) =
        tokio::task::spawn_blocking(move || st.races.store.merge_set(remote, now_ms()))
            .await?
            .map_err(store_error)?;
    if changed {
        tracing::info!(
            "courses : « Mes courses » mises à jour depuis un autre appareil ({} choisie(s))",
            merged.picks.len()
        );
        state.races.changed.notify_one();
    }
    Ok(Json(merged).into_response())
}

pub async fn get_settings(State(state): State<Arc<AppState>>) -> Result<Response, ApiError> {
    Ok(Json(state.races.store.settings()).into_response())
}

pub async fn put_settings(
    State(state): State<Arc<AppState>>,
    body: Bytes,
) -> Result<Response, ApiError> {
    let st = Arc::clone(&state);
    let saved = tokio::task::spawn_blocking(move || st.races.store.patch_settings(&body, now_ms()))
        .await?
        .map_err(store_error)?;
    // The hour correction moves picks: the other devices must follow.
    state.races.changed.notify_one();
    Ok(Json(saved).into_response())
}

/// Send the reminders that are due, once. Returns how many were sent.
pub fn send_due_reminders(state: &AppState, now_ms: i64) -> usize {
    let (picks, leads, fired) = state.races.store.reminder_input();
    let due = reminders::due_reminders(&picks, &leads, now_ms, &fired);
    if due.is_empty() {
        return 0;
    }
    let keys: Vec<String> = due.iter().map(|d| d.key.clone()).collect();
    // Marked first: a crash between the two loses a reminder rather than
    // sending it twice.
    state.races.store.mark_fired(&keys);
    for d in &due {
        tracing::info!(
            "rappel de course : {} ({}) à {}",
            d.reminder.name,
            d.key,
            d.reminder.start_ms
        );
        state.broadcast(&ServerMessage::RaceReminder(d.reminder.clone()));
    }
    due.len()
}

/// The reminder task: every [`REMINDER_TICK`] until shutdown.
pub async fn run_periodic(state: Arc<AppState>, mut shutdown: tokio::sync::watch::Receiver<bool>) {
    let mut tick = tokio::time::interval(REMINDER_TICK);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            _ = tick.tick() => {
                let st = Arc::clone(&state);
                let _ = tokio::task::spawn_blocking(move || send_due_reminders(&st, now_ms())).await;
            }
            _ = shutdown.changed() => {
                if *shutdown.borrow() { break; }
            }
        }
    }
}
