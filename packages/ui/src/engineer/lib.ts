import type {
  AnalysisReport,
  Axle,
  BalanceAnalysis,
  BrakeWindow,
  CamberHint,
  ChangeDirection,
  CornerPhase,
  CornerSpeed,
  DataQuality,
  DetectedSymptom,
  DrivingStyle,
  Evidence,
  FeedbackInterpretation,
  Finding,
  PlanIteration,
  ResolvedValue,
  ResolveStatus,
  SetupChange,
  SetupPhilosophy,
  SetupPlan,
  Severity,
  SymptomIntensity,
} from "@corde/protocol";
import type { Lang } from "../store/settings";

/* ------------------------------------------------------------------ balance colours */

/** Oversteer blue (−1) → neutral → understeer red (+1); matches tokens.css `--cold` / `--hot`. */
const OVERSTEER: [number, number, number] = [78, 161, 230];
const UNDERSTEER: [number, number, number] = [240, 80, 60];
const NEUTRAL: [number, number, number] = [147, 160, 174];

const clamp = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v));

/**
 * Background of a balance cell. The hue follows the sign of the index, the saturation its
 * magnitude (|0.3| is where a finding fires, |0.6| is "major"), and the alpha the confidence,
 * so an uncertain cell fades towards the panel instead of shouting.
 */
export function balanceColor(index: number, confidence: number, samples = 1): string {
  if (samples <= 0 || !Number.isFinite(index)) return "transparent";
  const mag = clamp(Math.abs(index), 0, 1);
  const target = index < 0 ? OVERSTEER : UNDERSTEER;
  // 0 → neutral grey, 0.6+ → full hue.
  const t = clamp(mag / 0.6, 0, 1);
  const rgb = NEUTRAL.map((n, i) => Math.round(n + ((target[i] ?? n) - n) * t));
  const alpha = 0.12 + 0.58 * clamp(confidence, 0, 1) * (0.35 + 0.65 * mag);
  return `rgba(${rgb[0]}, ${rgb[1]}, ${rgb[2]}, ${alpha.toFixed(3)})`;
}

/** Text colour for the index itself (always legible on the panel). */
export function balanceInk(index: number, samples = 1): string {
  if (samples <= 0 || !Number.isFinite(index)) return "var(--text-faint)";
  if (Math.abs(index) < 0.1) return "var(--text)";
  return index < 0 ? "var(--cold)" : "var(--hot)";
}

export type BalanceWord = "oversteer" | "neutral" | "understeer";

export function balanceWord(index: number, samples = 1): BalanceWord | null {
  if (samples <= 0 || !Number.isFinite(index)) return null;
  if (index <= -0.1) return "oversteer";
  if (index >= 0.1) return "understeer";
  return "neutral";
}

/** `+0.49` / `−0.12` / `0.00`. */
export function fmtIndex(index: number): string {
  if (!Number.isFinite(index)) return "—";
  if (Math.abs(index) < 0.005) return "0.00";
  return `${index < 0 ? "−" : "+"}${Math.abs(index).toFixed(2)}`;
}

/** A relative index of ±1 is this many robust standard deviations from the session reference. */
export const BALANCE_SIGMAS_FULL_SCALE = 5;

/**
 * `true` when the matrix is relative to the session reference (reports since the S2
 * calibration). Older cached reports hold the absolute index and no reference.
 */
export function isRelativeBalance(balance: Pick<BalanceAnalysis, "reference">): boolean {
  return balance.reference !== undefined && balance.reference !== null;
}

/** Relative index → deviation from the reference: `+1.5 σ` / `−3.0 σ` / `0.0 σ`. */
export function fmtSigmas(index: number): string {
  if (!Number.isFinite(index)) return "—";
  const s = index * BALANCE_SIGMAS_FULL_SCALE;
  if (Math.abs(s) < 0.05) return "0.0 σ";
  return `${s < 0 ? "−" : "+"}${Math.abs(s).toFixed(1)} σ`;
}

/* ------------------------------------------------------------------ severities */

export const SEVERITY_RANK: Record<Severity, number> = { major: 0, minor: 1, info: 2 };

/** Major first, then by confidence (highest first), then by id for a stable order. */
export function sortFindings(findings: readonly Finding[]): Finding[] {
  return [...findings].sort(
    (a, b) =>
      SEVERITY_RANK[a.severity] - SEVERITY_RANK[b.severity] ||
      b.confidence - a.confidence ||
      a.id.localeCompare(b.id),
  );
}

