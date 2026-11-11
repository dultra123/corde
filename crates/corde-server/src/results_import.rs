//! Importer of the game's results files (`<UserData>/Log/Results/*.xml`)
//! into the store (`results` / `result_laps`), feeding `/api/profile`.
//!
//! A scan runs at start-up, then every [`ImportConfig::interval`], and on
//! `POST /api/profile/import`. Each scan:
//!
//! 1. resolves the folder: `--lmu-results-dir`, else the `UserData` folder
//!    announced by the running game, else the Steam libraries;
//! 2. lists `*.xml`, keeps the files that are new or whose mtime changed
//!    (and, unless forced, skips the ones already rejected unchanged);
//! 3. parses them, works out which driver is the player
//!    ([`corde_lmu::results::pick_player`]: `--player-name`, the live
//!    session's driver name, names already imported, then "the human present
//!    in the most files") and stores each file with the player's laps.
//!
//! Files without a human driver (AI-only replays) or whose player cannot be
//! told apart are reported in the status, not stored; they are retried when
//! their mtime changes or on a forced scan — by then another file usually
//! settles the question.

use corde_lmu::results::{self, name_key, pick_player, RaceResultsFile, ResultDriver};
use corde_store::{NewResult, ResultLapRow, ResultRow, Store};
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Default pause between two scans.
pub const DEFAULT_INTERVAL: Duration = Duration::from_secs(60);
/// Longest list of skipped files kept in the status.
const MAX_SKIPPED_REPORTED: usize = 20;

#[derive(Debug, Clone)]
pub struct ImportConfig {
    pub enabled: bool,
    /// `--lmu-results-dir`.
    pub results_dir: Option<PathBuf>,
    /// `--player-name`.
    pub player_name: Option<String>,
    pub interval: Duration,
}

impl Default for ImportConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            results_dir: None,
            player_name: None,
            interval: DEFAULT_INTERVAL,
        }
    }
}

/// Where the results folder came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DirSource {
    /// `--lmu-results-dir`.
    Cli,
    /// `LMU_Data.paths.userData` of the running game.
    Game,
    /// Found through the Steam libraries.
    Steam,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SkippedFile {
    pub file_name: String,
    pub reason: String,
}

/// `import` part of `GET /api/profile`.
#[derive(Debug, Clone, PartialEq, Serialize, Default)]
pub struct ImportStatus {
    /// Importer active (not `--no-import`, folder found).
    pub enabled: bool,
    pub results_dir: Option<String>,
    pub dir_source: Option<DirSource>,
    pub last_scan_ms: Option<i64>,
    pub scanning: bool,
    /// `*.xml` files in the folder at the last scan.
    pub files_seen: u32,
    /// Files imported by the last scan.
    pub files_imported: u32,
    /// Results in the database.
    pub total_imported: i64,
    /// Files rejected (no player, ambiguous, unreadable).
    pub files_skipped: u32,
    pub skipped: Vec<SkippedFile>,
    /// Player name the importer relies on (`--player-name`, else learnt).
    pub player_name: Option<String>,
    /// Why nothing is imported, in French (folder missing…).
    pub error: Option<String>,
}

/// Hints the server passes to a scan (what only the live state knows).
#[derive(Debug, Clone, Default)]
pub struct ScanContext {
    /// `UserData` announced by the running game.
    pub game_user_data: Option<PathBuf>,
    /// Driver name of the live session.
    pub live_player_name: Option<String>,
}

pub struct ResultsImporter {
    store: Arc<Store>,
    cfg: ImportConfig,
    status: Mutex<ImportStatus>,
    /// Files rejected at a given mtime: not re-parsed every minute.
    rejected: Mutex<HashMap<String, (i64, String)>>,
    /// One scan at a time.
    scan_lock: Mutex<()>,
}

impl std::fmt::Debug for ResultsImporter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResultsImporter")
            .field("cfg", &self.cfg)
            .finish()
    }
}

/// Outcome of one scan (logged).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ScanReport {
    pub dir: Option<PathBuf>,
    pub seen: u32,
    pub imported: u32,
    pub skipped: u32,
    pub unchanged: u32,
    /// Recorded sessions whose laps were checked against a result file.
    pub reconciled: u32,
}

