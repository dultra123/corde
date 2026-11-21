//! # corde-iracing
//!
//! Adapter for iRacing. The game publishes one shared memory mapping; this crate reads it
//! **read-only** and turns it into `corde_protocol::TelemetryFrame`s. Nothing is ever
//! written to the game, nothing is sent to it (no command, no camera or pit request), and
//! nothing has to be installed or switched on in it.
//!
//! * [`layout`] — the header of the mapping and the lines of its table of variables.
//! * [`vars`] — the variables, read by name through that table.
//! * [`yaml`] — the session text of the game (lines of `Key: value`), read without ever
//!   refusing one, and its copy without names for the diagnostics.
//! * [`session`] — what Corde takes from the session text: circuit, sessions, cars,
//!   drivers.
//! * [`reader`] — the game's mapping on Windows; an image in a file for the tests.
//! * [`convert`] — a row → a frame, and the lap count waited for with its time.
//! * [`odometer`] — where the car is on the map, from its speed and heading (the game
//!   publishes no coordinates).
//! * [`source`] — [`IracingSource`], the `corde_core::TelemetrySource` implementation.
//! * [`fake`] — a plausible mapping for the tests and the development tools.
//!
//! Binary: `iracing-probe` (shows what the mapping holds; first thing to run with the
//! game).
//!
//! **Not verified against the running game yet** (written from the SDK of the game and
//! from independent open-source readers): see `docs/JEUX.md`.

pub mod convert;
pub mod fake;
pub mod layout;
pub mod odometer;
pub mod reader;
pub mod session;
pub mod source;
pub mod vars;
pub mod yaml;

pub use convert::Converter;
pub use reader::{Image, RawMap};
pub use session::SessionDesc;
pub use source::{IracingSource, IracingSourceConfig};
pub use vars::{Row, VarTable};
