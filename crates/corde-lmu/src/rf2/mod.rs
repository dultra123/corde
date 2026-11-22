//! rFactor 2, read through the shared-memory plugin shipped with the game
//! (`rFactor2SharedMemoryMapPlugin64.dll`). Experimental: written from the public
//! description of the plugin, not yet checked against the game.
//!
//! rFactor 2 and Le Mans Ultimate share an engine: the plugin publishes the very
//! structures Le Mans Ultimate inherited. The two buffers are therefore put back in the
//! shape of Le Mans Ultimate's block ([`convert::assemble`]) and read by the converter
//! that was checked in game ([`crate::convert::to_frame`]): same units, same axes, same
//! lap timing. What Le Mans Ultimate added (levels of the driver aids, hybrid energy,
//! vehicle model…) does not exist here and stays empty.
//!
//! Read-only: none of the plugin's input buffers is opened.

pub mod convert;
pub mod fake;
pub mod layout;
pub mod reader;
pub mod source;

pub use convert::Rf2Converter;
pub use reader::{open_reader, Rf2Raw, Rf2Reader};
pub use source::{Rf2Source, Rf2SourceConfig};