impl ResultsImporter {
    pub fn new(store: Arc<Store>, cfg: ImportConfig) -> Arc<Self> {
        let status = ImportStatus {
            enabled: cfg.enabled,
            player_name: cfg.player_name.clone(),
            total_imported: store.db().result_count().unwrap_or(0),
            error: (!cfg.enabled).then(|| "Import désactivé (--no-import).".to_string()),
            ..Default::default()
        };
        Arc::new(Self {
            store,
            cfg,
            status: Mutex::new(status),
            rejected: Mutex::new(HashMap::new()),
            scan_lock: Mutex::new(()),
        })
    }

    pub fn config(&self) -> &ImportConfig {
        &self.cfg
    }

    pub fn interval(&self) -> Duration {
        self.cfg.interval
    }

    pub fn status(&self) -> ImportStatus {
        self.status
            .lock()
            .map(|s| s.clone())
            .unwrap_or_else(|p| p.into_inner().clone())
    }

    fn update_status(&self, f: impl FnOnce(&mut ImportStatus)) {
        let mut g = match self.status.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        f(&mut g);
    }

    /// The folder to scan and where it came from.
    pub fn resolve_dir(&self, ctx: &ScanContext) -> Option<(PathBuf, DirSource)> {
        if let Some(d) = &self.cfg.results_dir {
            return Some((d.clone(), DirSource::Cli));
        }
        if let Some(ud) = &ctx.game_user_data {
            let d = corde_lmu::settings::results_dir(ud);
            if d.is_dir() {
                return Some((d, DirSource::Game));
            }
        }
        corde_lmu::settings::locate_game()
            .map(|p| p.results_dir())
            .filter(|d| d.is_dir())
            .map(|d| (d, DirSource::Steam))
    }

    /// Run one scan (blocking: file IO and SQLite). `force` re-examines the
    /// files rejected earlier.
    pub fn scan(&self, ctx: &ScanContext, force: bool) -> ScanReport {
        let _guard = match self.scan_lock.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        let started = Instant::now();
        let mut report = ScanReport::default();
        if !self.cfg.enabled {
            return report;
        }
        self.update_status(|s| s.scanning = true);
        let outcome = self.scan_inner(ctx, force, &mut report);
        // The game is the referee: give every recorded session the official
        // lap times of its result file (new files, and sessions closed since
        // the last scan).
        match self.store.reconcile_pending(report.imported > 0) {
            Ok(reports) => {
                report.reconciled = reports.iter().filter(|r| r.skipped.is_none()).count() as u32
            }
            Err(e) => tracing::warn!("tours : recalage sur les résultats impossible — {e}"),
        }
        let now_ms = corde_store::db::now_ms();
        let total = self.store.db().result_count().unwrap_or(0);
        let rejected: Vec<SkippedFile> = {
            let g = match self.rejected.lock() {
                Ok(g) => g,
                Err(p) => p.into_inner(),
            };
            let mut v: Vec<SkippedFile> = g
                .iter()
                .map(|(name, (_, reason))| SkippedFile {
                    file_name: name.clone(),
                    reason: reason.clone(),
                })
                .collect();
            v.sort_by(|a, b| b.file_name.cmp(&a.file_name));
            v.truncate(MAX_SKIPPED_REPORTED);
            v
        };
        let mut previously_skipped = 0;
        self.update_status(|s| {
            previously_skipped = s.files_skipped;
            s.scanning = false;
            s.last_scan_ms = Some(now_ms);
            s.files_seen = report.seen;
            s.files_imported = report.imported;
            s.files_skipped = report.skipped;
            s.total_imported = total;
            s.skipped = rejected;
            match &outcome {
                Ok(()) => s.error = None,
                Err(msg) => s.error = Some(msg.clone()),
            }
        });
        match (&outcome, report.imported, report.skipped) {
            (Err(msg), _, _) => tracing::warn!("résultats : {msg}"),
            (Ok(()), 0, 0) => tracing::debug!(
                "résultats : {} fichier(s), rien de nouveau ({:?})",
                report.seen,
                started.elapsed()
            ),
            // The same files are still refused: already said once, the scan runs every minute.
            (Ok(()), 0, skipped) if skipped == previously_skipped => tracing::debug!(
                "résultats : {} fichier(s), rien de nouveau, {skipped} toujours ignoré(s) ({:?})",
                report.seen,
                started.elapsed()
            ),
            (Ok(()), imported, skipped) => tracing::info!(
                "résultats : {imported} fichier(s) importé(s), {skipped} ignoré(s) sur {} dans {} ({:?})",
                report.seen,
                report.dir.as_deref().map(|d| d.display().to_string()).unwrap_or_default(),
                started.elapsed()
            ),
        }
        report
    }