/* ------------------------------------------------------------------ data quality */

export type QualityLevel = "good" | "limited" | "poor";

/**
 * One word about what the analysis could rely on: `poor` under 3 clean laps (every
 * confidence is halved), `limited` when a channel family is missing, `good` otherwise.
 */
export function qualityLevel(q: DataQuality): QualityLevel {
  if (q.valid_laps < 3) return "poor";
  if (!q.has_slip || !q.has_ride_height) return "limited";
  return "good";
}

/** Channel families the server did not find (translation keys are built by the caller). */
export function missingChannels(q: DataQuality): ("slip" | "rideHeight" | "optimalTemp")[] {
  const out: ("slip" | "rideHeight" | "optimalTemp")[] = [];
  if (!q.has_slip) out.push("slip");
  if (!q.has_ride_height) out.push("rideHeight");
  if (!q.has_optimal_temp) out.push("optimalTemp");
  return out;
}

/* ------------------------------------------------------------------ driving style */

export type StyleKey =
  | "brake_aggression"
  | "trail_braking"
  | "throttle_smoothness"
  | "steering_smoothness"
  | "kerb_usage";

export const STYLE_KEYS: readonly StyleKey[] = [
  "brake_aggression",
  "trail_braking",
  "throttle_smoothness",
  "steering_smoothness",
  "kerb_usage",
];

export function styleValue(style: DrivingStyle, key: StyleKey): number {
  return clamp(Number.isFinite(style[key]) ? style[key] : 0, 0, 100);
}

/* ------------------------------------------------------------------ setup changes */

export type Arrow = "▲" | "▼" | "→" | "=";

export function directionArrow(d: ChangeDirection): Arrow {
  switch (d) {
    case "increase":
      return "▲";
    case "decrease":
      return "▼";
    case "toward_zero":
      return "→";
    case "set":
      return "=";
  }
}

export interface ChangeAmount {
  arrow: Arrow;
  /** What the pilot turns: `1 cran`, `2 crans`, or the rule's own wording when no step count. */
  amount: string;
  /** The rule's wording when it says more than the step count (`1 cran (ou arb_front −1)`). */
  note: string | null;
}

/** `1`, `2 crans`, `3 clics`: a bare number of engineer steps. */
const STEP_RE = /^\s*[+−-]?\s*\d+\s*(crans?|clics?|clicks?|steps?)?\s*$/i;
/** `15 Nm`, `3 kPa (essieu concerné)`, `0.5 %`: a physical quantity, not a number of clicks. */
const UNIT_RE =
  /^\s*[+−-]?\s*\d+([.,]\d+)?\s*(–|-)?\s*(\d+([.,]\d+)?)?\s*(nm|kpa|bar|psi|mm|cm|°|%|l|kg|km\/h)\b/i;

/**
 * `▲ 1 cran` for a counted change, `▼ 15 Nm` for a quantity, `= wet` for a discrete value, the
 * rule's own text otherwise. `note` carries the rest of the magnitude when it says more than the
 * number of steps (`1 cran (ou arb_front −1)`).
 */
export function changeAmount(c: SetupChange, lang: Lang): ChangeAmount {
  const arrow = directionArrow(c.direction);
  const magnitude = c.magnitude.trim();
  if (c.steps !== null && c.steps > 0 && !UNIT_RE.test(magnitude)) {
    const unit = lang === "fr" ? (c.steps > 1 ? "crans" : "cran") : c.steps > 1 ? "steps" : "step";
    const amount = `${c.steps} ${unit}`;
    const note = magnitude && !STEP_RE.test(magnitude) ? magnitude : null;
    return { arrow, amount, note };
  }
  return { arrow, amount: magnitude || (lang === "fr" ? "à ajuster" : "to adjust"), note: null };
}

/* ------------------------------------------------------------------ real setup (resolved changes) */

/** A label the driver wrote by hand in his file (« +2 crans »): it says nothing about the value. */
export function isHandLabel(label: string | null | undefined): boolean {
  return !!label && /cran|clic|click|step|corde/i.test(label);
}

/** `+1 cran`, `−2 crans`, `+1 step`. */
export function fmtSteps(n: number, lang: Lang): string {
  const sign = n > 0 ? "+" : n < 0 ? "−" : "";
  const abs = Math.abs(n);
  const word = lang === "fr" ? (abs > 1 ? "crans" : "cran") : abs > 1 ? "steps" : "step";
  return `${sign}${abs} ${word}`;
}

