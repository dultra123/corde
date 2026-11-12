//! SQLite side of the store: `<data-dir>/corde.db` (WAL), versioned
//! migrations, typed rows for `sessions / laps / events / setups /
//! sample_files` (v1), `results / result_laps` (v2, see `results.rs`),
//! `analyses` (v3, cached engineer reports), `feedback` (v4, what the driver
//! felt, stored as typed in the UI; v6 adds its interpretation) and
//! `plan_iterations` (v6, the engineer's answers to the feedback).
//!
//! One [`Db`] is one connection. The recorder owns its own connection on the
//! source thread; the HTTP API uses the one held by [`crate::Store`]. WAL mode
//! lets them run concurrently (readers never block the writer).

use crate::error::{Result, StoreError};
use corde_protocol::{
    Feedback, FeedbackInterpretation, GameEvent, GameId, LapNote, LapRecord, NewFeedback,
    PlanIteration, SessionType, SetupPhilosophy, VehicleClass, Weather,
};
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Duration;

/// Name of the database file inside the data directory.
pub const DB_FILE: &str = "corde.db";

/// Current schema version (= number of migrations).
pub const SCHEMA_VERSION: i64 = MIGRATIONS.len() as i64;

/// Added (on read) to an iteration computed in parallel with another one built on the same
/// previous iteration — feedback typed on the PC and on the home server between two syncs.
/// Followed by the number of the other iteration.
pub const PARALLEL_NOTE_FR: &str = "Calculée en parallèle d'une autre itération (ressentis saisis sur le PC et sur le serveur Corde entre deux synchronisations) : elle ne tient pas compte de l'itération n° ";

/// Migrations, applied in order inside one transaction each. Append only.
const MIGRATIONS: &[&str] = &[
    // v1 — initial schema.
    r#"
    CREATE TABLE sessions (
        id               TEXT PRIMARY KEY,
        game             TEXT NOT NULL,
        track_name       TEXT NOT NULL,
        track_length_m   REAL NOT NULL,
        session_type     TEXT NOT NULL,
        session_index    INTEGER NOT NULL,
        started_wall_ms  INTEGER NOT NULL,
        ended_wall_ms    INTEGER,
        player_name      TEXT NOT NULL,
        vehicle_name     TEXT NOT NULL,
        vehicle_model    TEXT NOT NULL,
        vehicle_class    TEXT NOT NULL,
        class_name       TEXT NOT NULL,
        server_name      TEXT NOT NULL,
        is_multiplayer   INTEGER NOT NULL,
        game_version     TEXT,
        source           TEXT NOT NULL,
        record_rate_hz   REAL NOT NULL,
        sample_count     INTEGER NOT NULL DEFAULT 0,
        lap_count        INTEGER NOT NULL DEFAULT 0,
        best_lap_s       REAL,
        weather_json     TEXT NOT NULL,
        notes            TEXT
    );
    CREATE INDEX sessions_started ON sessions(started_wall_ms DESC);
    CREATE INDEX sessions_track ON sessions(track_name);

    CREATE TABLE laps (
        session_id                 TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
        lap_number                 INTEGER NOT NULL,
        lap_time_s                 REAL NOT NULL,
        s1                         REAL,
        s2                         REAL,
        s3                         REAL,
        valid                      INTEGER NOT NULL,
        in_lap                     INTEGER NOT NULL,
        out_lap                    INTEGER NOT NULL,
        fuel_used_l                REAL NOT NULL,
        virtual_energy_used        REAL,
        top_speed_kmh              REAL NOT NULL,
        front_compound             TEXT NOT NULL,
        rear_compound              TEXT NOT NULL,
        avg_tyre_temp_c_json       TEXT NOT NULL,
        avg_tyre_pressure_kpa_json TEXT NOT NULL,
        max_brake_temp_c_json      TEXT NOT NULL,
        wear_remaining_end_json    TEXT NOT NULL,
        track_c                    REAL NOT NULL,
        ambient_c                  REAL NOT NULL,
        rain                       REAL NOT NULL,
        session_elapsed_end_s      REAL NOT NULL,
        sample_file                TEXT NOT NULL,
        sample_row_start           INTEGER NOT NULL,
        sample_row_end             INTEGER NOT NULL,
        PRIMARY KEY (session_id, lap_number)
    );

    CREATE TABLE events (
        id                INTEGER PRIMARY KEY,
        session_id        TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
        wall_ms           INTEGER NOT NULL,
        session_elapsed_s REAL NOT NULL,
        lap_number        INTEGER NOT NULL,
        kind              TEXT NOT NULL,
        payload_json      TEXT NOT NULL
    );
    CREATE INDEX events_session ON events(session_id, id);

    CREATE TABLE setups (
        id               INTEGER PRIMARY KEY,
        session_id       TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
        captured_wall_ms INTEGER NOT NULL,
        source           TEXT NOT NULL CHECK (source IN ('rest', 'svm', 'manual')),
        name             TEXT NOT NULL,
        content_json     TEXT NOT NULL
    );
    CREATE INDEX setups_session ON setups(session_id, id);

    -- One row per Parquet file of a session: which global sample rows it holds.
    CREATE TABLE sample_files (
        session_id  TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
        file_index  INTEGER NOT NULL,
        name        TEXT NOT NULL,
        row_start   INTEGER NOT NULL,
        row_end     INTEGER NOT NULL,
        bytes       INTEGER NOT NULL DEFAULT 0,
        closed      INTEGER NOT NULL DEFAULT 0,
        PRIMARY KEY (session_id, file_index)
    );
    "#,
    // v2 — results files imported from the game (`UserData/Log/Results/*.xml`).
    r#"
    CREATE TABLE results (
        id                     INTEGER PRIMARY KEY,
        file_name              TEXT NOT NULL UNIQUE,
        file_mtime_ms          INTEGER NOT NULL,
        imported_wall_ms       INTEGER NOT NULL,
        game_version           TEXT,
        track_venue            TEXT NOT NULL,
        track_course           TEXT NOT NULL,
        track_length_m         REAL NOT NULL,
        session_kind           TEXT NOT NULL,
        session_name           TEXT NOT NULL,
        started_wall_ms        INTEGER NOT NULL,
        minutes                REAL,
        laps_planned           INTEGER,
        setting                TEXT,
        server_name            TEXT,
        player_name            TEXT NOT NULL,
        player_position        INTEGER,
        player_class_position  INTEGER,
        player_grid_pos        INTEGER,
        player_class_grid_pos  INTEGER,
        player_class           TEXT NOT NULL,
        player_vehicle         TEXT NOT NULL,
        player_vehicle_name    TEXT NOT NULL,
        player_best_lap_s      REAL,
        player_laps            INTEGER NOT NULL,
        player_finish          TEXT NOT NULL,
        num_drivers            INTEGER NOT NULL,
        num_class_drivers      INTEGER NOT NULL,
        -- every driver of the file (JSON array): used to tell the local player apart
        driver_names_json      TEXT NOT NULL,
        -- the whole parsed file as JSON, zstd-compressed
        json_zstd              BLOB NOT NULL
    );
    CREATE INDEX results_started ON results(started_wall_ms DESC);
    CREATE INDEX results_track ON results(track_course);

    CREATE TABLE result_laps (
        result_id       INTEGER NOT NULL REFERENCES results(id) ON DELETE CASCADE,
        driver_name     TEXT NOT NULL,
        is_player       INTEGER NOT NULL,
        lap_number      INTEGER NOT NULL,
        position        INTEGER,
        elapsed_s       REAL,
        lap_time_s      REAL,
        s1              REAL,
        s2              REAL,
        s3              REAL,
        top_speed_kmh   REAL,
        fuel_fraction   REAL,
        front_compound  TEXT,
        rear_compound   TEXT,
        tyre_wear_json  TEXT,
        pit             INTEGER NOT NULL DEFAULT 0,
        PRIMARY KEY (result_id, driver_name, lap_number)
    );
    "#,
    // v3 — cached post-session analysis reports (`corde-engineer`), one per session.
    r#"
    CREATE TABLE analyses (
        session_id    TEXT PRIMARY KEY REFERENCES sessions(id) ON DELETE CASCADE,
        generated_ms  INTEGER NOT NULL,
        -- the whole `AnalysisReport` as JSON
        report_json   TEXT NOT NULL
    );
    "#,
    // v4 — the driver's own feedback on a session (free text + the plan tried), stored only
    // for now: the engineer will read it in a later lot.
    r#"
    CREATE TABLE feedback (
        id          INTEGER PRIMARY KEY AUTOINCREMENT,
        session_id  TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
        created_ms  INTEGER NOT NULL,
        -- `aggressive` | `safe` | `wet`, NULL when no plan was tried
        plan        TEXT,
        text        TEXT NOT NULL
    );
    CREATE INDEX feedback_session ON feedback(session_id, created_ms);
    "#,
    // v5 — the game is the referee of lap times. `timed_by_game` = the lap time is the
    // game's official one; `note` = why a lap is special (`out_lap`, `in_lap`, `not_timed`,
    // `aborted`, `start_lap`); `time_source` = `live` (read while driving) or `results`
    // (corrected or added from the game's result file). `reconciled_result_id` = the
    // result file the laps of a session were checked against.
    r#"
    ALTER TABLE laps ADD COLUMN timed_by_game INTEGER NOT NULL DEFAULT 1;
    ALTER TABLE laps ADD COLUMN note TEXT;
    ALTER TABLE laps ADD COLUMN time_source TEXT NOT NULL DEFAULT 'live';
    ALTER TABLE sessions ADD COLUMN reconciled_result_id INTEGER;
    "#,
    // v6 — the engineer reads the feedback (`interpretation_json` = `FeedbackInterpretation`)
    // and answers with plan iterations (`plan_iterations`, one row per feedback / philosophy).
    r#"
    ALTER TABLE feedback ADD COLUMN interpretation_json TEXT NULL;
    CREATE TABLE plan_iterations (
        id             INTEGER PRIMARY KEY AUTOINCREMENT,
        session_id     TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
        created_ms     INTEGER NOT NULL,
        -- 1, 2, 3… per (session, base_plan)
        iteration      INTEGER NOT NULL,
        -- `aggressive` | `safe` | `wet`
        base_plan      TEXT NOT NULL,
        feedback_id    INTEGER NULL REFERENCES feedback(id) ON DELETE SET NULL,
        -- the whole `PlanIteration` as JSON
        iteration_json TEXT NOT NULL
    );
    CREATE INDEX plan_iterations_session ON plan_iterations(session_id, base_plan, iteration);
    "#,
    // v7 — home server (`corde-server --role hub`, ADR 0010). Stable identifiers for what two
    // databases may both create (`feedback.uid`, `plan_iterations.uid`, random 128-bit hex), the
    // iteration each one was built on (`parent_uid`: two iterations with the same parent were
    // computed in parallel on the PC and on the server), and the server's bookkeeping of what a
    // PC sent (`sync_received`, `sync_files`) or what it deleted for good (`sync_tombstones`).
    r#"
    ALTER TABLE feedback ADD COLUMN uid TEXT;
    UPDATE feedback SET uid = lower(hex(randomblob(16))) WHERE uid IS NULL;
    CREATE UNIQUE INDEX feedback_uid ON feedback(uid);
    ALTER TABLE plan_iterations ADD COLUMN uid TEXT;
    ALTER TABLE plan_iterations ADD COLUMN parent_uid TEXT;
    UPDATE plan_iterations SET uid = lower(hex(randomblob(16))) WHERE uid IS NULL;
    UPDATE plan_iterations SET parent_uid = (
        SELECT p.uid FROM plan_iterations p
        WHERE p.session_id = plan_iterations.session_id
          AND p.base_plan = plan_iterations.base_plan
          AND (p.iteration < plan_iterations.iteration
               OR (p.iteration = plan_iterations.iteration AND p.id < plan_iterations.id))
        ORDER BY p.iteration DESC, p.id DESC LIMIT 1);
    CREATE UNIQUE INDEX plan_iterations_uid ON plan_iterations(uid);
    CREATE TABLE sync_received (
        -- `session` | `result`
        kind         TEXT NOT NULL,
        -- session id, or result file name
        key          TEXT NOT NULL,
        -- SHA-256 (hex) of the bundle as the PC sent it
        fingerprint  TEXT NOT NULL,
        received_ms  INTEGER NOT NULL,
        PRIMARY KEY (kind, key)
    );
    CREATE TABLE sync_files (
        session_id  TEXT NOT NULL,
        name        TEXT NOT NULL,
        sha256      TEXT NOT NULL,
        bytes       INTEGER NOT NULL,
        PRIMARY KEY (session_id, name)
    );
    CREATE TABLE sync_tombstones (
        session_id  TEXT PRIMARY KEY,
        deleted_ms  INTEGER NOT NULL
    );
    "#,
];

