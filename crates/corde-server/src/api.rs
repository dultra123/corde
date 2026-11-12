//! HTTP API and router.
//!
//! ```text
//! GET    /api/health         {"ok":true,"version":…,"uptime_s":…,"llm":"dictionary"|"anthropic"}
//! GET    /api/status         SourceStatus
//! GET    /api/frame          TelemetryFrame (404 {"error":"no_frame"} until the first one)
//! GET    /api/laps           LapRecord[] of the current session
//! GET    /api/layouts        DashboardLayout[]
//! PUT    /api/layouts        body DashboardLayout[] → full replacement; body DashboardLayout → upsert
//! DELETE /api/layouts        remove every layout
//! GET    /api/layouts/{id}   DashboardLayout
//! PUT    /api/layouts/{id}   upsert (the body's id is overridden by the path)
//! DELETE /api/layouts/{id}   204 / 404
//! GET    /api/sessions?limit=&track=&type=         SessionRow[] (most recent first)
//! GET    /api/sessions/{id}                        session + laps + events + setups + files
//! GET    /api/sessions/{id}/laps/{lap}/samples?channels=speed_kmh,throttle&points=2000
//!                                                  {t_s:[…], lap_dist_m:[…], channels:[{name,unit,values}]}
//! DELETE /api/sessions/{id}                        204 / 404 / 409 (being recorded)
//! GET    /api/sessions/{id}/analysis?base=         AnalysisReport (corde-engineer), computed on demand and
//!                                                  cached in `analyses`; 404 unknown session, 422 not enough data;
//!                                                  plans resolved against `base` / the proposed .svm when known
//! POST   /api/sessions/{id}/analysis/recompute     drop the cache and compute again → AnalysisReport
//! GET    /api/sessions/{id}/feedback               Feedback[] (what the driver felt, oldest first, with the
//!                                                  engineer's interpretation when there is one)
//! POST   /api/sessions/{id}/feedback               body NewFeedback {plan?: "aggressive"|"safe"|"wet", text}
//!                                                  → 201 {feedback, interpretation, iteration}: the text is read
//!                                                  into symptoms (dictionary, or LLM when enabled), stored, and
//!                                                  a PlanIteration is built on the plan tried (or the last
//!                                                  iterated plan of the session) and the session's analysis
//!                                                  · 400 text empty / > 2000 chars or bad plan · 404 unknown session
//! GET    /api/sessions/{id}/iterations?plan=&base= PlanIteration[] (oldest first; `plan` filters one philosophy;
//!                                                  resolved against `base` / the proposed setup, see `setups`)
//! GET    /api/setups?session=                      SetupLibrary: the driver's .svm files for the session's car
//!                                                  and the base proposed (see `setups`)
//! GET    /api/sessions/{id}/setup-preview?base=&plan=  SetupPreview (plan resolved against a .svm, nothing written)
//! POST   /api/sessions/{id}/setup-files            body NewSetupFiles {base, plans} → 201 SetupFilesCreated: writes
//!                                                  Corde_<Plan>_<base>.svm next to the base, never overwrites
//!                                                  · 403 not sent from this PC · 409 game folder unknown
//! GET    /api/recording                            RecordingStatus
//! POST   /api/recording/start | /stop              force / stop the recorder → RecordingStatus
//! GET    /api/storage                              {path, sessions, files, db_bytes, samples_bytes, total_bytes}
//! GET    /api/channels                             [{name, unit, kind}] — the Parquet columns
//! GET    /api/profile                              {player_name, totals, tracks, vehicles, recent, import}
//! POST   /api/profile/import                       scan the results folder now → import status
//! GET    /api/results?track=&kind=&vehicle=&limit= ResultRow[] (imported results files, most recent first)
//! GET    /api/results/{id}                         result + player laps + parsed file + linked_session_id
//! GET    /api/settings                             {lan_access, online_schedule} — `<data-dir>/settings.json`
//! PUT    /api/settings                             body {lan_access?: bool, online_schedule?: bool}
//!                                                  → 200 {lan_access, online_schedule, restart_required}
//!                                                  · 400 not an object / wrong type / unknown key
//!                                                  · 403 not sent from this PC (see `net::may_change_settings`)
//! GET    /api/network                              {lan_access_setting, listening_on_lan, port, urls}
//! GET    /api/races/sources · /picks · /settings   online race schedule, picks, reminders (see `races`)
//! GET    /ws                 WebSocket (see `ws`)
//! /*                         static UI with SPA fallback, or a small landing page
//! ```
//!
//! Every request goes through [`crate::guard`] (Host, Origin, routes "this PC
//! only", body ≤ 1 MiB): see `docs/SECURITY-MODEL.md`.
//!
//! Without a usable store (database failed to open) the `/api/sessions*`,
//! `/api/recording*`, `/api/storage`, `/api/profile*` and `/api/results*`
//! routes answer 503.

