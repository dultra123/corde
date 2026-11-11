//! `.svm` setup files (`UserData/player/Settings/<Track>/<name>.svm`).
//!
//! Format (rFactor 2 heritage, see `docs/research/01-telemetrie-lmu.md` §3):
//!
//! ```text
//! VehicleClassSetting="BMW_M_Hybrid Hypercar WEC2025"
//! UpgradeSetting=(103,0,0,0)
//! //VEH=C:\...\15_25_WRT_A1285A64.VEH
//! //Note: settings commented out if using the default
//!
//! [GENERAL]
//! Notes=""
//! Symmetric=1
//! FuelSetting=89//0.94
//! VirtualEnergySetting=50//50% (12.5 laps)
//! [BASIC]
//! Downforce=0.500000
//! ```
//!
//! Every value is an **index** into the tables of the (encrypted) `.HDV`, and
//! LMU writes the displayed label after `//`. The parser keeps both, preserves
//! the order of sections and keys (`indexmap`), the comment / blank lines and
//! the line ending (CRLF as written by the game), so that
//! [`Setup::to_string`] reproduces an untouched file byte for byte.
//! [`diff`] compares two setups value by value.

use indexmap::IndexMap;
use std::fmt;
use std::path::Path;
use thiserror::Error;

/// Sections in the order the game writes them.
pub const KNOWN_SECTIONS: &[&str] = &[
    "GENERAL",
    "LEFTFENDER",
    "RIGHTFENDER",
    "FRONTWING",
    "REARWING",
    "BODYAERO",
    "SUSPENSION",
    "CONTROLS",
    "ENGINE",
    "DRIVELINE",
    "FRONTLEFT",
    "FRONTRIGHT",
    "REARLEFT",
    "REARRIGHT",
    "BASIC",
];

/// Line ending of a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineEnding {
    /// `\r\n` — what the game writes.
    #[default]
    Crlf,
    Lf,
}

impl LineEnding {
    pub fn as_str(self) -> &'static str {
        match self {
            LineEnding::Crlf => "\r\n",
            LineEnding::Lf => "\n",
        }
    }
}

/// The numeric part of a setting line.
#[derive(Debug, Clone, PartialEq)]
pub enum SetupNumber {
    /// `FuelSetting=89` — the usual case (index into the car's table).
    Int(i64),
    /// `Downforce=0.500000` — `[BASIC]` sliders.
    Float(f64),
    /// Not a number (`Notes=""`): kept verbatim.
    Text(String),
}

impl SetupNumber {
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            SetupNumber::Int(i) => Some(*i as f64),
            SetupNumber::Float(f) => Some(*f),
            SetupNumber::Text(_) => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            SetupNumber::Int(i) => Some(*i),
            SetupNumber::Float(f) => Some(f.round() as i64),
            SetupNumber::Text(_) => None,
        }
    }

    fn parse(raw: &str) -> SetupNumber {
        let t = raw.trim();
        if let Ok(i) = t.parse::<i64>() {
            SetupNumber::Int(i)
        } else if let Ok(f) = t.parse::<f64>() {
            if t.chars()
                .all(|c| c.is_ascii_digit() || matches!(c, '.' | '-' | '+' | 'e' | 'E'))
            {
                SetupNumber::Float(f)
            } else {
                SetupNumber::Text(raw.to_string())
            }
        } else {
            SetupNumber::Text(raw.to_string())
        }
    }
}

impl fmt::Display for SetupNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SetupNumber::Int(i) => write!(f, "{i}"),
            SetupNumber::Float(x) => write!(f, "{x:.6}"),
            SetupNumber::Text(s) => f.write_str(s),
        }
    }
}

/// One `Key=index//display` entry.
#[derive(Debug, Clone, PartialEq)]
pub struct SetupValue {
    pub index: SetupNumber,
    /// Text after `//` (`Some("")` when the separator is present but empty).
    pub display: Option<String>,
    /// Original spelling of the number, used to rewrite the file identically
    /// as long as the value was not changed (`"0.500000"` vs `"0.5"`).
    raw: Option<String>,
}

impl SetupValue {
    pub fn int(index: i64) -> Self {
        Self {
            index: SetupNumber::Int(index),
            display: None,
            raw: None,
        }
    }

