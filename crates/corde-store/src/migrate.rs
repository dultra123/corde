//! Takes over the data of the application's former name.
//!
//! Until 0.3.0 the application had a working name, hence another data folder
//! and database name (the `LEGACY_*` constants below; legacy name (Muret, before 0.3.0)).
//! This module moves both to the new names **before anything is opened**, and
//! never leaves the driver on an empty folder while the old one holds their
//! sessions:
//!
//! | Situation | What happens |
//! |---|---|
//! | only the old folder exists | renamed to the new name (one atomic move) |
//! | the move fails (files locked: the old application still runs) | the old folder is used as is for this run; the move is tried again next start |
//! | both exist | the new one is used, the old one is reported and left untouched (nothing merged or deleted) |
//! | both exist, the new one is **empty** and the old one holds a database | the empty new folder is removed, then the old one is moved as above |
//! | both exist, the new one holds no database but is not empty, the old one holds one | the old folder is used for this run (merging is never attempted) |
//! | neither / only the new one | nothing to do |
//!
//! Inside the chosen folder, [`LEGACY_DB_FILE`] becomes `corde.db` together with its
//! `-wal`, `-shm` (and `-journal`) side files, before SQLite opens it: the
//! side files are renamed first and everything is put back if one rename
//! fails, in which case the old name is opened. Every other name (`sessions/`,
//! `layouts/`, `settings.json`, `diagnostics/`, `logs/`) is unchanged.
//!
//! The functions only touch the file system and return what they did; the
//! caller logs it ([`DataDirMigration::log`], [`DbFileMigration::log`]) once
//! its logger is up (the desktop shell keeps its log *inside* the data folder,
//! so it can only open it after the folder is settled).

use crate::db::DB_FILE;
use std::io;
use std::path::{Path, PathBuf};

/// Name of the data folder under the per-user local data folder (Windows).
pub const DATA_DIR_NAME_WINDOWS: &str = "Corde";
/// Name of the data folder under the per-user local data folder (elsewhere).
pub const DATA_DIR_NAME: &str = "corde";
/// legacy name (Muret, before 0.3.0): data folder on Windows.
pub const LEGACY_DATA_DIR_NAME_WINDOWS: &str = "Muret";
/// legacy name (Muret, before 0.3.0): data folder elsewhere.
pub const LEGACY_DATA_DIR_NAME: &str = "muret";
/// legacy name (Muret, before 0.3.0): database file.
pub const LEGACY_DB_FILE: &str = "muret.db";

/// SQLite side files renamed together with the database (WAL, shared memory,
/// rollback journal). Renamed *before* the main file.
const DB_SIDE_SUFFIXES: [&str; 3] = ["-wal", "-shm", "-journal"];

fn local_data_base() -> PathBuf {
    directories::BaseDirs::new()
        .map(|b| b.data_local_dir().to_path_buf())
        .unwrap_or_else(std::env::temp_dir)
}

/// Per-user local data folder: `%LOCALAPPDATA%\Corde` on Windows,
/// `~/.local/share/corde` elsewhere (the temp dir as a last resort). Pure: see
/// [`prepare_default_data_dir`] for the folder to actually use.
pub fn default_data_dir() -> PathBuf {
    let name = if cfg!(windows) {
        DATA_DIR_NAME_WINDOWS
    } else {
        DATA_DIR_NAME
    };
    local_data_base().join(name)
}

/// The data folder of the former name, same parent as [`default_data_dir`].
/// legacy name (Muret, before 0.3.0)
pub fn legacy_data_dir() -> PathBuf {
    let name = if cfg!(windows) {
        LEGACY_DATA_DIR_NAME_WINDOWS
    } else {
        LEGACY_DATA_DIR_NAME
    };
    local_data_base().join(name)
}

