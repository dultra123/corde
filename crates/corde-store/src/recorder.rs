//! The recorder: a state machine fed with every frame on the source thread.
//!
//! ```text
//!            in_realtime && player && phase != SessionOver   (or POST /api/recording/start)
//!   Idle ──────────────────────────────────────────────────────────────────────▶ Recording
//!     ▲                                                                            │
//!     │  session changed (track / index / SessionStarted / time went backwards),   │
//!     │  SessionEnded, source lost, out of realtime > 60 s, stop requested,         │
//!     └──────────────────────── server shutdown (flush guaranteed) ◀───────────────┘
//! ```
//!
//! While recording: frames are decimated to `record_rate_hz` on session time
//! and appended to the Parquet writer; every lap closes with a SQLite commit
//! (lap row + counters + a Parquet checkpoint); a checkpoint also happens at
//! least every `flush_interval`, so a crash loses at most that much.

use crate::channels::Ctx;
use crate::db::{self, Db, LapRow, SampleFileRow, SessionRow};
use crate::error::Result;
use crate::samples::{FileInfo, SampleWriter, WriterConfig};
use crate::session_id::make_session_id;
use crate::store::Store;
use corde_protocol::{GameEvent, GamePhase, LapRecord, PlayerTelemetry, TelemetryFrame};
use serde::Serialize;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq)]
pub struct RecorderConfig {
    /// Start automatically when the player is on track (`--record` / `--no-record`).
    pub enabled: bool,
    /// Sample rate on disk, Hz (decimation on session time; never above the source rate).
    pub record_rate_hz: f32,
    /// Parquet checkpoint + SQLite counters at least this often.
    pub flush_interval: Duration,
    /// Close the session when the player stays out of realtime this long.
    pub realtime_timeout: Duration,
    pub writer: WriterConfig,
}

impl Default for RecorderConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            record_rate_hz: 50.0,
            flush_interval: Duration::from_secs(30),
            realtime_timeout: Duration::from_secs(60),
            writer: WriterConfig::default(),
        }
    }
}

/// What `GET /api/recording` returns.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct RecordingStatus {
    /// A session is open on disk.
    pub recording: bool,
    /// Auto-start is armed (`false` after `--no-record` or a manual stop,
    /// until the next session or a manual start).
    pub auto: bool,
    /// Manual start requested; waiting for a frame with a player.
    pub armed: bool,
    pub session_id: Option<String>,
    pub started_wall_ms: Option<i64>,
    /// Sample rows appended (buffered rows included).
    pub rows: u64,
    pub laps: u32,
    /// Bytes on disk for this session (Parquet files).
    pub bytes: u64,
    pub files: u32,
    pub record_rate_hz: f32,
    pub last_flush_wall_ms: Option<i64>,
    /// Why samples are not being written right now (recording but idle).
    pub idle_reason: Option<String>,
    /// Why the last session was closed.
    pub last_stop_reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Command {
    Start,
    Stop,
}

/// Shared between the recorder (source thread) and the HTTP API.
#[derive(Debug, Default)]
pub struct RecorderHandle {
    status: Mutex<RecordingStatus>,
    command: Mutex<Option<Command>>,
}

impl RecorderHandle {
    pub fn status(&self) -> RecordingStatus {
        self.status.lock().map(|s| s.clone()).unwrap_or_default()
    }

    /// `POST /api/recording/start`: record from the next frame with a player,
    /// whatever the realtime state, until stopped or the session changes.
    pub fn request_start(&self) {
        self.set_command(Command::Start);
    }

    /// `POST /api/recording/stop`: close the session; auto-start stays off
    /// until the next session.
    pub fn request_stop(&self) {
        self.set_command(Command::Stop);
    }

    fn set_command(&self, c: Command) {
        if let Ok(mut g) = self.command.lock() {
            *g = Some(c);
        }
        if let Ok(mut s) = self.status.lock() {
            s.armed = c == Command::Start;
        }
    }

    fn take_command(&self) -> Option<Command> {
        self.command.lock().ok().and_then(|mut g| g.take())
    }