    pub fn float(value: f64) -> Self {
        Self {
            index: SetupNumber::Float(value),
            display: None,
            raw: None,
        }
    }

    pub fn text(value: impl Into<String>) -> Self {
        Self {
            index: SetupNumber::Text(value.into()),
            display: None,
            raw: None,
        }
    }

    pub fn with_display(mut self, display: impl Into<String>) -> Self {
        self.display = Some(display.into());
        self
    }

    /// Parse the right-hand side of `Key=…`. A quoted text (`Notes="…"`) is
    /// kept whole: a `//` inside the driver's notes is not a label separator.
    pub fn parse(rhs: &str) -> Self {
        if rhs.trim_start().starts_with('"') {
            return Self {
                index: SetupNumber::Text(rhs.to_string()),
                display: None,
                raw: Some(rhs.to_string()),
            };
        }
        let (num, display) = match rhs.find("//") {
            Some(pos) => (&rhs[..pos], Some(rhs[pos + 2..].to_string())),
            None => (rhs, None),
        };
        Self {
            index: SetupNumber::parse(num),
            display,
            raw: Some(num.to_string()),
        }
    }

    /// Change the index (drops the display label, which no longer matches).
    pub fn set_index(&mut self, index: SetupNumber) {
        self.index = index;
        self.display = None;
        self.raw = None;
    }

    pub fn as_i64(&self) -> Option<i64> {
        self.index.as_i64()
    }

    pub fn as_f64(&self) -> Option<f64> {
        self.index.as_f64()
    }

    /// Change the index and write a new label after `//` (`None`: no label).
    pub fn set_index_with_display(&mut self, index: i64, display: Option<String>) {
        self.index = SetupNumber::Int(index);
        self.display = display;
        self.raw = None;
    }

    /// The label after `//`, trimmed (`""` when there is none).
    pub fn label(&self) -> &str {
        self.display.as_deref().map(str::trim).unwrap_or("")
    }

    fn number_text(&self) -> String {
        match &self.raw {
            Some(raw) if SetupNumber::parse(raw) == self.index => raw.clone(),
            _ => self.index.to_string(),
        }
    }
}

impl fmt::Display for SetupValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.number_text())?;
        if let Some(d) = &self.display {
            f.write_str("//")?;
            f.write_str(d)?;
        }
        Ok(())
    }
}

/// A `[SECTION]` with its values in file order.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Section {
    pub values: IndexMap<String, SetupValue>,
    /// Comment / blank / unparseable lines, with the number of values that
    /// precede them (so that the writer puts them back at the same place).
    other_lines: Vec<(usize, String)>,
}

impl Section {
    pub fn get(&self, key: &str) -> Option<&SetupValue> {
        self.values.get(key)
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut SetupValue> {
        self.values.get_mut(key)
    }

    pub fn set(&mut self, key: impl Into<String>, value: SetupValue) {
        self.values.insert(key.into(), value);
    }

    /// Lines that are not `Key=value` (comments such as `//FuelSetting=…`).
    pub fn comments(&self) -> impl Iterator<Item = &str> {
        self.other_lines.iter().map(|(_, l)| l.as_str())
    }
}

/// Header lines before the first section (`VehicleClassSetting`,
/// `UpgradeSetting`, `//` comments, blanks), kept verbatim.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SetupHeader {
    pub lines: Vec<String>,
}

impl SetupHeader {
    fn value_of(&self, key: &str) -> Option<&str> {
        self.lines.iter().find_map(|l| {
            let (k, v) = l.split_once('=')?;
            (k.trim() == key).then_some(v.trim())
        })
    }

    fn set_value(&mut self, key: &str, value: &str) {
        let line = format!("{key}={value}");
        if let Some(l) = self.lines.iter_mut().find(|l| {
            l.split_once('=')
                .map(|(k, _)| k.trim() == key)
                .unwrap_or(false)
        }) {
            *l = line;
        } else {
            self.lines.insert(0, line);
        }
    }

    /// `VehicleClassSetting="…"` without the quotes.
    pub fn vehicle_class_setting(&self) -> Option<String> {
        self.value_of("VehicleClassSetting")
            .map(|v| v.trim_matches('"').to_string())
    }

