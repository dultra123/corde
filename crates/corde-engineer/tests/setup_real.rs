//! Plans resolved against the driver's real `.svm` files
//! (`crates/corde-lmu/tests/fixtures/svm/`) and the files Corde writes.

use corde_engineer::setup::{
    build_setup, notes_for, resolve_change, resolve_plan, unavailable_params, ResolveInput,
};
use corde_engineer::Knowledge;
use corde_lmu::svm::{diff, Setup};
use corde_protocol::{ChangeDirection, ResolveStatus, SetupChange, SetupPhilosophy, SetupPlan};
use std::path::{Path, PathBuf};

fn fixture(name: &str) -> (Vec<u8>, Setup) {
    let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../corde-lmu/tests/fixtures/svm")
        .join(name);
    (std::fs::read(&path).unwrap(), Setup::read(&path).unwrap())
}

fn change(param: &str, dir: ChangeDirection, magnitude: &str) -> SetupChange {
    SetupChange {
        parameter_id: param.into(),
        parameter_key: String::new(),
        section: String::new(),
        label_fr: param.into(),
        direction: dir,
        magnitude: magnitude.into(),
        steps: None,
        priority: 1,
        rule_ids: vec!["TEST-01".into()],
        rationale_fr: String::new(),
        rationale_en: String::new(),
        side_effects_fr: vec![],
        conflicts_with: vec![],
        confidence: 0.6,
        resolved: None,
    }
}

#[test]
fn rear_bar_minus_one_reproduces_the_drivers_v2() {
    let k = Knowledge::embedded().unwrap();
    let (v1_bytes, v1) = fixture("BMW_LongBeach.svm");
    let (_, v2) = fixture("BMW_LongBeach_v2.svm");
    let lib = [&v1, &v2];
    let input = ResolveInput {
        base: &v1,
        library: &lib,
        bounds: None,
    };
    let plan = SetupPlan {
        philosophy: SetupPhilosophy::Safe,
        intent_fr: String::new(),
        changes: vec![change("susp.arb_rear", ChangeDirection::Decrease, "1 cran")],
        warnings_fr: vec![],
        dropped: vec![],
    };
    let resolved = resolve_plan(&k, &plan, &input);
    assert_eq!(resolved.changes.len(), 1);
    let r = resolved.changes[0].resolved.as_ref().unwrap();
    assert_eq!(r.status, ResolveStatus::Applied);
    let v = &r.values[0];
    assert_eq!(
        (v.section.as_str(), v.key.as_str()),
        ("SUSPENSION", "RearAntiSwaySetting")
    );
    assert_eq!((v.index_before, v.index_after), (2, 1));
    assert_eq!(v.label_before, "P2");
    assert_eq!(v.label_after.as_deref(), Some("P1"), "label seen in v2");

    let notes = notes_for("Soft & sur", "BMW_LongBeach.svm", &resolved.changes);
    assert_eq!(
        notes,
        "Corde - Soft et sur - base : BMW_LongBeach.svm - RearAntiSway -1"
    );
    let (out, changed) = build_setup(&v1, &resolved.changes, &notes);
    assert_eq!(changed, 2);
    // Same file as the driver's own v2, notes apart…
    let d: Vec<(String, String)> = diff(&out, &v2)
        .into_iter()
        .map(|x| (x.section, x.key))
        .collect();
    assert_eq!(d, [("GENERAL".to_string(), "Notes".to_string())]);
    // …and, byte for byte, the base with two lines changed.
    let text = out.to_string();
    let before = String::from_utf8(v1_bytes).unwrap();
    assert_eq!(text.split("\r\n").count(), before.split("\r\n").count());
    let changed_lines: Vec<&str> = before
        .split("\r\n")
        .zip(text.split("\r\n"))
        .filter(|(a, b)| a != b)
        .map(|(_, b)| b)
        .collect();
    assert_eq!(
        changed_lines,
        [
            "Notes=\"Corde - Soft et sur - base : BMW_LongBeach.svm - RearAntiSway -1\"",
            "RearAntiSwaySetting=1//P1"
        ]
    );
    assert!(text.contains("Front3rdTenderSpringSetting=0//Détaché\r\n"));
}