    fn update(&self, f: impl FnOnce(&mut RecordingStatus)) {
        if let Ok(mut s) = self.status.lock() {
            f(&mut s);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// Start / stop follow the game.
    Auto,
    /// Manual stop: no auto-start until the session changes.
    Paused,
    /// Manual start: record regardless of realtime; stop only on session change / stop.
    Forced,
}

/// One open session.
struct Active {
    id: String,
    track: String,
    session_index: i32,
    writer: SampleWriter,
    /// Session time of the next sample to keep.
    next_due_t: Option<f64>,
    last_t: f64,
    src_dt: f64,
    last_seq: Option<u64>,
    /// First global row of each lap seen in the samples.
    lap_starts: BTreeMap<i32, u64>,
    current_lap: Option<i32>,
    laps: u32,
    best_lap_s: Option<f32>,
    last_flush: Instant,
    last_wall_ms: i64,
    last_elapsed_s: f64,
    last_lap_number: i32,
    /// Rows appended since the last checkpoint.
    dirty: bool,
}

/// The recorder itself. Not `Sync`: lives on the source thread.
pub struct Recorder {
    store: Arc<Store>,
    db: Db,
    cfg: RecorderConfig,
    handle: Arc<RecorderHandle>,
    mode: Mode,
    active: Option<Active>,
    not_realtime_since: Option<Instant>,
    /// Session key (track, index) seen on the last frame, to reset manual overrides.
    last_key: Option<(String, i32)>,
}

impl std::fmt::Debug for Recorder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Recorder")
            .field("mode", &self.mode)
            .field("active", &self.active.as_ref().map(|a| &a.id))
            .finish()
    }
}

impl Recorder {
    /// Opens a dedicated database connection. The returned handle is for the API.
    pub fn new(store: Arc<Store>, cfg: RecorderConfig) -> Result<(Self, Arc<RecorderHandle>)> {
        let db = store.open_db()?;
        let handle = Arc::new(RecorderHandle::default());
        handle.update(|s| {
            s.auto = cfg.enabled;
            s.record_rate_hz = cfg.record_rate_hz;
        });
        let mode = Mode::Auto;
        Ok((
            Self {
                store,
                db,
                cfg,
                handle: Arc::clone(&handle),
                mode,
                active: None,
                not_realtime_since: None,
                last_key: None,
            },
            handle,
        ))
    }

    pub fn handle(&self) -> &Arc<RecorderHandle> {
        &self.handle
    }

    pub fn is_recording(&self) -> bool {
        self.active.is_some()
    }

    pub fn session_id(&self) -> Option<&str> {
        self.active.as_ref().map(|a| a.id.as_str())
    }

    /// A game event detected on `frame` (call before [`Recorder::on_frame`]).
    pub fn on_event(&mut self, frame: &TelemetryFrame, event: &GameEvent) -> Vec<GameEvent> {
        let mut out = Vec::new();
        match event {
            GameEvent::SessionStarted { .. } => {
                if let Some(ev) = self.stop("new session") {
                    out.push(ev);
                }
                // A new session clears any manual override.
                self.set_mode(Mode::Auto);
            }
            GameEvent::SessionEnded => {
                self.store_event(frame, event);
                if let Some(ev) = self.stop("session over") {
                    out.push(ev);
                }
            }
            GameEvent::RecordingStarted { .. } | GameEvent::RecordingStopped { .. } => {}
            _ => self.store_event(frame, event),
        }
        out
    }

