//! rFactor 2 as a telemetry source.
//!
//! A session is live when the plugin's buffers exist, name a circuit and vehicles, and
//! their write counters move. The plugin clears its buffers when a session ends: that is
//! how leaving a session is seen. A game that closes without clearing them is seen when
//! the buffers stop moving and can no longer be reopened.

use super::convert::Rf2Converter;
use super::layout::OFF_SCORING_INFO;
use super::reader::{open_reader, Rf2Raw, Rf2Reader};
use corde_core::{SourceError, SourceInfo, TelemetrySource};
use corde_protocol::{GameId, TelemetryFrame};
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// The game publishes its telemetry at up to 100 Hz (90 Hz by default, 50 in the plugin's
/// older builds); nothing faster is passed on.
pub const NATIVE_RATE_HZ: f32 = 100.0;
pub const SOURCE_ID: &str = "rf2-shm";
const LABEL: &str = "rFactor 2";

#[derive(Debug, Clone)]
pub struct Rf2SourceConfig {
    /// A folder holding the two buffers as files (tests, the fake game); `None`: the
    /// game's own mappings (Windows).
    pub dir: Option<PathBuf>,
    /// Without any write for this long, the buffers are reopened to see whether the game
    /// is still there (a pause, a loading screen) or gone.
    pub stall_timeout: Duration,
}

impl Default for Rf2SourceConfig {
    fn default() -> Self {
        Self {
            dir: None,
            stall_timeout: Duration::from_secs(5),
        }
    }
}

pub struct Rf2Source {
    cfg: Rf2SourceConfig,
    reader: Option<Box<dyn Rf2Reader>>,
    conv: Rf2Converter,
    raw: Rf2Raw,
    /// The counters at the last attempt to connect: a session is only taken once they
    /// have moved (buffers left behind by a game that crashed never do).
    seen: Option<(u32, u32)>,
    last_versions: Option<(u32, u32)>,
    last_change: Instant,
    last_frame: Option<Instant>,
}

/// Does the scoring buffer describe a session (a circuit, at least one vehicle)?
fn has_session(raw: &Rf2Raw) -> bool {
    raw.scoring_vehicles() > 0 && raw.scoring.get(OFF_SCORING_INFO).is_some_and(|b| *b != 0)
}

impl Rf2Source {
    pub fn new() -> Self {
        Self::with_config(Rf2SourceConfig::default())
    }

    pub fn with_config(cfg: Rf2SourceConfig) -> Self {
        Self {
            cfg,
            reader: None,
            conv: Rf2Converter::new(),
            raw: Rf2Raw::new(),
            seen: None,
            last_versions: None,
            last_change: Instant::now(),
            last_frame: None,
        }
    }

    fn lost(&mut self, why: impl Into<String>) -> SourceError {
        self.reader = None;
        self.seen = None;
        SourceError::NotAvailable(why.into())
    }
}

impl Default for Rf2Source {
    fn default() -> Self {
        Self::new()
    }
}

impl TelemetrySource for Rf2Source {
    fn info(&self) -> SourceInfo {
        SourceInfo {
            game: GameId::Rfactor2,
            id: SOURCE_ID,
            label: LABEL.into(),
            native_rate_hz: NATIVE_RATE_HZ,
        }
    }

    fn connect(&mut self) -> Result<(), SourceError> {
        self.reader = None;
        let mut reader = match open_reader(self.cfg.dir.as_deref()) {
            Ok(r) => r,
            Err(e) => {
                self.seen = None;
                return Err(e);
            }
        };
        if !reader.read(&mut self.raw)? {
            return Err(SourceError::NotAvailable(format!(
                "{LABEL} : zones en cours d'écriture"
            )));
        }
        if !has_session(&self.raw) {
            self.seen = None;
            return Err(SourceError::NotAvailable(format!(
                "{LABEL} : pas de session en cours"
            )));
        }
        // Alive, or left behind? Only a counter that moved since the last look says.
        let versions = self.raw.versions();
        match self.seen.replace(versions) {
            Some(before) if before != versions => {}
            _ => {
                return Err(SourceError::NotAvailable(format!(
                    "{LABEL} : session trouvée, en attente d'un signe de vie"
                )))
            }
        }
        tracing::info!("{LABEL} : {}", reader.describe());
        self.reader = Some(reader);
        self.conv = Rf2Converter::new();
        self.last_versions = None;
        self.last_change = Instant::now();
        self.last_frame = None;
        Ok(())
    }

    fn poll(&mut self) -> Result<Option<TelemetryFrame>, SourceError> {
        let Some(reader) = self.reader.as_mut() else {
            return Err(SourceError::NotAvailable("non connecté".into()));
        };
        let versions = match reader.versions() {
            Ok(v) => v,
            Err(SourceError::NotAvailable(why)) => return Err(self.lost(why)),
            Err(e) => return Err(e),
        };
        let now = Instant::now();
        if self.last_versions == Some(versions) {
            if now.duration_since(self.last_change) > self.cfg.stall_timeout {
                // Our own handle keeps a mapping alive after the game is gone: let go of
                // it and look again. Still there: a pause or a loading screen.
                self.reader = None;
                return match open_reader(self.cfg.dir.as_deref()) {
                    Ok(mut again) => {
                        let alive = again.read(&mut self.raw).unwrap_or(false);
                        if alive && !has_session(&self.raw) {
                            return Err(self.lost("session quittée"));
                        }
                        self.reader = Some(again);
                        self.last_change = now;
                        Ok(None)
                    }
                    Err(_) => Err(self.lost("plus rien ne bouge : le jeu a été fermé")),
                };
            }
            return Ok(None);
        }
        // Not faster than the game's own rate (the counters also move on writes that
        // change nothing).
        let min_gap = Duration::from_secs_f32(1.0 / NATIVE_RATE_HZ);
        if self
            .last_frame
            .is_some_and(|t| now.duration_since(t) < min_gap)
        {
            return Ok(None);
        }
        let whole = match reader.read(&mut self.raw) {
            Ok(w) => w,
            Err(SourceError::NotAvailable(why)) => return Err(self.lost(why)),
            Err(e) => return Err(e),
        };
        if !whole {
            // Caught in the middle of a write: the next poll takes it.
            return Ok(None);
        }
        self.last_versions = Some(self.raw.versions());
        self.last_change = now;
        if !has_session(&self.raw) {
            return Err(self.lost("session quittée"));
        }
        self.last_frame = Some(now);
        Ok(Some(self.conv.frame(&self.raw, SOURCE_ID)))
    }

    fn poll_interval(&self) -> Duration {
        Duration::from_millis(2)
    }

    fn disconnect(&mut self) {
        self.reader = None;
    }
}
