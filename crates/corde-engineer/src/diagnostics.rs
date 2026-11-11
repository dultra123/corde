//! Findings: the diagnostics of `knowledge/diagnostics.yaml` that the
//! engineer implements, with their thresholds and confidences. Each finding
//! carries the symptoms that `rules.rs` matches against `setup-rules.yaml`.
//!
//! Confidence = method confidence × coverage (halved under 3 clean laps) ×
//! channel availability. Thresholds are listed in `docs/ENGINEER.md`.

use crate::analysis::{
    cell_stands_out, cells, passes_of, phase_en, phase_fr, speed_en, speed_fr, Computed,
};
use corde_protocol::{
    BrakeWindow, CamberHint, CornerPhase, CornerSpeed, Evidence, Finding, Severity, VehicleClass,
};

const WHEEL_FR: [&str; 4] = ["AVG", "AVD", "ARG", "ARD"];
/// D-TYR-03: carcass further than this from the optimal temperature, °C
/// (`knowledge/diagnostics.yaml`: `T_core < T_opt − 8` / `> T_opt + 8`).
const TYRE_WINDOW_TOLERANCE_C: f64 = 8.0;
const WHEEL_EN: [&str; 4] = ["FL", "FR", "RL", "RR"];

fn ev(label: &str, value: f64, unit: &str) -> Evidence {
    Evidence {
        label: label.into(),
        value: value as f32,
        unit: unit.into(),
    }
}

struct Builder<'a> {
    c: &'a Computed,
    out: Vec<Finding>,
    labels: Vec<String>,
}

impl Builder<'_> {
    #[allow(clippy::too_many_arguments)]
    fn push(
        &mut self,
        id: impl Into<String>,
        diagnostic_id: &str,
        symptoms: &[&str],
        severity: Severity,
        confidence: f64,
        phase: Option<CornerPhase>,
        corner_speed: Option<CornerSpeed>,
        title: (String, String),
        detail: (String, String),
        evidence: Vec<Evidence>,
        lap_numbers: Vec<i32>,
    ) {
        self.out.push(Finding {
            id: id.into(),
            diagnostic_id: diagnostic_id.into(),
            symptoms: symptoms.iter().map(|s| s.to_string()).collect(),
            severity,
            confidence: (confidence * self.c.coverage_factor).clamp(0.0, 1.0) as f32,
            phase,
            corner_speed,
            title_fr: title.0,
            title_en: title.1,
            detail_fr: detail.0,
            detail_en: detail.1,
            evidence,
            lap_numbers,
        });
    }

    fn label(&mut self, l: &str) {
        if !self.labels.iter().any(|x| x == l) {
            self.labels.push(l.to_string());
        }
    }
}

/// Findings and French style labels.
pub fn findings(c: &Computed, class: VehicleClass) -> (Vec<Finding>, Vec<String>) {
    let mut b = Builder {
        c,
        out: Vec::new(),
        labels: Vec::new(),
    };
    conditions(&mut b);
    balance(&mut b);
    braking(&mut b, class);
    traction(&mut b);
    tyres(&mut b);
    platform(&mut b);
    dampers(&mut b);
    style(&mut b);
    consistency(&mut b);
    // Major first, then by confidence.
    b.out.sort_by(|a, x| {
        x.severity.cmp(&a.severity).then(
            x.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal),
        )
    });
    (b.out, b.labels)
}

fn conditions(b: &mut Builder) {
    let c = b.c;
    if c.wet_session {
        b.push(
            "D-WET-01/wet",
            "D-WET-01",
            &["WET_BASELINE"],
            Severity::Major,
            0.9,
            None,
            None,
            ("Session sous la pluie / piste humide".into(), "Wet session".into()),
            (
                "Pluie ou piste humide pendant les tours analysés : le paquet pluie s'applique, les constats d'équilibre au sec ne sont pas transposables.".into(),
                "Rain or a wet track during the analysed laps: the wet package applies, dry balance findings do not carry over.".into(),
            ),
            vec![],
            vec![],
        );
        let carcass: Vec<f64> = c
            .tyres
            .per_wheel
            .iter()
            .map(|w| w.avg_carcass_c as f64)
            .filter(|v| *v > 0.0)
            .collect();
        if let Some(m) = crate::metrics::mean(carcass.iter().copied()) {
            if m < 55.0 {
                b.push(
                    "D-WET-01/cold",
                    "D-WET-01",
                    &["WET_NO_TEMP"],
                    Severity::Minor,
                    0.5,
                    None,
                    None,
                    ("Pneus pluie froids".into(), "Wet tyres too cold".into()),
                    (
                        format!("Carcasse moyenne {m:.0} °C sous la pluie : les pneus pluie travaillent vers 60–80 °C."),
                        format!("Mean carcass {m:.0} °C in the wet: wet tyres work around 60–80 °C."),
                    ),
                    vec![ev("carcasse moyenne", m, "°C")],
                    vec![],
                );
            }
        }
    }
}

