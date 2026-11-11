//! The driver's setup library: every `.svm` file the game saved under
//! `UserData/player/Settings/<TrackFolder>/`, and how to pick the one a
//! session was driven with.
//!
//! * [`scan`] — one level of folders, tolerant: a file that cannot be read or
//!   that does not look like a setup is listed in `unreadable`, never fatal.
//! * [`vehicle_matches`] — tolerant comparison between the car of a session
//!   (`vehicle_model`, e.g. `BMW M4 LMGT3`) and `VehicleClassSetting`
//!   (`BMW_M4_LMGT3 GT3 WEC2026`, `GT3 Porsche_911_GT3_R_LMGT3 WEC2025`,
//!   `ELMS2025 LMP3 Ligier_JS_P325`): the distinctive words of the model
//!   (not the class or the season) must all appear in the setting, compared
//!   on lower-case letters and digits.
//! * [`track_folder`] — the folder of a track: the game names it after the
//!   track's files (`Longbeach`, `Road Atlanta`, `Lemans`…), not after the
//!   name shown in the session (`Grand Prix of Long Beach`, `Circuit de la
//!   Sarthe`…), hence a table of aliases ([`TRACK_ALIASES`]) and a fuzzy
//!   fallback on the folders that exist.
//! * [`choose_base`] — the file most recently modified before the end of the
//!   session, in the track's folder first.
//! * [`resolve_relative`] — turns a `Folder/File.svm` coming from the UI into
//!   a path that cannot leave the `Settings` folder.

use crate::svm::Setup;
use corde_protocol::SetupFileInfo;
use std::path::{Component, Path, PathBuf};
use std::time::UNIX_EPOCH;

/// Result of [`scan`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LibraryScan {
    /// Readable setups, most recently modified first.
    pub files: Vec<SetupFileInfo>,
    /// `Folder/File.svm` of the files that could not be read as a setup.
    pub unreadable: Vec<String>,
}

fn is_svm(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("svm"))
}

fn mtime_ms(path: &Path) -> i64 {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Read one setup file into its library entry (`folder` / `file` from the path).
pub fn file_info(path: &Path) -> Option<(SetupFileInfo, Setup)> {
    let setup = Setup::read(path).ok()?;
    if !setup.is_plausible() {
        return None;
    }
    let folder = path
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())?
        .to_string();
    let file = path.file_name()?.to_str()?.to_string();
    Some((
        SetupFileInfo {
            path: format!("{folder}/{file}"),
            folder,
            file,
            vehicle: setup.vehicle_class().unwrap_or_default(),
            mtime_ms: mtime_ms(path),
            notes: setup.notes().unwrap_or_default(),
        },
        setup,
    ))
}

/// Every `<settings_dir>/<Folder>/*.svm`. Never fails: a missing folder gives
/// an empty library.
pub fn scan(settings_dir: &Path) -> LibraryScan {
    let mut out = LibraryScan::default();
    let Ok(folders) = std::fs::read_dir(settings_dir) else {
        return out;
    };
    let mut folders: Vec<PathBuf> = folders
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    folders.sort();
    for folder in folders {
        let Ok(entries) = std::fs::read_dir(&folder) else {
            continue;
        };
        let mut files: Vec<PathBuf> = entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_file() && is_svm(p))
            .collect();
        files.sort();
        for path in files {
            match file_info(&path) {
                Some((info, _)) => out.files.push(info),
                None => {
                    let rel = path
                        .strip_prefix(settings_dir)
                        .map(|p| p.to_string_lossy().replace('\\', "/"))
                        .unwrap_or_else(|_| path.display().to_string());
                    out.unreadable.push(rel);
                }
            }
        }
    }
    out.files
        .sort_by(|a, b| b.mtime_ms.cmp(&a.mtime_ms).then(a.path.cmp(&b.path)));
    out
}

/// Why a relative setup path was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathError {
    /// Not `Folder/File.svm` (absolute, `..`, more or fewer levels…).
    NotInSettings(String),
    /// The extension is not `.svm`.
    NotSvm(String),
}

