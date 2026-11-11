//! The driver's real setups: library, base of a session, plans resolved
//! against it, and the `.svm` files Corde writes on request.
//!
//! ```text
//! GET  /api/setups?session=<id>                      SetupLibrary (files of the session's car, proposed base)
//! GET  /api/sessions/{id}/setup-preview?base=&plan=  SetupPreview (plan resolved, nothing written)
//!      plan = aggressive | safe | wet | iteration:<n> | iteration:<plan>:<n>
//! POST /api/sessions/{id}/setup-files                body NewSetupFiles {base, plans[]} → 201 SetupFilesCreated
//!                                                    · 400 bad body / path · 403 not from this PC
//!                                                    · 404 base not found · 409 UserData unknown
//! ```
//!
//! `GET …/analysis` and `GET …/iterations` take an optional `?base=` too:
//! their plans are resolved against that file (by default against the
//! proposed base) as soon as the game's `UserData` folder is known.
//!
//! Files are written **only** by `POST …/setup-files`, next to the base, as
//! `Corde_<Plan>_<base>.svm` (`_2`, `_3`… when the name exists: nothing is
//! ever overwritten), atomically, and every file written is logged.

use crate::api::{session_analysis, store_of, ApiError};
use crate::lmu_garage::load_bounds;
use crate::state::AppState;
use axum::body::Bytes;
use axum::extract::{ConnectInfo, Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use corde_engineer::setup::{
    build_setup, notes_for, resolve_iteration, resolve_report_plans, unavailable_params,
    ResolveInput,
};
use corde_lmu::garage::GarageBounds;
use corde_lmu::settings::setups_dir;
use corde_lmu::svm::Setup;
use corde_lmu::svm_library::{
    choose_base, file_info, resolve_relative, scan, track_folder, vehicle_matches, BaseReason,
    LibraryScan,
};
use corde_protocol::{
    AnalysisReport, CreatedSetupFile, NewSetupFiles, PlanIteration, SetupChange, SetupFileInfo,
    SetupFilesCreated, SetupLibrary, SetupPhilosophy, SetupPreview,
};
use corde_store::{SessionRow, Store, StoreError};
use serde::Deserialize;
use std::io;
use std::net::SocketAddr;
use std::path::{Path as FsPath, PathBuf};
use std::sync::Arc;

type AppResult = Result<Response, ApiError>;

/// Most plans written by one request.
pub const MAX_PLANS_PER_REQUEST: usize = 8;

fn bad(msg: impl Into<String>) -> ApiError {
    ApiError::new(StatusCode::BAD_REQUEST, msg)
}

fn user_data_unknown() -> ApiError {
    ApiError::new(
        StatusCode::CONFLICT,
        "dossier du jeu inconnu : lance Le Mans Ultimate une fois avec Corde ouvert (ou passe --game-user-data)",
    )
}

// ---------------------------------------------------------------------------
// Library of a session
// ---------------------------------------------------------------------------

/// The library seen from one session.
#[derive(Debug, Clone)]
pub struct SessionSetups {
    pub settings_dir: PathBuf,
    pub scan: LibraryScan,
    /// Files of the session's car: track folder first, then most recent first.
    pub candidates: Vec<SetupFileInfo>,
    pub track_folder: Option<String>,
    pub proposed: Option<(SetupFileInfo, BaseReason)>,
}

/// Files of the session's car and the base Corde proposes.
pub fn session_setups(user_data: &FsPath, session: &SessionRow) -> SessionSetups {
    let settings_dir = setups_dir(user_data);
    let lib = scan(&settings_dir);
    let mut folders: Vec<String> = lib.files.iter().map(|f| f.folder.clone()).collect();
    folders.sort();
    folders.dedup();
    let track = track_folder(&session.track_name, &folders);
    let mut candidates: Vec<SetupFileInfo> = lib
        .files
        .iter()
        .filter(|f| vehicle_matches(&session.vehicle_model, &f.vehicle))
        .cloned()
        .collect();
    let in_track = |f: &SetupFileInfo| {
        track
            .as_deref()
            .is_some_and(|t| t.eq_ignore_ascii_case(&f.folder))
    };
    candidates.sort_by(|a, b| {
        in_track(b)
            .cmp(&in_track(a))
            .then(b.mtime_ms.cmp(&a.mtime_ms))
            .then(a.path.cmp(&b.path))
    });
    // A file Corde wrote after the session started was not driven in it:
    // never propose it as this session's base (it stays in the list).
    let eligible: Vec<SetupFileInfo> = candidates
        .iter()
        .filter(|f| !(is_corde_file(f) && f.mtime_ms > session.started_wall_ms))
        .cloned()
        .collect();
    let proposed = choose_base(&eligible, track.as_deref(), session.ended_wall_ms)
        .map(|(f, why)| (f.clone(), why));
    SessionSetups {
        settings_dir,
        scan: lib,
        candidates,
        track_folder: track,
        proposed,
    }
}

/// Prefix of the files Corde writes (`Corde_<Plan>_<base>.svm`).
const FILE_PREFIX: &str = "Corde_";
/// Start of the `Notes` Corde writes in them.
const NOTES_PREFIX: &str = "Corde - ";
/// legacy name (Muret, before 0.3.0): files written under the former name are ours too.
const LEGACY_FILE_PREFIX: &str = "Muret_";
/// legacy name (Muret, before 0.3.0): notes written under the former name.
const LEGACY_NOTES_PREFIX: &str = "Muret - ";

/// A file written by Corde (`Corde_*.svm`, notes `Corde - …`), or under its
/// former name.
fn is_corde_file(f: &SetupFileInfo) -> bool {
    [FILE_PREFIX, LEGACY_FILE_PREFIX]
        .iter()
        .any(|p| f.file.starts_with(p))
        || [NOTES_PREFIX, LEGACY_NOTES_PREFIX]
            .iter()
            .any(|p| f.notes.starts_with(p))
}

fn reason_fr(why: BaseReason, folder: &str) -> String {
    match why {
        BaseReason::TrackFolderBeforeEnd => format!(
            "Le dernier réglage de cette voiture enregistré dans « {folder} » avant la fin de la session."
        ),
        BaseReason::OtherFolderBeforeEnd => format!(
            "Aucun réglage de cette voiture pour ce circuit : le dernier enregistré avant la session (dossier « {folder} »)."
        ),
        BaseReason::Latest => {
            "Tous les réglages de cette voiture sont plus récents que la session : le plus récent.".into()
        }
    }
}

/// What a plan is resolved against, loaded from disk.
pub struct SetupContext {
    pub settings_dir: PathBuf,
    pub base_info: SetupFileInfo,
    pub base: Setup,
    /// Every file with the same `VehicleClassSetting` as the base.
    pub library: Vec<Setup>,
    pub bounds: Option<GarageBounds>,
}

impl SetupContext {
    pub fn with_input<R>(&self, f: impl FnOnce(&ResolveInput) -> R) -> R {
        let lib: Vec<&Setup> = self.library.iter().collect();
        let input = ResolveInput {
            base: &self.base,
            library: &lib,
            bounds: self.bounds.as_ref(),
        };
        f(&input)
    }
}

/// The base of a session: `base` (`Folder/File.svm`) when given, else the
/// proposed one. `Ok(None)` when there is nothing to resolve against
/// (game never seen, no file for this car) and no base was asked for.
pub fn context_for(
    state: &AppState,
    session: &SessionRow,
    base: Option<&str>,
) -> Result<Option<SetupContext>, ApiError> {
    let base = base.map(str::trim).filter(|b| !b.is_empty());
    let Some(user_data) = state.game_user_data_or_located() else {
        return match base {
            Some(_) => Err(user_data_unknown()),
            None => Ok(None),
        };
    };
    let settings_dir = setups_dir(&user_data);
    let (base_info, base_setup, scan_files) = match base {
        Some(rel) => {
            let path = resolve_relative(&settings_dir, rel).map_err(|e| bad(e.to_string()))?;
            let (info, setup) = file_info(&path).ok_or_else(|| {
                ApiError::new(
                    StatusCode::NOT_FOUND,
                    format!("réglage « {rel} » introuvable ou illisible"),
                )
            })?;
            (info, setup, scan(&settings_dir).files)
        }
        None => {
            let lib = session_setups(&user_data, session);
            let Some((info, _)) = lib.proposed else {
                return Ok(None);
            };
            let path = settings_dir.join(&info.folder).join(&info.file);
            let Some((info, setup)) = file_info(&path) else {
                return Ok(None);
            };
            (info, setup, lib.scan.files)
        }
    };
    let library: Vec<Setup> = scan_files
        .iter()
        .filter(|f| f.vehicle == base_info.vehicle)
        .filter_map(|f| Setup::read(settings_dir.join(&f.folder).join(&f.file)).ok())
        .collect();
    let bounds = if vehicle_matches(&session.vehicle_model, &base_info.vehicle) {
        load_bounds(&state.data_dir, &session.vehicle_model)
    } else {
        None
    };
    Ok(Some(SetupContext {
        settings_dir,
        base_info,
        base: base_setup,
        library,
        bounds,
    }))
}

fn session_row(store: &Store, id: &str) -> Result<SessionRow, ApiError> {
    if !corde_store::valid_id(id) {
        return Err(bad(format!("invalid session id {id:?}")));
    }
    Ok(store
        .db()
        .get_session(id)?
        .ok_or_else(|| StoreError::not_found(format!("session {id}")))?)
}

/// The report with its plans resolved against the base (unchanged when
/// there is none).
pub fn resolve_report(
    state: &AppState,
    store: &Store,
    id: &str,
    mut report: AnalysisReport,
    base: Option<&str>,
) -> Result<AnalysisReport, ApiError> {
    let session = session_row(store, id)?;
    if let Some(ctx) = context_for(state, &session, base)? {
        let k = &state.engineer.knowledge;
        report.plans = ctx.with_input(|input| {
            resolve_report_plans(k, &report, input, Some(ctx.base_info.clone()))
        });
    }
    Ok(report)
}

/// Iterations resolved against the base (unchanged when there is none).
pub fn resolve_iterations(
    state: &AppState,
    store: &Store,
    id: &str,
    iterations: Vec<PlanIteration>,
    base: Option<&str>,
) -> Result<Vec<PlanIteration>, ApiError> {
    let session = session_row(store, id)?;
    match context_for(state, &session, base)? {
        Some(ctx) => {
            let k = &state.engineer.knowledge;
            Ok(ctx.with_input(|input| {
                iterations
                    .iter()
                    .map(|it| resolve_iteration(k, it, input))
                    .collect()
            }))
        }
        None => Ok(iterations),
    }
}

/// Parameters the proposed base does not let change (for a new iteration).
pub fn unavailable_for(state: &AppState, session: &SessionRow) -> Vec<String> {
    match context_for(state, session, None) {
        Ok(Some(ctx)) => {
            let k = &state.engineer.knowledge;
            ctx.with_input(|input| unavailable_params(k, input))
        }
        _ => Vec::new(),
    }
}

/// One iteration resolved against the proposed base (feedback answer).
pub fn resolve_one_iteration(
    state: &AppState,
    session: &SessionRow,
    it: &PlanIteration,
) -> PlanIteration {
    match context_for(state, session, None) {
        Ok(Some(ctx)) => {
            let k = &state.engineer.knowledge;
            ctx.with_input(|input| resolve_iteration(k, it, input))
        }
        _ => it.clone(),
    }
}

// ---------------------------------------------------------------------------
// Plans to write
// ---------------------------------------------------------------------------

/// `aggressive` | `safe` | `wet` | `iteration:<n>` | `iteration:<plan>:<n>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanRef {
    Plan(SetupPhilosophy),
    Iteration {
        plan: Option<SetupPhilosophy>,
        n: u32,
    },
}

