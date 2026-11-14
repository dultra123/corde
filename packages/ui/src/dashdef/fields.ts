import type { LapRecord, PlayerTelemetry, TelemetryFrame } from "@corde/protocol";
import {
  CLOSE_GAP_S,
  energyAutonomy,
  frontBias,
  fuelAutonomy,
  fuelToFinish,
  lapLimit,
  lapsLeft,
  lapsLeftTimed,
  playerDelta,
  setting,
} from "../dash/model";
import { BRAKE_SCALE, tempColor, tyreScale, wearColor } from "../lib/color";
import { fmtClock, fmtDelta, fmtGap, fmtGear, fmtLapTime, fmtTimeOfDay } from "../lib/format";
import { lapNo } from "../lib/laps";
import type { Lang } from "../store/settings";

/**
 * The data a « Valeur » or a « Barre » element can show: a closed catalogue of fields of the
 * protocol (no formula, no script). Each field says how to read it from the frame, how to write
 * it, its unit and, when it has one, its meaningful colour (faster / slower, cold / hot…). A
 * value the game does not give is `null`, shown « — ».
 */

export interface FieldCtx {
  frame: TelemetryFrame | null;
  laps: readonly LapRecord[];
}

export type FieldFormat =
  | "int"
  | "dec1"
  | "dec2"
  | "lap"
  | "sector"
  | "delta"
  | "gap"
  | "clock"
  | "time"
  | "pct"
  | "gear"
  | "place"
  | "text"
  | "onoff";

export type FieldGroup =
  | "car"
  | "inputs"
  | "timing"
  | "race"
  | "session"
  | "energy"
  | "electronics"
  | "tyres"
  | "brakes"
  | "weather";

export interface Label {
  fr: string;
  en: string;
}

export interface FieldSpec {
  id: string;
  group: FieldGroup;
  label: Label;
  /** A short label for a small cell (`TC`, `AV G`…), defaults to `label`. */
  short?: Label;
  format: FieldFormat;
  unit?: string;
  /** A fraction 0..1: a « Barre » can show it. */
  fraction?: boolean;
  read(ctx: FieldCtx): number | string | boolean | null;
  /** The meaningful colour of a value (a CSS colour), `null`: the text colour. */
  color?(v: number, ctx: FieldCtx): string | null;
}

const P = (ctx: FieldCtx): PlayerTelemetry | null => ctx.frame?.player ?? null;
const num = (v: number | null | undefined): number | null =>
  v === null || v === undefined || !Number.isFinite(v) ? null : v;

const CORNERS: { key: string; fr: string; en: string }[] = [
  { key: "fl", fr: "AV G", en: "FL" },
  { key: "fr", fr: "AV D", en: "FR" },
  { key: "rl", fr: "AR G", en: "RL" },
  { key: "rr", fr: "AR D", en: "RR" },
];

const faster = "var(--faster)";
const slower = "var(--slower)";

function levelColor(f: number): string | null {
  if (f < 0.1) return "var(--hot)";
  if (f < 0.2) return "var(--warm)";
  return null;
}

function fieldsOfCorners(): FieldSpec[] {
  const out: FieldSpec[] = [];
  CORNERS.forEach((c, i) => {
    out.push({
      id: `tyre_temp_${c.key}`,
      group: "tyres",
      label: { fr: `Pneu ${c.fr} · température`, en: `Tyre ${c.en} · temperature` },
      short: { fr: `${c.fr} °C`, en: `${c.en} °C` },
      format: "int",
      unit: "°",
      read: (ctx) => {
        const w = P(ctx)?.wheels[i];
        return w ? (w.temp_c[0] + w.temp_c[1] + w.temp_c[2]) / 3 : null;
      },
      color: (v, ctx) => tempColor(v, tyreScale(P(ctx)?.wheels[i]?.optimal_temp_c ?? null)),
    });
    out.push({
      id: `tyre_pressure_${c.key}`,
      group: "tyres",
      label: { fr: `Pneu ${c.fr} · pression`, en: `Tyre ${c.en} · pressure` },
      short: { fr: `${c.fr} kPa`, en: `${c.en} kPa` },
      format: "int",
      unit: "kPa",
      read: (ctx) => num(P(ctx)?.wheels[i]?.pressure_kpa),
    });
    out.push({
      id: `tyre_wear_${c.key}`,
      group: "tyres",
      label: { fr: `Pneu ${c.fr} · usure restante`, en: `Tyre ${c.en} · tread left` },
      short: { fr: `${c.fr} usure`, en: `${c.en} wear` },
      format: "pct",
      unit: "%",
      fraction: true,
      read: (ctx) => num(P(ctx)?.wheels[i]?.wear_remaining),
      color: (v) => wearColor(v),
    });
    out.push({
      id: `brake_temp_${c.key}`,
      group: "brakes",
      label: { fr: `Frein ${c.fr} · température`, en: `Brake ${c.en} · temperature` },
      short: { fr: `Frein ${c.fr}`, en: `Brake ${c.en}` },
      format: "int",
      unit: "°",
      read: (ctx) => num(P(ctx)?.wheels[i]?.brake_temp_c),
      color: (v) =>
        v < BRAKE_SCALE.optLow || v > BRAKE_SCALE.optHigh ? tempColor(v, BRAKE_SCALE) : null,
    });
  });
  return out;
}

