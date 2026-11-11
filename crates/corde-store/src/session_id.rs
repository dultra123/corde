//! Session identifiers: `YYYYMMDD-HHMMSS-<track-slug>-<type>` in local time,
//! e.g. `20261103-213000-paul-ricard-1a-race`. Also the directory name of the
//! session's sample files, hence the strict character set.

use chrono::{Local, TimeZone};
use corde_protocol::SessionType;

/// Longest accepted id (directory name).
pub const MAX_ID_LEN: usize = 96;

/// Lower-case ASCII slug: letters and digits kept, everything else collapsed
/// into single dashes, trimmed, at most `max` characters.
pub fn slug(s: &str, max: usize) -> String {
    let mut out = String::with_capacity(s.len());
    let mut dash = true; // swallow leading separators
    for ch in s.chars() {
        let ch = match ch {
            'a'..='z' | '0'..='9' => ch,
            'A'..='Z' => ch.to_ascii_lowercase(),
            // Common accented letters, so that "Spa-Francorchamps" or "Nürburgring" stay readable.
            'à' | 'â' | 'ä' | 'á' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'î' | 'ï' | 'í' => 'i',
            'ô' | 'ö' | 'ó' => 'o',
            'ù' | 'û' | 'ü' | 'ú' => 'u',
            'ç' => 'c',
            _ => {
                if !dash {
                    out.push('-');
                    dash = true;
                }
                continue;
            }
        };
        out.push(ch);
        dash = false;
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.len() > max {
        out.truncate(max);
        while out.ends_with('-') {
            out.pop();
        }
    }
    if out.is_empty() {
        "unknown".to_string()
    } else {
        out
    }
}

/// `"race"`, `"practice"`… (the protocol's snake_case names).
pub fn session_type_slug(t: SessionType) -> &'static str {
    match t {
        SessionType::TestDay => "test-day",
        SessionType::Practice => "practice",
        SessionType::Qualify => "qualify",
        SessionType::Warmup => "warmup",
        SessionType::Race => "race",
        SessionType::Unknown => "session",
    }
}

/// Local-time stamp `YYYYMMDD-HHMMSS` of a Unix millisecond timestamp.
pub fn stamp(wall_ms: u64) -> String {
    let secs = (wall_ms / 1000) as i64;
    match Local.timestamp_opt(secs, 0).single() {
        Some(t) => t.format("%Y%m%d-%H%M%S").to_string(),
        None => format!("{secs}"),
    }
}

/// Build the id of a new session.
pub fn make_session_id(wall_ms: u64, track_name: &str, session_type: SessionType) -> String {
    let id = format!(
        "{}-{}-{}",
        stamp(wall_ms),
        slug(track_name, 40),
        session_type_slug(session_type)
    );
    debug_assert!(valid_id(&id));
    id
}

/// Ids are directory names: ASCII letters, digits, `-`, `_`, no leading dot,
/// ≤ [`MAX_ID_LEN`] characters.
pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_ID_LEN
        && !id.starts_with('.')
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_is_safe_and_readable() {
        assert_eq!(slug("Paul Ricard - 1A", 40), "paul-ricard-1a");
        assert_eq!(
            slug("  Spa-Francorchamps (GP) ", 40),
            "spa-francorchamps-gp"
        );
        assert_eq!(slug("Nürburgring Nordschleife", 10), "nurburgrin");
        assert_eq!(slug("日本", 40), "unknown");
        assert_eq!(slug("", 40), "unknown");
    }

    #[test]
    fn id_format() {
        let id = make_session_id(1_762_200_000_000, "Paul Ricard - 1A", SessionType::Race);
        assert!(valid_id(&id), "{id}");
        assert!(id.ends_with("-paul-ricard-1a-race"), "{id}");
        assert_eq!(id.len(), 15 + 1 + "paul-ricard-1a-race".len());
        assert!(!valid_id("../x"));
        assert!(!valid_id(".hidden"));
        assert!(!valid_id("a/b"));
        assert!(!valid_id(""));
        assert!(valid_id("20261103-213000-paul-ricard-race-2"));
    }
}
