//! [`Store`]: the shared handle on a data directory — paths, the query
//! connection, and the "session being recorded" lock.

use crate::db::Db;
use crate::error::Result;
use crate::migrate::migrate_db_file;
use crate::session_id::valid_id;
use crate::StoreError;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

/// Sub-directory of the data directory holding one directory per session.
pub const SESSIONS_DIR: &str = "sessions";

pub struct Store {
    data_dir: PathBuf,
    /// `<data_dir>/corde.db`, or the former name when it could not be renamed.
    db_path: PathBuf,
    db: Mutex<Db>,
    /// Session currently being written by the recorder (delete is refused).
    active: Mutex<Option<String>>,
}

impl std::fmt::Debug for Store {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Store")
            .field("data_dir", &self.data_dir)
            .finish()
    }
}

impl Store {
    /// Open `<data_dir>/corde.db` (migrating it), create `<data_dir>/sessions`,
    /// and repair sessions left open by a crash. A database of the former name
    /// is renamed first ([`crate::migrate::migrate_db_file`]), or opened under
    /// that name when the rename fails.
    pub fn open(data_dir: &Path) -> Result<Arc<Self>> {
        std::fs::create_dir_all(data_dir.join(SESSIONS_DIR))?;
        let db_file = migrate_db_file(data_dir);
        db_file.log(data_dir);
        let db = Db::open(&db_file.path)?;
        let repaired = db.recover_open_sessions(Some(&data_dir.join(SESSIONS_DIR)))?;
        if !repaired.is_empty() {
            tracing::warn!(
                "{} session(s) repaired after an unclean stop",
                repaired.len()
            );
        }
        Ok(Arc::new(Self {
            data_dir: data_dir.to_path_buf(),
            db_path: db_file.path,
            db: Mutex::new(db),
            active: Mutex::new(None),
        }))
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn db_path(&self) -> PathBuf {
        self.db_path.clone()
    }

    pub fn sessions_dir(&self) -> PathBuf {
        self.data_dir.join(SESSIONS_DIR)
    }

    /// Directory of one session's sample files (the id is validated first).
    pub fn session_dir(&self, id: &str) -> Result<PathBuf> {
        if !valid_id(id) {
            return Err(StoreError::invalid("session id", id));
        }
        Ok(self.sessions_dir().join(id))
    }

    /// A fresh connection to the same database, for another thread (the recorder).
    pub fn open_db(&self) -> Result<Db> {
        Db::open(&self.db_path())
    }

    /// The query connection.
    pub fn db(&self) -> MutexGuard<'_, Db> {
        match self.db.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    /// Mark / unmark the session being recorded.
    pub fn set_active(&self, id: Option<String>) {
        if let Ok(mut g) = self.active.lock() {
            *g = id;
        }
    }

    pub fn active(&self) -> Option<String> {
        self.active.lock().ok().and_then(|g| g.clone())
    }

    pub fn is_active(&self, id: &str) -> bool {
        self.active().as_deref() == Some(id)
    }
}