// ---------------------------------------------------------------------------
// Rows
// ---------------------------------------------------------------------------

/// A row of `sessions` (what `GET /api/sessions` returns).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionRow {
    pub id: String,
    pub game: GameId,
    pub track_name: String,
    pub track_length_m: f32,
    pub session_type: SessionType,
    pub session_index: i32,
    pub started_wall_ms: i64,
    pub ended_wall_ms: Option<i64>,
    pub player_name: String,
    pub vehicle_name: String,
    pub vehicle_model: String,
    pub vehicle_class: VehicleClass,
    pub class_name: String,
    pub server_name: String,
    pub is_multiplayer: bool,
    pub game_version: Option<String>,
    pub source: String,
    pub record_rate_hz: f32,
    pub sample_count: i64,
    pub lap_count: i64,
    pub best_lap_s: Option<f32>,
    /// Weather at the start of the session.
    pub weather: Weather,
    pub notes: Option<String>,
}

/// A row of `laps`: the protocol's [`LapRecord`] plus where its samples live
/// (global, 0-based row indices across the session's Parquet files; `end` exclusive).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LapRow {
    pub session_id: String,
    #[serde(flatten)]
    pub lap: LapRecord,
    /// File holding the first sample of the lap (`samples-000.parquet`).
    pub sample_file: String,
    pub sample_row_start: i64,
    pub sample_row_end: i64,
    /// Where the timing comes from: `live` (read while driving) or `results`
    /// (corrected or added from the game's result file).
    #[serde(default = "default_time_source")]
    pub time_source: String,
}

fn default_time_source() -> String {
    TIME_SOURCE_LIVE.to_string()
}

