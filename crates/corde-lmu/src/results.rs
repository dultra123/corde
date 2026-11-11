//! Parser for the results files the game writes after every session
//! (`UserData/Log/Results/*.xml`, the "rFactorXML" format inherited from
//! rFactor 2).
//!
//! What the real files look like (`reference/samples/results-*.xml`, LMU 1.42):
//!
//! ```text
//! <rFactorXML version="1.0">
//!  <RaceResults>
//!   <Setting>Multiplayer</Setting> <ServerName/> <PlayerFile>Settings</PlayerFile>
//!   <DateTime>1793562616</DateTime> <TimeString>2026/11/01 20:50:16</TimeString>
//!   <TrackVenue>Paul Ricard Circuit</TrackVenue> <TrackCourse>Paul Ricard - 1A</TrackCourse>
//!   <TrackLength>5698.6</TrackLength> <GameVersion>1.4200</GameVersion>
//!   <VehiclesAllowed>Mercedes_AMG_GT3,</VehiclesAllowed> …
//!   <Race>                                  ← or Practice1, Qualify1, Warmup…
//!    <DateTime>1793563502</DateTime> <Laps>2147483647</Laps> <Minutes>30</Minutes>
//!    <Stream> <Score…/> <Incident…/> <TrackLimits…/> <Penalty…/> <ChatMessage…/> </Stream>
//!    <Driver>
//!     <Name>…</Name> <VehFile>…</VehFile> <VehName>…</VehName> <CarType>…</CarType>
//!     <CarClass>GT3</CarClass> <CarNumber>37</CarNumber> <TeamName>…</TeamName>
//!     <isPlayer>1</isPlayer> <GridPos>35</GridPos> <Position>23</Position>
//!     <ClassGridPos>16</ClassGridPos> <ClassPosition>12</ClassPosition>
//!     <Lap num="3" p="28" et="405.1001" s1="32.6645" s2="31.7909" s3="56.6792"
//!          topspeed="269.81" fcompound="0,Medium" rcompound="0,Medium" pit="1">121.1345</Lap>
//!     <BestLapTime>118.3745</BestLapTime> <FinishTime>1851.4140</FinishTime>
//!     <Laps>15</Laps> <Pitstops>0</Pitstops> <FinishStatus>Finished Normally</FinishStatus>
//!     <DNFReason>Engine</DNFReason> <ControlAndAids startLap="1" endLap="15">PlayerControl</…>
//!    </Driver>
//!   </Race>
//!  </RaceResults>
//! </rFactorXML>
//! ```
//!
//! Things verified on those files that differ from what one might expect:
//!
//! * **`isPlayer` means "human"**: in a multiplayer file every connected human
//!   has `isPlayer=1`, so it does not single out the local driver. Only in an
//!   offline session (AI opponents have `isPlayer=0`) is it unambiguous.
//!   [`pick_player`] implements the resolution (hints, then uniqueness, then
//!   "the name present in the most files").
//! * Sector attributes `s1/s2/s3` are **durations**, not cumulative splits
//!   (`s1 + s2 + s3 ≈ lap time`).
//! * A lap without a valid time has the text `--.----`; the first lap's `et`
//!   can be `--.---`.
//! * `fuel=` and `twfl/twfr/twrl/twrr=` (fuel fraction, tyre wear) were absent
//!   from the sample files; they are parsed when present.
//! * `Laps` of a timed session is `2147483647` (treated as "unlimited").
//!
//! The parser is tolerant: every field is optional in the XML; missing ones
//! become `None`, empty strings or zero.

use corde_protocol::{FinishStatus, SessionType};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Value of `<Laps>` meaning "no lap limit".
const UNLIMITED_LAPS: i64 = i32::MAX as i64;

#[derive(Debug, thiserror::Error)]
pub enum ResultsError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("xml: {0}")]
    Xml(#[from] roxmltree::Error),
    #[error("not a results file: {0}")]
    Format(String),
}

pub type Result<T> = std::result::Result<T, ResultsError>;

// ---------------------------------------------------------------------------
// Data model
// ---------------------------------------------------------------------------

