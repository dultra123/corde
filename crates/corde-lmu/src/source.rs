//! [`LmuSource`] — the `corde_core::TelemetrySource` implementation for Le
//! Mans Ultimate.
//!
//! Pipeline per [`TelemetrySource::poll`]:
//!
//! 1. the platform reader ([`crate::reader`]) copies a consistent snapshot of
//!    the `LMU_Data` block into an owned buffer;
//! 2. the two "witness" clocks of the block — `scoringInfo.mCurrentET` and the
//!    player's `mElapsedTime` — are compared with the previous snapshot: when
//!    neither moved, nothing new happened and `Ok(None)` is returned;
//! 3. otherwise the block is converted with [`crate::convert::to_frame`].
//!
//! Disconnection is detected in two ways: the reader reports that the mapping
//! (or the `/dev/shm` mirror) disappeared, or the witness clocks stayed frozen
//! for longer than [`LmuSourceConfig::stall_timeout`] (the game exited without
//! removing the mirror, is paused in a menu…). Both yield
//! [`SourceError::NotAvailable`], after which the runner calls
//! [`TelemetrySource::connect`] again with a back-off.

use crate::convert::to_frame;
use crate::layout::SharedMemoryObjectOut;
use crate::reader::{open_default_reader, RawReader, ReadOutcome};
use corde_core::{SourceError, SourceInfo, TelemetrySource};
use corde_protocol::{GameId, SourceStatus, TelemetryFrame};
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Native update rate of the telemetry block (Hz).
pub const NATIVE_RATE_HZ: f32 = 100.0;

/// Identifier used in [`TelemetryFrame::source`] on this platform.
pub const SOURCE_ID: &str = if cfg!(windows) {
    "lmu-shm"
} else {
    "lmu-shm-linux"
};

/// Options of [`LmuSource`].
#[derive(Debug, Clone)]
pub struct LmuSourceConfig {
    /// Path of the mirrored block on Linux (`/dev/shm/LMU_Data` by default).
    /// Ignored on Windows, where the native mapping is always used.
    pub shm_path: Option<PathBuf>,
    /// How long the witness clocks may stay frozen before the source reports
    /// itself as disconnected. Default 5 s.
    pub stall_timeout: Duration,
    /// Number of consecutive torn copies tolerated before an
    /// [`SourceError::Invalid`] is reported (the bridge writes too slowly, or
    /// the layout does not match). Default 200 (≈ 2 s at 100 Hz).
    pub max_consecutive_torn: u32,
}

impl Default for LmuSourceConfig {
    fn default() -> Self {
        Self {
            shm_path: None,
            stall_timeout: Duration::from_secs(5),
            max_consecutive_torn: 200,
        }
    }
}

/// Live health of the source, refreshed on every poll.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LmuStatus {
    /// Reader open and data moving.
    pub connected: bool,
    /// Description of the reader (path, mapping size, events / lock…).
    pub reader: Option<String>,
    /// `generic.gameVersion` of the last snapshot (e.g. `1430`).
    /// TODO(verify-on-pc): exact encoding of the integer (1.4.3 → 1430 ? 143 ?).
    pub game_version: Option<i32>,
    /// `scoringInfo.mInRealtime` of the last snapshot.
    pub in_realtime: bool,
    /// `telemetry.playerHasVehicle` of the last snapshot.
    pub player_has_vehicle: bool,
    /// `telemetry.activeVehicles` of the last snapshot.
    pub active_vehicles: u8,
    /// Frames emitted since the source was created.
    pub frames: u64,
    /// Snapshots copied (including those without new data).
    pub reads: u64,
    /// Copies rejected by the witness check.
    pub torn_reads: u64,
    /// Measured rate of new frames over the last second, Hz.
    pub input_rate_hz: f32,
    /// Seconds since the witness clocks last moved (0 when never read).
    pub frozen_for_s: f32,
    pub last_error: Option<String>,
    /// Identifier put in [`TelemetryFrame::source`] (`"lmu-shm"` / `"lmu-shm-linux"`).
    pub source_id: String,
}

