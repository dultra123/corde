//! The game's result file is the last word on lap times.
//!
//! When a session ends the game writes a result file (`UserData\Log\Results`)
//! listing every lap of every driver with its official time — or no time when
//! the lap did not count. Once such a file is imported and matched to a
//! recorded session, the laps of that session are checked against it:
//!
//! * a lap we have gets the official time and sectors, or loses its time when
//!   the game did not time it;
//! * a lap we lack (the recording stopped before the line) is added, with the
//!   samples we do have;
//! * the session's best lap is recomputed and its cached analysis dropped.
//!
//! This also repairs sessions recorded by Corde ≤ v0.1.0-alpha.1, whose live
//! lap times could lag one lap behind and whose last race lap was cut.
//!
//! Lap numbers: a recorded lap `n` (0-based, the game's telemetry counter) is
//! the result file's lap `n + 1`.

use crate::channels;
use crate::db::{lap_note_str, Db, LapRow, SessionRow, TIME_SOURCE_LIVE, TIME_SOURCE_RESULTS};
use crate::error::Result;
use crate::results::{ResultFilter, ResultLapRow, ResultRow};
use crate::store::Store;
use corde_protocol::{LapNote, LapRecord, SessionType};
use rusqlite::params;
use serde::Serialize;

/// A result file and a recorded session that started this close, on the same
/// track, with the same session type, are the same session.
pub const LINK_WINDOW_MS: i64 = 10 * 60 * 1000;
/// Largest accepted gap between the end of a lap on both sides, seconds.
const ET_TOLERANCE_S: f64 = 2.5;
/// An untimed lap shorter than this share of the best lap was abandoned.
const ABORTED_RATIO: f64 = 0.6;

/// What reconciling one session did.
#[derive(Debug, Clone, PartialEq, Serialize, Default)]
pub struct ReconcileReport {
    pub session_id: String,
    pub result_id: i64,
    /// Name of the result file.
    pub file_name: String,
    /// Laps found on both sides at the same session time (the proof that the
    /// file and the recording are the same session).
    pub matched: usize,
    /// Laps whose time, sectors or status changed.
    pub updated: usize,
    /// Laps added from the result file.
    pub inserted: usize,
    /// Laps of the session the game did not time.
    pub untimed: usize,
    pub best_lap_s: Option<f32>,
    /// Why nothing was done, when nothing was.
    pub skipped: Option<String>,
}

/// The corrected lap list of a session (pure, no I/O).
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    /// Every lap of the session after reconciliation, by lap number.
    pub laps: Vec<LapRow>,
    pub report: ReconcileReport,
}

fn end_et(rlaps: &[ResultLapRow], i: usize) -> Option<f64> {
    // `elapsed_s` is the session time at the *start* of a lap.
    match rlaps.get(i + 1).and_then(|n| n.elapsed_s) {
        Some(next_start) => Some(next_start),
        None => match (rlaps[i].elapsed_s, rlaps[i].lap_time_s) {
            (Some(start), Some(t)) => Some(start + f64::from(t)),
            _ => None,
        },
    }
}

