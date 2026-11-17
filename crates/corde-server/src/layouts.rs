//! Dashboard layouts persisted as `<data-dir>/layouts/<id>.json`.
//!
//! Small, synchronous, write-through store with an in-memory cache. Writes go
//! to a temporary file then `rename` (atomic on both Windows and Unix).
//!
//! Any device allowed to reach the server may save a layout (the phone edits its own), so
//! what is stored is bounded: at most [`MAX_LAYOUTS`] layouts of [`MAX_LAYOUT_BYTES`] each,
//! [`MAX_WIDGETS`] widgets, short names without control characters, and an id that is a
//! plain file name on every system (never a device name of Windows).

use corde_protocol::DashboardLayout;
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub struct LayoutStore {
    dir: PathBuf,
    cache: Mutex<BTreeMap<String, DashboardLayout>>,
}

impl std::fmt::Debug for LayoutStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LayoutStore")
            .field("dir", &self.dir)
            .finish()
    }
}

/// Most layouts kept (the interface makes a handful: one or two per kind of screen).
pub const MAX_LAYOUTS: usize = 100;
/// Largest layout, serialized (a full dashboard is a few kilobytes).
pub const MAX_LAYOUT_BYTES: usize = 256 * 1024;
/// Most widgets of one layout.
pub const MAX_WIDGETS: usize = 200;
/// Longest name of a layout, in characters.
pub const MAX_NAME_CHARS: usize = 120;
/// Longest id or kind of a widget, in characters.
pub const MAX_WIDGET_TEXT_CHARS: usize = 64;

/// Ids are used as file names: letters, digits, `-`, `_`, `.` (no leading nor trailing dot),
/// ≤ 64 chars, and never a device name of Windows (`con`, `nul`, `com1`… with or without
/// an extension: the file would be the device).
pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && !id.starts_with('.')
        && !id.ends_with('.')
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        && !corde_core::fsname::is_windows_device_name(id)
}

fn invalid(msg: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, msg.into())
}

fn short_text(what: &str, s: &str, max: usize) -> io::Result<()> {
    if s.chars().count() > max {
        return Err(invalid(format!("{what}: more than {max} characters")));
    }
    if s.chars().any(|c| c.is_control()) {
        return Err(invalid(format!("{what}: control character")));
    }
    Ok(())
}

/// What a layout must respect to be stored (its content stays the interface's business: a
/// widget it does not know is shown as such, a setting it does not know is ignored).
pub fn check(layout: &DashboardLayout) -> io::Result<()> {
    if !valid_id(&layout.id) {
        return Err(invalid("invalid layout id"));
    }
    short_text("layout name", &layout.name, MAX_NAME_CHARS)?;
    if layout.widgets.len() > MAX_WIDGETS {
        return Err(invalid(format!("layout: more than {MAX_WIDGETS} widgets")));
    }
    for w in &layout.widgets {
        short_text("widget id", &w.id, MAX_WIDGET_TEXT_CHARS)?;
        short_text("widget kind", &w.kind, MAX_WIDGET_TEXT_CHARS)?;
    }
    let size = serde_json::to_vec(layout).map_err(io::Error::other)?.len();
    if size > MAX_LAYOUT_BYTES {
        return Err(invalid(format!(
            "layout too large ({size} bytes, at most {MAX_LAYOUT_BYTES})"
        )));
    }
    Ok(())
}

impl LayoutStore {
    /// Open (creating it) `<data_dir>/layouts` and load every `*.json` in it.
    pub fn open(data_dir: &Path) -> io::Result<Self> {
        let dir = data_dir.join("layouts");
        fs::create_dir_all(&dir)?;
        let mut cache = BTreeMap::new();
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let read = fs::metadata(&path).and_then(|m| {
                if m.len() as usize > MAX_LAYOUT_BYTES * 4 {
                    Err(io::Error::other("file too large"))
                } else {
                    fs::read(&path)
                }
            });
            match read.and_then(|bytes| {
                serde_json::from_slice::<DashboardLayout>(&bytes).map_err(io::Error::other)
            }) {
                Ok(layout) if cache.len() >= MAX_LAYOUTS => tracing::warn!(
                    "ignoring layout {:?}: more than {MAX_LAYOUTS} in {}",
                    layout.id,
                    dir.display()
                ),
                Ok(layout) => match check(&layout) {
                    Ok(()) => {
                        cache.insert(layout.id.clone(), layout);
                    }
                    Err(e) => tracing::warn!("ignoring layout {}: {e}", path.display()),
                },
                Err(e) => tracing::warn!("ignoring unreadable layout {}: {e}", path.display()),
            }
        }
        tracing::info!("{} layout(s) loaded from {}", cache.len(), dir.display());
        Ok(Self {
            dir,
            cache: Mutex::new(cache),
        })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn list(&self) -> Vec<DashboardLayout> {
        self.lock().values().cloned().collect()
    }

