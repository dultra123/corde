//! The session text of the game, read as what it is: lines of `Key: value`, nested by
//! indentation, with lists of `- Key: value` items.
//!
//! It looks like YAML and is not always valid YAML (a driver's name is written as typed,
//! quotes and colons included), so a YAML parser can refuse a whole session because of one
//! name. This reader never refuses anything: a line it does not understand is skipped.
//!
//! The text is ISO-8859-1 unless its first lines say `Encoding: UTF8`.

/// A value of the session text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
    Scalar(String),
    Map(Vec<(String, Node)>),
    List(Vec<Node>),
}

static EMPTY: Node = Node::Scalar(String::new());

impl Node {
    /// The value under `key` (an empty scalar when there is none, so that lookups chain).
    pub fn get(&self, key: &str) -> &Node {
        match self {
            Node::Map(entries) => entries
                .iter()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v)
                .unwrap_or(&EMPTY),
            _ => &EMPTY,
        }
    }

    /// The items of a list (none for anything else).
    pub fn items(&self) -> &[Node] {
        match self {
            Node::List(items) => items,
            _ => &[],
        }
    }

    pub fn str(&self) -> &str {
        match self {
            Node::Scalar(s) => s,
            _ => "",
        }
    }

    /// The number a value starts with: « 5.79 km » → 5.79, « 1800.0000 sec » → 1800.
    pub fn number(&self) -> Option<f64> {
        let s = self.str().trim();
        let end = s
            .char_indices()
            .find(|(i, c)| {
                !(c.is_ascii_digit() || *c == '.' || (*i == 0 && (*c == '-' || *c == '+')))
            })
            .map(|(i, _)| i)
            .unwrap_or(s.len());
        s[..end].parse().ok()
    }

    pub fn int(&self) -> Option<i64> {
        self.number().map(|v| v as i64)
    }
}

/// Bytes of the game → text.
pub fn decode(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    let bytes = &bytes[..end];
    let head = &bytes[..bytes.len().min(256)];
    if head.windows(14).any(|w| w == b"Encoding: UTF8") {
        String::from_utf8_lossy(bytes).into_owned()
    } else {
        bytes.iter().map(|&b| b as char).collect()
    }
}

struct Line<'a> {
    indent: usize,
    text: &'a str,
}

fn unquote(v: &str) -> String {
    let v = v.trim();
    let quoted = v.len() >= 2
        && ((v.starts_with('"') && v.ends_with('"')) || (v.starts_with('\'') && v.ends_with('\'')));
    if quoted {
        v[1..v.len() - 1].to_string()
    } else {
        v.to_string()
    }
}

/// `Key: value` → (key, value); `Key:` → (key, "").
fn key_value(text: &str) -> Option<(&str, &str)> {
    if let Some((k, v)) = text.split_once(": ") {
        return Some((k.trim(), v));
    }
    text.strip_suffix(':').map(|k| (k.trim(), ""))
}

/// The whole text → a tree. Never fails.
pub fn parse(text: &str) -> Node {
    let mut lines: Vec<Line> = text
        .lines()
        .filter_map(|raw| {
            let trimmed = raw.trim_end();
            let content = trimmed.trim_start_matches(' ');
            if content.is_empty() || content == "---" || content == "..." {
                return None;
            }
            Some(Line {
                indent: trimmed.len() - content.len(),
                text: content,
            })
        })
        .collect();
    let mut pos = 0;
    let indent = lines.first().map(|l| l.indent).unwrap_or(0);
    map(&mut lines, &mut pos, indent)
}

fn is_item(text: &str) -> bool {
    text == "-" || text.starts_with("- ")
}

fn map(lines: &mut [Line], pos: &mut usize, indent: usize) -> Node {
    let mut entries = Vec::new();
    while let Some(line) = lines.get(*pos) {
        if line.indent != indent || is_item(line.text) {
            if line.indent > indent {
                // A line deeper than expected with no parent: skip it.
                *pos += 1;
                continue;
            }
            break;
        }
        let Some((key, value)) = key_value(line.text) else {
            *pos += 1;
            continue;
        };
        let key = key.to_string();
        *pos += 1;
        if !value.trim().is_empty() {
            // « Key: [] » is the game's empty list.
            let node = match value.trim() {
                "[]" => Node::List(Vec::new()),
                v => Node::Scalar(unquote(v)),
            };
            entries.push((key, node));
            continue;
        }
        let child = match lines.get(*pos) {
            // A list may sit at the indentation of its key.
            Some(next) if is_item(next.text) && next.indent >= indent => {
                let at = next.indent;
                list(lines, pos, at)
            }
            Some(next) if next.indent > indent => {
                let at = next.indent;
                map(lines, pos, at)
            }
            _ => Node::Scalar(String::new()),
        };
        entries.push((key, child));
    }
    Node::Map(entries)
}

