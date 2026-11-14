//! Race dashboards made in the Studio, persisted as `<data-dir>/dashes/<id>.json` (ADR 0015).
//!
//! Same shape as [`crate::layouts`]: a small synchronous write-through store with an in-memory
//! cache, writes to a temporary file then `rename`. Every definition is checked by
//! [`validate`] before it is stored, and again when it is read back from the disk: the same
//! rules as the interface (`packages/ui/src/dashdef/validate.ts`), kept in sync by the tests of
//! both sides.
//!
//! A definition is pure data. Nothing in it is ever run or injected as HTML by the interface;
//! the validation keeps it so anyway: bounded sizes, closed character sets for the ids,
//! `#rrggbb` colours, no markup, no address, no nested object.

use corde_protocol::{DashDefinition, DashElement, DASH_DEFINITION_VERSION};
use serde_json::Value;
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Largest definition, serialized (compact JSON).
pub const MAX_DASH_BYTES: usize = 256 * 1024;
/// Most definitions kept on the PC.
pub const MAX_DASHES: usize = 50;
pub const MAX_PAGES: usize = 12;
pub const MAX_ELEMENTS: usize = 80;
pub const MAX_ID_LEN: usize = 40;
pub const MAX_NAME_LEN: usize = 60;
pub const MAX_PAGE_NAME_LEN: usize = 40;
pub const MAX_PROP_KEYS: usize = 32;
pub const MAX_PROP_KEY_LEN: usize = 32;
pub const MAX_PROP_STRING_LEN: usize = 200;
pub const MAX_PROP_ARRAY_LEN: usize = 16;
pub const MAX_PROP_NUMBER: f64 = 1_000_000.0;
pub const MIN_COLS: u8 = 4;
pub const MAX_COLS: u8 = 48;
pub const MIN_ROWS: u8 = 2;
pub const MAX_ROWS: u8 = 32;
pub const MIN_FORMAT_SIDE: u16 = 100;
pub const MAX_FORMAT_SIDE: u16 = 4000;
/// Width / height of a format, at most this way or the other.
pub const MAX_FORMAT_RATIO: f64 = 8.0;
/// The dashboard shipped with Corde (`packages/ui/src/dashdef/builtin.ts`): never stored.
pub const BUILTIN_ID: &str = "corde";

/// Ids of definitions, pages and elements: `[a-z0-9-]{1,40}` (also a file name on the PC).
pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_ID_LEN
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// Element types: `[a-z0-9.-]{1,40}`.
fn valid_type(t: &str) -> bool {
    !t.is_empty()
        && t.len() <= MAX_ID_LEN
        && t.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'.')
}

/// `#rrggbb`.
pub fn valid_color(c: &str) -> bool {
    let b = c.as_bytes();
    b.len() == 7 && b[0] == b'#' && b[1..].iter().all(|x| x.is_ascii_hexdigit())
}