/// Builds the corrected lap list of `session` from the player's laps of a
/// result file (`rlaps`, 1-based lap numbers). `total_rows` is the number of
/// sample rows of the session.
pub fn plan(
    session: &SessionRow,
    laps: &[LapRow],
    result: &ResultRow,
    rlaps: &[ResultLapRow],
    total_rows: i64,
) -> Plan {
    let mut report = ReconcileReport {
        session_id: session.id.clone(),
        result_id: result.id,
        file_name: result.file_name.clone(),
        ..Default::default()
    };
    let mut rlaps: Vec<ResultLapRow> = rlaps.to_vec();
    rlaps.sort_by_key(|l| l.lap_number);
    let find = |n: i32| laps.iter().find(|l| l.lap.lap_number == n);

    // Are we looking at the same laps? Compare the end time of every lap
    // known on both sides.
    let (mut checked, mut matched) = (0usize, 0usize);
    for (i, r) in rlaps.iter().enumerate() {
        if let (Some(l), Some(e)) = (find(r.lap_number - 1), end_et(&rlaps, i)) {
            checked += 1;
            if (l.lap.session_elapsed_end_s - e).abs() <= ET_TOLERANCE_S {
                matched += 1;
            }
        }
    }
    report.matched = matched;
    let why = if rlaps.is_empty() {
        Some("aucun tour du pilote dans le fichier de résultats".to_string())
    } else if matched == 0 {
        Some("aucun tour commun vérifiable (même numéro, même instant)".to_string())
    } else if matched * 2 < checked {
        Some(format!(
            "les tours ne correspondent pas ({matched} sur {checked} alignés)"
        ))
    } else {
        None
    };
    if why.is_some() {
        report.skipped = why;
        return Plan {
            laps: laps.to_vec(),
            report,
        };
    }

    let best_timed = rlaps
        .iter()
        .filter(|r| !r.pit)
        .filter_map(|r| r.lap_time_s)
        .fold(None, |m: Option<f32>, t| Some(m.map_or(t, |m| m.min(t))));
    let is_race = session.session_type == SessionType::Race;

    let mut out: Vec<LapRow> = Vec::with_capacity(laps.len().max(rlaps.len()));
    for (i, r) in rlaps.iter().enumerate() {
        let n = r.lap_number - 1;
        let ours = find(n);
        let prev_pit = i > 0 && rlaps[i - 1].pit && rlaps[i - 1].lap_number == r.lap_number - 1;
        let end = end_et(&rlaps, i);
        let measured = match (r.elapsed_s, end) {
            (Some(a), Some(b)) if b > a => Some(b - a),
            _ => None,
        };
        let mut row = match ours {
            Some(l) => l.clone(),
            None => {
                // Only add the lap that follows one we recorded (the lap the
                // recording stopped in, or a hole between two laps).
                let prev = find(n - 1);
                let next = find(n + 1);
                if prev.is_none() {
                    continue;
                }
                let row_end = next.map(|l| l.sample_row_start).unwrap_or(total_rows);
                let row_start = prev
                    .map(|l| l.sample_row_end)
                    .unwrap_or(row_end)
                    .min(row_end);
                if row_start >= row_end {
                    // Nothing recorded for it: it belongs to another stint.
                    continue;
                }
                let model = prev.or(next).map(|l| &l.lap);
                LapRow {
                    session_id: session.id.clone(),
                    lap: LapRecord {
                        lap_number: n,
                        session_elapsed_end_s: end.unwrap_or(0.0),
                        front_compound: r
                            .front_compound
                            .clone()
                            .or_else(|| model.map(|m| m.front_compound.clone()))
                            .unwrap_or_default(),
                        rear_compound: r
                            .rear_compound
                            .clone()
                            .or_else(|| model.map(|m| m.rear_compound.clone()))
                            .unwrap_or_default(),
                        track_c: model.map(|m| m.track_c).unwrap_or(0.0),
                        ambient_c: model.map(|m| m.ambient_c).unwrap_or(0.0),
                        rain: model.map(|m| m.rain).unwrap_or(0.0),
                        ..Default::default()
                    },
                    sample_file: prev
                        .or(next)
                        .map(|l| l.sample_file.clone())
                        .unwrap_or_default(),
                    sample_row_start: row_start,
                    sample_row_end: row_end,
                    time_source: TIME_SOURCE_LIVE.to_string(),
                }
            }
        };
        let before = row.clone();
        let lap = &mut row.lap;

        let timed = r.lap_time_s.is_some();
        match r.lap_time_s {
            Some(t) => {
                lap.lap_time_s = t;
                lap.s1 = r.s1;
                lap.s2 = r.s2;
                lap.s3 = r.s3;
            }
            None => {
                if let Some(m) = measured {
                    lap.lap_time_s = m as f32;
                }
                lap.s1 = r.s1;
                lap.s2 = r.s2;
                lap.s3 = None;
            }
        }
        lap.timed_by_game = timed;
        // The first lap of a race starts on the grid: alpha 1 saw it start in
        // the garage and called it an out-lap.
        let race_start = is_race && r.lap_number == 1 && timed;
        lap.out_lap = (lap.out_lap && !race_start) || prev_pit;
        lap.in_lap = (r.pit || lap.in_lap) && !lap.out_lap;
        let aborted = lap.note == Some(LapNote::Aborted)
            || (!timed
                && !r.pit
                && match (
                    best_timed,
                    measured.or(ours.map(|l| f64::from(l.lap.lap_time_s))),
                ) {
                    (Some(b), Some(m)) => m < ABORTED_RATIO * f64::from(b),
                    _ => false,
                });
        lap.valid = timed && !lap.in_lap && !lap.out_lap && !aborted && lap.lap_time_s > 0.0;
        lap.note = if aborted {
            Some(LapNote::Aborted)
        } else if lap.out_lap {
            Some(LapNote::OutLap)
        } else if lap.in_lap {
            Some(LapNote::InLap)
        } else if !timed {
            Some(LapNote::NotTimed)
        } else if race_start {
            Some(LapNote::StartLap)
        } else {
            None
        };
        if let Some(e) = end {
            if ours.is_none() || lap.session_elapsed_end_s <= 0.0 {
                lap.session_elapsed_end_s = e;
            }
        }
        row.time_source = TIME_SOURCE_RESULTS.to_string();

        if ours.is_none() {
            report.inserted += 1;
        } else if row.lap != before.lap {
            report.updated += 1;
        }
        out.push(row);
    }
    // Laps the result file does not know (a slow-down lap after the flag…)
    // are kept as they are.
    for l in laps {
        if !out.iter().any(|o| o.lap.lap_number == l.lap.lap_number) {
            out.push(l.clone());
        }
    }
    out.sort_by_key(|l| l.lap.lap_number);
    // A start lap recorded by alpha 1 began in the garage: its fuel figure
    // includes whatever happened before the green flag. Unknown is better.
    let mut fuel: Vec<f32> = out
        .iter()
        .filter(|l| l.lap.valid && l.lap.note.is_none() && l.lap.fuel_used_l > 0.0)
        .map(|l| l.lap.fuel_used_l)
        .collect();
    fuel.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let typical = fuel.get(fuel.len() / 2).copied();
    for l in out.iter_mut() {
        if l.lap.note == Some(LapNote::StartLap)
            && typical.is_none_or(|t| l.lap.fuel_used_l > 2.5 * t)
            && laps
                .iter()
                .any(|o| o.lap.lap_number == l.lap.lap_number && o.lap.out_lap)
        {
            l.lap.fuel_used_l = 0.0;
        }
    }
    report.untimed = out.iter().filter(|l| !l.lap.timed_by_game).count();
    report.best_lap_s = best_valid(&out);
    Plan { laps: out, report }
}

