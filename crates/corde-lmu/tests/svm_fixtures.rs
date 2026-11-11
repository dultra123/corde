//! `.svm` parser / writer on real files saved by Le Mans Ultimate
//! (`tests/fixtures/svm/`, copied from the driver's `UserData\player\Settings`;
//! no personal data in them): CRLF, UTF-8 (« Détaché »), comment lines,
//! blank lines between sections, labels written by hand in the driver's
//! files (« +2 crans »).

use corde_lmu::svm::{diff, is_adjustable, LineEnding, Setup, SetupNumber};
use corde_lmu::svm_library::{scan, vehicle_matches};
use std::path::{Path, PathBuf};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/svm")
}

const FILES: [&str; 3] = [
    "BMW_LongBeach.svm",
    "BMW_LongBeach_v2.svm",
    "Ligier_LMP3_RoadAtlanta_v2.svm",
];

fn load(name: &str) -> (Vec<u8>, Setup) {
    let bytes = std::fs::read(fixtures().join(name)).unwrap();
    let setup = Setup::read(fixtures().join(name)).unwrap();
    (bytes, setup)
}

#[test]
fn real_files_round_trip_byte_for_byte() {
    for name in FILES {
        let (bytes, setup) = load(name);
        assert!(bytes.windows(2).any(|w| w == b"\r\n"), "{name}: CRLF");
        assert_eq!(setup.line_ending, LineEnding::Crlf, "{name}");
        assert!(setup.trailing_newline, "{name}");
        assert!(setup.is_plausible(), "{name}");
        assert_eq!(setup.to_string().as_bytes(), bytes.as_slice(), "{name}");
        // Every line of the file is either a header line, a section, a value
        // or a kept comment / blank: nothing is lost.
        let lines = String::from_utf8(bytes.clone()).unwrap().lines().count();
        let values = setup.values().count();
        let others: usize = setup.sections.values().map(|s| s.comments().count()).sum();
        assert_eq!(
            setup.header.lines.len() + setup.sections.len() + values + others,
            lines,
            "{name}"
        );
    }
}

#[test]
fn header_sections_and_notes() {
    let (_, bmw) = load("BMW_LongBeach.svm");
    assert_eq!(
        bmw.vehicle_class().as_deref(),
        Some("BMW_M4_LMGT3 GT3 WEC2026")
    );
    assert_eq!(bmw.header.upgrade_setting(), Some(vec![399_555, 0, 0, 0]));
    assert!(bmw.header.veh_path().unwrap().ends_with(".VEH"));
    assert_eq!(bmw.header.comments().count(), 8);
    let names: Vec<&str> = bmw.sections.keys().map(String::as_str).collect();
    assert_eq!(
        names,
        [
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
            "BASIC"
        ]
    );
    assert!(bmw.notes().unwrap().starts_with("Long Beach Sprint Trophy"));
    let (_, v2) = load("BMW_LongBeach_v2.svm");
    assert!(v2.notes().unwrap().contains("barre AR P2->P1"));

    // UTF-8 label kept as text.
    assert_eq!(
        bmw.get("SUSPENSION", "Front3rdTenderSpringSetting")
            .unwrap()
            .label(),
        "Détaché"
    );
    // The game reads the index only: hand-written labels are kept, not parsed.
    let rw = bmw.get("REARWING", "RWSetting").unwrap();
    assert_eq!(rw.index, SetupNumber::Int(8));
    assert_eq!(rw.label(), "+2 crans");
    assert_eq!(
        bmw.get("CONTROLS", "RearBrakeSetting").unwrap().label(),
        "51.5:48.5"
    );
}

#[test]
fn v1_and_v2_differ_by_notes_and_rear_bar_only() {
    let (_, v1) = load("BMW_LongBeach.svm");
    let (_, v2) = load("BMW_LongBeach_v2.svm");
    let d = diff(&v1, &v2);
    let keys: Vec<(&str, &str)> = d
        .iter()
        .map(|x| (x.section.as_str(), x.key.as_str()))
        .collect();
    assert_eq!(
        keys,
        [("GENERAL", "Notes"), ("SUSPENSION", "RearAntiSwaySetting")]
    );
    assert_eq!(d[1].a.as_ref().unwrap().as_i64(), Some(2));
    assert_eq!(d[1].b.as_ref().unwrap().as_i64(), Some(1));
    assert_eq!(d[1].b.as_ref().unwrap().label(), "P1");
}