    /// A lap reported by the lap tracker (call **before**
    /// [`Recorder::on_frame`] for that frame). The official time of a lap
    /// arrives a few frames after the line: samples of the next lap may
    /// already be on disk, so the lap ends where the next lap number starts.
    pub fn on_lap(&mut self, lap: &LapRecord) {
        let Some(a) = self.active.as_mut() else {
            return;
        };
        let rows = a.writer.rows();
        let end = a
            .lap_starts
            .range((lap.lap_number + 1)..)
            .next()
            .map(|(_, &row)| row)
            .unwrap_or(rows);
        // A lap without any sample (closed while idle) gets an empty range.
        let start = a.lap_starts.remove(&lap.lap_number).unwrap_or(end).min(end);
        // Forget starts of older laps (restarts) so the map stays tiny.
        a.lap_starts.retain(|&n, _| n > lap.lap_number);
        let files = a.writer.files();
        let sample_file = files
            .iter()
            .find(|f| f.row_start <= start && start < f.row_end)
            .map(|f| f.name.clone())
            .unwrap_or_else(|| a.writer.current_file_name());
        let row = LapRow {
            session_id: a.id.clone(),
            lap: lap.clone(),
            sample_file,
            sample_row_start: start as i64,
            sample_row_end: end as i64,
            time_source: db::TIME_SOURCE_LIVE.to_string(),
        };
        a.laps += 1;
        if lap.valid && lap.lap_time_s > 0.0 {
            a.best_lap_s = Some(match a.best_lap_s {
                Some(b) if b <= lap.lap_time_s => b,
                _ => lap.lap_time_s,
            });
        }
        // Commit point: the lap's samples are on disk before its row exists.
        self.checkpoint("lap");
        if let Err(e) = self.db.insert_lap(&row) {
            tracing::error!("recorder: cannot store lap {}: {e}", lap.lap_number);
        }
    }

    /// Every frame. Returns `RecordingStarted` / `RecordingStopped` events to broadcast.
    pub fn on_frame(&mut self, frame: &TelemetryFrame) -> Vec<GameEvent> {
        let mut out = Vec::new();
        let now = Instant::now();
        let key = (
            frame.session.track_name.clone(),
            frame.session.session_index,
        );

        // Manual commands.
        match self.handle.take_command() {
            Some(Command::Start) => {
                self.set_mode(Mode::Forced);
                if let Some(a) = &self.active {
                    tracing::info!("recorder: start requested, already recording {}", a.id);
                }
            }
            Some(Command::Stop) => {
                self.set_mode(Mode::Paused);
                if let Some(ev) = self.stop("stop requested") {
                    out.push(ev);
                }
            }
            None => {}
        }
        // A new session (track / index) clears any manual override.
        if self.last_key.as_ref() != Some(&key) {
            if self.last_key.is_some() {
                self.set_mode(Mode::Auto);
            }
            self.last_key = Some(key.clone());
        }

        // Session change / time travel while recording → close.
        let close_reason = self.active.as_ref().and_then(|a| {
            if a.track != key.0 || a.session_index != key.1 {
                Some("session changed")
            } else if frame.session.elapsed_s < a.last_t - 1.0 {
                Some("session restarted")
            } else {
                None
            }
        });
        if let Some(why) = close_reason {
            if let Some(ev) = self.stop(why) {
                out.push(ev);
            }
        }

        let in_realtime = frame.session.in_realtime;
        let player = frame.player.as_ref();

        // Out-of-realtime timer (auto mode only).
        if self.active.is_some() && self.mode != Mode::Forced {
            if in_realtime {
                self.not_realtime_since = None;
            } else {
                let since = *self.not_realtime_since.get_or_insert(now);
                if now.duration_since(since) >= self.cfg.realtime_timeout {
                    if let Some(ev) = self.stop("out of realtime") {
                        out.push(ev);
                    }
                    self.not_realtime_since = None;
                }
            }
        } else {
            self.not_realtime_since = None;
        }

        // Start?
        if self.active.is_none() {
            let auto_ok = self.mode == Mode::Auto
                && self.cfg.enabled
                && in_realtime
                && frame.session.phase != GamePhase::SessionOver;
            let forced = self.mode == Mode::Forced;
            if let Some(p) = player.filter(|_| auto_ok || forced) {
                match self.start(frame, p) {
                    Ok(ev) => out.push(ev),
                    Err(e) => {
                        tracing::error!("recorder: cannot start a session: {e}");
                        // Don't retry at 100 Hz.
                        self.mode = Mode::Paused;
                    }
                }
            }
        }

        // Record.
        let mut idle_reason: Option<&'static str> = None;
        let mut need_flush = false;
        let forced = self.mode == Mode::Forced;
        if let Some(a) = self.active.as_mut() {
            a.last_wall_ms = frame.wall_ms as i64;
            a.last_elapsed_s = frame.session.elapsed_s;
            match player {
                Some(p) if in_realtime || forced => {
                    if a.last_seq != Some(frame.seq) {
                        a.last_seq = Some(frame.seq);
                        if let Err(e) = Self::sample(a, &self.cfg, frame, p) {
                            tracing::error!("recorder: cannot write sample: {e}");
                        }
                    }
                }
                Some(_) => idle_reason = Some("not in realtime (menu / garage)"),
                None => idle_reason = Some("no player vehicle"),
            }
            need_flush = now.duration_since(a.last_flush) >= self.cfg.flush_interval;
        }
        if need_flush {
            self.checkpoint("timer");
        }
        self.publish_status(idle_reason);
        out
    }