fn best_valid(laps: &[LapRow]) -> Option<f32> {
    laps.iter()
        .filter(|l| l.lap.valid && l.lap.lap_time_s > 0.0)
        .map(|l| l.lap.lap_time_s)
        .fold(None, |m: Option<f32>, t| Some(m.map_or(t, |m| m.min(t))))
}

fn track_matches(a: &str, b: &str) -> bool {
    let (a, b) = (a.trim().to_lowercase(), b.trim().to_lowercase());
    !a.is_empty() && !b.is_empty() && (a == b || a.contains(&b) || b.contains(&a))
}

/// `reconciled_result_id` of a session no result file matched.
const NO_RESULT: i64 = 0;
/// A closed session younger than this may still get its result file.
const SETTLE_MS: i64 = 10 * 60 * 1000;

impl Db {
    /// Result files that may describe a recorded session: same session type,
    /// same track, same car, written while or shortly after it was recorded.
    pub fn results_for_session(&self, s: &SessionRow) -> Result<Vec<ResultRow>> {
        let end = s.ended_wall_ms.unwrap_or(s.started_wall_ms);
        let all = self.list_results(&ResultFilter {
            session_kind: Some(s.session_type),
            limit: Some(100_000),
            ..Default::default()
        })?;
        Ok(all
            .into_iter()
            .filter(|r| {
                let written = r.file_mtime_ms.max(r.started_wall_ms);
                r.started_wall_ms - LINK_WINDOW_MS <= end
                    && written + LINK_WINDOW_MS >= s.started_wall_ms
            })
            .filter(|r| {
                track_matches(&s.track_name, &r.track_course)
                    || track_matches(&s.track_name, &r.track_venue)
            })
            .filter(|r| {
                let (a, b) = (s.vehicle_name.trim(), r.player_vehicle_name.trim());
                a.is_empty() || b.is_empty() || a.eq_ignore_ascii_case(b)
            })
            .collect())
    }