export const FIELDS: readonly FieldSpec[] = [
  // ---- car
  {
    id: "speed_kmh",
    group: "car",
    label: { fr: "Vitesse", en: "Speed" },
    format: "int",
    unit: "km/h",
    read: (c) => num(P(c)?.motion.speed_kmh),
  },
  {
    id: "gear",
    group: "car",
    label: { fr: "Rapport", en: "Gear" },
    format: "gear",
    read: (c) => num(P(c)?.engine.gear),
  },
  {
    id: "rpm",
    group: "car",
    label: { fr: "Régime", en: "Revs" },
    format: "int",
    unit: "tr/min",
    read: (c) => num(P(c)?.engine.rpm),
  },
  {
    id: "rpm_fraction",
    group: "car",
    label: { fr: "Régime (part du maximum)", en: "Revs (share of maximum)" },
    short: { fr: "Régime", en: "Revs" },
    format: "pct",
    unit: "%",
    fraction: true,
    read: (c) => {
      const p = P(c);
      return p && p.engine.max_rpm > 0 ? Math.min(1, p.engine.rpm / p.engine.max_rpm) : null;
    },
    color: (v) => (v >= 0.97 ? "var(--cold)" : v >= 0.85 ? "var(--slower)" : null),
  },
  {
    id: "limiter",
    group: "car",
    label: { fr: "Limiteur des stands", en: "Pit limiter" },
    short: { fr: "Limiteur", en: "Limiter" },
    format: "onoff",
    read: (c) => P(c)?.engine.speed_limiter ?? null,
  },
  {
    id: "water_c",
    group: "car",
    label: { fr: "Eau", en: "Water" },
    format: "int",
    unit: "°",
    read: (c) => num(P(c)?.engine.water_c),
  },
  {
    id: "oil_c",
    group: "car",
    label: { fr: "Huile", en: "Oil" },
    format: "int",
    unit: "°",
    read: (c) => num(P(c)?.engine.oil_c),
  },
  // ---- inputs
  {
    id: "throttle",
    group: "inputs",
    label: { fr: "Accélérateur", en: "Throttle" },
    format: "pct",
    unit: "%",
    fraction: true,
    read: (c) => num(P(c)?.inputs.throttle),
    color: () => "var(--faster)",
  },
  {
    id: "brake",
    group: "inputs",
    label: { fr: "Frein", en: "Brake" },
    format: "pct",
    unit: "%",
    fraction: true,
    read: (c) => num(P(c)?.inputs.brake),
    color: () => "var(--slower)",
  },
  {
    id: "clutch",
    group: "inputs",
    label: { fr: "Embrayage", en: "Clutch" },
    format: "pct",
    unit: "%",
    fraction: true,
    read: (c) => num(P(c)?.inputs.clutch),
    color: () => "var(--cold)",
  },
  {
    id: "steering_deg",
    group: "inputs",
    label: { fr: "Angle du volant", en: "Steering angle" },
    short: { fr: "Volant", en: "Steering" },
    format: "int",
    unit: "°",
    read: (c) => num(P(c)?.inputs.steering_angle_deg),
  },
  {
    id: "g_lat",
    group: "inputs",
    label: { fr: "Accélération latérale", en: "Lateral g" },
    short: { fr: "G lat.", en: "Lat. g" },
    format: "dec2",
    unit: "g",
    read: (c) => num(P(c)?.motion.g_lat),
  },
  {
    id: "g_long",
    group: "inputs",
    label: { fr: "Accélération longitudinale", en: "Longitudinal g" },
    short: { fr: "G long.", en: "Long. g" },
    format: "dec2",
    unit: "g",
    read: (c) => num(P(c)?.motion.g_long),
  },
  // ---- timing
  {
    id: "lap_current",
    group: "timing",
    label: { fr: "Tour en cours", en: "Current lap" },
    format: "lap",
    read: (c) => num(P(c)?.lap.current_lap_s),
    color: (_v, c) => (P(c)?.lap.lap_invalidated ? slower : null),
  },
  {
    id: "lap_last",
    group: "timing",
    label: { fr: "Dernier tour", en: "Last lap" },
    format: "lap",
    read: (c) => num(P(c)?.lap.last_lap_s),
  },
  {
    id: "lap_best",
    group: "timing",
    label: { fr: "Meilleur tour", en: "Best lap" },
    format: "lap",
    read: (c) => num(P(c)?.lap.best_lap_s),
    color: () => "var(--best)",
  },
  {
    id: "lap_estimated",
    group: "timing",
    label: { fr: "Tour estimé", en: "Estimated lap" },
    format: "lap",
    read: (c) => {
      const v = num(P(c)?.lap.estimated_lap_s);
      return v !== null && v > 0 ? v : null;
    },
  },
  {
    id: "delta_best",
    group: "timing",
    label: { fr: "Écart au meilleur tour", en: "Delta to best lap" },
    short: { fr: "Écart", en: "Delta" },
    format: "delta",
    read: (c) => num(playerDelta(P(c))),
    color: (v) => (Math.abs(v) < 0.0005 ? null : v < 0 ? faster : slower),
  },
  {
    id: "lap_number",
    group: "timing",
    label: { fr: "Tour", en: "Lap" },
    format: "int",
    read: (c) => {
      const p = P(c);
      return p ? lapNo(p.lap.lap_number) : null;
    },
  },
  {
    id: "sector",
    group: "timing",
    label: { fr: "Secteur en cours", en: "Current sector" },
    short: { fr: "Secteur", en: "Sector" },
    format: "int",
    read: (c) => num(P(c)?.lap.current_sector),
  },
  ...([1, 2, 3] as const).flatMap((n): FieldSpec[] => [
    {
      id: `s${n}_last`,
      group: "timing",
      label: { fr: `S${n} · dernier tour`, en: `S${n} · last lap` },
      short: { fr: `S${n}`, en: `S${n}` },
      format: "sector",
      read: (c) => num(P(c)?.lap[`last_s${n}`]),
    },
    {
      id: `s${n}_best`,
      group: "timing",
      label: { fr: `S${n} · meilleur`, en: `S${n} · best` },
      short: { fr: `S${n} meill.`, en: `S${n} best` },
      format: "sector",
      read: (c) => num(P(c)?.lap[`best_s${n}`]),
      color: () => "var(--best)",
    },
  ]),
  {
    id: "lap_fraction",
    group: "timing",
    label: { fr: "Avancement du tour", en: "Lap progress" },
    format: "pct",
    unit: "%",
    fraction: true,
    read: (c) => num(P(c)?.lap.lap_fraction),
  },
  // ---- race
  {
    id: "position",
    group: "race",
    label: { fr: "Position", en: "Position" },
    format: "place",
    read: (c) => num(P(c)?.place),
  },
  {
    id: "class_position",
    group: "race",
    label: { fr: "Position dans la catégorie", en: "Class position" },
    short: { fr: "Pos. cat.", en: "Class pos." },
    format: "place",
    read: (c) => num(c.frame?.standings.find((s) => s.is_player)?.class_place),
  },
  {
    id: "cars",
    group: "race",
    label: { fr: "Voitures en piste", en: "Cars" },
    short: { fr: "Voitures", en: "Cars" },
    format: "int",
    read: (c) => num(c.frame?.session.num_vehicles),
  },
  {
    id: "gap_ahead",
    group: "race",
    label: { fr: "Écart devant", en: "Gap ahead" },
    short: { fr: "Devant", en: "Ahead" },
    format: "gap",
    read: (c) => num(P(c)?.gaps.place_ahead_s),
    color: (v) => (Math.abs(v) < CLOSE_GAP_S ? faster : null),
  },
  {
    id: "gap_behind",
    group: "race",
    label: { fr: "Écart derrière", en: "Gap behind" },
    short: { fr: "Derrière", en: "Behind" },
    format: "gap",
    read: (c) => num(P(c)?.gaps.place_behind_s),
    color: (v) => (Math.abs(v) < CLOSE_GAP_S ? slower : null),
  },
  {
    id: "gap_leader",
    group: "race",
    label: { fr: "Écart au premier", en: "Gap to leader" },
    short: { fr: "Au premier", en: "To leader" },
    format: "gap",
    read: (c) => {
      const me = c.frame?.standings.find((s) => s.is_player);
      return me && me.laps_behind_leader === 0 ? num(me.time_behind_leader_s) : null;
    },
  },
  {
    id: "pit_stops",
    group: "race",
    label: { fr: "Arrêts effectués", en: "Pit stops made" },
    short: { fr: "Arrêts", en: "Stops" },
    format: "int",
    read: (c) => num(c.frame?.standings.find((s) => s.is_player)?.num_pitstops),
  },
  // ---- session
  {
    id: "session_left",
    group: "session",
    label: { fr: "Temps restant", en: "Time left" },
    format: "clock",
    read: (c) => {
      const s = c.frame?.session;
      return s && s.end_s > 0 ? Math.max(0, s.time_remaining_s) : null;
    },
  },
  {
    id: "session_elapsed",
    group: "session",
    label: { fr: "Temps écoulé", en: "Time elapsed" },
    format: "clock",
    read: (c) => num(c.frame?.session.elapsed_s),
  },
  {
    id: "laps_left",
    group: "session",
    label: { fr: "Tours restants", en: "Laps to go" },
    format: "int",
    read: (c) => {
      const s = c.frame?.session ?? null;
      const p = P(c);
      return lapLimit(s) !== null ? lapsLeft(s, p) : lapsLeftTimed(s, [...c.laps]);
    },
  },
  {
    id: "time_of_day",
    group: "session",
    label: { fr: "Heure en jeu", en: "Time of day (game)" },
    short: { fr: "Heure", en: "Time" },
    format: "time",
    read: (c) => num(c.frame?.session.weather.time_of_day_s),
  },
  {
    id: "track_name",
    group: "session",
    label: { fr: "Circuit", en: "Track" },
    format: "text",
    read: (c) => c.frame?.session.track_name || null,
  },
  // ---- energy
  {
    id: "fuel_l",
    group: "energy",
    label: { fr: "Carburant", en: "Fuel" },
    format: "dec1",
    unit: "L",
    read: (c) => num(P(c)?.energy.fuel_l),
    color: (_v, c) => levelColor(P(c)?.energy.fuel_fraction ?? 1),
  },
  {
    id: "fuel_fraction",
    group: "energy",
    label: { fr: "Réservoir", en: "Tank" },
    format: "pct",
    unit: "%",
    fraction: true,
    read: (c) => num(P(c)?.energy.fuel_fraction),
    color: (v) => levelColor(v),
  },
  {
    id: "fuel_per_lap",
    group: "energy",
    label: { fr: "Carburant par tour", en: "Fuel per lap" },
    short: { fr: "L / tour", en: "L / lap" },
    format: "dec2",
    unit: "L",
    read: (c) => {
      const p = P(c);
      return p ? (fuelAutonomy(p.energy, [...c.laps])?.perLap ?? null) : null;
    },
  },
  {
    id: "fuel_laps",
    group: "energy",
    label: { fr: "Autonomie carburant", en: "Fuel range" },
    short: { fr: "Autonomie", en: "Range" },
    format: "dec1",
    unit: "tours",
    read: (c) => {
      const p = P(c);
      return p ? (fuelAutonomy(p.energy, [...c.laps])?.laps ?? null) : null;
    },
  },
  {
    id: "fuel_to_finish",
    group: "energy",
    label: { fr: "Carburant manquant pour finir", en: "Fuel missing to finish" },
    short: { fr: "Manque", en: "Missing" },
    format: "dec1",
    unit: "L",
    read: (c) => {
      const p = P(c);
      const s = c.frame?.session ?? null;
      if (!p) return null;
      const per = fuelAutonomy(p.energy, [...c.laps])?.perLap ?? null;
      const go = lapLimit(s) !== null ? lapsLeft(s, p) : lapsLeftTimed(s, [...c.laps]);
      return fuelToFinish(p.energy.fuel_l, per, go);
    },
    color: (v) => (v > 0 ? "var(--warm)" : faster),
  },
  {
    id: "virtual_energy",
    group: "energy",
    label: { fr: "Énergie virtuelle", en: "Virtual energy" },
    short: { fr: "Énergie", en: "Energy" },
    format: "pct",
    unit: "%",
    fraction: true,
    read: (c) => num(P(c)?.energy.virtual_energy),
    color: (v) => levelColor(v) ?? "var(--cold)",
  },
  {
    id: "energy_per_lap",
    group: "energy",
    label: { fr: "Énergie par tour", en: "Energy per lap" },
    short: { fr: "% / tour", en: "% / lap" },
    format: "pct",
    unit: "%",
    read: (c) => {
      const p = P(c);
      return p ? (energyAutonomy(p.energy, [...c.laps])?.perLap ?? null) : null;
    },
  },
  {
    id: "energy_laps",
    group: "energy",
    label: { fr: "Autonomie énergie", en: "Energy range" },
    short: { fr: "Autonomie", en: "Range" },
    format: "dec1",
    unit: "tours",
    read: (c) => {
      const p = P(c);
      return p ? (energyAutonomy(p.energy, [...c.laps])?.laps ?? null) : null;
    },
  },
  {
    id: "battery",
    group: "energy",
    label: { fr: "Batterie", en: "Battery" },
    format: "pct",
    unit: "%",
    fraction: true,
    read: (c) => num(P(c)?.energy.battery_soc),
    color: () => "var(--best)",
  },
  {
    id: "regen_kw",
    group: "energy",
    label: { fr: "Régénération", en: "Regeneration" },
    short: { fr: "Régén.", en: "Regen" },
    format: "int",
    unit: "kW",
    read: (c) => num(P(c)?.energy.regen_kw),
  },
  {
    id: "stops_planned",
    group: "energy",
    label: { fr: "Arrêts prévus", en: "Stops planned" },
    format: "int",
    read: (c) => num(P(c)?.energy.scheduled_stops),
  },
  // ---- electronics
  {
    id: "tc",
    group: "electronics",
    label: { fr: "Antipatinage (TC)", en: "Traction control (TC)" },
    short: { fr: "TC", en: "TC" },
    format: "int",
    read: (c) => {
      const e = P(c)?.electronics;
      return e ? setting(e.tc, e.tc_max) : null;
    },
    color: (_v, c) => (P(c)?.electronics.tc_active ? "var(--warm)" : null),
  },
  {
    id: "abs",
    group: "electronics",
    label: { fr: "ABS", en: "ABS" },
    format: "int",
    read: (c) => {
      const e = P(c)?.electronics;
      return e ? setting(e.abs, e.abs_max) : null;
    },
    color: (_v, c) => (P(c)?.electronics.abs_active ? "var(--warm)" : null),
  },
  {
    id: "motor_map",
    group: "electronics",
    label: { fr: "Cartographie moteur", en: "Engine map" },
    short: { fr: "Carto", en: "Map" },
    format: "int",
    read: (c) => {
      const e = P(c)?.electronics;
      return e ? setting(e.motor_map, e.motor_map_max) : null;
    },
  },
  {
    id: "brake_bias",
    group: "electronics",
    label: { fr: "Répartition de freinage (avant)", en: "Brake bias (front)" },
    short: { fr: "Freins AV", en: "Bias F" },
    format: "dec1",
    unit: "%",
    read: (c) => {
      const e = P(c)?.electronics;
      return e ? frontBias(e) : null;
    },
  },
  {
    id: "brake_migration",
    group: "electronics",
    label: { fr: "Migration de freinage", en: "Brake migration" },
    short: { fr: "Migration", en: "Migration" },
    format: "int",
    read: (c) => {
      const e = P(c)?.electronics;
      return e ? setting(e.brake_migration, e.brake_migration_max) : null;
    },
  },
  // ---- tyres and brakes
  ...fieldsOfCorners(),
  // ---- weather
  {
    id: "track_c",
    group: "weather",
    label: { fr: "Piste", en: "Track" },
    format: "int",
    unit: "°",
    read: (c) => num(c.frame?.session.weather.track_c),
  },
  {
    id: "air_c",
    group: "weather",
    label: { fr: "Air", en: "Air" },
    format: "int",
    unit: "°",
    read: (c) => num(c.frame?.session.weather.ambient_c),
  },
  {
    id: "rain",
    group: "weather",
    label: { fr: "Pluie", en: "Rain" },
    format: "pct",
    unit: "%",
    fraction: true,
    read: (c) => num(c.frame?.session.weather.rain),
    color: () => "var(--cold)",
  },
  {
    id: "wetness",
    group: "weather",
    label: { fr: "Piste mouillée", en: "Track wetness" },
    short: { fr: "Humidité", en: "Wetness" },
    format: "pct",
    unit: "%",
    fraction: true,
    read: (c) => num(c.frame?.session.weather.wetness_avg),
    color: () => "var(--cold)",
  },
];