/** What the game shows for a value, or its index when the label is unknown or hand-written. */
export function valueLabel(label: string | null | undefined, index: number, lang: Lang): string {
  const l = (label ?? "").trim();
  if (l && !isHandLabel(l)) return l;
  return lang === "fr" ? `cran ${index}` : `step ${index}`;
}

const WHEEL_WORD: Record<Lang, Record<string, string>> = {
  fr: { FL: "AVG", FR: "AVD", RL: "ARG", RR: "ARD" },
  en: { FL: "FL", FR: "FR", RL: "RL", RR: "RR" },
};

/** `AV`, `AR`, `4 roues`, `AVG`… for a group of wheels (`null` when not a per-wheel key). */
export function wheelsLabel(wheels: readonly (string | null)[], lang: Lang): string | null {
  const w = wheels.filter((x): x is string => !!x);
  if (w.length === 0) return null;
  const has = (k: string) => w.includes(k);
  if (w.length === 4) return lang === "fr" ? "4 roues" : "all 4";
  if (w.length === 2 && has("FL") && has("FR")) return lang === "fr" ? "AV" : "front";
  if (w.length === 2 && has("RL") && has("RR")) return lang === "fr" ? "AR" : "rear";
  return w.map((x) => WHEEL_WORD[lang][x] ?? x).join(" + ");
}

export interface ResolvedLine {
  /** `AV`, `AVG`… or null. */
  wheels: string | null;
  before: string;
  after: string;
  /** `−1 cran`; `""` when the index does not move. */
  delta: string;
  moved: boolean;
  status: ResolveStatus;
}

/**
 * `before → after` lines of a resolved change. Wheels with the same values share one line (`AV`,
 * `AR`, `4 roues`); the label after the change is the game's when Corde knows it, else the index
 * (`cran 7`) with the step count beside it.
 */
export function resolvedLines(c: SetupChange, lang: Lang): ResolvedLine[] {
  const r = c.resolved;
  if (!r) return [];
  const groups: { key: string; values: ResolvedValue[] }[] = [];
  for (const v of r.values) {
    const key = [
      v.key,
      v.index_before,
      v.index_after,
      v.label_before,
      v.label_after,
      v.status,
    ].join("|");
    const g = groups.find((x) => x.key === key);
    if (g) g.values.push(v);
    else groups.push({ key, values: [v] });
  }
  return groups.map(({ values }) => {
    const v = values[0] as ResolvedValue;
    const d = v.index_after - v.index_before;
    return {
      wheels: wheelsLabel(
        values.map((x) => x.wheel),
        lang,
      ),
      before: valueLabel(v.label_before, v.index_before, lang),
      after:
        d === 0
          ? valueLabel(v.label_before, v.index_before, lang)
          : valueLabel(v.label_after, v.index_after, lang),
      delta: d === 0 ? "" : fmtSteps(d, lang),
      moved: d !== 0,
      status: v.status,
    };
  });
}

export type ResolveTone = "ok" | "warn" | "manual" | "off";

/** Colour family of a status: applied (ok), applied with a caveat (warn), by hand, left out. */
export function statusTone(s: ResolveStatus): ResolveTone {
  switch (s) {
    case "applied":
      return "ok";
    case "clamped_min":
    case "clamped_max":
    case "unknown_bounds":
      return "warn";
    case "manual":
      return "manual";
    default:
      return "off";
  }
}

/** Notes of a resolved change in the UI language (the French list is the reference). */
export function resolvedNotes(c: SetupChange, lang: Lang): string[] {
  const r = c.resolved;
  if (!r) return [];
  if (lang === "en" && r.notes_en.length > 0) return r.notes_en;
  return r.notes_fr;
}

/** One-line text of a resolved change (`P2 → P1 (−1 cran)`, `AV −2.20 deg → −2.40 deg (−3 crans)`). */
export function resolvedText(c: SetupChange, lang: Lang): string | null {
  const lines = resolvedLines(c, lang);
  if (lines.length === 0) return null;
  return lines
    .map(
      (l) =>
        `${l.wheels ? `${l.wheels} ` : ""}${l.before} → ${l.after}${l.delta ? ` (${l.delta})` : ""}`,
    )
    .join(" ; ");
}

/** A plan the driver can turn into a `.svm` file. */
export interface FilePlanChoice {
  /** What `POST …/setup-files` takes: `safe`, `iteration:safe:2`. */
  ref: string;
  philosophy: SetupPhilosophy;
  /** `null` for the three plans of the report. */
  iteration: number | null;
}