    pub fn set_vehicle_class_setting(&mut self, v: &str) {
        self.set_value("VehicleClassSetting", &format!("\"{v}\""));
    }

    /// `UpgradeSetting=(103,0,0,0)` → `[103, 0, 0, 0]`.
    pub fn upgrade_setting(&self) -> Option<Vec<i64>> {
        let v = self.value_of("UpgradeSetting")?;
        let inner = v.trim().trim_start_matches('(').trim_end_matches(')');
        inner
            .split(',')
            .map(|s| s.trim().parse::<i64>().ok())
            .collect()
    }

    pub fn set_upgrade_setting(&mut self, v: &[i64]) {
        let joined: Vec<String> = v.iter().map(|i| i.to_string()).collect();
        self.set_value("UpgradeSetting", &format!("({})", joined.join(",")));
    }

    /// `//VEH=…` path when present.
    pub fn veh_path(&self) -> Option<String> {
        self.lines
            .iter()
            .find_map(|l| l.strip_prefix("//VEH=").map(|s| s.trim().to_string()))
    }

    pub fn comments(&self) -> impl Iterator<Item = &str> {
        self.lines
            .iter()
            .filter(|l| l.starts_with("//"))
            .map(|l| l.as_str())
    }
}

/// A parsed `.svm` file.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Setup {
    pub header: SetupHeader,
    /// Sections in file order, keys without brackets (`"GENERAL"`).
    pub sections: IndexMap<String, Section>,
    pub line_ending: LineEnding,
    /// Whether the original text ended with a line break (the game's files do).
    pub trailing_newline: bool,
}

#[derive(Debug, Error)]
pub enum SvmError {
    #[error("cannot read {path}: {source}")]
    Read {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("cannot write {path}: {source}")]
    Write {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("{path} is not a setup file (no [SECTION] found)")]
    NoSection { path: String },
}

impl Setup {
    /// Parse the text of a `.svm` file. Never fails: unknown lines are kept
    /// verbatim. Use [`Setup::is_plausible`] to check the result.
    pub fn parse(text: &str) -> Self {
        let line_ending = if text.contains("\r\n") {
            LineEnding::Crlf
        } else {
            LineEnding::Lf
        };
        let trailing_newline = text.ends_with('\n');
        let mut setup = Setup {
            header: SetupHeader::default(),
            sections: IndexMap::new(),
            line_ending,
            trailing_newline,
        };
        let mut current: Option<String> = None;
        let mut lines: Vec<&str> = text
            .split('\n')
            .map(|l| l.strip_suffix('\r').unwrap_or(l))
            .collect();
        if trailing_newline {
            lines.pop(); // the empty piece after the final line break
        }
        for line in lines {
            let trimmed = line.trim();
            if trimmed.starts_with('[') && trimmed.ends_with(']') && trimmed.len() >= 2 {
                let name = trimmed[1..trimmed.len() - 1].trim().to_string();
                setup.sections.entry(name.clone()).or_default();
                current = Some(name);
                continue;
            }
            match &current {
                None => setup.header.lines.push(line.to_string()),
                Some(name) => {
                    let section = setup.sections.get_mut(name).expect("section just inserted");
                    let is_comment = trimmed.starts_with("//") || trimmed.is_empty();
                    match (!is_comment).then(|| line.split_once('=')).flatten() {
                        Some((key, rhs)) if !key.trim().is_empty() => {
                            section
                                .values
                                .insert(key.trim().to_string(), SetupValue::parse(rhs));
                        }
                        _ => section
                            .other_lines
                            .push((section.values.len(), line.to_string())),
                    }
                }
            }
        }
        setup
    }

    /// Read and parse a file.
    pub fn read(path: impl AsRef<Path>) -> Result<Self, SvmError> {
        let path = path.as_ref();
        let bytes = std::fs::read(path).map_err(|source| SvmError::Read {
            path: path.display().to_string(),
            source,
        })?;
        // The game writes ASCII; be tolerant with Latin-1 notes.
        let text = String::from_utf8_lossy(&bytes);
        let setup = Self::parse(&text);
        if setup.sections.is_empty() {
            return Err(SvmError::NoSection {
                path: path.display().to_string(),
            });
        }
        Ok(setup)
    }