impl std::fmt::Display for PathError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotInSettings(p) => write!(
                f,
                "chemin refusé : « {p} » doit être de la forme Dossier/Fichier.svm dans le dossier Settings du jeu"
            ),
            Self::NotSvm(p) => write!(f, "chemin refusé : « {p} » n'est pas un fichier .svm"),
        }
    }
}

impl std::error::Error for PathError {}

/// `Folder/File.svm` (or `Folder\File.svm`) → `<settings_dir>/Folder/File.svm`,
/// refusing anything that could leave `settings_dir`: absolute paths, drive
/// letters, `..`, `.`, empty parts, more or fewer than two parts, another
/// extension.
pub fn resolve_relative(settings_dir: &Path, rel: &str) -> Result<PathBuf, PathError> {
    let norm = rel.replace('\\', "/");
    let parts: Vec<&str> = norm.split('/').collect();
    let bad = || PathError::NotInSettings(rel.to_string());
    if parts.len() != 2 {
        return Err(bad());
    }
    for part in &parts {
        if part.is_empty()
            || *part == "."
            || *part == ".."
            || part.contains(':')
            || part.trim() != *part
            || part.chars().any(|c| c.is_control())
        {
            return Err(bad());
        }
        let comps: Vec<Component> = Path::new(part).components().collect();
        if comps.len() != 1 || !matches!(comps[0], Component::Normal(_)) {
            return Err(bad());
        }
    }
    if !is_svm(Path::new(parts[1])) {
        return Err(PathError::NotSvm(rel.to_string()));
    }
    Ok(settings_dir.join(parts[0]).join(parts[1]))
}

// ---------------------------------------------------------------------------
// Matching
// ---------------------------------------------------------------------------

/// Lower case, accents folded, every other character → space.
pub fn fold(s: &str) -> String {
    s.chars()
        .flat_map(|c| c.to_lowercase())
        .map(|c| match c {
            'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => 'a',
            'ç' => 'c',
            'è' | 'é' | 'ê' | 'ë' => 'e',
            'ì' | 'í' | 'î' | 'ï' => 'i',
            'ñ' => 'n',
            'ò' | 'ó' | 'ô' | 'õ' | 'ö' => 'o',
            'ù' | 'ú' | 'û' | 'ü' => 'u',
            'ý' | 'ÿ' => 'y',
            c if c.is_ascii_alphanumeric() => c,
            _ => ' ',
        })
        .collect()
}

/// Letters and digits only (`"BMW_M4_LMGT3 GT3"` → `"bmwm4lmgt3gt3"`).
pub fn alnum(s: &str) -> String {
    fold(s).chars().filter(|c| *c != ' ').collect()
}

fn words(s: &str) -> Vec<String> {
    fold(s).split_whitespace().map(str::to_string).collect()
}

/// Words that name a class, a series or a season, not a car.
fn is_generic_word(w: &str) -> bool {
    const GENERIC: &[&str] = &[
        "gt3", "lmgt3", "gte", "lmgte", "lmp1", "lmp2", "lmp3", "lmh", "lmdh", "hypercar", "hyper",
        "wec", "elms", "imsa", "gtp", "evo", "evo2",
    ];
    if GENERIC.contains(&w) {
        return true;
    }
    // Seasons: 2025, wec2026, elms2025…
    let digits: String = w.chars().filter(|c| c.is_ascii_digit()).collect();
    let letters: String = w.chars().filter(|c| c.is_ascii_alphabetic()).collect();
    digits.len() == 4
        && (digits.starts_with("19") || digits.starts_with("20"))
        && (letters.is_empty() || ["wec", "elms", "imsa", "lmu"].contains(&letters.as_str()))
}

/// The words of a car name that tell it apart (`BMW M4 LMGT3` → `bmw`, `m4`).
pub fn distinctive_words(model: &str) -> Vec<String> {
    words(model)
        .into_iter()
        .filter(|w| !is_generic_word(w))
        .collect()
}