/// One results file = one session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct RaceResultsFile {
    /// `GameVersion` (`"1.4200"`).
    pub game_version: Option<String>,
    /// `Setting` (`"Multiplayer"`, `"Single Player"`…).
    pub setting: Option<String>,
    pub server_name: Option<String>,
    /// `PlayerFile` (profile name; `"Settings"` on LMU).
    pub player_file: Option<String>,
    /// Top-level `DateTime` (when the file was written), Unix seconds.
    pub file_date_time: Option<i64>,
    /// `TrackVenue` (`"Paul Ricard Circuit"`).
    pub track_venue: String,
    /// `TrackCourse` (`"Paul Ricard - 1A"`) — the layout, what the live
    /// telemetry calls the track name.
    pub track_course: String,
    pub track_event: Option<String>,
    pub track_length_m: f32,
    /// `VehiclesAllowed` (`"Mercedes_AMG_GT3,"`) — the closest thing to a mod name.
    pub mod_name: Option<String>,
    /// Element name of the session section (`"Practice1"`, `"Race"`).
    pub session_name: String,
    pub session_kind: SessionType,
    /// Session start, Unix seconds (`DateTime` of the section, else of the file).
    pub date_time: i64,
    pub time_string: Option<String>,
    /// Session length, minutes (`None` when lap-limited only).
    pub minutes: Option<f32>,
    /// Planned laps (`None` when timed / unlimited).
    pub laps_planned: Option<i32>,
    pub most_laps_completed: Option<i32>,
    pub drivers: Vec<ResultDriver>,
    /// Contacts, track-limit decisions and penalties from `<Stream>`, in file
    /// order (chat and timing lines are dropped).
    pub events: Vec<StreamEvent>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ResultDriver {
    pub name: String,
    /// `VehName` (`"Mercedes AMG Custom Team 2025 #397"`).
    pub vehicle_name: String,
    /// `VehFile` (`"397_25_AMG.VEH"`).
    pub vehicle_file: String,
    /// `CarType` (`"Mercedes-AMG LMGT3"`) — the model, what the UI shows.
    pub car_type: String,
    /// `CarClass` (`"GT3"`, `"LMP2_ELMS"`, `"Hypercar"`).
    pub car_class: String,
    /// `Category` (`"WEC 2025, GT3, Mercedes-AMG LMGT3"`).
    pub category: Option<String>,
    pub car_number: Option<String>,
    pub team: String,
    /// `isPlayer` — a human (see the module docs).
    pub is_player: bool,
    pub connected: Option<bool>,
    pub server_scored: Option<bool>,
    pub grid_pos: Option<i32>,
    pub position: Option<i32>,
    pub class_grid_pos: Option<i32>,
    pub class_position: Option<i32>,
    pub points: Option<f32>,
    pub class_points: Option<f32>,
    /// Raw `FinishStatus` (`"Finished Normally"`, `"DNF"`, `"None"`, `"DQ"`).
    pub finish_status: String,
    pub finish: FinishStatus,
    pub dnf_reason: Option<String>,
    pub best_lap_s: Option<f32>,
    /// Race time at the flag, seconds (finishers only).
    pub finish_time_s: Option<f32>,
    /// `<Laps>` of the driver (laps completed).
    pub laps_completed: i32,
    pub pitstops: i32,
    /// `ControlAndAids` ranges.
    pub aids: Vec<AidsRange>,
    pub laps: Vec<ResultLap>,
    /// Derived from `<Stream>`.
    pub incidents: IncidentSummary,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ResultLap {
    pub num: i32,
    /// Position at the end of the lap (`p`); the game writes 105 for "not
    /// classified yet" in practice.
    pub position: Option<i32>,
    /// Session time at the end of the lap (`et`), seconds.
    pub elapsed_s: Option<f64>,
    /// `None` when the text is `--.----` (no time / invalid).
    pub lap_time_s: Option<f32>,
    pub s1: Option<f32>,
    pub s2: Option<f32>,
    pub s3: Option<f32>,
    /// `topspeed`, km/h (`None` when 0 = not observed).
    pub top_speed_kmh: Option<f32>,
    /// `fuel` attribute (0..1) when the game writes it.
    pub fuel_fraction: Option<f32>,
    /// Compound name without the index (`"0,Medium"` → `"Medium"`).
    pub front_compound: Option<String>,
    pub rear_compound: Option<String>,
    /// `twfl/twfr/twrl/twrr` (remaining fraction, FL FR RL RR) when written.
    pub tyre_wear: Option<[f32; 4]>,
    /// `pit="1"`: the lap ended in (or went through) the pits.
    pub pit: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct AidsRange {
    pub start_lap: i32,
    pub end_lap: i32,
    /// `"PlayerControl,ABS=1,Clutch,AutoBlip"`.
    pub text: String,
}

/// Counts derived from `<Stream>` for one driver.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct IncidentSummary {
    /// `Incident … reported contact (…) with …` lines.
    pub contacts: u32,
    /// Of which with another vehicle.
    pub contacts_with_vehicles: u32,
    /// `TrackLimits` lines resolved as `Warning`.
    pub track_limit_warnings: u32,
    /// `TrackLimits` lines resolved as `Invalid Lap…`.
    pub invalid_laps: u32,
    /// `Penalty` lines received (not the "served" ones).
    pub penalties: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StreamKind {
    Contact,
    TrackLimits,
    Penalty,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StreamEvent {
    pub kind: StreamKind,
    /// Session time, seconds.
    pub et_s: f32,
    /// Driver concerned (the reporting one for a contact).
    pub driver: String,
    pub lap: Option<i32>,
    /// Other driver for a contact with a vehicle.
    pub other: Option<String>,
    /// Human text of the line (`"Warning"`, `"Invalid Lap Cut Track"`,
    /// `"Drive Thru — Speeding"`, `"contact with Immovable (236.15)"`).
    pub text: String,
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// Parse a results file from disk. Invalid UTF-8 is replaced, a BOM skipped.
pub fn parse_file(path: &Path) -> Result<RaceResultsFile> {
    let bytes = std::fs::read(path)?;
    let text = String::from_utf8_lossy(&bytes);
    parse_str(text.trim_start_matches('\u{feff}'))
}

/// Cut whatever follows the closing tag of the document.
///
/// The game sometimes leaves a stray byte after `</rFactorXML>` (seen on a
/// pilot's PC: a single 0xF0 at the very end of a qualifying file); a strict
/// XML parser then refuses the whole file, and the session it describes
/// could not be matched with its official lap times.
fn trim_after_root(text: &str) -> &str {
    const CLOSE: &str = "</rFactorXML>";
    match text.rfind(CLOSE) {
        Some(i) => &text[..i + CLOSE.len()],
        None => text,
    }
}

/// Parse the content of a results file.
pub fn parse_str(text: &str) -> Result<RaceResultsFile> {
    let text = trim_after_root(text);
    let opts = roxmltree::ParsingOptions {
        allow_dtd: true,
        ..Default::default()
    };
    let doc = roxmltree::Document::parse_with_options(text, opts)?;
    let root = doc.root_element();
    let results = if root.has_tag_name("RaceResults") {
        root
    } else {
        child(root, "RaceResults").ok_or_else(|| {
            ResultsError::Format(format!(
                "root element <{}> without <RaceResults>",
                root.tag_name().name()
            ))
        })?
    };

    let mut file = RaceResultsFile {
        game_version: child_text(results, "GameVersion"),
        setting: child_text(results, "Setting"),
        server_name: child_text(results, "ServerName"),
        player_file: child_text(results, "PlayerFile"),
        file_date_time: child_i64(results, "DateTime"),
        track_venue: child_text(results, "TrackVenue").unwrap_or_default(),
        track_course: child_text(results, "TrackCourse").unwrap_or_default(),
        track_event: child_text(results, "TrackEvent"),
        track_length_m: child_f32(results, "TrackLength").unwrap_or(0.0),
        mod_name: child_text(results, "VehiclesAllowed")
            .map(|s| s.trim_end_matches(',').to_string()),
        ..Default::default()
    };
    if file.track_course.is_empty() {
        file.track_course = file.track_venue.clone();
    }

    // The session section: prefer one with drivers, else the first known one.
    let sections: Vec<roxmltree::Node> = results
        .children()
        .filter(|n| n.is_element() && session_kind_of(n.tag_name().name()).is_some())
        .collect();
    let section = sections
        .iter()
        .find(|n| child(**n, "Driver").is_some())
        .or(sections.first())
        .copied()
        .ok_or_else(|| {
            ResultsError::Format("no session section (Practice/Qualify/Race…)".into())
        })?;

    file.session_name = section.tag_name().name().to_string();
    file.session_kind = session_kind_of(&file.session_name).unwrap_or_default();
    file.date_time = child_i64(section, "DateTime")
        .or(file.file_date_time)
        .unwrap_or(0);
    file.time_string = child_text(section, "TimeString");
    file.minutes = child_f32(section, "Minutes").filter(|m| *m > 0.0);
    file.laps_planned = child_i64(section, "Laps")
        .filter(|l| *l > 0 && *l < UNLIMITED_LAPS)
        .map(|l| l as i32);
    file.most_laps_completed = child_i64(section, "MostLapsCompleted").map(|v| v as i32);

    file.drivers = section
        .children()
        .filter(|n| n.has_tag_name("Driver"))
        .map(parse_driver)
        .collect();

    if let Some(stream) = child(section, "Stream") {
        file.events = parse_stream(stream);
        let mut by_name: HashMap<String, usize> = HashMap::new();
        for (i, d) in file.drivers.iter().enumerate() {
            by_name.insert(name_key(&d.name), i);
        }
        for ev in &file.events {
            let Some(&i) = by_name.get(&name_key(&ev.driver)) else {
                continue;
            };
            let inc = &mut file.drivers[i].incidents;
            match ev.kind {
                StreamKind::Contact => {
                    inc.contacts += 1;
                    if ev.other.is_some() {
                        inc.contacts_with_vehicles += 1;
                    }
                }
                StreamKind::TrackLimits => {
                    if ev.text.starts_with("Invalid Lap") {
                        inc.invalid_laps += 1;
                    } else if ev.text == "Warning" {
                        inc.track_limit_warnings += 1;
                    }
                }
                StreamKind::Penalty => inc.penalties += 1,
            }
        }
    }
    Ok(file)
}

fn parse_driver(n: roxmltree::Node) -> ResultDriver {
    let finish_status = child_text(n, "FinishStatus").unwrap_or_default();
    ResultDriver {
        name: child_text(n, "Name").unwrap_or_default(),
        vehicle_name: child_text(n, "VehName").unwrap_or_default(),
        vehicle_file: child_text(n, "VehFile").unwrap_or_default(),
        car_type: child_text(n, "CarType").unwrap_or_default(),
        car_class: child_text(n, "CarClass").unwrap_or_default(),
        category: child_text(n, "Category"),
        car_number: child_text(n, "CarNumber"),
        team: child_text(n, "TeamName").unwrap_or_default(),
        is_player: child_i64(n, "isPlayer").unwrap_or(0) != 0,
        connected: child_i64(n, "Connected").map(|v| v != 0),
        server_scored: child_i64(n, "ServerScored").map(|v| v != 0),
        grid_pos: child_i32(n, "GridPos"),
        position: child_i32(n, "Position"),
        class_grid_pos: child_i32(n, "ClassGridPos"),
        class_position: child_i32(n, "ClassPosition"),
        points: child_f32(n, "Points"),
        class_points: child_f32(n, "ClassPoints"),
        finish: finish_status_of(&finish_status),
        finish_status,
        dnf_reason: child_text(n, "DNFReason"),
        best_lap_s: child_f32(n, "BestLapTime").filter(|t| *t > 0.0),
        finish_time_s: child_f32(n, "FinishTime").filter(|t| *t > 0.0),
        laps_completed: child_i32(n, "Laps").unwrap_or(0),
        pitstops: child_i32(n, "Pitstops").unwrap_or(0),
        aids: n
            .children()
            .filter(|c| c.has_tag_name("ControlAndAids"))
            .map(|c| AidsRange {
                start_lap: attr_i32(c, "startLap").unwrap_or(0),
                end_lap: attr_i32(c, "endLap").unwrap_or(0),
                text: c.text().unwrap_or_default().trim().to_string(),
            })
            .collect(),
        laps: n
            .children()
            .filter(|c| c.has_tag_name("Lap"))
            .map(parse_lap)
            .collect(),
        incidents: IncidentSummary::default(),
    }
}

fn parse_lap(n: roxmltree::Node) -> ResultLap {
    let wear = match (
        attr_f32(n, "twfl"),
        attr_f32(n, "twfr"),
        attr_f32(n, "twrl"),
        attr_f32(n, "twrr"),
    ) {
        (Some(a), Some(b), Some(c), Some(d)) => Some([a, b, c, d]),
        _ => None,
    };
    ResultLap {
        num: attr_i32(n, "num").unwrap_or(0),
        position: attr_i32(n, "p"),
        elapsed_s: n
            .attribute("et")
            .and_then(parse_num::<f64>)
            .filter(|v| *v >= 0.0),
        lap_time_s: n.text().and_then(parse_num::<f32>).filter(|v| *v > 0.0),
        s1: attr_time(n, "s1"),
        s2: attr_time(n, "s2"),
        s3: attr_time(n, "s3"),
        top_speed_kmh: attr_f32(n, "topspeed").filter(|v| *v > 0.0),
        fuel_fraction: attr_f32(n, "fuel").filter(|v| *v >= 0.0),
        front_compound: n.attribute("fcompound").map(compound_name),
        rear_compound: n.attribute("rcompound").map(compound_name),
        tyre_wear: wear,
        pit: attr_i32(n, "pit").unwrap_or(0) != 0,
    }
}

/// `<Stream>` lines that matter for a driver's record. Lines repeated verbatim
/// by the game (every `TrackLimits` decision appears twice) are kept once.
fn parse_stream(stream: roxmltree::Node) -> Vec<StreamEvent> {
    let mut out: Vec<StreamEvent> = Vec::new();
    let mut last_raw: Option<(String, String)> = None;
    for n in stream.children().filter(|n| n.is_element()) {
        let tag = n.tag_name().name().to_string();
        let text = n.text().unwrap_or_default().trim().to_string();
        if last_raw.as_ref() == Some(&(tag.clone(), text.clone())) {
            continue;
        }
        let et_s = attr_f32(n, "et").unwrap_or(0.0);
        let ev = match tag.as_str() {
            "Incident" => parse_incident(et_s, &text),
            "TrackLimits" => n.attribute("Driver").map(|d| StreamEvent {
                kind: StreamKind::TrackLimits,
                et_s,
                driver: d.trim().to_string(),
                lap: attr_i32(n, "Lap"),
                other: None,
                text: text.clone(),
            }),
            "Penalty" => n.attribute("Driver").map(|d| {
                let penalty = n.attribute("Penalty").unwrap_or("").trim();
                let reason = n.attribute("Reason").unwrap_or("").trim();
                StreamEvent {
                    kind: StreamKind::Penalty,
                    et_s,
                    driver: d.trim().to_string(),
                    lap: attr_i32(n, "Lap").filter(|l| *l > 0),
                    other: None,
                    text: match (penalty.is_empty(), reason.is_empty()) {
                        (false, false) => format!("{penalty} — {reason}"),
                        (false, true) => penalty.to_string(),
                        _ => text.clone(),
                    },
                }
            }),
            _ => None,
        };
        last_raw = Some((tag, text));
        if let Some(ev) = ev {
            out.push(ev);
        }
    }
    out
}

/// `Andrei Vykh(46) reported contact (236.15) with Immovable`
/// `alex martin(36) reported contact (1844.92) with another vehicle Tomas Wieczek(8)`
fn parse_incident(et_s: f32, text: &str) -> Option<StreamEvent> {
    let (who, rest) = text.split_once(" reported contact ")?;
    let driver = strip_id(who);
    let (magnitude, with) = rest.split_once(" with ")?;
    let magnitude = magnitude.trim_matches(|c| c == '(' || c == ')');
    let with = with.trim();
    let other = with
        .strip_prefix("another vehicle ")
        .map(|o| strip_id(o).to_string());
    let what = match &other {
        Some(o) => o.clone(),
        None => with.to_string(),
    };
    Some(StreamEvent {
        kind: StreamKind::Contact,
        et_s,
        driver: driver.to_string(),
        lap: None,
        other,
        text: format!("contact with {what} ({magnitude})"),
    })
}

/// `"Tomas Wieczek(8)"` → `"Tomas Wieczek"`.
fn strip_id(s: &str) -> &str {
    let s = s.trim();
    match s.rfind('(') {
        Some(i)
            if s.ends_with(')') && s[i + 1..s.len() - 1].bytes().all(|b| b.is_ascii_digit()) =>
        {
            s[..i].trim()
        }
        _ => s,
    }
}

// ---------------------------------------------------------------------------
// Player resolution
// ---------------------------------------------------------------------------

/// Normalised driver name for comparisons (trimmed, case-folded).
pub fn name_key(name: &str) -> String {
    name.trim().to_lowercase()
}

/// Which driver of `file` is the local player.
///
/// * `hints` — names known to be the player (CLI option, live telemetry,
///   previous imports); the first one found among the humans wins.
/// * `seen` — for every driver name (as [`name_key`]), the number of results
///   files it appears in (this one included). The local player is in every
///   file, so the human with the strictly highest count is picked when that
///   count is at least 2.
///
/// Returns `None` when the file has no human, or when the choice stays
/// ambiguous (a lone multiplayer file without a hint).
pub fn pick_player(
    file: &RaceResultsFile,
    hints: &[String],
    seen: &HashMap<String, u32>,
) -> Option<usize> {
    let humans: Vec<usize> = file
        .drivers
        .iter()
        .enumerate()
        .filter(|(_, d)| d.is_player)
        .map(|(i, _)| i)
        .collect();
    if humans.is_empty() {
        return None;
    }
    for hint in hints {
        let key = name_key(hint);
        if key.is_empty() {
            continue;
        }
        if let Some(&i) = humans
            .iter()
            .find(|&&i| name_key(&file.drivers[i].name) == key)
        {
            return Some(i);
        }
    }
    if humans.len() == 1 {
        return Some(humans[0]);
    }
    let mut best: Option<(usize, u32)> = None;
    let mut tie = false;
    for &i in &humans {
        let count = seen
            .get(&name_key(&file.drivers[i].name))
            .copied()
            .unwrap_or(0);
        match best {
            Some((_, c)) if count == c => tie = true,
            Some((_, c)) if count < c => {}
            _ => {
                best = Some((i, count));
                tie = false;
            }
        }
    }
    match best {
        Some((i, count)) if !tie && count >= 2 => Some(i),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// `Practice1` → practice, `Qualify2` → qualify, `Race` / `Race1` → race,
/// `Warmup` → warmup, `TestDay` → test day. `None` for anything else (so the
/// header elements are never mistaken for a session).
pub fn session_kind_of(tag: &str) -> Option<SessionType> {
    let t = tag.to_ascii_lowercase();
    if t.starts_with("practice") {
        Some(SessionType::Practice)
    } else if t.starts_with("qualify") {
        Some(SessionType::Qualify)
    } else if t.starts_with("warmup") {
        Some(SessionType::Warmup)
    } else if t.starts_with("race") && !t.starts_with("raceresults") {
        Some(SessionType::Race)
    } else if t.starts_with("testday") {
        Some(SessionType::TestDay)
    } else {
        None
    }
}

pub fn finish_status_of(text: &str) -> FinishStatus {
    let t = text.trim().to_ascii_lowercase();
    if t.starts_with("finished") {
        FinishStatus::Finished
    } else if t.starts_with("dnf") {
        FinishStatus::Dnf
    } else if t.starts_with("dq") || t.starts_with("disq") {
        FinishStatus::Dq
    } else {
        FinishStatus::None
    }
}

/// `"0,Medium"` → `"Medium"`; `"Soft"` → `"Soft"`.
fn compound_name(raw: &str) -> String {
    let raw = raw.trim();
    match raw.split_once(',') {
        Some((idx, name)) if idx.trim().parse::<i32>().is_ok() => name.trim().to_string(),
        _ => raw.to_string(),
    }
}

fn child<'a, 'i>(n: roxmltree::Node<'a, 'i>, name: &str) -> Option<roxmltree::Node<'a, 'i>> {
    n.children().find(|c| c.has_tag_name(name))
}

fn child_text(n: roxmltree::Node, name: &str) -> Option<String> {
    child(n, name)
        .and_then(|c| c.text())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn child_i64(n: roxmltree::Node, name: &str) -> Option<i64> {
    child_text(n, name).and_then(|s| parse_num::<i64>(&s))
}

fn child_i32(n: roxmltree::Node, name: &str) -> Option<i32> {
    child_i64(n, name).map(|v| v.clamp(i32::MIN as i64, i32::MAX as i64) as i32)
}

fn child_f32(n: roxmltree::Node, name: &str) -> Option<f32> {
    child_text(n, name).and_then(|s| parse_num::<f32>(&s))
}

fn attr_f32(n: roxmltree::Node, name: &str) -> Option<f32> {
    n.attribute(name).and_then(parse_num::<f32>)
}

fn attr_i32(n: roxmltree::Node, name: &str) -> Option<i32> {
    n.attribute(name)
        .and_then(parse_num::<f64>)
        .map(|v| v.clamp(i32::MIN as f64, i32::MAX as f64) as i32)
}

/// A sector / lap time attribute: absent, `--.----` or ≤ 0 → `None`.
fn attr_time(n: roxmltree::Node, name: &str) -> Option<f32> {
    attr_f32(n, name).filter(|v| *v > 0.0)
}

/// Lenient number parsing: trims, accepts `--.----` as absent, falls back to
/// the float form for integers written as `12.0`.
fn parse_num<T: std::str::FromStr + LenientFromF64>(s: &str) -> Option<T> {
    let s = s.trim();
    if s.is_empty() || s.starts_with("--") {
        return None;
    }
    if let Ok(v) = s.parse::<T>() {
        return Some(v);
    }
    s.parse::<f64>().ok().and_then(T::from_f64)
}

/// Fallback conversion from a float when the direct parse fails.
pub trait LenientFromF64: Sized {
    fn from_f64(v: f64) -> Option<Self>;
}

impl LenientFromF64 for f32 {
    fn from_f64(v: f64) -> Option<Self> {
        Some(v as f32)
    }
}
impl LenientFromF64 for f64 {
    fn from_f64(v: f64) -> Option<Self> {
        Some(v)
    }
}
impl LenientFromF64 for i64 {
    fn from_f64(v: f64) -> Option<Self> {
        v.is_finite().then_some(v as i64)
    }
}

impl RaceResultsFile {
    /// Index of the driver named `name` (case-insensitive).
    pub fn driver_index(&self, name: &str) -> Option<usize> {
        let key = name_key(name);
        self.drivers.iter().position(|d| name_key(&d.name) == key)
    }

    /// Drivers of one class, best (lowest `class_position`, else `position`) first.
    pub fn class_of<'a>(&'a self, car_class: &str) -> Vec<&'a ResultDriver> {
        let mut v: Vec<&ResultDriver> = self
            .drivers
            .iter()
            .filter(|d| d.car_class == car_class)
            .collect();
        v.sort_by_key(|d| {
            (
                d.class_position.unwrap_or(i32::MAX),
                d.position.unwrap_or(i32::MAX),
            )
        });
        v
    }

    /// Keep only the stream events of `driver` (the player) — the rest is
    /// noise once the per-driver counters are computed.
    pub fn retain_events_of(&mut self, driver: &str) {
        let key = name_key(driver);
        self.events.retain(|e| {
            name_key(&e.driver) == key || e.other.as_deref().map(name_key) == Some(key.clone())
        });
    }
}

impl ResultDriver {
    /// Fastest timed lap found in `laps` (should equal `best_lap_s`).
    pub fn best_from_laps(&self) -> Option<&ResultLap> {
        self.laps
            .iter()
            .filter(|l| l.lap_time_s.is_some())
            .min_by(|a, b| {
                a.lap_time_s
                    .partial_cmp(&b.lap_time_s)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    }

    /// Sum of the lap times that exist (for distance-independent statistics).
    pub fn timed_laps(&self) -> usize {
        self.laps.iter().filter(|l| l.lap_time_s.is_some()).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn sample(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../reference/samples")
            .join(name)
    }

    fn practice() -> RaceResultsFile {
        parse_file(&sample("results-P1-paulricard.xml")).expect("parse practice file")
    }

    fn race() -> RaceResultsFile {
        parse_file(&sample("results-R1-paulricard.xml")).expect("parse race file")
    }

    #[test]
    fn practice_header_and_drivers() {
        let f = practice();
        assert_eq!(f.game_version.as_deref(), Some("1.4200"));
        assert_eq!(f.setting.as_deref(), Some("Multiplayer"));
        assert_eq!(f.server_name, None);
        assert_eq!(f.track_venue, "Silverstone Circuit");
        assert_eq!(f.track_course, "Silverstone International Circuit");
        assert_eq!(f.track_length_m, 2962.1);
        assert_eq!(f.mod_name.as_deref(), Some("Mercedes_AMG_GT3"));
        assert_eq!(f.session_name, "Practice1");
        assert_eq!(f.session_kind, SessionType::Practice);
        assert_eq!(f.file_date_time, Some(1_793_560_250));
        assert_eq!(f.date_time, 1_793_560_254);
        assert_eq!(f.minutes, Some(120.0));
        assert_eq!(f.laps_planned, None, "2147483647 = unlimited");
        assert_eq!(f.most_laps_completed, Some(56));
        assert_eq!(f.drivers.len(), 4);
        // Every connected human is `isPlayer` in a multiplayer file.
        assert!(f.drivers.iter().all(|d| d.is_player));

        let robin = &f.drivers[0];
        assert_eq!(robin.name, "Robin Vale#4821");
        assert_eq!(robin.car_type, "Porsche 911 GT3 R LMGT3");
        assert_eq!(robin.car_class, "GT3");
        assert_eq!(robin.car_number.as_deref(), Some("21"));
        assert_eq!(robin.team, "Vales Motorspot");
        assert_eq!(robin.position, Some(1));
        assert_eq!(robin.class_position, Some(1));
        assert_eq!(robin.grid_pos, None);
        assert_eq!(robin.best_lap_s, Some(62.4312));
        assert_eq!(robin.laps_completed, 56);
        assert_eq!(robin.laps.len(), 56);
        assert_eq!(robin.finish, FinishStatus::Finished);
        assert_eq!(robin.aids.len(), 3);
        assert_eq!(robin.aids[1].text, "PlayerControl,Clutch,AutoBlip");
        // Lap 1: no elapsed time, no lap time; lap 45 is the best one.
        assert_eq!(robin.laps[0].elapsed_s, None);
        assert_eq!(robin.laps[0].lap_time_s, None);
        assert_eq!(robin.laps[0].front_compound.as_deref(), Some("Medium"));
        let best = robin.best_from_laps().unwrap();
        assert_eq!(best.num, 45);
        assert_eq!(best.lap_time_s, robin.best_lap_s);
        assert_eq!(robin.laps[53].top_speed_kmh, Some(246.39));
        assert_eq!(robin.laps[52].top_speed_kmh, None);

        let me = f
            .driver_index("Alex Martin")
            .map(|i| &f.drivers[i])
            .unwrap();
        assert_eq!(me.team, "Martin Racing");
        assert_eq!(me.laps_completed, 0);
        assert!(me.laps.is_empty());
        assert_eq!(me.best_lap_s, None);
        assert_eq!(me.position, Some(4));
        assert_eq!(me.finish, FinishStatus::None);
    }

    #[test]
    fn race_player_block() {
        let f = race();
        assert_eq!(f.track_venue, "Paul Ricard Circuit");
        assert_eq!(f.track_course, "Paul Ricard - 1A");
        assert_eq!(f.track_length_m, 5698.6);
        assert_eq!(f.session_name, "Race");
        assert_eq!(f.session_kind, SessionType::Race);
        assert_eq!(f.date_time, 1_793_563_502);
        assert_eq!(f.minutes, Some(30.0));
        assert_eq!(f.laps_planned, None);
        assert_eq!(f.drivers.len(), 38);
        assert_eq!(f.drivers.iter().filter(|d| d.is_player).count(), 38);

        let i = f.driver_index("alex martin").unwrap();
        let me = &f.drivers[i];
        assert_eq!(me.vehicle_name, "Mercedes AMG Custom Team 2025 #397");
        assert_eq!(me.vehicle_file, "397_25_AMG.VEH");
        assert_eq!(me.car_type, "Mercedes-AMG LMGT3");
        assert_eq!(me.car_class, "GT3");
        assert_eq!(me.car_number.as_deref(), Some("37"));
        assert_eq!(me.grid_pos, Some(35));
        assert_eq!(me.position, Some(23));
        assert_eq!(me.class_grid_pos, Some(16));
        assert_eq!(me.class_position, Some(12));
        assert_eq!(me.best_lap_s, Some(118.3745));
        assert_eq!(me.finish_time_s, Some(1851.414));
        assert_eq!(me.laps_completed, 15);
        assert_eq!(me.laps.len(), 15);
        assert_eq!(me.pitstops, 0);
        assert_eq!(me.finish_status, "Finished Normally");
        assert_eq!(me.finish, FinishStatus::Finished);
        assert_eq!(me.dnf_reason, None);
        assert_eq!(
            me.aids,
            vec![AidsRange {
                start_lap: 1,
                end_lap: 15,
                text: "PlayerControl".into()
            }]
        );

        // Laps 1–2 have no time (formation + first flying lap behind traffic).
        assert_eq!(me.laps[0].lap_time_s, None);
        assert_eq!(me.laps[0].elapsed_s, Some(144.2025));
        assert_eq!(me.laps[0].position, Some(27));
        assert_eq!(me.laps[1].lap_time_s, None);
        assert_eq!(me.timed_laps(), 13);
        // Sectors are durations: s1 + s2 + s3 ≈ lap time, on every timed lap.
        for l in me.laps.iter().filter(|l| l.lap_time_s.is_some()) {
            let sum = l.s1.unwrap() + l.s2.unwrap() + l.s3.unwrap();
            assert!(
                (sum - l.lap_time_s.unwrap()).abs() < 0.002,
                "lap {}: {sum} vs {:?}",
                l.num,
                l.lap_time_s
            );
        }
        let best = me.best_from_laps().unwrap();
        assert_eq!(best.num, 9);
        assert_eq!(best.lap_time_s, Some(118.3745));
        assert_eq!(best.position, Some(28));
        assert_eq!(best.top_speed_kmh, Some(270.86));
        assert_eq!(best.fuel_fraction, None);
        assert_eq!(best.tyre_wear, None);
        assert!(!best.pit);

        // Class standings: 19 GT3 cars (19 LMP2), the player 12th, one lap
        // down on the class winner (15 laps vs 16).
        let gt3 = f.class_of("GT3");
        assert_eq!(gt3.len(), 19);
        assert_eq!(gt3[0].class_position, Some(1));
        assert_eq!(gt3[0].name, "Hugo Lambrin");
        assert_eq!(gt3[0].car_type, "BMW M4 LMGT3");
        assert_eq!(gt3[0].laps_completed, 16);
        assert_eq!(gt3[0].finish_time_s, Some(1841.8375));
        assert_eq!(gt3[11].name, "alex martin");
        // XML entities are decoded.
        assert!(f.drivers.iter().any(|d| d.name == "Liam O'Dellan"));

        // A DNF with a reason, a pit lap flag, and a driver without laps.
        let gilles = &f.drivers[0];
        assert_eq!(gilles.finish, FinishStatus::Dnf);
        assert_eq!(gilles.dnf_reason.as_deref(), Some("Engine"));
        assert!(f.drivers.iter().any(|d| d.laps.iter().any(|l| l.pit)));
        assert!(f.drivers.iter().any(|d| d.laps.is_empty()));
    }

    #[test]
    fn stream_incidents_are_counted_per_driver() {
        let f = race();
        let me = &f.drivers[f.driver_index("alex martin").unwrap()];
        // Two contacts with other cars (Wieczek, Vilaren), two "Warning"
        // track-limit decisions, no penalty.
        assert_eq!(me.incidents.contacts, 2);
        assert_eq!(me.incidents.contacts_with_vehicles, 2);
        assert_eq!(me.incidents.track_limit_warnings, 2);
        assert_eq!(me.incidents.invalid_laps, 0);
        assert_eq!(me.incidents.penalties, 0);
        let milo = &f.drivers[f.driver_index("Milo Trzyn").unwrap()];
        assert!(milo.incidents.penalties >= 1);
        let contact = f
            .events
            .iter()
            .find(|e| e.kind == StreamKind::Contact && e.driver == "alex martin")
            .unwrap();
        assert_eq!(contact.other.as_deref(), Some("Tomas Wieczek"));
        assert_eq!(contact.text, "contact with Tomas Wieczek (1844.92)");
        assert!((contact.et_s - 736.1).abs() < 0.01);

        let mut only_me = f.clone();
        only_me.retain_events_of("Alex Martin");
        assert!(only_me.events.len() < f.events.len());
        assert!(only_me
            .events
            .iter()
            .all(|e| e.driver == "alex martin" || e.other.as_deref() == Some("alex martin")));

        // Practice: the duplicated TrackLimits lines are deduplicated.
        let p = practice();
        let robin = &p.drivers[0];
        assert_eq!(robin.incidents.invalid_laps, 1);
    }

    #[test]
    fn player_resolution() {
        let p = practice();
        let r = race();
        // Alone, a multiplayer file is ambiguous…
        assert_eq!(pick_player(&p, &[], &HashMap::new()), None);
        // …unless a hint names one of the humans (case-insensitive).
        assert_eq!(
            pick_player(&p, &["  ALEX martin ".into()], &HashMap::new()),
            p.driver_index("alex martin")
        );
        assert_eq!(pick_player(&p, &["Nobody".into()], &HashMap::new()), None);
        // Across the two files only the local player is present in both.
        let mut seen: HashMap<String, u32> = HashMap::new();
        for f in [&p, &r] {
            for d in &f.drivers {
                *seen.entry(name_key(&d.name)).or_default() += 1;
            }
        }
        assert_eq!(seen["alex martin"], 2);
        assert_eq!(pick_player(&p, &[], &seen), p.driver_index("alex martin"));
        assert_eq!(pick_player(&r, &[], &seen), r.driver_index("alex martin"));

        // Offline session: the only `isPlayer` is the player.
        let mut solo = p.clone();
        for (i, d) in solo.drivers.iter_mut().enumerate() {
            d.is_player = i == 2;
        }
        assert_eq!(pick_player(&solo, &[], &HashMap::new()), Some(2));
        for d in solo.drivers.iter_mut() {
            d.is_player = false;
        }
        assert_eq!(pick_player(&solo, &["Robin Vale#4821".into()], &seen), None);
    }

    #[test]
    fn tolerant_on_minimal_and_odd_input() {
        let xml = r#"<?xml version="1.0"?><rFactorXML version="1.0"><RaceResults>
            <TrackVenue>Somewhere</TrackVenue><DateTime>100</DateTime>
            <Qualify2><Laps>12</Laps>
              <Driver><Name>A</Name><isPlayer>1</isPlayer>
                <Lap num="1" p="3" et="--.---" fuel="0.52" twfl="0.9" twfr="0.91" twrl="0.8" twrr="0.85" fcompound="Soft">--.----</Lap>
                <Lap num="2" et="95.5" s1="30" s2="30.5" s3="35">95.5</Lap>
              </Driver>
              <Driver><Name>B</Name><isPlayer>0</isPlayer><FinishStatus>DQ</FinishStatus></Driver>
            </Qualify2></RaceResults></rFactorXML>"#;
        let f = parse_str(xml).unwrap();
        assert_eq!(f.track_venue, "Somewhere");
        assert_eq!(
            f.track_course, "Somewhere",
            "course falls back to the venue"
        );
        assert_eq!(f.session_kind, SessionType::Qualify);
        assert_eq!(f.date_time, 100, "section without DateTime → file DateTime");
        assert_eq!(f.laps_planned, Some(12));
        assert_eq!(f.minutes, None);
        let a = &f.drivers[0];
        assert_eq!(a.laps[0].fuel_fraction, Some(0.52));
        assert_eq!(a.laps[0].tyre_wear, Some([0.9, 0.91, 0.8, 0.85]));
        assert_eq!(a.laps[0].front_compound.as_deref(), Some("Soft"));
        assert_eq!(a.laps[0].rear_compound, None);
        assert_eq!(a.laps[1].lap_time_s, Some(95.5));
        assert_eq!(a.laps[1].position, None);
        assert_eq!(f.drivers[1].finish, FinishStatus::Dq);
        assert_eq!(pick_player(&f, &[], &HashMap::new()), Some(0));

        // A stray byte after the closing tag (0xF0 read as U+FFFD), as the game
        // sometimes writes, does not cost the whole file.
        let dirty = format!("{xml}\r\n{}", '\u{fffd}');
        assert_eq!(parse_str(&dirty).unwrap(), f);
        assert_eq!(
            trim_after_root("<a/>junk"),
            "<a/>junk",
            "only after the root"
        );

        assert!(parse_str("<nope/>").is_err());
        assert!(parse_str("<RaceResults><TrackVenue>x</TrackVenue></RaceResults>").is_err());
        assert!(parse_str("not xml at all").is_err());
    }

    #[test]
    fn helpers() {
        assert_eq!(session_kind_of("Practice3"), Some(SessionType::Practice));
        assert_eq!(session_kind_of("Race"), Some(SessionType::Race));
        assert_eq!(session_kind_of("Race2"), Some(SessionType::Race));
        assert_eq!(session_kind_of("RaceResults"), None);
        assert_eq!(session_kind_of("TrackVenue"), None);
        assert_eq!(
            finish_status_of("Finished Normally"),
            FinishStatus::Finished
        );
        assert_eq!(finish_status_of("None"), FinishStatus::None);
        assert_eq!(compound_name("0,Medium"), "Medium");
        assert_eq!(compound_name("Wet"), "Wet");
        assert_eq!(strip_id("Tomas Wieczek(8)"), "Tomas Wieczek");
        assert_eq!(strip_id("Robin Vale#4821(29)"), "Robin Vale#4821");
        assert_eq!(strip_id("Immovable"), "Immovable");
        assert_eq!(parse_num::<f32>("--.----"), None);
        assert_eq!(parse_num::<i64>("12.0"), Some(12));
    }
}