fn list(lines: &mut [Line], pos: &mut usize, indent: usize) -> Node {
    let mut items = Vec::new();
    while let Some(line) = lines.get(*pos) {
        if line.indent != indent || !is_item(line.text) {
            break;
        }
        let rest = line.text[1..].trim_start_matches(' ');
        if rest.is_empty() {
            *pos += 1;
            // « - » alone: the item is the block below it.
            match lines.get(*pos) {
                Some(next) if next.indent > indent => {
                    let at = next.indent;
                    items.push(map(lines, pos, at));
                }
                _ => items.push(Node::Scalar(String::new())),
            }
        } else if key_value(rest).is_some() {
            // « - Key: value »: the first line of a block that goes on two columns in.
            let at = indent + (line.text.len() - rest.len());
            lines[*pos] = Line {
                indent: at,
                text: rest,
            };
            items.push(map(lines, pos, at));
        } else {
            items.push(Node::Scalar(unquote(rest)));
            *pos += 1;
        }
    }
    Node::List(items)
}

/// Keys whose values say nothing about anyone: kept in a diagnostic copy. Every other
/// value is removed from it (names, identifiers, licence, paths of files, the setup).
const HARMLESS: &[&str] = &[
    "Encoding",
    "TrackName",
    "TrackID",
    "TrackLength",
    "TrackLengthOfficial",
    "TrackDisplayName",
    "TrackDisplayShortName",
    "TrackConfigName",
    "TrackNumTurns",
    "TrackPitSpeedLimit",
    "TrackType",
    "TrackDirection",
    "TrackWeatherType",
    "TrackSkies",
    "TrackSurfaceTemp",
    "TrackAirTemp",
    "TrackWindVel",
    "TrackWindDir",
    "TrackRelativeHumidity",
    "TrackFogLevel",
    "TrackPrecipitation",
    "EventType",
    "Category",
    "SimMode",
    "TeamRacing",
    "NumCarClasses",
    "NumCarTypes",
    "HeatRacing",
    "BuildVersion",
    "NumStarters",
    "StandingStart",
    "ShortParadeLap",
    "IsFixedSetup",
    "SessionNum",
    "SessionLaps",
    "SessionTime",
    "SessionType",
    "SessionName",
    "SessionSubType",
    "Position",
    "ClassPosition",
    "CarIdx",
    "Lap",
    "Time",
    "FastestLap",
    "FastestTime",
    "LastTime",
    "LapsComplete",
    "DriverCarIdx",
    "PaceCarIdx",
    "DriverCarIsElectric",
    "DriverCarIdleRPM",
    "DriverCarRedLine",
    "DriverCarFuelKgPerLtr",
    "DriverCarFuelMaxLtr",
    "DriverCarMaxFuelPct",
    "DriverCarGearNumForward",
    "DriverCarSLFirstRPM",
    "DriverCarSLShiftRPM",
    "DriverCarSLLastRPM",
    "DriverCarSLBlinkRPM",
    "DriverPitTrkPct",
    "DriverCarEstLapTime",
    "TireIndex",
    "TireCompoundType",
    "CarNumber",
    "CarNumberRaw",
    "CarPath",
    "CarClassID",
    "CarID",
    "CarIsPaceCar",
    "CarIsAI",
    "CarIsElectric",
    "CarScreenName",
    "CarScreenNameShort",
    "CarClassShortName",
    "CarClassRelSpeed",
    "CarClassEstLapTime",
    "IsSpectator",
    "SectorNum",
    "SectorStartPct",
];

