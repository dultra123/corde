//! [`RallySource`] — the `corde_core::TelemetrySource` implementation for Assetto Corsa
//! Rally.
//!
//! The game publishes pages of the names of Competizione's: what tells them apart is in
//! the physics page (temperatures in kelvins, see [`super::convert::is_rally`]).
//!
//! * `connect` opens the pages and checks that a stage is loaded (in the menus the
//!   graphics page is all zeros) and that the physics page is this game's. A blank
//!   physics page (a pause, a replay, the stop zone) does not say whose it is: not
//!   available, and looked at again a moment later.
//! * `poll` copies the pages; a new frame is made when one of the two step counters moved,
//!   at most [`NATIVE_RATE_HZ`] times a second.
//! * The game left (the pages are gone, the graphics page went back to zeros, or nothing
//!   moved for [`RallySourceConfig::stall_timeout`]): not available, and the mappings are
//!   closed.

use super::convert::{is_rally, RallyConverter};
use crate::layout::status;
use crate::reader::{open_default_reader, PageReader};
use corde_core::{SourceError, SourceInfo, TelemetrySource};
use corde_protocol::{GameId, TelemetryFrame};
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Rate of the frames Corde makes from the game (Hz).
pub const NATIVE_RATE_HZ: f32 = 100.0;
/// Identifier put in `TelemetryFrame::source`.
pub const SOURCE_ID: &str = "acrally-shm";
const LABEL: &str = "Assetto Corsa Rally";

#[derive(Debug, Clone)]
pub struct RallySourceConfig {
    /// Read three files of this directory instead of the game's mappings (tests,
    /// development).
    pub dir: Option<PathBuf>,
    /// How long nothing may move before the game is taken for gone. Default 5 s (the
    /// counter of the physics page keeps running through a pause).
    pub stall_timeout: Duration,
}

impl Default for RallySourceConfig {
    fn default() -> Self {
        Self {
            dir: None,
            stall_timeout: Duration::from_secs(5),
        }
    }
}

pub struct RallySource {
    cfg: RallySourceConfig,
    reader: Option<Box<dyn PageReader>>,
    conv: RallyConverter,
    origin: Instant,
    last_ids: Option<(i32, i32)>,
    last_change: Instant,
    last_frame: Option<Instant>,
}

impl RallySource {
    pub fn new() -> Self {
        Self::with_config(RallySourceConfig::default())
    }

    pub fn with_config(cfg: RallySourceConfig) -> Self {
        let now = Instant::now();
        Self {
            cfg,
            reader: None,
            conv: RallyConverter::new(),
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

impl Default for RallySource {
    fn default() -> Self {
        Self::new()
    }
}

impl TelemetrySource for RallySource {
    fn info(&self) -> SourceInfo {
        SourceInfo {
            game: GameId::AcRally,
            id: SOURCE_ID,
            label: LABEL.into(),
            native_rate_hz: NATIVE_RATE_HZ,
        }
    }

    fn connect(&mut self) -> Result<(), SourceError> {
        self.reader = None;
        let mut reader = open_default_reader(self.cfg.dir.as_deref())?;
        let pages = reader.read()?;
        if pages.graphics.status == status::OFF {
            return Err(SourceError::NotAvailable(format!(
                "{LABEL} : pas d'épreuve en cours"
            )));
        }
        match is_rally(&pages.physics) {
            Some(true) => {}
            Some(false) => {
                return Err(SourceError::NotAvailable(format!(
                    "ces pages ne sont pas celles de {LABEL} (températures en °C)"
                )))
            }
            None => {
                return Err(SourceError::NotAvailable(format!(
                    "{LABEL} : page physique vide (pause, replay ou zone d'arrêt)"
                )))
            }
        }
        tracing::info!("{LABEL} : {}", reader.describe());
        self.reader = Some(reader);
        self.conv = RallyConverter::new();
        self.last_ids = None;
        self.last_change = Instant::now();
        self.last_frame = None;
        Ok(())
    }

    fn poll(&mut self) -> Result<Option<TelemetryFrame>, SourceError> {
        let Some(reader) = self.reader.as_mut() else {
            return Err(SourceError::NotAvailable("non connecté".into()));
        };
        let pages = match reader.read() {
            Ok(p) => p,
            Err(SourceError::NotAvailable(why)) => return Err(self.lost(why)),
            Err(e) => return Err(e),
        };
        if pages.graphics.status == status::OFF {
            return Err(self.lost("épreuve quittée"));
        }
        if is_rally(&pages.physics) == Some(false) {
            return Err(self.lost("un autre jeu a pris la place"));
        }
        let now = Instant::now();
        let ids = (pages.physics.packet_id, pages.graphics.packet_id);
        if self.last_ids == Some(ids) {
            if now.duration_since(self.last_change) > self.cfg.stall_timeout {
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
