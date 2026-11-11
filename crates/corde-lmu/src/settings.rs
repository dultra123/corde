//! Locating the game and reading its settings.
//!
//! * [`webui_port`] / [`webui_port_from_str`] — the `WebUI port` key of
//!   `UserData/player/Settings.JSON` (default [`DEFAULT_WEBUI_PORT`]). The file
//!   is rFactor-style JSON: sections of `"Key": value` pairs, each followed by
//!   a `"Key#": "comment"` entry. Parsing is tolerant: standard JSON first,
//!   then a plain text scan when the file is not strictly valid.
//! * [`LmuPaths`] / [`locate_game`] — the install folder from the Steam
//!   libraries (`libraryfolders.vdf`, default paths on Windows and Linux) or
//!   from `LMU_Data.paths.userData` when the game is running.
//!
//! Nothing here touches the Windows registry: Steam always writes
//! `libraryfolders.vdf`, which lists every library on every drive.

use crate::layout::SharedMemoryObjectOut;
use std::path::{Path, PathBuf};

/// Default port of the local REST API (`WebUI port`).
pub const DEFAULT_WEBUI_PORT: u16 = 6397;
/// Steam application id of Le Mans Ultimate.
pub const STEAM_APP_ID: u32 = 2_399_420;
/// Name of the game folder under `steamapps/common`.
pub const GAME_FOLDER_NAME: &str = "Le Mans Ultimate";

// ---------------------------------------------------------------------------
// Settings.JSON
// ---------------------------------------------------------------------------

/// Path of `Settings.JSON` for a `UserData` folder.
pub fn settings_json_path(user_data: &Path) -> PathBuf {
    user_data.join("player").join("Settings.JSON")
}

/// Path of the setups folder (`UserData/player/Settings/<Track>/*.svm`).
pub fn setups_dir(user_data: &Path) -> PathBuf {
    user_data.join("player").join("Settings")
}

/// Path of the native telemetry recordings (`UserData/Telemetry/*.duckdb`).
pub fn telemetry_dir(user_data: &Path) -> PathBuf {
    user_data.join("Telemetry")
}

/// Path of the session results files (`UserData/Log/Results/*.xml`).
pub fn results_dir(user_data: &Path) -> PathBuf {
    user_data.join("Log").join("Results")
}

/// Read the `WebUI port` of a `Settings.JSON` file. `None` when the file
/// cannot be read or has no such key (callers fall back to
/// [`DEFAULT_WEBUI_PORT`]).
pub fn webui_port(settings_json: &Path) -> Option<u16> {
    let text = std::fs::read_to_string(settings_json).ok()?;
    webui_port_from_str(&text)
}

/// Same as [`webui_port`] on the file's content.
pub fn webui_port_from_str(text: &str) -> Option<u16> {
    find_key_number(text, "WebUI port").and_then(|v| u16::try_from(v).ok())
}

/// Find the integer value of `key` anywhere in an rFactor-style JSON text
/// (any nesting depth, key compared case-insensitively, `"Key#"` comment
/// entries ignored). Falls back to a textual scan when the JSON is invalid
/// (the game occasionally writes trailing commas or `NaN`).
pub fn find_key_number(text: &str, key: &str) -> Option<i64> {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(text) {
        if let Some(n) = find_key_in_value(&v, key) {
            return Some(n);
        }
    }
    scan_key_number(text, key)
}

fn find_key_in_value(v: &serde_json::Value, key: &str) -> Option<i64> {
    match v {
        serde_json::Value::Object(map) => {
            for (k, val) in map {
                if k.trim().eq_ignore_ascii_case(key) {
                    if let Some(n) = value_as_i64(val) {
                        return Some(n);
                    }
                }
            }
            map.values().find_map(|val| find_key_in_value(val, key))
        }
        serde_json::Value::Array(items) => items.iter().find_map(|val| find_key_in_value(val, key)),
        _ => None,
    }
}

fn value_as_i64(v: &serde_json::Value) -> Option<i64> {
    match v {
        serde_json::Value::Number(n) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)),
        serde_json::Value::String(s) => s.trim().parse::<i64>().ok(),
        _ => None,
    }
}