    fn scan_inner(
        &self,
        ctx: &ScanContext,
        force: bool,
        report: &mut ScanReport,
    ) -> Result<(), String> {
        let Some((dir, source)) = self.resolve_dir(ctx) else {
            self.update_status(|s| {
                s.enabled = false;
                s.results_dir = None;
                s.dir_source = None;
            });
            return Err(
                "Dossier des résultats introuvable : lance le jeu une fois, ou indique-le avec \
                 --lmu-results-dir \"<Le Mans Ultimate>\\UserData\\Log\\Results\"."
                    .to_string(),
            );
        };
        self.update_status(|s| {
            s.enabled = true;
            s.results_dir = Some(dir.display().to_string());
            s.dir_source = Some(source);
        });
        report.dir = Some(dir.clone());

        if !dir.is_dir() {
            return Err(format!(
                "Dossier des résultats introuvable : {} (vérifie --lmu-results-dir).",
                dir.display()
            ));
        }
        let entries = std::fs::read_dir(&dir)
            .map_err(|e| format!("Dossier des résultats illisible ({}) : {e}", dir.display()))?;
        let mut files: Vec<(String, i64, PathBuf)> = Vec::new();
        for e in entries.flatten() {
            let path = e.path();
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if !name.to_ascii_lowercase().ends_with(".xml") {
                continue;
            }
            let Ok(meta) = e.metadata() else { continue };
            if !meta.is_file() {
                continue;
            }
            let mtime = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0);
            files.push((name.to_string(), mtime, path));
        }
        files.sort();
        report.seen = files.len() as u32;

        let known: HashMap<String, i64> = self
            .store
            .imported_files()
            .map_err(|e| format!("base de données : {e}"))?
            .into_iter()
            .map(|f| (f.file_name, f.file_mtime_ms))
            .collect();
        if force {
            if let Ok(mut g) = self.rejected.lock() {
                g.clear();
            }
        }

        // Parse what is new.
        let mut parsed: Vec<(String, i64, RaceResultsFile)> = Vec::new();
        for (name, mtime, path) in files {
            if known.get(&name) == Some(&mtime) {
                report.unchanged += 1;
                continue;
            }
            let already_rejected = self
                .rejected
                .lock()
                .ok()
                .map(|g| g.get(&name).map(|(m, _)| *m) == Some(mtime))
                .unwrap_or(false);
            if already_rejected && !force {
                report.skipped += 1;
                continue;
            }
            match results::parse_file(&path) {
                Ok(f) => parsed.push((name, mtime, f)),
                Err(e) => {
                    tracing::warn!("résultats : {name} ignoré — {e}");
                    self.reject(&name, mtime, format!("fichier illisible : {e}"));
                    report.skipped += 1;
                }
            }
        }
        if parsed.is_empty() {
            return Ok(());
        }
        parsed.sort_by_key(|(_, _, f)| f.date_time);

        // Who is the player: hints, then name frequency across the library.
        let mut hints: Vec<String> = Vec::new();
        if let Some(n) = &self.cfg.player_name {
            hints.push(n.clone());
        }
        if let Some(n) = &ctx.live_player_name {
            hints.push(n.clone());
        }
        hints.extend(self.store.result_player_names().unwrap_or_default());
        let mut seen = self.store.result_driver_counts().unwrap_or_default();
        for (name, _, f) in &parsed {
            // Names already counted from the database for a re-imported file.
            if known.contains_key(name) {
                continue;
            }
            let mut here: Vec<String> = Vec::new();
            for d in &f.drivers {
                let k = name_key(&d.name);
                if !here.contains(&k) {
                    *seen.entry(k.clone()).or_default() += 1;
                    here.push(k);
                }
            }
        }