    /// Write the file with its original line ending.
    pub fn write(&self, path: impl AsRef<Path>) -> Result<(), SvmError> {
        let path = path.as_ref();
        std::fs::write(path, self.to_string()).map_err(|source| SvmError::Write {
            path: path.display().to_string(),
            source,
        })
    }

    /// `true` when the text looks like a setup written by the game.
    pub fn is_plausible(&self) -> bool {
        self.header.vehicle_class_setting().is_some() && self.sections.contains_key("GENERAL")
    }

    pub fn section(&self, name: &str) -> Option<&Section> {
        self.sections.get(name)
    }

    pub fn section_mut(&mut self, name: &str) -> &mut Section {
        self.sections.entry(name.to_string()).or_default()
    }

    /// `setup.get("CONTROLS", "RearBrakeSetting")`.
    pub fn get(&self, section: &str, key: &str) -> Option<&SetupValue> {
        self.sections.get(section)?.values.get(key)
    }

    pub fn set(&mut self, section: &str, key: &str, value: SetupValue) {
        self.section_mut(section).set(key, value);
    }

    /// Serialise with a given line ending.
    pub fn to_string_with(&self, eol: LineEnding) -> String {
        let nl = eol.as_str();
        let mut out = String::new();
        for l in &self.header.lines {
            out.push_str(l);
            out.push_str(nl);
        }
        for (name, section) in &self.sections {
            out.push('[');
            out.push_str(name);
            out.push(']');
            out.push_str(nl);
            let mut others = section.other_lines.iter().peekable();
            for (i, (key, value)) in section.values.iter().enumerate() {
                while let Some((pos, line)) = others.peek() {
                    if *pos <= i {
                        out.push_str(line);
                        out.push_str(nl);
                        others.next();
                    } else {
                        break;
                    }
                }
                out.push_str(key);
                out.push('=');
                out.push_str(&value.to_string());
                out.push_str(nl);
            }
            for (_, line) in others {
                out.push_str(line);
                out.push_str(nl);
            }
        }
        if !self.trailing_newline && out.ends_with(nl) {
            out.truncate(out.len() - nl.len());
        }
        out
    }
}

impl fmt::Display for Setup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_string_with(self.line_ending))
    }
}

/// Labels the game writes for a parameter the car does not let the driver
/// change (compared case-insensitively, accents folded for « Détaché »).
pub const NON_ADJUSTABLE_LABELS: &[&str] =
    &["non-adjustable", "n/a", "fixed", "detache", "detached"];

/// `false` when the game marks the value as not adjustable on this car
/// (`Non-adjustable`, `N/A`, `Fixed`, `Détaché` after `//`). Such a value is
/// never changed by Corde. A value without label is assumed adjustable.
pub fn is_adjustable(value: &SetupValue) -> bool {
    if matches!(value.index, SetupNumber::Text(_)) {
        return false;
    }
    let label: String = value
        .label()
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'é' | 'è' | 'ê' => 'e',
            other => other,
        })
        .collect();
    !NON_ADJUSTABLE_LABELS.contains(&label.as_str())
}

impl Setup {
    /// `VehicleClassSetting` of the header (`"BMW_M4_LMGT3 GT3 WEC2026"`).
    pub fn vehicle_class(&self) -> Option<String> {
        self.header.vehicle_class_setting()
    }

    /// `Notes="…"` of `[GENERAL]`, without the quotes (`None` when absent).
    pub fn notes(&self) -> Option<String> {
        let v = self.get("GENERAL", "Notes")?;
        match &v.index {
            SetupNumber::Text(t) => {
                let t = t.trim();
                let t = t.strip_prefix('"').unwrap_or(t);
                Some(t.strip_suffix('"').unwrap_or(t).to_string())
            }
            other => Some(other.to_string()),
        }
    }