/** The three plans, then the latest iteration of each plan that has one. */
export function filePlanChoices(iterations: readonly PlanIteration[]): FilePlanChoice[] {
  const out: FilePlanChoice[] = PHILOSOPHIES.map((p) => ({
    ref: p,
    philosophy: p,
    iteration: null,
  }));
  for (const p of PHILOSOPHIES) {
    const latest = iterations
      .filter((it) => it.base_plan === p)
      .reduce<PlanIteration | null>(
        (best, it) => (!best || it.iteration > best.iteration ? it : best),
        null,
      );
    if (latest) {
      out.push({
        ref: `iteration:${p}:${latest.iteration}`,
        philosophy: p,
        iteration: latest.iteration,
      });
    }
  }
  return out;
}

/** Priority 1 (first thing to try) → 5 filled dots; priority 5 → 1 dot. */
export function priorityDots(priority: number): number {
  return clamp(6 - Math.round(priority), 1, 5);
}

export const PHILOSOPHIES: readonly SetupPhilosophy[] = ["aggressive", "safe", "wet"];

/* ------------------------------------------------------------------ text export */

const PHILOSOPHY_NAME: Record<Lang, Record<SetupPhilosophy, string>> = {
  fr: { aggressive: "Agressif", safe: "Soft & sûr", wet: "Pluie" },
  en: { aggressive: "Aggressive", safe: "Soft & safe", wet: "Wet" },
};

export function philosophyName(p: SetupPhilosophy, lang: Lang): string {
  return PHILOSOPHY_NAME[lang][p];
}

/**
 * A readable version of a plan, to paste next to the game's garage: one line per change in the
 * server's (methodological) order, then the warnings. Pure, so it is tested.
 */
export function planToText(
  plan: SetupPlan,
  meta: Pick<AnalysisReport, "track_name" | "vehicle" | "session_id">,
  lang: Lang,
  now: Date = new Date(),
): string {
  const fr = lang === "fr";
  const lines: string[] = [];
  lines.push(
    `${fr ? "Corde — réglages proposés" : "Corde — proposed setup changes"} · ${philosophyName(plan.philosophy, lang)}`,
  );
  lines.push(`${meta.track_name} · ${meta.vehicle} · ${meta.session_id}`);
  lines.push(now.toISOString().slice(0, 16).replace("T", " "));
  lines.push("");
  lines.push(plan.intent_fr);
  lines.push("");
  plan.changes.forEach((c, i) => {
    const { arrow, amount, note } = changeAmount(c, lang);
    const where = c.section ? `${c.section} / ${c.parameter_key}` : c.parameter_key;
    lines.push(`${i + 1}. ${c.label_fr} : ${arrow} ${amount}${note ? ` (${note})` : ""}`);
    const real = resolvedText(c, lang);
    if (real) lines.push(`   ${fr ? "dans ton réglage" : "in your setup"} : ${real}`);
    lines.push(`   ${fr ? "priorité" : "priority"} ${c.priority} · ${where}`);
    const why = fr ? c.rationale_fr : c.rationale_en;
    if (why) lines.push(`   ${fr ? "pourquoi" : "why"} : ${why}`);
    if (c.side_effects_fr.length) {
      lines.push(
        `   ${fr ? "effets secondaires" : "side effects"} : ${c.side_effects_fr.join(" ; ")}`,
      );
    }
  });
  if (plan.dropped.length) {
    lines.push("");
    lines.push(fr ? "Écartés :" : "Left out:");
    for (const d of plan.dropped) {
      const why = resolvedNotes(d, lang).at(-1) ?? "";
      lines.push(`- ${d.label_fr}${why ? ` : ${why}` : ""}`);
    }
  }
  if (plan.warnings_fr.length) {
    lines.push("");
    lines.push(fr ? "Avertissements :" : "Warnings:");
    for (const w of plan.warnings_fr) lines.push(`- ${w}`);
  }
  lines.push("");
  lines.push(
    fr
      ? "Réglages calculés par Corde : la page Ingénieur crée les fichiers .svm prêts à charger dans le garage du jeu."
      : "Setup changes computed by Corde: the Engineer page writes ready-to-load .svm files for the game's garage.",
  );
  return lines.join("\n");
}

