import type {
  DataQuality,
  DetectedSymptom,
  FeedbackInterpretation,
  Finding,
  PlanIteration,
  SetupChange,
  SetupPlan,
} from "@corde/protocol";
import { describe, expect, it } from "vitest";
import {
  balanceColor,
  balanceInk,
  balanceWord,
  changeAmount,
  directionArrow,
  filePlanChoices,
  fmtEvidence,
  fmtIndex,
  fmtPct,
  fmtSigmas,
  fmtSigned,
  fmtSteps,
  interpretationQuestions,
  isHandLabel,
  isRelativeBalance,
  iterationOfFeedback,
  iterationToText,
  missingChannels,
  planExport,
  planToText,
  priorityDots,
  qualityLevel,
  resolvedLines,
  resolvedNotes,
  resolvedText,
  sortFindings,
  sortIterations,
  statusTone,
  symptomChips,
  symptomLabel,
  valueLabel,
  wheelsLabel,
} from "./lib";

const change = (over: Partial<SetupChange> = {}): SetupChange => ({
  parameter_id: "aero.rear_wing",
  parameter_key: "RWSetting",
  section: "REARWING",
  label_fr: "Aileron arrière",
  direction: "increase",
  magnitude: "1 cran",
  steps: 1,
  priority: 1,
  rule_ids: ["US-FAST-01"],
  rationale_fr: "Sous-virage en virage rapide → plus d'appui arrière.",
  rationale_en: "Fast-corner understeer → more rear downforce.",
  side_effects_fr: ["Vmax moindre"],
  conflicts_with: [],
  confidence: 0.6,
  ...over,
});

const plan: SetupPlan = {
  philosophy: "safe",
  dropped: [],
  intent_fr: "Voiture stable et prévisible pour les relais longs.",
  changes: [
    change(),
    change({
      parameter_id: "controls.tc",
      parameter_key: "TractionControlMapSetting",
      section: "CONTROLS",
      label_fr: "Contrôle de traction",
      direction: "decrease",
      magnitude: "1 (jamais < 6-6-6 sans test)",
      steps: 1,
      priority: 3,
      side_effects_fr: [],
      conflicts_with: ["PHI-SAFE-01"],
    }),
    change({
      parameter_id: "tyres.compound",
      parameter_key: "CompoundSetting",
      section: "",
      label_fr: "Composé de pneu",
      direction: "set",
      magnitude: "wet",
      steps: null,
      priority: 5,
      side_effects_fr: [],
    }),
  ],
  warnings_fr: ["Session sèche : paquet pluie générique."],
};

const meta = {
  session_id: "20261104-120241-paul-ricard-1a-race",
  track_name: "Paul Ricard - 1A",
  vehicle: "Corde Racing #88",
  generated_ms: 1_762_300_000_000,
};