/// The session text without anything that names or identifies somebody: for the copy
/// one sends when a value looks wrong.
pub fn anonymise(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for raw in text.lines() {
        let content = raw.trim_start_matches(' ');
        let body = content.strip_prefix("- ").unwrap_or(content);
        match body.split_once(": ") {
            Some((key, value)) if !value.trim().is_empty() && !HARMLESS.contains(&key.trim()) => {
                // Keep the line (the structure), drop what it says.
                out.push_str(&raw[..raw.len() - value.len() - 1]);
            }
            _ => out.push_str(raw),
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEXT: &str = "---
WeekendInfo:
 TrackName: spa up
 TrackLength: 6.93 km
 TrackDisplayName: Circuit de Spa-Francorchamps
 TrackConfigName: Grand Prix Pits
 SubSessionID: 0
 WeekendOptions:
  TimeOfDay: 12:55 pm
  IsFixedSetup: 1
 TelemetryOptions:
  TelemetryDiskFile: \"\"

SessionInfo:
 Sessions:
 - SessionNum: 0
   SessionLaps: unlimited
   SessionTime: 1800.0000 sec
   SessionType: Practice
   SessionSubType:
   ResultsPositions:
   - Position: 1
     CarIdx: 3
   - Position: 2
     CarIdx: 0
   ResultsFastestLap:
   - CarIdx: 255
     FastestTime: -1.0000
   ResultsOfficial: 0
 - SessionNum: 1
   SessionType: Race
   ResultsPositions: []

DriverInfo:
 DriverCarIdx: 0
 DriverCarFuelMaxLtr: 120.000
 Drivers:
 - CarIdx: 0
   UserName: A \"quoted\": name's
   CarNumber: \"64\"
   CarScreenName: BMW M4 GT3
 - CarIdx: 3
   UserName: Second Driver
   CarNumber: \"7\"
...
";

    #[test]
    fn nested_blocks_and_lists_are_read() {
        let root = parse(TEXT);
        let w = root.get("WeekendInfo");
        assert_eq!(w.get("TrackName").str(), "spa up");
        assert_eq!(w.get("TrackLength").number(), Some(6.93));
        assert_eq!(w.get("WeekendOptions").get("TimeOfDay").str(), "12:55 pm");
        assert_eq!(w.get("WeekendOptions").get("IsFixedSetup").int(), Some(1));
        assert_eq!(w.get("TelemetryOptions").get("TelemetryDiskFile").str(), "");
        let sessions = root.get("SessionInfo").get("Sessions").items();
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0].get("SessionTime").number(), Some(1800.0));
        assert_eq!(sessions[0].get("SessionLaps").number(), None, "unlimited");
        assert_eq!(sessions[0].get("SessionSubType").str(), "");
        let results = sessions[0].get("ResultsPositions").items();
        assert_eq!(results.len(), 2);
        assert_eq!(results[1].get("CarIdx").int(), Some(0));
        // What follows a nested list still belongs to the item.
        assert_eq!(sessions[0].get("ResultsOfficial").int(), Some(0));
        assert_eq!(sessions[1].get("SessionType").str(), "Race");
        assert!(sessions[1].get("ResultsPositions").items().is_empty());
        let drivers = root.get("DriverInfo").get("Drivers").items();
        assert_eq!(drivers.len(), 2);
        // A name a YAML parser would choke on is just a name.
        assert_eq!(drivers[0].get("UserName").str(), "A \"quoted\": name's");
        assert_eq!(drivers[0].get("CarNumber").str(), "64");
        assert_eq!(drivers[1].get("CarIdx").int(), Some(3));
        // Missing things chain to nothing instead of failing.
        assert_eq!(root.get("Nope").get("Deeper").str(), "");
        assert!(root.get("Nope").items().is_empty());
    }

    #[test]
    fn garbage_is_survived() {
        for text in [
            "",
            "\n\n",
            "no colon here\n  nor here",
            "- a\n- b",
            ":\n :\n",
            "a:\n   b: 1\n  c: 2\n d: 3",
        ] {
            let _ = parse(text);
        }
        assert_eq!(parse("a:\n   b: 1\n d: 3").get("a").get("b").int(), Some(1));
    }

    #[test]
    fn the_encoding_follows_what_the_text_says() {
        // ISO-8859-1 by default: 0xE9 is « é ».
        assert_eq!(decode(b"UserName: Ren\xE9\0junk"), "UserName: René");
        // UTF-8 when announced.
        let utf8 = "WeekendInfo:\n Encoding: UTF8\n UserName: René".as_bytes();
        assert!(decode(utf8).ends_with("René"));
    }

    #[test]
    fn a_diagnostic_copy_names_nobody() {
        let clean = anonymise(TEXT);
        assert!(!clean.contains("quoted"), "{clean}");
        assert!(!clean.contains("Second Driver"));
        // The structure and what describes the track and the cars are kept.
        let root = parse(&clean);
        assert_eq!(root.get("WeekendInfo").get("TrackName").str(), "spa up");
        let drivers = root.get("DriverInfo").get("Drivers").items();
        assert_eq!(drivers.len(), 2);
        assert_eq!(drivers[0].get("UserName").str(), "");
        assert_eq!(drivers[0].get("CarScreenName").str(), "BMW M4 GT3");
        assert_eq!(drivers[1].get("CarIdx").int(), Some(3));
        assert_eq!(
            root.get("SessionInfo").get("Sessions").items()[0]
                .get("SessionType")
                .str(),
            "Practice"
        );
    }
}