/** The JSON file a plan is exported as (the plan plus where it comes from). */
export function planExport(
  plan: SetupPlan,
  report: Pick<AnalysisReport, "session_id" | "track_name" | "vehicle" | "generated_ms">,
): string {
  return JSON.stringify(
    {
      session_id: report.session_id,
      track_name: report.track_name,
      vehicle: report.vehicle,
      generated_ms: report.generated_ms,
      plan,
    },
    null,
    2,
  );
}

/* ------------------------------------------------------------------ small helpers */

export const CAMBER_KEYS: Record<CamberHint, string> = {
  too_much: "eng.tyres.camber.tooMuch",
  too_little: "eng.tyres.camber.tooLittle",
  ok: "eng.tyres.camber.ok",
  unknown: "eng.tyres.camber.unknown",
};

export const BRAKE_WINDOW_COLOR: Record<BrakeWindow, string> = {
  cold: "var(--cold)",
  ok: "var(--optimal)",
  hot: "var(--hot)",
  unknown: "var(--text-faint)",
};

/**
 * `720 °C`, `78.2 %`, `0.32`, `95/100`: as many decimals as the magnitude needs (none ≥ 100,
 * one ≥ 10, two below), the unit glued when it starts with `/`.
 */
export function fmtEvidence(e: Evidence): string {
  const v = e.value;
  if (!Number.isFinite(v)) return "—";
  const abs = Math.abs(v);
  const digits = Number.isInteger(v) ? 0 : abs >= 100 ? 0 : abs >= 10 ? 1 : 2;
  const num = `${v < 0 ? "−" : ""}${abs.toFixed(digits)}`;
  if (!e.unit) return num;
  return e.unit.startsWith("/") ? `${num}${e.unit}` : `${num} ${e.unit}`;
}

/** `+1.1` / `−0.4` with a unit, `—` when absent. */
export function fmtSigned(v: number | null | undefined, digits = 1, unit = ""): string {
  if (v === null || v === undefined || !Number.isFinite(v)) return "—";
  const sign = v > 0 ? "+" : v < 0 ? "−" : "";
  return `${sign}${Math.abs(v).toFixed(digits)}${unit}`;
}

/* ------------------------------------------------------------------ feedback interpretation */

export type SymptomChipKind = "phase" | "speed" | "axle" | "intensity" | "wet";

export interface SymptomChip {
  kind: SymptomChipKind;
  text: string;
}

const PHASE_WORD: Record<Lang, Record<CornerPhase, string>> = {
  fr: { entry: "entrée", mid: "milieu", exit: "sortie" },
  en: { entry: "entry", mid: "mid", exit: "exit" },
};
const SPEED_WORD: Record<Lang, Record<CornerSpeed, string>> = {
  fr: { slow: "lents", medium: "moyens", fast: "rapides" },
  en: { slow: "slow", medium: "medium", fast: "fast" },
};
const AXLE_WORD: Record<Lang, Record<Axle, string>> = {
  fr: { front: "avant", rear: "arrière" },
  en: { front: "front", rear: "rear" },
};
const INTENSITY_WORD: Record<Lang, Record<SymptomIntensity, string>> = {
  fr: { mild: "un peu", marked: "beaucoup", severe: "très marqué" },
  en: { mild: "a bit", marked: "a lot", severe: "severe" },
};
const WET_WORD: Record<Lang, string> = { fr: "pluie", en: "wet" };

/** The attribute chips of a detected symptom, in a fixed order (phase, speed, axle, intensity, wet). */
export function symptomChips(s: DetectedSymptom, lang: Lang): SymptomChip[] {
  const out: SymptomChip[] = [];
  if (s.phase) out.push({ kind: "phase", text: PHASE_WORD[lang][s.phase] });
  if (s.corner_speed) out.push({ kind: "speed", text: SPEED_WORD[lang][s.corner_speed] });
  if (s.axle) out.push({ kind: "axle", text: AXLE_WORD[lang][s.axle] });
  if (s.intensity) out.push({ kind: "intensity", text: INTENSITY_WORD[lang][s.intensity] });
  if (s.wet) out.push({ kind: "wet", text: WET_WORD[lang] });
  return out;
}

export function symptomLabel(s: DetectedSymptom, lang: Lang): string {
  return lang === "fr" ? s.label_fr : s.label_en;
}

/** `85 %` */
export function fmtPct(v: number): string {
  if (!Number.isFinite(v)) return "—";
  return `${Math.round(clamp(v, 0, 1) * 100)} %`;
}