    /// Result file the laps of a session were last checked against
    /// (`Some(0)`: checked, no file matched).
    pub fn reconciled_result_of(&self, session_id: &str) -> Result<Option<i64>> {
        Ok(self.conn().query_row(
            "SELECT reconciled_result_id FROM sessions WHERE id = ?1",
            [session_id],
            |r| r.get::<_, Option<i64>>(0),
        )?)
    }

    fn set_reconciled(&self, session_id: &str, result_id: i64) -> Result<()> {
        self.conn().execute(
            "UPDATE sessions SET reconciled_result_id = ?2 WHERE id = ?1",
            params![session_id, result_id],
        )?;
        Ok(())
    }

    /// Closed sessions to check: never checked, plus (with `recheck`) those
    /// no file matched so far.
    fn sessions_to_reconcile(&self, recheck: bool) -> Result<Vec<String>> {
        let mut stmt = self.conn().prepare(
            "SELECT id FROM sessions
             WHERE ended_wall_ms IS NOT NULL
               AND (reconciled_result_id IS NULL OR (?1 AND reconciled_result_id = ?2))
             ORDER BY started_wall_ms",
        )?;
        let ids = stmt
            .query_map(params![recheck, NO_RESULT], |r| r.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(ids)
    }

    /// Replaces the laps of a session and its counters in one transaction;
    /// drops the cached analysis.
    fn apply_plan(&self, plan: &Plan) -> Result<()> {
        let id = &plan.report.session_id;
        self.conn().execute_batch("BEGIN IMMEDIATE")?;
        let run = || -> Result<()> {
            self.conn()
                .execute("DELETE FROM laps WHERE session_id = ?1", [id])?;
            for l in &plan.laps {
                self.insert_lap(l)?;
            }
            self.conn().execute(
                "UPDATE sessions SET lap_count = ?2, best_lap_s = ?3, reconciled_result_id = ?4
                 WHERE id = ?1",
                params![
                    id,
                    plan.laps.len() as i64,
                    plan.report.best_lap_s,
                    plan.report.result_id
                ],
            )?;
            self.delete_analysis(id)?;
            Ok(())
        };
        match run() {
            Ok(()) => {
                self.conn().execute_batch("COMMIT")?;
                Ok(())
            }
            Err(e) => {
                let _ = self.conn().execute_batch("ROLLBACK");
                Err(e)
            }
        }
    }
}

/// Averages of one lap computed from its samples (for a lap added from the
/// result file).
#[derive(Debug, Clone, Default, PartialEq)]
struct SampleStats {
    top_speed_kmh: f32,
    fuel_used_l: f32,
    avg_tyre_temp_c: [f32; 4],
    avg_tyre_pressure_kpa: [f32; 4],
    max_brake_temp_c: [f32; 4],
    wear_remaining_end: [f32; 4],
}

impl Store {
    fn sample_stats(&self, session_id: &str, row_start: i64, row_end: i64) -> Option<SampleStats> {
        if row_end <= row_start {
            return None;
        }
        const W: [&str; 4] = ["_fl", "_fr", "_rl", "_rr"];
        let mut names: Vec<String> = vec!["speed_kmh".into(), "fuel_l".into()];
        for base in [
            "temp_l_c",
            "temp_c_c",
            "temp_r_c",
            "pressure_kpa",
            "brake_temp_c",
            "wear_remaining",
        ] {
            for w in W {
                names.push(format!("{base}{w}"));
            }
        }
        let chans: Vec<&channels::Channel> = names
            .iter()
            .map(|n| channels::channel(n))
            .collect::<Option<_>>()?;
        let files = self.db().sample_files_of(session_id).ok()?;
        let dir = self.session_dir(session_id).ok()?;
        let mut cols: Vec<Vec<Option<f64>>> = chans.iter().map(|_| Vec::new()).collect();
        crate::query::read_range(
            &dir,
            &files,
            &chans,
            row_start.max(0) as u64,
            row_end.max(0) as u64,
            &mut cols,
        )
        .ok()?;
        if cols[0].is_empty() {
            return None;
        }
        let vals = |i: usize| cols[i].iter().flatten().copied().filter(|v| v.is_finite());
        let mean = |i: usize| {
            let (s, n) = vals(i).fold((0.0, 0usize), |(s, n), v| (s + v, n + 1));
            if n > 0 {
                (s / n as f64) as f32
            } else {
                0.0
            }
        };
        let max = |i: usize| vals(i).fold(0.0f64, f64::max) as f32;
        let first = |i: usize| vals(i).next();
        let last = |i: usize| vals(i).next_back();
        let mut st = SampleStats {
            top_speed_kmh: max(0),
            fuel_used_l: match (first(1), last(1)) {
                (Some(a), Some(b)) => (a - b).max(0.0) as f32,
                _ => 0.0,
            },
            ..Default::default()
        };
        for w in 0..4 {
            st.avg_tyre_temp_c[w] = (mean(2 + w) + mean(6 + w) + mean(10 + w)) / 3.0;
            st.avg_tyre_pressure_kpa[w] = mean(14 + w);
            st.max_brake_temp_c[w] = max(18 + w);
            st.wear_remaining_end[w] = last(22 + w).unwrap_or(0.0) as f32;
        }
        Some(st)
    }