fn balance(b: &mut Builder) {
    let c = b.c;
    // Every index is relative to the session reference (what this car usually
    // does in this session): only a phase / speed range that stands out
    // becomes a finding.
    let Some(reference) = c.balance.reference.as_ref() else {
        return;
    };
    for (phase, speed, cell) in cells(&c.balance.per_phase) {
        let cell_passes = passes_of(&c.balance_cell_passes, phase, speed);
        if cell.confidence < 0.15 || !cell_stands_out(cell, cell_passes) {
            continue;
        }
        let idx = cell.understeer_index as f64;
        let sigmas = idx * crate::metrics::BALANCE_SIGMAS_FULL_SCALE;
        let under = idx > 0.0;
        let speed_tag = match speed {
            CornerSpeed::Fast => "FAST",
            _ => "SLOW", // the rules only know slow / fast; medium corners count as mechanical grip
        };
        let phase_tag = match phase {
            CornerPhase::Entry => "ENTRY",
            CornerPhase::Mid => "MID",
            CornerPhase::Exit => "EXIT",
        };
        let symptom = format!(
            "{}_{}_{}",
            if under { "US" } else { "OS" },
            phase_tag,
            speed_tag
        );
        let severity = if idx.abs() > crate::metrics::BALANCE_MAJOR_INDEX {
            Severity::Major
        } else {
            Severity::Minor
        };
        // Barely above the trigger → less confident than a saturated index.
        let strength = (0.4 + idx.abs()).min(1.0);
        let passes = c
            .balance
            .corners
            .iter()
            .filter(|k| k.speed_class == Some(speed))
            .map(|k| k.passes)
            .sum::<u32>();
        b.push(
            format!("D-BAL-01/{}/{}", phase_en(phase), speed_en(speed)),
            "D-BAL-01",
            &[symptom.as_str()],
            severity,
            cell.confidence as f64 * strength,
            Some(phase),
            Some(speed),
            (
                format!(
                    "{} en {} de virages {} (par rapport à ta référence)",
                    if under { "Plus de sous-virage" } else { "Plus de survirage" },
                    phase_fr(phase),
                    speed_fr(speed)
                ),
                format!(
                    "More {} on {} of {} corners (vs your session reference)",
                    if under { "understeer" } else { "oversteer" },
                    phase_en(phase),
                    speed_en(speed)
                ),
            ),
            (
                format!(
                    "Par rapport à ta référence de la session (le comportement habituel de cette voiture sur ces tours), la voiture est nettement plus {} ici : écart {sigmas:+.1} σ (indice relatif {idx:+.2}, seuil ±{:.2}), sur {} échantillons et {cell_passes} passage(s) ({passes} passage(s) de virages {} au total). Indice absolu {:+.2} pour une référence de {:+.2} : l'absolu ne se compare pas d'une voiture à l'autre. Estimé par les angles de dérive et/ou le gradient volant − lacet.",
                    if under { "sous-vireuse" } else { "survireuse" },
                    reference.trigger_index,
                    cell.samples,
                    speed_fr(speed),
                    cell.raw_index,
                    reference.raw_index,
                ),
                format!(
                    "Compared with your session reference (how this car usually behaves on these laps), the car is clearly more {} here: {sigmas:+.1} σ (relative index {idx:+.2}, trigger ±{:.2}), over {} samples and {cell_passes} pass(es) ({passes} {} corner pass(es) in total). Absolute index {:+.2} against a reference of {:+.2}: absolute values do not compare across cars. From slip angles and/or the steering − yaw gradient.",
                    if under { "understeery" } else { "oversteery" },
                    reference.trigger_index,
                    cell.samples,
                    speed_en(speed),
                    cell.raw_index,
                    reference.raw_index,
                ),
            ),
            vec![
                ev("écart à la référence", sigmas, "σ"),
                ev("indice relatif", idx, ""),
                ev("indice absolu", cell.raw_index as f64, ""),
                ev("référence absolue", reference.raw_index as f64, ""),
                ev("échantillons", cell.samples as f64, ""),
                ev("passages", cell_passes as f64, ""),
            ],
            vec![],
        );
    }
}

