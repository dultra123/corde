//! [`EvoSource`] — the `corde_core::TelemetrySource` implementation for Assetto Corsa EVO.
//!
//! * `connect` opens the three pages of the game and checks that a session is running:
//!   at the menus the game is there but has nothing to say, and the source reports itself
//!   not available — which lets Corde look at the other games meanwhile.
//! * `poll` copies the pages; a new frame is made when one of the two step counters moved,
//!   at most [`NATIVE_RATE_HZ`] times a second.
//! * The game left (the pages are gone, the status went to « off », or nothing moved for
//!   [`EvoSourceConfig::stall_timeout`] while a session was live): not available, and the
//!   mappings are closed.

use super::convert::EvoConverter;
use super::layout::{status, text};
use crate::reader::{open_reader, PageReader, EVO_PAGES};
use corde_core::{SourceError, SourceInfo, TelemetrySource};
use corde_protocol::{GameId, TelemetryFrame};
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Rate of the frames Corde makes from the game (Hz).
pub const NATIVE_RATE_HZ: f32 = 100.0;
/// Identifier put in `TelemetryFrame::source`.
pub const SOURCE_ID: &str = "acevo-shm";
const LABEL: &str = "Assetto Corsa EVO";

#[derive(Debug, Clone)]
pub struct EvoSourceConfig {
    /// Read three files of this directory instead of the game's mappings (tests,
    /// development).
    pub dir: Option<PathBuf>,
    /// How long nothing may move while a session is live before the game is taken for
    /// gone. Default 5 s.
    pub stall_timeout: Duration,
    /// The same while the game is paused (the pages are frozen then). Default 30 min.
    pub paused_timeout: Duration,
}

impl Default for EvoSourceConfig {
    fn default() -> Self {
        Self {
            dir: None,
            stall_timeout: Duration::from_secs(5),
            paused_timeout: Duration::from_secs(30 * 60),
        }
    }
}

pub struct EvoSource {
    cfg: EvoSourceConfig,
    reader: Option<Box<dyn PageReader>>,
    conv: EvoConverter,
    origin: Instant,
    last_ids: Option<(i32, i32)>,
    last_change: Instant,
    last_frame: Option<Instant>,
}

impl EvoSource {
    pub fn new() -> Self {
        Self::with_config(EvoSourceConfig::default())
    }

    pub fn with_config(cfg: EvoSourceConfig) -> Self {
        let now = Instant::now();
        Self {
            cfg,
            reader: None,
            conv: EvoConverter::new(),
            origin: now,
            last_ids: None,
            last_change: now,
            last_frame: None,
        }
    }

    fn lost(&mut self, why: impl Into<String>) -> SourceError {
        self.reader = None;
        SourceError::NotAvailable(why.into())
    }
}

impl Default for EvoSource {
    fn default() -> Self {
        Self::new()
    }
}

impl TelemetrySource for EvoSource {
    fn info(&self) -> SourceInfo {
        SourceInfo {
            game: GameId::AcEvo,
            id: SOURCE_ID,
            label: LABEL.into(),
            native_rate_hz: NATIVE_RATE_HZ,
        }
    }

    fn connect(&mut self) -> Result<(), SourceError> {
        self.reader = None;
        let mut reader = open_reader(&EVO_PAGES, self.cfg.dir.as_deref())?;
        let pages = reader.read_raw()?.evo()?;
        if pages.graphics.status == status::OFF {
            return Err(SourceError::NotAvailable(format!(
                "{LABEL} est lancé, sans session en cours"
            )));
        }
        let (interface, build) = (pages.statics.sm_version, pages.statics.ac_evo_version);
        tracing::info!(
            "{LABEL} : {} — interface {}, jeu {}",
            reader.describe(),
            text(&interface),
            text(&build)
        );
        self.reader = Some(reader);
        self.conv = EvoConverter::new();
        self.last_ids = None;
        self.last_change = Instant::now();
        self.last_frame = None;
        Ok(())
    }

    fn poll(&mut self) -> Result<Option<TelemetryFrame>, SourceError> {
        let Some(reader) = self.reader.as_mut() else {
            return Err(SourceError::NotAvailable("non connecté".into()));
        };
        let pages = match reader.read_raw().and_then(|raw| raw.evo()) {
            Ok(p) => p,
            Err(SourceError::NotAvailable(why)) => return Err(self.lost(why)),
            Err(e) => return Err(e),
        };
        if pages.graphics.status == status::OFF {
            return Err(self.lost("session quittée"));
        }
        let now = Instant::now();
        let ids = (pages.physics.packet_id, pages.graphics.packet_id);
        if self.last_ids == Some(ids) {
            let limit = if pages.graphics.status == status::PAUSE {
                self.cfg.paused_timeout
            } else {
                self.cfg.stall_timeout
            };
            if now.duration_since(self.last_change) > limit {
                return Err(self.lost("plus rien ne bouge : le jeu a été fermé ?"));
            }
            return Ok(None);
        }
        self.last_ids = Some(ids);
        self.last_change = now;
        let min_gap = Duration::from_secs_f32(1.0 / NATIVE_RATE_HZ);
        if self
            .last_frame
            .is_some_and(|t| now.duration_since(t) < min_gap)
        {
            return Ok(None);
        }
        self.last_frame = Some(now);
        Ok(Some(self.conv.frame(
            &pages,
            now.duration_since(self.origin),
            SOURCE_ID,
        )))
    }

    fn poll_interval(&self) -> Duration {
        Duration::from_millis(2)
    }

    fn disconnect(&mut self) {
        self.reader = None;
    }
}