fn philosophy(s: &str) -> Option<SetupPhilosophy> {
    match s {
        "aggressive" => Some(SetupPhilosophy::Aggressive),
        "safe" => Some(SetupPhilosophy::Safe),
        "wet" => Some(SetupPhilosophy::Wet),
        _ => None,
    }
}

fn philosophy_label(p: SetupPhilosophy) -> &'static str {
    match p {
        SetupPhilosophy::Aggressive => "Agressif",
        SetupPhilosophy::Safe => "Soft & sûr",
        SetupPhilosophy::Wet => "Pluie",
    }
}

fn philosophy_tag(p: SetupPhilosophy) -> &'static str {
    match p {
        SetupPhilosophy::Aggressive => "Agressif",
        SetupPhilosophy::Safe => "Sur",
        SetupPhilosophy::Wet => "Pluie",
    }
}

impl PlanRef {
    pub fn parse(s: &str) -> Result<Self, ApiError> {
        let s = s.trim();
        if let Some(p) = philosophy(s) {
            return Ok(Self::Plan(p));
        }
        let wrong = || {
            bad(format!(
                "plan inconnu « {s} » (aggressive, safe, wet, iteration:<n> ou iteration:<plan>:<n>)"
            ))
        };
        let rest = s.strip_prefix("iteration:").ok_or_else(wrong)?;
        let (plan, n) = match rest.split_once(':') {
            Some((p, n)) => (Some(philosophy(p).ok_or_else(wrong)?), n),
            None => (None, rest),
        };
        let n: u32 = n.parse().map_err(|_| wrong())?;
        if n == 0 {
            return Err(wrong());
        }
        Ok(Self::Iteration { plan, n })
    }
}