fn braking(b: &mut Builder, class: VehicleClass) {
    let c = b.c;
    let n = c.n_laps.max(1) as f64;
    let front = (c.lockups_per_wheel[0] + c.lockups_per_wheel[1]) as f64;
    let rear = (c.lockups_per_wheel[2] + c.lockups_per_wheel[3]) as f64;
    let total = front + rear;
    if c.data_quality.has_slip && total / n >= 0.5 {
        let share = front / total;
        let per_lap = total / n;
        let severity = if per_lap >= 2.0 {
            Severity::Major
        } else {
            Severity::Minor
        };
        let evidence = vec![
            ev("blocages par tour", per_lap, "/tour"),
            ev("part avant", share, ""),
            ev("AVG", c.lockups_per_wheel[0] as f64, ""),
            ev("AVD", c.lockups_per_wheel[1] as f64, ""),
            ev("ARG", c.lockups_per_wheel[2] as f64, ""),
            ev("ARD", c.lockups_per_wheel[3] as f64, ""),
        ];
        let (symptoms, title_fr, title_en, conf): (Vec<&str>, &str, &str, f64) = if share >= 0.7 {
            (
                vec!["BRK_LOCK_FRONT"],
                "Blocages des roues avant au freinage",
                "Front wheels locking under braking",
                0.8,
            )
        } else if share <= 0.3 {
            (
                vec!["BRK_LOCK_REAR"],
                "Blocages des roues arrière au freinage",
                "Rear wheels locking under braking",
                0.8,
            )
        } else {
            (
                vec!["BRK_LOCK_FRONT", "BRK_LOCK_REAR"],
                "Blocages sur les deux essieux",
                "Lock-ups on both axles",
                0.5,
            )
        };
        b.push(
            "D-BRK-03/axle",
            "D-BRK-03",
            &symptoms,
            severity,
            conf,
            None,
            None,
            (title_fr.into(), title_en.into()),
            (
                format!("{per_lap:.1} blocage(s) par tour (slip < −0,15 pendant ≥ 60 ms, frein > 30 %), {:.0} % à l'avant.", share * 100.0),
                format!("{per_lap:.1} lock-up(s) per lap (slip < −0.15 for ≥ 60 ms, brake > 30 %), {:.0} % on the front axle.", share * 100.0),
            ),
            evidence,
            c.lockup_laps.clone(),
        );
        if front >= 2.0 && c.lockups_inner_front as f64 >= 0.7 * front {
            b.push(
                "D-BRK-03/inner-front",
                "D-BRK-03",
                &["BRK_LOCK_INNER_FRONT"],
                Severity::Minor,
                0.6,
                Some(CornerPhase::Entry),
                None,
                ("Blocage de la roue avant intérieure en entrée".into(), "Inside front locking on entry".into()),
                (
                    format!("{} des {} blocages avant surviennent sur la roue intérieure en appui (trail braking appuyé, détente avant trop ferme ou trop de carrossage).", c.lockups_inner_front, front),
                    format!("{} of the {} front lock-ups happen on the inside wheel under lateral load (heavy trail braking, stiff front rebound or too much camber).", c.lockups_inner_front, front),
                ),
                vec![ev("blocages intérieurs", c.lockups_inner_front as f64, ""), ev("blocages avant", front, "")],
                c.lockup_laps.clone(),
            );
        }
    }

    // Thermal window per axle.
    let (cold, hot) = crate::metrics::brake_window_c(class);
    for (axle, wheels, name_fr, name_en) in [
        (0usize, [0usize, 1usize], "avant", "front"),
        (1, [2, 3], "arrière", "rear"),
    ] {
        let ws: Vec<_> = wheels.iter().map(|&w| &c.brakes.per_wheel[w]).collect();
        let max = ws.iter().map(|w| w.max_c as f64).fold(0.0, f64::max);
        let typical = ws
            .iter()
            .map(|w| w.typical_peak_c as f64)
            .fold(0.0, f64::max);
        let avg = crate::metrics::mean(ws.iter().map(|w| w.avg_c as f64)).unwrap_or(0.0);
        let state = if ws.iter().all(|w| w.window == BrakeWindow::Hot) {
            Some(("BRK_OVERHEAT", "Freins {} trop chauds", "{} brakes too hot"))
        } else if ws.iter().all(|w| w.window == BrakeWindow::Cold) {
            Some(("BRK_COLD", "Freins {} froids", "{} brakes cold"))
        } else {
            None
        };
        if let Some((symptom, fr, en)) = state {
            b.push(
                format!("D-BRK-01/{}", ["front", "rear"][axle]),
                "D-BRK-01",
                &[symptom],
                Severity::Minor,
                0.4,
                None,
                None,
                (fr.replace("{}", name_fr), en.replace("{}", &capitalize(name_en))),
                (
                    format!("Pic typique par tour {typical:.0} °C (médiane des maxima de chaque tour ; maximum absolu {max:.0} °C, moyenne {avg:.0} °C) ; fenêtre indicative {cold:.0}–{hot:.0} °C pour cette classe (base de connaissances, vérifiée sur cinq sessions GT3 réelles ; à confirmer sur la couleur du HUD du jeu)."),
                    format!("Typical peak per lap {typical:.0} °C (median of each lap's maximum; absolute maximum {max:.0} °C, mean {avg:.0} °C); indicative window {cold:.0}–{hot:.0} °C for this class (knowledge base, checked on five real GT3 sessions; confirm on the game's HUD colours)."),
                ),
                vec![
                    ev("pic typique", typical, "°C"),
                    ev("pic", max, "°C"),
                    ev("moyenne", avg, "°C"),
                ],
                vec![],
            );
        }
    }

    if let Some(a) = c.abs_activity {
        if a > 0.30 {
            b.push(
                "D-BRK-04/abs",
                "D-BRK-04",
                &["ABS_TOO_ACTIVE"],
                Severity::Minor,
                0.5,
                None,
                None,
                ("ABS très sollicité".into(), "ABS very active".into()),
                (
                    format!(
                        "ABS actif pendant {:.0} % du temps de freinage (> 30 %).",
                        a * 100.0
                    ),
                    format!(
                        "ABS active for {:.0} % of the braking time (> 30 %).",
                        a * 100.0
                    ),
                ),
                vec![ev("activité ABS", a * 100.0, "%")],
                vec![],
            );
        }
    }

    let corr = c.braking_corrections_per_lap;
    if corr > 1.0 {
        b.push(
            "D-BRK-05/straight",
            "D-BRK-05",
            &["BRK_UNSTABLE_STRAIGHT"],
            if corr > 3.0 { Severity::Major } else { Severity::Minor },
            0.4,
            None,
            None,
            ("Corrections de volant en freinage droit".into(), "Steering corrections while braking straight".into()),
            (
                format!("{corr:.1} correction(s) de plus de 10° par tour pendant les freinages en ligne droite : arrière instable."),
                format!("{corr:.1} correction(s) above 10° per lap while braking in a straight line: unstable rear."),
            ),
            vec![ev("corrections par tour", corr, "/tour")],
            vec![],
        );
    }
}

