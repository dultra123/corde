//! Results imported from the game's `UserData/Log/Results/*.xml` files: the
//! `results` / `result_laps` tables (schema v2) and the driver profile built
//! from them.
//!
//! The store is game-agnostic: the server parses a file (`corde_lmu::results`),
//! decides which driver is the player and hands a [`NewResult`] over — header
//! columns for the queries, the player's laps as rows, and the whole parsed
//! file as JSON (zstd-compressed) for the detail page.

use crate::db::{enum_parse, enum_str, now_ms, Db};
use crate::error::{Result, StoreError};
use crate::store::Store;
use corde_protocol::{FinishStatus, SessionType};
use rusqlite::{params, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// zstd level for the JSON blob (fast, ~10× on these files).
const JSON_ZSTD_LEVEL: i32 = 3;
/// Default / hard cap of `limit` in [`ResultFilter`].
pub const DEFAULT_RESULTS_LIMIT: usize = 100;
pub const MAX_RESULTS_LIMIT: usize = 100_000;

// ---------------------------------------------------------------------------
// Rows
// ---------------------------------------------------------------------------

/// One imported results file (what `GET /api/results` returns).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ResultRow {
    pub id: i64,
    pub file_name: String,
    pub file_mtime_ms: i64,
    pub imported_wall_ms: i64,
    pub game_version: Option<String>,
    pub track_venue: String,
    pub track_course: String,
    pub track_length_m: f32,
    pub session_kind: SessionType,
    /// Raw section name (`Practice1`, `Race`).
    pub session_name: String,
    pub started_wall_ms: i64,
    pub minutes: Option<f32>,
    pub laps_planned: Option<i32>,
    /// `Multiplayer`, `Single Player`…
    pub setting: Option<String>,
    pub server_name: Option<String>,
    pub player_name: String,
    pub player_position: Option<i32>,
    pub player_class_position: Option<i32>,
    pub player_grid_pos: Option<i32>,
    pub player_class_grid_pos: Option<i32>,
    pub player_class: String,
    /// Car model (`Mercedes-AMG LMGT3`).
    pub player_vehicle: String,
    /// Livery / entry (`Mercedes AMG Custom Team 2025 #397`).
    pub player_vehicle_name: String,
    pub player_best_lap_s: Option<f32>,
    pub player_laps: i32,
    pub player_finish: FinishStatus,
    pub num_drivers: i32,
    pub num_class_drivers: i32,
}

/// One lap of `result_laps`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ResultLapRow {
    pub result_id: i64,
    pub driver_name: String,
    pub is_player: bool,
    pub lap_number: i32,
    pub position: Option<i32>,
    pub elapsed_s: Option<f64>,
    pub lap_time_s: Option<f32>,
    pub s1: Option<f32>,
    pub s2: Option<f32>,
    pub s3: Option<f32>,
    pub top_speed_kmh: Option<f32>,
    pub fuel_fraction: Option<f32>,
    pub front_compound: Option<String>,
    pub rear_compound: Option<String>,
    pub tyre_wear: Option<[f32; 4]>,
    pub pit: bool,
}

/// What the importer hands to [`Db::upsert_result`].
#[derive(Debug, Clone, PartialEq)]
pub struct NewResult {
    /// `id`, `imported_wall_ms` are ignored (assigned by the store).
    pub row: ResultRow,
    /// Every driver of the file.
    pub driver_names: Vec<String>,
    /// Laps to keep as rows (the player's; `result_id` is ignored).
    pub laps: Vec<ResultLapRow>,
    /// The whole parsed file.
    pub json: serde_json::Value,
}

/// `GET /api/results/{id}`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ResultDetail {
    #[serde(flatten)]
    pub row: ResultRow,
    /// The player's laps.
    pub laps: Vec<ResultLapRow>,
    /// The parsed file (classification of every driver, their laps, events).
    pub file: serde_json::Value,
}

/// Filter of [`Db::list_results`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ResultFilter {
    /// Case-insensitive substring of the course or venue name.
    pub track: Option<String>,
    pub session_kind: Option<SessionType>,
    /// Exact vehicle model.
    pub vehicle: Option<String>,
    /// Most recent first; default 100, max 100 000.
    pub limit: Option<usize>,
}

/// File name + mtime of every imported file (to skip unchanged ones).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedFile {
    pub id: i64,
    pub file_name: String,
    pub file_mtime_ms: i64,
}