/// A plan ready to be written: what it is, its resolved changes.
struct ResolvedPlan {
    reference: String,
    label: String,
    tag: String,
    changes: Vec<SetupChange>,
    dropped: Vec<SetupChange>,
    warnings: Vec<String>,
}

fn resolved_plan(
    state: &AppState,
    store: &Store,
    id: &str,
    ctx: &SetupContext,
    plan: &str,
) -> Result<ResolvedPlan, ApiError> {
    let k = &state.engineer.knowledge;
    match PlanRef::parse(plan)? {
        PlanRef::Plan(p) => {
            let report = session_analysis(store, id, false)?;
            let plans = ctx.with_input(|input| {
                resolve_report_plans(k, &report, input, Some(ctx.base_info.clone()))
            });
            let pl = match p {
                SetupPhilosophy::Aggressive => plans.aggressive,
                SetupPhilosophy::Safe => plans.safe,
                SetupPhilosophy::Wet => plans.wet,
            };
            Ok(ResolvedPlan {
                reference: plan.trim().to_string(),
                label: philosophy_label(p).to_string(),
                tag: philosophy_tag(p).to_string(),
                changes: pl.changes,
                dropped: pl.dropped,
                warnings: pl.warnings_fr,
            })
        }
        PlanRef::Iteration { plan: p, n } => {
            let its = store.db().list_iterations(id, p)?;
            let it = its
                .iter()
                .rev()
                .find(|it| it.iteration == n)
                .ok_or_else(|| {
                    ApiError::new(
                        StatusCode::NOT_FOUND,
                        format!("itération {n} introuvable pour cette session"),
                    )
                })?;
            let r = ctx.with_input(|input| resolve_iteration(k, it, input));
            Ok(ResolvedPlan {
                reference: plan.trim().to_string(),
                label: format!("Itération {n} ({})", philosophy_label(it.base_plan)),
                tag: format!("Iteration{n}_{}", philosophy_tag(it.base_plan)),
                changes: r.changes,
                dropped: r.dropped,
                warnings: r.notes_fr,
            })
        }
    }
}