    /// The source is gone (game closed): close the session.
    pub fn on_disconnect(&mut self) -> Vec<GameEvent> {
        self.stop("source disconnected").into_iter().collect()
    }

    /// Server shutdown: flush and close. Safe to call twice.
    pub fn shutdown(&mut self) -> Vec<GameEvent> {
        self.stop("server stopped").into_iter().collect()
    }

    // --- internals ----------------------------------------------------------

    fn set_mode(&mut self, mode: Mode) {
        if self.mode != mode {
            self.mode = mode;
            let auto = mode == Mode::Auto && self.cfg.enabled;
            self.handle.update(|st| st.auto = auto);
        }
    }

    fn sample(
        a: &mut Active,
        cfg: &RecorderConfig,
        frame: &TelemetryFrame,
        p: &PlayerTelemetry,
    ) -> Result<()> {
        let t = frame.session.elapsed_s;
        let period = 1.0 / f64::from(cfg.record_rate_hz.max(0.1));
        let dt = t - a.last_t;
        if a.next_due_t.is_some() && dt > 1e-4 && dt < 1.0 {
            a.src_dt = dt;
        }
        a.last_t = t;
        let due = match a.next_due_t {
            None => true,
            Some(next) => t + 0.5 * a.src_dt >= next,
        };
        if !due {
            return Ok(());
        }
        let mut next = a.next_due_t.map(|n| n + period).unwrap_or(t + period);
        if next <= t {
            next = t + period;
        }
        a.next_due_t = Some(next);

        let lap_number = p.lap.lap_number;
        let row = a.writer.append(&Ctx { frame, player: p })?;
        if a.current_lap != Some(lap_number) {
            a.current_lap = Some(lap_number);
            a.lap_starts.entry(lap_number).or_insert(row);
        }
        a.last_lap_number = lap_number;
        a.dirty = true;
        Ok(())
    }

