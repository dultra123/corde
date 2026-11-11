//! User settings persisted as `<data-dir>/settings.json`.
//!
//! **Contract shared with the desktop shell** (`apps/desktop`): the file lives
//! at the root of the data directory and holds one JSON object; its boolean
//! key `lan_access` (default `false`) decides whether the server listens on
//! the local network (see [`crate::config::resolve_bind`]).
//!
//! ```json
//! { "lan_access": false }
//! ```
//!
//! Reading is tolerant: a missing or unreadable file means the defaults, and
//! keys this version does not know are kept untouched on the next save, so the
//! shell — or a later version — can store its own values in the same file.

use serde_json::{Map, Value};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// File name inside the data directory.
pub const SETTINGS_FILE: &str = "settings.json";

const KEY_LAN_ACCESS: &str = "lan_access";

/// What `settings.json` holds.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Settings {
    /// Listen on every interface, so that a phone or another PC of the local
    /// network can open Corde. `false` (default) = this PC only, which also
    /// spares the Windows firewall prompt on first launch.
    pub lan_access: bool,
    /// Keys of the file this version does not know, written back as they are.
    pub extra: Map<String, Value>,
}

impl Settings {
    /// `<data-dir>/settings.json`.
    pub fn path(data_dir: &Path) -> PathBuf {
        data_dir.join(SETTINGS_FILE)
    }

    /// Read the settings of a data directory. Never fails: a missing file is
    /// the defaults, an unreadable one too (with a warning in the log).
    pub fn load(data_dir: &Path) -> Self {
        let path = Self::path(data_dir);
        let bytes = match fs::read(&path) {
            Ok(b) => b,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Self::default(),
            Err(e) => {
                tracing::warn!(
                    "réglages illisibles ({}) : valeurs par défaut — {e}",
                    path.display()
                );
                return Self::default();
            }
        };
        // A file saved by Notepad may start with a UTF-8 byte order mark.
        let json = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&bytes);
        match serde_json::from_slice::<Value>(json) {
            Ok(value) => Self::from_json(value).unwrap_or_else(|| {
                tracing::warn!(
                    "{} ne contient pas un objet JSON : valeurs par défaut",
                    path.display()
                );
                Self::default()
            }),
            Err(e) => {
                tracing::warn!(
                    "{} n'est pas du JSON valide ({e}) : valeurs par défaut",
                    path.display()
                );
                Self::default()
            }
        }
    }

    /// Build the settings from the content of the file. `None` when it is not
    /// a JSON object. A `lan_access` that is not a boolean counts as `false`.
    pub fn from_json(value: Value) -> Option<Self> {
        let Value::Object(mut map) = value else {
            return None;
        };
        let lan_access = match map.remove(KEY_LAN_ACCESS) {
            Some(Value::Bool(b)) => b,
            Some(other) => {
                tracing::warn!("réglage `lan_access` invalide ({other}) : false");
                false
            }
            None => false,
        };
        Some(Self {
            lan_access,
            extra: map,
        })
    }

    /// The JSON object written to disk (known keys first, then the unknown ones).
    pub fn to_json(&self) -> Value {
        let mut map = Map::new();
        map.insert(KEY_LAN_ACCESS.to_string(), Value::Bool(self.lan_access));
        for (k, v) in &self.extra {
            if k != KEY_LAN_ACCESS {
                map.insert(k.clone(), v.clone());
            }
        }
        Value::Object(map)
    }

    /// Write `<data-dir>/settings.json` (temporary file then `rename`, atomic
    /// on both Windows and Unix). The directory is created when missing.
    pub fn save(&self, data_dir: &Path) -> io::Result<()> {
        fs::create_dir_all(data_dir)?;
        let mut json = serde_json::to_vec_pretty(&self.to_json()).map_err(io::Error::other)?;
        json.push(b'\n');
        let path = Self::path(data_dir);
        let tmp = data_dir.join(format!("{SETTINGS_FILE}.tmp"));
        fs::write(&tmp, json)?;
        fs::rename(&tmp, &path)
    }
}

/// The settings of the running server. The file is the single source of
/// truth (the desktop shell may write it too): every read goes to disk, every
/// change is a read-modify-write under a lock.
#[derive(Debug)]
pub struct SettingsStore {
    data_dir: PathBuf,
    write_lock: Mutex<()>,
}