/// Textual scan: `"<key>" : <number>` with any whitespace, case-insensitive.
fn scan_key_number(text: &str, key: &str) -> Option<i64> {
    let lower = text.to_ascii_lowercase();
    let needle = format!("\"{}\"", key.to_ascii_lowercase());
    let mut from = 0;
    while let Some(pos) = lower[from..].find(&needle) {
        let after = from + pos + needle.len();
        let rest = &text[after..];
        let rest = rest.trim_start();
        if let Some(rest) = rest.strip_prefix(':') {
            let rest = rest.trim_start().trim_start_matches('"');
            let digits: String = rest
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == '-')
                .collect();
            if let Ok(n) = digits.parse::<i64>() {
                return Some(n);
            }
        }
        from = after;
    }
    None
}

// ---------------------------------------------------------------------------
// Game folder
// ---------------------------------------------------------------------------

/// The folders Corde needs around an installation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LmuPaths {
    /// `…/steamapps/common/Le Mans Ultimate`
    pub game_dir: PathBuf,
    /// `<game_dir>/UserData`
    pub user_data: PathBuf,
}

impl LmuPaths {
    pub fn from_game_dir(game_dir: impl Into<PathBuf>) -> Self {
        let game_dir = game_dir.into();
        let user_data = game_dir.join("UserData");
        Self {
            game_dir,
            user_data,
        }
    }

    /// From a `UserData` folder (e.g. `LMU_Data.paths.userData`).
    pub fn from_user_data(user_data: impl Into<PathBuf>) -> Self {
        let user_data = user_data.into();
        let game_dir = user_data
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| user_data.clone());
        Self {
            game_dir,
            user_data,
        }
    }

    pub fn settings_json(&self) -> PathBuf {
        settings_json_path(&self.user_data)
    }

    pub fn setups_dir(&self) -> PathBuf {
        setups_dir(&self.user_data)
    }

    pub fn telemetry_dir(&self) -> PathBuf {
        telemetry_dir(&self.user_data)
    }

    pub fn results_dir(&self) -> PathBuf {
        results_dir(&self.user_data)
    }

    /// `WebUI port` of this installation, or the default.
    pub fn webui_port(&self) -> u16 {
        webui_port(&self.settings_json()).unwrap_or(DEFAULT_WEBUI_PORT)
    }

    /// `true` when `UserData` exists on disk.
    pub fn exists(&self) -> bool {
        self.user_data.is_dir()
    }
}

/// `UserData` folder announced by the game in `LMU_Data.paths.userData`.
///
/// On Windows the path is used as is. On Linux the game writes a Windows path
/// (`C:\…` or `Z:\…`) that is mapped to the Proton prefix of the app
/// (`<steam>/steamapps/compatdata/2399420/pfx/drive_c`) when it can be found,
/// else returned untouched.
pub fn user_data_from_shm(raw: &SharedMemoryObjectOut) -> Option<LmuPaths> {
    let p = raw.paths.user_data();
    if p.is_empty() {
        return None;
    }
    let path = if cfg!(windows) {
        PathBuf::from(p)
    } else {
        windows_path_to_unix(&p, proton_prefix().as_deref())
    };
    Some(LmuPaths::from_user_data(path))
}

/// Map a Windows path to the host filesystem under a Wine/Proton prefix:
/// `Z:\x\y` → `/x/y`, `C:\x` → `<prefix>/drive_c/x`, other drives →
/// `<prefix>/dosdevices/<d>:/x`. Without a prefix the path is returned with
/// forward slashes.
pub fn windows_path_to_unix(path: &str, prefix: Option<&Path>) -> PathBuf {
    let bytes = path.as_bytes();
    if bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic() {
        let drive = (bytes[0] as char).to_ascii_lowercase();
        let rest = path[2..].replace('\\', "/");
        let rest = rest.trim_start_matches('/');
        if drive == 'z' {
            return PathBuf::from(format!("/{rest}"));
        }
        if let Some(prefix) = prefix {
            // Built as a string on purpose: on Windows `Path::join("d:")`
            // would treat the drive as a prefix and discard everything before.
            let base = prefix.to_string_lossy();
            let base = base.trim_end_matches('/');
            return if drive == 'c' {
                PathBuf::from(format!("{base}/drive_c/{rest}"))
            } else {
                PathBuf::from(format!("{base}/dosdevices/{drive}:/{rest}"))
            };
        }
    }
    PathBuf::from(path.replace('\\', "/"))
}