    /// Checks the laps of a closed recorded session against the result file
    /// that describes it — the one sharing the most laps with it when several
    /// could. `Ok(None)` when the session is unknown, still being recorded, or
    /// no file could describe it; a report with `skipped` set when files were
    /// found but none shares a verifiable lap with the recording.
    pub fn reconcile_session(&self, session_id: &str) -> Result<Option<ReconcileReport>> {
        let Some(session) = self.db().get_session(session_id)? else {
            return Ok(None);
        };
        if session.ended_wall_ms.is_none() {
            return Ok(None);
        }
        let laps = self.db().laps_of(&session.id)?;
        let candidates = self.db().results_for_session(&session)?;
        let mut best: Option<Plan> = None;
        let mut rejected: Option<ReconcileReport> = None;
        // Most recent first: at equal evidence the latest file (the most
        // complete one) wins.
        for r in &candidates {
            let rlaps: Vec<ResultLapRow> = self
                .db()
                .result_laps_of(r.id)?
                .into_iter()
                .filter(|l| l.is_player)
                .collect();
            let p = plan(&session, &laps, r, &rlaps, session.sample_count);
            if p.report.skipped.is_some() {
                rejected.get_or_insert(p.report);
            } else if best
                .as_ref()
                .is_none_or(|b| p.report.matched > b.report.matched)
            {
                best = Some(p);
            }
        }
        let Some(mut plan) = best else {
            return Ok(rejected);
        };
        // Laps added from the file: fill their statistics from the samples.
        for row in plan.laps.iter_mut() {
            let added = !laps.iter().any(|l| l.lap.lap_number == row.lap.lap_number);
            if !added {
                continue;
            }
            if let Some(st) =
                self.sample_stats(&session.id, row.sample_row_start, row.sample_row_end)
            {
                row.lap.top_speed_kmh = st.top_speed_kmh;
                row.lap.fuel_used_l = st.fuel_used_l;
                row.lap.avg_tyre_temp_c = st.avg_tyre_temp_c;
                row.lap.avg_tyre_pressure_kpa = st.avg_tyre_pressure_kpa;
                row.lap.max_brake_temp_c = st.max_brake_temp_c;
                row.lap.wear_remaining_end = st.wear_remaining_end;
            }
        }
        self.db().apply_plan(&plan)?;
        let r = &plan.report;
        tracing::info!(
            "tours : session {} recalée sur {} — {} tour(s) en commun, {} corrigé(s), {} ajouté(s), {} non chronométré(s), meilleur {:?}",
            r.session_id,
            r.file_name,
            r.matched,
            r.updated,
            r.inserted,
            r.untimed,
            r.best_lap_s
        );
        Ok(Some(plan.report))
    }

    /// Reconciles every closed session not checked yet. With `recheck`, also
    /// those no result file matched so far (call it when new files came in).
    /// Returns what was done for the sessions a file matched.
    pub fn reconcile_pending(&self, recheck: bool) -> Result<Vec<ReconcileReport>> {
        let ids = self.db().sessions_to_reconcile(recheck)?;
        let now = crate::db::now_ms();
        let mut out = Vec::new();
        for id in ids {
            match self.reconcile_session(&id) {
                Ok(Some(rep)) if rep.skipped.is_none() => out.push(rep),
                Ok(other) => {
                    if let Some(rep) = &other {
                        tracing::debug!(
                            "tours : session {id} — {} : {}",
                            rep.file_name,
                            rep.skipped.as_deref().unwrap_or_default()
                        );
                    }
                    // No file for it. Once the session is old enough for its
                    // file to exist, stop asking until new files come in.
                    let ended = self
                        .db()
                        .get_session(&id)?
                        .and_then(|s| s.ended_wall_ms)
                        .unwrap_or(now);
                    if now - ended > SETTLE_MS {
                        self.db().set_reconciled(&id, NO_RESULT)?;
                    }
                }
                Err(e) => tracing::error!("tours : session {id} — recalage impossible : {e}"),
            }
        }
        Ok(out)
    }