    fn start(&mut self, frame: &TelemetryFrame, p: &PlayerTelemetry) -> Result<GameEvent> {
        let s = &frame.session;
        let mut id = make_session_id(frame.wall_ms, &s.track_name, s.session_type);
        let base = id.clone();
        let mut n = 2;
        while self.db.session_exists(&id)? {
            id = format!("{base}-{n}");
            n += 1;
        }
        let dir = self.store.session_dir(&id)?;
        let writer = SampleWriter::new(&dir, self.cfg.writer.clone())?;
        let row = SessionRow {
            id: id.clone(),
            game: s.game,
            track_name: s.track_name.clone(),
            track_length_m: s.track_length_m,
            session_type: s.session_type,
            session_index: s.session_index,
            started_wall_ms: frame.wall_ms as i64,
            ended_wall_ms: None,
            player_name: s.player_name.clone(),
            vehicle_name: p.vehicle_name.clone(),
            vehicle_model: p.vehicle_model.clone(),
            vehicle_class: p.vehicle_class,
            class_name: p.class_name.clone(),
            server_name: s.server_name.clone(),
            is_multiplayer: s.is_multiplayer,
            game_version: None,
            source: frame.source.clone(),
            record_rate_hz: self.cfg.record_rate_hz,
            sample_count: 0,
            lap_count: 0,
            best_lap_s: None,
            weather: s.weather.clone(),
            notes: None,
        };
        self.db.insert_session(&row)?;
        self.db.insert_event(
            &id,
            frame.wall_ms as i64,
            s.elapsed_s,
            p.lap.lap_number,
            &GameEvent::SessionStarted { session: s.clone() },
        )?;
        self.store.set_active(Some(id.clone()));
        tracing::info!(
            "enregistrement démarré : {id} ({} Hz, {})",
            self.cfg.record_rate_hz,
            dir.display()
        );
        self.active = Some(Active {
            id: id.clone(),
            track: s.track_name.clone(),
            session_index: s.session_index,
            writer,
            next_due_t: None,
            last_t: s.elapsed_s,
            src_dt: 0.01,
            last_seq: None,
            lap_starts: BTreeMap::new(),
            current_lap: None,
            laps: 0,
            best_lap_s: None,
            last_flush: Instant::now(),
            last_wall_ms: frame.wall_ms as i64,
            last_elapsed_s: s.elapsed_s,
            last_lap_number: p.lap.lap_number,
            dirty: false,
        });
        self.handle.update(|st| {
            st.recording = true;
            st.armed = false;
            st.session_id = Some(id.clone());
            st.started_wall_ms = Some(frame.wall_ms as i64);
            st.rows = 0;
            st.laps = 0;
            st.bytes = 0;
            st.files = 0;
            st.last_flush_wall_ms = None;
            st.idle_reason = None;
        });
        Ok(GameEvent::RecordingStarted { session_id: id })
    }

    /// Close the open session (no-op when idle).
    fn stop(&mut self, reason: &str) -> Option<GameEvent> {
        let mut a = self.active.take()?;
        let files = match a.writer.close() {
            Ok(files) => files,
            Err(e) => {
                tracing::error!("recorder: closing {} failed: {e}", a.id);
                a.writer.files()
            }
        };
        self.store_files(&a.id, &files);
        let rows = a.writer.rows();
        if let Err(e) = self.db.close_session(
            &a.id,
            a.last_wall_ms,
            rows as i64,
            i64::from(a.laps),
            a.best_lap_s,
            None,
        ) {
            tracing::error!("recorder: cannot close session {}: {e}", a.id);
        }
        self.store.set_active(None);
        let bytes: u64 = files.iter().map(|f| f.bytes).sum();
        tracing::info!(
            "enregistrement terminé : {} — {} tours, {} échantillons, {} fichier(s), {:.1} Mo ({reason})",
            a.id,
            a.laps,
            rows,
            files.len(),
            bytes as f64 / 1e6
        );
        let reason = reason.to_string();
        self.handle.update(|st| {
            st.recording = false;
            st.armed = false;
            st.session_id = None;
            st.started_wall_ms = None;
            st.rows = rows;
            st.laps = a.laps;
            st.bytes = bytes;
            st.files = files.len() as u32;
            st.idle_reason = None;
            st.last_stop_reason = Some(reason);
        });
        Some(GameEvent::RecordingStopped {
            session_id: a.id,
            laps: a.laps,
            samples: rows,
        })
    }

    /// Parquet checkpoint + SQLite counters + file table.
    fn checkpoint(&mut self, why: &str) {
        let Some(a) = self.active.as_mut() else {
            return;
        };
        a.last_flush = Instant::now();
        let files = if a.dirty {
            match a.writer.checkpoint() {
                Ok(files) => {
                    a.dirty = false;
                    Some(files)
                }
                Err(e) => {
                    tracing::error!("recorder: checkpoint failed: {e}");
                    None
                }
            }
        } else {
            None
        };
        let id = a.id.clone();
        let rows = a.writer.rows() as i64;
        let (laps, best) = (i64::from(a.laps), a.best_lap_s);
        if let Some(files) = files {
            self.store_files(&id, &files);
            if let Err(e) = self.db.update_session_progress(&id, rows, laps, best) {
                tracing::error!("recorder: cannot update counters: {e}");
            }
            tracing::debug!(
                "recorder: checkpoint ({why}) rows={rows} files={}",
                files.len()
            );
        }
        let wall = db::now_ms();
        self.handle.update(|st| st.last_flush_wall_ms = Some(wall));
    }