/// `BMW_LongBeach` from `BMW_LongBeach.svm`, without an earlier
/// `Corde_<tag>_` prefix (or the former name's), with safe characters only.
fn base_stem(file: &str) -> String {
    let mut stem = file
        .strip_suffix(".svm")
        .or_else(|| file.strip_suffix(".SVM"))
        .unwrap_or(file)
        .to_string();
    while let Some(rest) = stem
        .strip_prefix(FILE_PREFIX)
        .or_else(|| stem.strip_prefix(LEGACY_FILE_PREFIX))
    {
        stem = match rest.split_once('_') {
            Some((_, r)) if !r.is_empty() => r.to_string(),
            _ => rest.to_string(),
        };
        // `Corde_Iteration2_Sur_x`: drop the plan word too.
        for tag in ["Sur_", "Agressif_", "Pluie_"] {
            if let Some(r) = stem.strip_prefix(tag) {
                stem = r.to_string();
            }
        }
    }
    let clean: String = stem
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | ' ' | '.' | '(' | ')') {
                c
            } else {
                '_'
            }
        })
        .take(80)
        .collect();
    if clean.trim().is_empty() {
        "reglage".to_string()
    } else {
        clean
    }
}

/// `Corde_<tag>_<base>.svm`.
pub fn file_name_for(tag: &str, base_file: &str) -> String {
    format!("{FILE_PREFIX}{tag}_{}.svm", base_stem(base_file))
}