describe("balance colours", () => {
  it("is blue for oversteer, red for understeer, grey around zero", () => {
    expect(balanceColor(-0.8, 1)).toMatch(/^rgba\(78, 161, 230, /);
    expect(balanceColor(0.8, 1)).toMatch(/^rgba\(240, 80, 60, /);
    expect(balanceColor(0, 1)).toMatch(/^rgba\(147, 160, 174, /);
    expect(balanceInk(-0.5)).toBe("var(--cold)");
    expect(balanceInk(0.5)).toBe("var(--hot)");
    expect(balanceInk(0.05)).toBe("var(--text)");
  });

  it("fades with the confidence and is transparent without samples", () => {
    const alpha = (css: string) => Number(css.match(/, ([\d.]+)\)$/)?.[1]);
    expect(alpha(balanceColor(0.5, 1))).toBeGreaterThan(alpha(balanceColor(0.5, 0.2)));
    expect(alpha(balanceColor(0.5, 0.2))).toBeGreaterThan(0.1);
    expect(alpha(balanceColor(0.9, 1))).toBeLessThanOrEqual(0.7);
    expect(balanceColor(0.5, 1, 0)).toBe("transparent");
    expect(balanceColor(Number.NaN, 1)).toBe("transparent");
    expect(balanceInk(0.5, 0)).toBe("var(--text-faint)");
  });

  it("saturates at |0.6| (a major finding) and never beyond ±1", () => {
    expect(balanceColor(0.6, 1)).toMatch(/^rgba\(240, 80, 60, /);
    expect(balanceColor(3, 1)).toMatch(/^rgba\(240, 80, 60, /);
    // halfway between grey and red
    expect(balanceColor(0.3, 1)).toMatch(/^rgba\(194, 120, 117, /);
  });

  it("names the balance and formats the index", () => {
    expect(balanceWord(-0.3)).toBe("oversteer");
    expect(balanceWord(0.3)).toBe("understeer");
    expect(balanceWord(0.02)).toBe("neutral");
    expect(balanceWord(0.5, 0)).toBeNull();
    expect(fmtIndex(0.4851)).toBe("+0.49");
    expect(fmtIndex(-0.12)).toBe("−0.12");
    expect(fmtIndex(0.001)).toBe("0.00");
    expect(fmtIndex(Number.NaN)).toBe("—");
  });

  it("knows a matrix relative to the session reference and shows its deviation in σ", () => {
    expect(isRelativeBalance({})).toBe(false);
    expect(
      isRelativeBalance({
        reference: {
          raw_index: 0.38,
          slip_balance: 0.25,
          slip_balance_sigma: 0.08,
          gradient_deg_per_g: null,
          gradient_sigma_deg_per_g: null,
          passes: 120,
          trigger_index: 0.3,
        },
      }),
    ).toBe(true);
    // ±1 = 5 σ: the 0.3 trigger is 1.5 σ.
    expect(fmtSigmas(0.3)).toBe("+1.5 σ");
    expect(fmtSigmas(-0.62)).toBe("−3.1 σ");
    expect(fmtSigmas(0.004)).toBe("0.0 σ");
    expect(fmtSigmas(Number.NaN)).toBe("—");
  });
});

describe("setup changes as text", () => {
  it("turns a direction into an arrow and steps into crans / steps", () => {
    expect(directionArrow("increase")).toBe("▲");
    expect(directionArrow("decrease")).toBe("▼");
    expect(directionArrow("toward_zero")).toBe("→");
    expect(directionArrow("set")).toBe("=");
    expect(changeAmount(change(), "fr")).toEqual({ arrow: "▲", amount: "1 cran", note: null });
    expect(changeAmount(change({ steps: 2, magnitude: "2 clics" }), "fr")).toEqual({
      arrow: "▲",
      amount: "2 crans",
      note: null,
    });
    expect(changeAmount(change({ steps: 2, magnitude: "2 clics" }), "en").amount).toBe("2 steps");
    expect(changeAmount(change({ steps: 1 }), "en").amount).toBe("1 step");
  });

  it("keeps the rule's wording when it says more than the step count", () => {
    const c = change({
      direction: "decrease",
      steps: 1,
      magnitude: "1 cran (ou arb_front −1)",
    });
    expect(changeAmount(c, "fr")).toEqual({
      arrow: "▼",
      amount: "1 cran",
      note: "1 cran (ou arb_front −1)",
    });
    expect(changeAmount(change({ steps: null, magnitude: "0.5–1 % vs base" }), "fr")).toEqual({
      arrow: "▲",
      amount: "0.5–1 % vs base",
      note: null,
    });
    expect(changeAmount(change({ direction: "set", steps: null, magnitude: "wet" }), "fr")).toEqual(
      { arrow: "=", amount: "wet", note: null },
    );
    expect(changeAmount(change({ steps: null, magnitude: "" }), "fr").amount).toBe("à ajuster");
  });

  it("does not call a physical quantity a number of clicks", () => {
    // the server parses the leading integer of any magnitude into `steps`
    expect(changeAmount(change({ steps: 15, magnitude: "15 Nm" }), "fr")).toEqual({
      arrow: "▲",
      amount: "15 Nm",
      note: null,
    });
    expect(changeAmount(change({ steps: 3, magnitude: "3 kPa (essieu concerné)" }), "fr")).toEqual({
      arrow: "▲",
      amount: "3 kPa (essieu concerné)",
      note: null,
    });
    expect(changeAmount(change({ steps: 3, magnitude: "+3–5 mm (+ AR)" }), "en").amount).toBe(
      "+3–5 mm (+ AR)",
    );
    expect(changeAmount(change({ steps: 1, magnitude: "1 (agressif) / +1 (sûr)" }), "fr")).toEqual({
      arrow: "▲",
      amount: "1 cran",
      note: "1 (agressif) / +1 (sûr)",
    });
  });

  it("maps priority 1 to five dots and priority 5 to one", () => {
    expect(priorityDots(1)).toBe(5);
    expect(priorityDots(3)).toBe(3);
    expect(priorityDots(5)).toBe(1);
    expect(priorityDots(9)).toBe(1);
    expect(priorityDots(0)).toBe(5);
  });

  it("writes a plan as a pasteable list, in the server's order", () => {
    const text = planToText(plan, meta, "fr", new Date("2026-11-04T12:30:00Z"));
    const lines = text.split("\n");
    expect(lines[0]).toBe("Corde — réglages proposés · Soft & sûr");
    expect(lines[1]).toBe(
      "Paul Ricard - 1A · Corde Racing #88 · 20261104-120241-paul-ricard-1a-race",
    );
    expect(lines[2]).toBe("2026-11-04 12:30");
    expect(text).toContain("Voiture stable et prévisible pour les relais longs.");
    expect(text).toContain("1. Aileron arrière : ▲ 1 cran\n   priorité 1 · REARWING / RWSetting");
    expect(text).toContain("   pourquoi : Sous-virage en virage rapide → plus d'appui arrière.");
    expect(text).toContain("   effets secondaires : Vmax moindre");
    expect(text).toContain(
      "2. Contrôle de traction : ▼ 1 cran (1 (jamais < 6-6-6 sans test))\n   priorité 3 · CONTROLS / TractionControlMapSetting",
    );
    expect(text).toContain("3. Composé de pneu : = wet\n   priorité 5 · CompoundSetting");
    expect(text).toContain("Avertissements :\n- Session sèche : paquet pluie générique.");
    expect(text.trimEnd().endsWith("garage du jeu.")).toBe(true);
    expect(text).not.toContain("prochaine version");
    expect(text.indexOf("1. Aileron")).toBeLessThan(text.indexOf("2. Contrôle"));
  });

  it("writes the English version with the English rationale", () => {
    const text = planToText(plan, meta, "en");
    expect(text.startsWith("Corde — proposed setup changes · Soft & safe")).toBe(true);
    expect(text).toContain("why : Fast-corner understeer → more rear downforce.");
    expect(text).toContain("   priority 3 · CONTROLS / TractionControlMapSetting");
    expect(text).toContain("Warnings:");
  });

  it("exports the plan as JSON with where it comes from", () => {
    const json = JSON.parse(planExport(plan, meta)) as Record<string, unknown>;
    expect(json.session_id).toBe(meta.session_id);
    expect(json.track_name).toBe("Paul Ricard - 1A");
    expect(json.generated_ms).toBe(meta.generated_ms);
    expect((json.plan as SetupPlan).changes).toHaveLength(3);
    expect((json.plan as SetupPlan).philosophy).toBe("safe");
  });
});

describe("findings and data quality", () => {
  const finding = (id: string, severity: Finding["severity"], confidence: number): Finding => ({
    id,
    diagnostic_id: id,
    symptoms: [],
    severity,
    confidence,
    phase: null,
    corner_speed: null,
    title_fr: id,
    title_en: id,
    detail_fr: "",
    detail_en: "",
    evidence: [],
    lap_numbers: [],
  });

  it("sorts major first, then by confidence, then by id", () => {
    const sorted = sortFindings([
      finding("c", "info", 0.9),
      finding("b", "minor", 0.4),
      finding("a", "minor", 0.4),
      finding("d", "major", 0.2),
      finding("e", "minor", 0.8),
    ]).map((f) => f.id);
    expect(sorted).toEqual(["d", "e", "a", "b", "c"]);
  });

  const quality = (over: Partial<DataQuality> = {}): DataQuality => ({
    sample_rate_hz: 20,
    valid_laps: 5,
    total_laps: 6,
    has_slip: true,
    has_ride_height: true,
    has_optimal_temp: false,
    wet_session: false,
    notes: [],
    ...over,
  });

  it("rates the data and lists the missing channels", () => {
    expect(qualityLevel(quality())).toBe("good");
    expect(qualityLevel(quality({ has_slip: false }))).toBe("limited");
    expect(qualityLevel(quality({ valid_laps: 2, has_slip: false }))).toBe("poor");
    expect(missingChannels(quality())).toEqual(["optimalTemp"]);
    expect(missingChannels(quality({ has_slip: false, has_ride_height: false }))).toEqual([
      "slip",
      "rideHeight",
      "optimalTemp",
    ]);
    expect(missingChannels(quality({ has_optimal_temp: true }))).toEqual([]);
  });

  it("formats evidence with the decimals its magnitude needs", () => {
    expect(fmtEvidence({ label: "pic", value: 720, unit: "°C" })).toBe("720 °C");
    expect(fmtEvidence({ label: "moyenne", value: 653.85, unit: "°C" })).toBe("654 °C");
    expect(fmtEvidence({ label: "activité ABS", value: 78.24, unit: "%" })).toBe("78.2 %");
    expect(fmtEvidence({ label: "indice", value: 0.48503, unit: "" })).toBe("0.49");
    expect(fmtEvidence({ label: "I−O AVG", value: -4.9389, unit: "°C" })).toBe("−4.94 °C");
    expect(fmtEvidence({ label: "agressivité", value: 95.19, unit: "/100" })).toBe("95.2/100");
    expect(fmtEvidence({ label: "échantillons", value: 765, unit: "" })).toBe("765");
    expect(fmtEvidence({ label: "x", value: Number.NaN, unit: "" })).toBe("—");
  });

  it("formats signed numbers", () => {
    expect(fmtSigned(1.25, 1, " kPa")).toBe("+1.3 kPa");
    expect(fmtSigned(-0.5)).toBe("−0.5");
    expect(fmtSigned(0, 0)).toBe("0");
    expect(fmtSigned(null)).toBe("—");
  });
});

describe("feedback interpretation and plan iterations", () => {
  const symptom = (over: Partial<DetectedSymptom> = {}): DetectedSymptom => ({
    symptom_id: "US_ENTRY_SLOW",
    label_fr: "Sous-virage en entrée (virages lents)",
    label_en: "Understeer on entry (slow corners)",
    phase: "entry",
    corner_speed: "slow",
    axle: "front",
    wet: false,
    intensity: null,
    negated: false,
    matched_text: "l'avant ne tourne pas",
    confidence: 0.85,
    ...over,
  });
  const interpretation: FeedbackInterpretation = {
    feedback_id: 3,
    symptoms: [
      symptom(),
      symptom({
        symptom_id: "OS_EXIT",
        label_fr: "Survirage en sortie",
        label_en: "Oversteer on exit",
        phase: "exit",
        corner_speed: null,
        axle: "rear",
        intensity: "severe",
        wet: true,
        negated: true,
        matched_text: "l'arrière ne part plus",
        confidence: 0.7,
      }),
    ],
    questions_fr: ["Dans les virages lents, rapides ou les deux ?"],
    questions_en: ["In slow corners, fast corners or both?"],
    unmatched_fragments: ["surtout au T3"],
    extractor: "dictionary",
    confidence: 0.78,
    notes_fr: [],
  };
  const iteration: PlanIteration = {
    current_setup_known: false,
    dropped: [],
    id: 11,
    session_id: meta.session_id,
    created_ms: 1_762_300_100_000,
    iteration: 2,
    base_plan: "safe",
    feedback_id: 3,
    interpretation,
    data_findings_used: ["D-BAL-01/entry/slow"],
    changes: [
      change({
        parameter_id: "brake.bias_front",
        label_fr: "Répartition de freinage",
        direction: "decrease",
        magnitude: "0,5 %",
        steps: null,
      }),
      change(),
    ],
    delta: [
      change({
        parameter_id: "brake.bias_front",
        parameter_key: "RearBrakeSetting",
        section: "CONTROLS",
        label_fr: "Répartition de freinage",
        direction: "decrease",
        magnitude: "0,5 %",
        steps: null,
        rationale_fr: "Itération 2 — Ressenti : Sous-virage en entrée → on redonne de la rotation.",
        rationale_en: "Iteration 2 — Felt: understeer on entry → rule US-ENTRY-SLOW-01",
      }),
    ],
    agreements_fr: ["Ressenti « Sous-virage » confirmé par les données (confiance 50 % → 80 %)."],
    disagreements_fr: [],
    notes_fr: ["Itération 2 : 1 changement sur la famille « freinage », 1 cran chacun."],
  };

  it("turns a symptom's attributes into ordered chips in both languages", () => {
    expect(symptomChips(interpretation.symptoms[0] as DetectedSymptom, "fr")).toEqual([
      { kind: "phase", text: "entrée" },
      { kind: "speed", text: "lents" },
      { kind: "axle", text: "avant" },
    ]);
    expect(
      symptomChips(interpretation.symptoms[1] as DetectedSymptom, "en").map((c) => c.text),
    ).toEqual(["exit", "rear", "severe", "wet"]);
    expect(symptomChips(symptom({ phase: null, corner_speed: null, axle: null }), "fr")).toEqual(
      [],
    );
    expect(symptomLabel(symptom(), "en")).toBe("Understeer on entry (slow corners)");
    expect(fmtPct(0.846)).toBe("85 %");
    expect(fmtPct(Number.NaN)).toBe("—");
  });

  it("picks the questions of the UI language when both lists line up", () => {
    expect(interpretationQuestions(interpretation, "en")).toEqual([
      "In slow corners, fast corners or both?",
    ]);
    expect(interpretationQuestions({ ...interpretation, questions_en: [] }, "en")).toEqual(
      interpretation.questions_fr,
    );
  });

  it("finds the iteration of a feedback and sorts newest first", () => {
    const older: PlanIteration = {
      ...iteration,
      id: 4,
      iteration: 1,
      feedback_id: 1,
      created_ms: 1,
    };
    expect(iterationOfFeedback([older, iteration], 3)?.id).toBe(11);
    expect(iterationOfFeedback([older, iteration], 99)).toBeNull();
    expect(sortIterations([older, iteration]).map((i) => i.id)).toEqual([11, 4]);
  });

  it("writes an iteration as text: symptoms, the delta first, verdicts, the cumulative plan", () => {
    const text = iterationToText(iteration, meta, "fr", new Date("2026-11-04T12:30:00Z"));
    const lines = text.split("\n");
    expect(lines[0]).toBe("Corde — itération 2 · Soft & sûr");
    expect(lines[1]).toBe(`${meta.track_name} · ${meta.vehicle} · ${meta.session_id}`);
    expect(lines[2]).toBe("2026-11-04 12:30");
    expect(text).toContain("Symptômes ressentis :");
    expect(text).toContain(
      "- Sous-virage en entrée (virages lents) [entrée, lents, avant] — « l'avant ne tourne pas »",
    );
    expect(text).toContain("- (plus de) Survirage en sortie [sortie, arrière, très marqué, pluie]");
    expect(text).toContain(
      "À changer maintenant :\n1. Répartition de freinage : ▼ 0,5 % · CONTROLS / RearBrakeSetting",
    );
    expect(text).toContain("   pourquoi : Itération 2 — Ressenti");
    expect(text).toContain("Ressenti et données d'accord :\n- Ressenti « Sous-virage »");
    expect(text).not.toContain("Désaccords");
    expect(text).toContain("Notes :\n- Itération 2 : 1 changement");
    expect(text).toContain(
      "Plan cumulé :\n1. Répartition de freinage : ▼ 0,5 %\n2. Aileron arrière : ▲ 1 cran",
    );
    const en = iterationToText({ ...iteration, delta: [] }, meta, "en");
    expect(en).toContain("Corde — iteration 2 · Soft & safe");
    expect(en).toContain("Change now:\n- nothing new");
    expect(en).toContain("Symptoms felt:");
    expect(en).toContain("(no more) Oversteer on exit [exit, rear, severe, wet]");
  });
});

/* ------------------------------------------------------------------ real setup */

const resolvedBar = change({
  parameter_id: "susp.arb_rear",
  parameter_key: "RearAntiSwaySetting",
  section: "SUSPENSION",
  label_fr: "Barre antiroulis arrière",
  direction: "decrease",
  resolved: {
    status: "applied",
    steps: -1,
    values: [
      {
        section: "SUSPENSION",
        key: "RearAntiSwaySetting",
        wheel: null,
        index_before: 2,
        index_after: 1,
        label_before: "P2",
        label_after: "P1",
        min: 0,
        max: null,
        status: "applied",
      },
    ],
    notes_fr: [],
    notes_en: [],
  },
});

const camber = (fl: [number, string | null], fr: [number, string | null]) =>
  change({
    parameter_id: "geo.camber_front",
    parameter_key: "CamberSetting",
    section: "FRONTLEFT / FRONTRIGHT",
    label_fr: "Carrossage avant",
    resolved: {
      status: "unknown_bounds",
      steps: -3,
      values: [
        {
          section: "FRONTLEFT",
          key: "CamberSetting",
          wheel: "FL",
          index_before: 22,
          index_after: fl[0],
          label_before: "-2.20 deg",
          label_after: fl[1],
          min: 0,
          max: null,
          status: "unknown_bounds",
        },
        {
          section: "FRONTRIGHT",
          key: "CamberSetting",
          wheel: "FR",
          index_before: 22,
          index_after: fr[0],
          label_before: "-2.20 deg",
          label_after: fr[1],
          min: 0,
          max: null,
          status: "unknown_bounds",
        },
      ],
      notes_fr: ["0.2–0.3° → 3 crans (pas appris sur tes fichiers : 0.0667 deg par cran)."],
      notes_en: ["0.2–0.3° → 3 steps (step learnt from your files: 0.0667 deg per step)."],
    },
  });

describe("resolved changes", () => {
  it("counts steps with a sign and the right word", () => {
    expect(fmtSteps(1, "fr")).toBe("+1 cran");
    expect(fmtSteps(-2, "fr")).toBe("−2 crans");
    expect(fmtSteps(-1, "en")).toBe("−1 step");
    expect(fmtSteps(3, "en")).toBe("+3 steps");
  });

  it("shows the game's label, never a hand-written one", () => {
    expect(isHandLabel("+2 crans")).toBe(true);
    expect(isHandLabel("Corde : +1 cran")).toBe(true);
    expect(isHandLabel("P2")).toBe(false);
    expect(valueLabel("P2", 2, "fr")).toBe("P2");
    expect(valueLabel("+2 crans", 8, "fr")).toBe("cran 8");
    expect(valueLabel(null, 7, "en")).toBe("step 7");
    expect(valueLabel("  ", 7, "fr")).toBe("cran 7");
  });

  it("names groups of wheels", () => {
    expect(wheelsLabel(["FL", "FR"], "fr")).toBe("AV");
    expect(wheelsLabel(["RL", "RR"], "en")).toBe("rear");
    expect(wheelsLabel(["FL", "FR", "RL", "RR"], "fr")).toBe("4 roues");
    expect(wheelsLabel(["FL"], "fr")).toBe("AVG");
    expect(wheelsLabel([null], "fr")).toBeNull();
  });

  it("writes before → after, grouping identical wheels", () => {
    expect(resolvedLines(resolvedBar, "fr")).toEqual([
      { wheels: null, before: "P2", after: "P1", delta: "−1 cran", moved: true, status: "applied" },
    ]);
    expect(resolvedText(resolvedBar, "fr")).toBe("P2 → P1 (−1 cran)");
    const same = camber([19, "-2.40 deg"], [19, "-2.40 deg"]);
    expect(resolvedText(same, "fr")).toBe("AV -2.20 deg → -2.40 deg (−3 crans)");
    // Unknown label after the change: the index, with the step count.
    const split = camber([19, null], [20, null]);
    expect(resolvedText(split, "fr")).toBe(
      "AVG -2.20 deg → cran 19 (−3 crans) ; AVD -2.20 deg → cran 20 (−2 crans)",
    );
    expect(resolvedText(change(), "fr")).toBeNull();
    expect(resolvedNotes(same, "en")[0]).toContain("step learnt");
    expect(resolvedNotes(same, "fr")[0]).toContain("pas appris");
  });

  it("colours statuses by what the driver has to do", () => {
    expect(statusTone("applied")).toBe("ok");
    expect(statusTone("unknown_bounds")).toBe("warn");
    expect(statusTone("clamped_max")).toBe("warn");
    expect(statusTone("manual")).toBe("manual");
    expect(statusTone("not_adjustable")).toBe("off");
    expect(statusTone("duplicate")).toBe("off");
  });

  it("puts the real values and the left-out changes in the text export", () => {
    const resolvedPlan: SetupPlan = {
      ...plan,
      changes: [resolvedBar],
      dropped: [
        change({
          parameter_id: "diff.power",
          label_fr: "Différentiel en charge",
          resolved: {
            status: "not_adjustable",
            steps: 0,
            values: [],
            notes_fr: [
              "Non réglable sur cette voiture (DiffPowerSetting = « Non-adjustable ») : écarté.",
            ],
            notes_en: ["Not adjustable on this car: left out."],
          },
        }),
      ],
    };
    const text = planToText(resolvedPlan, meta, "fr");
    expect(text).toContain("   dans ton réglage : P2 → P1 (−1 cran)");
    expect(text).toContain(
      "Écartés :\n- Différentiel en charge : Non réglable sur cette voiture (DiffPowerSetting = « Non-adjustable ») : écarté.",
    );
    expect(planToText(resolvedPlan, meta, "en")).toContain(
      "Left out:\n- Différentiel en charge : Not adjustable",
    );
  });

  it("offers the three plans and the latest iteration of each plan as files", () => {
    const it = (base_plan: "safe" | "wet", n: number) =>
      ({ base_plan, iteration: n }) as unknown as PlanIteration;
    expect(filePlanChoices([]).map((c) => c.ref)).toEqual(["aggressive", "safe", "wet"]);
    const choices = filePlanChoices([it("safe", 1), it("safe", 3), it("wet", 1), it("safe", 2)]);
    expect(choices.map((c) => c.ref)).toEqual([
      "aggressive",
      "safe",
      "wet",
      "iteration:safe:3",
      "iteration:wet:1",
    ]);
    expect(choices[3]).toEqual({ ref: "iteration:safe:3", philosophy: "safe", iteration: 3 });
  });
});
