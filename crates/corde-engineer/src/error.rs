//! Error type of the engineer.

#[derive(Debug, thiserror::Error)]
pub enum EngineerError {
    #[error("store: {0}")]
    Store(#[from] corde_store::StoreError),
    /// The session exists but cannot be analysed (no clean lap, no samples…).
    /// The message is in French, ready for the API.
    #[error("{0}")]
    NotEnoughData(String),
    /// The knowledge base could not be read.
    #[error("base de connaissances : {0}")]
    Knowledge(String),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}

impl EngineerError {
    pub fn is_not_found(&self) -> bool {
        matches!(self, Self::Store(e) if e.is_not_found())
    }

    pub fn is_not_enough_data(&self) -> bool {
        matches!(self, Self::NotEnoughData(_))
    }
}

pub type Result<T> = std::result::Result<T, EngineerError>;