fn traction(b: &mut Builder) {
    let c = b.c;
    let n = c.n_laps.max(1) as f64;
    let rl = c.spins_per_wheel[2] as f64;
    let rr = c.spins_per_wheel[3] as f64;
    let rear = rl + rr;
    if c.data_quality.has_slip && rear / n >= 1.0 {
        let per_lap = rear / n;
        let balanced = rl.min(rr) >= 0.5 * rl.max(rr);
        let severity = if per_lap >= 3.0 {
            Severity::Major
        } else {
            Severity::Minor
        };
        let (id, diag, symptom, fr, en) = if balanced {
            (
                "D-TRC-01/both",
                "D-TRC-01",
                "TRC_WHEELSPIN_BOTH",
                "Patinage des deux roues arrière en sortie",
                "Both rear wheels spinning on exit",
            )
        } else {
            (
                "D-TRC-02/inner",
                "D-TRC-02",
                "TRC_WHEELSPIN_INNER",
                "Patinage d'une seule roue arrière",
                "Single rear wheel spinning",
            )
        };
        b.push(
            id,
            diag,
            &[symptom],
            severity,
            0.7,
            Some(CornerPhase::Exit),
            None,
            (fr.into(), en.into()),
            (
                format!("{per_lap:.1} patinage(s) par tour (slip > +0,15 pendant ≥ 60 ms, gaz > 50 %) : ARG {rl:.0}, ARD {rr:.0}."),
                format!("{per_lap:.1} wheel-spin event(s) per lap (slip > +0.15 for ≥ 60 ms, throttle > 50 %): RL {rl:.0}, RR {rr:.0}."),
            ),
            vec![ev("patinages par tour", per_lap, "/tour"), ev("ARG", rl, ""), ev("ARD", rr, "")],
            c.spin_laps.clone(),
        );
    }
    if let Some(tc) = c.tc_exit_activity {
        if tc > 0.25 {
            b.push(
                "D-TRC-03/tc",
                "D-TRC-03",
                &["TC_TOO_INTRUSIVE"],
                Severity::Minor,
                0.5,
                Some(CornerPhase::Exit),
                None,
                ("Contrôle de traction très sollicité en sortie".into(), "Traction control very active on exit".into()),
                (
                    format!("TC actif pendant {:.0} % des phases de sortie (> 25 %) : TC trop intrusif ou vraie perte de motricité.", tc * 100.0),
                    format!("TC active for {:.0} % of the exit phases (> 25 %): intrusive TC or genuine traction loss.", tc * 100.0),
                ),
                vec![ev("activité TC en sortie", tc * 100.0, "%")],
                vec![],
            );
        }
    }
}

fn capitalize(s: &str) -> String {
    let mut ch = s.chars();
    match ch.next() {
        Some(f) => f.to_uppercase().collect::<String>() + ch.as_str(),
        None => String::new(),
    }
}