/// What [`migrate_data_dir`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataDirOutcome {
    /// Nothing to take over (no old folder, or only the new one).
    Nothing,
    /// The old folder was renamed to the new name.
    Moved,
    /// The empty new folder was removed, then the old folder renamed to it.
    MovedOverEmpty,
    /// The move failed: the old folder is used as is for this run.
    MoveFailed { error: String },
    /// Both exist: the new one is used, the old one left untouched.
    BothExist,
    /// Both exist, the new one holds no database but is not empty while the
    /// old one holds one: the old one is used for this run.
    NewHasNoData,
}

/// Result of [`migrate_data_dir`]: the folder to use and how it was chosen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataDirMigration {
    /// The folder to open for this run.
    pub dir: PathBuf,
    /// The folder under the new name.
    pub new: PathBuf,
    /// The folder under the old name (legacy name: Muret, before 0.3.0).
    pub legacy: PathBuf,
    pub outcome: DataDirOutcome,
}

impl DataDirMigration {
    /// `true` when this run uses the old folder (the migration is not done).
    pub fn uses_legacy(&self) -> bool {
        self.dir == self.legacy && self.dir != self.new
    }

    /// One log line per outcome worth telling (nothing for [`DataDirOutcome::Nothing`]).
    pub fn log(&self) {
        let (new, legacy) = (self.new.display(), self.legacy.display());
        match &self.outcome {
            DataDirOutcome::Nothing => {}
            DataDirOutcome::Moved | DataDirOutcome::MovedOverEmpty => tracing::info!(
                "data folder of the former name moved: {legacy} -> {new} (sessions, layouts and settings kept)"
            ),
            DataDirOutcome::MoveFailed { error } => tracing::warn!(
                "could not move the data folder of the former name {legacy} to {new} ({error}); \
                 the former application may still be running. Using {legacy} as is for this run; \
                 the move will be tried again at the next start"
            ),
            DataDirOutcome::BothExist => tracing::warn!(
                "data folder of the former name still present: {legacy} (left untouched, not merged); \
                 using {new}"
            ),
            DataDirOutcome::NewHasNoData => tracing::warn!(
                "{new} exists but holds no database while {legacy} (former name) does: \
                 using {legacy} for this run. Move or remove {new} to finish the migration"
            ),
        }
    }
}

fn holds_database(dir: &Path) -> bool {
    dir.join(DB_FILE).is_file() || dir.join(LEGACY_DB_FILE).is_file()
}

fn is_empty_dir(dir: &Path) -> bool {
    std::fs::read_dir(dir)
        .map(|mut entries| entries.next().is_none())
        .unwrap_or(false)
}

/// Settle the data folder: see the module documentation. `new` and `legacy`
/// are full paths (the default ones come from [`default_data_dir`] and
/// [`legacy_data_dir`]).
pub fn migrate_data_dir(new: &Path, legacy: &Path) -> DataDirMigration {
    migrate_data_dir_with(new, legacy, |from, to| std::fs::rename(from, to))
}

/// [`migrate_data_dir`] with the rename injected (tests simulate a locked folder).
pub fn migrate_data_dir_with(
    new: &Path,
    legacy: &Path,
    rename: impl Fn(&Path, &Path) -> io::Result<()>,
) -> DataDirMigration {
    let result = |dir: &Path, outcome| DataDirMigration {
        dir: dir.to_path_buf(),
        new: new.to_path_buf(),
        legacy: legacy.to_path_buf(),
        outcome,
    };
    // `exists()` follows links: a link named like the old folder counts as it.
    if !legacy.is_dir() {
        return result(new, DataDirOutcome::Nothing);
    }
    let mut over_empty = false;
    if new.exists() {
        if holds_database(new) || !holds_database(legacy) {
            return result(new, DataDirOutcome::BothExist);
        }
        // The new folder has no database, the old one has: never start on it.
        // An empty one (left by an earlier attempt, say) is not data: remove
        // it and move as usual. Anything else is never merged.
        if is_empty_dir(new) && std::fs::remove_dir(new).is_ok() {
            over_empty = true;
        } else {
            return result(legacy, DataDirOutcome::NewHasNoData);
        }
    }
    if let Some(parent) = new.parent() {
        // Normally the parent of the old folder too; harmless when it exists.
        let _ = std::fs::create_dir_all(parent);
    }
    match rename(legacy, new) {
        Ok(()) if over_empty => result(new, DataDirOutcome::MovedOverEmpty),
        Ok(()) => result(new, DataDirOutcome::Moved),
        Err(e) => result(
            legacy,
            DataDirOutcome::MoveFailed {
                error: e.to_string(),
            },
        ),
    }
}

