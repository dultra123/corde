//! Copying a PC's data to the home server (`corde-server --role hub`, ADR 0010).
//!
//! The PC stays the only source of telemetry. It sends the server, for every closed
//! session, a [`SessionBundle`] (the rows of `sessions`, `laps`, `events`, `setups`,
//! `sample_files` and the result file the laps were reconciled against) plus the Parquet
//! files, and every imported result as a [`ResultBundle`]. A bundle **replaces** what the
//! server holds for that session or file, so sending one twice changes nothing.
//!
//! Feedback and plan iterations are the only rows both sides create (the phone can type a
//! feedback on the server while the PC is off). They carry random identifiers (`uid`,
//! schema v7) and are **merged** both ways, never replaced nor deleted: each side adds the
//! rows it lacks ([`Db::merge_feedback`]), then the iterations of a plan are renumbered in
//! chronological order, so both databases end up with the same list. Two iterations built
//! on the same previous one (feedback typed on both sides between two syncs) are both kept;
//! the later one says so in its notes ([`crate::db::PARALLEL_NOTE_FR`]).
//!
//! The bookkeeping tables (`sync_received`, `sync_files`, `sync_tombstones`) are only
//! written on the server.

use crate::db::{enum_str, now_ms, Db, EventRow, LapRow, SampleFileRow, SessionRow, SetupRow};
use crate::error::{Result, StoreError};
use crate::results::{NewResult, ResultLapRow, ResultRow};
use corde_protocol::{FeedbackInterpretation, PlanIteration, SetupPhilosophy};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// `sync_received.kind` of a session bundle.
pub const KIND_SESSION: &str = "session";
/// `sync_received.kind` of a result bundle.
pub const KIND_RESULT: &str = "result";

/// Everything the server needs to show a recorded session, except its Parquet files and its
/// feedback (merged separately).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionBundle {
    pub session: SessionRow,
    pub laps: Vec<LapRow>,
    /// In the PC's order; the ids are the PC's (the server numbers its own).
    pub events: Vec<EventRow>,
    pub setups: Vec<SetupRow>,
    pub sample_files: Vec<SampleFileRow>,
    /// File name of the result the laps were reconciled against: `None` = not checked yet,
    /// `Some("")` = checked, no file matched.
    pub reconciled_result_file: Option<String>,
}

/// One imported result file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResultBundle {
    /// `id` and `imported_wall_ms` are the PC's (ignored by the server).
    pub row: ResultRow,
    pub driver_names: Vec<String>,
    pub laps: Vec<ResultLapRow>,
    /// The whole parsed file.
    pub file: serde_json::Value,
}

/// One feedback, by its stable identifier.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeedbackItem {
    pub uid: String,
    pub created_ms: i64,
    pub plan: Option<SetupPhilosophy>,
    pub text: String,
    pub interpretation: Option<FeedbackInterpretation>,
}

/// One plan iteration, by its stable identifier.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IterationItem {
    pub uid: String,
    /// The iteration it was built on (same session and plan), `None` for a first one.
    pub parent_uid: Option<String>,
    /// The feedback that triggered it.
    pub feedback_uid: Option<String>,
    /// As stored (ids are the sender's: rewritten on arrival).
    pub iteration: PlanIteration,
}

/// The feedback and iterations of one session.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct FeedbackSet {
    pub feedback: Vec<FeedbackItem>,
    pub iterations: Vec<IterationItem>,
}

/// What [`Db::merge_feedback`] added.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct MergeReport {
    pub feedback_added: usize,
    pub iterations_added: usize,
}

/// `uid`s of the feedback and iterations of one session (what a manifest lists).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct FeedbackUids {
    pub feedback: BTreeSet<String>,
    pub iterations: BTreeSet<String>,
}