/// [`LapRow::time_source`]: timing read live from the game.
pub const TIME_SOURCE_LIVE: &str = "live";
/// [`LapRow::time_source`]: timing taken from the game's result file.
pub const TIME_SOURCE_RESULTS: &str = "results";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventRow {
    pub id: i64,
    pub session_id: String,
    pub wall_ms: i64,
    pub session_elapsed_s: f64,
    pub lap_number: i32,
    /// `GameEvent` tag (`pit_entry`, `flag_changed`…).
    pub kind: String,
    /// The whole event as JSON (`{"kind": …, …}`).
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SetupRow {
    pub id: i64,
    pub session_id: String,
    pub captured_wall_ms: i64,
    /// `rest` | `svm` | `manual`.
    pub source: String,
    pub name: String,
    pub content: serde_json::Value,
}

/// A row of `analyses` (schema v3): the cached report of a session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnalysisRow {
    pub session_id: String,
    /// Unix ms when the report was computed.
    pub generated_ms: i64,
    /// The `AnalysisReport` as JSON text.
    pub report_json: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SampleFileRow {
    pub session_id: String,
    pub file_index: i32,
    pub name: String,
    pub row_start: i64,
    pub row_end: i64,
    pub bytes: i64,
    pub closed: bool,
}

/// Filter of [`Db::list_sessions`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SessionFilter {
    /// Case-insensitive substring of `track_name`.
    pub track: Option<String>,
    /// Exact `session_type` (`race`, `practice`…).
    pub session_type: Option<SessionType>,
    /// Most recent first; default 50, max 1000.
    pub limit: Option<usize>,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Serde name of a unit enum variant (`GameId::Lmu` → `"lmu"`).
pub fn enum_str<T: Serialize>(v: &T) -> String {
    match serde_json::to_value(v) {
        Ok(serde_json::Value::String(s)) => s,
        Ok(other) => other.to_string(),
        Err(_) => "unknown".into(),
    }
}

/// Inverse of [`enum_str`]; falls back to `Default` for unknown names.
pub fn enum_parse<T: for<'de> Deserialize<'de> + Default>(s: &str) -> T {
    serde_json::from_value(serde_json::Value::String(s.to_string())).unwrap_or_default()
}

fn json_arr(a: &[f32; 4]) -> String {
    serde_json::to_string(a).unwrap_or_else(|_| "[0,0,0,0]".into())
}

fn parse_arr(s: &str) -> [f32; 4] {
    serde_json::from_str(s).unwrap_or([0.0; 4])
}

// ---------------------------------------------------------------------------
// Db
// ---------------------------------------------------------------------------

/// One SQLite connection with the schema applied.
pub struct Db {
    conn: Connection,
}

impl std::fmt::Debug for Db {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Db").finish()
    }
}