/// [`migrate_data_dir`] on the default folders: what `corde-server`,
/// `corde-store` and the desktop shell call when no data folder is given.
pub fn prepare_default_data_dir() -> DataDirMigration {
    migrate_data_dir(&default_data_dir(), &legacy_data_dir())
}

/// What [`migrate_db_file`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DbFileOutcome {
    /// Nothing to take over.
    Nothing,
    /// The old database (and its side files) now has the new name.
    Renamed,
    /// A rename failed and everything was put back: the old name is opened.
    RenameFailed { error: String },
    /// Both exist: the new one is opened, the old one left untouched.
    BothExist,
}

/// Result of [`migrate_db_file`]: the database file to open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DbFileMigration {
    pub path: PathBuf,
    pub outcome: DbFileOutcome,
}

impl DbFileMigration {
    pub fn log(&self, dir: &Path) {
        let legacy = dir.join(LEGACY_DB_FILE);
        match &self.outcome {
            DbFileOutcome::Nothing => {}
            DbFileOutcome::Renamed => tracing::info!(
                "database of the former name renamed: {} -> {}",
                legacy.display(),
                self.path.display()
            ),
            DbFileOutcome::RenameFailed { error } => tracing::warn!(
                "could not rename {} ({error}); opening it under its former name for this run",
                legacy.display()
            ),
            DbFileOutcome::BothExist => tracing::warn!(
                "database of the former name still present: {} (left untouched); using {}",
                legacy.display(),
                self.path.display()
            ),
        }
    }
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut s = path.as_os_str().to_os_string();
    s.push(suffix);
    PathBuf::from(s)
}

/// Rename `<dir>/`[`LEGACY_DB_FILE`] and its side files to `<dir>/corde.db`
/// before it is opened. Returns the file to open.
pub fn migrate_db_file(dir: &Path) -> DbFileMigration {
    migrate_db_file_with(dir, |from, to| std::fs::rename(from, to))
}

/// [`migrate_db_file`] with the rename injected (tests simulate a locked file).
pub fn migrate_db_file_with(
    dir: &Path,
    rename: impl Fn(&Path, &Path) -> io::Result<()>,
) -> DbFileMigration {
    let new = dir.join(DB_FILE);
    let legacy = dir.join(LEGACY_DB_FILE);
    if !legacy.is_file() {
        return DbFileMigration {
            path: new,
            outcome: DbFileOutcome::Nothing,
        };
    }
    if new.exists() {
        return DbFileMigration {
            path: new,
            outcome: DbFileOutcome::BothExist,
        };
    }
    // Side files first, the main file last: until the main file is renamed,
    // a failure leaves the old database openable once the side files are back.
    let mut moves: Vec<(PathBuf, PathBuf)> = DB_SIDE_SUFFIXES
        .iter()
        .map(|s| (with_suffix(&legacy, s), with_suffix(&new, s)))
        .filter(|(from, _)| from.exists())
        .collect();
    moves.push((legacy.clone(), new.clone()));
    let mut done: Vec<&(PathBuf, PathBuf)> = Vec::new();
    for m in &moves {
        if let Err(e) = rename(&m.0, &m.1) {
            for (from, to) in done.iter().rev() {
                if let Err(back) = rename(to, from) {
                    tracing::error!(
                        "could not put {} back to {}: {back}",
                        to.display(),
                        from.display()
                    );
                }
            }
            return DbFileMigration {
                path: legacy,
                outcome: DbFileOutcome::RenameFailed {
                    error: format!("{}: {e}", m.0.display()),
                },
            };
        }
        done.push(m);
    }
    DbFileMigration {
        path: new,
        outcome: DbFileOutcome::Renamed,
    }
}

