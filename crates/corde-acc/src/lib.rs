//! # corde-acc
//!
//! Adapter for Assetto Corsa Competizione and for the first Assetto Corsa. Each game
//! publishes three shared memory pages (physics, graphics, static) under the same names,
//! in two versions of one layout; this crate reads them **read-only** and turns them into
//! `corde_protocol::TelemetryFrame`s. Nothing is ever written to the game, and nothing has
//! to be installed or switched on in it.
//!
//! * [`layout`] — byte-exact mirror of the three pages, with size and offset tests.
//! * [`reader`] — the game's named mappings on Windows; three files of a directory for the
//!   tests.
//! * [`convert`] — the pages → a frame, and what the game does not publish (session clock,
//!   lap distance).
//! * [`tracks`] — lengths of the circuits of the game.
//! * [`source`] — [`AccSource`], the `corde_core::TelemetrySource` implementation.
//! * [`fake`] — plausible pages for the tests and the development tools.
//! * [`evo`] — Assetto Corsa EVO: pages of its own names, a physics page of the same
//!   layout, new graphics and static pages.
//! * [`rally`] — Assetto Corsa Rally: Competizione's pages, temperatures in kelvins,
//!   stages instead of laps.
//!
//! Binary: `acc-probe` (shows what the pages hold; first thing to run with the game).
//!
//! **Not verified against the running game yet** (written from the documentation of the
//! game and two independent open-source readers): see `docs/JEUX.md`.

pub mod convert;
pub mod evo;
pub mod fake;
pub mod layout;
pub mod rally;
pub mod reader;
pub mod source;
pub mod tracks;

pub use convert::Converter;
pub use evo::{EvoSource, EvoSourceConfig};
pub use layout::{Game, Graphics, GraphicsAc, Physics, Static};
pub use rally::{RallySource, RallySourceConfig};
pub use reader::{PageReader, Pages, RawPages};
pub use source::{AccSource, AccSourceConfig};
