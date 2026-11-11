//! # corde-core
//!
//! Game-independent building blocks:
//!
//! * [`TelemetrySource`] — the trait every game adapter implements (LMU shared
//!   memory, the built-in simulator, a replay file…). The server owns one source
//!   at a time and polls it on a dedicated thread.
//! * [`LapTracker`] — turns the stream of frames into completed [`LapRecord`]s
//!   and lap-boundary events.
//! * [`derive`] — physics helpers shared by adapters (slip ratio, slip angle, g's).
//!
//! Nothing here knows about LMU specifics; those live in `corde-lmu`.

pub mod derive;
pub mod lap_tracker;
pub mod source;

pub use lap_tracker::LapTracker;
pub use source::{SourceError, SourceInfo, TelemetrySource};