    fn store_files(&self, session_id: &str, files: &[FileInfo]) {
        for f in files {
            let row = SampleFileRow {
                session_id: session_id.to_string(),
                file_index: f.index as i32,
                name: f.name.clone(),
                row_start: f.row_start as i64,
                row_end: f.row_end as i64,
                bytes: f.bytes as i64,
                closed: f.closed,
            };
            if let Err(e) = self.db.upsert_sample_file(&row) {
                tracing::error!("recorder: cannot store file row {}: {e}", f.name);
            }
        }
    }

    fn store_event(&self, frame: &TelemetryFrame, event: &GameEvent) {
        let Some(a) = &self.active else { return };
        let lap = frame
            .player
            .as_ref()
            .map(|p| p.lap.lap_number)
            .unwrap_or(a.last_lap_number);
        if let Err(e) = self.db.insert_event(
            &a.id,
            frame.wall_ms as i64,
            frame.session.elapsed_s,
            lap,
            event,
        ) {
            tracing::error!("recorder: cannot store event: {e}");
        }
    }

    fn publish_status(&self, idle_reason: Option<&'static str>) {
        let Some(a) = &self.active else {
            return;
        };
        // Called at frame rate: no allocation here.
        let rows = a.writer.rows();
        let bytes = a.writer.bytes();
        let files = a.writer.file_count() as u32;
        let laps = a.laps;
        self.handle.update(|st| {
            st.rows = rows;
            st.bytes = bytes;
            st.files = files;
            st.laps = laps;
            st.idle_reason = idle_reason.map(str::to_string);
        });
    }
}

impl Drop for Recorder {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::SessionFilter;
    use corde_protocol::{SessionType, PROTOCOL_VERSION};
    use std::path::PathBuf;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "corde-recorder-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    /// A 100 Hz frame stream: `i` is the tick, laps of 300 ticks (3 s).
    fn frame(i: u64, in_realtime: bool) -> TelemetryFrame {
        frame_in(i, in_realtime, "Test Ring", 1)
    }

    fn frame_in(i: u64, in_realtime: bool, track: &str, index: i32) -> TelemetryFrame {
        let mut f = TelemetryFrame {
            v: PROTOCOL_VERSION,
            seq: i + 1,
            wall_ms: 1_762_200_000_000 + i * 10,
            source: "test".into(),
            ..Default::default()
        };
        f.session.track_name = track.into();
        f.session.session_type = SessionType::Practice;
        f.session.session_index = index;
        f.session.elapsed_s = i as f64 * 0.01;
        f.session.in_realtime = in_realtime;
        f.session.phase = GamePhase::Green;
        let mut p = PlayerTelemetry {
            vehicle_name: "Car".into(),
            ..Default::default()
        };
        p.lap.lap_number = (i / 300) as i32 + 1;
        p.lap.lap_start_s = (i / 300 * 300) as f64 * 0.01;
        p.lap.last_lap_s = Some(3.0);
        p.motion.speed_kmh = 100.0 + (i % 300) as f32;
        f.player = Some(p);
        f
    }