/// The Proton prefix of the game (`…/steamapps/compatdata/2399420/pfx`), when
/// one exists in a known Steam library.
pub fn proton_prefix() -> Option<PathBuf> {
    steam_library_dirs()
        .into_iter()
        .map(|lib| {
            lib.join("steamapps")
                .join("compatdata")
                .join(STEAM_APP_ID.to_string())
                .join("pfx")
        })
        .find(|p| p.is_dir())
}

/// Steam root folders to look at on this platform (existing or not).
pub fn default_steam_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if cfg!(windows) {
        for var in ["ProgramFiles(x86)", "ProgramFiles"] {
            if let Some(pf) = std::env::var_os(var) {
                roots.push(PathBuf::from(pf).join("Steam"));
            }
        }
        roots.push(PathBuf::from(r"C:\Program Files (x86)\Steam"));
        roots.push(PathBuf::from(r"C:\Program Files\Steam"));
    } else if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        roots.push(home.join(".steam").join("steam"));
        roots.push(home.join(".steam").join("root"));
        roots.push(home.join(".local").join("share").join("Steam"));
        roots.push(
            home.join(".var")
                .join("app")
                .join("com.valvesoftware.Steam")
                .join(".local")
                .join("share")
                .join("Steam"),
        );
    }
    dedup_paths(roots)
}

/// Every Steam library folder found: the default roots plus the entries of
/// their `libraryfolders.vdf`.
pub fn steam_library_dirs() -> Vec<PathBuf> {
    let mut libs = Vec::new();
    for root in default_steam_roots() {
        if !root.is_dir() {
            continue;
        }
        libs.push(root.clone());
        for vdf in [
            root.join("steamapps").join("libraryfolders.vdf"),
            root.join("config").join("libraryfolders.vdf"),
        ] {
            if let Ok(text) = std::fs::read_to_string(&vdf) {
                libs.extend(parse_libraryfolders_vdf(&text));
            }
        }
    }
    dedup_paths(libs)
}

/// Extract the `"path"` values of a `libraryfolders.vdf` (Valve KeyValues
/// text). Backslashes are escaped (`\\`) in the file.
pub fn parse_libraryfolders_vdf(text: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for line in text.lines() {
        let mut tokens = vdf_tokens(line).into_iter();
        if let (Some(key), Some(value)) = (tokens.next(), tokens.next()) {
            if key.eq_ignore_ascii_case("path") && !value.is_empty() {
                out.push(PathBuf::from(value));
            }
        }
    }
    out
}

/// Quoted tokens of one VDF line, with `\\` and `\"` unescaped.
fn vdf_tokens(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                if in_quotes {
                    out.push(std::mem::take(&mut cur));
                }
                in_quotes = !in_quotes;
            }
            '\\' if in_quotes => match chars.next() {
                Some('\\') => cur.push('\\'),
                Some('"') => cur.push('"'),
                Some('n') => cur.push('\n'),
                Some('t') => cur.push('\t'),
                Some(other) => {
                    cur.push('\\');
                    cur.push(other);
                }
                None => cur.push('\\'),
            },
            _ if in_quotes => cur.push(c),
            _ => {}
        }
    }
    out
}

/// Candidate installation folders, most likely first (not checked on disk).
pub fn candidate_game_dirs() -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = steam_library_dirs()
        .into_iter()
        .map(|lib| lib.join("steamapps").join("common").join(GAME_FOLDER_NAME))
        .collect();
    if cfg!(windows) {
        out.push(PathBuf::from(
            r"C:\Program Files (x86)\Steam\steamapps\common\Le Mans Ultimate",
        ));
    }
    dedup_paths(out)
}

/// First installation whose `UserData` folder exists on disk.
pub fn locate_game() -> Option<LmuPaths> {
    candidate_game_dirs()
        .into_iter()
        .map(LmuPaths::from_game_dir)
        .find(LmuPaths::exists)
}

/// [`locate_game`], preferring the folder announced by a running game
/// (`LMU_Data.paths.userData`) when it exists on disk.
pub fn locate_game_with_shm(raw: Option<&SharedMemoryObjectOut>) -> Option<LmuPaths> {
    if let Some(p) = raw.and_then(user_data_from_shm) {
        if p.exists() {
            return Some(p);
        }
    }
    locate_game()
}