/// `true` when the setup's `VehicleClassSetting` is for the session's car.
/// Every distinctive word of `vehicle_model` must appear in the setting
/// (letters and digits compared). A model without distinctive word never
/// matches (`GT3` alone could be any car).
pub fn vehicle_matches(vehicle_model: &str, vehicle_class_setting: &str) -> bool {
    let wanted = distinctive_words(vehicle_model);
    if wanted.is_empty() {
        return false;
    }
    let setting_words = words(vehicle_class_setting);
    let setting = alnum(vehicle_class_setting);
    wanted.iter().all(|w| {
        // Short words (`r`, `m4`) must be whole words; longer ones may be glued
        // to their neighbours (`911GT3R`).
        if w.len() <= 2 {
            setting_words.iter().any(|s| s == w)
        } else {
            setting.contains(w.as_str())
        }
    })
}

/// Folders the game uses for the circuits of Le Mans Ultimate, with words of
/// the names the game shows for them (`TrackName` / `TrackVenue`).
pub const TRACK_ALIASES: &[(&str, &[&str])] = &[
    ("Longbeach", &["long beach", "longbeach"]),
    ("Road Atlanta", &["road atlanta", "michelin raceway"]),
    ("Spa", &["spa", "francorchamps"]),
    ("Lemans", &["sarthe", "le mans", "lemans"]),
    ("Paulricard", &["paul ricard", "castellet"]),
    ("Sebring", &["sebring"]),
    ("Daytonarc", &["daytona"]),
    ("Lagunaseca", &["laguna seca", "weathertech raceway"]),
    ("Circuit De Barcelona", &["barcelona", "catalunya"]),
    (
        "Circuit Of The Americas",
        &["circuit of the americas", "americas", "cota", "austin"],
    ),
    (
        "Interlagos",
        &["interlagos", "jose carlos pace", "sao paulo"],
    ),
    ("Portimao", &["portimao", "algarve"]),
    ("Qatar", &["qatar", "lusail", "losail"]),
    ("Fuji", &["fuji"]),
    ("Imola", &["imola", "enzo e dino ferrari"]),
    ("Monza", &["monza"]),
    ("Bahrain", &["bahrain", "sakhir"]),
    ("Silverstone", &["silverstone"]),
];

/// The setups folder of a track among `folders` (the ones that exist), or
/// the alias table's name when none of them matches yet.
///
/// 1. alias table: a keyword of the table found in the track name;
/// 2. fuzzy: a folder whose letters and digits are contained in the track
///    name's (or the reverse), else whose words all appear in the track name.
pub fn track_folder(track_name: &str, folders: &[String]) -> Option<String> {
    let track = format!(
        " {} ",
        fold(track_name)
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    );
    let track_alnum = alnum(track_name);
    if track_alnum.is_empty() {
        return None;
    }
    let existing = |name: &str| {
        folders
            .iter()
            .find(|f| alnum(f) == alnum(name))
            .cloned()
            .unwrap_or_else(|| name.to_string())
    };
    for (folder, keywords) in TRACK_ALIASES {
        if keywords.iter().any(|k| {
            let k = fold(k);
            // Whole words for short keywords (`spa`, `cota`), substring otherwise.
            if k.len() <= 4 {
                track.contains(&format!(" {k} "))
            } else {
                track.contains(&k) || track_alnum.contains(&alnum(&k))
            }
        }) {
            return Some(existing(folder));
        }
    }
    let mut best: Option<(usize, &String)> = None;
    for f in folders {
        let fa = alnum(f);
        if fa.len() < 3 {
            continue;
        }
        let score = if track_alnum.contains(&fa) || fa.contains(&track_alnum) {
            fa.len()
        } else {
            let fw = words(f);
            if !fw.is_empty() && fw.iter().all(|w| track.contains(&format!(" {w} "))) {
                fw.iter().map(String::len).sum()
            } else {
                0
            }
        };
        if score > 0 && best.is_none_or(|(s, _)| score > s) {
            best = Some((score, f));
        }
    }
    best.map(|(_, f)| f.clone())
}

/// How [`choose_base`] picked its file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BaseReason {
    /// The latest one saved before the end of the session, in the track's folder.
    TrackFolderBeforeEnd,
    /// The latest one saved before the end of the session, in another folder.
    OtherFolderBeforeEnd,
    /// Every candidate was saved after the session: the most recent one.
    Latest,
}