    /// Replace the notes. Quotes and line breaks are removed (they would end
    /// the value), the rest is written as is.
    pub fn set_notes(&mut self, notes: &str) {
        let clean: String = notes
            .chars()
            .filter(|c| !matches!(c, '"' | '\r' | '\n'))
            .collect();
        let text = format!("\"{clean}\"");
        match self.section_mut("GENERAL").get_mut("Notes") {
            Some(v) => {
                v.index = SetupNumber::Text(text.clone());
                v.display = None;
                v.raw = Some(text);
            }
            None => {
                let general = self.section_mut("GENERAL");
                let mut values = IndexMap::new();
                values.insert("Notes".to_string(), SetupValue::text(text));
                for (k, v) in general.values.drain(..) {
                    values.insert(k, v);
                }
                // Comment lines keep their place relative to the values.
                for (pos, _) in general.other_lines.iter_mut() {
                    *pos += 1;
                }
                general.values = values;
            }
        }
    }

    /// Every `Key=value` of the file in order: `(section, key, value)`.
    pub fn values(&self) -> impl Iterator<Item = (&str, &str, &SetupValue)> {
        self.sections.iter().flat_map(|(name, section)| {
            section
                .values
                .iter()
                .map(move |(k, v)| (name.as_str(), k.as_str(), v))
        })
    }
}

/// One difference between two setups.
#[derive(Debug, Clone, PartialEq)]
pub struct SetupDiff {
    pub section: String,
    pub key: String,
    /// `None` when the key is missing on that side.
    pub a: Option<SetupValue>,
    pub b: Option<SetupValue>,
}

impl fmt::Display for SetupDiff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let show = |v: &Option<SetupValue>| match v {
            Some(v) => match &v.display {
                Some(d) if !d.is_empty() => format!("{} ({})", v.index, d),
                _ => v.index.to_string(),
            },
            None => "—".to_string(),
        };
        write!(
            f,
            "[{}] {}: {} → {}",
            self.section,
            self.key,
            show(&self.a),
            show(&self.b)
        )
    }
}