fn dedup_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    for p in paths {
        if !out.contains(&p) {
            out.push(p);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SETTINGS: &str = r#"{
  "Graphics Options":{
    "Allow HUD":1,
    "Allow HUD#":"Whether to show the HUD"
  },
  "Miscellaneous":{
    "WebUI port":6397,
    "WebUI port#":"Port used by the web UI backend",
    "Automatically Record Telemetry":true
  }
}"#;

    #[test]
    fn reads_webui_port_from_json() {
        assert_eq!(webui_port_from_str(SETTINGS), Some(6397));
        assert_eq!(webui_port_from_str("{}"), None);
        assert_eq!(
            webui_port_from_str(r#"{"Misc":{"webui PORT":"6400"}}"#),
            Some(6400)
        );
    }

    #[test]
    fn reads_webui_port_from_broken_json() {
        // trailing comma + NaN: serde rejects it, the scan still finds the key
        let broken = r#"{ "Miscellaneous":{ "Speed": NaN, "WebUI port" : 6398, } }"#;
        assert_eq!(webui_port_from_str(broken), Some(6398));
        assert_eq!(
            webui_port_from_str(r#"{"WebUI port#":"comment only"}"#),
            None
        );
    }

    #[test]
    fn parses_libraryfolders_vdf() {
        let vdf = r#""libraryfolders"
{
	"0"
	{
		"path"		"C:\\Program Files (x86)\\Steam"
		"label"		""
		"apps"
		{
			"2399420"		"12345"
		}
	}
	"1"
	{
		"path"		"D:\\SteamLibrary"
	}
}"#;
        let libs = parse_libraryfolders_vdf(vdf);
        assert_eq!(
            libs,
            vec![
                PathBuf::from(r"C:\Program Files (x86)\Steam"),
                PathBuf::from(r"D:\SteamLibrary")
            ]
        );
        let linux = "\"path\"\t\t\"/home/me/.local/share/Steam\"\n";
        assert_eq!(
            parse_libraryfolders_vdf(linux),
            vec![PathBuf::from("/home/me/.local/share/Steam")]
        );
    }

    #[test]
    fn windows_paths_map_to_prefix() {
        let prefix = Path::new("/home/me/.steam/steam/steamapps/compatdata/2399420/pfx");
        assert_eq!(
            windows_path_to_unix(r"C:\Program Files (x86)\Steam\steamapps\common\Le Mans Ultimate\UserData", Some(prefix)),
            PathBuf::from("/home/me/.steam/steam/steamapps/compatdata/2399420/pfx/drive_c/Program Files (x86)/Steam/steamapps/common/Le Mans Ultimate/UserData")
        );
        assert_eq!(
            windows_path_to_unix(r"Z:\home\me\games\UserData", Some(prefix)),
            PathBuf::from("/home/me/games/UserData")
        );
        assert_eq!(
            windows_path_to_unix(r"D:\Games\LMU\UserData", Some(prefix)),
            PathBuf::from(
                "/home/me/.steam/steam/steamapps/compatdata/2399420/pfx/dosdevices/d:/Games/LMU/UserData"
            )
        );
        assert_eq!(
            windows_path_to_unix(r"C:\x\UserData", None),
            PathBuf::from("C:/x/UserData")
        );
    }

    #[test]
    fn paths_helpers() {
        let p = LmuPaths::from_game_dir("/games/Le Mans Ultimate");
        assert_eq!(
            p.user_data,
            PathBuf::from("/games/Le Mans Ultimate/UserData")
        );
        assert_eq!(
            p.settings_json(),
            PathBuf::from("/games/Le Mans Ultimate/UserData/player/Settings.JSON")
        );
        assert_eq!(
            p.setups_dir(),
            PathBuf::from("/games/Le Mans Ultimate/UserData/player/Settings")
        );
        assert_eq!(
            p.results_dir(),
            PathBuf::from("/games/Le Mans Ultimate/UserData/Log/Results")
        );
        let q = LmuPaths::from_user_data("/games/Le Mans Ultimate/UserData");
        assert_eq!(q, p);
        assert_eq!(q.webui_port(), DEFAULT_WEBUI_PORT);
        assert!(!candidate_game_dirs().is_empty() || !cfg!(windows));
    }

    #[test]
    fn user_data_from_block() {
        let mut raw = SharedMemoryObjectOut::boxed();
        assert!(user_data_from_shm(&raw).is_none());
        crate::layout::set_cstr(&mut raw.paths.user_data, r"C:\LMU\UserData");
        let p = user_data_from_shm(&raw).unwrap();
        assert!(p.user_data.to_string_lossy().ends_with("UserData"));
    }
}