#[test]
fn adjustable_or_not_from_the_labels() {
    let (_, bmw) = load("BMW_LongBeach.svm");
    let (_, ligier) = load("Ligier_LMP3_RoadAtlanta_v2.svm");
    let adj = |s: &Setup, sec: &str, key: &str| is_adjustable(s.get(sec, key).unwrap());
    // BMW GT3: only the preload of the differential can be changed.
    assert!(!adj(&bmw, "DRIVELINE", "DiffPowerSetting"));
    assert!(!adj(&bmw, "DRIVELINE", "DiffCoastSetting"));
    assert!(adj(&bmw, "DRIVELINE", "DiffPreloadSetting"));
    assert!(!adj(&bmw, "SUSPENSION", "Front3rdSpringSetting")); // N/A
    assert!(!adj(&bmw, "SUSPENSION", "Front3rdTenderSpringSetting")); // Détaché
    assert!(!adj(&bmw, "DRIVELINE", "Gear1Setting")); // Fixed
    assert!(!adj(&bmw, "SUSPENSION", "LeftCasterSetting")); // Non-adjustable
    assert!(adj(&bmw, "SUSPENSION", "RearAntiSwaySetting"));
    assert!(!adj(&bmw, "GENERAL", "Notes"));
    // Ligier LMP3: the three settings of the differential and the third elements.
    assert!(adj(&ligier, "DRIVELINE", "DiffPowerSetting"));
    assert!(adj(&ligier, "DRIVELINE", "DiffCoastSetting"));
    assert!(adj(&ligier, "DRIVELINE", "DiffPreloadSetting"));
    assert!(adj(&ligier, "SUSPENSION", "Front3rdSpringSetting"));
    assert!(adj(&ligier, "SUSPENSION", "Rear3rdPackerSetting"));
    assert!(!adj(&ligier, "SUSPENSION", "Front3rdTenderSpringSetting"));
}

#[test]
fn editing_one_value_keeps_everything_else() {
    let (bytes, mut s) = load("BMW_LongBeach.svm");
    s.section_mut("SUSPENSION")
        .get_mut("RearAntiSwaySetting")
        .unwrap()
        .set_index_with_display(1, Some("P1".into()));
    s.set_notes("Corde - essai");
    let out = s.to_string();
    let before = String::from_utf8(bytes).unwrap();
    let changed: Vec<(&str, &str)> = before
        .split("\r\n")
        .zip(out.split("\r\n"))
        .filter(|(a, b)| a != b)
        .collect();
    assert_eq!(
        changed,
        [
            (
                "Notes=\"Long Beach Sprint Trophy - base RA v2 + repartition 51.5, aileron AR +2 crans (motricite virages lents), fast bump AR 3->2 (bosses rue)\"",
                "Notes=\"Corde - essai\""
            ),
            ("RearAntiSwaySetting=2//P2", "RearAntiSwaySetting=1//P1"),
        ]
    );
    // Notes cannot break the line or the quoting.
    s.set_notes("a \"b\"\r\nc // d");
    assert_eq!(s.notes().as_deref(), Some("a bc // d"));
    let back = Setup::parse(&s.to_string());
    assert_eq!(back.notes().as_deref(), Some("a bc // d"));
}

#[test]
fn library_scan_reads_the_fixtures_and_skips_broken_files() {
    let dir = std::env::temp_dir().join(format!("corde-svm-lib-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let lb = dir.join("Longbeach");
    let ra = dir.join("Road Atlanta");
    std::fs::create_dir_all(&lb).unwrap();
    std::fs::create_dir_all(&ra).unwrap();
    for (name, to) in [
        ("BMW_LongBeach.svm", &lb),
        ("BMW_LongBeach_v2.svm", &lb),
        ("Ligier_LMP3_RoadAtlanta_v2.svm", &ra),
    ] {
        std::fs::copy(fixtures().join(name), to.join(name)).unwrap();
    }
    std::fs::write(lb.join("broken.svm"), "not a setup").unwrap();
    std::fs::write(lb.join("readme.txt"), "ignored").unwrap();
    std::fs::write(dir.join("top-level.svm"), "ignored").unwrap();

    let lib = scan(&dir);
    assert_eq!(lib.files.len(), 3, "{lib:?}");
    assert_eq!(lib.unreadable, ["Longbeach/broken.svm"]);
    let bmw: Vec<_> = lib
        .files
        .iter()
        .filter(|f| vehicle_matches("BMW M4 LMGT3", &f.vehicle))
        .collect();
    assert_eq!(bmw.len(), 2);
    assert!(bmw.iter().all(|f| f.folder == "Longbeach"));
    let ligier = lib
        .files
        .iter()
        .find(|f| f.folder == "Road Atlanta")
        .unwrap();
    assert_eq!(ligier.path, "Road Atlanta/Ligier_LMP3_RoadAtlanta_v2.svm");
    assert_eq!(ligier.vehicle, "ELMS2025 LMP3 Ligier_JS_P325");
    assert!(ligier.notes.starts_with("Ligier LMP3 Road Atlanta v2"));
    assert!(ligier.mtime_ms > 0);
    assert!(scan(&dir.join("missing")).files.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

/// Round trip of every `.svm` of a folder given by `CORDE_SVM_DIR` (for the
/// driver's own library): `CORDE_SVM_DIR=… cargo test -p corde-lmu --test
/// svm_fixtures -- --ignored`.
#[test]
#[ignore]
fn round_trip_of_a_whole_folder() {
    let Ok(dir) = std::env::var("CORDE_SVM_DIR") else {
        return;
    };
    let mut n = 0;
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("svm") {
            continue;
        }
        let bytes = std::fs::read(&path).unwrap();
        let setup = Setup::read(&path).unwrap();
        assert_eq!(
            setup.to_string().as_bytes(),
            bytes.as_slice(),
            "{}",
            path.display()
        );
        n += 1;
    }
    eprintln!("{n} fichiers identiques à l'octet");
}