    /// Forgets which sessions were checked, so that the next
    /// [`Store::reconcile_pending`] goes through all of them again.
    pub fn reconcile_reset(&self) -> Result<()> {
        self.db()
            .conn()
            .execute("UPDATE sessions SET reconciled_result_id = NULL", [])?;
        Ok(())
    }
}

/// Text form of a note for logs and tools.
pub fn note_label(n: Option<LapNote>) -> &'static str {
    n.map(lap_note_str).unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(kind: SessionType) -> SessionRow {
        SessionRow {
            id: "20261109-210405-test-race".into(),
            game: corde_protocol::GameId::Lmu,
            track_name: "Test Ring".into(),
            track_length_m: 4000.0,
            session_type: kind,
            session_index: 10,
            started_wall_ms: 1_000_000,
            ended_wall_ms: Some(3_000_000),
            player_name: "Driver".into(),
            vehicle_name: "Car #1".into(),
            vehicle_model: "Car".into(),
            vehicle_class: corde_protocol::VehicleClass::Gt3,
            class_name: "GT3".into(),
            server_name: String::new(),
            is_multiplayer: true,
            game_version: None,
            source: "lmu-shm".into(),
            record_rate_hz: 50.0,
            sample_count: 10_000,
            lap_count: 0,
            best_lap_s: None,
            weather: Default::default(),
            notes: None,
        }
    }

    fn ours(n: i32, time: f32, end_et: f64, rows: (i64, i64), valid: bool) -> LapRow {
        LapRow {
            session_id: "20261109-210405-test-race".into(),
            lap: LapRecord {
                lap_number: n,
                lap_time_s: time,
                valid,
                session_elapsed_end_s: end_et,
                front_compound: "Medium".into(),
                rear_compound: "Medium".into(),
                track_c: 30.0,
                ..Default::default()
            },
            sample_file: "samples-000.parquet".into(),
            sample_row_start: rows.0,
            sample_row_end: rows.1,
            time_source: TIME_SOURCE_LIVE.to_string(),
        }
    }

    fn theirs(num: i32, start_et: Option<f64>, time: Option<f32>, pit: bool) -> ResultLapRow {
        ResultLapRow {
            result_id: 7,
            driver_name: "Driver".into(),
            is_player: true,
            lap_number: num,
            elapsed_s: start_et,
            lap_time_s: time,
            s1: time.map(|t| t * 0.2),
            s2: time.map(|t| t * 0.5),
            s3: time.map(|t| t * 0.3),
            pit,
            ..Default::default()
        }
    }

    fn result() -> ResultRow {
        ResultRow {
            id: 7,
            file_name: "2026_11_09_21_37_34-07R1.xml".into(),
            session_kind: SessionType::Race,
            ..Default::default()
        }
    }

