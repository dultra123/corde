//! Race dashboards made in the Studio, persisted as `<data-dir>/dashes/<id>.json` (ADR 0015).
//!
//! Same shape as [`crate::layouts`]: a small synchronous write-through store with an in-memory
//! cache, writes to a temporary file then `rename`. Every definition is checked by
//! [`validate`] before it is stored, and again when it is read back from the disk: the same
//! rules as the interface (`packages/ui/src/dashdef/model.ts`), kept in sync by the tests of
//! both sides.
//!
//! A definition is pure data. Nothing in it is ever run or injected as HTML by the interface;
//! the validation keeps it so anyway: bounded sizes, closed character sets for the ids,
//! `#rrggbb` colours, no markup, no address, no nested object. A picture or a video is cited by
//! the id of a file of [`crate::dash_assets`] only; conditions and colour rules are a field of
//! the catalogue, a comparison of a closed list and a bounded number (ADR 0016); the shape of
//! the page is one of a closed list, its corner radius a bounded number (ADR 0018).

use crate::dash_assets::valid_asset_id;
use corde_protocol::{
    DashBackground, DashCondition, DashDefinition, DashElement, DashFormat, DashStyle,
    DashThreshold, DASH_DEFINITION_VERSION,
};
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
/// The shapes of a page (ADR 0018): a closed list. Left out of a definition: `rect`.
pub const SHAPES: [&str; 3] = ["rect", "rounded", "round"];
/// The format that takes the whole screen, whatever its proportions: it has no shape.
pub const FILL_PRESET: &str = "fill";
/// The formats the Studio offers: `(id, width, height, shape)`, the same list as `PRESETS` of
/// `packages/ui/src/dashdef/model.ts` (plus `custom`, any size within the bounds). A preset is
/// only a name for these values: [`validate`] does not depend on this list, and an id that is
/// not in it (a definition made by a later version) is drawn as a custom size.
pub const PRESETS: [(&str, u16, u16, Option<&str>); 11] = [
    ("phone-landscape", 852, 393, None),
    ("phone-portrait", 393, 852, None),
    ("tablet", 1024, 768, None),
    ("screen-16-9", 1920, 1080, None),
    ("wheel-strip", 800, 240, None),
    ("wheel-round", 480, 480, Some("round")),
    ("wheel-moza", 780, 248, None),
    ("square", 720, 720, None),
    ("screen-5-3", 800, 480, None),
    ("screen-3-2", 480, 320, None),
    (FILL_PRESET, 852, 393, None),
];
/// The dashboard shipped with Corde (`packages/ui/src/dashdef/builtin.ts`): never stored.
pub const BUILTIN_ID: &str = "corde";

/// Device names of Windows: a file called `con.json` or `nul.json` there is the device, not a
/// file, so such an id can never be stored (the Studio never makes one, `freshId`).
pub fn windows_device_name(id: &str) -> bool {
    corde_core::fsname::is_windows_device_name(id)
}

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
        // A media setting (`asset`, `posterAsset`…): the id of a file of the PC, or nothing.
        if is_asset_key(k) {
            match v {
                Value::String(s) if s.is_empty() || valid_asset_id(s) => {}
                Value::Null => {}
                _ => return Err(format!("{at}: not a media id")),
            }
        }
    }
    Ok(())
}

/// A setting that cites a media (`asset`, `…Asset`).
fn is_asset_key(k: &str) -> bool {
    k == "asset" || k.ends_with("Asset")
}

/// Comparisons of the conditions and colour rules: a closed list (no `<` in a definition).
pub const OPS: [&str; 4] = ["lt", "le", "ge", "gt"];
/// What a condition can test (ADR 0016).
pub const CONDITIONS: [&str; 5] = ["limiter", "pits", "yellow", "blue", "field"];
/// The fonts of the numbers (ADR 0016), all shipped with Corde.
pub const FONTS: [&str; 4] = ["condensed", "barlow", "digital", "wide"];
/// How a picture fills its box.
pub const FITS: [&str; 3] = ["contain", "cover", "fill"];
pub const MAX_THRESHOLDS: usize = 4;
pub const MAX_RADIUS: u8 = 40;
pub const MAX_BORDER_WIDTH: u8 = 8;