const BY_ID = new Map(FIELDS.map((f) => [f.id, f]));

export function getField(id: unknown): FieldSpec | undefined {
  return typeof id === "string" ? BY_ID.get(id) : undefined;
}

export const FIELD_GROUPS: Record<FieldGroup, Label> = {
  car: { fr: "Voiture", en: "Car" },
  inputs: { fr: "Commandes", en: "Inputs" },
  timing: { fr: "Chronos", en: "Timing" },
  race: { fr: "Course", en: "Race" },
  session: { fr: "Session", en: "Session" },
  energy: { fr: "Carburant & énergie", en: "Fuel & energy" },
  electronics: { fr: "Électronique", en: "Electronics" },
  tyres: { fr: "Pneus", en: "Tyres" },
  brakes: { fr: "Freins", en: "Brakes" },
  weather: { fr: "Météo", en: "Weather" },
};

export function label(l: Label, lang: Lang): string {
  return lang === "en" ? l.en : l.fr;
}

/** The unit as shown (`tours` / `laps`). */
export function unitOf(f: FieldSpec, lang: Lang): string {
  if (f.unit === "tours") return lang === "en" ? "laps" : "tours";
  return f.unit ?? "";
}

function decimalsOf(fmt: FieldFormat): number {
  return fmt === "dec1" ? 1 : fmt === "dec2" ? 2 : 0;
}