fn valid_prop_key(k: &str) -> bool {
    let mut it = k.bytes();
    matches!(it.next(), Some(b) if b.is_ascii_alphabetic())
        && k.len() <= MAX_PROP_KEY_LEN
        && it.all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// A text of a definition: bounded, no control character, no markup, no address.
fn check_text(what: &str, s: &str, min: usize, max: usize) -> Result<(), String> {
    let n = s.chars().count();
    if n < min || n > max {
        return Err(format!("{what}: length {n} not in {min}..={max}"));
    }
    if s.chars().any(|c| c.is_control()) {
        return Err(format!("{what}: control character"));
    }
    if s.contains('<') || s.contains('>') {
        return Err(format!("{what}: '<' and '>' are not allowed"));
    }
    let lower = s.to_ascii_lowercase();
    if lower.contains("://")
        || lower.contains("www.")
        || ["javascript:", "data:", "vbscript:", "file:"]
            .iter()
            .any(|p| lower.trim_start().starts_with(p))
    {
        return Err(format!("{what}: no address allowed"));
    }
    Ok(())
}

fn check_scalar(what: &str, v: &Value) -> Result<(), String> {
    match v {
        Value::Null | Value::Bool(_) => Ok(()),
        Value::Number(n) => match n.as_f64() {
            Some(f) if f.is_finite() && f.abs() <= MAX_PROP_NUMBER => Ok(()),
            _ => Err(format!("{what}: number out of bounds")),
        },
        Value::String(s) => {
            check_text(what, s, 0, MAX_PROP_STRING_LEN)?;
            if s.starts_with('#') && !valid_color(s) {
                return Err(format!("{what}: colours are #rrggbb"));
            }
            Ok(())
        }
        _ => Err(format!("{what}: not a string, number or boolean")),
    }
}

fn check_props(what: &str, props: &Value) -> Result<(), String> {
    let Value::Object(map) = props else {
        return Err(format!("{what}: props must be an object"));
    };
    if map.len() > MAX_PROP_KEYS {
        return Err(format!("{what}: more than {MAX_PROP_KEYS} props"));
    }
    for (k, v) in map {
        if !valid_prop_key(k) {
            return Err(format!("{what}: invalid prop name {k:?}"));
        }
        let at = format!("{what}.{k}");
        match v {
            Value::Array(items) => {
                if items.len() > MAX_PROP_ARRAY_LEN {
                    return Err(format!("{at}: more than {MAX_PROP_ARRAY_LEN} items"));
                }
                for item in items {
                    check_scalar(&at, item)?;
                }
            }
            other => check_scalar(&at, other)?,
        }
        // A colour setting (`color`, `fillColor`…): `#rrggbb`, or `auto` (the element's own).
        if k == "color" || k.ends_with("Color") {
            match v {
                Value::String(s) if s == "auto" || valid_color(s) => {}
                Value::Null => {}
                _ => return Err(format!("{at}: colours are #rrggbb")),
            }
        }
    }
    Ok(())
}

fn check_element(what: &str, e: &DashElement, cols: u8, rows: u8) -> Result<(), String> {
    if !valid_id(&e.id) {
        return Err(format!("{what}: invalid id"));
    }
    if !valid_type(&e.kind) {
        return Err(format!("{what}: invalid type"));
    }
    if e.w == 0 || e.h == 0 {
        return Err(format!("{what}: empty size"));
    }
    if u16::from(e.x) + u16::from(e.w) > u16::from(cols)
        || u16::from(e.y) + u16::from(e.h) > u16::from(rows)
    {
        return Err(format!("{what}: outside the page"));
    }
    check_props(what, &e.props)
}

/// Checks a definition against every rule of ADR 0015 (the size included).
pub fn validate(d: &DashDefinition) -> Result<(), String> {
    if d.v != DASH_DEFINITION_VERSION {
        return Err(format!("unsupported version {}", d.v));
    }
    if !valid_id(&d.id) {
        return Err("invalid id".into());
    }
    check_text("name", &d.name, 1, MAX_NAME_LEN)?;
    if d.name.trim().is_empty() {
        return Err("name: empty".into());
    }
    if let Some(a) = &d.author {
        check_text("author", a, 0, MAX_NAME_LEN)?;
    }
    let f = &d.format;
    if f.preset.is_empty() || f.preset.len() > 24 || !valid_id(&f.preset) {
        return Err("format: invalid preset".into());
    }
    if !(MIN_FORMAT_SIDE..=MAX_FORMAT_SIDE).contains(&f.width)
        || !(MIN_FORMAT_SIDE..=MAX_FORMAT_SIDE).contains(&f.height)
    {
        return Err("format: size out of bounds".into());
    }
    let ratio = f64::from(f.width) / f64::from(f.height);
    if !(1.0 / MAX_FORMAT_RATIO..=MAX_FORMAT_RATIO).contains(&ratio) {
        return Err("format: proportions out of bounds".into());
    }
    let t = &d.theme;
    for (k, c) in [
        ("background", &t.background),
        ("panel", &t.panel),
        ("line", &t.line),
        ("text", &t.text),
        ("dim", &t.dim),
        ("accent", &t.accent),
    ] {
        if !valid_color(c) {
            return Err(format!("theme.{k}: colours are #rrggbb"));
        }
    }
    if d.pages.is_empty() || d.pages.len() > MAX_PAGES {
        return Err(format!("pages: 1..={MAX_PAGES}"));
    }
    let mut page_ids = HashSet::new();
    for (i, p) in d.pages.iter().enumerate() {
        let what = format!("pages[{i}]");
        if !valid_id(&p.id) || !page_ids.insert(p.id.as_str()) {
            return Err(format!("{what}: invalid or repeated id"));
        }
        check_text(&format!("{what}.name"), &p.name, 1, MAX_PAGE_NAME_LEN)?;
        if !(MIN_COLS..=MAX_COLS).contains(&p.cols) || !(MIN_ROWS..=MAX_ROWS).contains(&p.rows) {
            return Err(format!("{what}: grid out of bounds"));
        }
        if p.elements.len() > MAX_ELEMENTS {
            return Err(format!("{what}: more than {MAX_ELEMENTS} elements"));
        }
        let mut ids = HashSet::new();
        for (j, e) in p.elements.iter().enumerate() {
            let what = format!("{what}.elements[{j}]");
            check_element(&what, e, p.cols, p.rows)?;
            if !ids.insert(e.id.as_str()) {
                return Err(format!("{what}: repeated id"));
            }
        }
    }
    let size = serde_json::to_vec(d).map_err(|e| e.to_string())?.len();
    if size > MAX_DASH_BYTES {
        return Err(format!("definition too large ({size} bytes)"));
    }
    Ok(())
}

/// Why a definition was not stored.
#[derive(Debug)]
pub enum DashError {
    /// Refused by [`validate`] (or the reserved id).
    Invalid(String),
    /// Already [`MAX_DASHES`] definitions.
    Full,
    Io(io::Error),
}

impl std::fmt::Display for DashError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DashError::Invalid(m) => write!(f, "invalid dashboard: {m}"),
            DashError::Full => write!(f, "too many dashboards (at most {MAX_DASHES})"),
            DashError::Io(e) => write!(f, "{e}"),
        }
    }
}

