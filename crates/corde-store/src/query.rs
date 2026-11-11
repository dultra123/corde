//! Read side of the store: what the HTTP API and the CLI expose.

use crate::channels::{self, Channel};
use crate::db::{EventRow, LapRow, SampleFileRow, SessionFilter, SessionRow, SetupRow};
use crate::decimate;
use crate::error::{Result, StoreError};
use crate::samples;
use crate::session_id::valid_id;
use crate::store::Store;
use serde::Serialize;
use std::path::Path;

/// Default `points` of `lap_samples`.
pub const DEFAULT_MAX_POINTS: usize = 2000;
/// Hard cap of `points` (a lap at 50 Hz is ≤ ~15 000 rows anyway).
pub const MAX_MAX_POINTS: usize = 50_000;

/// `GET /api/sessions/{id}`.
#[derive(Debug, Clone, Serialize)]
pub struct SessionDetail {
    #[serde(flatten)]
    pub session: SessionRow,
    pub laps: Vec<LapRow>,
    pub events: Vec<EventRow>,
    pub setups: Vec<SetupRow>,
    pub files: Vec<SampleFileRow>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChannelSeries {
    pub name: String,
    pub unit: String,
    /// One value per point; `null` where the game gave nothing.
    pub values: Vec<Option<f64>>,
}

/// `GET /api/sessions/{id}/laps/{lap}/samples`.
#[derive(Debug, Clone, Serialize)]
pub struct LapSamples {
    pub session_id: String,
    pub lap_number: i32,
    /// Rows of the lap on disk before decimation.
    pub total_rows: u64,
    /// Points returned (length of every array below).
    pub points: usize,
    pub t_s: Vec<f64>,
    pub lap_dist_m: Vec<f64>,
    pub channels: Vec<ChannelSeries>,
}

/// `GET /api/storage`.
#[derive(Debug, Clone, Serialize)]
pub struct StorageStats {
    pub path: String,
    pub sessions: i64,
    pub files: u64,
    pub db_bytes: u64,
    pub samples_bytes: u64,
    pub total_bytes: u64,
}

impl Store {
    pub fn list_sessions(&self, filter: &SessionFilter) -> Result<Vec<SessionRow>> {
        self.db().list_sessions(filter)
    }

    /// Session with its laps, events, setups and files; `NotFound` otherwise.
    pub fn get_session(&self, id: &str) -> Result<SessionDetail> {
        if !valid_id(id) {
            return Err(StoreError::invalid("session id", id));
        }
        let db = self.db();
        let session = db
            .get_session(id)?
            .ok_or_else(|| StoreError::not_found(format!("session {id}")))?;
        Ok(SessionDetail {
            laps: db.laps_of(id)?,
            events: db.events_of(id)?,
            setups: db.setups_of(id)?,
            files: db.sample_files_of(id)?,
            session,
        })
    }