fn tyres(b: &mut Builder) {
    let c = b.c;
    let w = &c.tyres.per_wheel;
    for (axle, wheels, fr, en) in [
        (0usize, [0usize, 1usize], "avant", "front"),
        (1, [2, 3], "arrière", "rear"),
    ] {
        let hints: Vec<CamberHint> = wheels.iter().map(|&i| w[i].camber_hint).collect();
        let spreads: Vec<Evidence> = wheels
            .iter()
            .map(|&i| {
                ev(
                    &format!("I−O {}", WHEEL_FR[i]),
                    w[i].imo_spread_c as f64,
                    "°C",
                )
            })
            .collect();
        if hints.contains(&CamberHint::TooMuch) && !hints.contains(&CamberHint::TooLittle) {
            b.push(
                format!("D-TYR-01/{}", ["front", "rear"][axle]),
                "D-TYR-01",
                &["TYR_HOT_INNER"],
                Severity::Minor,
                0.5,
                None,
                None,
                (format!("Intérieur des pneus {fr} trop chaud (carrossage)"), format!("Inner shoulder of the {en} tyres too hot (camber)")),
                (
                    format!("Écart intérieur − extérieur > 12 °C en appui sur l'essieu {fr} : trop de carrossage négatif (orientation des zones à confirmer sur capture réelle)."),
                    format!("Inner − outer spread > 12 °C under load on the {en} axle: too much negative camber (zone orientation to confirm on real data)."),
                ),
                spreads,
                vec![],
            );
        } else if hints.contains(&CamberHint::TooLittle) && !hints.contains(&CamberHint::TooMuch) {
            b.push(
                format!("D-TYR-01/{}", ["front", "rear"][axle]),
                "D-TYR-01",
                &["TYR_HOT_OUTER"],
                Severity::Minor,
                0.4,
                None,
                None,
                (format!("Extérieur des pneus {fr} plus chaud que l'intérieur"), format!("Outer shoulder of the {en} tyres hotter than the inner")),
                (
                    format!("Extérieur plus chaud que l'intérieur en appui sur l'essieu {fr} : pas assez de carrossage pour le roulis présent (seuil à calibrer)."),
                    format!("Outer hotter than inner under load on the {en} axle: not enough camber for the roll (threshold to calibrate)."),
                ),
                spreads,
                vec![],
            );
        }
        // Middle vs shoulders → pressure.
        let dm = crate::metrics::mean(wheels.iter().map(|&i| {
            let t = &w[i];
            if t.inner_c > 0.0 && t.outer_c > 0.0 {
                (t.middle_c - (t.inner_c + t.outer_c) / 2.0) as f64
            } else {
                f64::NAN
            }
        }));
        if let Some(dm) = dm {
            if dm.abs() > 3.0 {
                let (symptom, tfr, ten) = if dm > 0.0 {
                    (
                        "TYR_HOT_MIDDLE",
                        "centre plus chaud que les bords (surgonflage)",
                        "centre hotter than the shoulders (over-inflated)",
                    )
                } else {
                    (
                        "TYR_COLD_MIDDLE",
                        "bords plus chauds que le centre (sous-gonflage)",
                        "shoulders hotter than the centre (under-inflated)",
                    )
                };
                b.push(
                    format!("D-TYR-02/{}", ["front", "rear"][axle]),
                    "D-TYR-02",
                    &[symptom],
                    Severity::Minor,
                    0.4,
                    None,
                    None,
                    (format!("Pneus {fr} : {tfr}"), format!("{} tyres: {ten}", capitalize(en))),
                    (
                        format!("ΔM = centre − moyenne des bords = {dm:+.1} °C (seuil ±3 °C ; effet faible dans le modèle rF2, pondérer avec la pression à chaud)."),
                        format!("ΔM = centre − mean of the shoulders = {dm:+.1} °C (threshold ±3 °C; weak effect in the rF2 model, weigh with the hot pressure)."),
                    ),
                    vec![ev("ΔM", dm, "°C")],
                    vec![],
                );
            }
        }
    }

    // Carcass level against the compound's optimal temperature (D-TYR-03),
    // only when the game gave it (recorded since S2).
    if c.data_quality.has_optimal_temp {
        for (axle, wheels, fr, en) in [
            (0usize, [0usize, 1usize], "avant", "front"),
            (1, [2, 3], "arrière", "rear"),
        ] {
            let deltas: Vec<f64> = wheels
                .iter()
                .filter_map(|&i| w[i].delta_to_optimal_c.map(|d| d as f64))
                .collect();
            if deltas.len() < 2 {
                continue;
            }
            let d = deltas.iter().sum::<f64>() / deltas.len() as f64;
            let opt = crate::metrics::mean(
                wheels
                    .iter()
                    .filter_map(|&i| w[i].optimal_c.map(|v| v as f64)),
            )
            .unwrap_or(0.0);
            let state = if d < -TYRE_WINDOW_TOLERANCE_C {
                Some(("TYR_COLD", "froids", "cold", "sous"))
            } else if d > TYRE_WINDOW_TOLERANCE_C {
                Some((
                    if axle == 0 {
                        "TYR_HOT_FRONT"
                    } else {
                        "TYR_HOT_REAR"
                    },
                    "trop chauds",
                    "too hot",
                    "au-dessus de",
                ))
            } else {
                None
            };
            if let Some((symptom, word_fr, word_en, rel_fr)) = state {
                b.push(
                    format!("D-TYR-03/{}", ["front", "rear"][axle]),
                    "D-TYR-03",
                    &[symptom],
                    Severity::Minor,
                    0.6,
                    None,
                    None,
                    (
                        format!("Pneus {fr} {word_fr} (carcasse vs température optimale)"),
                        format!("{} tyres {word_en} (carcass vs optimal temperature)", capitalize(en)),
                    ),
                    (
                        format!("Carcasse moyenne {d:+.1} °C {rel_fr} la température optimale du composé donnée par le jeu ({opt:.0} °C) ; tolérance ±{TYRE_WINDOW_TOLERANCE_C:.0} °C."),
                        format!("Mean carcass {d:+.1} °C from the compound's optimal temperature given by the game ({opt:.0} °C); tolerance ±{TYRE_WINDOW_TOLERANCE_C:.0} °C."),
                    ),
                    vec![ev("écart à l'optimum", d, "°C"), ev("optimum", opt, "°C")],
                    vec![],
                );
            }
        }
    }

    // Front / rear carcass gradient.
    let front = crate::metrics::mean(
        [w[0].avg_carcass_c as f64, w[1].avg_carcass_c as f64]
            .into_iter()
            .filter(|v| *v > 0.0),
    );
    let rear = crate::metrics::mean(
        [w[2].avg_carcass_c as f64, w[3].avg_carcass_c as f64]
            .into_iter()
            .filter(|v| *v > 0.0),
    );
    if let (Some(f), Some(r)) = (front, rear) {
        let d = f - r;
        if d.abs() > 8.0 {
            let (symptom, tfr, ten) = if d > 0.0 {
                (
                    "TYR_HOT_FRONT",
                    "Pneus avant nettement plus chauds que l'arrière",
                    "Front tyres much hotter than the rear",
                )
            } else {
                (
                    "TYR_HOT_REAR",
                    "Pneus arrière nettement plus chauds que l'avant",
                    "Rear tyres much hotter than the front",
                )
            };
            b.push(
                "D-TYR-05/gradient",
                "D-TYR-05",
                &[symptom],
                Severity::Minor,
                0.5,
                None,
                None,
                (tfr.into(), ten.into()),
                (
                    format!("Carcasse avant {f:.0} °C, arrière {r:.0} °C (écart {d:+.0} °C, seuil ±8 °C) : l'essieu chaud travaille trop (glissement)."),
                    format!("Front carcass {f:.0} °C, rear {r:.0} °C (gap {d:+.0} °C, threshold ±8 °C): the hot axle is overworked (sliding)."),
                ),
                vec![ev("carcasse AV", f, "°C"), ev("carcasse AR", r, "°C")],
                vec![],
            );
        }
    }

    // Degradation.
    if let Some(slope) = c.lap_stats.degradation_s_per_lap {
        let slope = slope as f64;
        if slope > 0.08 {
            let wear: Vec<f64> = w.iter().map(|t| t.wear_per_lap_pct as f64).collect();
            let mut evidence = vec![ev("pente", slope, "s/tour")];
            for (i, v) in wear.iter().enumerate() {
                evidence.push(ev(&format!("usure {}", WHEEL_FR[i]), *v, "%/tour"));
            }
            let limiting = (0..4).find(|&i| {
                let others = crate::metrics::mean((0..4).filter(|&j| j != i).map(|j| wear[j]))
                    .unwrap_or(0.0);
                wear[i] > 0.1 && wear[i] > 1.3 * others
            });
            b.push(
                "D-TYR-06/degradation",
                "D-TYR-06",
                &["TYR_WEAR_FAST"],
                if slope > 0.25 { Severity::Major } else { Severity::Minor },
                0.5,
                None,
                None,
                ("Dégradation rapide des temps au tour".into(), "Fast lap-time degradation".into()),
                (
                    format!(
                        "Temps au tour qui augmentent de {slope:.2} s par tour (seuil 0,08 s/tour, non corrigé du carburant){}.",
                        limiting.map(|i| format!(" ; pneu limitant : {} ({:.2} %/tour)", WHEEL_FR[i], wear[i])).unwrap_or_default()
                    ),
                    format!(
                        "Lap times rising by {slope:.2} s per lap (threshold 0.08 s/lap, not fuel-corrected){}.",
                        limiting.map(|i| format!("; limiting tyre: {} ({:.2} %/lap)", WHEEL_EN[i], wear[i])).unwrap_or_default()
                    ),
                ),
                evidence,
                vec![],
            );
        }
    }

    // Pressure drift: information only.
    let drift = w
        .iter()
        .map(|t| t.pressure_drift_kpa as f64)
        .fold(0.0, |a: f64, v| if v.abs() > a.abs() { v } else { a });
    if drift.abs() > 8.0 {
        b.push(
            "D-TYR-04/drift",
            "D-TYR-04",
            &[],
            Severity::Info,
            0.6,
            None,
            None,
            ("Dérive de pression sur le relais".into(), "Pressure drift over the stint".into()),
            (
                format!("Pression qui dérive de {drift:+.0} kPa entre le premier et le dernier tour analysé : les pressions à froid ne visent pas encore la pression à chaud cible."),
                format!("Pressure drifting by {drift:+.0} kPa between the first and the last analysed lap: cold pressures do not yet target the hot pressure."),
            ),
            w.iter().enumerate().map(|(i, t)| ev(&format!("dérive {}", WHEEL_FR[i]), t.pressure_drift_kpa as f64, "kPa")).collect(),
            vec![],
        );
    }
}