/// Values that differ between `a` and `b` (by index or display label), in
/// the order of `a`'s sections then `b`'s extra ones.
pub fn diff(a: &Setup, b: &Setup) -> Vec<SetupDiff> {
    let mut out = Vec::new();
    let empty = Section::default();
    let mut names: Vec<&String> = a.sections.keys().collect();
    for n in b.sections.keys() {
        if !names.contains(&n) {
            names.push(n);
        }
    }
    for name in names {
        let sa = a.sections.get(name).unwrap_or(&empty);
        let sb = b.sections.get(name).unwrap_or(&empty);
        let mut keys: Vec<&String> = sa.values.keys().collect();
        for k in sb.values.keys() {
            if !keys.contains(&k) {
                keys.push(k);
            }
        }
        for key in keys {
            let va = sa.values.get(key);
            let vb = sb.values.get(key);
            let same = match (va, vb) {
                (Some(x), Some(y)) => x.index == y.index && x.display == y.display,
                (None, None) => true,
                _ => false,
            };
            if !same {
                out.push(SetupDiff {
                    section: name.clone(),
                    key: key.clone(),
                    a: va.cloned(),
                    b: vb.cloned(),
                });
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sample written after the research notes (§3): structure and labels of
    /// a Hypercar file, shortened.
    const SAMPLE_LF: &str = r#"VehicleClassSetting="BMW_M_Hybrid Hypercar WEC2025"
UpgradeSetting=(103,0,0,0)
//VEH=C:\Games\Le Mans Ultimate\Installed\Vehicles\BMW_M_Hybrid_V8_2023\1.19\15_25_WRT_A1285A64.VEH
//UpgradeClass=
//Tyre Restrictions=1
//AI Tweaks=3
//Atmos Conditions=3
//Note: settings commented out if using the default

[GENERAL]
Notes=""
Symmetric=1
FuelSetting=89//0.94
FuelCapacitySetting=0//47.0L (13.3 laps)
VirtualEnergySetting=50//50% (12.5 laps)
NumPitstopsSetting=0//0
//Pitstop1Setting=0//N/A
[FRONTWING]
FWSetting=3//4
[REARWING]
RWSetting=12//P13
[BODYAERO]
WaterRadiatorSetting=2//3
OilRadiatorSetting=2//3
BrakeDuctSetting=3//4
BrakeDuctRearSetting=3//4
[SUSPENSION]
FrontAntiSwaySetting=4//5
RearAntiSwaySetting=2//3
Front3rdSpringSetting=10//110 N/mm
[CONTROLS]
SteerLockSetting=14//540 deg (15:1)
RearBrakeSetting=45//55.0:45.0
BrakeMigrationSetting=2//2
BrakePressureSetting=100//100%
TCSetting=4//4
ABSSetting=6//6 (Balanced)
TractionControlMapSetting=3//3
[ENGINE]
RevLimitSetting=0//100%
RegenerationMapSetting=4//170kW
ElectricMotorMapSetting=1//30kW
EngineMixtureSetting=1//Normal
[DRIVELINE]
FinalDriveSetting=3//3.80
Gear1Setting=5//2.95
Gear7Setting=11//1.05
DiffPumpSetting=0//Non-adjustable
[FRONTLEFT]
CamberSetting=24//-3.2 deg
PressureSetting=18//150 kPa
SpringSetting=12//130 N/mm
RideHeightSetting=8//45 mm
SlowBumpSetting=6//6 clicks
CompoundSetting=1//Medium
[FRONTRIGHT]
CamberSetting=24//-3.2 deg
PressureSetting=18//150 kPa
[REARLEFT]
CamberSetting=18//-2.4 deg
PressureSetting=16//148 kPa
[REARRIGHT]
CamberSetting=18//-2.4 deg
PressureSetting=16//148 kPa
[BASIC]
Downforce=0.500000
Balance=0.500000
Ride=0.500000
Gearing=0.500000
Custom=1
"#;

    fn sample_crlf() -> String {
        SAMPLE_LF.replace('\n', "\r\n")
    }

    #[test]
    fn parses_header_sections_and_values() {
        let s = Setup::parse(SAMPLE_LF);
        assert!(s.is_plausible());
        assert_eq!(s.line_ending, LineEnding::Lf);
        assert_eq!(
            s.header.vehicle_class_setting().as_deref(),
            Some("BMW_M_Hybrid Hypercar WEC2025")
        );
        assert_eq!(s.header.upgrade_setting(), Some(vec![103, 0, 0, 0]));
        assert!(s.header.veh_path().unwrap().ends_with(".VEH"));
        assert_eq!(s.header.comments().count(), 6);
        let names: Vec<&String> = s.sections.keys().collect();
        assert_eq!(names[0], "GENERAL");
        assert_eq!(names.last().unwrap().as_str(), "BASIC");
        assert_eq!(s.sections.len(), 13);

        let fuel = s.get("GENERAL", "FuelSetting").unwrap();
        assert_eq!(fuel.index, SetupNumber::Int(89));
        assert_eq!(fuel.display.as_deref(), Some("0.94"));
        let notes = s.get("GENERAL", "Notes").unwrap();
        assert_eq!(notes.index, SetupNumber::Text("\"\"".into()));
        assert_eq!(notes.display, None);
        let df = s.get("BASIC", "Downforce").unwrap();
        assert_eq!(df.index, SetupNumber::Float(0.5));
        assert_eq!(
            s.get("CONTROLS", "RearBrakeSetting")
                .unwrap()
                .display
                .as_deref(),
            Some("55.0:45.0")
        );
        // commented-out line kept as a comment of the section
        let general = s.section("GENERAL").unwrap();
        assert_eq!(
            general.comments().collect::<Vec<_>>(),
            vec!["//Pitstop1Setting=0//N/A"]
        );
        assert_eq!(general.values.len(), 6);
    }

    #[test]
    fn roundtrip_is_byte_identical() {
        let crlf = sample_crlf();
        let s = Setup::parse(&crlf);
        assert_eq!(s.line_ending, LineEnding::Crlf);
        assert!(s.trailing_newline);
        assert_eq!(s.to_string(), crlf, "CRLF round-trip");

        let s2 = Setup::parse(SAMPLE_LF);
        assert_eq!(s2.to_string(), SAMPLE_LF, "LF round-trip");

        // converting the line ending on purpose
        assert_eq!(s2.to_string_with(LineEnding::Crlf), crlf);

        // no trailing newline
        let trimmed = SAMPLE_LF.trim_end_matches('\n');
        let s3 = Setup::parse(trimmed);
        assert!(!s3.trailing_newline);
        assert_eq!(s3.to_string(), trimmed);
    }

    #[test]
    fn edits_and_diff() {
        let a = Setup::parse(SAMPLE_LF);
        let mut b = a.clone();
        b.section_mut("FRONTWING")
            .get_mut("FWSetting")
            .unwrap()
            .set_index(SetupNumber::Int(5));
        b.set(
            "CONTROLS",
            "RearBrakeSetting",
            SetupValue::int(47).with_display("53.0:47.0"),
        );
        b.set("ENGINE", "EngineBrakingMapSetting", SetupValue::int(2));
        b.section_mut("BASIC").values.shift_remove("Custom");

        let d = diff(&a, &b);
        let keys: Vec<(String, String)> = d
            .iter()
            .map(|x| (x.section.clone(), x.key.clone()))
            .collect();
        assert_eq!(
            keys,
            vec![
                ("FRONTWING".into(), "FWSetting".into()),
                ("CONTROLS".into(), "RearBrakeSetting".into()),
                ("ENGINE".into(), "EngineBrakingMapSetting".into()),
                ("BASIC".into(), "Custom".into()),
            ]
        );
        assert_eq!(d[0].a.as_ref().unwrap().as_i64(), Some(3));
        assert_eq!(d[0].b.as_ref().unwrap().as_i64(), Some(5));
        assert!(
            d[0].b.as_ref().unwrap().display.is_none(),
            "label dropped after set_index"
        );
        assert!(d[2].a.is_none());
        assert!(d[3].b.is_none());
        assert_eq!(
            d[1].to_string(),
            "[CONTROLS] RearBrakeSetting: 45 (55.0:45.0) → 47 (53.0:47.0)"
        );
        assert!(diff(&a, &a).is_empty());

        // the edited file is written in the same shape
        let text = b.to_string();
        assert!(text.contains("\nFWSetting=5\n"));
        assert!(text.contains("\nRearBrakeSetting=47//53.0:47.0\n"));
        assert!(text.contains("\nEngineBrakingMapSetting=2\n"));
        assert!(
            text.contains("Downforce=0.500000"),
            "untouched floats keep their spelling"
        );
        let reparsed = Setup::parse(&text);
        assert!(diff(&b, &reparsed).is_empty());
    }

    #[test]
    fn header_setters_and_file_io() {
        let mut s = Setup::parse(SAMPLE_LF);
        s.header
            .set_vehicle_class_setting("BMW_M4_LMGT3 GT3 WEC2025");
        s.header.set_upgrade_setting(&[7, 0, 0, 0]);
        assert_eq!(
            s.header.vehicle_class_setting().as_deref(),
            Some("BMW_M4_LMGT3 GT3 WEC2025")
        );
        assert_eq!(s.header.upgrade_setting(), Some(vec![7, 0, 0, 0]));
        assert!(s.to_string().starts_with(
            "VehicleClassSetting=\"BMW_M4_LMGT3 GT3 WEC2025\"\nUpgradeSetting=(7,0,0,0)\n"
        ));

        let dir = std::env::temp_dir().join(format!("corde-svm-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.svm");
        s.write(&path).unwrap();
        let back = Setup::read(&path).unwrap();
        assert_eq!(back, s);
        std::fs::write(&path, "just text\n").unwrap();
        assert!(matches!(
            Setup::read(&path),
            Err(SvmError::NoSection { .. })
        ));
        assert!(matches!(
            Setup::read(dir.join("missing.svm")),
            Err(SvmError::Read { .. })
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn number_parsing_edge_cases() {
        assert_eq!(SetupNumber::parse("12"), SetupNumber::Int(12));
        assert_eq!(SetupNumber::parse("-3"), SetupNumber::Int(-3));
        assert_eq!(SetupNumber::parse("0.250000"), SetupNumber::Float(0.25));
        assert_eq!(SetupNumber::parse("\"\""), SetupNumber::Text("\"\"".into()));
        assert_eq!(SetupNumber::parse("inf"), SetupNumber::Text("inf".into()));
        let v = SetupValue::parse("4//");
        assert_eq!(v.index, SetupNumber::Int(4));
        assert_eq!(v.display.as_deref(), Some(""));
        assert_eq!(v.to_string(), "4//");
        assert_eq!(SetupValue::float(0.5).to_string(), "0.500000");
    }
}
