//! The adapter contract.

use corde_protocol::{GameId, TelemetryFrame};
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SourceError {
    /// The game is not running / the shared memory does not exist yet.
    /// The runner keeps retrying with a back-off; this is not fatal.
    #[error("game not available: {0}")]
    NotAvailable(String),
    /// The data was read but is unusable (bad size, inconsistent snapshot…).
    #[error("invalid data: {0}")]
    Invalid(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Other(String),
}

/// Static description of a source.
#[derive(Debug, Clone)]
pub struct SourceInfo {
    pub game: GameId,
    /// Short identifier used in [`TelemetryFrame::source`] ("lmu-shm", "sim"…).
    pub id: &'static str,
    /// Human readable label.
    pub label: String,
    /// Rate at which the game updates its data; drives the poll interval.
    pub native_rate_hz: f32,
}

/// A pollable telemetry source.
///
/// Implementations are **synchronous** and must be cheap to poll (a memcpy and a
/// conversion). The server runs them on a dedicated thread at
/// `native_rate_hz` and pushes the frames to its broadcast channel.
pub trait TelemetrySource: Send {
    fn info(&self) -> SourceInfo;

    /// Try to connect to the game. Called until it succeeds, then again after
    /// [`Self::poll`] returned [`SourceError::NotAvailable`].
    fn connect(&mut self) -> Result<(), SourceError>;

    /// Read the newest snapshot. Returns `Ok(None)` when nothing new is
    /// available since the last call (the caller then sleeps briefly).
    fn poll(&mut self) -> Result<Option<TelemetryFrame>, SourceError>;

    /// Suggested sleep between two polls when `poll` returned `Ok(None)`.
    fn poll_interval(&self) -> Duration {
        let hz = self.info().native_rate_hz.max(1.0);
        Duration::from_secs_f32(1.0 / hz / 2.0)
    }

    /// Release handles. Default: nothing.
    fn disconnect(&mut self) {}

    /// The game's user data folder (where it writes results, setups…), when
    /// the source learnt it from the game. Default: unknown.
    fn user_data_dir(&self) -> Option<std::path::PathBuf> {
        None
    }
}
