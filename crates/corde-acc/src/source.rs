//! [`AccSource`] — the `corde_core::TelemetrySource` implementation for Assetto Corsa
//! Competizione, and for the first Assetto Corsa ([`AccSource::assetto_corsa`]): the two
//! games publish pages of the same names, told apart by the version of their interface.
//!
//! * `connect` opens the pages, checks that they are those of the game this source is for
//!   and that a session is running: at the menus the game is
//!   there but has nothing to say, and the source reports itself not available — which
//!   lets Corde look at the other games meanwhile.
//! * `poll` copies the pages; a new frame is made when one of the two step counters moved,
//!   at most [`NATIVE_RATE_HZ`] times a second (the game rewrites the physics page far more
//!   often).
//! * The game left (the pages are gone, the status went to « off », or nothing moved for
//!   [`AccSourceConfig::stall_timeout`] while a session was live): not available, and the
//!   mappings are closed — a handle left open would keep the pages of a dead game alive.

use crate::convert::Converter;
use crate::layout::{status, Game};
use crate::reader::{open_default_reader, PageReader};
use corde_core::{SourceError, SourceInfo, TelemetrySource};
use corde_protocol::TelemetryFrame;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Rate of the frames Corde makes from the game (Hz).
pub const NATIVE_RATE_HZ: f32 = 100.0;
/// Identifier put in `TelemetryFrame::source` for Competizione.
pub const SOURCE_ID: &str = "acc-shm";
/// The same for the first Assetto Corsa.
pub const SOURCE_ID_AC: &str = "ac-shm";

#[derive(Debug, Clone)]
pub struct AccSourceConfig {
    /// Which of the two games this source reads. Default: Competizione.
    pub game: Game,
    /// Read three files of this directory instead of the game's mappings (tests,
    /// development).
    pub dir: Option<PathBuf>,
    /// How long nothing may move while a session is live before the game is taken for
    /// gone. Default 5 s.
    pub stall_timeout: Duration,
    /// The same while the game is paused (the pages are frozen then). Default 30 min.
    pub paused_timeout: Duration,
}

impl Default for AccSourceConfig {
    fn default() -> Self {
        Self {
            game: Game::Competizione,
            dir: None,
            stall_timeout: Duration::from_secs(5),
            paused_timeout: Duration::from_secs(30 * 60),
        }
    }
}

pub struct AccSource {
    cfg: AccSourceConfig,
    reader: Option<Box<dyn PageReader>>,
    conv: Converter,
    origin: Instant,
    last_ids: Option<(i32, i32)>,
    last_change: Instant,
    last_frame: Option<Instant>,
}

impl AccSource {
    /// For Assetto Corsa Competizione.
    pub fn new() -> Self {
        Self::with_config(AccSourceConfig::default())
    }

    /// For the first Assetto Corsa.
    pub fn assetto_corsa() -> Self {
        Self::with_config(AccSourceConfig {
            game: Game::AssettoCorsa,
            ..AccSourceConfig::default()
        })
    }

    fn source_id(&self) -> &'static str {
        match self.cfg.game {
            Game::Competizione => SOURCE_ID,
            Game::AssettoCorsa => SOURCE_ID_AC,
        }
    }

    pub fn with_config(cfg: AccSourceConfig) -> Self {
        let now = Instant::now();
        Self {
            cfg,
            reader: None,
            conv: Converter::new(),
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

impl Default for AccSource {
    fn default() -> Self {
        Self::new()
    }
}

impl TelemetrySource for AccSource {
    fn info(&self) -> SourceInfo {
        SourceInfo {
            game: self.cfg.game.id(),
            id: self.source_id(),
            label: self.cfg.game.label().into(),
            native_rate_hz: NATIVE_RATE_HZ,
        }
    }

    fn connect(&mut self) -> Result<(), SourceError> {
        self.reader = None;
        let mut reader = open_default_reader(self.cfg.dir.as_deref())?;
        let raw = reader.read_raw()?;
        let game = self.cfg.game;
        match raw.game()? {
            Some(found) if found == game => {}
            Some(found) => {
                return Err(SourceError::NotAvailable(format!(
                    "ces pages sont celles de {}, pas de {}",
                    found.label(),
                    game.label()
                )))
            }
            None => {
                return Err(SourceError::NotAvailable(format!(
                    "{} : pages sans numéro d'interface (le jeu démarre ?)",
                    game.label()
                )))
            }
        }
        let pages = raw.pages_of(game)?;
        if pages.graphics.status == status::OFF {
            return Err(SourceError::NotAvailable(format!(
                "{} est lancé, sans session en cours",
                game.label()
            )));
        }
        tracing::info!("{} : {}", game.label(), reader.describe());
        self.reader = Some(reader);
        self.conv = Converter::for_game(game);
        self.last_ids = None;
        self.last_change = Instant::now();
        self.last_frame = None;
        Ok(())
    }

    fn poll(&mut self) -> Result<Option<TelemetryFrame>, SourceError> {
        let Some(reader) = self.reader.as_mut() else {
            return Err(SourceError::NotAvailable("non connecté".into()));
        };
        let game = self.cfg.game;
        let pages = match reader.read_raw().and_then(|raw| raw.pages_of(game)) {
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
        let id = self.source_id();
        Ok(Some(self.conv.frame(
            &pages,
            now.duration_since(self.origin),
            id,
        )))
    }

    fn poll_interval(&self) -> Duration {
        Duration::from_millis(2)
    }

    fn disconnect(&mut self) {
        self.reader = None;
    }
}