fn platform(b: &mut Builder) {
    let c = b.c;
    let per_lap = c.suspension.bottoming_events_per_lap as f64;
    if per_lap > 0.5 {
        let laps: Vec<i32> = c
            .bottoming_per_lap
            .iter()
            .filter(|x| x.1 > 0)
            .map(|x| x.0)
            .collect();
        b.push(
            "D-PLT-01/bottoming",
            "D-PLT-01",
            &["PLT_BOTTOMING"],
            if per_lap > 2.0 { Severity::Major } else { Severity::Minor },
            if c.data_quality.has_ride_height { 0.6 } else { 0.4 },
            None,
            None,
            ("Talonnage".into(), "Bottoming".into()),
            (
                format!(
                    "{per_lap:.1} talonnage(s) par tour (hauteur < 5 mm ou suspension à ≥ 98 % de sa course pendant ≥ 40 ms) ; hauteur mini AV {:.0} mm, AR {:.0} mm.",
                    c.suspension.min_ride_height_mm[0], c.suspension.min_ride_height_mm[1]
                ),
                format!(
                    "{per_lap:.1} bottoming event(s) per lap (ride height < 5 mm or suspension at ≥ 98 % of its travel for ≥ 40 ms); minimum ride height front {:.0} mm, rear {:.0} mm.",
                    c.suspension.min_ride_height_mm[0], c.suspension.min_ride_height_mm[1]
                ),
            ),
            vec![
                ev("talonnages par tour", per_lap, "/tour"),
                ev("hauteur mini AV", c.suspension.min_ride_height_mm[0] as f64, "mm"),
                ev("hauteur mini AR", c.suspension.min_ride_height_mm[1] as f64, "mm"),
            ],
            laps,
        );
    }
    let sat = c.suspension.travel_saturation_pct;
    if let Some((i, v)) = sat.iter().enumerate().map(|(i, v)| (i, *v as f64)).fold(
        None,
        |a: Option<(usize, f64)>, x| match a {
            Some(b) if b.1 >= x.1 => Some(b),
            _ => Some(x),
        },
    ) {
        if v > 10.0 {
            b.push(
                "D-PLT-03/travel",
                "D-PLT-03",
                &["PLT_BOTTOMING"],
                Severity::Minor,
                0.4,
                None,
                None,
                ("Suspension en butée une grande partie du tour".into(), "Suspension on the bump stops much of the lap".into()),
                (
                    format!("Roue {} à ≥ 98 % de sa course observée pendant {v:.0} % du temps (> 10 %) : packer trop court ou ressort trop mou.", WHEEL_FR[i]),
                    format!("Wheel {} at ≥ 98 % of its observed travel for {v:.0} % of the time (> 10 %): packer too short or spring too soft.", WHEEL_EN[i]),
                ),
                sat.iter().enumerate().map(|(k, s)| ev(&format!("saturation {}", WHEEL_FR[k]), *s as f64, "%")).collect(),
                vec![],
            );
        }
    }
}