        for (name, mtime, mut file) in parsed {
            if !file.drivers.iter().any(|d| d.is_player) {
                self.reject(
                    &name,
                    mtime,
                    "aucun pilote humain dans ce fichier (replay ou IA seulement)".into(),
                );
                report.skipped += 1;
                continue;
            }
            let Some(idx) = pick_player(&file, &hints, &seen) else {
                self.reject(
                    &name,
                    mtime,
                    "impossible de savoir qui tu es parmi les pilotes (ajoute --player-name \"Ton Nom\")"
                        .into(),
                );
                report.skipped += 1;
                continue;
            };
            let player_name = file.drivers[idx].name.clone();
            if !hints.iter().any(|h| name_key(h) == name_key(&player_name)) {
                hints.insert(0, player_name.clone());
            }
            file.retain_events_of(&player_name);
            let new = to_new_result(&name, mtime, &file, idx);
            match self.store.upsert_result(&new) {
                Ok(id) => {
                    report.imported += 1;
                    if let Ok(mut g) = self.rejected.lock() {
                        g.remove(&name);
                    }
                    tracing::info!(
                        "résultats : {name} → #{id} {} {:?} — {} {}/{} ({} tours, meilleur {:?})",
                        file.track_course,
                        file.session_kind,
                        player_name,
                        new.row.player_class_position.unwrap_or(0),
                        new.row.num_class_drivers,
                        new.row.player_laps,
                        new.row.player_best_lap_s
                    );
                }
                Err(e) => {
                    tracing::error!("résultats : {name} non enregistré — {e}");
                    self.reject(&name, mtime, format!("enregistrement impossible : {e}"));
                    report.skipped += 1;
                }
            }
        }
        self.update_status(|s| {
            if s.player_name.is_none() {
                s.player_name = hints.first().cloned();
            }
        });
        Ok(())
    }

    fn reject(&self, name: &str, mtime: i64, reason: String) {
        if let Ok(mut g) = self.rejected.lock() {
            g.insert(name.to_string(), (mtime, reason));
        }
    }
}

/// Header row + player laps + JSON for the store.
pub fn to_new_result(
    file_name: &str,
    file_mtime_ms: i64,
    file: &RaceResultsFile,
    player_idx: usize,
) -> NewResult {
    let me: &ResultDriver = &file.drivers[player_idx];
    let class_count = file
        .drivers
        .iter()
        .filter(|d| d.car_class == me.car_class)
        .count() as i32;
    let row = ResultRow {
        id: 0,
        file_name: file_name.to_string(),
        file_mtime_ms,
        imported_wall_ms: 0,
        game_version: file.game_version.clone(),
        track_venue: file.track_venue.clone(),
        track_course: file.track_course.clone(),
        track_length_m: file.track_length_m,
        session_kind: file.session_kind,
        session_name: file.session_name.clone(),
        started_wall_ms: file.date_time.saturating_mul(1000),
        minutes: file.minutes,
        laps_planned: file.laps_planned,
        setting: file.setting.clone(),
        server_name: file.server_name.clone(),
        player_name: me.name.clone(),
        player_position: me.position,
        player_class_position: me.class_position,
        player_grid_pos: me.grid_pos,
        player_class_grid_pos: me.class_grid_pos,
        player_class: me.car_class.clone(),
        player_vehicle: if me.car_type.is_empty() {
            me.vehicle_name.clone()
        } else {
            me.car_type.clone()
        },
        player_vehicle_name: me.vehicle_name.clone(),
        player_best_lap_s: me
            .best_lap_s
            .or_else(|| me.best_from_laps().and_then(|l| l.lap_time_s)),
        player_laps: me.laps_completed.max(me.laps.len() as i32),
        player_finish: me.finish,
        num_drivers: file.drivers.len() as i32,
        num_class_drivers: class_count,
    };
    let laps = me
        .laps
        .iter()
        .map(|l| ResultLapRow {
            result_id: 0,
            driver_name: me.name.clone(),
            is_player: true,
            lap_number: l.num,
            position: l.position,
            elapsed_s: l.elapsed_s,
            lap_time_s: l.lap_time_s,
            s1: l.s1,
            s2: l.s2,
            s3: l.s3,
            top_speed_kmh: l.top_speed_kmh,
            fuel_fraction: l.fuel_fraction,
            front_compound: l.front_compound.clone(),
            rear_compound: l.rear_compound.clone(),
            tyre_wear: l.tyre_wear,
            pit: l.pit,
        })
        .collect();
    NewResult {
        row,
        driver_names: file.drivers.iter().map(|d| d.name.clone()).collect(),
        laps,
        json: serde_json::to_value(file).unwrap_or(serde_json::Value::Null),
    }
}