impl LmuStatus {
    /// The protocol-level status, to be completed by the server
    /// (`clients`, `recording`, `output_rate_hz`).
    pub fn to_source_status(&self) -> SourceStatus {
        SourceStatus {
            connected: self.connected,
            game: GameId::Lmu,
            source: self.source_id.clone(),
            input_rate_hz: self.input_rate_hz,
            output_rate_hz: 0.0,
            in_realtime: self.in_realtime,
            game_version: self.game_version.map(format_game_version),
            last_error: self.last_error.clone(),
            clients: 0,
            recording: false,
        }
    }
}

/// `generic.gameVersion` → display string.
///
/// TODO(verify-on-pc): the integer's encoding is unknown; `1430` is shown as
/// "1.4.3.0" by analogy with the `rF2` build numbers. Replace once observed.
pub fn format_game_version(v: i32) -> String {
    if v <= 0 {
        return v.to_string();
    }
    let s = v.to_string();
    let parts: Vec<String> = s.chars().map(|c| c.to_string()).collect();
    parts.join(".")
}

/// Le Mans Ultimate shared-memory source.
pub struct LmuSource {
    cfg: LmuSourceConfig,
    reader: Option<Box<dyn RawReader>>,
    buf: Box<SharedMemoryObjectOut>,
    /// Skeleton of the previous frame (only `player` is kept): what
    /// [`to_frame`] needs to carry scoring fields over while the scoring
    /// block lags behind the telemetry block. Cloning the whole frame
    /// (104 standings) at 100 Hz would be wasteful.
    prev_frame: Option<TelemetryFrame>,
    seq: u64,
    /// Witness clocks of the last *fresh* snapshot: `(mCurrentET, player mElapsedTime)`.
    last_witness: Option<(f64, f64)>,
    last_change: Instant,
    consecutive_torn: u32,
    rate_window_start: Instant,
    rate_window_frames: u32,
    status: LmuStatus,
    /// `RawReader::source_id` of the open reader (else [`SOURCE_ID`]).
    source_id: &'static str,
}

impl Default for LmuSource {
    fn default() -> Self {
        Self::new()
    }
}

impl LmuSource {
    /// Source with the default configuration (native mapping on Windows,
    /// `/dev/shm/LMU_Data` on Linux).
    pub fn new() -> Self {
        Self::with_config(LmuSourceConfig::default())
    }

    pub fn with_config(cfg: LmuSourceConfig) -> Self {
        let now = Instant::now();
        Self {
            cfg,
            reader: None,
            buf: SharedMemoryObjectOut::boxed(),
            prev_frame: None,
            seq: 0,
            last_witness: None,
            last_change: now,
            consecutive_torn: 0,
            rate_window_start: now,
            rate_window_frames: 0,
            status: LmuStatus {
                source_id: SOURCE_ID.to_string(),
                ..Default::default()
            },
            source_id: SOURCE_ID,
        }
    }

    pub fn config(&self) -> &LmuSourceConfig {
        &self.cfg
    }

    /// Current health (cheap clone).
    pub fn status(&self) -> LmuStatus {
        let mut s = self.status.clone();
        s.frozen_for_s = if self.last_witness.is_some() {
            self.last_change.elapsed().as_secs_f32()
        } else {
            0.0
        };
        s
    }

    /// The last consistent raw snapshot (zeroed before the first read).
    /// Useful for tools (`lmu-probe`) that want to look at raw fields.
    pub fn raw(&self) -> &SharedMemoryObjectOut {
        &self.buf
    }

    /// The player's part of the last frame emitted, if any.
    pub fn last_player(&self) -> Option<&corde_protocol::PlayerTelemetry> {
        self.prev_frame.as_ref().and_then(|f| f.player.as_ref())
    }

    pub fn is_connected(&self) -> bool {
        self.reader.is_some() && self.status.connected
    }

    fn fail(&mut self, err: SourceError) -> SourceError {
        self.status.connected = false;
        self.status.last_error = Some(err.to_string());
        if matches!(err, SourceError::NotAvailable(_)) {
            self.reader = None;
            self.status.reader = None;
        }
        err
    }

    fn note_frame(&mut self) {
        self.status.frames += 1;
        self.rate_window_frames += 1;
        let elapsed = self.rate_window_start.elapsed();
        if elapsed >= Duration::from_secs(1) {
            self.status.input_rate_hz = self.rate_window_frames as f32 / elapsed.as_secs_f32();
            self.rate_window_start = Instant::now();
            self.rate_window_frames = 0;
        }
    }