impl SettingsStore {
    pub fn open(data_dir: &Path) -> Self {
        Self {
            data_dir: data_dir.to_path_buf(),
            write_lock: Mutex::new(()),
        }
    }

    /// The settings as they are on disk right now (defaults without a file).
    pub fn get(&self) -> Settings {
        Settings::load(&self.data_dir)
    }

    /// Apply `change` to the settings on disk and write them back; keys this
    /// version does not know are preserved.
    pub fn update(&self, change: impl FnOnce(&mut Settings)) -> io::Result<Settings> {
        let _guard = match self.write_lock.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        let mut next = Settings::load(&self.data_dir);
        change(&mut next);
        next.save(&self.data_dir)?;
        Ok(next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn tmp_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "corde-settings-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let _ = fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn missing_file_means_defaults_and_lan_is_closed() {
        let dir = tmp_dir("missing");
        let s = Settings::load(&dir);
        assert_eq!(s, Settings::default());
        assert!(!s.lan_access);
        assert!(!dir.exists(), "loading must not create anything");
    }

    #[test]
    fn save_then_load_round_trips() {
        let dir = tmp_dir("roundtrip");
        let s = Settings {
            lan_access: true,
            ..Default::default()
        };
        s.save(&dir).unwrap();
        assert_eq!(Settings::load(&dir), s);
        let text = fs::read_to_string(Settings::path(&dir)).unwrap();
        let on_disk: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(on_disk, json!({ "lan_access": true }));
        assert!(!dir.join("settings.json.tmp").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn unknown_keys_survive_a_save() {
        let dir = tmp_dir("unknown");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            Settings::path(&dir),
            r#"{ "lan_access": true, "window": { "w": 1280, "h": 800 }, "autostart": false }"#,
        )
        .unwrap();
        let mut s = Settings::load(&dir);
        assert!(s.lan_access);
        assert_eq!(s.extra.len(), 2);
        s.lan_access = false;
        s.save(&dir).unwrap();
        let on_disk: Value =
            serde_json::from_slice(&fs::read(Settings::path(&dir)).unwrap()).unwrap();
        assert_eq!(
            on_disk,
            json!({ "lan_access": false, "window": { "w": 1280, "h": 800 }, "autostart": false })
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn broken_files_fall_back_to_defaults() {
        let dir = tmp_dir("broken");
        fs::create_dir_all(&dir).unwrap();
        for content in ["", "not json", "[true]", "42", r#"{ "lan_access": "yes" }"#] {
            fs::write(Settings::path(&dir), content).unwrap();
            assert!(
                !Settings::load(&dir).lan_access,
                "{content:?} must not open the LAN"
            );
        }
        // A wrong type for one key does not throw the other keys away.
        fs::write(
            Settings::path(&dir),
            r#"{ "lan_access": 1, "theme": "dark" }"#,
        )
        .unwrap();
        let s = Settings::load(&dir);
        assert!(!s.lan_access);
        assert_eq!(s.extra.get("theme"), Some(&json!("dark")));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_byte_order_mark_is_ignored() {
        // Notepad saves "UTF-8 with BOM": the setting must still be read.
        let dir = tmp_dir("bom");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            Settings::path(&dir),
            "\u{feff}{ \"lan_access\": true, \"theme\": \"dark\" }",
        )
        .unwrap();
        let s = Settings::load(&dir);
        assert!(s.lan_access);
        assert_eq!(s.extra.get("theme"), Some(&json!("dark")));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn store_reads_the_file_and_keeps_keys_added_by_someone_else() {
        let dir = tmp_dir("store");
        let store = SettingsStore::open(&dir);
        assert!(!store.get().lan_access);
        let saved = store.update(|s| s.lan_access = true).unwrap();
        assert!(saved.lan_access);
        assert!(Settings::load(&dir).lan_access);

        // The shell rewrites the file while the server runs: the store follows.
        fs::write(
            Settings::path(&dir),
            r#"{ "lan_access": false, "minimized": true }"#,
        )
        .unwrap();
        assert!(!store.get().lan_access);
        let saved = store.update(|s| s.lan_access = true).unwrap();
        assert!(saved.lan_access);
        let reloaded = Settings::load(&dir);
        assert!(reloaded.lan_access);
        assert_eq!(reloaded.extra.get("minimized"), Some(&json!(true)));
        let _ = fs::remove_dir_all(&dir);
    }
}