impl From<io::Error> for DashError {
    fn from(e: io::Error) -> Self {
        DashError::Io(e)
    }
}

pub struct DashStore {
    dir: PathBuf,
    cache: Mutex<BTreeMap<String, DashDefinition>>,
}

impl std::fmt::Debug for DashStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DashStore").field("dir", &self.dir).finish()
    }
}

impl DashStore {
    /// Open (creating it) `<data_dir>/dashes` and load every valid `*.json` in it.
    pub fn open(data_dir: &Path) -> io::Result<Self> {
        let dir = data_dir.join("dashes");
        fs::create_dir_all(&dir)?;
        let mut cache = BTreeMap::new();
        for entry in fs::read_dir(&dir)? {
            let path = entry?.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            let read = fs::metadata(&path).and_then(|m| {
                if m.len() as usize > MAX_DASH_BYTES * 4 {
                    Err(io::Error::other("file too large"))
                } else {
                    fs::read(&path)
                }
            });
            match read.and_then(|b| {
                serde_json::from_slice::<DashDefinition>(&b).map_err(io::Error::other)
            }) {
                Ok(d) if cache.len() >= MAX_DASHES => tracing::warn!(
                    "ignoring dashboard {:?}: more than {MAX_DASHES} in {}",
                    d.id,
                    dir.display()
                ),
                Ok(d) => match validate(&d) {
                    Ok(()) if d.id == stem && d.id != BUILTIN_ID => {
                        cache.insert(d.id.clone(), d);
                    }
                    Ok(()) => tracing::warn!(
                        "ignoring dashboard {:?}: its id does not match {}",
                        d.id,
                        path.display()
                    ),
                    Err(e) => tracing::warn!("ignoring dashboard {}: {e}", path.display()),
                },
                Err(e) => tracing::warn!("ignoring unreadable dashboard {}: {e}", path.display()),
            }
        }
        tracing::info!("{} dashboard(s) loaded from {}", cache.len(), dir.display());
        Ok(Self {
            dir,
            cache: Mutex::new(cache),
        })
    }