// ---------------------------------------------------------------------------
// Profile
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Default)]
pub struct ProfileTotals {
    pub sessions: i64,
    pub races: i64,
    pub qualifyings: i64,
    pub practices: i64,
    /// Laps completed by the player, every session type.
    pub laps: i64,
    pub km: f64,
    /// Races finished first overall / on the overall podium.
    pub wins: i64,
    pub podiums: i64,
    pub class_wins: i64,
    pub class_podiums: i64,
    /// Races the player did not finish (DNF / DQ).
    pub dnfs: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Default)]
pub struct TrackSummary {
    pub track_venue: String,
    pub track_course: String,
    pub track_length_m: f32,
    pub sessions: i64,
    pub races: i64,
    pub laps: i64,
    pub km: f64,
    pub best_lap_s: Option<f32>,
    pub best_lap_vehicle: Option<String>,
    pub best_lap_date: Option<i64>,
    pub best_lap_result_id: Option<i64>,
    /// Best / mean class position in races.
    pub best_class_position: Option<i32>,
    pub avg_class_position: Option<f32>,
    pub last_date: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Default)]
pub struct VehicleSummary {
    pub vehicle_name: String,
    pub car_class: String,
    pub sessions: i64,
    pub races: i64,
    pub laps: i64,
    pub km: f64,
    pub best_lap_s: Option<f32>,
    pub best_track: Option<String>,
    pub best_lap_date: Option<i64>,
    pub best_lap_result_id: Option<i64>,
    pub last_date: i64,
}

/// `GET /api/profile` minus the import status (added by the server).
#[derive(Debug, Clone, PartialEq, Serialize, Default)]
pub struct ProfileSummary {
    /// Most frequent player name over the imported files.
    pub player_name: Option<String>,
    pub totals: ProfileTotals,
    /// Most recently visited first.
    pub tracks: Vec<TrackSummary>,
    /// Most used first.
    pub vehicles: Vec<VehicleSummary>,
    /// Last 10 results.
    pub recent: Vec<ResultRow>,
}

/// How many recent results the profile carries.
pub const RECENT_RESULTS: usize = 10;

// ---------------------------------------------------------------------------
// Db
// ---------------------------------------------------------------------------