/// A field of the catalogue (`fuel_laps`): `[a-z0-9_]{1,40}`. The catalogue itself lives in the
/// interface; an unknown field there never holds.
fn valid_field_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= MAX_ID_LEN
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

fn check_number(what: &str, v: f64) -> Result<(), String> {
    if v.is_finite() && v.abs() <= MAX_PROP_NUMBER {
        Ok(())
    } else {
        Err(format!("{what}: number out of bounds"))
    }
}

fn check_opacity(what: &str, v: Option<f64>) -> Result<(), String> {
    match v {
        Some(o) if !(o.is_finite() && (0.0..=1.0).contains(&o)) => {
            Err(format!("{what}: opacity is 0 to 1"))
        }
        _ => Ok(()),
    }
}

fn check_opt_color(what: &str, c: &Option<String>) -> Result<(), String> {
    match c {
        Some(c) if !valid_color(c) => Err(format!("{what}: colours are #rrggbb")),
        _ => Ok(()),
    }
}

fn check_style(what: &str, s: &DashStyle) -> Result<(), String> {
    check_opt_color(&format!("{what}.bg"), &s.bg)?;
    check_opt_color(&format!("{what}.border"), &s.border)?;
    check_opacity(&format!("{what}.opacity"), s.opacity)?;
    if s.radius.is_some_and(|r| r > MAX_RADIUS) {
        return Err(format!("{what}.radius: 0 to {MAX_RADIUS}"));
    }
    if s.border_width.is_some_and(|w| w > MAX_BORDER_WIDTH) {
        return Err(format!("{what}.border_width: 0 to {MAX_BORDER_WIDTH}"));
    }
    Ok(())
}

fn check_condition(what: &str, c: &DashCondition) -> Result<(), String> {
    if !CONDITIONS.contains(&c.when.as_str()) {
        return Err(format!("{what}: unknown condition"));
    }
    if c.when == "field" {
        match (&c.field, &c.op, c.value) {
            (Some(f), Some(op), Some(v)) => {
                if !valid_field_id(f) {
                    return Err(format!("{what}: invalid field"));
                }
                if !OPS.contains(&op.as_str()) {
                    return Err(format!("{what}: unknown comparison"));
                }
                check_number(what, v)
            }
            _ => Err(format!("{what}: a field, a comparison and a number")),
        }
    } else if c.field.is_some() || c.op.is_some() || c.value.is_some() {
        Err(format!("{what}: nothing to compare"))
    } else {
        Ok(())
    }
}

fn check_thresholds(what: &str, list: &[DashThreshold]) -> Result<(), String> {
    if list.len() > MAX_THRESHOLDS {
        return Err(format!("{what}: at most {MAX_THRESHOLDS} colour rules"));
    }
    for (i, t) in list.iter().enumerate() {
        let at = format!("{what}[{i}]");
        if !OPS.contains(&t.op.as_str()) {
            return Err(format!("{at}: unknown comparison"));
        }
        check_number(&at, t.value)?;
        if !valid_color(&t.color) {
            return Err(format!("{at}: colours are #rrggbb"));
        }
    }
    Ok(())
}

fn check_background(what: &str, b: &DashBackground) -> Result<(), String> {
    check_opt_color(&format!("{what}.color"), &b.color)?;
    if let Some(a) = &b.asset {
        if !valid_asset_id(a) {
            return Err(format!("{what}.asset: not a media id"));
        }
    }
    if let Some(f) = &b.fit {
        if !FITS.contains(&f.as_str()) {
            return Err(format!("{what}.fit: contain, cover or fill"));
        }
    }
    check_opacity(&format!("{what}.opacity"), b.opacity)
}

/// The media a definition cites (its elements' settings and its pages' backgrounds).
pub fn referenced_assets(d: &DashDefinition) -> Vec<String> {
    let mut out = Vec::new();
    for p in &d.pages {
        if let Some(a) = p.background.as_ref().and_then(|b| b.asset.as_ref()) {
            out.push(a.clone());
        }
        for e in &p.elements {
            if let Value::Object(map) = &e.props {
                for (k, v) in map {
                    if let Value::String(s) = v {
                        if is_asset_key(k) && valid_asset_id(s) {
                            out.push(s.clone());
                        }
                    }
                }
            }
        }
    }
    out
}

/// The largest corner radius of a `rounded` page: half its short side.
pub fn max_shape_radius(width: u16, height: u16) -> u16 {
    width.min(height) / 2
}