/**
 * The text of a value. `decimals` (0..3) overrides the field's own for plain numbers and
 * percentages; `null` (or a negative number): the field's own.
 */
export function formatValue(
  f: FieldSpec,
  v: number | string | boolean | null,
  decimals: number | null = null,
  lang: Lang = "fr",
): string {
  if (v === null || v === undefined) return "—";
  if (typeof v === "string") return v;
  if (typeof v === "boolean") return v ? (lang === "en" ? "ON" : "OUI") : "—";
  if (!Number.isFinite(v)) return "—";
  const d = decimals !== null && decimals >= 0 ? Math.min(3, Math.round(decimals)) : null;
  switch (f.format) {
    case "lap":
      return fmtLapTime(v, { minutes: true });
    case "sector":
      return fmtLapTime(v, { ms: 3 });
    case "delta":
      return fmtDelta(v, d ?? 3);
    case "gap":
      return `${v < 0 ? "−" : f.id === "gap_ahead" ? "−" : "+"}${fmtGap(Math.abs(v), d ?? 1)}`;
    case "clock":
      return fmtClock(v);
    case "time":
      return fmtTimeOfDay(v);
    case "gear":
      return fmtGear(v);
    case "place":
      return `P${Math.round(v)}`;
    case "pct":
      return (v * 100).toFixed(d ?? 0);
    case "onoff":
      return v ? "ON" : "—";
    default:
      return v.toFixed(d ?? decimalsOf(f.format));
  }
}
