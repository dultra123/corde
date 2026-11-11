//! # corde-lmu
//!
//! Le Mans Ultimate adapter for Corde:
//!
//! * [`layout`] — byte-exact Rust mirror of the native `LMU_Data` shared memory
//!   block (Studio 397 interface, LMU ≥ 1.2), with size/offset tests
//!   (`tests/layout_offsets.rs`, generated from `reference/lmu-shared-memory-layout.txt`).
//! * [`reader`] — raw block readers: Win32 file mapping (events + cooperative
//!   lock) on Windows, `/dev/shm/LMU_Data` mirror on Linux (torn-read check).
//! * [`convert`] — `SharedMemoryObjectOut` → `corde_protocol::TelemetryFrame`.
//! * [`source`] — [`LmuSource`], the `corde_core::TelemetrySource` implementation.
//! * [`settings`] — locating the game folder, `UserData`, and the `WebUI port`.
//! * [`results`] — parser for the `UserData/Log/Results/*.xml` files the game
//!   writes after every session (classification, laps, incidents).
//! * [`rest`] — minimal synchronous GET-only client for the local REST API.
//! * [`svm`] — parser / writer / diff for `.svm` setup files.
//! * [`svm_library`] — the driver's setups (`UserData/player/Settings/*/*.svm`),
//!   matching a session's car and track, choosing the base setup.
//! * [`garage`] — tolerant extraction of the setup bounds from the garage data
//!   of the local REST API.
//! * [`fake`] — plausible synthetic blocks for tests and the `lmu-shm-fake` tool.
//!
//! Binaries: `lmu-probe` (first thing to run on the gaming PC) and
//! `lmu-shm-fake` (Linux: feeds `/dev/shm/LMU_Data` without the game).
//!
//! Everything marked `TODO(verify-on-pc)` is an assumption that still has to be
//! confirmed against the real game on a Windows PC.

pub mod convert;
pub mod fake;
pub mod garage;
pub mod layout;
pub mod reader;
pub mod rest;
pub mod results;
pub mod settings;
pub mod source;
pub mod svm;
pub mod svm_library;

pub use convert::to_frame;
pub use layout::{SharedMemoryObjectOut, TelemInfoV01, VehicleScoringInfoV01};
pub use rest::{RestClient, RestError};
pub use results::{RaceResultsFile, ResultDriver, ResultLap};
pub use settings::{LmuPaths, DEFAULT_WEBUI_PORT};
pub use source::{LmuSource, LmuSourceConfig, LmuStatus};
pub use svm::{is_adjustable, Setup, SetupDiff, SetupValue};