fn dampers(b: &mut Builder) {
    let c = b.c;
    let h = &c.suspension.damper_velocity_hist;
    let total: f32 = h
        .iter()
        .map(|x| x.slow_bump_pct + x.slow_rebound_pct + x.fast_bump_pct + x.fast_rebound_pct)
        .sum();
    if total <= 0.0 {
        return;
    }
    for (i, x) in h.iter().enumerate() {
        if x.skew_slow.abs() > 5.0 {
            b.push(
                format!("D-DMP-01/skew-{}", WHEEL_EN[i].to_lowercase()),
                "D-DMP-01",
                &[],
                Severity::Info,
                0.5,
                None,
                None,
                (format!("Histogramme amortisseur {} asymétrique", WHEEL_FR[i]), format!("Asymmetric damper histogram {}", WHEEL_EN[i])),
                (
                    format!(
                        "Zone lente : {:.0} % compression contre {:.0} % détente (écart {:+.0} points, cible < 5) : {}.",
                        x.slow_bump_pct, x.slow_rebound_pct, x.skew_slow,
                        if x.skew_slow > 0.0 { "plus de détente lente ou moins de compression lente" } else { "plus de compression lente ou moins de détente lente" }
                    ),
                    format!(
                        "Slow band: {:.0} % bump vs {:.0} % rebound (skew {:+.0} points, target < 5): {}.",
                        x.slow_bump_pct, x.slow_rebound_pct, x.skew_slow,
                        if x.skew_slow > 0.0 { "more slow rebound or less slow bump" } else { "more slow bump or less slow rebound" }
                    ),
                ),
                vec![ev("skew", x.skew_slow as f64, "pts")],
                vec![],
            );
        }
    }
    let pct_slow = crate::metrics::mean(
        h.iter()
            .map(|x| (x.slow_bump_pct + x.slow_rebound_pct) as f64),
    )
    .unwrap_or(0.0);
    if pct_slow < 35.0 {
        b.push(
            "D-DMP-01/floaty",
            "D-DMP-01",
            &["PLT_FLOATY"],
            Severity::Info, // informational until the thresholds are calibrated per car
            0.3,
            None,
            None,
            ("Amortissement faible (voiture qui flotte)".into(), "Light damping (floaty car)".into()),
            (
                format!("{pct_slow:.0} % du temps seulement en zone lente (< 25 mm/s) : distribution large, voiture sous-amortie (seuil 35 %, à calibrer par voiture)."),
                format!("Only {pct_slow:.0} % of the time in the slow band (< 25 mm/s): wide distribution, under-damped car (threshold 35 %, to calibrate per car)."),
            ),
            vec![ev("zone lente", pct_slow, "%")],
            vec![],
        );
    } else if pct_slow > 92.0 {
        b.push(
            "D-DMP-01/nervous",
            "D-DMP-01",
            &["PLT_NERVOUS"],
            Severity::Info, // informational until the thresholds are calibrated per car
            0.3,
            None,
            None,
            ("Amortissement très fort (voiture nerveuse)".into(), "Heavy damping (nervous car)".into()),
            (
                format!("{pct_slow:.0} % du temps en zone lente : pointe étroite, voiture très amortie (seuil 92 %, à calibrer par voiture)."),
                format!("{pct_slow:.0} % of the time in the slow band: narrow peak, heavily damped car (threshold 92 %, to calibrate per car)."),
            ),
            vec![ev("zone lente", pct_slow, "%")],
            vec![],
        );
    }
}