    /// The shape of a real race recorded by alpha 1: start lap measured from
    /// the garage, times one lap late, a lap the game did not time recorded as
    /// valid, and the last lap missing.
    #[test]
    fn repairs_a_race_recorded_by_alpha_1() {
        let s = session(SessionType::Race);
        let laps = vec![
            // start lap: 249.8 s "out-lap" for us, 92.0 s for the game
            {
                let mut l = ours(0, 249.8, 251.2, (0, 2300), false);
                l.lap.out_lap = true;
                l.lap.fuel_used_l = 41.0;
                l
            },
            ours(1, 84.695, 335.8, (2300, 4800), true),
            ours(2, 84.695, 419.0, (4800, 7300), true), // stale: really 83.170
            ours(3, 83.170, 502.0, (7300, 9000), true), // stale: really 83.075 (best)
            ours(4, 83.075, 584.8, (9000, 9500), true), // not timed by the game
        ];
        let rlaps = vec![
            theirs(1, Some(159.2), Some(92.013), false),
            theirs(2, Some(251.22), Some(84.695), false),
            theirs(3, Some(335.92), Some(83.170), false),
            theirs(4, Some(419.10), Some(83.075), false),
            theirs(5, Some(502.16), None, false),
            theirs(6, Some(584.84), Some(84.027), false), // the lap we never saw end
        ];
        let p = plan(&s, &laps, &result(), &rlaps, 10_000);
        assert_eq!(p.report.skipped, None);
        assert_eq!(p.laps.len(), 6);
        let t: Vec<f32> = p.laps.iter().map(|l| l.lap.lap_time_s).collect();
        assert!((t[0] - 92.013).abs() < 1e-3);
        assert!((t[2] - 83.170).abs() < 1e-3);
        assert!((t[3] - 83.075).abs() < 1e-3);
        assert!(
            (t[4] - 82.68).abs() < 0.01,
            "measured between crossings: {}",
            t[4]
        );
        assert!((t[5] - 84.027).abs() < 1e-3);
        // Start lap: timed by the game, flagged as such, no longer an out-lap.
        assert!(p.laps[0].lap.timed_by_game && p.laps[0].lap.valid);
        assert!(!p.laps[0].lap.out_lap);
        assert_eq!(p.laps[0].lap.note, Some(LapNote::StartLap));
        assert_eq!(
            p.laps[0].lap.fuel_used_l, 0.0,
            "pre-race fuel figure dropped"
        );
        // The untimed lap is not valid and cannot be the best lap.
        assert!(!p.laps[4].lap.valid && !p.laps[4].lap.timed_by_game);
        assert_eq!(p.laps[4].lap.note, Some(LapNote::NotTimed));
        assert_eq!(p.laps[4].lap.s3, None);
        assert_eq!(p.report.best_lap_s, Some(83.075));
        // The missing last lap: added with the rows left after the previous lap.
        let last = &p.laps[5];
        assert_eq!(last.lap.lap_number, 5);
        assert_eq!((last.sample_row_start, last.sample_row_end), (9500, 10_000));
        assert!(last.lap.valid);
        assert_eq!(last.lap.front_compound, "Medium");
        assert!((last.lap.session_elapsed_end_s - 668.867).abs() < 0.01);
        assert_eq!(p.report.inserted, 1);
        assert_eq!(p.report.untimed, 1);
        assert!(p.report.updated >= 4, "{:?}", p.report);
        assert!(p.laps.iter().all(|l| l.time_source == TIME_SOURCE_RESULTS));
        // Idempotent.
        let again = plan(&s, &p.laps, &result(), &rlaps, 10_000);
        assert_eq!(again.laps, p.laps);
        assert_eq!(again.report.updated + again.report.inserted, 0);
    }

    #[test]
    fn start_lap_of_a_race_is_flagged_when_we_saw_it_from_the_grid() {
        let s = session(SessionType::Race);
        let laps = vec![
            ours(0, 92.0, 251.2, (0, 2300), true),
            ours(1, 84.7, 335.9, (2300, 4800), true),
        ];
        let rlaps = vec![
            theirs(1, Some(159.2), Some(92.013), false),
            theirs(2, Some(251.22), Some(84.695), false),
        ];
        let p = plan(&s, &laps, &result(), &rlaps, 4800);
        assert_eq!(p.laps[0].lap.note, Some(LapNote::StartLap));
        assert!(p.laps[0].lap.valid);
        assert_eq!(p.laps[1].lap.note, None);
    }

    #[test]
    fn practice_with_out_lap_cut_laps_and_a_return_to_the_garage() {
        let s = session(SessionType::Practice);
        let laps = vec![
            {
                let mut l = ours(0, 3120.4, 3122.4, (0, 5900), false);
                l.lap.out_lap = true;
                l
            },
            ours(1, 94.093, 3216.4, (5900, 8700), true),
            ours(2, 86.794, 3303.2, (8700, 11300), true),
            ours(3, 86.794, 3391.0, (11300, 13900), true), // stale
            // Esc ten seconds into the lap, recorded as a 21 s "valid lap".
            ours(4, 21.211, 3412.2, (13900, 14500), true),
        ];
        let rlaps = vec![
            theirs(1, None, None, false),
            theirs(2, Some(3122.4), Some(94.093), false),
            theirs(3, Some(3216.5), Some(86.794), false),
            theirs(4, Some(3303.3), Some(87.742), false),
            theirs(5, Some(3391.04), None, false),
        ];
        let p = plan(&s, &laps, &result(), &rlaps, 14_500);
        assert_eq!(p.report.skipped, None);
        assert_eq!(p.laps[0].lap.note, Some(LapNote::OutLap));
        assert!(!p.laps[0].lap.valid && !p.laps[0].lap.timed_by_game);
        assert!((p.laps[3].lap.lap_time_s - 87.742).abs() < 1e-3);
        let esc = &p.laps[4].lap;
        assert_eq!(esc.note, Some(LapNote::Aborted));
        assert!(!esc.valid && !esc.timed_by_game);
        assert!(
            (esc.lap_time_s - 21.211).abs() < 1e-3,
            "kept as measured live"
        );
        assert_eq!(p.report.best_lap_s, Some(86.794));
    }

