//! Assetto Corsa EVO.
//!
//! The game publishes three pages of its own (`Local\acevo_pmf_physics`, `…_graphics`,
//! `…_static`): the physics page has the layout of Competizione's, the two others are
//! new. Read-only, like the rest of this crate; nothing has to be switched on in the game.
//!
//! * [`layout`] — the graphics and static pages, with size and offset tests.
//! * [`convert`] — the pages → a frame.
//! * [`source`] — [`EvoSource`], the `corde_core::TelemetrySource` implementation.
//! * [`fake`] — plausible pages for the tests and the development tools.
//!
//! **Not verified against the running game yet**: see `docs/JEUX.md`.

pub mod convert;
pub mod fake;
pub mod layout;
pub mod source;

pub use convert::EvoConverter;
pub use layout::{GraphicsEvo, StaticEvo};
pub use source::{EvoSource, EvoSourceConfig};