/// Write `bytes` as `dir/name`, or `name_2`, `name_3`… when it exists:
/// never overwrites. Atomic (temporary file, then link / rename).
pub fn write_new_file(dir: &FsPath, name: &str, bytes: &[u8]) -> io::Result<PathBuf> {
    let (stem, ext) = name.rsplit_once('.').unwrap_or((name, "svm"));
    for n in 1..1000 {
        let candidate = if n == 1 {
            name.to_string()
        } else {
            format!("{stem}_{n}.{ext}")
        };
        let target = dir.join(&candidate);
        if target.exists() {
            continue;
        }
        let tmp = dir.join(format!(".{candidate}.corde-tmp"));
        std::fs::write(&tmp, bytes)?;
        match std::fs::hard_link(&tmp, &target) {
            Ok(()) => {
                let _ = std::fs::remove_file(&tmp);
                return Ok(target);
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                let _ = std::fs::remove_file(&tmp);
                continue;
            }
            Err(_) => {
                // No hard links on this file system: check, then rename.
                if target.exists() {
                    let _ = std::fs::remove_file(&tmp);
                    continue;
                }
                std::fs::rename(&tmp, &target)?;
                return Ok(target);
            }
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        format!("trop de fichiers {name} dans {}", dir.display()),
    ))
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Default)]
pub struct LibraryQuery {
    pub session: Option<String>,
}