    fn refresh_status_from_block(&mut self) {
        let raw = &self.buf;
        self.status.game_version = Some(raw.generic.game_version);
        self.status.in_realtime = raw.scoring.scoring_info.in_realtime != 0;
        self.status.player_has_vehicle = raw.telemetry.player_has_vehicle != 0;
        self.status.active_vehicles = raw.telemetry.active_vehicles;
    }
}

impl TelemetrySource for LmuSource {
    fn info(&self) -> SourceInfo {
        SourceInfo {
            game: GameId::Lmu,
            id: self.source_id,
            label: if cfg!(windows) {
                "Le Mans Ultimate (mémoire partagée LMU_Data)".to_string()
            } else {
                format!(
                    "Le Mans Ultimate (miroir {})",
                    self.cfg
                        .shm_path
                        .as_ref()
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|| "/dev/shm/LMU_Data".to_string())
                )
            },
            native_rate_hz: NATIVE_RATE_HZ,
        }
    }

    fn connect(&mut self) -> Result<(), SourceError> {
        let reader = open_default_reader(self.cfg.shm_path.as_deref())?;
        tracing::info!(reader = %reader.describe(), "LMU shared memory opened");
        self.status.reader = Some(reader.describe());
        self.source_id = reader.source_id();
        self.status.source_id = self.source_id.to_string();
        self.status.connected = true;
        self.status.last_error = None;
        self.reader = Some(reader);
        // Keep `last_witness` across reconnections so that a stale mirror
        // does not produce one frame per reconnection; only the stall clock
        // restarts.
        self.last_change = Instant::now();
        self.consecutive_torn = 0;
        self.rate_window_start = Instant::now();
        self.rate_window_frames = 0;
        Ok(())
    }

    /// Note: on Windows a poll waits on the game's update events (up to
    /// 2 × 200 ms when nothing is signalled), so the runner thread should be
    /// dedicated to the source — which is what `corde-server` does.
    fn poll(&mut self) -> Result<Option<TelemetryFrame>, SourceError> {
        let Some(reader) = self.reader.as_mut() else {
            return Err(self.fail(SourceError::NotAvailable("not connected".into())));
        };

        let outcome = match reader.read(&mut self.buf) {
            Ok(o) => o,
            Err(e) => return Err(self.fail(e)),
        };
        self.status.reads += 1;

        match outcome {
            ReadOutcome::NoNewData => {}
            ReadOutcome::Torn => {
                self.status.torn_reads += 1;
                self.consecutive_torn += 1;
                if self.consecutive_torn >= self.cfg.max_consecutive_torn {
                    self.consecutive_torn = 0;
                    return Err(self.fail(SourceError::Invalid(format!(
                        "{} consecutive torn copies of LMU_Data (writer too slow or layout mismatch)",
                        self.cfg.max_consecutive_torn
                    ))));
                }
            }
            ReadOutcome::Fresh => {
                self.consecutive_torn = 0;
                let witness = (self.buf.current_et(), self.buf.player_elapsed_time());
                if self.last_witness != Some(witness) {
                    self.last_witness = Some(witness);
                    self.last_change = Instant::now();
                    self.refresh_status_from_block();
                    self.status.connected = true;
                    self.status.last_error = None;
                    self.seq += 1;
                    let frame = to_frame(
                        &self.buf,
                        self.prev_frame.as_ref(),
                        self.seq,
                        self.source_id,
                    );
                    self.prev_frame = Some(TelemetryFrame {
                        player: frame.player.clone(),
                        ..Default::default()
                    });
                    self.note_frame();
                    return Ok(Some(frame));
                }
            }
        }

        // Nothing new: is the game still there?
        let frozen = self.last_change.elapsed();
        if frozen >= self.cfg.stall_timeout {
            let reader = self.reader.as_mut().expect("reader checked above");
            match reader.probe() {
                // The mapping is gone (game exited, bridge stopped): reconnect.
                Err(e) => return Err(self.fail(e)),
                // The mapping is still there but frozen: the game is paused,
                // in the menus or loading a track. Stay connected, report
                // "not in realtime" and probe again after another stall period.
                Ok(()) => {
                    self.status.in_realtime = false;
                    self.status.frozen_for_s = frozen.as_secs_f32();
                    self.status.last_error = Some(format!(
                        "LMU_Data figé depuis {:.0} s (jeu en pause, au menu ou en chargement)",
                        frozen.as_secs_f32()
                    ));
                    self.last_change = Instant::now();
                }
            }
        }
        Ok(None)
    }

    fn disconnect(&mut self) {
        self.reader = None;
        self.status.connected = false;
        self.status.reader = None;
        self.status.input_rate_hz = 0.0;
    }

    /// `LMU_Data.paths.userData` of the last block read (mapped to the host
    /// filesystem on Linux), once the game filled it.
    fn user_data_dir(&self) -> Option<PathBuf> {
        self.reader.as_ref()?;
        crate::settings::user_data_from_shm(&self.buf).map(|p| p.user_data)
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::fake::FakeWorld;
    use crate::layout::SHM_TOTAL_SIZE;
    use std::io::{Seek, SeekFrom, Write};

    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("corde-source-{}-{}", std::process::id(), name))
    }

    fn write_block(path: &PathBuf, raw: &SharedMemoryObjectOut) {
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(path)
            .unwrap();
        f.seek(SeekFrom::Start(0)).unwrap();
        f.write_all(raw.as_bytes()).unwrap();
        f.flush().unwrap();
    }

    #[test]
    fn not_available_without_file() {
        let p = temp_path("missing");
        let _ = std::fs::remove_file(&p);
        let mut src = LmuSource::with_config(LmuSourceConfig {
            shm_path: Some(p),
            ..Default::default()
        });
        assert!(matches!(src.connect(), Err(SourceError::NotAvailable(_))));
        assert!(matches!(src.poll(), Err(SourceError::NotAvailable(_))));
        assert!(!src.is_connected());
    }

    #[test]
    fn emits_only_on_change_and_detects_stall() {
        let p = temp_path("flow");
        let mut world = FakeWorld::new();
        let mut raw = SharedMemoryObjectOut::boxed();
        world.step(0.5);
        world.write_into(&mut raw);
        write_block(&p, &raw);
        assert_eq!(
            std::fs::metadata(&p).unwrap().len() as usize,
            SHM_TOTAL_SIZE
        );

        let mut src = LmuSource::with_config(LmuSourceConfig {
            shm_path: Some(p.clone()),
            stall_timeout: Duration::from_millis(60),
            ..Default::default()
        });
        assert_eq!(src.info().id, "lmu-shm-linux");
        src.connect().unwrap();
        assert!(src.is_connected());

        let f1 = src.poll().unwrap().expect("first frame");
        assert_eq!(f1.seq, 1);
        assert_eq!(f1.source, "lmu-shm-linux");
        assert_eq!(f1.session.num_vehicles, 4);
        assert!(f1.player.is_some());
        assert!(src.status().connected);
        assert_eq!(src.status().game_version, Some(1430));
        assert_eq!(src.status().frames, 1);

        // Same data → nothing new.
        assert!(src.poll().unwrap().is_none());
        assert!(src.poll().unwrap().is_none());

        // Advance the world → new frame.
        world.step(0.01);
        world.write_into(&mut raw);
        write_block(&p, &raw);
        let f2 = src.poll().unwrap().expect("second frame");
        assert_eq!(f2.seq, 2);
        assert!(f2.session.elapsed_s > f1.session.elapsed_s);

        // Frozen beyond the stall timeout while the file still exists: the
        // game is paused or in the menus. We stay connected, flag it, and do
        // not emit anything.
        std::thread::sleep(Duration::from_millis(80));
        assert!(src.poll().unwrap().is_none());
        assert!(src.is_connected());
        assert!(!src.status().in_realtime);
        assert!(src.status().last_error.is_some());
        assert!(src.status().frozen_for_s > 0.0);

        // Data moves again → a frame, and the flag clears.
        world.step(0.01);
        world.write_into(&mut raw);
        write_block(&p, &raw);
        let f3 = src.poll().unwrap().expect("frame after resume");
        assert_eq!(f3.seq, 3);
        assert!(src.status().last_error.is_none());

        // File removed → NotAvailable after the stall timeout.
        std::fs::remove_file(&p).unwrap();
        std::thread::sleep(Duration::from_millis(80));
        assert!(matches!(src.poll(), Err(SourceError::NotAvailable(_))));
        assert!(!src.is_connected());
    }

    #[test]
    fn game_version_formatting() {
        assert_eq!(format_game_version(1430), "1.4.3.0");
        assert_eq!(format_game_version(0), "0");
    }
}
