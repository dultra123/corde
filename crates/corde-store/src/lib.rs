//! # corde-store
//!
//! Session recording for Corde.
//!
//! ```text
//! <data-dir>/
//! ├─ corde.db                       SQLite (WAL): sessions, laps, events, setups, sample_files,
//! │                                 results, result_laps (imported XML results), analyses (cached reports),
//! │                                 feedback (what the driver felt)
//! └─ sessions/
//!    └─ 20261103-213000-paul-ricard-1a-race/
//!       ├─ samples-000.parquet      ≈ 10 min or 50 000 rows each, zstd, row groups of 4096
//!       └─ samples-001.parquet
//! ```
//!
//! * [`channels`] — the fixed Parquet column set ([`channels::CHANNELS`]).
//! * [`samples`] — [`SampleWriter`] (buffered Arrow builders → Parquet, file
//!   rotation, crash-safe checkpoints) and column readers.
//! * [`db`] — schema, migrations and typed rows.
//! * [`recorder`] — the [`Recorder`] state machine fed by the server's source thread.
//! * [`query`] — read API used by `/api/sessions*` and the CLI.
//! * [`decimate`] — min/max-preserving decimation for graphs.
//! * [`results`] — results files imported from the game (`results` /
//!   `result_laps`, schema v2) and the driver profile built from them.
//! * [`sync`] — what a PC sends its home server (`corde-server --role hub`, schema v7).

pub mod channels;
pub mod db;
pub mod decimate;
pub mod error;
pub mod legacy;
pub mod migrate;
pub mod query;
pub mod reconcile;
pub mod recorder;
pub mod results;
pub mod samples;
pub mod session_id;
pub mod store;
pub mod sync;

pub use channels::{Channel, ChannelKind, CHANNELS};
pub use db::{
    AnalysisRow, Db, EventRow, LapRow, SampleFileRow, SessionFilter, SessionRow, SetupRow,
};
pub use error::{Result, StoreError};
pub use migrate::{default_data_dir, prepare_default_data_dir};
pub use query::{ChannelSeries, LapSamples, SessionDetail, StorageStats};
pub use reconcile::ReconcileReport;
pub use recorder::{Recorder, RecorderConfig, RecorderHandle, RecordingStatus};
pub use results::{
    ImportedFile, NewResult, ProfileSummary, ProfileTotals, ResultDetail, ResultFilter,
    ResultLapRow, ResultRow, TrackSummary, VehicleSummary,
};
pub use samples::{FileInfo, SampleWriter, WriterConfig};
pub use session_id::{make_session_id, valid_id};
pub use store::Store;
pub use sync::{
    FeedbackItem, FeedbackSet, FeedbackUids, IterationItem, MergeReport, ResultBundle,
    SessionBundle,
};