/// `GET /api/setups?session=<id>`.
pub async fn list_setups(
    State(state): State<Arc<AppState>>,
    Query(q): Query<LibraryQuery>,
) -> AppResult {
    let session_id = q.session.filter(|s| !s.is_empty());
    let store = match &session_id {
        Some(_) => Some(store_of(&state)?),
        None => None,
    };
    let lib = tokio::task::spawn_blocking(move || -> Result<SetupLibrary, ApiError> {
        let session = match (&session_id, &store) {
            (Some(id), Some(store)) => Some(session_row(store, id)?),
            _ => None,
        };
        let mut out = SetupLibrary {
            vehicle: session
                .as_ref()
                .map(|s| s.vehicle_model.clone())
                .unwrap_or_default(),
            track_name: session
                .as_ref()
                .map(|s| s.track_name.clone())
                .unwrap_or_default(),
            ..Default::default()
        };
        let Some(user_data) = state.game_user_data_or_located() else {
            return Ok(out);
        };
        out.user_data_known = true;
        match &session {
            Some(session) => {
                let lib = session_setups(&user_data, session);
                out.settings_dir = Some(lib.settings_dir.display().to_string());
                out.track_folder = lib.track_folder.clone();
                out.total_files = lib.scan.files.len() as u32;
                out.unreadable = lib.scan.unreadable.clone();
                out.proposed_reason_fr = lib
                    .proposed
                    .as_ref()
                    .map(|(f, why)| reason_fr(*why, &f.folder));
                out.proposed = lib.proposed.map(|(f, _)| f);
                out.candidates = lib.candidates;
                out.bounds_known = load_bounds(&state.data_dir, &session.vehicle_model).is_some();
            }
            None => {
                let dir = setups_dir(&user_data);
                let lib = scan(&dir);
                out.settings_dir = Some(dir.display().to_string());
                out.total_files = lib.files.len() as u32;
                out.unreadable = lib.unreadable;
                out.candidates = lib.files;
            }
        }
        Ok(out)
    })
    .await??;
    Ok(Json(lib).into_response())
}

#[derive(Debug, Deserialize, Default)]
pub struct PreviewQuery {
    pub base: Option<String>,
    pub plan: Option<String>,
}

fn preview(
    state: &AppState,
    store: &Store,
    id: &str,
    base: Option<&str>,
    plan: &str,
) -> Result<SetupPreview, ApiError> {
    let session = session_row(store, id)?;
    let ctx = context_for(state, &session, base)?.ok_or_else(|| {
        if state.game_user_data_or_located().is_none() {
            user_data_unknown()
        } else {
            ApiError::new(
                StatusCode::NOT_FOUND,
                "aucun réglage de cette voiture dans le dossier Settings du jeu : sauvegarde d'abord un réglage dans le garage",
            )
        }
    })?;
    let p = resolved_plan(state, store, id, &ctx, plan)?;
    let notes = notes_for(&p.label, &ctx.base_info.file, &p.changes);
    let (_, changed) = build_setup(&ctx.base, &p.changes, &notes);
    Ok(SetupPreview {
        base: ctx.base_info.clone(),
        plan: p.reference,
        plan_label_fr: p.label,
        file_name: file_name_for(&p.tag, &ctx.base_info.file),
        notes,
        changes: p.changes,
        dropped: p.dropped,
        warnings_fr: p.warnings,
        changed_lines: changed,
    })
}

/// `GET /api/sessions/{id}/setup-preview?base=&plan=`.
pub async fn setup_preview(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(q): Query<PreviewQuery>,
) -> AppResult {
    let store = store_of(&state)?;
    let plan = q
        .plan
        .filter(|p| !p.trim().is_empty())
        .ok_or_else(|| bad("paramètre plan manquant"))?;
    let out =
        tokio::task::spawn_blocking(move || preview(&state, &store, &id, q.base.as_deref(), &plan))
            .await??;
    Ok(Json(out).into_response())
}

fn validate_files_body(body: &[u8]) -> Result<NewSetupFiles, ApiError> {
    let req: NewSetupFiles =
        serde_json::from_slice(body).map_err(|e| bad(format!("corps invalide : {e}")))?;
    if req.base.trim().is_empty() {
        return Err(bad("base manquante"));
    }
    if req.plans.is_empty() {
        return Err(bad("aucun plan demandé"));
    }
    if req.plans.len() > MAX_PLANS_PER_REQUEST {
        return Err(bad(format!(
            "au plus {MAX_PLANS_PER_REQUEST} plans par demande"
        )));
    }
    let mut seen = Vec::new();
    for p in &req.plans {
        PlanRef::parse(p)?;
        if seen.contains(&p.trim()) {
            return Err(bad(format!("plan « {p} » demandé deux fois")));
        }
        seen.push(p.trim());
    }
    Ok(req)
}