#[test]
fn non_adjustable_parameters_are_left_out_on_the_bmw_and_kept_on_the_ligier() {
    let k = Knowledge::embedded().unwrap();
    let (_, bmw) = fixture("BMW_LongBeach.svm");
    let (_, ligier) = fixture("Ligier_LMP3_RoadAtlanta_v2.svm");
    let plan = SetupPlan {
        philosophy: SetupPhilosophy::Aggressive,
        intent_fr: String::new(),
        changes: vec![
            change("diff.power", ChangeDirection::Decrease, "1 cran"),
            change("diff.preload", ChangeDirection::Decrease, "15 Nm"),
        ],
        warnings_fr: vec![],
        dropped: vec![],
    };

    let lib = [&bmw];
    let input = ResolveInput {
        base: &bmw,
        library: &lib,
        bounds: None,
    };
    let resolved = resolve_plan(&k, &plan, &input);
    assert_eq!(resolved.changes.len(), 1);
    assert_eq!(resolved.changes[0].parameter_id, "diff.preload");
    assert_eq!(resolved.dropped.len(), 1);
    let d = resolved.dropped[0].resolved.as_ref().unwrap();
    assert_eq!(d.status, ResolveStatus::NotAdjustable);
    assert!(d.notes_fr[0].contains("Non-adjustable"), "{:?}", d.notes_fr);
    assert!(resolved
        .warnings_fr
        .iter()
        .any(|w| w.contains("diff.power écarté")));
    let un = unavailable_params(&k, &input);
    for p in ["diff.power", "diff.coast", "susp.third_front", "geo.caster"] {
        assert!(un.contains(&p.to_string()), "{p} on the BMW: {un:?}");
    }
    assert!(!un.contains(&"diff.preload".to_string()));
    assert!(
        !un.contains(&"aero.rake".to_string()),
        "rake goes through the rear ride height"
    );

    let lib = [&ligier];
    let input = ResolveInput {
        base: &ligier,
        library: &lib,
        bounds: None,
    };
    let resolved = resolve_plan(&k, &plan, &input);
    assert_eq!(resolved.changes.len(), 2);
    assert!(resolved.dropped.is_empty());
    let power = resolved.changes[0].resolved.as_ref().unwrap();
    assert_eq!(
        (power.values[0].index_before, power.values[0].index_after),
        (2, 1)
    );
    let un = unavailable_params(&k, &input);
    assert!(!un.contains(&"diff.power".to_string()));
    assert!(!un.contains(&"susp.third_front".to_string()));
}

#[test]
fn the_camber_step_is_learnt_from_two_files_of_the_same_car() {
    let k = Knowledge::embedded().unwrap();
    let (bytes, base) = fixture("BMW_LongBeach.svm");
    // A second BMW file with the camber of the driver's other BMW setup
    // (index 40 ↔ −1.00 deg in `GT3_M4_Balanced_LongBeach.svm`).
    let other = Setup::parse(
        &String::from_utf8(bytes)
            .unwrap()
            .replace("CamberSetting=22//-2.20 deg", "CamberSetting=40//-1.00 deg"),
    );
    let lib = [&base, &other];
    let input = ResolveInput {
        base: &base,
        library: &lib,
        bounds: None,
    };
    // « + » = more negative camber; 0.2 deg ≈ 3 steps of 0.0667 deg.
    let c = resolve_change(
        &k,
        &change("geo.camber_front", ChangeDirection::Increase, "0.2–0.3°"),
        &input,
    );
    let r = c.resolved.unwrap();
    assert_eq!(r.status, ResolveStatus::Applied);
    assert_eq!(r.values.len(), 2, "both front wheels");
    for v in &r.values {
        assert_eq!((v.index_before, v.index_after), (22, 19));
        assert_eq!(v.label_after.as_deref(), Some("-2.40 deg"));
    }
    assert!(
        r.notes_fr.iter().any(|n| n.contains("pas appris")),
        "{:?}",
        r.notes_fr
    );
    // Without the second file: one step, and a note.
    let lib = [&base];
    let input = ResolveInput {
        base: &base,
        library: &lib,
        bounds: None,
    };
    let r = resolve_change(
        &k,
        &change("geo.camber_front", ChangeDirection::Increase, "0.2–0.3°"),
        &input,
    )
    .resolved
    .unwrap();
    assert_eq!(r.values[0].index_after, 21);
    assert_eq!(r.values[0].label_after, None);
    assert!(
        r.notes_fr.iter().any(|n| n.contains("1 cran")),
        "{:?}",
        r.notes_fr
    );

    // The rear wing of this file carries a hand-written label: noted, not used.
    let r = resolve_change(
        &k,
        &change("aero.rear_wing", ChangeDirection::Decrease, "1 cran"),
        &input,
    )
    .resolved
    .unwrap();
    assert_eq!((r.values[0].index_before, r.values[0].index_after), (8, 7));
    assert!(r.notes_fr.iter().any(|n| n.contains("écrit à la main")));
    let (out, _) = build_setup(&base, &[c_with(r)], "Corde");
    assert!(out.to_string().contains("RWSetting=7//Corde : -1 cran\r\n"));
}

fn c_with(r: corde_protocol::ResolvedChange) -> SetupChange {
    let mut c = change("aero.rear_wing", ChangeDirection::Decrease, "1 cran");
    c.resolved = Some(r);
    c
}