    /// [`DashStore::open`], or an empty store when the folder cannot be read (the server
    /// still runs; saving a dashboard then fails with an error).
    pub fn open_or_empty(data_dir: &Path) -> Self {
        Self::open(data_dir).unwrap_or_else(|e| {
            tracing::error!("dashboards folder unreadable: {e}");
            Self {
                dir: data_dir.join("dashes"),
                cache: Mutex::new(BTreeMap::new()),
            }
        })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn list(&self) -> Vec<DashDefinition> {
        self.lock().values().cloned().collect()
    }

    pub fn get(&self, id: &str) -> Option<DashDefinition> {
        self.lock().get(id).cloned()
    }

    /// Insert or replace one definition after [`validate`].
    pub fn upsert(&self, d: DashDefinition) -> Result<DashDefinition, DashError> {
        if d.id == BUILTIN_ID {
            return Err(DashError::Invalid("the id « corde » is reserved".into()));
        }
        validate(&d).map_err(DashError::Invalid)?;
        let mut cache = self.lock();
        if !cache.contains_key(&d.id) && cache.len() >= MAX_DASHES {
            return Err(DashError::Full);
        }
        self.write_file(&d)?;
        cache.insert(d.id.clone(), d.clone());
        Ok(d)
    }

    /// Returns `true` when the definition existed.
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

    /// Remove every definition; returns how many were removed.
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

    fn write_file(&self, d: &DashDefinition) -> io::Result<()> {
        let json = serde_json::to_vec_pretty(d).map_err(io::Error::other)?;
        let tmp = self.dir.join(format!("{}.json.tmp", d.id));
        fs::write(&tmp, json)?;
        fs::rename(&tmp, self.path_of(&d.id))
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, BTreeMap<String, DashDefinition>> {
        match self.cache.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use corde_protocol::{DashFormat, DashPageDef, DashTheme};
    use serde_json::json;

    fn tmp_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "corde-dashes-{}-{}-{:?}",
            name,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let _ = fs::remove_dir_all(&d);
        d
    }

    pub(crate) fn dash(id: &str) -> DashDefinition {
        DashDefinition {
            v: 1,
            id: id.into(),
            name: format!("Dash {id}"),
            author: None,
            format: DashFormat {
                preset: "phone-landscape".into(),
                width: 852,
                height: 393,
            },
            theme: DashTheme {
                background: "#000000".into(),
                panel: "#0d1116".into(),
                line: "#1f2731".into(),
                text: "#f2f5f8".into(),
                dim: "#93a0ae".into(),
                accent: "#ffb454".into(),
            },
            pages: vec![DashPageDef {
                id: "p1".into(),
                name: "Course".into(),
                cols: 24,
                rows: 12,
                elements: vec![DashElement {
                    id: "e1".into(),
                    kind: "value".into(),
                    x: 0,
                    y: 0,
                    w: 6,
                    h: 3,
                    props: json!({ "field": "speed_kmh", "label": "Vitesse", "color": "#ff0000", "decimals": 0, "framed": true }),
                }],
            }],
            updated_ms: 1,
        }
    }

    fn with_props(props: Value) -> DashDefinition {
        let mut d = dash("x");
        d.pages[0].elements[0].props = props;
        d
    }

    #[test]
    fn a_plain_definition_is_valid() {
        assert_eq!(validate(&dash("mon-dash")), Ok(()));
    }

    #[test]
    fn ids() {
        assert!(valid_id("a"));
        assert!(valid_id("mon-dash-2"));
        assert!(valid_id(&"a".repeat(40)));
        for bad in ["", "A", "a_b", "a.b", "../x", "a/b", "a b", "é", ".x"] {
            assert!(!valid_id(bad), "{bad:?}");
        }
        assert!(!valid_id(&"a".repeat(41)));
        let mut d = dash("x");
        d.id = "../evil".into();
        assert!(validate(&d).is_err());
        let mut d = dash("x");
        d.pages[0].id = "P1".into();
        assert!(validate(&d).is_err());
        let mut d = dash("x");
        d.pages[0].elements[0].id = "e 1".into();
        assert!(validate(&d).is_err());
        let mut d = dash("x");
        d.pages[0].elements[0].kind = "<script>".into();
        assert!(validate(&d).is_err());
    }

    #[test]
    fn version_and_names() {
        let mut d = dash("x");
        d.v = 2;
        assert!(validate(&d).is_err());
        for bad in [
            "",
            "   ",
            "<b>x</b>",
            "voir https://exemple.org",
            "www.exemple.org",
            "javascript:alert(1)",
            "a\u{0}b",
            &"n".repeat(61),
        ] {
            let mut d = dash("x");
            d.name = bad.into();
            assert!(validate(&d).is_err(), "{bad:?}");
        }
        let mut d = dash("x");
        d.name = "GT3 · nuit (v2) — Spa".into();
        d.author = Some("dultra__".into());
        assert_eq!(validate(&d), Ok(()));
        d.author = Some("x".repeat(61));
        assert!(validate(&d).is_err());
    }

    #[test]
    fn format_and_theme() {
        let mut d = dash("x");
        d.format.width = 99;
        assert!(validate(&d).is_err());
        let mut d = dash("x");
        d.format.height = 4001;
        assert!(validate(&d).is_err());
        let mut d = dash("x");
        d.format.width = 4000;
        d.format.height = 400;
        assert!(validate(&d).is_err(), "ratio 10");
        let mut d = dash("x");
        d.format.width = 3200;
        d.format.height = 400;
        assert_eq!(validate(&d), Ok(()), "ratio 8");
        let mut d = dash("x");
        d.format.preset = "Fill!".into();
        assert!(validate(&d).is_err());
        for bad in ["red", "#fff", "#12345g", "#1234567", "url(x)"] {
            let mut d = dash("x");
            d.theme.accent = bad.into();
            assert!(validate(&d).is_err(), "{bad}");
        }
    }

    #[test]
    fn pages_and_elements_bounds() {
        let mut d = dash("x");
        d.pages.clear();
        assert!(validate(&d).is_err());
        let mut d = dash("x");
        let p = d.pages[0].clone();
        d.pages = (0..12)
            .map(|i| DashPageDef {
                id: format!("p{i}"),
                ..p.clone()
            })
            .collect();
        assert_eq!(validate(&d), Ok(()));
        d.pages.push(DashPageDef {
            id: "p12".into(),
            ..p.clone()
        });
        assert!(validate(&d).is_err(), "13 pages");
        let mut d = dash("x");
        d.pages.push(p.clone());
        assert!(validate(&d).is_err(), "repeated page id");

        let mut d = dash("x");
        let e = d.pages[0].elements[0].clone();
        d.pages[0].elements = (0..80)
            .map(|i| DashElement {
                id: format!("e{i}"),
                ..e.clone()
            })
            .collect();
        assert_eq!(validate(&d), Ok(()));
        d.pages[0].elements.push(DashElement {
            id: "e80".into(),
            ..e.clone()
        });
        assert!(validate(&d).is_err(), "81 elements");
        let mut d = dash("x");
        d.pages[0].elements.push(e.clone());
        assert!(validate(&d).is_err(), "repeated element id");

        for (cols, rows, ok) in [
            (4, 2, true),
            (48, 32, true),
            (3, 12, false),
            (49, 12, false),
            (24, 1, false),
            (24, 33, false),
        ] {
            let mut d = dash("x");
            d.pages[0].cols = cols;
            d.pages[0].rows = rows;
            d.pages[0].elements[0].w = 1;
            d.pages[0].elements[0].h = 1;
            assert_eq!(validate(&d).is_ok(), ok, "{cols}×{rows}");
        }
        for (x, y, w, h, ok) in [
            (18, 9, 6, 3, true),
            (19, 0, 6, 3, false),
            (0, 10, 6, 3, false),
            (0, 0, 0, 3, false),
            (0, 0, 3, 0, false),
        ] {
            let mut d = dash("x");
            let e = &mut d.pages[0].elements[0];
            (e.x, e.y, e.w, e.h) = (x, y, w, h);
            assert_eq!(validate(&d).is_ok(), ok, "{x},{y} {w}×{h}");
        }
    }

    #[test]
    fn unknown_types_and_props_are_kept() {
        let mut d = with_props(json!({ "futureThing": 3, "mode": "x" }));
        d.pages[0].elements[0].kind = "gauge.round-v9".into();
        assert_eq!(validate(&d), Ok(()));
    }

    #[test]
    fn props_are_flat_bounded_and_inert() {
        assert_eq!(validate(&with_props(json!({}))), Ok(()));
        assert_eq!(
            validate(&with_props(json!({ "list": ["a", 1, true, null] }))),
            Ok(())
        );
        for bad in [
            json!([]),
            json!("x"),
            json!({ "nested": { "a": 1 } }),
            json!({ "list": [[1]] }),
            json!({ "list": (0..17).collect::<Vec<_>>() }),
            json!({ "n": 1e7 }),
            json!({ "n": -1_000_001 }),
            json!({ "color": "red" }),
            json!({ "color": "#abc" }),
            json!({ "text": "<img src=x onerror=alert(1)>" }),
            json!({ "text": "https://exemple.org/logo.png" }),
            json!({ "text": "data:image/png;base64,AAAA" }),
            json!({ "text": "x".repeat(201) }),
            json!({ "1bad": 1 }),
            json!({ "bad-key": 1 }),
            json!({ "__proto__": 1 }),
        ] {
            assert!(validate(&with_props(bad.clone())).is_err(), "{bad}");
        }
        let many: serde_json::Map<String, Value> =
            (0..33).map(|i| (format!("k{i}"), json!(i))).collect();
        assert!(validate(&with_props(Value::Object(many))).is_err());
    }

    #[test]
    fn size_limit() {
        // 12 pages × 80 elements of 30 keys × ~190 characters ≈ 5 MB: far too large.
        let mut d = dash("x");
        let props: serde_json::Map<String, Value> = (0..30)
            .map(|i| (format!("k{i}"), json!("x".repeat(190))))
            .collect();
        let e = DashElement {
            props: Value::Object(props),
            ..d.pages[0].elements[0].clone()
        };
        d.pages[0].elements = (0..10)
            .map(|i| DashElement {
                id: format!("e{i}"),
                ..e.clone()
            })
            .collect();
        assert_eq!(validate(&d), Ok(()), "≈ 60 KB");
        d.pages[0].elements = (0..60)
            .map(|i| DashElement {
                id: format!("e{i}"),
                ..e.clone()
            })
            .collect();
        let err = validate(&d).unwrap_err();
        assert!(err.contains("too large"), "{err}");
    }

    #[test]
    fn store_crud_persists_on_disk() {
        let dir = tmp_dir("crud");
        let store = DashStore::open(&dir).unwrap();
        assert!(store.list().is_empty());
        store.upsert(dash("gt3")).unwrap();
        store.upsert(dash("hyper")).unwrap();
        assert!(dir.join("dashes").join("gt3.json").is_file());
        let again = DashStore::open(&dir).unwrap();
        assert_eq!(again.list().len(), 2);
        assert_eq!(again.get("hyper").unwrap().name, "Dash hyper");
        assert!(again.delete("gt3").unwrap());
        assert!(!again.delete("gt3").unwrap());
        assert!(!again.delete("../gt3").unwrap());
        assert_eq!(again.clear().unwrap(), 1);
        assert!(again.list().is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn store_refuses_invalid_reserved_and_too_many() {
        let dir = tmp_dir("refuse");
        let store = DashStore::open(&dir).unwrap();
        assert!(matches!(
            store.upsert(dash("corde")),
            Err(DashError::Invalid(_))
        ));
        let mut bad = dash("x");
        bad.id = "../evil".into();
        assert!(matches!(store.upsert(bad), Err(DashError::Invalid(_))));
        assert!(!dir.join("evil.json").exists());
        for i in 0..MAX_DASHES {
            store.upsert(dash(&format!("d{i}"))).unwrap();
        }
        assert!(matches!(
            store.upsert(dash("one-more")),
            Err(DashError::Full)
        ));
        // Replacing one that exists is still possible.
        store.upsert(dash("d3")).unwrap();
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn open_skips_malformed_and_mismatched_files() {
        let dir = tmp_dir("malformed");
        let dashes = dir.join("dashes");
        fs::create_dir_all(&dashes).unwrap();
        fs::write(dashes.join("broken.json"), b"{ not json").unwrap();
        fs::write(dashes.join("empty.json"), b"{}").unwrap();
        let mut bad = dash("bad");
        bad.theme.text = "white".into();
        fs::write(dashes.join("bad.json"), serde_json::to_vec(&bad).unwrap()).unwrap();
        // A file whose name does not match its id.
        fs::write(
            dashes.join("other.json"),
            serde_json::to_vec(&dash("gt3")).unwrap(),
        )
        .unwrap();
        fs::write(
            dashes.join("corde.json"),
            serde_json::to_vec(&dash("corde")).unwrap(),
        )
        .unwrap();
        fs::write(
            dashes.join("good.json"),
            serde_json::to_vec(&dash("good")).unwrap(),
        )
        .unwrap();
        fs::write(dashes.join("notes.txt"), b"hello").unwrap();
        let store = DashStore::open(&dir).unwrap();
        let ids: Vec<String> = store.list().into_iter().map(|d| d.id).collect();
        assert_eq!(ids, vec!["good".to_string()]);
        let _ = fs::remove_dir_all(&dir);
    }
}