    pub fn get(&self, id: &str) -> Option<DashboardLayout> {
        self.lock().get(id).cloned()
    }

    /// Insert or replace one layout, after [`check`]; refused beyond [`MAX_LAYOUTS`].
    pub fn upsert(&self, layout: DashboardLayout) -> io::Result<DashboardLayout> {
        check(&layout)?;
        let mut cache = self.lock();
        if !cache.contains_key(&layout.id) && cache.len() >= MAX_LAYOUTS {
            return Err(invalid(format!("too many layouts (at most {MAX_LAYOUTS})")));
        }
        self.write_file(&layout)?;
        cache.insert(layout.id.clone(), layout.clone());
        Ok(layout)
    }

    /// Replace the whole collection (files of layouts absent from `layouts` are removed).
    pub fn replace_all(&self, layouts: Vec<DashboardLayout>) -> io::Result<Vec<DashboardLayout>> {
        if layouts.len() > MAX_LAYOUTS {
            return Err(invalid(format!("too many layouts (at most {MAX_LAYOUTS})")));
        }
        for l in &layouts {
            check(l).map_err(|e| invalid(format!("layout {:?}: {e}", l.id)))?;
        }
        let mut next = BTreeMap::new();
        for l in layouts {
            self.write_file(&l)?;
            next.insert(l.id.clone(), l);
        }
        let mut cache = self.lock();
        for id in cache.keys() {
            if !next.contains_key(id) {
                let _ = fs::remove_file(self.path_of(id));
            }
        }
        *cache = next;
        Ok(cache.values().cloned().collect())
    }

    /// Returns `true` when the layout existed.
    pub fn delete(&self, id: &str) -> io::Result<bool> {
        if !valid_id(id) {
            return Ok(false);
        }
        let existed = self.lock().remove(id).is_some();
        match fs::remove_file(self.path_of(id)) {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(existed),
            Err(e) => Err(e),
        }
    }

    /// Remove every layout; returns how many were removed.
    pub fn clear(&self) -> io::Result<usize> {
        let mut cache = self.lock();
        let n = cache.len();
        for id in cache.keys() {
            match fs::remove_file(self.path_of(id)) {
                Ok(()) => {}
                Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                Err(e) => return Err(e),
            }
        }
        cache.clear();
        Ok(n)
    }

    fn path_of(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{id}.json"))
    }