    #[test]
    fn pit_laps() {
        let s = session(SessionType::Race);
        let laps = vec![
            ours(3, 83.8, 522.4, (0, 1000), true),
            ours(4, 136.0, 658.4, (1000, 2000), true),
            ours(5, 197.5, 856.0, (2000, 3000), true),
            ours(6, 85.9, 941.9, (3000, 4000), true),
        ];
        let rlaps = vec![
            theirs(4, Some(438.64), Some(83.8165), false),
            theirs(5, Some(522.46), Some(136.014), true),
            theirs(6, Some(658.46), Some(197.5449), false),
            theirs(7, Some(856.02), Some(85.9354), false),
        ];
        let p = plan(&s, &laps, &result(), &rlaps, 4000);
        let notes: Vec<_> = p.laps.iter().map(|l| l.lap.note).collect();
        assert_eq!(
            notes,
            vec![None, Some(LapNote::InLap), Some(LapNote::OutLap), None]
        );
        assert!(p.laps[1].lap.timed_by_game && !p.laps[1].lap.valid);
        assert!(p.laps[2].lap.timed_by_game && !p.laps[2].lap.valid);
        assert_eq!(p.report.best_lap_s, Some(83.8165));
    }

    #[test]
    fn a_file_about_other_laps_changes_nothing() {
        let s = session(SessionType::Practice);
        let laps = vec![
            ours(0, 90.0, 500.0, (0, 1000), true),
            ours(1, 88.0, 588.0, (1000, 2000), true),
            ours(2, 87.0, 675.0, (2000, 3000), true),
        ];
        // Same lap numbers, a completely different timeline.
        let rlaps = vec![
            theirs(1, Some(4000.0), Some(95.0), false),
            theirs(2, Some(4095.0), Some(94.0), false),
            theirs(3, Some(4189.0), Some(93.0), false),
        ];
        let p = plan(&s, &laps, &result(), &rlaps, 3000);
        assert!(p.report.skipped.is_some());
        assert_eq!(p.laps, laps);
        // No laps at all in the file.
        let p = plan(&s, &laps, &result(), &[], 3000);
        assert!(p.report.skipped.is_some());
    }

    #[test]
    fn laps_far_from_the_recording_are_not_invented() {
        let s = session(SessionType::Practice);
        // We only recorded laps 10 and 11 (0-based) of a long stint.
        let laps = vec![
            ours(10, 88.0, 1088.0, (0, 1000), true),
            ours(11, 87.0, 1175.0, (1000, 2000), true),
        ];
        let rlaps: Vec<ResultLapRow> = (1..=14)
            .map(|n| theirs(n, Some(120.0 + 88.0 * (n - 1) as f64), Some(88.0), false))
            .collect();
        let p = plan(&s, &laps, &result(), &rlaps, 2000);
        let nums: Vec<i32> = p.laps.iter().map(|l| l.lap.lap_number).collect();
        // Nothing was recorded after lap 11: nothing is added.
        assert_eq!(nums, vec![10, 11]);
        assert_eq!(p.report.matched, 2);
        // With samples left after the last recorded lap, the next one is ours.
        let p = plan(&s, &laps, &result(), &rlaps, 2600);
        let nums: Vec<i32> = p.laps.iter().map(|l| l.lap.lap_number).collect();
        assert_eq!(nums, vec![10, 11, 12]);
        assert_eq!(
            (p.laps[2].sample_row_start, p.laps[2].sample_row_end),
            (2000, 2600)
        );
    }
}