/// Prefix of the application's environment variables.
pub const ENV_PREFIX: &str = "CORDE_";
/// legacy name (Muret, before 0.3.0): prefix of the environment variables, still read as a fallback.
pub const LEGACY_ENV_PREFIX: &str = "MURET_";

/// The variable of the former name standing for `name` (`CORDE_LOG` → its
/// legacy twin), `None` when `name` does not start with [`ENV_PREFIX`].
pub fn legacy_env_name(name: &str) -> Option<String> {
    name.strip_prefix(ENV_PREFIX)
        .map(|rest| format!("{LEGACY_ENV_PREFIX}{rest}"))
}

/// `std::env::var(name)`, falling back to the variable of the former name
/// ([`legacy_env_name`]) with a warning in the log when only that one is set.
pub fn env_var(name: &str) -> Result<String, std::env::VarError> {
    env_var_with(name, |n| std::env::var(n))
}

/// [`env_var`] with the lookup injected (tests).
pub fn env_var_with(
    name: &str,
    get: impl Fn(&str) -> Result<String, std::env::VarError>,
) -> Result<String, std::env::VarError> {
    let first = get(name);
    if first.is_ok() {
        return first;
    }
    let Some(legacy) = legacy_env_name(name) else {
        return first;
    };
    match get(&legacy) {
        Ok(v) => {
            tracing::warn!("environment variable {legacy} is deprecated: rename it {name}");
            Ok(v)
        }
        Err(_) => first,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn env_var_falls_back_to_the_former_name() {
        use std::env::VarError;
        let legacy = legacy_env_name("CORDE_LOG").unwrap();
        assert_eq!(legacy, format!("{LEGACY_ENV_PREFIX}LOG"));
        assert_eq!(legacy_env_name("PATH"), None);
        let only_legacy = |n: &str| {
            if n == legacy {
                Ok("debug".to_string())
            } else {
                Err(VarError::NotPresent)
            }
        };
        assert_eq!(env_var_with("CORDE_LOG", only_legacy).unwrap(), "debug");
        let both = |n: &str| Ok(if n == "CORDE_LOG" { "info" } else { "debug" }.to_string());
        assert_eq!(env_var_with("CORDE_LOG", both).unwrap(), "info");
        let none = |_: &str| Err(VarError::NotPresent);
        assert_eq!(env_var_with("CORDE_LOG", none), Err(VarError::NotPresent));
    }

    fn temp(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "corde-migrate-{tag}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A data folder as the former application left it.
    fn legacy_folder(root: &Path) -> PathBuf {
        // legacy name (Muret, before 0.3.0)
        let legacy = root.join("Muret");
        fs::create_dir_all(legacy.join("sessions").join("s1")).unwrap();
        fs::create_dir_all(legacy.join("layouts")).unwrap();
        fs::write(legacy.join(LEGACY_DB_FILE), b"db").unwrap();
        fs::write(legacy.join("sessions/s1/samples-000.parquet"), b"pq").unwrap();
        fs::write(legacy.join("layouts/a.json"), b"{}").unwrap();
        fs::write(legacy.join("settings.json"), b"{}").unwrap();
        legacy
    }

    fn refuse(_: &Path, _: &Path) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::PermissionDenied, "locked"))
    }

    #[test]
    fn only_the_legacy_folder_is_moved() {
        let root = temp("legacy-only");
        let legacy = legacy_folder(&root);
        let new = root.join("Corde");
        let m = migrate_data_dir(&new, &legacy);
        assert_eq!(m.outcome, DataDirOutcome::Moved);
        assert_eq!(m.dir, new);
        assert!(!m.uses_legacy());
        assert!(!legacy.exists());
        assert!(new.join(LEGACY_DB_FILE).is_file());
        assert!(new.join("sessions/s1/samples-000.parquet").is_file());
        assert!(new.join("layouts/a.json").is_file());
        assert!(new.join("settings.json").is_file());
        // Second start: nothing left to do.
        let again = migrate_data_dir(&new, &legacy);
        assert_eq!(again.outcome, DataDirOutcome::Nothing);
        assert_eq!(again.dir, new);
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn only_the_new_folder_or_none() {
        let root = temp("new-only");
        let new = root.join("Corde");
        let legacy = root.join("Muret"); // legacy name (Muret, before 0.3.0)
        let m = migrate_data_dir(&new, &legacy);
        assert_eq!(
            (m.outcome, m.dir.clone()),
            (DataDirOutcome::Nothing, new.clone())
        );
        assert!(!new.exists(), "nothing is created here");
        fs::create_dir_all(&new).unwrap();
        fs::write(new.join(DB_FILE), b"db").unwrap();
        let m = migrate_data_dir(&new, &legacy);
        assert_eq!((m.outcome, m.dir), (DataDirOutcome::Nothing, new.clone()));
        assert!(new.join(DB_FILE).is_file());
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn both_folders_new_one_wins_nothing_merged() {
        let root = temp("both");
        let legacy = legacy_folder(&root);
        let new = root.join("Corde");
        fs::create_dir_all(&new).unwrap();
        fs::write(new.join(DB_FILE), b"new db").unwrap();
        let m = migrate_data_dir(&new, &legacy);
        assert_eq!(m.outcome, DataDirOutcome::BothExist);
        assert_eq!(m.dir, new);
        // Untouched on both sides.
        assert!(legacy.join(LEGACY_DB_FILE).is_file());
        assert!(legacy.join("sessions/s1/samples-000.parquet").is_file());
        assert!(!new.join("sessions").exists());
        assert_eq!(fs::read(new.join(DB_FILE)).unwrap(), b"new db");
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn move_impossible_keeps_the_legacy_folder_for_this_run() {
        let root = temp("locked");
        let legacy = legacy_folder(&root);
        let new = root.join("Corde");
        let m = migrate_data_dir_with(&new, &legacy, refuse);
        assert!(matches!(m.outcome, DataDirOutcome::MoveFailed { .. }));
        assert_eq!(m.dir, legacy);
        assert!(m.uses_legacy());
        assert!(!new.exists(), "no empty folder left behind");
        assert!(legacy.join(LEGACY_DB_FILE).is_file());
        // Next start, the files are free again: the move happens.
        let next = migrate_data_dir(&new, &legacy);
        assert_eq!(next.outcome, DataDirOutcome::Moved);
        assert!(new.join("sessions/s1/samples-000.parquet").is_file());
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn move_impossible_for_real() {
        // A path under a regular file can never be created, whoever runs the test.
        let root = temp("real-fail");
        let legacy = legacy_folder(&root);
        fs::write(root.join("not-a-dir"), b"").unwrap();
        let new = root.join("not-a-dir").join("Corde");
        let m = migrate_data_dir(&new, &legacy);
        assert!(matches!(m.outcome, DataDirOutcome::MoveFailed { .. }));
        assert_eq!(m.dir, legacy);
        assert!(legacy.join(LEGACY_DB_FILE).is_file());
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn empty_new_folder_never_hides_the_legacy_data() {
        let root = temp("empty-new");
        let legacy = legacy_folder(&root);
        let new = root.join("Corde");
        fs::create_dir_all(&new).unwrap();
        let m = migrate_data_dir(&new, &legacy);
        assert_eq!(m.outcome, DataDirOutcome::MovedOverEmpty);
        assert_eq!(m.dir, new);
        assert!(new.join(LEGACY_DB_FILE).is_file());
        assert!(!legacy.exists());

        // Not empty but without a database: the legacy folder is used, nothing merged.
        let root2 = temp("nodata-new");
        let legacy2 = legacy_folder(&root2);
        let new2 = root2.join("Corde");
        fs::create_dir_all(new2.join("logs")).unwrap();
        let m = migrate_data_dir(&new2, &legacy2);
        assert_eq!(m.outcome, DataDirOutcome::NewHasNoData);
        assert_eq!(m.dir, legacy2);
        assert!(new2.join("logs").is_dir());
        assert!(legacy2.join(LEGACY_DB_FILE).is_file());
        fs::remove_dir_all(&root).ok();
        fs::remove_dir_all(&root2).ok();
    }

    #[test]
    fn database_renamed_with_its_wal_and_shm() {
        let dir = temp("db-wal");
        fs::write(dir.join(LEGACY_DB_FILE), b"main").unwrap();
        fs::write(dir.join("muret.db-wal"), b"wal").unwrap(); // legacy name (Muret, before 0.3.0)
        fs::write(dir.join("muret.db-shm"), b"shm").unwrap(); // legacy name (Muret, before 0.3.0)
        let m = migrate_db_file(&dir);
        assert_eq!(m.outcome, DbFileOutcome::Renamed);
        assert_eq!(m.path, dir.join(DB_FILE));
        assert_eq!(fs::read(dir.join("corde.db")).unwrap(), b"main");
        assert_eq!(fs::read(dir.join("corde.db-wal")).unwrap(), b"wal");
        assert_eq!(fs::read(dir.join("corde.db-shm")).unwrap(), b"shm");
        assert!(!dir.join(LEGACY_DB_FILE).exists());
        assert!(!dir.join("muret.db-wal").exists()); // legacy name (Muret, before 0.3.0)
        assert_eq!(migrate_db_file(&dir).outcome, DbFileOutcome::Nothing);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn database_rename_failure_puts_everything_back() {
        let dir = temp("db-fail");
        fs::write(dir.join(LEGACY_DB_FILE), b"main").unwrap();
        fs::write(dir.join("muret.db-wal"), b"wal").unwrap(); // legacy name (Muret, before 0.3.0)
                                                              // The main file is locked: the WAL, already renamed, must come back.
        let m = migrate_db_file_with(&dir, |from, to| {
            if from.file_name().unwrap() == LEGACY_DB_FILE {
                Err(io::Error::new(io::ErrorKind::PermissionDenied, "locked"))
            } else {
                fs::rename(from, to)
            }
        });
        assert!(matches!(m.outcome, DbFileOutcome::RenameFailed { .. }));
        assert_eq!(m.path, dir.join(LEGACY_DB_FILE));
        assert_eq!(fs::read(dir.join("muret.db-wal")).unwrap(), b"wal"); // legacy name (Muret, before 0.3.0)
        assert!(!dir.join("corde.db-wal").exists());
        assert!(!dir.join("corde.db").exists());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn both_databases_new_one_wins() {
        let dir = temp("db-both");
        fs::write(dir.join(LEGACY_DB_FILE), b"old").unwrap();
        fs::write(dir.join(DB_FILE), b"new").unwrap();
        let m = migrate_db_file(&dir);
        assert_eq!(m.outcome, DbFileOutcome::BothExist);
        assert_eq!(m.path, dir.join(DB_FILE));
        assert_eq!(fs::read(dir.join(LEGACY_DB_FILE)).unwrap(), b"old");
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn default_folders_differ_by_name_only() {
        assert_eq!(default_data_dir().parent(), legacy_data_dir().parent());
        let name = default_data_dir()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_ascii_lowercase();
        assert_eq!(name, "corde");
    }
}
