//! Dashboard layouts persisted as `<data-dir>/layouts/<id>.json`.
//!
//! Small, synchronous, write-through store with an in-memory cache. Writes go
//! to a temporary file then `rename` (atomic on both Windows and Unix).

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

/// Ids are used as file names: letters, digits, `-`, `_`, `.` (no leading dot), ≤ 64 chars.
pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && !id.starts_with('.')
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
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
            match fs::read(&path).and_then(|bytes| {
                serde_json::from_slice::<DashboardLayout>(&bytes).map_err(io::Error::other)
            }) {
                Ok(layout) if valid_id(&layout.id) => {
                    cache.insert(layout.id.clone(), layout);
                }
                Ok(layout) => tracing::warn!(
                    "ignoring layout with invalid id {:?} in {}",
                    layout.id,
                    path.display()
                ),
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

    /// Insert or replace one layout (its `id` must be valid).
    pub fn upsert(&self, layout: DashboardLayout) -> io::Result<DashboardLayout> {
        if !valid_id(&layout.id) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid layout id",
            ));
        }
        self.write_file(&layout)?;
        self.lock().insert(layout.id.clone(), layout.clone());
        Ok(layout)
    }

    /// Replace the whole collection (files of layouts absent from `layouts` are removed).
    pub fn replace_all(&self, layouts: Vec<DashboardLayout>) -> io::Result<Vec<DashboardLayout>> {
        if let Some(bad) = layouts.iter().find(|l| !valid_id(&l.id)) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("invalid layout id {:?}", bad.id),
            ));
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
        let dir = tmp_dir("ids");
        let store = LayoutStore::open(&dir).unwrap();
        assert!(store.upsert(layout("../evil")).is_err());
        let _ = fs::remove_dir_all(&dir);
    }
}