/// The setup a session was most likely driven with, among `candidates`
/// (already filtered on the car): the file most recently modified before
/// `before_ms` in `folder`, else in any folder, else the most recent one.
pub fn choose_base<'a>(
    candidates: &'a [SetupFileInfo],
    folder: Option<&str>,
    before_ms: Option<i64>,
) -> Option<(&'a SetupFileInfo, BaseReason)> {
    let in_folder = |f: &SetupFileInfo| folder.is_some_and(|d| alnum(d) == alnum(&f.folder));
    let before = |f: &SetupFileInfo| before_ms.is_none_or(|t| f.mtime_ms <= t);
    let latest = |pred: &dyn Fn(&SetupFileInfo) -> bool| {
        candidates
            .iter()
            .filter(|f| pred(f))
            .max_by(|a, b| a.mtime_ms.cmp(&b.mtime_ms).then(b.path.cmp(&a.path)))
    };
    if let Some(f) = latest(&|f| in_folder(f) && before(f)) {
        return Some((f, BaseReason::TrackFolderBeforeEnd));
    }
    if let Some(f) = latest(&|f| before(f)) {
        return Some((f, BaseReason::OtherFolderBeforeEnd));
    }
    latest(&|_| true).map(|f| (f, BaseReason::Latest))
}

/// Name of the folder a car's garage captures are kept in
/// (`BMW M4 LMGT3` → `bmw-m4-lmgt3`).
pub fn car_dir_name(vehicle_model: &str) -> String {
    let s = fold(vehicle_model)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("-");
    if s.is_empty() {
        "inconnue".to_string()
    } else {
        s.chars().take(64).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cars_are_matched_on_their_distinctive_words() {
        assert!(vehicle_matches("BMW M4 LMGT3", "BMW_M4_LMGT3 GT3 WEC2026"));
        assert!(vehicle_matches(
            "Porsche 911 GT3 R LMGT3",
            "GT3 Porsche_911_GT3_R_LMGT3 WEC2025"
        ));
        assert!(vehicle_matches(
            "Ligier JS P325",
            "ELMS2025 LMP3 Ligier_JS_P325"
        ));
        assert!(vehicle_matches(
            "Mercedes-AMG LMGT3",
            "Mercedes_AMG_LMGT3 GT3 WEC2026"
        ));
        // Same brand, another car.
        assert!(!vehicle_matches(
            "BMW M4 LMGT3",
            "BMW_M_Hybrid Hypercar WEC2025"
        ));
        assert!(!vehicle_matches(
            "BMW M4 LMGT3",
            "GT3 Porsche_911_GT3_R_LMGT3 WEC2025"
        ));
        assert!(!vehicle_matches(
            "Porsche 963",
            "GT3 Porsche_911_GT3_R_LMGT3 WEC2025"
        ));
        // Only a class: no decision.
        assert!(!vehicle_matches("GT3", "BMW_M4_LMGT3 GT3 WEC2026"));
        assert!(!vehicle_matches("", "BMW_M4_LMGT3 GT3 WEC2026"));
        assert_eq!(distinctive_words("BMW M4 LMGT3"), ["bmw", "m4"]);
        assert_eq!(
            distinctive_words("Ferrari 296 LMGT3 Evo 2025"),
            ["ferrari", "296"]
        );
    }

    #[test]
    fn track_names_find_their_folder() {
        let folders: Vec<String> = ["Longbeach", "Road Atlanta", "Spa", "Lemans", "Paulricard"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let cases = [
            ("Grand Prix of Long Beach", "Longbeach"),
            ("Michelin Raceway Road Atlanta", "Road Atlanta"),
            ("Circuit de Spa-Francorchamps", "Spa"),
            ("Circuit de la Sarthe", "Lemans"),
            ("Paul Ricard - 1A", "Paulricard"),
            ("Autódromo Internacional do Algarve", "Portimao"),
            ("Lusail International Circuit", "Qatar"),
            ("Autodromo Nazionale Monza", "Monza"),
            ("Circuit of the Americas", "Circuit Of The Americas"),
            ("Sebring International Raceway", "Sebring"),
        ];
        for (track, folder) in cases {
            assert_eq!(
                track_folder(track, &folders).as_deref(),
                Some(folder),
                "{track}"
            );
        }
        // An existing folder spelled differently is returned as it exists.
        let odd = vec!["LONGBEACH".to_string()];
        assert_eq!(
            track_folder("Long Beach Street Circuit", &odd).as_deref(),
            Some("LONGBEACH")
        );
        // Fuzzy fallback on an unknown circuit.
        let custom = vec!["Nordschleife".to_string()];
        assert_eq!(
            track_folder("Nurburgring Nordschleife 24h", &custom).as_deref(),
            Some("Nordschleife")
        );
        assert_eq!(track_folder("Unknown place", &custom), None);
        assert_eq!(track_folder("", &custom), None);
        // "spa" is a whole word, not a part of "Spain".
        assert_eq!(track_folder("Spain test track", &[]), None);
    }

    fn info(path: &str, mtime_ms: i64) -> SetupFileInfo {
        let (folder, file) = path.split_once('/').unwrap();
        SetupFileInfo {
            folder: folder.into(),
            file: file.into(),
            path: path.into(),
            vehicle: "BMW_M4_LMGT3 GT3 WEC2026".into(),
            mtime_ms,
            notes: String::new(),
        }
    }

    #[test]
    fn base_is_the_latest_before_the_end_in_the_track_folder() {
        let files = vec![
            info("Longbeach/a.svm", 100),
            info("Longbeach/b.svm", 300),
            info("Longbeach/c.svm", 900), // saved after the session
            info("Road Atlanta/d.svm", 500),
        ];
        let (f, why) = choose_base(&files, Some("Longbeach"), Some(600)).unwrap();
        assert_eq!(
            (f.path.as_str(), why),
            ("Longbeach/b.svm", BaseReason::TrackFolderBeforeEnd)
        );
        let (f, why) = choose_base(&files, Some("Spa"), Some(600)).unwrap();
        assert_eq!(
            (f.path.as_str(), why),
            ("Road Atlanta/d.svm", BaseReason::OtherFolderBeforeEnd)
        );
        let (f, why) = choose_base(&files, Some("Longbeach"), Some(50)).unwrap();
        assert_eq!(
            (f.path.as_str(), why),
            ("Longbeach/c.svm", BaseReason::Latest)
        );
        let (f, _) = choose_base(&files, None, None).unwrap();
        assert_eq!(f.path, "Longbeach/c.svm");
        assert!(choose_base(&[], Some("Spa"), None).is_none());
    }

    #[test]
    fn relative_paths_cannot_leave_the_settings_folder() {
        let root = Path::new("/games/LMU/UserData/player/Settings");
        assert_eq!(
            resolve_relative(root, "Longbeach/BMW_LongBeach.svm").unwrap(),
            root.join("Longbeach").join("BMW_LongBeach.svm")
        );
        assert_eq!(
            resolve_relative(root, "Road Atlanta\\Corde_Safe_x.SVM").unwrap(),
            root.join("Road Atlanta").join("Corde_Safe_x.SVM")
        );
        for bad in [
            "../Settings.JSON",
            "../../x.svm",
            "Longbeach/../../x.svm",
            "/etc/x.svm",
            "C:/x/y.svm",
            "C:x.svm",
            "x.svm",
            "a/b/c.svm",
            "./x.svm",
            "Longbeach/",
            " Longbeach/x.svm",
            "",
        ] {
            assert!(
                matches!(
                    resolve_relative(root, bad),
                    Err(PathError::NotInSettings(_))
                ),
                "{bad:?}"
            );
        }
        assert!(matches!(
            resolve_relative(root, "Longbeach/x.txt"),
            Err(PathError::NotSvm(_))
        ));
    }

    #[test]
    fn car_folder_names_are_safe() {
        assert_eq!(car_dir_name("BMW M4 LMGT3"), "bmw-m4-lmgt3");
        assert_eq!(car_dir_name("Mercedes-AMG LMGT3"), "mercedes-amg-lmgt3");
        assert_eq!(car_dir_name("../.."), "inconnue");
        assert_eq!(
            car_dir_name("Porsche 911 GT3 R (2025)"),
            "porsche-911-gt3-r-2025"
        );
    }
}
