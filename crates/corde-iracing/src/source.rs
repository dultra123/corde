//! [`IracingSource`] — the `corde_core::TelemetrySource` implementation for iRacing.
//!
//! * `connect` opens the mapping of the game and checks that a session is running: the
//!   game leaves its mapping behind when it closes, with the « connected » bit cleared,
//!   and the source then reports itself not available — which lets Corde look at the
//!   other games meanwhile. A game that crashed may leave the bit set, and another program
//!   that reads the same mapping keeps it alive: `connect` therefore also waits a moment
//!   and checks that the game is still writing.
//! * `poll` copies the newest row; a new frame is made each time the game wrote one
//!   (60 times a second). The session text is read again when the game says it changed,
//!   and the table of the variables when the game moved it (another car).
//! * The game left (the bit went off, the mapping is gone, or no row was written for
//!   [`IracingSourceConfig::stall_timeout`]): not available, and the mapping is closed.

use crate::convert::Converter;
use crate::layout::Header;
use crate::reader::{self, open_default_map, RawMap};
use crate::session::SessionDesc;
use crate::vars::VarTable;
use corde_core::{SourceError, SourceInfo, TelemetrySource};
use corde_protocol::{GameId, TelemetryFrame};
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Rate at which the game writes its rows (Hz).
pub const NATIVE_RATE_HZ: f32 = 60.0;
/// Identifier put in `TelemetryFrame::source`.
pub const SOURCE_ID: &str = "iracing-shm";
pub const LABEL: &str = "iRacing";

#[derive(Debug, Clone)]
pub struct IracingSourceConfig {
    /// Read an image of the mapping in this file instead of the game's mapping (tests,
    /// development).
    pub file: Option<PathBuf>,
    /// How long no row may be written before the game is taken for gone. Default 5 s.
    pub stall_timeout: Duration,
    /// How long `connect` waits to see the game write a new row (it writes one every
    /// 17 ms). Zero: no check. Default 100 ms.
    pub connect_wait: Duration,
}

impl Default for IracingSourceConfig {
    fn default() -> Self {
        Self {
            file: None,
            stall_timeout: Duration::from_secs(5),
            connect_wait: Duration::from_millis(100),
        }
    }
}

/// What was read once and is kept while the game does not change it.
struct Live {
    map: Box<dyn RawMap>,
    table: VarTable,
    /// Where the table was and how large, when it was read.
    table_at: (i32, i32, i32),
    desc: SessionDesc,
    info_update: i32,
}

fn table_place(h: &Header) -> (i32, i32, i32) {
    (h.var_header_offset, h.num_vars, h.buf_len)
}

pub struct IracingSource {
    cfg: IracingSourceConfig,
    live: Option<Live>,
    conv: Converter,
    last_tick: Option<i32>,
    last_change: Instant,
}

impl IracingSource {
    pub fn new() -> Self {
        Self::with_config(IracingSourceConfig::default())
    }

    pub fn with_config(cfg: IracingSourceConfig) -> Self {
        Self {
            cfg,
            live: None,
            conv: Converter::new(),
            last_tick: None,
            last_change: Instant::now(),
        }
    }

    fn lost(&mut self, why: impl Into<String>) -> SourceError {
        self.live = None;
        SourceError::NotAvailable(why.into())
    }
}

impl Default for IracingSource {
    fn default() -> Self {
        Self::new()
    }
}

impl TelemetrySource for IracingSource {
    fn info(&self) -> SourceInfo {
        SourceInfo {
            game: GameId::Iracing,
            id: SOURCE_ID,
            label: LABEL.into(),
            native_rate_hz: NATIVE_RATE_HZ,
        }
    }

    fn connect(&mut self) -> Result<(), SourceError> {
        self.live = None;
        let mut map = open_default_map(self.cfg.file.as_deref())?;
        let h = reader::header(map.as_mut())?;
        reader::check_version(&h)?;
        if !h.connected() {
            return Err(SourceError::NotAvailable(format!(
                "{LABEL} n'a pas de session en cours"
            )));
        }
        if !self.cfg.connect_wait.is_zero() {
            // Still writing? (What a crashed game leaves behind says « connected » too.)
            let before = reader::latest_row(map.as_mut())?.tick;
            let deadline = Instant::now() + self.cfg.connect_wait;
            loop {
                std::thread::sleep(Duration::from_millis(10));
                if reader::latest_row(map.as_mut())?.tick != before {
                    break;
                }
                if Instant::now() >= deadline {
                    return Err(SourceError::NotAvailable(format!(
                        "{LABEL} ne publie plus rien (jeu fermé ?)"
                    )));
                }
            }
        }
        let table = reader::var_table(map.as_mut(), &h)?;
        if !table.has("SessionTime") {
            return Err(SourceError::NotAvailable(format!(
                "{LABEL} : table des variables incomplète (le jeu démarre ?)"
            )));
        }
        let desc = SessionDesc::parse(&reader::session_text(map.as_mut(), &h)?);
        tracing::info!(
            "{LABEL} : {} ({} variables, {} pilotes)",
            map.describe(),
            table.len(),
            desc.drivers.len()
        );
        self.live = Some(Live {
            map,
            table,
            table_at: table_place(&h),
            desc,
            info_update: h.session_info_update,
        });
        self.conv = Converter::new();
        self.last_tick = None;
        self.last_change = Instant::now();
        Ok(())
    }

    fn poll(&mut self) -> Result<Option<TelemetryFrame>, SourceError> {
        let Some(live) = self.live.as_mut() else {
            return Err(SourceError::NotAvailable("non connecté".into()));
        };
        let tick = match reader::latest_row(live.map.as_mut()) {
            Ok(t) => t,
            Err(SourceError::NotAvailable(why)) => return Err(self.lost(why)),
            Err(e) => return Err(e),
        };
        if !tick.header.connected() {
            return Err(self.lost("session quittée"));
        }
        let now = Instant::now();
        if self.last_tick == Some(tick.tick) {
            if now.duration_since(self.last_change) > self.cfg.stall_timeout {
                return Err(self.lost("plus rien ne bouge : le jeu a été fermé ?"));
            }
            return Ok(None);
        }
        self.last_tick = Some(tick.tick);
        self.last_change = now;
        // Another car: the game publishes other variables, elsewhere.
        if table_place(&tick.header) != live.table_at {
            live.table = reader::var_table(live.map.as_mut(), &tick.header)?;
            live.table_at = table_place(&tick.header);
        }
        // The session text changed (a driver joined, another session of the event).
        if tick.header.session_info_update != live.info_update {
            let text = reader::session_text(live.map.as_mut(), &tick.header)?;
            live.desc = SessionDesc::parse(&text);
            live.info_update = tick.header.session_info_update;
        }
        Ok(Some(self.conv.frame(
            &live.desc,
            live.table.row(&tick.row),
            SOURCE_ID,
        )))
    }

    fn poll_interval(&self) -> Duration {
        Duration::from_millis(4)
    }

    fn disconnect(&mut self) {
        self.live = None;
    }
}