fn create_files(
    state: &AppState,
    store: &Store,
    id: &str,
    req: &NewSetupFiles,
) -> Result<SetupFilesCreated, ApiError> {
    let session = session_row(store, id)?;
    if state.game_user_data_or_located().is_none() {
        return Err(user_data_unknown());
    }
    let ctx = context_for(state, &session, Some(&req.base))?.ok_or_else(user_data_unknown)?;
    let folder_dir = ctx.settings_dir.join(&ctx.base_info.folder);
    // Everything is computed before the first file is written.
    let mut ready = Vec::new();
    for plan in &req.plans {
        let p = resolved_plan(state, store, id, &ctx, plan)?;
        let notes = notes_for(&p.label, &ctx.base_info.file, &p.changes);
        let (setup, changed) = build_setup(&ctx.base, &p.changes, &notes);
        ready.push((p, setup, changed));
    }
    let mut files = Vec::new();
    for (p, setup, changed) in ready {
        let name = file_name_for(&p.tag, &ctx.base_info.file);
        let path = write_new_file(&folder_dir, &name, setup.to_string().as_bytes())
            .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        let file = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or(name);
        tracing::info!(
            "réglage créé : {} ({}, base {}, {} ligne(s) modifiée(s))",
            path.display(),
            p.label,
            ctx.base_info.path,
            changed
        );
        files.push(CreatedSetupFile {
            plan: p.reference,
            plan_label_fr: p.label,
            path: format!("{}/{file}", ctx.base_info.folder),
            file,
            changed_lines: changed,
        });
    }
    let names: Vec<&str> = files.iter().map(|f| f.file.as_str()).collect();
    let first = names.first().copied().unwrap_or("Corde_…");
    Ok(SetupFilesCreated {
        folder: ctx.base_info.folder.clone(),
        instructions_fr: format!(
            "Dans le garage du jeu : Réglages (Setups) → rafraîchir la liste → charger « {first} » (dossier « {} »). Fichiers : {}.",
            ctx.base_info.folder,
            names.join(", ")
        ),
        instructions_en: format!(
            "In the game's garage: Setups → refresh the list → load \"{first}\" (folder \"{}\"). Files: {}.",
            ctx.base_info.folder,
            names.join(", ")
        ),
        files,
    })
}