/** The questions of an interpretation in the UI language (FR list is the reference). */
export function interpretationQuestions(i: FeedbackInterpretation, lang: Lang): string[] {
  if (lang === "en" && i.questions_en.length === i.questions_fr.length) return i.questions_en;
  return i.questions_fr;
}

/* ------------------------------------------------------------------ plan iterations */

/** The iteration that answered a feedback, if any. */
export function iterationOfFeedback(
  iterations: readonly PlanIteration[],
  feedbackId: number,
): PlanIteration | null {
  return iterations.find((it) => it.feedback_id === feedbackId) ?? null;
}

/** Newest first, then by plan name for a stable order. */
export function sortIterations(iterations: readonly PlanIteration[]): PlanIteration[] {
  return [...iterations].sort(
    (a, b) => b.created_ms - a.created_ms || b.id - a.id || a.base_plan.localeCompare(b.base_plan),
  );
}

/**
 * A readable version of an iteration: the delta first (what to change now), then the agreements /
 * disagreements, the notes and the whole cumulative plan. Pure, so it is tested.
 */
export function iterationToText(
  it: PlanIteration,
  meta: Pick<AnalysisReport, "track_name" | "vehicle" | "session_id">,
  lang: Lang,
  now: Date = new Date(),
): string {
  const fr = lang === "fr";
  const lines: string[] = [];
  lines.push(
    `${fr ? "Corde — itération" : "Corde — iteration"} ${it.iteration} · ${philosophyName(it.base_plan, lang)}`,
  );
  lines.push(`${meta.track_name} · ${meta.vehicle} · ${meta.session_id}`);
  lines.push(now.toISOString().slice(0, 16).replace("T", " "));
  const interp = it.interpretation;
  if (interp?.symptoms.length) {
    lines.push("");
    lines.push(fr ? "Symptômes ressentis :" : "Symptoms felt:");
    for (const s of interp.symptoms) {
      const chips = symptomChips(s, lang)
        .map((c) => c.text)
        .join(", ");
      lines.push(
        `- ${s.negated ? (fr ? "(plus de) " : "(no more) ") : ""}${symptomLabel(s, lang)}${chips ? ` [${chips}]` : ""} — « ${s.matched_text} »`,
      );
    }
  }
  lines.push("");
  lines.push(fr ? "À changer maintenant :" : "Change now:");
  if (it.delta.length === 0) {
    lines.push(fr ? "- rien de nouveau" : "- nothing new");
  }
  it.delta.forEach((c, i) => {
    const { arrow, amount, note } = changeAmount(c, lang);
    const where = c.section ? `${c.section} / ${c.parameter_key}` : c.parameter_key;
    lines.push(
      `${i + 1}. ${c.label_fr} : ${arrow} ${amount}${note ? ` (${note})` : ""} · ${where}`,
    );
    const real = resolvedText(c, lang);
    if (real) lines.push(`   ${fr ? "dans ton réglage" : "in your setup"} : ${real}`);
    const why = fr ? c.rationale_fr : c.rationale_en;
    if (why) lines.push(`   ${fr ? "pourquoi" : "why"} : ${why}`);
  });
  if (it.agreements_fr.length) {
    lines.push("");
    lines.push(fr ? "Ressenti et données d'accord :" : "Feeling and data agree:");
    for (const a of it.agreements_fr) lines.push(`- ${a}`);
  }
  if (it.disagreements_fr.length) {
    lines.push("");
    lines.push(fr ? "Désaccords :" : "Disagreements:");
    for (const d of it.disagreements_fr) lines.push(`- ${d}`);
  }
  if (it.notes_fr.length) {
    lines.push("");
    lines.push(fr ? "Notes :" : "Notes:");
    for (const n of it.notes_fr) lines.push(`- ${n}`);
  }
  lines.push("");
  lines.push(fr ? "Plan cumulé :" : "Cumulative plan:");
  it.changes.forEach((c, i) => {
    const { arrow, amount } = changeAmount(c, lang);
    const real = resolvedText(c, lang);
    lines.push(`${i + 1}. ${c.label_fr} : ${arrow} ${amount}${real ? ` — ${real}` : ""}`);
  });
  if (it.dropped?.length) {
    lines.push("");
    lines.push(fr ? "Écartés :" : "Left out:");
    for (const d of it.dropped) {
      const why = resolvedNotes(d, lang).at(-1) ?? "";
      lines.push(`- ${d.label_fr}${why ? ` : ${why}` : ""}`);
    }
  }
  return lines.join("\n");
}