fn style(b: &mut Builder) {
    let s = b.c.style.clone();
    if s.brake_aggression > 75.0 {
        b.label("freinage agressif");
        b.push(
            "S-01/hard",
            "S-01",
            &[],
            Severity::Info,
            0.6,
            None,
            None,
            ("Freinage agressif".into(), "Aggressive braking".into()),
            (
                format!("Indice d'attaque au freinage {:.0}/100 : voiture sensible aux blocages, préférer une migration plus faible / une répartition plus avant.", s.brake_aggression),
                format!("Brake attack index {:.0}/100: the car is lock-up prone, prefer less migration / more forward bias.", s.brake_aggression),
            ),
            vec![ev("agressivité", s.brake_aggression as f64, "/100")],
            vec![],
        );
    } else if s.brake_aggression > 0.0 && s.brake_aggression < 35.0 {
        b.label("freinage progressif");
    }
    if s.trail_braking > 50.0 {
        b.label("trail braking marqué");
        b.push(
            "S-02/high",
            "S-02",
            &[],
            Severity::Info,
            0.6,
            Some(CornerPhase::Entry),
            None,
            ("Trail braking marqué".into(), "Pronounced trail braking".into()),
            (
                format!("{:.0} % des entrées se font frein et appui latéral ensemble : le pilote tourne la voiture au frein et tolère un peu de survirage d'entrée.", s.trail_braking),
                format!("{:.0} % of the entries mix brake and lateral load: the driver rotates the car on the brakes and accepts some entry oversteer.", s.trail_braking),
            ),
            vec![ev("trail braking", s.trail_braking as f64, "%")],
            vec![],
        );
    } else if s.trail_braking < 20.0 {
        b.label("peu de trail braking");
    }
    if b.c.has_steering && s.steering_smoothness < 40.0 {
        b.label("volant nerveux");
        b.push(
            "S-03/nervous",
            "S-03",
            &["PLT_NERVOUS"],
            Severity::Minor,
            0.4,
            None,
            None,
            ("Volant nerveux".into(), "Busy steering".into()),
            (
                format!("Lissage du volant {:.0}/100 : voiture trop pointue ou pilote qui cherche le grip ; orientation « sûre » conseillée.", s.steering_smoothness),
                format!("Steering smoothness {:.0}/100: car too pointy or a driver hunting for grip; the safe orientation is advised.", s.steering_smoothness),
            ),
            vec![ev("lissage volant", s.steering_smoothness as f64, "/100")],
            vec![],
        );
    } else if b.c.has_steering && s.steering_smoothness > 75.0 {
        b.label("volant lisse");
    }
    if s.kerb_usage > 8.0 {
        b.label("gros usage des vibreurs");
        b.push(
            "S-04/kerbs",
            "S-04",
            &["PLT_KERB_UNSETTLED"],
            Severity::Minor,
            0.4,
            None,
            None,
            ("Gros usage des vibreurs".into(), "Heavy kerb usage".into()),
            (
                format!("{:.1} % du tour sur les vibreurs : amortissement rapide plus souple et packers plus longs conseillés.", s.kerb_usage),
                format!("{:.1} % of the lap on the kerbs: softer fast damping and longer packers advised.", s.kerb_usage),
            ),
            vec![ev("vibreurs", s.kerb_usage as f64, "%")],
            vec![],
        );
    }
    if b.c.has_throttle && s.throttle_smoothness < 40.0 {
        b.label("gaz brutaux");
        b.push(
            "S-05/brutal",
            "S-05",
            &[],
            Severity::Info,
            0.5,
            Some(CornerPhase::Exit),
            None,
            ("Application des gaz brutale".into(), "Harsh throttle application".into()),
            (
                format!("Lissage des gaz {:.0}/100 : TC plus haut, diff power plus fermé ou barre AR plus souple limitent le patinage.", s.throttle_smoothness),
                format!("Throttle smoothness {:.0}/100: higher TC, more power lock or a softer rear bar limit wheel-spin.", s.throttle_smoothness),
            ),
            vec![ev("lissage gaz", s.throttle_smoothness as f64, "/100")],
            vec![],
        );
    } else if b.c.has_throttle && s.throttle_smoothness > 75.0 {
        b.label("gaz progressifs");
    }
    if s.coasting_pct > 15.0 {
        b.label("beaucoup de roue libre");
        b.push(
            "S-09/coasting",
            "S-09",
            &[],
            Severity::Info,
            0.6,
            None,
            None,
            ("Beaucoup de roue libre".into(), "A lot of coasting".into()),
            (
                format!("{:.0} % du temps au-dessus de 60 km/h sans gaz ni frein : marge de temps au tour côté pilotage avant de toucher au setup.", s.coasting_pct),
                format!("{:.0} % of the time above 60 km/h with neither pedal: lap time to find in the driving before touching the setup.", s.coasting_pct),
            ),
            vec![ev("roue libre", s.coasting_pct as f64, "%")],
            vec![],
        );
    }
}

fn consistency(b: &mut Builder) {
    let c = b.c;
    let st = &c.lap_stats;
    if c.n_laps >= 2 {
        if st.consistency_score < 60.0 {
            b.label("pilote irrégulier");
            b.push(
                "D-CON-01/irregular",
                "D-CON-01",
                &["PHILOSOPHY_SAFE"],
                Severity::Minor,
                0.6,
                None,
                None,
                ("Temps au tour irréguliers".into(), "Irregular lap times".into()),
                (
                    format!(
                        "Régularité {:.0}/100 (écart-type {:.2} s sur {} tours) : une voiture plus stable (orientation sûre) aidera plus qu'un gain de pointe.",
                        st.consistency_score, st.std_valid_s.unwrap_or(0.0), c.n_laps
                    ),
                    format!(
                        "Consistency {:.0}/100 (std dev {:.2} s over {} laps): a more stable car (safe orientation) will help more than outright pace.",
                        st.consistency_score, st.std_valid_s.unwrap_or(0.0), c.n_laps
                    ),
                ),
                vec![ev("régularité", st.consistency_score as f64, "/100"), ev("écart-type", st.std_valid_s.unwrap_or(0.0) as f64, "s")],
                vec![],
            );
        } else if st.consistency_score > 85.0 {
            b.label("pilote régulier");
            b.push(
                "D-CON-01/regular",
                "D-CON-01",
                &["PHILOSOPHY_AGGRESSIVE"],
                Severity::Minor,
                0.5,
                None,
                None,
                ("Temps au tour réguliers".into(), "Consistent lap times".into()),
                (
                    format!(
                        "Régularité {:.0}/100 (écart-type {:.2} s sur {} tours) : le pilote peut exploiter une voiture plus vive.",
                        st.consistency_score, st.std_valid_s.unwrap_or(0.0), c.n_laps
                    ),
                    format!(
                        "Consistency {:.0}/100 (std dev {:.2} s over {} laps): the driver can exploit a sharper car.",
                        st.consistency_score, st.std_valid_s.unwrap_or(0.0), c.n_laps
                    ),
                ),
                vec![ev("régularité", st.consistency_score as f64, "/100")],
                vec![],
            );
        }
    }
    if let Some(f) = st.fuel_per_lap_l {
        if f > 0.0 {
            b.push(
                "D-CON-02/fuel",
                "D-CON-02",
                &[],
                Severity::Info,
                0.9,
                None,
                None,
                ("Consommation par tour".into(), "Fuel per lap".into()),
                (
                    format!(
                        "{f:.2} L par tour{}.",
                        st.energy_per_lap
                            .map(|e| format!(", énergie virtuelle {e:.3} par tour"))
                            .unwrap_or_default()
                    ),
                    format!(
                        "{f:.2} L per lap{}.",
                        st.energy_per_lap
                            .map(|e| format!(", virtual energy {e:.3} per lap"))
                            .unwrap_or_default()
                    ),
                ),
                vec![ev("carburant", f as f64, "L/tour")],
                vec![],
            );
        }
    }
}