/// `POST /api/sessions/{id}/setup-files`: from this PC only (same guard as
/// `PUT /api/settings`).
pub async fn create_setup_files(
    State(state): State<Arc<AppState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    Path(id): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> AppResult {
    let origin = headers
        .get(header::ORIGIN)
        .map(|v| v.to_str().unwrap_or(""));
    if !crate::net::may_change_settings(peer.ip(), origin) {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "les fichiers de réglage ne se créent que depuis le PC qui fait tourner Corde",
        ));
    }
    let req = validate_files_body(&body)?;
    let store = store_of(&state)?;
    let out =
        tokio::task::spawn_blocking(move || create_files(&state, &store, &id, &req)).await??;
    Ok((StatusCode::CREATED, Json(out)).into_response())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_of_both_names_are_ours() {
        let file = |file: &str, notes: &str| SetupFileInfo {
            file: file.into(),
            notes: notes.into(),
            ..SetupFileInfo::default()
        };
        assert!(is_corde_file(&file("Corde_Sur_BMW.svm", "")));
        assert!(is_corde_file(&file(
            "x.svm",
            "Corde - Sur - base : BMW.svm"
        )));
        assert!(is_corde_file(&file("Muret_Sur_BMW.svm", ""))); // legacy name (Muret, before 0.3.0)
        assert!(is_corde_file(&file(
            "x.svm",
            "Muret - Sur - base : BMW.svm"
        ))); // legacy name (Muret, before 0.3.0)
        assert!(!is_corde_file(&file("BMW_LongBeach.svm", "mon réglage")));
    }

    #[test]
    fn plan_references() {
        assert_eq!(
            PlanRef::parse("safe").unwrap(),
            PlanRef::Plan(SetupPhilosophy::Safe)
        );
        assert_eq!(
            PlanRef::parse("iteration:2").unwrap(),
            PlanRef::Iteration { plan: None, n: 2 }
        );
        assert_eq!(
            PlanRef::parse("iteration:wet:3").unwrap(),
            PlanRef::Iteration {
                plan: Some(SetupPhilosophy::Wet),
                n: 3
            }
        );
        for bad in [
            "",
            "fast",
            "iteration:",
            "iteration:0",
            "iteration:x:1",
            "iteration:-1",
        ] {
            assert!(PlanRef::parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn file_names() {
        assert_eq!(
            file_name_for("Sur", "BMW_LongBeach.svm"),
            "Corde_Sur_BMW_LongBeach.svm"
        );
        assert_eq!(
            file_name_for("Agressif", "Corde_Sur_BMW_LongBeach.svm"),
            "Corde_Agressif_BMW_LongBeach.svm"
        );
        assert_eq!(
            file_name_for("Pluie", "Corde_Iteration2_Sur_BMW_LongBeach_2.svm"),
            "Corde_Pluie_BMW_LongBeach_2.svm"
        );
        assert_eq!(
            file_name_for("Sur", "réglage é.svm"),
            "Corde_Sur_r_glage _.svm"
        );
        assert_eq!(file_name_for("Sur", ".svm"), "Corde_Sur_reglage.svm");
        // Files written under the former name: prefix dropped, new prefix written.
        assert_eq!(
            file_name_for("Sur", "Muret_Agressif_BMW_LongBeach.svm"), // legacy name (Muret, before 0.3.0)
            "Corde_Sur_BMW_LongBeach.svm"
        );
        assert_eq!(
            file_name_for("Sur", "Corde_Pluie_Muret_Sur_BMW_LongBeach.svm"), // legacy name (Muret, before 0.3.0)
            "Corde_Sur_BMW_LongBeach.svm"
        );
    }

    #[test]
    fn new_files_never_overwrite() {
        let dir = std::env::temp_dir().join(format!("corde-setups-write-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let a = write_new_file(&dir, "Corde_Sur_x.svm", b"one").unwrap();
        let b = write_new_file(&dir, "Corde_Sur_x.svm", b"two").unwrap();
        let c = write_new_file(&dir, "Corde_Sur_x.svm", b"three").unwrap();
        assert_eq!(a.file_name().unwrap(), "Corde_Sur_x.svm");
        assert_eq!(b.file_name().unwrap(), "Corde_Sur_x_2.svm");
        assert_eq!(c.file_name().unwrap(), "Corde_Sur_x_3.svm");
        assert_eq!(std::fs::read(&a).unwrap(), b"one");
        assert_eq!(std::fs::read(&b).unwrap(), b"two");
        let leftovers: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".corde-tmp"))
            .collect();
        assert!(leftovers.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn request_bodies_are_checked() {
        assert!(validate_files_body(
            br#"{"base":"Longbeach/a.svm","plans":["safe","iteration:1"]}"#
        )
        .is_ok());
        for body in [
            &br#"{"base":"","plans":["safe"]}"#[..],
            br#"{"base":"Longbeach/a.svm","plans":[]}"#,
            br#"{"base":"Longbeach/a.svm","plans":["fast"]}"#,
            br#"{"base":"Longbeach/a.svm","plans":["safe","safe"]}"#,
            br#"{"plans":["safe"]}"#,
            b"not json",
        ] {
            let e = validate_files_body(body).unwrap_err();
            assert_eq!(e.status(), StatusCode::BAD_REQUEST);
        }
    }
}