    #[test]
    fn records_decimates_and_closes_cleanly() {
        let dir = tmp("basic");
        let store = Store::open(&dir).unwrap();
        let cfg = RecorderConfig {
            record_rate_hz: 20.0,
            writer: WriterConfig {
                row_group_rows: 64,
                ..Default::default()
            },
            ..Default::default()
        };
        let (mut rec, handle) = Recorder::new(Arc::clone(&store), cfg).unwrap();
        let mut tracker = corde_core_like_tracker();
        let mut started = None;
        let mut laps = 0;
        for i in 0..1000u64 {
            let f = frame(i, true);
            if let Some(lap) = tracker.feed(&f) {
                rec.on_lap(&lap);
                laps += 1;
            }
            for ev in rec.on_frame(&f) {
                if let GameEvent::RecordingStarted { session_id } = ev {
                    started = Some(session_id);
                }
            }
        }
        let id = started.expect("recording started");
        assert!(rec.is_recording());
        assert!(store.is_active(&id));
        assert_eq!(laps, 3);
        let st = handle.status();
        assert!(st.recording);
        // 10 s at 20 Hz ≈ 200 rows (±1).
        assert!((199..=201).contains(&st.rows), "rows {}", st.rows);
        assert_eq!(st.laps, 3);

        // Delete is refused while recording.
        assert!(matches!(
            store.delete_session(&id),
            Err(crate::StoreError::Busy(_))
        ));

        let evs = rec.shutdown();
        assert!(matches!(
            evs.as_slice(),
            [GameEvent::RecordingStopped { laps: 3, .. }]
        ));
        assert!(!rec.is_recording());
        assert!(store.active().is_none());

        let detail = store.get_session(&id).unwrap();
        assert_eq!(detail.session.lap_count, 3);
        assert!(detail.session.ended_wall_ms.is_some());
        assert!((199..=201).contains(&detail.session.sample_count));
        assert_eq!(detail.laps.len(), 3);
        assert_eq!(detail.files.len(), 1);
        assert!(detail.files[0].closed);
        assert_eq!(detail.events[0].kind, "session_started");
        // Lap 2 covers ticks 300..600 → rows 60..120 at 20 Hz.
        let lap2 = &detail.laps[1];
        assert_eq!(lap2.lap.lap_number, 2);
        assert!((59..=61).contains(&lap2.sample_row_start), "{lap2:?}");
        assert!((119..=121).contains(&lap2.sample_row_end), "{lap2:?}");
        let s = store
            .lap_samples(&id, 2, &["speed_kmh", "lap_number"], 1000)
            .unwrap();
        assert_eq!(s.points as i64, lap2.sample_row_end - lap2.sample_row_start);
        assert!(s.t_s.windows(2).all(|w| w[1] > w[0]));
        assert!(s.channels[1].values.iter().all(|v| *v == Some(2.0)));
        let dt = s.t_s[1] - s.t_s[0];
        assert!((dt - 0.05).abs() < 1e-6, "dt {dt}");
        // Decimated request keeps the ends.
        let d = store.lap_samples(&id, 2, &["speed_kmh"], 10).unwrap();
        assert!(d.points <= 12 && d.points >= 6, "{}", d.points);
        assert_eq!(d.t_s[0], s.t_s[0]);
        assert_eq!(*d.t_s.last().unwrap(), *s.t_s.last().unwrap());
        assert!(store.lap_samples(&id, 2, &["nope"], 10).is_err());
        assert!(store
            .lap_samples(&id, 9, &["speed_kmh"], 10)
            .unwrap_err()
            .is_not_found());

        let stats = store.storage_stats().unwrap();
        assert_eq!(stats.sessions, 1);
        assert!(stats.samples_bytes > 0 && stats.db_bytes > 0);
        assert_eq!(
            store
                .list_sessions(&SessionFilter::default())
                .unwrap()
                .len(),
            1
        );
        store.delete_session(&id).unwrap();
        assert!(!dir.join("sessions").join(&id).exists());
        assert!(store.get_session(&id).unwrap_err().is_not_found());
        drop(rec);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn menu_visits_and_session_changes() {
        let dir = tmp("menu");
        let store = Store::open(&dir).unwrap();
        let cfg = RecorderConfig {
            record_rate_hz: 10.0,
            realtime_timeout: Duration::from_millis(200),
            ..Default::default()
        };
        let (mut rec, handle) = Recorder::new(Arc::clone(&store), cfg).unwrap();
        // Not in realtime: nothing starts.
        for i in 0..50 {
            assert!(rec.on_frame(&frame(i, false)).is_empty());
        }
        assert!(!rec.is_recording());
        // On track: starts.
        let evs = rec.on_frame(&frame(50, true));
        assert!(matches!(
            evs.as_slice(),
            [GameEvent::RecordingStarted { .. }]
        ));
        for i in 51..150 {
            rec.on_frame(&frame(i, true));
        }
        let rows_before = handle.status().rows;
        // Short menu visit: keeps the session, writes nothing.
        for i in 150..170 {
            rec.on_frame(&frame(i, false));
        }
        assert!(rec.is_recording());
        assert_eq!(handle.status().rows, rows_before);
        assert!(handle.status().idle_reason.is_some());
        // Long menu visit (> timeout) closes.
        std::thread::sleep(Duration::from_millis(250));
        let evs = rec.on_frame(&frame(170, false));
        assert!(
            matches!(evs.as_slice(), [GameEvent::RecordingStopped { .. }]),
            "{evs:?}"
        );
        assert!(!rec.is_recording());
        // Back on track → new session; then a session change closes it and
        // starts another one on the same frame.
        let evs = rec.on_frame(&frame(171, true));
        assert!(matches!(
            evs.as_slice(),
            [GameEvent::RecordingStarted { .. }]
        ));
        let other = frame_in(172, true, "Elsewhere", 2);
        let evs = rec.on_event(
            &other,
            &GameEvent::SessionStarted {
                session: other.session.clone(),
            },
        );
        assert!(matches!(
            evs.as_slice(),
            [GameEvent::RecordingStopped { .. }]
        ));
        let evs = rec.on_frame(&other);
        assert!(matches!(
            evs.as_slice(),
            [GameEvent::RecordingStarted { .. }]
        ));
        // Manual stop pauses auto-start; manual start forces even off-track.
        handle.request_stop();
        let evs = rec.on_frame(&frame_in(173, true, "Elsewhere", 2));
        assert!(matches!(
            evs.as_slice(),
            [GameEvent::RecordingStopped { .. }]
        ));
        assert!(rec
            .on_frame(&frame_in(174, true, "Elsewhere", 2))
            .is_empty());
        assert!(!rec.is_recording());
        handle.request_start();
        assert!(handle.status().armed);
        let evs = rec.on_frame(&frame_in(175, false, "Elsewhere", 2));
        assert!(matches!(
            evs.as_slice(),
            [GameEvent::RecordingStarted { .. }]
        ));
        assert!(!handle.status().armed);
        rec.shutdown();
        let all = store.list_sessions(&SessionFilter::default()).unwrap();
        assert_eq!(all.len(), 4, "{all:#?}");
        assert!(all.iter().all(|s| s.ended_wall_ms.is_some()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn disabled_recorder_never_starts_unless_forced() {
        let dir = tmp("off");
        let store = Store::open(&dir).unwrap();
        let cfg = RecorderConfig {
            enabled: false,
            ..Default::default()
        };
        let (mut rec, handle) = Recorder::new(Arc::clone(&store), cfg).unwrap();
        for i in 0..100 {
            assert!(rec.on_frame(&frame(i, true)).is_empty());
        }
        assert!(!handle.status().auto);
        handle.request_start();
        assert!(matches!(
            rec.on_frame(&frame(100, true)).as_slice(),
            [GameEvent::RecordingStarted { .. }]
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Minimal lap detector for the tests (the real one lives in corde-core,
    /// which this crate does not depend on).
    struct MiniTracker {
        current: Option<i32>,
    }

    fn corde_core_like_tracker() -> MiniTracker {
        MiniTracker { current: None }
    }

    impl MiniTracker {
        fn feed(&mut self, f: &TelemetryFrame) -> Option<LapRecord> {
            let p = f.player.as_ref()?;
            let n = p.lap.lap_number;
            match self.current {
                None => {
                    self.current = Some(n);
                    None
                }
                Some(c) if n > c => {
                    self.current = Some(n);
                    Some(LapRecord {
                        lap_number: c,
                        lap_time_s: 3.0,
                        valid: true,
                        session_elapsed_end_s: f.session.elapsed_s,
                        ..Default::default()
                    })
                }
                _ => None,
            }
        }
    }
}