    /// Samples of one lap, decimated to at most `max_points` rows (min/max of
    /// every requested channel preserved). `t_s` and `lap_dist_m` always come
    /// back as axes; they can also be requested as channels.
    pub fn lap_samples(
        &self,
        session_id: &str,
        lap_number: i32,
        channel_names: &[&str],
        max_points: usize,
    ) -> Result<LapSamples> {
        if !valid_id(session_id) {
            return Err(StoreError::invalid("session id", session_id));
        }
        let max_points = if max_points == 0 {
            DEFAULT_MAX_POINTS
        } else {
            max_points.min(MAX_MAX_POINTS)
        };
        let requested: Vec<&'static Channel> = channel_names
            .iter()
            .map(|n| channels::channel(n).ok_or_else(|| StoreError::invalid("channel", *n)))
            .collect::<Result<_>>()?;
        let (lap, files) = {
            let db = self.db();
            let lap = db.get_lap(session_id, lap_number)?.ok_or_else(|| {
                StoreError::not_found(format!("lap {lap_number} of session {session_id}"))
            })?;
            (lap, db.sample_files_of(session_id)?)
        };
        let dir = self.session_dir(session_id)?;
        let (row_start, row_end) = (
            lap.sample_row_start.max(0) as u64,
            lap.sample_row_end.max(0) as u64,
        );

        // Columns read: the two axes and what repairing them needs (wall
        // clock, speed), then the requested channels.
        const AXES: usize = 4;
        let t_ch = channels::channel("t_s").expect("t_s channel");
        let d_ch = channels::channel("lap_dist_m").expect("lap_dist_m channel");
        let w_ch = channels::channel("wall_ms").expect("wall_ms channel");
        let v_ch = channels::channel("speed_kmh").expect("speed_kmh channel");
        let mut read: Vec<&Channel> = vec![t_ch, d_ch, w_ch, v_ch];
        read.extend(requested.iter().copied());
        let mut cols: Vec<Vec<Option<f64>>> = read.iter().map(|_| Vec::new()).collect();
        read_range(&dir, &files, &read, row_start, row_end, &mut cols)?;
        let n = cols[0].len();

        let nan =
            |c: &[Option<f64>]| -> Vec<f64> { c.iter().map(|v| v.unwrap_or(f64::NAN)).collect() };
        let (mut t_all, mut d_all) = (nan(&cols[0]), nan(&cols[1]));
        // Sessions recorded with the old 0.2 s sample clock get usable axes.
        crate::legacy::repair_axes(&mut t_all, &mut d_all, &nan(&cols[2]), &nan(&cols[3]));
        // Channels that are also axes must show the repaired values.
        for (ch, col) in requested.iter().zip(cols[AXES..].iter_mut()) {
            let fixed = match ch.name {
                "t_s" => &t_all,
                "lap_dist_m" => &d_all,
                _ => continue,
            };
            *col = fixed.iter().map(|v| v.is_finite().then_some(*v)).collect();
        }

        let channel_cols: Vec<&[Option<f64>]> = cols[AXES..].iter().map(|c| c.as_slice()).collect();
        let idx = decimate::select_indices(&channel_cols, n, max_points);
        let t_s = idx.iter().map(|&i| t_all[i]).collect();
        let lap_dist_m = idx.iter().map(|&i| d_all[i]).collect();
        let series = requested
            .iter()
            .zip(cols[AXES..].iter())
            .map(|(ch, col)| ChannelSeries {
                name: ch.name.to_string(),
                unit: ch.unit.to_string(),
                values: decimate::take(col, &idx),
            })
            .collect();
        Ok(LapSamples {
            session_id: session_id.to_string(),
            lap_number,
            total_rows: n as u64,
            points: idx.len(),
            t_s,
            lap_dist_m,
            channels: series,
        })
    }

    /// Remove a session from the database and its directory from disk.
    /// `Busy` when it is being recorded, `NotFound` when unknown.
    pub fn delete_session(&self, id: &str) -> Result<()> {
        let dir = self.session_dir(id)?;
        if self.is_active(id) {
            return Err(StoreError::Busy(id.to_string()));
        }
        let existed = self.db().delete_session(id)?;
        match std::fs::remove_dir_all(&dir) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
        if existed {
            Ok(())
        } else {
            Err(StoreError::not_found(format!("session {id}")))
        }
    }

    /// Sizes on disk (database + WAL + every session directory).
    pub fn storage_stats(&self) -> Result<StorageStats> {
        let sessions = self.db().session_count()?;
        let db_bytes = ["", "-wal", "-shm"]
            .iter()
            .map(|suffix| {
                let mut p = self.db_path().into_os_string();
                p.push(suffix);
                std::fs::metadata(p).map(|m| m.len()).unwrap_or(0)
            })
            .sum::<u64>();
        let (samples_bytes, files) = dir_size(&self.sessions_dir());
        Ok(StorageStats {
            path: self.data_dir().display().to_string(),
            sessions,
            files,
            db_bytes,
            samples_bytes,
            total_bytes: db_bytes + samples_bytes,
        })
    }
}

/// Read `[row_start, row_end)` (global rows) across the session's files into
/// `cols` (one per channel in `read`). Files that cannot be read (no footer:
/// crash before a checkpoint) are skipped with a warning.
pub(crate) fn read_range(
    dir: &Path,
    files: &[SampleFileRow],
    read: &[&Channel],
    row_start: u64,
    row_end: u64,
    cols: &mut [Vec<Option<f64>>],
) -> Result<()> {
    for f in files {
        let (fs, fe) = (f.row_start.max(0) as u64, f.row_end.max(0) as u64);
        let lo = row_start.max(fs);
        let hi = row_end.min(fe);
        if lo >= hi {
            continue;
        }
        let path = dir.join(&f.name);
        match samples::read_columns(&path, read, lo - fs, hi - fs) {
            Ok(part) => {
                for (dst, src) in cols.iter_mut().zip(part) {
                    dst.extend(src);
                }
            }
            Err(e) => {
                tracing::warn!("cannot read {}: {e} — skipped", path.display());
            }
        }
    }
    Ok(())
}

/// Total size and file count under `dir` (0 when absent).
pub fn dir_size(dir: &Path) -> (u64, u64) {
    let mut bytes = 0;
    let mut files = 0;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in entries.flatten() {
            let Ok(meta) = e.metadata() else { continue };
            if meta.is_dir() {
                stack.push(e.path());
            } else {
                bytes += meta.len();
                files += 1;
            }
        }
    }
    (bytes, files)
}