/// The format of a definition: its preset id, its size, its proportions, its shape (ADR 0018).
fn check_format(f: &DashFormat) -> Result<(), String> {
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
    let shape = f.shape.as_deref().unwrap_or("rect");
    if !SHAPES.contains(&shape) {
        return Err("format: unknown shape".into());
    }
    if f.preset == FILL_PRESET && shape != "rect" {
        return Err("format: the whole screen has no shape".into());
    }
    match (shape, f.radius) {
        ("rounded", Some(r)) if (1..=max_shape_radius(f.width, f.height)).contains(&r) => Ok(()),
        ("rounded", _) => Err("format: radius is 1 to half the short side".into()),
        (_, Some(_)) => Err("format: a radius only with rounded corners".into()),
        _ => Ok(()),
    }
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
    check_props(what, &e.props)?;
    if let Some(s) = &e.style {
        check_style(&format!("{what}.style"), s)?;
    }
    if let Some(c) = &e.show {
        check_condition(&format!("{what}.show"), c)?;
    }
    if let Some(t) = &e.thresholds {
        check_thresholds(&format!("{what}.thresholds"), t)?;
    }
    Ok(())
}

/// Checks a definition against every rule of ADRs 0015, 0016 and 0018 (the size included).
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
    check_format(&d.format)?;
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
    if let Some(f) = &t.font {
        if !FONTS.contains(&f.as_str()) {
            return Err("theme.font: unknown font".into());
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
        if let Some(b) = &p.background {
            check_background(&format!("{what}.background"), b)?;
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
        if windows_device_name(&d.id) {
            return Err(DashError::Invalid(
                "this id is a device name on Windows".into(),
            ));
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
    use corde_protocol::{DashPageDef, DashTheme};
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
                shape: None,
                radius: None,
            },
            theme: DashTheme {
                background: "#000000".into(),
                panel: "#0d1116".into(),
                line: "#1f2731".into(),
                text: "#f2f5f8".into(),
                dim: "#93a0ae".into(),
                accent: "#ffb454".into(),
                font: None,
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
                    style: None,
                    show: None,
                    thresholds: None,
                }],
                background: None,
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

    /// A definition with this format (`shape` and `radius` as in the JSON, `null`: left out).
    fn shaped(
        preset: &str,
        width: u16,
        height: u16,
        shape: Option<&str>,
        radius: Option<u16>,
    ) -> DashDefinition {
        let mut d = dash("x");
        d.format = DashFormat {
            preset: preset.into(),
            width,
            height,
            shape: shape.map(Into::into),
            radius,
        };
        d
    }

    // The same cases as `packages/ui/src/dashdef/model.test.ts` (« the shape of the page »).
    #[test]
    fn page_shapes_are_a_closed_list_with_a_bounded_radius() {
        for (preset, w, h, shape, radius, ok) in [
            // No shape: a rectangle, as every definition before ADR 0018.
            ("custom", 480, 480, None, None, true),
            ("custom", 480, 480, Some("rect"), None, true),
            // Round: the circle or the ellipse inscribed in the size, no radius.
            ("wheel-round", 480, 480, Some("round"), None, true),
            ("custom", 800, 480, Some("round"), None, true),
            ("custom", 480, 480, Some("round"), Some(40), false),
            // Rounded corners: a radius from 1 to half the short side.
            ("custom", 800, 480, Some("rounded"), Some(1), true),
            ("custom", 800, 480, Some("rounded"), Some(240), true),
            ("custom", 480, 800, Some("rounded"), Some(240), true),
            ("custom", 101, 300, Some("rounded"), Some(50), true),
            ("custom", 101, 300, Some("rounded"), Some(51), false),
            ("custom", 800, 480, Some("rounded"), Some(241), false),
            ("custom", 800, 480, Some("rounded"), Some(0), false),
            ("custom", 800, 480, Some("rounded"), None, false),
            // A radius without rounded corners.
            ("custom", 800, 480, None, Some(20), false),
            ("custom", 800, 480, Some("rect"), Some(20), false),
            // Not in the list.
            ("custom", 480, 480, Some("circle"), None, false),
            ("custom", 480, 480, Some("Round"), None, false),
            ("custom", 480, 480, Some(""), None, false),
            ("custom", 480, 480, Some("polygon(0 0)"), None, false),
            ("custom", 480, 480, Some("url(x)"), None, false),
            // The whole screen has no shape of its own.
            ("fill", 852, 393, None, None, true),
            ("fill", 852, 393, Some("rect"), None, true),
            ("fill", 852, 393, Some("round"), None, false),
            ("fill", 852, 393, Some("rounded"), Some(20), false),
            // A shape does not loosen the bounds of the size.
            ("custom", 99, 480, Some("round"), None, false),
            ("custom", 4000, 400, Some("round"), None, false),
        ] {
            let d = shaped(preset, w, h, shape, radius);
            assert_eq!(
                validate(&d).is_ok(),
                ok,
                "{preset} {w}×{h} {shape:?} {radius:?}"
            );
        }
        assert_eq!(max_shape_radius(800, 480), 240);
        assert_eq!(max_shape_radius(101, 300), 50);
    }

    #[test]
    fn every_preset_is_valid_and_an_unknown_one_is_a_custom_size() {
        for (id, w, h, shape) in PRESETS {
            assert_eq!(validate(&shaped(id, w, h, shape, None)), Ok(()), "{id}");
        }
        let ids: HashSet<&str> = PRESETS.iter().map(|p| p.0).collect();
        assert_eq!(ids.len(), PRESETS.len(), "no preset twice");
        // A preset of a later version: kept, drawn at its size.
        assert_eq!(
            validate(&shaped("wheel-oval-2030", 600, 400, None, None)),
            Ok(())
        );
    }

    #[test]
    fn shapes_in_json() {
        // Read: the fields of ADR 0018, `null` as left out.
        let mut json = serde_json::to_value(dash("x")).unwrap();
        json["format"] =
            json!({ "preset": "wheel-round", "width": 480, "height": 480, "shape": "round" });
        let d: DashDefinition = serde_json::from_value(json.clone()).unwrap();
        assert_eq!(d.format.shape.as_deref(), Some("round"));
        assert_eq!(validate(&d), Ok(()));
        // Written back as it came.
        assert_eq!(serde_json::to_value(&d).unwrap(), json);
        json["format"] = json!({ "preset": "custom", "width": 800, "height": 480, "shape": "rounded", "radius": 24 });
        let d: DashDefinition = serde_json::from_value(json.clone()).unwrap();
        assert_eq!(
            (d.format.shape.as_deref(), d.format.radius),
            (Some("rounded"), Some(24))
        );
        assert_eq!(validate(&d), Ok(()));
        assert_eq!(serde_json::to_value(&d).unwrap(), json);
        json["format"] = json!({ "preset": "custom", "width": 800, "height": 480, "shape": null, "radius": null });
        let d: DashDefinition = serde_json::from_value(json.clone()).unwrap();
        assert_eq!((d.format.shape.as_deref(), d.format.radius), (None, None));
        assert_eq!(validate(&d), Ok(()));
        // Not the right kind of value: refused when read.
        for bad in [
            json!({ "preset": "custom", "width": 800, "height": 480, "shape": 3 }),
            json!({ "preset": "custom", "width": 800, "height": 480, "shape": ["round"] }),
            json!({ "preset": "custom", "width": 800, "height": 480, "shape": "rounded", "radius": 12.5 }),
            json!({ "preset": "custom", "width": 800, "height": 480, "shape": "rounded", "radius": -4 }),
            json!({ "preset": "custom", "width": 800, "height": 480, "shape": "rounded", "radius": "12" }),
            json!({ "preset": "custom", "width": 800, "height": 480, "shape": "rounded", "radius": 70000 }),
        ] {
            json["format"] = bad.clone();
            assert!(
                serde_json::from_value::<DashDefinition>(json.clone()).is_err(),
                "{bad}"
            );
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

    fn asset(ext: &str) -> String {
        format!("{}.{ext}", "0123456789abcdef".repeat(4))
    }

    #[test]
    fn media_settings_cite_an_id_only() {
        for ok in [
            json!({ "asset": asset("png") }),
            json!({ "asset": asset("webm"), "fit": "cover" }),
            json!({ "asset": "" }),
            json!({ "asset": null }),
            json!({ "posterAsset": asset("jpg") }),
        ] {
            assert_eq!(validate(&with_props(ok.clone())), Ok(()), "{ok}");
        }
        for bad in [
            json!({ "asset": "https://exemple.org/logo.png" }),
            json!({ "asset": "file:///C:/Users/x/logo.png" }),
            json!({ "asset": "C:\\Users\\x\\logo.png" }),
            json!({ "asset": "../dashes/x.json" }),
            json!({ "asset": "logo.png" }),
            json!({ "asset": asset("svg") }),
            json!({ "asset": "data:image/png;base64,AAAA" }),
            json!({ "asset": 3 }),
            json!({ "asset": true }),
            json!({ "posterAsset": "x" }),
        ] {
            assert!(validate(&with_props(bad.clone())).is_err(), "{bad}");
        }
        let mut d = with_props(json!({ "asset": asset("gif") }));
        d.pages[0].background = Some(DashBackground {
            color: Some("#101820".into()),
            asset: Some(asset("mp4")),
            fit: Some("contain".into()),
            opacity: Some(0.5),
        });
        assert_eq!(validate(&d), Ok(()));
        let mut refs = referenced_assets(&d);
        refs.sort();
        assert_eq!(refs, vec![asset("gif"), asset("mp4")]);
    }

    #[test]
    fn page_backgrounds() {
        let base = DashBackground {
            color: None,
            asset: None,
            fit: None,
            opacity: None,
        };
        for (b, ok) in [
            (base.clone(), true),
            (
                DashBackground {
                    color: Some("#000000".into()),
                    ..base.clone()
                },
                true,
            ),
            (
                DashBackground {
                    opacity: Some(0.0),
                    ..base.clone()
                },
                true,
            ),
            (
                DashBackground {
                    color: Some("red".into()),
                    ..base.clone()
                },
                false,
            ),
            (
                DashBackground {
                    asset: Some("https://x.org/a.png".into()),
                    ..base.clone()
                },
                false,
            ),
            (
                DashBackground {
                    asset: Some(asset("svg")),
                    ..base.clone()
                },
                false,
            ),
            (
                DashBackground {
                    fit: Some("tile".into()),
                    ..base.clone()
                },
                false,
            ),
            (
                DashBackground {
                    opacity: Some(1.5),
                    ..base.clone()
                },
                false,
            ),
            (
                DashBackground {
                    opacity: Some(-0.1),
                    ..base.clone()
                },
                false,
            ),
        ] {
            let mut d = dash("x");
            d.pages[0].background = Some(b.clone());
            assert_eq!(validate(&d).is_ok(), ok, "{b:?}");
        }
    }

    #[test]
    fn element_styles() {
        let ok = DashStyle {
            bg: Some("#112233".into()),
            opacity: Some(0.8),
            radius: Some(40),
            border: Some("#ffffff".into()),
            border_width: Some(8),
        };
        let mut d = dash("x");
        d.pages[0].elements[0].style = Some(ok.clone());
        assert_eq!(validate(&d), Ok(()));
        for bad in [
            DashStyle {
                bg: Some("rgba(0,0,0,0)".into()),
                ..ok.clone()
            },
            DashStyle {
                border: Some("url(x)".into()),
                ..ok.clone()
            },
            DashStyle {
                opacity: Some(2.0),
                ..ok.clone()
            },
            DashStyle {
                opacity: Some(f64::NAN),
                ..ok.clone()
            },
            DashStyle {
                radius: Some(41),
                ..ok.clone()
            },
            DashStyle {
                border_width: Some(9),
                ..ok.clone()
            },
        ] {
            let mut d = dash("x");
            d.pages[0].elements[0].style = Some(bad.clone());
            assert!(validate(&d).is_err(), "{bad:?}");
        }
    }

    fn cond(
        when: &str,
        field: Option<&str>,
        op: Option<&str>,
        value: Option<f64>,
    ) -> DashCondition {
        DashCondition {
            when: when.into(),
            field: field.map(Into::into),
            op: op.map(Into::into),
            value,
        }
    }

    #[test]
    fn conditions_are_a_closed_list() {
        for c in [
            cond("limiter", None, None, None),
            cond("pits", None, None, None),
            cond("yellow", None, None, None),
            cond("blue", None, None, None),
            cond("field", Some("fuel_laps"), Some("lt"), Some(2.0)),
            cond("field", Some("tyre_temp_fl"), Some("ge"), Some(-1e6)),
        ] {
            let mut d = dash("x");
            d.pages[0].elements[0].show = Some(c.clone());
            assert_eq!(validate(&d), Ok(()), "{c:?}");
        }
        for c in [
            cond("always", None, None, None),
            cond("script", None, None, None),
            cond("LIMITER", None, None, None),
            cond("limiter", Some("rpm"), None, None),
            cond("field", None, Some("lt"), Some(2.0)),
            cond("field", Some("fuel_laps"), None, Some(2.0)),
            cond("field", Some("fuel_laps"), Some("lt"), None),
            cond("field", Some("fuel_laps"), Some("<"), Some(2.0)),
            cond("field", Some("fuel_laps"), Some("eq"), Some(2.0)),
            cond("field", Some("fuel laps"), Some("lt"), Some(2.0)),
            cond("field", Some("x.y"), Some("lt"), Some(2.0)),
            cond("field", Some("fuel_laps"), Some("lt"), Some(1e7)),
            cond("field", Some("fuel_laps"), Some("lt"), Some(f64::INFINITY)),
        ] {
            let mut d = dash("x");
            d.pages[0].elements[0].show = Some(c.clone());
            assert!(validate(&d).is_err(), "{c:?}");
        }
    }

    #[test]
    fn colour_rules() {
        let rule = |op: &str, value: f64, color: &str| DashThreshold {
            op: op.into(),
            value,
            color: color.into(),
        };
        let mut d = dash("x");
        d.pages[0].elements[0].thresholds = Some(vec![
            rule("lt", 1.0, "#ff0000"),
            rule("le", 2.0, "#ff8800"),
            rule("ge", 100.0, "#00ff00"),
            rule("gt", 120.0, "#0000ff"),
        ]);
        assert_eq!(validate(&d), Ok(()));
        d.pages[0].elements[0]
            .thresholds
            .as_mut()
            .unwrap()
            .push(rule("lt", 0.0, "#000000"));
        assert!(validate(&d).is_err(), "5 rules");
        for bad in [
            rule(">", 1.0, "#ff0000"),
            rule("lt", 1.0, "red"),
            rule("lt", 1e9, "#ff0000"),
            rule("lt", f64::NAN, "#ff0000"),
        ] {
            let mut d = dash("x");
            d.pages[0].elements[0].thresholds = Some(vec![bad.clone()]);
            assert!(validate(&d).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn fonts_are_a_closed_list() {
        for f in FONTS {
            let mut d = dash("x");
            d.theme.font = Some(f.into());
            assert_eq!(validate(&d), Ok(()), "{f}");
        }
        for f in ["Comic Sans", "url(x)", "", "digital2"] {
            let mut d = dash("x");
            d.theme.font = Some(f.into());
            assert!(validate(&d).is_err(), "{f}");
        }
    }

    #[test]
    fn a_definition_of_1_2_reads_the_same() {
        // Without any of the fields of ADR 0016: valid, and written back without them.
        let d = dash("x");
        let json = serde_json::to_value(&d).unwrap();
        for k in ["style", "show", "thresholds"] {
            assert!(json["pages"][0]["elements"][0].get(k).is_none(), "{k}");
        }
        assert!(json["pages"][0].get("background").is_none());
        assert!(json["theme"].get("font").is_none());
        // Nor those of ADR 0018: a definition without a shape is written back without one.
        assert!(json["format"].get("shape").is_none());
        assert!(json["format"].get("radius").is_none());
        assert_eq!(
            json["format"],
            json!({ "preset": "phone-landscape", "width": 852, "height": 393 })
        );
        let back: DashDefinition = serde_json::from_value(json).unwrap();
        assert_eq!(back, d);
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
    fn store_refuses_windows_device_names() {
        let dir = tmp_dir("devices");
        let store = DashStore::open(&dir).unwrap();
        for id in ["con", "prn", "aux", "nul", "com1", "lpt9"] {
            assert!(windows_device_name(id), "{id}");
            assert!(
                matches!(store.upsert(dash(id)), Err(DashError::Invalid(_))),
                "{id}"
            );
        }
        for id in ["con-2", "console", "com", "com10", "nul-1", "lpt"] {
            assert!(!windows_device_name(id), "{id}");
        }
        assert!(store.upsert(dash("con-2")).is_ok());
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