/// `true` for an identifier made by SQLite's `lower(hex(randomblob(16)))`, or any short
/// lower-case hex string (what the API accepts from the other side).
pub fn valid_uid(uid: &str) -> bool {
    (8..=64).contains(&uid.len())
        && uid
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// `true` for the name of a sample file of a session (`samples-000.parquet`): letters,
/// digits, `-`, `_`, `.`, ending in `.parquet`, no leading dot, at most 64 bytes.
pub fn valid_sample_file_name(name: &str) -> bool {
    name.len() <= 64
        && name.ends_with(".parquet")
        && !name.starts_with('.')
        && !name.contains("..")
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}

impl Db {
    // --- sessions -------------------------------------------------------------

    /// Ids of the closed sessions, oldest first (what a PC may send).
    pub fn closed_session_ids(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn().prepare(
            "SELECT id FROM sessions WHERE ended_wall_ms IS NOT NULL ORDER BY started_wall_ms, id",
        )?;
        let ids = stmt
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(ids)
    }

    /// The bundle of one session, `None` when unknown.
    pub fn export_session(&self, id: &str) -> Result<Option<SessionBundle>> {
        let Some(session) = self.get_session(id)? else {
            return Ok(None);
        };
        let reconciled: Option<(Option<i64>, Option<String>)> = self
            .conn()
            .query_row(
                "SELECT s.reconciled_result_id, r.file_name FROM sessions s
                 LEFT JOIN results r ON r.id = s.reconciled_result_id WHERE s.id = ?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let reconciled_result_file = match reconciled {
            Some((Some(0), _)) => Some(String::new()),
            Some((Some(_), Some(name))) => Some(name),
            _ => None,
        };
        Ok(Some(SessionBundle {
            laps: self.laps_of(id)?,
            events: self.events_of(id)?,
            setups: self.setups_of(id)?,
            sample_files: self.sample_files_of(id)?,
            session,
            reconciled_result_file,
        }))
    }

    /// Replace what this database holds for the bundle's session by the bundle, in one
    /// transaction: session row upserted (its feedback and iterations are kept), laps,
    /// events, setups and sample files replaced, cached analysis dropped (it is computed
    /// again here, from the data received).
    pub fn import_session(&self, b: &SessionBundle) -> Result<()> {
        let s = &b.session;
        if !crate::valid_id(&s.id) {
            return Err(StoreError::invalid("session id", &s.id));
        }
        if s.ended_wall_ms.is_none() {
            return Err(StoreError::invalid("session", "still being recorded"));
        }
        for f in &b.sample_files {
            if !valid_sample_file_name(&f.name) {
                return Err(StoreError::invalid("sample file name", &f.name));
            }
        }
        for l in &b.laps {
            if !valid_sample_file_name(&l.sample_file) && !l.sample_file.is_empty() {
                return Err(StoreError::invalid("lap sample file", &l.sample_file));
            }
        }
        let reconciled: Option<i64> = match b.reconciled_result_file.as_deref() {
            None => None,
            Some("") => Some(0),
            Some(name) => self
                .conn()
                .query_row("SELECT id FROM results WHERE file_name = ?1", [name], |r| {
                    r.get(0)
                })
                .optional()?,
        };
        let tx = self.conn().unchecked_transaction()?;
        tx.execute(
            "INSERT INTO sessions (id, game, track_name, track_length_m, session_type, session_index,
                started_wall_ms, ended_wall_ms, player_name, vehicle_name, vehicle_model, vehicle_class,
                class_name, server_name, is_multiplayer, game_version, source, record_rate_hz,
                sample_count, lap_count, best_lap_s, weather_json, notes, reconciled_result_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18,
                ?19, ?20, ?21, ?22, ?23, ?24)
             ON CONFLICT(id) DO UPDATE SET
                game = excluded.game, track_name = excluded.track_name,
                track_length_m = excluded.track_length_m, session_type = excluded.session_type,
                session_index = excluded.session_index, started_wall_ms = excluded.started_wall_ms,
                ended_wall_ms = excluded.ended_wall_ms, player_name = excluded.player_name,
                vehicle_name = excluded.vehicle_name, vehicle_model = excluded.vehicle_model,
                vehicle_class = excluded.vehicle_class, class_name = excluded.class_name,
                server_name = excluded.server_name, is_multiplayer = excluded.is_multiplayer,
                game_version = excluded.game_version, source = excluded.source,
                record_rate_hz = excluded.record_rate_hz, sample_count = excluded.sample_count,
                lap_count = excluded.lap_count, best_lap_s = excluded.best_lap_s,
                weather_json = excluded.weather_json, notes = excluded.notes,
                reconciled_result_id = excluded.reconciled_result_id",
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
                reconciled,
            ],
        )?;
        for table in ["laps", "events", "setups", "sample_files", "analyses"] {
            tx.execute(
                &format!("DELETE FROM {table} WHERE session_id = ?1"),
                [&s.id],
            )?;
        }
        for l in &b.laps {
            let mut l = l.clone();
            l.session_id = s.id.clone();
            self.insert_lap(&l)?;
        }
        for e in &b.events {
            tx.execute(
                "INSERT INTO events (session_id, wall_ms, session_elapsed_s, lap_number, kind, payload_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    s.id,
                    e.wall_ms,
                    e.session_elapsed_s,
                    e.lap_number,
                    e.kind,
                    e.payload.to_string()
                ],
            )?;
        }
        for st in &b.setups {
            self.insert_setup(
                &s.id,
                st.captured_wall_ms,
                &st.source,
                &st.name,
                &st.content,
            )?;
        }
        for f in &b.sample_files {
            let mut f = f.clone();
            f.session_id = s.id.clone();
            self.upsert_sample_file(&f)?;
        }
        tx.commit()?;
        Ok(())
    }

    // --- results --------------------------------------------------------------

    /// File names of every imported result, oldest first.
    pub fn result_file_names(&self) -> Result<Vec<String>> {
        let mut stmt = self
            .conn()
            .prepare("SELECT file_name FROM results ORDER BY started_wall_ms, id")?;
        let names = stmt
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(names)
    }

    /// The bundle of one imported result, by file name.
    pub fn export_result(&self, file_name: &str) -> Result<Option<ResultBundle>> {
        let found: Option<(i64, String)> = self
            .conn()
            .query_row(
                "SELECT id, driver_names_json FROM results WHERE file_name = ?1",
                [file_name],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let Some((id, names)) = found else {
            return Ok(None);
        };
        let Some(detail) = self.get_result(id)? else {
            return Ok(None);
        };
        Ok(Some(ResultBundle {
            row: detail.row,
            driver_names: serde_json::from_str(&names).unwrap_or_default(),
            laps: self.result_laps_of(id)?,
            file: detail.file,
        }))
    }

    /// Insert or replace (by file name) an imported result received from a PC.
    pub fn import_result(&self, b: &ResultBundle) -> Result<i64> {
        let name = &b.row.file_name;
        if name.is_empty()
            || name.len() > 255
            || name.contains(['/', '\\', '\0'])
            || name.starts_with('.')
        {
            return Err(StoreError::invalid("result file name", name));
        }
        self.upsert_result(&NewResult {
            row: b.row.clone(),
            driver_names: b.driver_names.clone(),
            laps: b.laps.clone(),
            json: b.file.clone(),
        })
    }

    // --- feedback and iterations ----------------------------------------------

    /// `uid`s of the feedback and iterations of every session that has some.
    pub fn feedback_uids_by_session(&self) -> Result<BTreeMap<String, FeedbackUids>> {
        let mut out: BTreeMap<String, FeedbackUids> = BTreeMap::new();
        let mut stmt = self
            .conn()
            .prepare("SELECT session_id, uid FROM feedback WHERE uid IS NOT NULL")?;
        for row in stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))? {
            let (s, uid) = row?;
            out.entry(s).or_default().feedback.insert(uid);
        }
        let mut stmt = self
            .conn()
            .prepare("SELECT session_id, uid FROM plan_iterations WHERE uid IS NOT NULL")?;
        for row in stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))? {
            let (s, uid) = row?;
            out.entry(s).or_default().iterations.insert(uid);
        }
        Ok(out)
    }

    /// The feedback and iterations of one session, oldest first.
    pub fn export_feedback(&self, session_id: &str) -> Result<FeedbackSet> {
        let mut set = FeedbackSet::default();
        for f in self.list_feedback_with_uids(session_id)? {
            set.feedback.push(f);
        }
        let mut stmt = self.conn().prepare(
            "SELECT i.uid, i.parent_uid, f.uid, i.iteration_json FROM plan_iterations i
             LEFT JOIN feedback f ON f.id = i.feedback_id
             WHERE i.session_id = ?1 AND i.uid IS NOT NULL ORDER BY i.created_ms, i.uid, i.id",
        )?;
        let rows = stmt
            .query_map([session_id], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, String>(3)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        for (uid, parent_uid, feedback_uid, json) in rows {
            set.iterations.push(IterationItem {
                uid,
                parent_uid,
                feedback_uid,
                iteration: serde_json::from_str(&json)?,
            });
        }
        Ok(set)
    }

    fn list_feedback_with_uids(&self, session_id: &str) -> Result<Vec<FeedbackItem>> {
        let mut stmt = self.conn().prepare(
            "SELECT uid, created_ms, plan, text, interpretation_json FROM feedback
             WHERE session_id = ?1 AND uid IS NOT NULL ORDER BY created_ms, id",
        )?;
        let rows = stmt
            .query_map([session_id], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, Option<String>>(4)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows
            .into_iter()
            .map(|(uid, created_ms, plan, text, interp)| FeedbackItem {
                uid,
                created_ms,
                plan: plan.and_then(|p| serde_json::from_value(serde_json::Value::String(p)).ok()),
                text,
                interpretation: interp.and_then(|j| serde_json::from_str(&j).ok()),
            })
            .collect())
    }

    /// Add the feedback and iterations of `set` this database lacks (by `uid`) to a
    /// session, then renumber the iterations of the plans that changed. Never deletes nor
    /// modifies a row it already has. Unknown session → `NotFound`.
    pub fn merge_feedback(&self, session_id: &str, set: &FeedbackSet) -> Result<MergeReport> {
        if !self.session_exists(session_id)? {
            return Err(StoreError::not_found(format!("session {session_id}")));
        }
        for f in &set.feedback {
            if !valid_uid(&f.uid) {
                return Err(StoreError::invalid("feedback uid", &f.uid));
            }
            if f.text.chars().count() > 4000 {
                return Err(StoreError::invalid("feedback", "text too long"));
            }
        }
        for it in &set.iterations {
            let parent_ok = it.parent_uid.as_deref().is_none_or(valid_uid);
            let feedback_ok = it.feedback_uid.as_deref().is_none_or(valid_uid);
            if !valid_uid(&it.uid) || !parent_ok || !feedback_ok {
                return Err(StoreError::invalid("iteration uid", &it.uid));
            }
        }
        let mut report = MergeReport::default();
        let tx = self.conn().unchecked_transaction()?;
        for f in &set.feedback {
            let exists: bool = tx
                .query_row(
                    "SELECT 1 FROM feedback WHERE uid = ?1",
                    [&f.uid],
                    |_| Ok(()),
                )
                .optional()?
                .is_some();
            if exists {
                continue;
            }
            tx.execute(
                "INSERT INTO feedback (session_id, created_ms, plan, text, uid) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![session_id, f.created_ms, f.plan.as_ref().map(enum_str), f.text, f.uid],
            )?;
            let id = tx.last_insert_rowid();
            if let Some(mut interp) = f.interpretation.clone() {
                interp.feedback_id = id;
                tx.execute(
                    "UPDATE feedback SET interpretation_json = ?2 WHERE id = ?1",
                    params![id, serde_json::to_string(&interp)?],
                )?;
            }
            report.feedback_added += 1;
        }
        let mut plans: BTreeSet<String> = BTreeSet::new();
        for item in &set.iterations {
            let exists: bool = tx
                .query_row(
                    "SELECT 1 FROM plan_iterations WHERE uid = ?1",
                    [&item.uid],
                    |_| Ok(()),
                )
                .optional()?
                .is_some();
            if exists {
                continue;
            }
            let feedback_id: Option<i64> = match &item.feedback_uid {
                Some(uid) => tx
                    .query_row("SELECT id FROM feedback WHERE uid = ?1", [uid], |r| {
                        r.get(0)
                    })
                    .optional()?,
                None => None,
            };
            let mut it = item.iteration.clone();
            it.session_id = session_id.to_string();
            it.feedback_id = feedback_id;
            if let Some(interp) = it.interpretation.as_mut() {
                interp.feedback_id = feedback_id.unwrap_or(0);
            }
            if it.created_ms <= 0 {
                it.created_ms = now_ms();
            }
            let base_plan = enum_str(&it.base_plan);
            tx.execute(
                "INSERT INTO plan_iterations (session_id, created_ms, iteration, base_plan, feedback_id,
                    iteration_json, uid, parent_uid)
                 VALUES (?1, ?2, ?3, ?4, ?5, '{}', ?6, ?7)",
                params![
                    session_id,
                    it.created_ms,
                    it.iteration as i64,
                    base_plan,
                    feedback_id,
                    item.uid,
                    item.parent_uid
                ],
            )?;
            it.id = tx.last_insert_rowid();
            tx.execute(
                "UPDATE plan_iterations SET iteration_json = ?2 WHERE id = ?1",
                params![it.id, serde_json::to_string(&it)?],
            )?;
            plans.insert(base_plan);
            report.iterations_added += 1;
        }
        for plan in &plans {
            renumber(&tx, session_id, plan)?;
        }
        tx.commit()?;
        Ok(report)
    }

    // --- server bookkeeping -------------------------------------------------------

    /// What was received, by key: `{session id | result file name → fingerprint}`.
    pub fn sync_received(&self, kind: &str) -> Result<BTreeMap<String, String>> {
        let mut stmt = self
            .conn()
            .prepare("SELECT key, fingerprint FROM sync_received WHERE kind = ?1")?;
        let rows = stmt
            .query_map([kind], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })?
            .collect::<std::result::Result<BTreeMap<_, _>, _>>()?;
        Ok(rows)
    }

    pub fn set_sync_received(&self, kind: &str, key: &str, fingerprint: &str) -> Result<()> {
        self.conn().execute(
            "INSERT INTO sync_received (kind, key, fingerprint, received_ms) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(kind, key) DO UPDATE SET fingerprint = excluded.fingerprint,
                received_ms = excluded.received_ms",
            params![kind, key, fingerprint, now_ms()],
        )?;
        Ok(())
    }

    /// Sample files received, by session: `{session → {file name → SHA-256 hex}}`.
    pub fn sync_files(&self) -> Result<BTreeMap<String, BTreeMap<String, String>>> {
        let mut out: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
        let mut stmt = self
            .conn()
            .prepare("SELECT session_id, name, sha256 FROM sync_files")?;
        for row in stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })? {
            let (s, name, sha) = row?;
            out.entry(s).or_default().insert(name, sha);
        }
        Ok(out)
    }

    pub fn set_sync_file(
        &self,
        session_id: &str,
        name: &str,
        sha256: &str,
        bytes: i64,
    ) -> Result<()> {
        self.conn().execute(
            "INSERT INTO sync_files (session_id, name, sha256, bytes) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(session_id, name) DO UPDATE SET sha256 = excluded.sha256, bytes = excluded.bytes",
            params![session_id, name, sha256, bytes],
        )?;
        Ok(())
    }

    /// Sessions deleted on the server: a PC does not send them again.
    pub fn tombstones(&self) -> Result<Vec<String>> {
        let mut stmt = self
            .conn()
            .prepare("SELECT session_id FROM sync_tombstones ORDER BY session_id")?;
        let ids = stmt
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(ids)
    }

    /// Remember that a session was deleted on the server, and forget what was received
    /// for it.
    pub fn add_tombstone(&self, session_id: &str) -> Result<()> {
        self.conn().execute(
            "INSERT OR REPLACE INTO sync_tombstones (session_id, deleted_ms) VALUES (?1, ?2)",
            params![session_id, now_ms()],
        )?;
        self.conn()
            .execute("DELETE FROM sync_files WHERE session_id = ?1", [session_id])?;
        self.conn().execute(
            "DELETE FROM sync_received WHERE kind = ?1 AND key = ?2",
            params![KIND_SESSION, session_id],
        )?;
        Ok(())
    }

    pub fn is_tombstoned(&self, session_id: &str) -> Result<bool> {
        Ok(self
            .conn()
            .query_row(
                "SELECT 1 FROM sync_tombstones WHERE session_id = ?1",
                [session_id],
                |_| Ok(()),
            )
            .optional()?
            .is_some())
    }
}