    fn write_file(&self, layout: &DashboardLayout) -> io::Result<()> {
        let json = serde_json::to_vec_pretty(layout).map_err(io::Error::other)?;
        let path = self.path_of(&layout.id);
        let tmp = self.dir.join(format!("{}.json.tmp", layout.id));
        fs::write(&tmp, json)?;
        fs::rename(&tmp, &path)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, BTreeMap<String, DashboardLayout>> {
        match self.cache.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use corde_protocol::{DeviceKind, WidgetPlacement};

    fn tmp_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "corde-layouts-{}-{}-{}",
            name,
            std::process::id(),
            std::time::SystemTime::now()
                .elapsed()
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let _ = fs::remove_dir_all(&d);
        d
    }

    fn layout(id: &str) -> DashboardLayout {
        DashboardLayout {
            id: id.into(),
            name: format!("Layout {id}"),
            device: DeviceKind::Desktop,
            columns: 12,
            widgets: vec![WidgetPlacement {
                id: "w1".into(),
                kind: "tyres".into(),
                x: 0,
                y: 0,
                w: 4,
                h: 3,
                config: serde_json::json!({}),
            }],
            updated_ms: 1,
        }
    }

    #[test]
    fn crud_persists_on_disk() {
        let dir = tmp_dir("crud");
        let store = LayoutStore::open(&dir).unwrap();
        assert!(store.list().is_empty());
        store.upsert(layout("desk")).unwrap();
        store.upsert(layout("phone")).unwrap();
        assert_eq!(store.list().len(), 2);
        assert!(dir.join("layouts").join("desk.json").is_file());

        // Reopen → reloaded from disk.
        let store2 = LayoutStore::open(&dir).unwrap();
        assert_eq!(store2.get("phone").unwrap().name, "Layout phone");

        assert!(store2.delete("desk").unwrap());
        assert!(!store2.delete("desk").unwrap());
        assert!(!dir.join("layouts").join("desk.json").exists());

        let all = store2.replace_all(vec![layout("a"), layout("b")]).unwrap();
        assert_eq!(all.len(), 2);
        assert!(!dir.join("layouts").join("phone.json").exists());
        assert_eq!(store2.clear().unwrap(), 2);
        assert!(store2.list().is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_bad_ids() {
        assert!(valid_id("desktop-1"));
        assert!(valid_id("a.b_c"));
        assert!(!valid_id(""));
        assert!(!valid_id("../x"));
        assert!(!valid_id(".hidden"));
        assert!(!valid_id("a/b"));
        assert!(!valid_id("a b"));
        assert!(!valid_id(&"x".repeat(65)));
        assert!(!valid_id("a."));
        let dir = tmp_dir("ids");
        let store = LayoutStore::open(&dir).unwrap();
        assert!(store.upsert(layout("../evil")).is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    /// On Windows `con.json` or `com1.json` is the device, not a file: such an id is never a
    /// file name here.
    #[test]
    fn refuses_windows_device_names() {
        let dir = tmp_dir("devices");
        let store = LayoutStore::open(&dir).unwrap();
        for id in [
            "con",
            "NUL",
            "aux",
            "prn",
            "com1",
            "LPT9",
            "con.v2",
            "nul.backup",
        ] {
            assert!(!valid_id(id), "{id}");
            assert!(store.upsert(layout(id)).is_err(), "{id}");
            assert!(store.replace_all(vec![layout(id)]).is_err(), "{id}");
        }
        assert!(store.list().is_empty());
        assert!(store.upsert(layout("console")).is_ok());
        assert!(store.upsert(layout("com10")).is_ok());
        let _ = fs::remove_dir_all(&dir);
    }

    /// Any device of the network may save a layout: what is kept is bounded.
    #[test]
    fn what_is_stored_is_bounded() {
        let dir = tmp_dir("bounds");
        let store = LayoutStore::open(&dir).unwrap();
        // Too many layouts.
        for i in 0..MAX_LAYOUTS {
            store.upsert(layout(&format!("l{i}"))).unwrap();
        }
        let e = store.upsert(layout("one-more")).unwrap_err();
        assert_eq!(e.kind(), io::ErrorKind::InvalidInput);
        // An existing one can still be saved again.
        store.upsert(layout("l0")).unwrap();
        let many: Vec<_> = (0..=MAX_LAYOUTS)
            .map(|i| layout(&format!("m{i}")))
            .collect();
        assert!(store.replace_all(many).is_err());
        assert_eq!(store.list().len(), MAX_LAYOUTS, "nothing was replaced");
        // Too large, too many widgets, a name that is not one.
        let mut big = layout("l0");
        big.widgets[0].config = serde_json::json!({ "x": "y".repeat(MAX_LAYOUT_BYTES) });
        assert!(store.upsert(big).is_err());
        let mut crowded = layout("l0");
        crowded.widgets = (0..=MAX_WIDGETS)
            .map(|i| WidgetPlacement {
                id: format!("w{i}"),
                ..crowded.widgets[0].clone()
            })
            .collect();
        assert!(store.upsert(crowded).is_err());
        let mut named = layout("l0");
        named.name = "x".repeat(MAX_NAME_CHARS + 1);
        assert!(store.upsert(named).is_err());
        let mut named = layout("l0");
        named.name = "line\nbreak".into();
        assert!(store.upsert(named).is_err());
        let mut kind = layout("l0");
        kind.widgets[0].kind = "k".repeat(MAX_WIDGET_TEXT_CHARS + 1);
        assert!(store.upsert(kind).is_err());
        let _ = fs::remove_dir_all(&dir);
    }
}