/// ±10 min: a recorded session that started this close, on the same track,
/// is the same session as a results file.
pub const LINK_WINDOW_MS: i64 = 10 * 60 * 1000;

/// Recorded session matching a result (same track, start within ±10 min).
pub fn linked_session(store: &Store, row: &ResultRow) -> Option<String> {
    store
        .find_session_near(row.started_wall_ms, &row.track_course, LINK_WINDOW_MS)
        .ok()
        .flatten()
        .or_else(|| {
            store
                .find_session_near(row.started_wall_ms, &row.track_venue, LINK_WINDOW_MS)
                .ok()
                .flatten()
        })
}

/// Periodic scan until `shutdown` flips; the first one runs right away.
pub async fn run_periodic(
    importer: Arc<ResultsImporter>,
    state: Arc<crate::state::AppState>,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) {
    let interval = importer.interval();
    loop {
        let ctx = ScanContext {
            game_user_data: state.game_user_data(),
            live_player_name: state.live_player_name(),
        };
        let imp = Arc::clone(&importer);
        let _ = tokio::task::spawn_blocking(move || imp.scan(&ctx, false)).await;
        tokio::select! {
            _ = tokio::time::sleep(interval) => {}
            _ = shutdown.changed() => {
                if *shutdown.borrow() {
                    break;
                }
            }
        }
    }
}

