//! Assetto Corsa Rally.
//!
//! The game publishes pages of the same names and layout as Competizione
//! (`Local\acpmf_*`). What tells it apart, and what it does differently:
//!
//! * **Temperatures are in kelvins** (air, road, tyres, brakes, water). That is how its
//!   pages are recognised: the air of the two other games is never above 200.
//! * **The static page is written late** (zeros when a stage loads) and names no version
//!   of the game.
//! * **`Graphics::status` stays at zero**, and the clock is only written as a text.
//! * **No laps**: a stage is driven from a start line to a finish line. The game's clock
//!   reads zero on the start line, runs from the moment the line is crossed, and freezes
//!   on the finish line while the car rolls on into the stop zone. A stage restarted puts
//!   the clock back to zero.
//! * **The physics page is blank** (all zeros but its counter) while the game is not
//!   simulating the car: a pause, the stop zone, a replay.
//!
//! Corde's model is made of laps: [`convert`] turns each run of a stage into one lap,
//! timed by the game's clock.
//!
//! * [`convert`] — the pages → a frame; the runs.
//! * [`source`] — [`RallySource`], the `corde_core::TelemetrySource` implementation.
//! * [`fake`] — plausible pages for the tests and the development tools.
//!
//! First written from what two independent open-source readers observed of the game, then
//! corrected (1.14.1) from pages copied from the running game, driving and paused: the
//! game fills less of its pages than they have room for — no status, the clock as a text
//! only, a static page with the car, the stage and its length (see [`convert::normalised`]
//! and ADR 0020 § H). **Still to be seen in the game**: the start and the finish of a run,
//! and what the pages hold in the menus. See `docs/JEUX.md`.

pub mod convert;
pub mod fake;
pub mod source;

pub use convert::RallyConverter;
pub use source::{RallySource, RallySourceConfig};