/// Iterations of one (session, plan) numbered 1, 2, 3… in chronological order (ties broken
/// by `uid`, the same on both sides).
fn renumber(conn: &rusqlite::Connection, session_id: &str, base_plan: &str) -> Result<()> {
    let mut stmt = conn.prepare(
        "SELECT id, iteration, iteration_json FROM plan_iterations
         WHERE session_id = ?1 AND base_plan = ?2 ORDER BY created_ms, uid, id",
    )?;
    let rows = stmt
        .query_map(params![session_id, base_plan], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    drop(stmt);
    for (i, (id, current, json)) in rows.into_iter().enumerate() {
        let n = i as i64 + 1;
        if n == current {
            continue;
        }
        let mut it: PlanIteration = serde_json::from_str(&json)?;
        it.iteration = n as u32;
        conn.execute(
            "UPDATE plan_iterations SET iteration = ?2, iteration_json = ?3 WHERE id = ?1",
            params![id, n, serde_json::to_string(&it)?],
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::tests::sample_session;
    use corde_protocol::{LapRecord, NewFeedback};

    fn closed(id: &str) -> SessionRow {
        let mut s = sample_session(id);
        s.ended_wall_ms = Some(s.started_wall_ms + 60_000);
        s
    }

    fn iteration(
        session: &str,
        n: u32,
        feedback_id: Option<i64>,
        created_ms: i64,
    ) -> PlanIteration {
        PlanIteration {
            id: 0,
            session_id: session.into(),
            created_ms,
            iteration: n,
            base_plan: SetupPhilosophy::Safe,
            feedback_id,
            interpretation: None,
            data_findings_used: vec![],
            changes: vec![],
            delta: vec![],
            agreements_fr: vec![],
            disagreements_fr: vec![],
            notes_fr: vec![],
            current_setup_known: false,
            dropped: vec![],
        }
    }

    #[test]
    fn a_session_bundle_round_trips_and_replaces() {
        let pc = Db::open_in_memory().unwrap();
        let s = closed("20261103-213000-paul-ricard-1a-race");
        pc.insert_session(&s).unwrap();
        pc.insert_lap(&LapRow {
            session_id: s.id.clone(),
            lap: LapRecord {
                lap_number: 1,
                lap_time_s: 101.5,
                valid: true,
                ..Default::default()
            },
            sample_file: "samples-000.parquet".into(),
            sample_row_start: 0,
            sample_row_end: 100,
            time_source: "live".into(),
        })
        .unwrap();
        pc.upsert_sample_file(&SampleFileRow {
            session_id: s.id.clone(),
            file_index: 0,
            name: "samples-000.parquet".into(),
            row_start: 0,
            row_end: 100,
            bytes: 1234,
            closed: true,
        })
        .unwrap();
        let b = pc.export_session(&s.id).unwrap().unwrap();
        assert_eq!(b.reconciled_result_file, None);

        let hub = Db::open_in_memory().unwrap();
        hub.import_session(&b).unwrap();
        // A feedback typed on the server survives a second import.
        hub.add_feedback(
            &s.id,
            &NewFeedback {
                plan: None,
                text: "sous-vire".into(),
            },
        )
        .unwrap();
        hub.import_session(&b).unwrap();
        let again = hub.export_session(&s.id).unwrap().unwrap();
        assert_eq!(again.session, b.session);
        assert_eq!(again.laps, b.laps);
        assert_eq!(again.sample_files, b.sample_files);
        assert_eq!(hub.list_feedback(&s.id).unwrap().len(), 1);

        // An open session or a bad file name is refused.
        let mut open = b.clone();
        open.session.ended_wall_ms = None;
        assert!(hub.import_session(&open).is_err());
        let mut bad = b.clone();
        bad.sample_files[0].name = "../x.parquet".into();
        assert!(hub.import_session(&bad).is_err());
    }

    #[test]
    fn feedback_merges_both_ways_without_duplicates() {
        let pc = Db::open_in_memory().unwrap();
        let hub = Db::open_in_memory().unwrap();
        let s = closed("s1");
        pc.insert_session(&s).unwrap();
        hub.insert_session(&s).unwrap();

        // PC: one feedback and its iteration.
        let f = pc
            .add_feedback(
                "s1",
                &NewFeedback {
                    plan: None,
                    text: "sous-vire en entrée".into(),
                },
            )
            .unwrap();
        let mut it = iteration("s1", 1, Some(f.id), 1000);
        pc.add_iteration(&mut it).unwrap();
        // Server (phone): another one, in parallel on the same plan.
        let g = hub
            .add_feedback(
                "s1",
                &NewFeedback {
                    plan: None,
                    text: "survire en sortie".into(),
                },
            )
            .unwrap();
        let mut it2 = iteration("s1", 1, Some(g.id), 2000);
        hub.add_iteration(&mut it2).unwrap();

        let from_pc = pc.export_feedback("s1").unwrap();
        let from_hub = hub.export_feedback("s1").unwrap();
        let r = hub.merge_feedback("s1", &from_pc).unwrap();
        assert_eq!(
            r,
            MergeReport {
                feedback_added: 1,
                iterations_added: 1
            }
        );
        let r = pc.merge_feedback("s1", &from_hub).unwrap();
        assert_eq!(
            r,
            MergeReport {
                feedback_added: 1,
                iterations_added: 1
            }
        );
        // Twice: nothing more.
        assert_eq!(
            hub.merge_feedback("s1", &from_pc).unwrap(),
            MergeReport::default()
        );

        // Same list on both sides, numbered in time order, the parallel one flagged.
        let a = pc.list_iterations("s1", None).unwrap();
        let b = hub.list_iterations("s1", None).unwrap();
        assert_eq!(a.len(), 2);
        let nums = |v: &[PlanIteration]| {
            v.iter()
                .map(|i| (i.iteration, i.created_ms))
                .collect::<Vec<_>>()
        };
        assert_eq!(nums(&a), vec![(1, 1000), (2, 2000)]);
        assert_eq!(nums(&a), nums(&b));
        assert!(a[0].notes_fr.is_empty());
        assert!(
            a[1].notes_fr.iter().any(|n| n.contains("en parallèle")),
            "{:?}",
            a[1].notes_fr
        );
        assert!(b[1].notes_fr.iter().any(|n| n.contains("en parallèle")));
        // The iteration points at the local copy of its feedback.
        let fb_pc = pc.list_feedback("s1").unwrap();
        assert_eq!(fb_pc.len(), 2);
        let hub_fb_on_pc = fb_pc
            .iter()
            .find(|x| x.text == "survire en sortie")
            .unwrap();
        assert_eq!(a[1].feedback_id, Some(hub_fb_on_pc.id));
        assert_eq!(
            a[1].id,
            pc.last_iteration("s1", SetupPhilosophy::Safe)
                .unwrap()
                .unwrap()
                .id
        );

        // An iteration built after the merge has a single parent: no flag.
        let mut it3 = iteration("s1", 3, None, 3000);
        pc.add_iteration(&mut it3).unwrap();
        let a = pc.list_iterations("s1", None).unwrap();
        assert!(a[2].notes_fr.is_empty());

        // Bad identifiers are refused, unknown sessions too.
        let mut bad = from_pc.clone();
        bad.feedback[0].uid = "NOT-HEX".into();
        assert!(hub.merge_feedback("s1", &bad).is_err());
        assert!(hub.merge_feedback("nope", &from_pc).is_err());
    }

    #[test]
    fn hub_bookkeeping() {
        let hub = Db::open_in_memory().unwrap();
        hub.set_sync_received(KIND_SESSION, "s1", "abc").unwrap();
        hub.set_sync_received(KIND_SESSION, "s1", "def").unwrap();
        assert_eq!(hub.sync_received(KIND_SESSION).unwrap()["s1"], "def");
        hub.set_sync_file("s1", "samples-000.parquet", "f00", 10)
            .unwrap();
        assert_eq!(
            hub.sync_files().unwrap()["s1"]["samples-000.parquet"],
            "f00"
        );
        hub.add_tombstone("s1").unwrap();
        assert!(hub.is_tombstoned("s1").unwrap());
        assert!(hub.sync_files().unwrap().is_empty());
        assert!(hub.sync_received(KIND_SESSION).unwrap().is_empty());
        assert_eq!(hub.tombstones().unwrap(), vec!["s1".to_string()]);
    }

    #[test]
    fn names_and_uids() {
        assert!(valid_sample_file_name("samples-000.parquet"));
        for bad in [
            "../a.parquet",
            ".x.parquet",
            "a/b.parquet",
            "a.txt",
            "a\\b.parquet",
        ] {
            assert!(!valid_sample_file_name(bad), "{bad}");
        }
        assert!(valid_uid("0123456789abcdef0123456789abcdef"));
        assert!(!valid_uid("short") && !valid_uid("0123456789ABCDEF"));
    }
}