/// Helper for tests and tools: scan one folder once with a fixed player name.
pub fn scan_dir_once(store: Arc<Store>, dir: &Path, player_name: Option<&str>) -> ScanReport {
    let importer = ResultsImporter::new(
        store,
        ImportConfig {
            results_dir: Some(dir.to_path_buf()),
            player_name: player_name.map(str::to_string),
            ..Default::default()
        },
    );
    importer.scan(&ScanContext::default(), true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use corde_protocol::SessionType;
    use corde_store::ResultFilter;

    fn samples_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../reference/samples")
    }

    fn temp_store(tag: &str) -> (Arc<Store>, PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "corde-import-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        (Store::open(&dir).unwrap(), dir)
    }

    #[test]
    fn imports_both_samples_without_a_hint() {
        let (store, dir) = temp_store("samples");
        let importer = ResultsImporter::new(
            Arc::clone(&store),
            ImportConfig {
                results_dir: Some(samples_dir()),
                ..Default::default()
            },
        );
        let report = importer.scan(&ScanContext::default(), false);
        assert_eq!(report.seen, 2, "two *.xml in reference/samples");
        assert_eq!(report.imported, 2);
        assert_eq!(report.skipped, 0);
        let st = importer.status();
        assert!(st.enabled);
        assert_eq!(st.dir_source, Some(DirSource::Cli));
        assert_eq!(st.total_imported, 2);
        assert_eq!(st.files_imported, 2);
        assert_eq!(st.player_name.as_deref(), Some("alex martin"));
        assert!(st.error.is_none());

        let rows = store.list_results(&ResultFilter::default()).unwrap();
        assert_eq!(rows.len(), 2);
        let race = &rows[0];
        assert_eq!(race.file_name, "results-R1-paulricard.xml");
        assert_eq!(race.session_kind, SessionType::Race);
        assert_eq!(race.track_course, "Paul Ricard - 1A");
        assert_eq!(race.player_name, "alex martin");
        assert_eq!(race.player_position, Some(23));
        assert_eq!(race.player_class_position, Some(12));
        assert_eq!(race.num_drivers, 38);
        assert_eq!(race.num_class_drivers, 19);
        assert_eq!(race.player_best_lap_s, Some(118.3745));
        assert_eq!(race.player_laps, 15);
        assert_eq!(race.player_vehicle, "Mercedes-AMG LMGT3");
        assert_eq!(race.started_wall_ms, 1_793_563_502_000);
        let practice = &rows[1];
        assert_eq!(practice.session_kind, SessionType::Practice);
        assert_eq!(practice.player_laps, 0);
        assert_eq!(practice.player_best_lap_s, None);

        let detail = store.get_result(race.id).unwrap();
        assert_eq!(detail.laps.len(), 15);
        assert!(detail.laps.iter().all(|l| l.is_player));
        assert_eq!(detail.file["drivers"].as_array().unwrap().len(), 38);
        // Only the player's stream events are kept in the blob.
        let events = detail.file["events"].as_array().unwrap();
        assert!(!events.is_empty());
        assert!(events
            .iter()
            .all(|e| e["driver"] == "alex martin" || e["other"] == "alex martin"));

        // Second scan: nothing changed.
        let again = importer.scan(&ScanContext::default(), false);
        assert_eq!(again.imported, 0);
        assert_eq!(again.unchanged, 2);
        assert_eq!(store.db().result_count().unwrap(), 2);

        let profile = store.profile_summary().unwrap();
        assert_eq!(profile.player_name.as_deref(), Some("alex martin"));
        assert_eq!(profile.totals.sessions, 2);
        assert_eq!(profile.totals.races, 1);
        assert_eq!(profile.totals.laps, 15);
        assert!((profile.totals.km - 15.0 * 5.6986).abs() < 0.01);
        assert_eq!(profile.tracks.len(), 2);
        assert_eq!(profile.tracks[0].track_course, "Paul Ricard - 1A");
        assert_eq!(profile.tracks[0].best_class_position, Some(12));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn single_multiplayer_file_needs_a_hint() {
        let (store, dir) = temp_store("single");
        let only = dir.join("results");
        std::fs::create_dir_all(&only).unwrap();
        std::fs::copy(
            samples_dir().join("results-P1-paulricard.xml"),
            only.join("results-P1-paulricard.xml"),
        )
        .unwrap();
        let importer = ResultsImporter::new(
            Arc::clone(&store),
            ImportConfig {
                results_dir: Some(only.clone()),
                ..Default::default()
            },
        );
        let r = importer.scan(&ScanContext::default(), false);
        assert_eq!((r.seen, r.imported, r.skipped), (1, 0, 1));
        let st = importer.status();
        assert_eq!(st.skipped.len(), 1);
        assert!(st.skipped[0].reason.contains("--player-name"));
        // Unchanged rejected files are not re-parsed…
        let r = importer.scan(&ScanContext::default(), false);
        assert_eq!((r.imported, r.skipped), (0, 1));
        // …the live session name settles it (forced scan re-examines them).
        let ctx = ScanContext {
            live_player_name: Some("Alex Martin".into()),
            ..Default::default()
        };
        let r = importer.scan(&ctx, true);
        assert_eq!((r.imported, r.skipped), (1, 0));
        assert!(importer.status().skipped.is_empty());

        // A broken file is reported, not fatal.
        std::fs::write(
            only.join("broken.xml"),
            "<rFactorXML><RaceResults></RaceResults>",
        )
        .unwrap();
        let r = importer.scan(&ScanContext::default(), false);
        assert_eq!((r.seen, r.imported, r.skipped), (2, 0, 1));
        assert!(importer.status().skipped[0].reason.contains("illisible"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn missing_dir_disables_with_a_message() {
        let (store, dir) = temp_store("missing");
        let importer = ResultsImporter::new(
            Arc::clone(&store),
            ImportConfig {
                results_dir: Some(dir.join("nope")),
                ..Default::default()
            },
        );
        let r = importer.scan(&ScanContext::default(), false);
        assert_eq!(r.imported, 0);
        let st = importer.status();
        assert!(st.error.as_deref().unwrap().contains("introuvable"));
        assert!(
            st.enabled,
            "configured but missing: stays enabled, reports the error"
        );
        let off = ResultsImporter::new(
            store,
            ImportConfig {
                enabled: false,
                ..Default::default()
            },
        );
        assert_eq!(
            off.scan(&ScanContext::default(), true),
            ScanReport::default()
        );
        assert!(!off.status().enabled);
        let _ = std::fs::remove_dir_all(dir);
    }
}