impl Db {
    /// Open (creating it) the database at `path`, switch it to WAL and apply
    /// pending migrations. Idempotent.
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        conn.busy_timeout(Duration::from_secs(5))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let mut db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    /// In-memory database (tests).
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let mut db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    /// Applied schema version.
    pub fn schema_version(&self) -> Result<i64> {
        Ok(self
            .conn
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM schema_version",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0))
    }

    fn migrate(&mut self) -> Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_version (
                version INTEGER PRIMARY KEY,
                applied_wall_ms INTEGER NOT NULL
            );",
        )?;
        let current = self.schema_version()?;
        for (i, sql) in MIGRATIONS.iter().enumerate() {
            let version = i as i64 + 1;
            if version <= current {
                continue;
            }
            let tx = self.conn.transaction()?;
            tx.execute_batch(sql)?;
            tx.execute(
                "INSERT INTO schema_version (version, applied_wall_ms) VALUES (?1, ?2)",
                params![version, now_ms()],
            )?;
            tx.commit()?;
            tracing::info!("corde.db: schema migrated to v{version}");
        }
        Ok(())
    }

    // --- sessions -----------------------------------------------------------

    pub fn insert_session(&self, s: &SessionRow) -> Result<()> {
        self.conn.execute(
            "INSERT INTO sessions (id, game, track_name, track_length_m, session_type, session_index,
                started_wall_ms, ended_wall_ms, player_name, vehicle_name, vehicle_model, vehicle_class,
                class_name, server_name, is_multiplayer, game_version, source, record_rate_hz,
                sample_count, lap_count, best_lap_s, weather_json, notes)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18,
                ?19, ?20, ?21, ?22, ?23)",
            params![
                s.id,
                enum_str(&s.game),
                s.track_name,
                s.track_length_m,
                enum_str(&s.session_type),
                s.session_index,
                s.started_wall_ms,
                s.ended_wall_ms,
                s.player_name,
                s.vehicle_name,
                s.vehicle_model,
                enum_str(&s.vehicle_class),
                s.class_name,
                s.server_name,
                s.is_multiplayer,
                s.game_version,
                s.source,
                s.record_rate_hz,
                s.sample_count,
                s.lap_count,
                s.best_lap_s,
                serde_json::to_string(&s.weather)?,
                s.notes,
            ],
        )?;
        Ok(())
    }

    pub fn session_exists(&self, id: &str) -> Result<bool> {
        Ok(self
            .conn
            .query_row("SELECT 1 FROM sessions WHERE id = ?1", [id], |_| Ok(()))
            .optional()?
            .is_some())
    }

    /// Running counters, written at every flush so that a crash loses little.
    pub fn update_session_progress(
        &self,
        id: &str,
        sample_count: i64,
        lap_count: i64,
        best_lap_s: Option<f32>,
    ) -> Result<()> {
        self.conn.execute(
            "UPDATE sessions SET sample_count = ?2, lap_count = ?3, best_lap_s = ?4 WHERE id = ?1",
            params![id, sample_count, lap_count, best_lap_s],
        )?;
        Ok(())
    }

    pub fn close_session(
        &self,
        id: &str,
        ended_wall_ms: i64,
        sample_count: i64,
        lap_count: i64,
        best_lap_s: Option<f32>,
        notes: Option<&str>,
    ) -> Result<()> {
        self.conn.execute(
            "UPDATE sessions SET ended_wall_ms = ?2, sample_count = ?3, lap_count = ?4, best_lap_s = ?5,
                notes = COALESCE(?6, notes) WHERE id = ?1",
            params![id, ended_wall_ms, sample_count, lap_count, best_lap_s, notes],
        )?;
        Ok(())
    }

    /// Sessions left open by a crash (`ended_wall_ms IS NULL`): close them with
    /// the best end time we have (newest file in `<sessions_dir>/<id>`, else
    /// last event, else start). Returns the ids that were repaired.
    pub fn recover_open_sessions(&self, sessions_dir: Option<&Path>) -> Result<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, started_wall_ms FROM sessions WHERE ended_wall_ms IS NULL")?;
        let open: Vec<(String, i64)> = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<std::result::Result<_, _>>()?;
        drop(stmt);
        for (id, started) in &open {
            let last_event: Option<i64> = self
                .conn
                .query_row(
                    "SELECT MAX(wall_ms) FROM events WHERE session_id = ?1",
                    [id],
                    |r| r.get(0),
                )
                .optional()?
                .flatten();
            let newest_file = sessions_dir.and_then(|d| newest_mtime_ms(&d.join(id)));
            let ended = newest_file.or(last_event).unwrap_or(*started).max(*started);
            let (laps, best): (i64, Option<f32>) = self.conn.query_row(
                "SELECT COUNT(*), MIN(CASE WHEN valid THEN lap_time_s END) FROM laps WHERE session_id = ?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            let samples: i64 = self
                .conn
                .query_row(
                    "SELECT COALESCE(MAX(row_end), 0) FROM sample_files WHERE session_id = ?1",
                    [id],
                    |r| r.get(0),
                )
                .unwrap_or(0);
            self.close_session(
                id,
                ended,
                samples,
                laps,
                best,
                Some("recovered: not closed cleanly (crash or power loss)"),
            )?;
            tracing::warn!("session {id} was not closed cleanly — repaired");
        }
        Ok(open.into_iter().map(|(id, _)| id).collect())
    }

    pub fn list_sessions(&self, filter: &SessionFilter) -> Result<Vec<SessionRow>> {
        let limit = filter.limit.unwrap_or(50).clamp(1, 1000) as i64;
        let track = filter
            .track
            .as_deref()
            .filter(|t| !t.trim().is_empty())
            .map(|t| format!("%{}%", t.trim().to_lowercase()));
        let kind = filter.session_type.map(|t| enum_str(&t));
        let mut stmt = self.conn.prepare(
            "SELECT * FROM sessions
             WHERE (?1 IS NULL OR LOWER(track_name) LIKE ?1)
               AND (?2 IS NULL OR session_type = ?2)
             ORDER BY started_wall_ms DESC, id DESC
             LIMIT ?3",
        )?;
        let rows = stmt
            .query_map(params![track, kind, limit], session_from_row)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn get_session(&self, id: &str) -> Result<Option<SessionRow>> {
        Ok(self
            .conn
            .query_row(
                "SELECT * FROM sessions WHERE id = ?1",
                [id],
                session_from_row,
            )
            .optional()?)
    }

    pub fn delete_session(&self, id: &str) -> Result<bool> {
        let n = self
            .conn
            .execute("DELETE FROM sessions WHERE id = ?1", [id])?;
        Ok(n > 0)
    }

    pub fn session_count(&self) -> Result<i64> {
        Ok(self
            .conn
            .query_row("SELECT COUNT(*) FROM sessions", [], |r| r.get(0))?)
    }

    // --- laps ---------------------------------------------------------------

    pub fn insert_lap(&self, l: &LapRow) -> Result<()> {
        let r = &l.lap;
        self.conn.execute(
            "INSERT OR REPLACE INTO laps (session_id, lap_number, lap_time_s, s1, s2, s3, valid, in_lap,
                out_lap, fuel_used_l, virtual_energy_used, top_speed_kmh, front_compound, rear_compound,
                avg_tyre_temp_c_json, avg_tyre_pressure_kpa_json, max_brake_temp_c_json,
                wear_remaining_end_json, track_c, ambient_c, rain, session_elapsed_end_s,
                sample_file, sample_row_start, sample_row_end, timed_by_game, note, time_source)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18,
                ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28)",
            params![
                l.session_id,
                r.lap_number,
                r.lap_time_s,
                r.s1,
                r.s2,
                r.s3,
                r.valid,
                r.in_lap,
                r.out_lap,
                r.fuel_used_l,
                r.virtual_energy_used,
                r.top_speed_kmh,
                r.front_compound,
                r.rear_compound,
                json_arr(&r.avg_tyre_temp_c),
                json_arr(&r.avg_tyre_pressure_kpa),
                json_arr(&r.max_brake_temp_c),
                json_arr(&r.wear_remaining_end),
                r.track_c,
                r.ambient_c,
                r.rain,
                r.session_elapsed_end_s,
                l.sample_file,
                l.sample_row_start,
                l.sample_row_end,
                r.timed_by_game,
                r.note.map(lap_note_str),
                l.time_source,
            ],
        )?;
        Ok(())
    }

    pub fn laps_of(&self, session_id: &str) -> Result<Vec<LapRow>> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM laps WHERE session_id = ?1 ORDER BY lap_number")?;
        let rows = stmt
            .query_map([session_id], lap_from_row)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn get_lap(&self, session_id: &str, lap_number: i32) -> Result<Option<LapRow>> {
        Ok(self
            .conn
            .query_row(
                "SELECT * FROM laps WHERE session_id = ?1 AND lap_number = ?2",
                params![session_id, lap_number],
                lap_from_row,
            )
            .optional()?)
    }

    // --- events -------------------------------------------------------------

    pub fn insert_event(
        &self,
        session_id: &str,
        wall_ms: i64,
        session_elapsed_s: f64,
        lap_number: i32,
        event: &GameEvent,
    ) -> Result<i64> {
        let payload = serde_json::to_value(event)?;
        let kind = payload
            .get("kind")
            .and_then(|k| k.as_str())
            .unwrap_or("unknown")
            .to_string();
        self.conn.execute(
            "INSERT INTO events (session_id, wall_ms, session_elapsed_s, lap_number, kind, payload_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                session_id,
                wall_ms,
                session_elapsed_s,
                lap_number,
                kind,
                payload.to_string()
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn events_of(&self, session_id: &str) -> Result<Vec<EventRow>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, session_id, wall_ms, session_elapsed_s, lap_number, kind, payload_json FROM events WHERE session_id = ?1 ORDER BY id")?;
        let rows = stmt
            .query_map([session_id], |r| {
                let payload: String = r.get(6)?;
                Ok(EventRow {
                    id: r.get(0)?,
                    session_id: r.get(1)?,
                    wall_ms: r.get(2)?,
                    session_elapsed_s: r.get(3)?,
                    lap_number: r.get(4)?,
                    kind: r.get(5)?,
                    payload: serde_json::from_str(&payload).unwrap_or(serde_json::Value::Null),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    // --- setups -------------------------------------------------------------

    /// Store a setup captured during a session. `source` is `rest` (LMU REST
    /// `getPlayerGarageData`), `svm` (file) or `manual`. Filled by a later batch.
    pub fn insert_setup(
        &self,
        session_id: &str,
        captured_wall_ms: i64,
        source: &str,
        name: &str,
        content: &serde_json::Value,
    ) -> Result<i64> {
        if !matches!(source, "rest" | "svm" | "manual") {
            return Err(StoreError::invalid("setup source", source));
        }
        self.conn.execute(
            "INSERT INTO setups (session_id, captured_wall_ms, source, name, content_json)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                session_id,
                captured_wall_ms,
                source,
                name,
                content.to_string()
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn setups_of(&self, session_id: &str) -> Result<Vec<SetupRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, session_id, captured_wall_ms, source, name, content_json FROM setups WHERE session_id = ?1 ORDER BY id",
        )?;
        let rows = stmt
            .query_map([session_id], |r| {
                let content: String = r.get(5)?;
                Ok(SetupRow {
                    id: r.get(0)?,
                    session_id: r.get(1)?,
                    captured_wall_ms: r.get(2)?,
                    source: r.get(3)?,
                    name: r.get(4)?,
                    content: serde_json::from_str(&content).unwrap_or(serde_json::Value::Null),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    // --- sample files -------------------------------------------------------

    pub fn upsert_sample_file(&self, f: &SampleFileRow) -> Result<()> {
        self.conn.execute(
            "INSERT INTO sample_files (session_id, file_index, name, row_start, row_end, bytes, closed)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(session_id, file_index) DO UPDATE SET
                row_end = excluded.row_end, bytes = excluded.bytes, closed = excluded.closed",
            params![
                f.session_id,
                f.file_index,
                f.name,
                f.row_start,
                f.row_end,
                f.bytes,
                f.closed
            ],
        )?;
        Ok(())
    }

    pub fn sample_files_of(&self, session_id: &str) -> Result<Vec<SampleFileRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT session_id, file_index, name, row_start, row_end, bytes, closed FROM sample_files WHERE session_id = ?1 ORDER BY file_index",
        )?;
        let rows = stmt
            .query_map([session_id], |r| {
                Ok(SampleFileRow {
                    session_id: r.get(0)?,
                    file_index: r.get(1)?,
                    name: r.get(2)?,
                    row_start: r.get(3)?,
                    row_end: r.get(4)?,
                    bytes: r.get(5)?,
                    closed: r.get(6)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    // --- analyses (schema v3) ------------------------------------------------

    /// Cached analysis report of a session, as stored (`generated_ms`, JSON text).
    pub fn get_analysis(&self, session_id: &str) -> Result<Option<AnalysisRow>> {
        Ok(self
            .conn
            .query_row(
                "SELECT session_id, generated_ms, report_json FROM analyses WHERE session_id = ?1",
                [session_id],
                |r| {
                    Ok(AnalysisRow {
                        session_id: r.get(0)?,
                        generated_ms: r.get(1)?,
                        report_json: r.get(2)?,
                    })
                },
            )
            .optional()?)
    }

    /// Insert or replace the cached report of a session.
    pub fn put_analysis(
        &self,
        session_id: &str,
        generated_ms: i64,
        report_json: &str,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO analyses (session_id, generated_ms, report_json) VALUES (?1, ?2, ?3)
             ON CONFLICT(session_id) DO UPDATE SET
                generated_ms = excluded.generated_ms, report_json = excluded.report_json",
            params![session_id, generated_ms, report_json],
        )?;
        Ok(())
    }

    /// Drop the cached report of a session; `true` when one existed.
    pub fn delete_analysis(&self, session_id: &str) -> Result<bool> {
        let n = self
            .conn
            .execute("DELETE FROM analyses WHERE session_id = ?1", [session_id])?;
        Ok(n > 0)
    }

    // --- feedback (schema v4, interpretation v6) ------------------------------

    /// Store what the driver felt about a session; `text` is kept as given
    /// (the API trims and bounds it). Fails on an unknown session (foreign key).
    pub fn add_feedback(&self, session_id: &str, f: &NewFeedback) -> Result<Feedback> {
        let created_ms = now_ms();
        let plan = f.plan.as_ref().map(enum_str);
        self.conn.execute(
            "INSERT INTO feedback (session_id, created_ms, plan, text, uid)
             VALUES (?1, ?2, ?3, ?4, lower(hex(randomblob(16))))",
            params![session_id, created_ms, plan, f.text],
        )?;
        Ok(Feedback {
            id: self.conn.last_insert_rowid(),
            session_id: session_id.to_string(),
            created_ms,
            plan: f.plan,
            text: f.text.clone(),
            interpretation: None,
        })
    }

    /// Attach (or replace) the engineer's reading of a feedback; `true` when
    /// the feedback exists.
    pub fn set_feedback_interpretation(
        &self,
        feedback_id: i64,
        interpretation: &FeedbackInterpretation,
    ) -> Result<bool> {
        let json = serde_json::to_string(interpretation)?;
        let n = self.conn.execute(
            "UPDATE feedback SET interpretation_json = ?2 WHERE id = ?1",
            params![feedback_id, json],
        )?;
        Ok(n > 0)
    }

    /// One feedback by id (with its interpretation when stored).
    pub fn get_feedback(&self, feedback_id: i64) -> Result<Option<Feedback>> {
        Ok(self
            .conn
            .query_row(
                "SELECT id, session_id, created_ms, plan, text, interpretation_json
                 FROM feedback WHERE id = ?1",
                [feedback_id],
                feedback_from_row,
            )
            .optional()?)
    }

    /// Every feedback of a session, oldest first, with interpretations.
    pub fn list_feedback(&self, session_id: &str) -> Result<Vec<Feedback>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, session_id, created_ms, plan, text, interpretation_json FROM feedback
             WHERE session_id = ?1 ORDER BY created_ms, id",
        )?;
        let rows = stmt
            .query_map([session_id], feedback_from_row)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    // --- plan iterations (schema v6) ----------------------------------------

    /// Store an iteration; the row id is written back into `it.id`. The
    /// iteration number is taken from `it.iteration` (the caller numbers it
    /// with [`Db::last_iteration`]). Fails on an unknown session or feedback.
    pub fn add_iteration(&self, it: &mut PlanIteration) -> Result<i64> {
        if it.created_ms <= 0 {
            it.created_ms = now_ms();
        }
        let base_plan = enum_str(&it.base_plan);
        // The JSON is stored with the final id so that reads need no patching.
        let json_without_id = serde_json::to_string(it)?;
        // Built on the latest iteration of the same plan (none for the first one).
        let parent: Option<String> = self
            .conn
            .query_row(
                "SELECT uid FROM plan_iterations WHERE session_id = ?1 AND base_plan = ?2
                 ORDER BY iteration DESC, id DESC LIMIT 1",
                params![it.session_id, base_plan],
                |r| r.get(0),
            )
            .optional()?
            .flatten();
        self.conn.execute(
            "INSERT INTO plan_iterations (session_id, created_ms, iteration, base_plan, feedback_id,
                iteration_json, uid, parent_uid)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, lower(hex(randomblob(16))), ?7)",
            params![
                it.session_id,
                it.created_ms,
                it.iteration as i64,
                base_plan,
                it.feedback_id,
                json_without_id,
                parent
            ],
        )?;
        let id = self.conn.last_insert_rowid();
        it.id = id;
        let json = serde_json::to_string(it)?;
        self.conn.execute(
            "UPDATE plan_iterations SET iteration_json = ?2 WHERE id = ?1",
            params![id, json],
        )?;
        Ok(id)
    }

    /// Iterations of a session, oldest first (all plans, or one).
    pub fn list_iterations(
        &self,
        session_id: &str,
        base_plan: Option<SetupPhilosophy>,
    ) -> Result<Vec<PlanIteration>> {
        let plan = base_plan.as_ref().map(enum_str);
        let mut stmt = self.conn.prepare(
            "SELECT iteration_json, base_plan, parent_uid FROM plan_iterations
             WHERE session_id = ?1 AND (?2 IS NULL OR base_plan = ?2)
             ORDER BY created_ms, uid, id",
        )?;
        let rows = stmt
            .query_map(params![session_id, plan], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<String>>(2)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let mut out = Vec::with_capacity(rows.len());
        // (plan, parent) → iteration number of the first child seen.
        let mut first_child: std::collections::HashMap<(String, Option<String>), u32> =
            std::collections::HashMap::new();
        for (json, plan, parent) in rows {
            let mut it = serde_json::from_str::<PlanIteration>(&json)?;
            match first_child.get(&(plan.clone(), parent.clone())) {
                Some(n) => it.notes_fr.push(format!("{PARALLEL_NOTE_FR}{n}.")),
                None => {
                    first_child.insert((plan, parent), it.iteration);
                }
            }
            out.push(it);
        }
        Ok(out)
    }

    /// The most recent iteration of a session for one plan, if any.
    pub fn last_iteration(
        &self,
        session_id: &str,
        base_plan: SetupPhilosophy,
    ) -> Result<Option<PlanIteration>> {
        let plan = enum_str(&base_plan);
        let json: Option<String> = self
            .conn
            .query_row(
                "SELECT iteration_json FROM plan_iterations
                 WHERE session_id = ?1 AND base_plan = ?2
                 ORDER BY iteration DESC, id DESC LIMIT 1",
                params![session_id, plan],
                |r| r.get(0),
            )
            .optional()?;
        Ok(match json {
            Some(j) => Some(serde_json::from_str(&j)?),
            None => None,
        })
    }

    /// Sum of `sample_files.bytes` over every session.
    pub fn sample_bytes_total(&self) -> Result<i64> {
        Ok(self.conn.query_row(
            "SELECT COALESCE(SUM(bytes), 0) FROM sample_files",
            [],
            |r| r.get(0),
        )?)
    }
}

/// `id, session_id, created_ms, plan, text, interpretation_json` → [`Feedback`];
/// an unreadable interpretation is dropped (logged) rather than failing the row.
fn feedback_from_row(r: &Row<'_>) -> rusqlite::Result<Feedback> {
    let plan: Option<String> = r.get(3)?;
    let interpretation: Option<String> = r.get(5)?;
    let id: i64 = r.get(0)?;
    Ok(Feedback {
        id,
        session_id: r.get(1)?,
        created_ms: r.get(2)?,
        plan: plan.and_then(|p| serde_json::from_value(serde_json::Value::String(p)).ok()),
        text: r.get(4)?,
        interpretation: interpretation.and_then(|j| match serde_json::from_str(&j) {
            Ok(i) => Some(i),
            Err(e) => {
                tracing::warn!("feedback {id}: interprétation illisible ({e})");
                None
            }
        }),
    })
}

fn session_from_row(r: &Row<'_>) -> rusqlite::Result<SessionRow> {
    let game: String = r.get("game")?;
    let session_type: String = r.get("session_type")?;
    let vehicle_class: String = r.get("vehicle_class")?;
    let weather: String = r.get("weather_json")?;
    Ok(SessionRow {
        id: r.get("id")?,
        game: enum_parse(&game),
        track_name: r.get("track_name")?,
        track_length_m: r.get("track_length_m")?,
        session_type: enum_parse(&session_type),
        session_index: r.get("session_index")?,
        started_wall_ms: r.get("started_wall_ms")?,
        ended_wall_ms: r.get("ended_wall_ms")?,
        player_name: r.get("player_name")?,
        vehicle_name: r.get("vehicle_name")?,
        vehicle_model: r.get("vehicle_model")?,
        vehicle_class: enum_parse(&vehicle_class),
        class_name: r.get("class_name")?,
        server_name: r.get("server_name")?,
        is_multiplayer: r.get("is_multiplayer")?,
        game_version: r.get("game_version")?,
        source: r.get("source")?,
        record_rate_hz: r.get("record_rate_hz")?,
        sample_count: r.get("sample_count")?,
        lap_count: r.get("lap_count")?,
        best_lap_s: r.get("best_lap_s")?,
        weather: serde_json::from_str(&weather).unwrap_or_default(),
        notes: r.get("notes")?,
    })
}

fn lap_from_row(r: &Row<'_>) -> rusqlite::Result<LapRow> {
    let avg_t: String = r.get("avg_tyre_temp_c_json")?;
    let avg_p: String = r.get("avg_tyre_pressure_kpa_json")?;
    let max_b: String = r.get("max_brake_temp_c_json")?;
    let wear: String = r.get("wear_remaining_end_json")?;
    Ok(LapRow {
        session_id: r.get("session_id")?,
        lap: LapRecord {
            lap_number: r.get("lap_number")?,
            lap_time_s: r.get("lap_time_s")?,
            s1: r.get("s1")?,
            s2: r.get("s2")?,
            s3: r.get("s3")?,
            valid: r.get("valid")?,
            in_lap: r.get("in_lap")?,
            out_lap: r.get("out_lap")?,
            fuel_used_l: r.get("fuel_used_l")?,
            virtual_energy_used: r.get("virtual_energy_used")?,
            top_speed_kmh: r.get("top_speed_kmh")?,
            front_compound: r.get("front_compound")?,
            rear_compound: r.get("rear_compound")?,
            avg_tyre_temp_c: parse_arr(&avg_t),
            avg_tyre_pressure_kpa: parse_arr(&avg_p),
            max_brake_temp_c: parse_arr(&max_b),
            wear_remaining_end: parse_arr(&wear),
            track_c: r.get("track_c")?,
            ambient_c: r.get("ambient_c")?,
            rain: r.get("rain")?,
            session_elapsed_end_s: r.get("session_elapsed_end_s")?,
            timed_by_game: r.get("timed_by_game")?,
            note: r
                .get::<_, Option<String>>("note")?
                .as_deref()
                .and_then(lap_note_from_str),
        },
        sample_file: r.get("sample_file")?,
        sample_row_start: r.get("sample_row_start")?,
        sample_row_end: r.get("sample_row_end")?,
        time_source: r.get("time_source")?,
    })
}

/// Database spelling of a [`LapNote`] (same as its JSON form).
pub fn lap_note_str(n: LapNote) -> &'static str {
    match n {
        LapNote::OutLap => "out_lap",
        LapNote::InLap => "in_lap",
        LapNote::NotTimed => "not_timed",
        LapNote::Aborted => "aborted",
        LapNote::StartLap => "start_lap",
    }
}

pub fn lap_note_from_str(s: &str) -> Option<LapNote> {
    Some(match s {
        "out_lap" => LapNote::OutLap,
        "in_lap" => LapNote::InLap,
        "not_timed" => LapNote::NotTimed,
        "aborted" => LapNote::Aborted,
        "start_lap" => LapNote::StartLap,
        _ => return None,
    })
}

/// Modification time (Unix ms) of the newest file directly under `dir`.
fn newest_mtime_ms(dir: &Path) -> Option<i64> {
    let entries = std::fs::read_dir(dir).ok()?;
    entries
        .flatten()
        .filter_map(|e| e.metadata().ok()?.modified().ok())
        .filter_map(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .max()
}

/// Unix milliseconds now.
pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn sample_session(id: &str) -> SessionRow {
        SessionRow {
            id: id.into(),
            game: GameId::Sim,
            track_name: "Paul Ricard - 1A".into(),
            track_length_m: 5698.6,
            session_type: SessionType::Race,
            session_index: 10,
            started_wall_ms: 1_762_200_000_000,
            ended_wall_ms: None,
            player_name: "Pilote".into(),
            vehicle_name: "Mercedes-AMG GT3 #1".into(),
            vehicle_model: "Mercedes-AMG GT3".into(),
            vehicle_class: VehicleClass::Gt3,
            class_name: "GT3".into(),
            server_name: String::new(),
            is_multiplayer: false,
            game_version: None,
            source: "sim".into(),
            record_rate_hz: 50.0,
            sample_count: 0,
            lap_count: 0,
            best_lap_s: None,
            weather: Weather {
                ambient_c: 24.0,
                track_c: 31.5,
                ..Default::default()
            },
            notes: None,
        }
    }

    #[test]
    fn migrations_are_idempotent_on_disk() {
        let dir = std::env::temp_dir().join(format!("corde-store-db-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join(DB_FILE);
        {
            let db = Db::open(&path).unwrap();
            assert_eq!(db.schema_version().unwrap(), SCHEMA_VERSION);
            let mode: String = db
                .conn()
                .pragma_query_value(None, "journal_mode", |r| r.get(0))
                .unwrap();
            assert_eq!(mode.to_lowercase(), "wal");
            db.insert_session(&sample_session("s1")).unwrap();
        }
        {
            let db = Db::open(&path).unwrap();
            assert_eq!(db.schema_version().unwrap(), SCHEMA_VERSION);
            assert!(db.session_exists("s1").unwrap());
            let applied: i64 = db
                .conn()
                .query_row("SELECT COUNT(*) FROM schema_version", [], |r| r.get(0))
                .unwrap();
            assert_eq!(applied, SCHEMA_VERSION);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn crud_roundtrip_and_cascade() {
        let db = Db::open_in_memory().unwrap();
        let s = sample_session("20261103-213000-paul-ricard-1a-race");
        db.insert_session(&s).unwrap();
        let got = db.get_session(&s.id).unwrap().unwrap();
        assert_eq!(got, s);

        let lap = LapRow {
            session_id: s.id.clone(),
            lap: LapRecord {
                lap_number: 3,
                lap_time_s: 115.2,
                s1: Some(30.0),
                s2: Some(45.0),
                s3: Some(40.2),
                valid: true,
                fuel_used_l: 2.3,
                top_speed_kmh: 281.0,
                front_compound: "Medium".into(),
                rear_compound: "Medium".into(),
                avg_tyre_temp_c: [80.0, 81.0, 82.0, 83.0],
                ..Default::default()
            },
            sample_file: "samples-000.parquet".into(),
            sample_row_start: 100,
            sample_row_end: 5860,
            time_source: TIME_SOURCE_LIVE.to_string(),
        };
        db.insert_lap(&lap).unwrap();
        let laps = db.laps_of(&s.id).unwrap();
        assert_eq!(laps, vec![lap.clone()]);
        assert_eq!(db.get_lap(&s.id, 3).unwrap(), Some(lap));
        assert_eq!(db.get_lap(&s.id, 4).unwrap(), None);

        let ev_id = db
            .insert_event(
                &s.id,
                1_762_200_010_000,
                12.5,
                3,
                &GameEvent::PitEntry { lap_number: 3 },
            )
            .unwrap();
        assert!(ev_id > 0);
        let events = db.events_of(&s.id).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind, "pit_entry");
        assert_eq!(events[0].payload["lap_number"], 3);

        db.insert_setup(&s.id, 1, "manual", "Base", &serde_json::json!({"wing": 5}))
            .unwrap();
        assert!(db
            .insert_setup(&s.id, 1, "bogus", "x", &serde_json::json!({}))
            .is_err());
        assert_eq!(db.setups_of(&s.id).unwrap()[0].content["wing"], 5);

        db.upsert_sample_file(&SampleFileRow {
            session_id: s.id.clone(),
            file_index: 0,
            name: "samples-000.parquet".into(),
            row_start: 0,
            row_end: 6000,
            bytes: 123_456,
            closed: false,
        })
        .unwrap();
        db.upsert_sample_file(&SampleFileRow {
            session_id: s.id.clone(),
            file_index: 0,
            name: "samples-000.parquet".into(),
            row_start: 0,
            row_end: 7000,
            bytes: 200_000,
            closed: true,
        })
        .unwrap();
        let files = db.sample_files_of(&s.id).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].row_end, 7000);
        assert!(files[0].closed);
        assert_eq!(db.sample_bytes_total().unwrap(), 200_000);

        // Filters.
        let all = db.list_sessions(&SessionFilter::default()).unwrap();
        assert_eq!(all.len(), 1);
        let f = SessionFilter {
            track: Some("ricard".into()),
            session_type: Some(SessionType::Race),
            limit: Some(10),
        };
        assert_eq!(db.list_sessions(&f).unwrap().len(), 1);
        let f = SessionFilter {
            session_type: Some(SessionType::Practice),
            ..Default::default()
        };
        assert!(db.list_sessions(&f).unwrap().is_empty());

        // Recovery closes the open session using the last event time.
        let repaired = db.recover_open_sessions(None).unwrap();
        assert_eq!(repaired, vec![s.id.clone()]);
        let got = db.get_session(&s.id).unwrap().unwrap();
        assert_eq!(got.ended_wall_ms, Some(1_762_200_010_000));
        assert_eq!(got.lap_count, 1);
        assert_eq!(got.sample_count, 7000);
        assert!(got.notes.unwrap().starts_with("recovered"));
        assert!(db.recover_open_sessions(None).unwrap().is_empty());

        // Analyses cache (v3).
        assert!(db.get_analysis(&s.id).unwrap().is_none());
        db.put_analysis(&s.id, 1, r#"{"v":1}"#).unwrap();
        db.put_analysis(&s.id, 2, r#"{"v":2}"#).unwrap();
        let a = db.get_analysis(&s.id).unwrap().unwrap();
        assert_eq!(a.generated_ms, 2);
        assert_eq!(a.report_json, r#"{"v":2}"#);
        assert!(
            db.put_analysis("unknown-session", 1, "{}").is_err(),
            "foreign key"
        );

        // Feedback (v4).
        assert!(db.list_feedback(&s.id).unwrap().is_empty());
        let first = db
            .add_feedback(
                &s.id,
                &NewFeedback {
                    plan: Some(corde_protocol::SetupPhilosophy::Safe),
                    text: "l'avant ne tourne pas en entrée des virages lents".into(),
                },
            )
            .unwrap();
        assert!(first.id > 0 && first.created_ms > 0);
        assert_eq!(first.session_id, s.id);
        let second = db
            .add_feedback(
                &s.id,
                &NewFeedback {
                    plan: None,
                    text: "instable au freinage".into(),
                },
            )
            .unwrap();
        assert!(second.id > first.id);
        let list = db.list_feedback(&s.id).unwrap();
        assert_eq!(list, vec![first.clone(), second.clone()]);
        assert_eq!(list[0].plan, Some(corde_protocol::SetupPhilosophy::Safe));
        assert_eq!(list[1].plan, None);
        let stored_plan: Option<String> = db
            .conn()
            .query_row("SELECT plan FROM feedback WHERE id = ?1", [first.id], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(stored_plan.as_deref(), Some("safe"), "serde name on disk");
        assert!(
            db.add_feedback("unknown-session", &NewFeedback::default())
                .is_err(),
            "foreign key"
        );

        // Interpretation (v6): absent, then attached, then read back joined.
        assert!(first.interpretation.is_none());
        let interp = FeedbackInterpretation {
            feedback_id: first.id,
            symptoms: vec![corde_protocol::DetectedSymptom {
                symptom_id: "US_ENTRY_SLOW".into(),
                label_fr: "Sous-virage en entrée (virages lents)".into(),
                label_en: "Understeer on entry (slow corners)".into(),
                phase: Some(corde_protocol::CornerPhase::Entry),
                corner_speed: Some(corde_protocol::CornerSpeed::Slow),
                axle: Some(corde_protocol::Axle::Front),
                wet: false,
                intensity: None,
                negated: false,
                matched_text: "l'avant ne tourne pas".into(),
                confidence: 0.85,
            }],
            questions_fr: vec![],
            questions_en: vec![],
            unmatched_fragments: vec![],
            extractor: corde_protocol::ExtractorKind::Dictionary,
            confidence: 0.85,
            notes_fr: vec![],
        };
        assert!(db.set_feedback_interpretation(first.id, &interp).unwrap());
        assert!(!db.set_feedback_interpretation(9999, &interp).unwrap());
        let got = db.get_feedback(first.id).unwrap().unwrap();
        assert_eq!(got.interpretation, Some(interp.clone()));
        assert_eq!(got.text, first.text);
        let list = db.list_feedback(&s.id).unwrap();
        assert_eq!(list[0].interpretation, Some(interp.clone()));
        assert!(list[1].interpretation.is_none());
        assert!(db.get_feedback(9999).unwrap().is_none());

        // Plan iterations (v6): numbered per plan, listed oldest first, last per plan.
        assert!(db.list_iterations(&s.id, None).unwrap().is_empty());
        assert!(db
            .last_iteration(&s.id, corde_protocol::SetupPhilosophy::Safe)
            .unwrap()
            .is_none());
        let mut it1 = PlanIteration {
            id: 0,
            session_id: s.id.clone(),
            created_ms: 0,
            iteration: 1,
            base_plan: corde_protocol::SetupPhilosophy::Safe,
            feedback_id: Some(first.id),
            interpretation: Some(interp.clone()),
            data_findings_used: vec!["D-BAL-01/entry/slow".into()],
            changes: vec![],
            delta: vec![],
            agreements_fr: vec!["ok".into()],
            disagreements_fr: vec![],
            notes_fr: vec![],
            current_setup_known: false,
            dropped: vec![],
        };
        let id1 = db.add_iteration(&mut it1).unwrap();
        assert!(id1 > 0);
        assert_eq!(it1.id, id1);
        assert!(it1.created_ms > 0, "created_ms filled in");
        let mut it2 = PlanIteration {
            iteration: 2,
            feedback_id: Some(second.id),
            created_ms: it1.created_ms + 1,
            ..it1.clone()
        };
        db.add_iteration(&mut it2).unwrap();
        let mut other = PlanIteration {
            iteration: 1,
            base_plan: corde_protocol::SetupPhilosophy::Aggressive,
            feedback_id: None,
            created_ms: it1.created_ms + 2,
            ..it1.clone()
        };
        db.add_iteration(&mut other).unwrap();
        let all = db.list_iterations(&s.id, None).unwrap();
        assert_eq!(all.len(), 3);
        assert_eq!(all[0], it1, "stored JSON carries the final id");
        assert_eq!(all[1].iteration, 2);
        assert_eq!(
            all[2].base_plan,
            corde_protocol::SetupPhilosophy::Aggressive
        );
        let safe = db
            .list_iterations(&s.id, Some(corde_protocol::SetupPhilosophy::Safe))
            .unwrap();
        assert_eq!(safe.len(), 2);
        let last = db
            .last_iteration(&s.id, corde_protocol::SetupPhilosophy::Safe)
            .unwrap()
            .unwrap();
        assert_eq!(last.iteration, 2);
        assert_eq!(last.id, it2.id);
        let mut bad = PlanIteration {
            session_id: "unknown-session".into(),
            ..it1.clone()
        };
        assert!(db.add_iteration(&mut bad).is_err(), "foreign key (session)");
        let mut bad = PlanIteration {
            feedback_id: Some(9999),
            ..it1.clone()
        };
        assert!(
            db.add_iteration(&mut bad).is_err(),
            "foreign key (feedback)"
        );

        // Cascade.
        assert!(db.delete_session(&s.id).unwrap());
        assert!(!db.delete_session(&s.id).unwrap());
        assert!(db.laps_of(&s.id).unwrap().is_empty());
        assert!(db.events_of(&s.id).unwrap().is_empty());
        assert!(db.sample_files_of(&s.id).unwrap().is_empty());
        assert!(db.get_analysis(&s.id).unwrap().is_none());
        assert!(!db.delete_analysis(&s.id).unwrap());
        assert!(db.list_feedback(&s.id).unwrap().is_empty());
        assert!(db.list_iterations(&s.id, None).unwrap().is_empty());
        let n: i64 = db
            .conn()
            .query_row("SELECT COUNT(*) FROM plan_iterations", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0);
    }

    #[test]
    fn v6_upgrades_an_existing_v5_database() {
        // Replays the v1..v5 migrations (v5 = lap timing fix), writes v5 rows (a session with
        // `reconciled_result_id`, a lap with `timed_by_game` / `note` / `time_source`, a
        // feedback), then opens with the current code: the v6 column and table must appear
        // and nothing written before may be lost.
        let dir = std::env::temp_dir().join(format!("corde-store-v6-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(DB_FILE);
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(
                "CREATE TABLE schema_version (version INTEGER PRIMARY KEY, applied_wall_ms INTEGER NOT NULL);",
            )
            .unwrap();
            for (i, m) in MIGRATIONS.iter().take(5).enumerate() {
                conn.execute_batch(m).unwrap();
                conn.execute(
                    "INSERT INTO schema_version (version, applied_wall_ms) VALUES (?1, ?2)",
                    params![(i + 1) as i64, 1_i64],
                )
                .unwrap();
            }
            conn.execute(
                "INSERT INTO sessions (id, game, track_name, track_length_m, session_type, session_index,
                 started_wall_ms, ended_wall_ms, player_name, vehicle_name, vehicle_model, vehicle_class,
                 class_name, server_name, is_multiplayer, game_version, source, record_rate_hz,
                 sample_count, lap_count, best_lap_s, weather_json, notes, reconciled_result_id)
                 VALUES ('old', 'lmu', 'Spa', 0, 'race', 1, 1, NULL, '', '', '', 'gt3', '', '', 0, NULL, 'sim', 50, 0, 1, 137.5, '{}', NULL, 42)",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO laps (session_id, lap_number, lap_time_s, s1, s2, s3, valid, in_lap, out_lap,
                 fuel_used_l, virtual_energy_used, top_speed_kmh, front_compound, rear_compound,
                 avg_tyre_temp_c_json, avg_tyre_pressure_kpa_json, max_brake_temp_c_json,
                 wear_remaining_end_json, track_c, ambient_c, rain, session_elapsed_end_s, sample_file,
                 sample_row_start, sample_row_end, timed_by_game, note, time_source)
                 VALUES ('old', 0, 137.5, NULL, NULL, NULL, 1, 0, 0, 2.5, NULL, 250.0, '', '',
                 '[0,0,0,0]', '[0,0,0,0]', '[0,0,0,0]', '[1,1,1,1]', 30.0, 20.0, 0.0, 140.0,
                 'samples-000.parquet', 0, 100, 0, 'start_lap', 'results')",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO feedback (session_id, created_ms, plan, text) VALUES ('old', 5, 'safe', 'instable')",
                [],
            )
            .unwrap();
        }
        let db = Db::open(&path).unwrap();
        assert_eq!(db.schema_version().unwrap(), SCHEMA_VERSION);
        let list = db.list_feedback("old").unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].text, "instable");
        assert!(list[0].interpretation.is_none());
        assert!(db.list_iterations("old", None).unwrap().is_empty());
        let laps = db.laps_of("old").unwrap();
        assert_eq!(laps.len(), 1, "v5 laps kept");
        assert!(!laps[0].lap.timed_by_game);
        assert_eq!(laps[0].lap.note, Some(LapNote::StartLap));
        assert_eq!(laps[0].time_source, TIME_SOURCE_RESULTS);
        let reconciled: Option<i64> = db
            .conn()
            .query_row(
                "SELECT reconciled_result_id FROM sessions WHERE id = 'old'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(reconciled, Some(42), "v5 session column kept");
        drop(db);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn v7_gives_existing_feedback_and_iterations_stable_ids() {
        // A v6 database (0.6) with one feedback and two iterations of the same plan: after the
        // upgrade every row has a uid, and the second iteration names the first as its parent.
        let dir = std::env::temp_dir().join(format!("corde-store-v7-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(DB_FILE);
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(
                "CREATE TABLE schema_version (version INTEGER PRIMARY KEY, applied_wall_ms INTEGER NOT NULL);",
            )
            .unwrap();
            for (i, m) in MIGRATIONS.iter().take(6).enumerate() {
                conn.execute_batch(m).unwrap();
                conn.execute(
                    "INSERT INTO schema_version (version, applied_wall_ms) VALUES (?1, ?2)",
                    params![(i + 1) as i64, 1_i64],
                )
                .unwrap();
            }
            let mut s = sample_session("old");
            s.ended_wall_ms = Some(2);
            Db { conn }.insert_session(&s).unwrap();
            let conn = Connection::open(&path).unwrap();
            conn.execute(
                "INSERT INTO feedback (session_id, created_ms, plan, text) VALUES ('old', 5, 'safe', 'x')",
                [],
            )
            .unwrap();
            for n in 1..=2_i64 {
                conn.execute(
                    "INSERT INTO plan_iterations (session_id, created_ms, iteration, base_plan, iteration_json)
                     VALUES ('old', ?1, ?1, 'safe', '{}')",
                    [n],
                )
                .unwrap();
            }
        }
        let db = Db::open(&path).unwrap();
        assert_eq!(db.schema_version().unwrap(), SCHEMA_VERSION);
        let uid: Option<String> = db
            .conn()
            .query_row("SELECT uid FROM feedback", [], |r| r.get(0))
            .unwrap();
        assert!(uid.is_some_and(|u| crate::sync::valid_uid(&u)));
        let rows: Vec<(i64, String, Option<String>)> = db
            .conn()
            .prepare("SELECT iteration, uid, parent_uid FROM plan_iterations ORDER BY iteration")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .unwrap()
            .collect::<std::result::Result<_, _>>()
            .unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].2, None);
        assert_eq!(rows[1].2.as_deref(), Some(rows[0].1.as_str()));
        drop(db);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