impl Db {
    /// Insert or replace (by `file_name`) a result and its laps. Returns the
    /// row id, which is kept across re-imports of the same file.
    pub fn upsert_result(&self, r: &NewResult) -> Result<i64> {
        let json_text = serde_json::to_vec(&r.json)?;
        let json_zstd = zstd::encode_all(&json_text[..], JSON_ZSTD_LEVEL)?;
        let names = serde_json::to_string(&r.driver_names)?;
        let row = &r.row;
        let tx = self.conn().unchecked_transaction()?;
        let id: i64 = tx.query_row(
            "INSERT INTO results (file_name, file_mtime_ms, imported_wall_ms, game_version, track_venue,
                track_course, track_length_m, session_kind, session_name, started_wall_ms, minutes,
                laps_planned, setting, server_name, player_name, player_position, player_class_position,
                player_grid_pos, player_class_grid_pos, player_class, player_vehicle, player_vehicle_name,
                player_best_lap_s, player_laps, player_finish, num_drivers, num_class_drivers,
                driver_names_json, json_zstd)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19,
                ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29)
             ON CONFLICT(file_name) DO UPDATE SET
                file_mtime_ms = excluded.file_mtime_ms, imported_wall_ms = excluded.imported_wall_ms,
                game_version = excluded.game_version, track_venue = excluded.track_venue,
                track_course = excluded.track_course, track_length_m = excluded.track_length_m,
                session_kind = excluded.session_kind, session_name = excluded.session_name,
                started_wall_ms = excluded.started_wall_ms, minutes = excluded.minutes,
                laps_planned = excluded.laps_planned, setting = excluded.setting,
                server_name = excluded.server_name, player_name = excluded.player_name,
                player_position = excluded.player_position,
                player_class_position = excluded.player_class_position,
                player_grid_pos = excluded.player_grid_pos,
                player_class_grid_pos = excluded.player_class_grid_pos,
                player_class = excluded.player_class, player_vehicle = excluded.player_vehicle,
                player_vehicle_name = excluded.player_vehicle_name,
                player_best_lap_s = excluded.player_best_lap_s, player_laps = excluded.player_laps,
                player_finish = excluded.player_finish, num_drivers = excluded.num_drivers,
                num_class_drivers = excluded.num_class_drivers,
                driver_names_json = excluded.driver_names_json, json_zstd = excluded.json_zstd
             RETURNING id",
            params![
                row.file_name,
                row.file_mtime_ms,
                now_ms(),
                row.game_version,
                row.track_venue,
                row.track_course,
                row.track_length_m,
                enum_str(&row.session_kind),
                row.session_name,
                row.started_wall_ms,
                row.minutes,
                row.laps_planned,
                row.setting,
                row.server_name,
                row.player_name,
                row.player_position,
                row.player_class_position,
                row.player_grid_pos,
                row.player_class_grid_pos,
                row.player_class,
                row.player_vehicle,
                row.player_vehicle_name,
                row.player_best_lap_s,
                row.player_laps,
                enum_str(&row.player_finish),
                row.num_drivers,
                row.num_class_drivers,
                names,
                json_zstd,
            ],
            |r| r.get(0),
        )?;
        tx.execute("DELETE FROM result_laps WHERE result_id = ?1", [id])?;
        {
            let mut stmt = tx.prepare(
                "INSERT OR REPLACE INTO result_laps (result_id, driver_name, is_player, lap_number, position,
                    elapsed_s, lap_time_s, s1, s2, s3, top_speed_kmh, fuel_fraction, front_compound,
                    rear_compound, tyre_wear_json, pit)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
            )?;
            for l in &r.laps {
                let wear = match &l.tyre_wear {
                    Some(w) => Some(serde_json::to_string(w)?),
                    None => None,
                };
                stmt.execute(params![
                    id,
                    l.driver_name,
                    l.is_player,
                    l.lap_number,
                    l.position,
                    l.elapsed_s,
                    l.lap_time_s,
                    l.s1,
                    l.s2,
                    l.s3,
                    l.top_speed_kmh,
                    l.fuel_fraction,
                    l.front_compound,
                    l.rear_compound,
                    wear,
                    l.pit,
                ])?;
            }
        }
        tx.commit()?;
        Ok(id)
    }

    pub fn list_results(&self, filter: &ResultFilter) -> Result<Vec<ResultRow>> {
        let limit = filter
            .limit
            .unwrap_or(DEFAULT_RESULTS_LIMIT)
            .clamp(1, MAX_RESULTS_LIMIT) as i64;
        let track = filter
            .track
            .as_deref()
            .filter(|t| !t.trim().is_empty())
            .map(|t| format!("%{}%", t.trim().to_lowercase()));
        let kind = filter.session_kind.map(|k| enum_str(&k));
        let vehicle = filter
            .vehicle
            .as_deref()
            .filter(|v| !v.trim().is_empty())
            .map(str::to_string);
        let mut stmt = self.conn().prepare(&format!(
            "SELECT {RESULT_COLUMNS} FROM results
                 WHERE (?1 IS NULL OR LOWER(track_course) LIKE ?1 OR LOWER(track_venue) LIKE ?1)
                   AND (?2 IS NULL OR session_kind = ?2)
                   AND (?3 IS NULL OR player_vehicle = ?3)
                 ORDER BY started_wall_ms DESC, id DESC
                 LIMIT ?4"
        ))?;
        let rows = stmt
            .query_map(params![track, kind, vehicle, limit], result_from_row)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn get_result(&self, id: i64) -> Result<Option<ResultDetail>> {
        let found = self
            .conn()
            .query_row(
                &format!("SELECT {RESULT_COLUMNS}, json_zstd FROM results WHERE id = ?1"),
                [id],
                |r| {
                    let row = result_from_row(r)?;
                    let blob: Vec<u8> = r.get("json_zstd")?;
                    Ok((row, blob))
                },
            )
            .optional()?;
        let Some((row, blob)) = found else {
            return Ok(None);
        };
        let json = zstd::decode_all(&blob[..])?;
        let file: serde_json::Value = serde_json::from_slice(&json)?;
        Ok(Some(ResultDetail {
            laps: self.result_laps_of(id)?,
            row,
            file,
        }))
    }

    pub fn result_laps_of(&self, id: i64) -> Result<Vec<ResultLapRow>> {
        let mut stmt = self.conn().prepare(
            "SELECT result_id, driver_name, is_player, lap_number, position, elapsed_s, lap_time_s, s1, s2, s3,
                    top_speed_kmh, fuel_fraction, front_compound, rear_compound, tyre_wear_json, pit
             FROM result_laps WHERE result_id = ?1 ORDER BY is_player DESC, driver_name, lap_number",
        )?;
        let rows = stmt
            .query_map([id], |r| {
                let wear: Option<String> = r.get(14)?;
                Ok(ResultLapRow {
                    result_id: r.get(0)?,
                    driver_name: r.get(1)?,
                    is_player: r.get(2)?,
                    lap_number: r.get(3)?,
                    position: r.get(4)?,
                    elapsed_s: r.get(5)?,
                    lap_time_s: r.get(6)?,
                    s1: r.get(7)?,
                    s2: r.get(8)?,
                    s3: r.get(9)?,
                    top_speed_kmh: r.get(10)?,
                    fuel_fraction: r.get(11)?,
                    front_compound: r.get(12)?,
                    rear_compound: r.get(13)?,
                    tyre_wear: wear.and_then(|w| serde_json::from_str(&w).ok()),
                    pit: r.get(15)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn delete_result(&self, id: i64) -> Result<bool> {
        Ok(self
            .conn()
            .execute("DELETE FROM results WHERE id = ?1", [id])?
            > 0)
    }

    pub fn result_count(&self) -> Result<i64> {
        Ok(self
            .conn()
            .query_row("SELECT COUNT(*) FROM results", [], |r| r.get(0))?)
    }

    /// Name and mtime of every imported file.
    pub fn imported_files(&self) -> Result<Vec<ImportedFile>> {
        let mut stmt = self
            .conn()
            .prepare("SELECT id, file_name, file_mtime_ms FROM results")?;
        let rows = stmt
            .query_map([], |r| {
                Ok(ImportedFile {
                    id: r.get(0)?,
                    file_name: r.get(1)?,
                    file_mtime_ms: r.get(2)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// For every driver name (lower-cased, trimmed) the number of imported
    /// files it appears in — the local player is in all of them.
    pub fn result_driver_counts(&self) -> Result<HashMap<String, u32>> {
        let mut stmt = self
            .conn()
            .prepare("SELECT driver_names_json FROM results")?;
        let mut counts: HashMap<String, u32> = HashMap::new();
        let names = stmt.query_map([], |r| r.get::<_, String>(0))?;
        for json in names {
            let list: Vec<String> = serde_json::from_str(&json?).unwrap_or_default();
            let mut seen_here: Vec<String> = Vec::new();
            for n in list {
                let key = n.trim().to_lowercase();
                if !seen_here.contains(&key) {
                    *counts.entry(key.clone()).or_default() += 1;
                    seen_here.push(key);
                }
            }
        }
        Ok(counts)
    }

    /// Player names used so far, most frequent first (hints for the next import).
    pub fn result_player_names(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn().prepare(
            "SELECT player_name FROM results GROUP BY player_name ORDER BY COUNT(*) DESC, MAX(started_wall_ms) DESC",
        )?;
        let rows = stmt
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Recorded session on the same track that started within `window_ms` of
    /// `started_wall_ms` (closest first). Track names are compared loosely:
    /// equal, or one contains the other (the live name is the course name).
    pub fn find_session_near(
        &self,
        started_wall_ms: i64,
        track: &str,
        window_ms: i64,
    ) -> Result<Option<String>> {
        let key = track.trim().to_lowercase();
        if key.is_empty() {
            return Ok(None);
        }
        let mut stmt = self.conn().prepare(
            "SELECT id, track_name, started_wall_ms FROM sessions
             WHERE started_wall_ms BETWEEN ?1 AND ?2
             ORDER BY ABS(started_wall_ms - ?3)",
        )?;
        let rows = stmt.query_map(
            params![
                started_wall_ms - window_ms,
                started_wall_ms + window_ms,
                started_wall_ms
            ],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
        )?;
        for row in rows {
            let (id, name) = row?;
            let n = name.trim().to_lowercase();
            if n == key || n.contains(&key) || key.contains(&n) {
                return Ok(Some(id));
            }
        }
        Ok(None)
    }

    /// Aggregates over every imported result.
    pub fn profile_summary(&self) -> Result<ProfileSummary> {
        let all = self.list_results(&ResultFilter {
            limit: Some(MAX_RESULTS_LIMIT),
            ..Default::default()
        })?;
        Ok(summarize(&all))
    }
}

/// Build the profile from result rows (newest first). Pure, so it is testable
/// without a database.
pub fn summarize(all: &[ResultRow]) -> ProfileSummary {
    let mut totals = ProfileTotals::default();
    let mut names: HashMap<&str, usize> = HashMap::new();
    let mut tracks: Vec<TrackSummary> = Vec::new();
    let mut vehicles: Vec<VehicleSummary> = Vec::new();
    let mut class_pos_sum: HashMap<usize, (f64, i64)> = HashMap::new();

    for r in all {
        *names.entry(r.player_name.as_str()).or_default() += 1;
        let km = f64::from(r.player_laps.max(0)) * f64::from(r.track_length_m) / 1000.0;
        let is_race = r.session_kind == SessionType::Race;
        totals.sessions += 1;
        totals.laps += i64::from(r.player_laps.max(0));
        totals.km += km;
        match r.session_kind {
            SessionType::Race => totals.races += 1,
            SessionType::Qualify => totals.qualifyings += 1,
            SessionType::Practice | SessionType::TestDay => totals.practices += 1,
            _ => {}
        }
        if is_race {
            if r.player_position == Some(1) {
                totals.wins += 1;
            }
            if matches!(r.player_position, Some(p) if p <= 3) {
                totals.podiums += 1;
            }
            if r.player_class_position == Some(1) {
                totals.class_wins += 1;
            }
            if matches!(r.player_class_position, Some(p) if p <= 3) {
                totals.class_podiums += 1;
            }
            if matches!(r.player_finish, FinishStatus::Dnf | FinishStatus::Dq) {
                totals.dnfs += 1;
            }
        }

        // --- per track
        let ti = match tracks
            .iter()
            .position(|t| t.track_course == r.track_course && t.track_venue == r.track_venue)
        {
            Some(i) => i,
            None => {
                tracks.push(TrackSummary {
                    track_venue: r.track_venue.clone(),
                    track_course: r.track_course.clone(),
                    track_length_m: r.track_length_m,
                    last_date: r.started_wall_ms,
                    ..Default::default()
                });
                tracks.len() - 1
            }
        };
        let t = &mut tracks[ti];
        t.sessions += 1;
        t.laps += i64::from(r.player_laps.max(0));
        t.km += km;
        t.last_date = t.last_date.max(r.started_wall_ms);
        if is_race {
            t.races += 1;
            if let Some(cp) = r.player_class_position.filter(|p| *p > 0) {
                t.best_class_position = Some(t.best_class_position.map_or(cp, |b| b.min(cp)));
                let e = class_pos_sum.entry(ti).or_default();
                e.0 += f64::from(cp);
                e.1 += 1;
            }
        }
        if let Some(b) = r.player_best_lap_s.filter(|b| *b > 0.0) {
            if t.best_lap_s.is_none_or(|cur| b < cur) {
                t.best_lap_s = Some(b);
                t.best_lap_vehicle = Some(r.player_vehicle.clone());
                t.best_lap_date = Some(r.started_wall_ms);
                t.best_lap_result_id = Some(r.id);
            }
        }

        // --- per vehicle
        let vi = match vehicles
            .iter()
            .position(|v| v.vehicle_name == r.player_vehicle && v.car_class == r.player_class)
        {
            Some(i) => i,
            None => {
                vehicles.push(VehicleSummary {
                    vehicle_name: r.player_vehicle.clone(),
                    car_class: r.player_class.clone(),
                    last_date: r.started_wall_ms,
                    ..Default::default()
                });
                vehicles.len() - 1
            }
        };
        let v = &mut vehicles[vi];
        v.sessions += 1;
        v.laps += i64::from(r.player_laps.max(0));
        v.km += km;
        v.last_date = v.last_date.max(r.started_wall_ms);
        if is_race {
            v.races += 1;
        }
        if let Some(b) = r.player_best_lap_s.filter(|b| *b > 0.0) {
            if v.best_lap_s.is_none_or(|cur| b < cur) {
                v.best_lap_s = Some(b);
                v.best_track = Some(r.track_course.clone());
                v.best_lap_date = Some(r.started_wall_ms);
                v.best_lap_result_id = Some(r.id);
            }
        }
    }
    for (ti, (sum, n)) in class_pos_sum {
        if n > 0 {
            tracks[ti].avg_class_position = Some((sum / n as f64) as f32);
        }
    }
    tracks.sort_by_key(|t| std::cmp::Reverse(t.last_date));
    vehicles.sort_by(|a, b| {
        b.sessions
            .cmp(&a.sessions)
            .then(b.last_date.cmp(&a.last_date))
    });
    let player_name = names
        .into_iter()
        .max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(a.0)))
        .map(|(n, _)| n.to_string());
    let mut recent: Vec<ResultRow> = all.iter().take(RECENT_RESULTS).cloned().collect();
    recent.sort_by_key(|r| std::cmp::Reverse(r.started_wall_ms));
    ProfileSummary {
        player_name,
        totals,
        tracks,
        vehicles,
        recent,
    }
}

const RESULT_COLUMNS: &str = "id, file_name, file_mtime_ms, imported_wall_ms, game_version, track_venue, track_course,
    track_length_m, session_kind, session_name, started_wall_ms, minutes, laps_planned, setting, server_name,
    player_name, player_position, player_class_position, player_grid_pos, player_class_grid_pos, player_class,
    player_vehicle, player_vehicle_name, player_best_lap_s, player_laps, player_finish, num_drivers,
    num_class_drivers";

fn result_from_row(r: &Row<'_>) -> rusqlite::Result<ResultRow> {
    let kind: String = r.get("session_kind")?;
    let finish: String = r.get("player_finish")?;
    Ok(ResultRow {
        id: r.get("id")?,
        file_name: r.get("file_name")?,
        file_mtime_ms: r.get("file_mtime_ms")?,
        imported_wall_ms: r.get("imported_wall_ms")?,
        game_version: r.get("game_version")?,
        track_venue: r.get("track_venue")?,
        track_course: r.get("track_course")?,
        track_length_m: r.get("track_length_m")?,
        session_kind: enum_parse(&kind),
        session_name: r.get("session_name")?,
        started_wall_ms: r.get("started_wall_ms")?,
        minutes: r.get("minutes")?,
        laps_planned: r.get("laps_planned")?,
        setting: r.get("setting")?,
        server_name: r.get("server_name")?,
        player_name: r.get("player_name")?,
        player_position: r.get("player_position")?,
        player_class_position: r.get("player_class_position")?,
        player_grid_pos: r.get("player_grid_pos")?,
        player_class_grid_pos: r.get("player_class_grid_pos")?,
        player_class: r.get("player_class")?,
        player_vehicle: r.get("player_vehicle")?,
        player_vehicle_name: r.get("player_vehicle_name")?,
        player_best_lap_s: r.get("player_best_lap_s")?,
        player_laps: r.get("player_laps")?,
        player_finish: enum_parse(&finish),
        num_drivers: r.get("num_drivers")?,
        num_class_drivers: r.get("num_class_drivers")?,
    })
}

// ---------------------------------------------------------------------------
// Store wrappers (the HTTP API uses these)
// ---------------------------------------------------------------------------

impl Store {
    pub fn upsert_result(&self, r: &NewResult) -> Result<i64> {
        self.db().upsert_result(r)
    }

    pub fn list_results(&self, filter: &ResultFilter) -> Result<Vec<ResultRow>> {
        self.db().list_results(filter)
    }

    /// `NotFound` when unknown.
    pub fn get_result(&self, id: i64) -> Result<ResultDetail> {
        self.db()
            .get_result(id)?
            .ok_or_else(|| StoreError::not_found(format!("result {id}")))
    }

    pub fn profile_summary(&self) -> Result<ProfileSummary> {
        self.db().profile_summary()
    }

    pub fn imported_files(&self) -> Result<Vec<ImportedFile>> {
        self.db().imported_files()
    }

    pub fn result_driver_counts(&self) -> Result<HashMap<String, u32>> {
        self.db().result_driver_counts()
    }

    pub fn result_player_names(&self) -> Result<Vec<String>> {
        self.db().result_player_names()
    }

    pub fn find_session_near(
        &self,
        started_wall_ms: i64,
        track: &str,
        window_ms: i64,
    ) -> Result<Option<String>> {
        self.db()
            .find_session_near(started_wall_ms, track, window_ms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::SessionRow;
    use corde_protocol::{GameId, VehicleClass, Weather};

    fn row(id: i64, file: &str, kind: SessionType, course: &str, started: i64) -> ResultRow {
        ResultRow {
            id,
            file_name: file.into(),
            file_mtime_ms: 1,
            track_venue: format!("{course} venue"),
            track_course: course.into(),
            track_length_m: 5000.0,
            session_kind: kind,
            session_name: "Race".into(),
            started_wall_ms: started,
            player_name: "Pilote".into(),
            player_class: "GT3".into(),
            player_vehicle: "Mercedes-AMG LMGT3".into(),
            player_vehicle_name: "Mercedes AMG #397".into(),
            player_laps: 10,
            player_finish: FinishStatus::Finished,
            num_drivers: 20,
            num_class_drivers: 10,
            ..Default::default()
        }
    }

    fn new_result(row: ResultRow, names: &[&str]) -> NewResult {
        let laps = (1..=row.player_laps)
            .map(|n| ResultLapRow {
                driver_name: row.player_name.clone(),
                is_player: true,
                lap_number: n,
                position: Some(5),
                elapsed_s: Some(f64::from(n) * 100.0),
                lap_time_s: if n == 1 {
                    None
                } else {
                    Some(100.0 + f32::from(n as u8))
                },
                s1: Some(30.0),
                s2: Some(30.0),
                s3: Some(40.0 + f32::from(n as u8)),
                front_compound: Some("Medium".into()),
                rear_compound: Some("Medium".into()),
                tyre_wear: (n == 2).then_some([0.9, 0.91, 0.92, 0.93]),
                pit: n == 1,
                ..Default::default()
            })
            .collect();
        NewResult {
            driver_names: names.iter().map(|s| s.to_string()).collect(),
            laps,
            json: serde_json::json!({ "file": row.file_name, "drivers": names }),
            row,
        }
    }

    #[test]
    fn upsert_list_get_and_counts() {
        let db = Db::open_in_memory().unwrap();
        assert!(db.schema_version().unwrap() >= 2);
        let mut r1 = row(
            0,
            "2026_11_01_21_05_02-54R1.xml",
            SessionType::Race,
            "Paul Ricard - 1A",
            1_793_563_502_000,
        );
        r1.player_position = Some(23);
        r1.player_class_position = Some(12);
        r1.player_best_lap_s = Some(118.3745);
        let id1 = db
            .upsert_result(&new_result(r1.clone(), &["Pilote", "Other", "Third"]))
            .unwrap();
        assert!(id1 > 0);
        let r2 = row(
            0,
            "2026_11_01_20_10_54-54P1.xml",
            SessionType::Practice,
            "Silverstone International Circuit",
            1_793_560_254_000,
        );
        let id2 = db
            .upsert_result(&new_result(r2.clone(), &["Pilote", "Someone"]))
            .unwrap();
        assert_ne!(id1, id2);

        // Re-import keeps the id, replaces the laps.
        let mut r1b = r1.clone();
        r1b.file_mtime_ms = 2;
        r1b.player_laps = 3;
        let again = db
            .upsert_result(&new_result(r1b, &["Pilote", "Other"]))
            .unwrap();
        assert_eq!(again, id1);
        assert_eq!(db.result_laps_of(id1).unwrap().len(), 3);
        assert_eq!(db.result_count().unwrap(), 2);

        let all = db.list_results(&ResultFilter::default()).unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].id, id1, "most recent first");
        assert_eq!(all[0].file_mtime_ms, 2);
        assert_eq!(all[0].player_finish, FinishStatus::Finished);
        let f = ResultFilter {
            track: Some("silver".into()),
            ..Default::default()
        };
        assert_eq!(db.list_results(&f).unwrap().len(), 1);
        let f = ResultFilter {
            session_kind: Some(SessionType::Race),
            vehicle: Some("Mercedes-AMG LMGT3".into()),
            ..Default::default()
        };
        assert_eq!(db.list_results(&f).unwrap()[0].id, id1);
        let f = ResultFilter {
            vehicle: Some("Nope".into()),
            ..Default::default()
        };
        assert!(db.list_results(&f).unwrap().is_empty());

        let d = db.get_result(id1).unwrap().unwrap();
        assert_eq!(d.row.player_best_lap_s, Some(118.3745));
        assert_eq!(d.file["file"], "2026_11_01_21_05_02-54R1.xml");
        assert_eq!(d.laps.len(), 3);
        assert_eq!(d.laps[0].lap_time_s, None);
        assert!(d.laps[0].pit);
        assert_eq!(d.laps[1].tyre_wear, Some([0.9, 0.91, 0.92, 0.93]));
        assert_eq!(d.laps[2].tyre_wear, None);
        assert!(db.get_result(999).unwrap().is_none());

        let files = db.imported_files().unwrap();
        assert_eq!(files.len(), 2);
        let counts = db.result_driver_counts().unwrap();
        assert_eq!(counts["pilote"], 2);
        assert_eq!(counts["other"], 1);
        assert_eq!(counts.get("third"), None, "replaced on re-import");
        assert_eq!(
            db.result_player_names().unwrap(),
            vec!["Pilote".to_string()]
        );

        assert!(db.delete_result(id2).unwrap());
        assert!(!db.delete_result(id2).unwrap());
        assert!(db.result_laps_of(id2).unwrap().is_empty());
    }

    #[test]
    fn links_results_to_recorded_sessions() {
        let db = Db::open_in_memory().unwrap();
        let s = SessionRow {
            id: "20261101-210500-paul-ricard-1a-race".into(),
            game: GameId::Lmu,
            track_name: "Paul Ricard - 1A".into(),
            track_length_m: 5698.6,
            session_type: SessionType::Race,
            session_index: 10,
            started_wall_ms: 1_793_563_502_000 + 90_000,
            ended_wall_ms: Some(1_793_565_000_000),
            player_name: "alex martin".into(),
            vehicle_name: "Mercedes".into(),
            vehicle_model: "Mercedes-AMG LMGT3".into(),
            vehicle_class: VehicleClass::Gt3,
            class_name: "GT3".into(),
            server_name: String::new(),
            is_multiplayer: true,
            game_version: None,
            source: "lmu".into(),
            record_rate_hz: 50.0,
            sample_count: 0,
            lap_count: 15,
            best_lap_s: Some(118.4),
            weather: Weather::default(),
            notes: None,
        };
        db.insert_session(&s).unwrap();
        let ten_min = 10 * 60 * 1000;
        assert_eq!(
            db.find_session_near(1_793_563_502_000, "Paul Ricard - 1A", ten_min)
                .unwrap(),
            Some(s.id.clone())
        );
        // loose match on the venue name, and on case
        assert_eq!(
            db.find_session_near(1_793_563_502_000, "paul ricard", ten_min)
                .unwrap(),
            Some(s.id.clone())
        );
        assert_eq!(
            db.find_session_near(1_793_563_502_000, "Silverstone", ten_min)
                .unwrap(),
            None
        );
        assert_eq!(
            db.find_session_near(1_793_563_502_000 - 2 * ten_min, "Paul Ricard - 1A", ten_min)
                .unwrap(),
            None
        );
        assert_eq!(
            db.find_session_near(1_793_563_502_000, "", ten_min)
                .unwrap(),
            None
        );
    }

    #[test]
    fn profile_aggregates() {
        let mut rows = vec![
            row(1, "a", SessionType::Race, "Paul Ricard - 1A", 3_000),
            row(2, "b", SessionType::Race, "Paul Ricard - 1A", 2_000),
            row(3, "c", SessionType::Practice, "Silverstone", 1_000),
            row(4, "d", SessionType::Qualify, "Silverstone", 500),
        ];
        rows[0].player_position = Some(1);
        rows[0].player_class_position = Some(1);
        rows[0].player_best_lap_s = Some(120.0);
        rows[1].player_position = Some(5);
        rows[1].player_class_position = Some(3);
        rows[1].player_best_lap_s = Some(118.0);
        rows[1].player_vehicle = "BMW M4 LMGT3".into();
        rows[1].player_finish = FinishStatus::Dnf;
        rows[2].player_best_lap_s = Some(62.0);
        rows[2].player_laps = 56;
        rows[2].track_length_m = 2962.1;
        rows[3].player_best_lap_s = None;
        rows[3].player_laps = 0;
        rows[3].player_name = "Autre".into();

        let p = summarize(&rows);
        assert_eq!(p.player_name.as_deref(), Some("Pilote"));
        assert_eq!(p.totals.sessions, 4);
        assert_eq!(p.totals.races, 2);
        assert_eq!(p.totals.qualifyings, 1);
        assert_eq!(p.totals.practices, 1);
        assert_eq!(p.totals.laps, 76);
        assert!((p.totals.km - (20.0 * 5.0 + 56.0 * 2.9621)).abs() < 1e-3);
        assert_eq!(p.totals.wins, 1);
        assert_eq!(p.totals.podiums, 1);
        assert_eq!(p.totals.class_wins, 1);
        assert_eq!(p.totals.class_podiums, 2);
        assert_eq!(p.totals.dnfs, 1);

        assert_eq!(p.tracks.len(), 2);
        let pr = &p.tracks[0];
        assert_eq!(pr.track_course, "Paul Ricard - 1A");
        assert_eq!(pr.sessions, 2);
        assert_eq!(pr.races, 2);
        assert_eq!(pr.best_lap_s, Some(118.0));
        assert_eq!(pr.best_lap_vehicle.as_deref(), Some("BMW M4 LMGT3"));
        assert_eq!(pr.best_lap_result_id, Some(2));
        assert_eq!(pr.best_class_position, Some(1));
        assert_eq!(pr.avg_class_position, Some(2.0));
        assert_eq!(pr.last_date, 3_000);
        let sv = &p.tracks[1];
        assert_eq!(sv.races, 0);
        assert_eq!(sv.best_class_position, None);
        assert_eq!(sv.best_lap_s, Some(62.0));

        assert_eq!(p.vehicles.len(), 2);
        assert_eq!(p.vehicles[0].vehicle_name, "Mercedes-AMG LMGT3");
        assert_eq!(p.vehicles[0].sessions, 3);
        assert_eq!(p.vehicles[0].best_lap_s, Some(62.0));
        assert_eq!(p.vehicles[0].best_track.as_deref(), Some("Silverstone"));
        assert_eq!(p.vehicles[1].vehicle_name, "BMW M4 LMGT3");
        assert_eq!(p.recent.len(), 4);
        assert_eq!(p.recent[0].id, 1);

        assert_eq!(summarize(&[]), ProfileSummary::default());
    }
}