use crate::results_import::{linked_session, ImportStatus, ResultsImporter, ScanContext};
use crate::state::{AppState, SERVER_VERSION};
use crate::ws::ws_handler;
use axum::body::Bytes;
use axum::extract::{ConnectInfo, DefaultBodyLimit, Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{any, get, post};
use axum::{Json, Router};
use corde_engineer::IterationContext;
use corde_protocol::{
    AnalysisReport, DashboardLayout, Feedback, FeedbackInterpretation, NewFeedback, PlanIteration,
    SetupPhilosophy,
};
use corde_store::{
    ProfileSummary, RecorderHandle, ResultDetail, ResultFilter, SessionFilter, Store, StoreError,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;

type AppResult = Result<Response, ApiError>;

/// `{"error": "..."}` with a status code.
#[derive(Debug)]
pub struct ApiError(StatusCode, String);

impl ApiError {
    pub(crate) fn new(code: StatusCode, msg: impl Into<String>) -> Self {
        Self(code, msg.into())
    }
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn status(&self) -> StatusCode {
        self.0
    }
    fn not_found(what: &str) -> Self {
        Self(StatusCode::NOT_FOUND, what.to_string())
    }
    fn bad_request(msg: impl Into<String>) -> Self {
        Self(StatusCode::BAD_REQUEST, msg.into())
    }
}

impl From<std::io::Error> for ApiError {
    fn from(e: std::io::Error) -> Self {
        match e.kind() {
            std::io::ErrorKind::InvalidInput => Self::bad_request(e.to_string()),
            _ => Self(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
        }
    }
}

impl From<tokio::task::JoinError> for ApiError {
    fn from(e: tokio::task::JoinError) -> Self {
        Self(StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    }
}

impl From<corde_engineer::EngineerError> for ApiError {
    fn from(e: corde_engineer::EngineerError) -> Self {
        use corde_engineer::EngineerError as E;
        match e {
            E::Store(s) => Self::from(s),
            E::NotEnoughData(msg) => Self(StatusCode::UNPROCESSABLE_ENTITY, msg),
            other => Self(StatusCode::INTERNAL_SERVER_ERROR, other.to_string()),
        }
    }
}

impl From<StoreError> for ApiError {
    fn from(e: StoreError) -> Self {
        let code = match &e {
            StoreError::NotFound(_) => StatusCode::NOT_FOUND,
            StoreError::Invalid { .. } => StatusCode::BAD_REQUEST,
            StoreError::Busy(_) => StatusCode::CONFLICT,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        Self(code, e.to_string())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(json!({ "error": self.1 }))).into_response()
    }
}

pub fn router(state: Arc<AppState>, ui_dir: Option<PathBuf>) -> Router {
    router_with(state, ui_dir, &[])
}

/// The routes of the home server (`--role hub`): pairing, devices, receiving a PC's data.
fn hub_routes(api: Router<Arc<AppState>>) -> Router<Arc<AppState>> {
    use crate::hub::{receive, routes};
    use crate::sync_wire::{BUNDLE_MAX, CHUNK_MAX};
    use axum::routing::{patch, put};
    let bundle_limit = DefaultBodyLimit::max(BUNDLE_MAX);
    let chunk_limit = DefaultBodyLimit::max(CHUNK_MAX + 64 * 1024);
    api.route("/api/hub/pairing", get(routes::pairing_state))
        .route("/api/hub/pair", post(routes::pair))
        .route("/api/hub/status", get(routes::status))
        .route("/api/hub/devices", get(routes::list_devices))
        .route(
            "/api/hub/devices/{id}",
            patch(routes::patch_device).delete(routes::delete_device),
        )
        .route(
            "/api/hub/local/pairing-code",
            post(routes::local_pairing_code),
        )
        .route("/api/hub/local/devices", get(routes::local_devices))
        .route(
            "/api/hub/local/devices/{id}",
            axum::routing::delete(routes::local_revoke),
        )
        .route("/api/sync/hello", post(receive::hello))
        .route(
            "/api/sync/results",
            post(receive::put_result).layer(bundle_limit),
        )
        .route(
            "/api/sync/files/{id}/{name}",
            put(receive::put_chunk).layer(chunk_limit),
        )
        .route(
            "/api/sync/files/{id}/{name}/commit",
            post(receive::commit_file),
        )
        .route(
            "/api/sync/sessions/{id}",
            put(receive::put_session).layer(bundle_limit),
        )
        .route(
            "/api/sync/sessions/{id}/feedback",
            get(receive::get_feedback)
                .post(receive::post_feedback)
                .layer(bundle_limit),
        )
}

/// The routes of the PC's link to its home server.
fn link_routes(api: Router<Arc<AppState>>) -> Router<Arc<AppState>> {
    use crate::hub_link as l;
    api.route(
        "/api/hub-link",
        get(l::get_link).put(l::put_link).delete(l::delete_link),
    )
    .route("/api/hub-link/pair", post(l::pair_route))
    .route("/api/hub-link/sync", post(l::sync_route))
    .route("/api/hub-link/resend", post(l::resend_route))
}

/// [`router`] with extra host names this server answers to (`--allow-host`).
pub fn router_with(state: Arc<AppState>, ui_dir: Option<PathBuf>, allow_host: &[String]) -> Router {
    let api = Router::new()
        .route("/api/health", get(health))
        .route("/api/status", get(status))
        .route("/api/frame", get(frame))
        .route("/api/laps", get(laps))
        .route(
            "/api/layouts",
            get(list_layouts).put(put_layouts).delete(delete_layouts),
        )
        .route(
            "/api/layouts/{id}",
            get(get_layout).put(put_layout).delete(delete_layout),
        )
        .route("/api/sessions", get(list_sessions))
        .route(
            "/api/sessions/{id}",
            get(get_session).delete(delete_session),
        )
        .route("/api/sessions/{id}/laps/{lap}/samples", get(lap_samples))
        .route("/api/sessions/{id}/analysis", get(get_analysis))
        .route(
            "/api/sessions/{id}/analysis/recompute",
            post(recompute_analysis),
        )
        .route(
            "/api/sessions/{id}/feedback",
            get(list_feedback).post(add_feedback),
        )
        .route("/api/sessions/{id}/iterations", get(list_iterations))
        .route(
            "/api/sessions/{id}/setup-preview",
            get(crate::setups::setup_preview),
        )
        .route(
            "/api/sessions/{id}/setup-files",
            post(crate::setups::create_setup_files),
        )
        .route("/api/setups", get(crate::setups::list_setups))
        .route("/api/recording", get(recording))
        .route("/api/recording/start", post(recording_start))
        .route("/api/recording/stop", post(recording_stop))
        .route("/api/storage", get(storage))
        .route("/api/channels", get(channels))
        .route("/api/profile", get(profile))
        .route("/api/profile/import", post(profile_import))
        .route("/api/results", get(list_results))
        .route("/api/results/{id}", get(get_result))
        .route("/api/settings", get(get_settings).put(put_settings))
        .route("/api/network", get(network))
        .route("/api/races/sources", get(crate::races::get_sources))
        .route(
            "/api/races/sources/{source}/unreadable",
            post(crate::races::post_unreadable),
        )
        .route("/api/races/picks", get(crate::races::list_picks))
        .route(
            "/api/races/picks/{id}",
            axum::routing::put(crate::races::put_pick).delete(crate::races::delete_pick),
        )
        .route(
            "/api/races/settings",
            get(crate::races::get_settings).put(crate::races::put_settings),
        )
        .route("/ws", any(ws_handler));
    let api = if state.is_hub() {
        hub_routes(api)
    } else {
        link_routes(api)
    };

    let app = match ui_dir {
        Some(dir) => {
            let index = ServeFile::new(dir.join("index.html"));
            api.fallback_service(ServeDir::new(&dir).fallback(index))
        }
        None => api.fallback(landing),
    };

    // Outermost last: the guard (Host, Origin, PC-only routes) sees every
    // request before CORS answers a preflight. See `guard` and
    // docs/SECURITY-MODEL.md.
    // On the home server, the paired-device check runs right after the guard.
    let policy = Arc::new(crate::guard::HostPolicy::for_server(
        state.listen,
        allow_host,
        state.is_hub(),
    ));
    app.layer(axum::middleware::from_fn(static_cache_policy))
        .layer(DefaultBodyLimit::max(crate::guard::MAX_BODY_BYTES))
        .layer(axum::middleware::from_fn_with_state(
            Arc::clone(&state),
            crate::hub::hub_guard,
        ))
        .layer(crate::guard::cors_layer())
        .layer(axum::middleware::from_fn_with_state(
            policy,
            crate::guard::guard,
        ))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// `Cache-Control` of the web UI's files.
///
/// Without it a browser falls back on *heuristic* freshness (a tenth of the age of
/// `Last-Modified`): after an update the embedded webview, a phone or an OBS browser source kept
/// showing the previous interface — `index.html` and the bundles it names, all from the cache —
/// for minutes to days, while the API already answered with the new version.
///
/// * `/assets/*` (file names carry a content hash): cached for good;
/// * every other UI file (`index.html`, the SPA fallback, icons, manifest): always revalidated;
/// * `/api/*` and `/ws`: untouched.
pub(crate) fn static_cache_control(path: &str) -> Option<&'static str> {
    if path == "/ws" || path == "/api" || path.starts_with("/api/") {
        None
    } else if path.starts_with("/assets/") {
        Some("public, max-age=31536000, immutable")
    } else {
        Some("no-cache")
    }
}

async fn static_cache_policy(
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    let path = req.uri().path().to_owned();
    let Some(value) = static_cache_control(&path) else {
        return next.run(req).await;
    };
    let mut res = next.run(req).await;
    if path.starts_with("/assets/") {
        // A bundle that is not there (a previous version's) gets the SPA fallback from the
        // file service: HTML under a script's address, which would then be cached for good.
        let is_html = res
            .headers()
            .get(header::CONTENT_TYPE)
            .is_some_and(|v| v.as_bytes().starts_with(b"text/html"));
        if is_html || !(res.status().is_success() || res.status() == StatusCode::NOT_MODIFIED) {
            return (
                StatusCode::NOT_FOUND,
                [(header::CACHE_CONTROL, "no-store")],
                "not found",
            )
                .into_response();
        }
    }
    res.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static(value),
    );
    res
}

async fn health(State(state): State<Arc<AppState>>) -> Json<Value> {
    Json(json!({
        "ok": true,
        "version": SERVER_VERSION,
        "protocol_version": corde_protocol::PROTOCOL_VERSION,
        "uptime_s": state.uptime_s(),
        "llm": state.engineer.llm,
        "role": if state.is_hub() { "hub" } else { "desktop" },
    }))
}

async fn status(State(state): State<Arc<AppState>>) -> Json<corde_protocol::SourceStatus> {
    Json(state.status(state.default_rate as f32))
}

async fn frame(State(state): State<Arc<AppState>>) -> AppResult {
    match state.latest_frame() {
        Some(slot) => Ok(json_response(serde_json::to_string(&slot.frame).map_err(
            |e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
        )?)),
        None => Err(ApiError::not_found("no_frame")),
    }
}

fn json_response(body: String) -> Response {
    ([(header::CONTENT_TYPE, "application/json")], body).into_response()
}

async fn laps(State(state): State<Arc<AppState>>) -> Json<Vec<corde_protocol::LapRecord>> {
    Json(state.laps())
}

async fn list_layouts(State(state): State<Arc<AppState>>) -> Json<Vec<DashboardLayout>> {
    Json(state.layouts.list())
}

async fn put_layouts(State(state): State<Arc<AppState>>, Json(body): Json<Value>) -> AppResult {
    match body {
        Value::Array(_) => {
            let layouts: Vec<DashboardLayout> = serde_json::from_value(body)
                .map_err(|e| ApiError::bad_request(format!("invalid layouts: {e}")))?;
            let stored =
                tokio::task::spawn_blocking(move || state.layouts.replace_all(layouts)).await??;
            Ok(Json(stored).into_response())
        }
        Value::Object(_) => {
            let layout: DashboardLayout = serde_json::from_value(body)
                .map_err(|e| ApiError::bad_request(format!("invalid layout: {e}")))?;
            let stored =
                tokio::task::spawn_blocking(move || state.layouts.upsert(layout)).await??;
            Ok(Json(stored).into_response())
        }
        _ => Err(ApiError::bad_request(
            "expected a DashboardLayout or an array of them",
        )),
    }
}

async fn delete_layouts(State(state): State<Arc<AppState>>) -> AppResult {
    let n = tokio::task::spawn_blocking(move || state.layouts.clear()).await??;
    Ok(Json(json!({ "deleted": n })).into_response())
}

async fn get_layout(State(state): State<Arc<AppState>>, Path(id): Path<String>) -> AppResult {
    match state.layouts.get(&id) {
        Some(l) => Ok(Json(l).into_response()),
        None => Err(ApiError::not_found("layout_not_found")),
    }
}

async fn put_layout(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(mut layout): Json<DashboardLayout>,
) -> AppResult {
    if !crate::layouts::valid_id(&id) {
        return Err(ApiError::bad_request("invalid layout id"));
    }
    layout.id = id;
    let stored = tokio::task::spawn_blocking(move || state.layouts.upsert(layout)).await??;
    Ok(Json(stored).into_response())
}

async fn delete_layout(State(state): State<Arc<AppState>>, Path(id): Path<String>) -> AppResult {
    let existed = tokio::task::spawn_blocking(move || state.layouts.delete(&id)).await??;
    if existed {
        Ok(StatusCode::NO_CONTENT.into_response())
    } else {
        Err(ApiError::not_found("layout_not_found"))
    }
}

// --- sessions / recording ----------------------------------------------------

pub(crate) fn store_of(state: &AppState) -> Result<Arc<Store>, ApiError> {
    state.store.clone().ok_or(ApiError(
        StatusCode::SERVICE_UNAVAILABLE,
        "session store unavailable (database could not be opened)".into(),
    ))
}

fn recorder_of(state: &AppState) -> Result<Arc<RecorderHandle>, ApiError> {
    state.recorder.clone().ok_or(ApiError(
        StatusCode::SERVICE_UNAVAILABLE,
        "recorder unavailable (database could not be opened)".into(),
    ))
}

#[derive(Debug, Deserialize, Default)]
pub struct SessionsQuery {
    pub limit: Option<usize>,
    /// Case-insensitive substring of the track name.
    pub track: Option<String>,
    /// `race`, `qualify`, `practice`, `warmup`, `test_day`.
    #[serde(rename = "type")]
    pub session_type: Option<String>,
}

async fn list_sessions(
    State(state): State<Arc<AppState>>,
    Query(q): Query<SessionsQuery>,
) -> AppResult {
    let store = store_of(&state)?;
    let session_type = match q.session_type.as_deref().filter(|t| !t.is_empty()) {
        Some(t) => Some(
            serde_json::from_value::<corde_protocol::SessionType>(Value::String(t.to_string()))
                .map_err(|_| ApiError::bad_request(format!("unknown session type {t:?}")))?,
        ),
        None => None,
    };
    let filter = SessionFilter {
        track: q.track,
        session_type,
        limit: q.limit,
    };
    let rows = tokio::task::spawn_blocking(move || store.list_sessions(&filter)).await??;
    Ok(Json(rows).into_response())
}

async fn get_session(State(state): State<Arc<AppState>>, Path(id): Path<String>) -> AppResult {
    let store = store_of(&state)?;
    let detail = tokio::task::spawn_blocking(move || store.get_session(&id)).await??;
    Ok(Json(detail).into_response())
}

async fn delete_session(State(state): State<Arc<AppState>>, Path(id): Path<String>) -> AppResult {
    let store = store_of(&state)?;
    let hub = state.is_hub();
    tokio::task::spawn_blocking(move || -> Result<(), StoreError> {
        store.delete_session(&id)?;
        // On the home server, remembered so that the PC does not send it again.
        if hub {
            store.db().add_tombstone(&id)?;
            tracing::info!("session {id} deleted on the server (not received again)");
        }
        Ok(())
    })
    .await??;
    Ok(StatusCode::NO_CONTENT.into_response())
}

#[derive(Debug, Deserialize, Default)]
pub struct SamplesQuery {
    /// Comma-separated channel names (default `speed_kmh,throttle,brake`).
    pub channels: Option<String>,
    /// Maximum points per channel after decimation (default 2000).
    pub points: Option<usize>,
}

async fn lap_samples(
    State(state): State<Arc<AppState>>,
    Path((id, lap)): Path<(String, i32)>,
    Query(q): Query<SamplesQuery>,
) -> AppResult {
    let store = store_of(&state)?;
    let names: Vec<String> = q
        .channels
        .as_deref()
        .unwrap_or("speed_kmh,throttle,brake")
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    let points = q.points.unwrap_or(corde_store::query::DEFAULT_MAX_POINTS);
    let samples = tokio::task::spawn_blocking(move || {
        let refs: Vec<&str> = names.iter().map(String::as_str).collect();
        store.lap_samples(&id, lap, &refs, points)
    })
    .await??;
    Ok(Json(samples).into_response())
}

// --- analysis (corde-engineer) -----------------------------------------------

/// Cached report of a session, or a fresh one (cached unless the session is
/// still being recorded). `force` ignores the cache.
pub(crate) fn session_analysis(
    store: &Store,
    id: &str,
    force: bool,
) -> Result<AnalysisReport, ApiError> {
    if !corde_store::valid_id(id) {
        return Err(ApiError::bad_request(format!("invalid session id {id:?}")));
    }
    if !force {
        if let Some(row) = store.db().get_analysis(id)? {
            match serde_json::from_str::<AnalysisReport>(&row.report_json) {
                Ok(report) if report.engine_version >= corde_engineer::ENGINE_VERSION => {
                    return Ok(report)
                }
                Ok(report) => tracing::info!(
                    "cached analysis of {id} written by engine v{} (current v{}): recomputing",
                    report.engine_version,
                    corde_engineer::ENGINE_VERSION
                ),
                Err(e) => tracing::warn!("analyse en cache illisible pour {id} ({e}) : recalcul"),
            }
        }
    }
    let report =
        corde_engineer::analyse_session(store, id, &corde_engineer::AnalyseOptions::default())?;
    if store.is_active(id) {
        tracing::debug!("session {id} en cours d'enregistrement : analyse non mise en cache");
    } else {
        let json = serde_json::to_string(&report)
            .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        store
            .db()
            .put_analysis(id, report.generated_ms as i64, &json)?;
    }
    Ok(report)
}

/// `?base=Folder/File.svm`: the setup the plans are resolved against
/// (default: the base proposed for the session).
#[derive(Debug, Deserialize, Default)]
pub struct BaseQuery {
    pub base: Option<String>,
}

async fn get_analysis(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(q): Query<BaseQuery>,
) -> AppResult {
    let store = store_of(&state)?;
    let report = tokio::task::spawn_blocking(move || {
        let report = session_analysis(&store, &id, false)?;
        crate::setups::resolve_report(&state, &store, &id, report, q.base.as_deref())
    })
    .await??;
    Ok(Json(report).into_response())
}

async fn recompute_analysis(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(q): Query<BaseQuery>,
) -> AppResult {
    let store = store_of(&state)?;
    let report = tokio::task::spawn_blocking(move || {
        let report = session_analysis(&store, &id, true)?;
        crate::setups::resolve_report(&state, &store, &id, report, q.base.as_deref())
    })
    .await??;
    Ok(Json(report).into_response())
}

// --- driver feedback ---------------------------------------------------------

/// Longest accepted feedback, in characters.
pub const FEEDBACK_MAX_CHARS: usize = 2000;

/// Trims the text and checks its length; the plan is already typed by serde.
fn validate_feedback(body: Value) -> Result<NewFeedback, ApiError> {
    let mut f: NewFeedback = serde_json::from_value(body)
        .map_err(|e| ApiError::bad_request(format!("ressenti invalide : {e}")))?;
    f.text = f.text.trim().to_string();
    let n = f.text.chars().count();
    if n == 0 {
        return Err(ApiError::bad_request("le ressenti est vide"));
    }
    if n > FEEDBACK_MAX_CHARS {
        return Err(ApiError::bad_request(format!(
            "le ressenti dépasse {FEEDBACK_MAX_CHARS} caractères ({n})"
        )));
    }
    Ok(f)
}

async fn list_feedback(State(state): State<Arc<AppState>>, Path(id): Path<String>) -> AppResult {
    let store = store_of(&state)?;
    let rows = tokio::task::spawn_blocking(move || -> Result<_, StoreError> {
        let db = store.db();
        if !db.session_exists(&id)? {
            return Err(StoreError::not_found(format!("session {id}")));
        }
        db.list_feedback(&id)
    })
    .await??;
    Ok(Json(rows).into_response())
}

/// `POST /api/sessions/{id}/feedback` → 201.
#[derive(Debug, Serialize)]
struct FeedbackResponse {
    feedback: Feedback,
    interpretation: FeedbackInterpretation,
    iteration: PlanIteration,
}

/// Store the feedback, read it, and answer with the next plan iteration.
fn feedback_loop(
    state: &AppState,
    store: &Store,
    id: &str,
    new: &NewFeedback,
) -> Result<FeedbackResponse, ApiError> {
    let session = store
        .db()
        .get_session(id)?
        .ok_or_else(|| StoreError::not_found(format!("session {id}")))?;
    let mut feedback = store.db().add_feedback(id, new)?;
    let mut interpretation = state.engineer.extractor.extract(&new.text, None);
    interpretation.feedback_id = feedback.id;
    store
        .db()
        .set_feedback_interpretation(feedback.id, &interpretation)?;
    feedback.interpretation = Some(interpretation.clone());

    // The plan the loop runs on: the one tried, else the last iterated one, else safe.
    let philosophy = match new.plan {
        Some(p) => p,
        None => store
            .db()
            .list_iterations(id, None)?
            .last()
            .map(|it| it.base_plan)
            .unwrap_or(SetupPhilosophy::Safe),
    };
    let previous = store.db().last_iteration(id, philosophy)?;
    // The session's analysis, when it can be computed (422 while too few laps).
    let report = match session_analysis(store, id, false) {
        Ok(r) => Some(r),
        Err(e) => {
            tracing::debug!("ressenti sur {id} sans analyse ({}) ", e.1);
            None
        }
    };
    let base: Vec<corde_protocol::SetupChange> = match (&previous, &report) {
        (Some(_), _) => Vec::new(),
        (None, Some(r)) => match philosophy {
            SetupPhilosophy::Aggressive => r.plans.aggressive.changes.clone(),
            SetupPhilosophy::Safe => r.plans.safe.changes.clone(),
            SetupPhilosophy::Wet => r.plans.wet.changes.clone(),
        },
        (None, None) => Vec::new(),
    };
    // Parameters the driver's car does not let change are never proposed.
    let unavailable = crate::setups::unavailable_for(state, &session);
    let wet_session = report
        .as_ref()
        .map(|r| r.data_quality.wet_session)
        .unwrap_or(session.weather.rain > 0.2);
    let ctx = IterationContext {
        session_id: id,
        philosophy,
        vehicle_class: session.vehicle_class,
        wet_session,
        base: &base,
        previous: previous.as_ref(),
        latest_report: report.as_ref(),
        feedback_id: Some(feedback.id),
        unavailable_params: &unavailable,
    };
    let mut iteration =
        corde_engineer::iterate_plan(&ctx, &interpretation, &state.engineer.knowledge);
    // Stored relative; answered resolved against the driver's setup.
    store.db().add_iteration(&mut iteration)?;
    let iteration = crate::setups::resolve_one_iteration(state, &session, &iteration);
    Ok(FeedbackResponse {
        feedback,
        interpretation,
        iteration,
    })
}

async fn add_feedback(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> AppResult {
    let store = store_of(&state)?;
    let feedback = validate_feedback(body)?;
    let resp = tokio::task::spawn_blocking(move || feedback_loop(&state, &store, &id, &feedback))
        .await??;
    Ok((StatusCode::CREATED, Json(resp)).into_response())
}

#[derive(Debug, Deserialize, Default)]
pub struct IterationsQuery {
    /// `aggressive` | `safe` | `wet`.
    pub plan: Option<String>,
    /// Setup the iterations are resolved against (default: proposed base).
    pub base: Option<String>,
}

async fn list_iterations(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(q): Query<IterationsQuery>,
) -> AppResult {
    let store = store_of(&state)?;
    let plan = match q.plan.as_deref().filter(|p| !p.is_empty()) {
        Some(p) => Some(
            serde_json::from_value::<SetupPhilosophy>(Value::String(p.to_string()))
                .map_err(|_| ApiError::bad_request(format!("unknown plan {p:?}")))?,
        ),
        None => None,
    };
    let rows = tokio::task::spawn_blocking(move || -> Result<_, ApiError> {
        let rows = {
            let db = store.db();
            if !db.session_exists(&id)? {
                return Err(StoreError::not_found(format!("session {id}")).into());
            }
            db.list_iterations(&id, plan)?
        };
        crate::setups::resolve_iterations(&state, &store, &id, rows, q.base.as_deref())
    })
    .await??;
    Ok(Json(rows).into_response())
}

async fn recording(State(state): State<Arc<AppState>>) -> AppResult {
    let rec = recorder_of(&state)?;
    Ok(Json(rec.status()).into_response())
}

async fn recording_start(State(state): State<Arc<AppState>>) -> AppResult {
    let rec = recorder_of(&state)?;
    rec.request_start();
    Ok((StatusCode::ACCEPTED, Json(rec.status())).into_response())
}

async fn recording_stop(State(state): State<Arc<AppState>>) -> AppResult {
    let rec = recorder_of(&state)?;
    rec.request_stop();
    Ok((StatusCode::ACCEPTED, Json(rec.status())).into_response())
}

async fn storage(State(state): State<Arc<AppState>>) -> AppResult {
    let store = store_of(&state)?;
    let stats = tokio::task::spawn_blocking(move || store.storage_stats()).await??;
    Ok(Json(stats).into_response())
}

// --- results / profile -------------------------------------------------------

fn importer_of(state: &AppState) -> Result<Arc<ResultsImporter>, ApiError> {
    state.importer.clone().ok_or(ApiError(
        StatusCode::SERVICE_UNAVAILABLE,
        "results importer unavailable (database could not be opened)".into(),
    ))
}

/// `GET /api/profile`.
#[derive(Debug, Serialize)]
struct ProfileResponse {
    #[serde(flatten)]
    summary: ProfileSummary,
    import: ImportStatus,
}

async fn profile(State(state): State<Arc<AppState>>) -> AppResult {
    let store = store_of(&state)?;
    let import = if state.is_hub() {
        // The server reads no game file: the results come from the PC.
        ImportStatus {
            error: Some(
                "Sur le serveur Corde, les résultats arrivent du PC à chaque synchronisation."
                    .into(),
            ),
            ..Default::default()
        }
    } else {
        importer_of(&state)?.status()
    };
    let st = Arc::clone(&store);
    let summary = tokio::task::spawn_blocking(move || st.profile_summary()).await??;
    let import = if state.is_hub() {
        ImportStatus {
            total_imported: summary.totals.sessions,
            ..import
        }
    } else {
        import
    };
    Ok(Json(ProfileResponse { summary, import }).into_response())
}

/// `POST /api/profile/import`: scan now (re-examining skipped files) and
/// answer with the import status.
async fn profile_import(State(state): State<Arc<AppState>>) -> AppResult {
    let importer = importer_of(&state)?;
    let ctx = ScanContext {
        game_user_data: state.game_user_data(),
        live_player_name: state.live_player_name(),
    };
    let imp = Arc::clone(&importer);
    tokio::task::spawn_blocking(move || imp.scan(&ctx, true)).await?;
    Ok(Json(importer.status()).into_response())
}

#[derive(Debug, Deserialize, Default)]
pub struct ResultsQuery {
    pub limit: Option<usize>,
    /// Case-insensitive substring of the course or venue name.
    pub track: Option<String>,
    /// `race`, `qualify`, `practice`, `warmup`, `test_day` (`type` is accepted too).
    pub kind: Option<String>,
    #[serde(rename = "type")]
    pub kind_alias: Option<String>,
    /// Exact car model.
    pub vehicle: Option<String>,
}

async fn list_results(
    State(state): State<Arc<AppState>>,
    Query(q): Query<ResultsQuery>,
) -> AppResult {
    let store = store_of(&state)?;
    let kind = q.kind.or(q.kind_alias);
    let session_kind = match kind.as_deref().filter(|t| !t.is_empty()) {
        Some(t) => Some(
            serde_json::from_value::<corde_protocol::SessionType>(Value::String(t.to_string()))
                .map_err(|_| ApiError::bad_request(format!("unknown session kind {t:?}")))?,
        ),
        None => None,
    };
    let filter = ResultFilter {
        track: q.track,
        session_kind,
        vehicle: q.vehicle,
        limit: q.limit,
    };
    let rows = tokio::task::spawn_blocking(move || store.list_results(&filter)).await??;
    Ok(Json(rows).into_response())
}

/// `GET /api/results/{id}`.
#[derive(Debug, Serialize)]
struct ResultResponse {
    #[serde(flatten)]
    detail: ResultDetail,
    /// Recorded Corde session of the same track that started within ±10 min.
    linked_session_id: Option<String>,
}

async fn get_result(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> AppResult {
    let store = store_of(&state)?;
    let resp = tokio::task::spawn_blocking(move || -> Result<ResultResponse, StoreError> {
        let detail = store.get_result(id)?;
        let linked_session_id = linked_session(&store, &detail.row);
        Ok(ResultResponse {
            detail,
            linked_session_id,
        })
    })
    .await??;
    Ok(Json(resp).into_response())
}

// --- settings / network ------------------------------------------------------

async fn get_settings(State(state): State<Arc<AppState>>) -> AppResult {
    let settings = tokio::task::spawn_blocking(move || state.settings.get()).await?;
    Ok(Json(json!({
        "lan_access": settings.lan_access,
        "online_schedule": settings.online_schedule,
    }))
    .into_response())
}

/// What a `PUT /api/settings` body may change. Every key is optional: an
/// absent one keeps its value.
#[derive(Debug, Default, PartialEq, Eq)]
struct SettingsPatch {
    lan_access: Option<bool>,
    online_schedule: Option<bool>,
}

fn validate_settings(body: &[u8]) -> Result<SettingsPatch, ApiError> {
    let value: Value = serde_json::from_slice(body)
        .map_err(|e| ApiError::bad_request(format!("réglages invalides : {e}")))?;
    let Value::Object(map) = value else {
        return Err(ApiError::bad_request(
            "réglages invalides : un objet JSON est attendu, par exemple {\"lan_access\": true}",
        ));
    };
    let mut patch = SettingsPatch::default();
    for (key, value) in map {
        match (key.as_str(), value) {
            ("lan_access", Value::Bool(b)) => patch.lan_access = Some(b),
            ("online_schedule", Value::Bool(b)) => patch.online_schedule = Some(b),
            (k @ ("lan_access" | "online_schedule"), other) => {
                return Err(ApiError::bad_request(format!(
                    "réglage `{k}` invalide : true ou false attendu, reçu {other}"
                )))
            }
            (other, _) => {
                return Err(ApiError::bad_request(format!(
                    "réglage inconnu : `{other}`"
                )))
            }
        }
    }
    Ok(patch)
}

/// `PUT /api/settings`: saves the settings and tells whether a restart is
/// needed for the network part (the listening address is chosen at start-up).
async fn put_settings(
    State(state): State<Arc<AppState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    body: Bytes,
) -> AppResult {
    let origin = match headers.get(header::ORIGIN) {
        Some(v) => Some(v.to_str().unwrap_or("")),
        None => None,
    };
    // On the home server the hub guard already required an administrator device.
    if !state.is_hub() && !crate::net::may_change_settings(peer.ip(), origin) {
        return Err(ApiError(
            StatusCode::FORBIDDEN,
            "les réglages ne se changent que depuis le PC qui fait tourner Corde".into(),
        ));
    }
    let patch = validate_settings(&body)?;
    if state.is_hub() && patch.lan_access.is_some() {
        return Err(ApiError::bad_request(
            "sans objet sur le serveur Corde : il écoute toujours sur le réseau local (appareils appairés seulement)",
        ));
    }
    let listen = state.listen;
    let saved = tokio::task::spawn_blocking(move || {
        state.settings.update(|s| {
            if let Some(lan) = patch.lan_access {
                s.lan_access = lan;
            }
            if let Some(on) = patch.online_schedule {
                s.online_schedule = on;
            }
        })
    })
    .await??;
    let restart_required = saved.lan_access != crate::net::listens_on_lan(listen.ip());
    if patch.lan_access.is_some() {
        tracing::info!(
            "réglage enregistré : accès réseau local {}{}",
            if saved.lan_access {
                "activé"
            } else {
                "désactivé"
            },
            if restart_required {
                " (appliqué au prochain démarrage)"
            } else {
                ""
            }
        );
    }
    if let Some(on) = patch.online_schedule {
        tracing::info!(
            "réglage enregistré : lecture du planning des courses en ligne {}",
            if on { "activée" } else { "désactivée" }
        );
    }
    Ok(Json(json!({
        "lan_access": saved.lan_access,
        "online_schedule": saved.online_schedule,
        "restart_required": restart_required,
    }))
    .into_response())
}

async fn network(State(state): State<Arc<AppState>>) -> AppResult {
    let info = tokio::task::spawn_blocking(move || {
        crate::net::network_info(state.listen, &state.settings.get())
    })
    .await?;
    Ok(Json(info).into_response())
}

async fn channels() -> Json<Value> {
    Json(Value::Array(
        corde_store::CHANNELS
            .iter()
            .map(|c| json!({ "name": c.name, "unit": c.unit, "kind": format!("{:?}", c.kind).to_lowercase() }))
            .collect(),
    ))
}

/// Shown when no UI build is available: tells the user how to reach the API.
async fn landing(State(state): State<Arc<AppState>>) -> Html<String> {
    let s = state.status(state.default_rate as f32);
    Html(format!(
        r#"<!doctype html><html lang="fr"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>corde-server</title>
<style>body{{font:15px/1.5 system-ui,sans-serif;margin:2rem auto;max-width:42rem;padding:0 1rem;color:#eee;background:#141418}}code{{background:#26262c;padding:.1em .35em;border-radius:4px}}a{{color:#7cc4ff}}</style></head>
<body><h1>corde-server {version}</h1>
<p>Source : <code>{source}</code> — {state} · {clients} client(s) · entrée {rate:.0} Hz</p>
<p>Aucun tableau de bord n'est servi (pas de dossier UI : <code>--ui-dir</code>). L'API reste disponible :</p>
<ul><li><a href="/api/status">/api/status</a></li><li><a href="/api/frame">/api/frame</a></li><li><a href="/api/laps">/api/laps</a></li><li><a href="/api/layouts">/api/layouts</a></li><li><a href="/api/sessions">/api/sessions</a> (+ <code>/{{id}}/analysis</code>, <code>/{{id}}/feedback</code>, <code>/{{id}}/iterations</code>) · <a href="/api/recording">/api/recording</a> · <a href="/api/storage">/api/storage</a></li><li><a href="/api/profile">/api/profile</a> · <a href="/api/results">/api/results</a></li><li><a href="/api/settings">/api/settings</a> · <a href="/api/network">/api/network</a></li><li><a href="/api/health">/api/health</a></li><li>WebSocket <code>/ws</code></li></ul>
</body></html>"#,
        version = SERVER_VERSION,
        source = s.source,
        state = if s.connected {
            "connectée"
        } else {
            "en attente du jeu"
        },
        clients = s.clients,
        rate = s.input_rate_hz,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_body_is_validated() {
        assert_eq!(
            validate_settings(br#"{"lan_access": true}"#).unwrap(),
            SettingsPatch {
                lan_access: Some(true),
                ..Default::default()
            }
        );
        assert_eq!(
            validate_settings(br#"{"lan_access": false}"#).unwrap(),
            SettingsPatch {
                lan_access: Some(false),
                ..Default::default()
            }
        );
        assert_eq!(
            validate_settings(br#"{"online_schedule": true, "lan_access": false}"#).unwrap(),
            SettingsPatch {
                lan_access: Some(false),
                online_schedule: Some(true),
            }
        );
        // Nothing to change is fine: the caller gets the current settings back.
        assert_eq!(validate_settings(b"{}").unwrap(), SettingsPatch::default());
        for bad in [
            &b""[..],
            b"oui",
            b"true",
            b"[]",
            br#"{"lan_access": "true"}"#,
            br#"{"lan_access": 1}"#,
            br#"{"lan_access": null}"#,
            br#"{"lan_acces": true}"#,
            br#"{"online_schedule": "true"}"#,
            br#"{"online_schedule": null}"#,
        ] {
            let err = validate_settings(bad).unwrap_err();
            assert_eq!(
                err.0,
                StatusCode::BAD_REQUEST,
                "{:?}",
                String::from_utf8_lossy(bad)
            );
        }
    }
}
