import type { TranslationKey } from "../i18n";
import { COLORS } from "../lib/color";
import type { ChannelInfo } from "../net/sessions-api";
import type { Lang } from "../store/settings";

/**
 * Client-side knowledge about the recorder's channels (`GET /api/channels` gives names, units
 * and physical kinds; the labels, categories and chart presets live here).
 */

export const WHEEL_SUFFIXES = ["_fl", "_fr", "_rl", "_rr"] as const;

/** Short wheel labels per language (FL/FR/RL/RR in English, AVG/AVD/ARG/ARD in French). */
export const WHEEL_LABELS: Record<Lang, [string, string, string, string]> = {
  fr: ["AVG", "AVD", "ARG", "ARD"],
  en: ["FL", "FR", "RL", "RR"],
};

/** One colour per wheel, fixed order, reused by every per-wheel chart. */
export const WHEEL_COLORS: [string, string, string, string] = [
  COLORS.cold,
  COLORS.accent,
  COLORS.best,
  COLORS.optimal,
];

/** Categorical palette for custom panels (fixed order, never cycled past the end). */
export const SERIES_COLORS = [
  COLORS.cold,
  COLORS.accent,
  COLORS.best,
  COLORS.optimal,
  COLORS.hot,
  "#e9edf2",
  "#9ad1ff",
  "#ff8f8f",
] as const;

export type Category =
  | "speed"
  | "inputs"
  | "motion"
  | "energy"
  | "electronics"
  | "aero"
  | "track"
  | "tyres"
  | "brakes"
  | "susp"
  | "other";

export const CATEGORY_ORDER: Category[] = [
  "speed",
  "inputs",
  "motion",
  "energy",
  "electronics",
  "aero",
  "track",
  "tyres",
  "brakes",
  "susp",
  "other",
];

export const categoryKey = (c: Category): TranslationKey => `cat.${c}` as TranslationKey;

/** Axis / bookkeeping columns that make no sense as a plotted channel. */
export const AXIS_CHANNELS = new Set([
  "t_s",
  "wall_ms",
  "lap_number",
  "lap_dist_m",
  "lap_fraction",
]);

const LABELS: Record<string, [fr: string, en: string]> = {
  speed_kmh: ["Vitesse", "Speed"],
  rpm: ["Régime", "RPM"],
  gear: ["Rapport", "Gear"],
  throttle: ["Gaz", "Throttle"],
  brake: ["Frein", "Brake"],
  steering: ["Volant", "Steering"],
  clutch: ["Embrayage", "Clutch"],
  throttle_raw: ["Gaz (pédale)", "Throttle (pedal)"],
  brake_raw: ["Frein (pédale)", "Brake (pedal)"],
  steering_raw: ["Volant (brut)", "Steering (raw)"],
  steering_angle_deg: ["Angle volant", "Steering angle"],
  g_lat: ["G latéral", "Lateral G"],
  g_long: ["G longitudinal", "Longitudinal G"],
  g_vert: ["G vertical", "Vertical G"],
  yaw_rate_dps: ["Vitesse de lacet", "Yaw rate"],
  pos_x: ["Position X", "Position X"],
  pos_y: ["Position Y", "Position Y"],
  pos_z: ["Position Z", "Position Z"],
  heading_rad: ["Cap", "Heading"],
  fuel_l: ["Carburant", "Fuel"],
  virtual_energy: ["Énergie virtuelle", "Virtual energy"],
  battery_soc: ["Batterie", "Battery"],
  regen_kw: ["Régénération", "Regen"],
  brake_bias_rear: ["Répartition frein (AR)", "Brake bias (rear)"],
  tc: ["TC", "TC"],
  abs: ["ABS", "ABS"],
  motor_map: ["Map moteur", "Engine map"],
  tc_active: ["TC actif", "TC active"],
  abs_active: ["ABS actif", "ABS active"],
  water_c: ["Eau", "Water"],
  oil_c: ["Huile", "Oil"],
  front_ride_height_mm: ["Hauteur AV", "Front ride height"],
  rear_ride_height_mm: ["Hauteur AR", "Rear ride height"],
  front_downforce_n: ["Appui AV", "Front downforce"],
  rear_downforce_n: ["Appui AR", "Rear downforce"],
  drag_n: ["Traînée", "Drag"],
  third_front_mm: ["3e élément AV", "Front third spring"],
  third_rear_mm: ["3e élément AR", "Rear third spring"],
  in_pits: ["Aux stands", "In pits"],
  in_pit_lane: ["Voie des stands", "Pit lane"],
  current_sector: ["Secteur", "Sector"],
  lap_invalidated: ["Tour invalidé", "Lap invalidated"],
  track_c: ["T° piste", "Track temp"],
  ambient_c: ["T° air", "Air temp"],
  rain: ["Pluie", "Rain"],
  wetness_avg: ["Humidité piste", "Track wetness"],
  susp_deflection_m: ["Débattement", "Suspension travel"],
  ride_height_m: ["Hauteur de caisse", "Ride height"],
  susp_force_n: ["Force suspension", "Suspension force"],
  brake_temp_c: ["T° frein", "Brake temp"],
  brake_pressure: ["Pression frein", "Brake pressure"],
  rotation_rads: ["Rotation roue", "Wheel rotation"],
  camber_deg: ["Carrossage", "Camber"],
  toe_deg: ["Pincement", "Toe"],
  lateral_patch_vel: ["Vit. latérale (contact)", "Lateral patch velocity"],
  longitudinal_patch_vel: ["Vit. longitudinale (contact)", "Longitudinal patch velocity"],
  lateral_ground_vel: ["Vit. latérale (sol)", "Lateral ground velocity"],
  longitudinal_ground_vel: ["Vit. longitudinale (sol)", "Longitudinal ground velocity"],
  lateral_force_n: ["Force latérale", "Lateral force"],
  longitudinal_force_n: ["Force longitudinale", "Longitudinal force"],
  load_n: ["Charge", "Load"],
  grip_fract: ["Grip", "Grip"],
  pressure_kpa: ["Pression", "Pressure"],
  temp_l_c: ["T° gauche", "Temp left"],
  temp_c_c: ["T° centre", "Temp centre"],
  temp_r_c: ["T° droite", "Temp right"],
  temp_inner_l_c: ["T° interne gauche", "Inner temp left"],
  temp_inner_c_c: ["T° interne centre", "Inner temp centre"],
  temp_inner_r_c: ["T° interne droite", "Inner temp right"],
  carcass_c: ["T° carcasse", "Carcass temp"],
  optimal_temp_c: ["T° optimale du pneu", "Optimal tyre temp"],
  wear_remaining: ["Usure restante", "Wear remaining"],
  slip_ratio: ["Slip ratio", "Slip ratio"],
  slip_angle_deg: ["Angle de dérive", "Slip angle"],
  surface: ["Surface", "Surface"],
};

const CATEGORY_OF: Record<string, Category> = {
  speed_kmh: "speed",
  rpm: "speed",
  gear: "speed",
  water_c: "speed",
  oil_c: "speed",
  throttle: "inputs",
  brake: "inputs",
  steering: "inputs",
  clutch: "inputs",
  throttle_raw: "inputs",
  brake_raw: "inputs",
  steering_raw: "inputs",
  steering_angle_deg: "inputs",
  g_lat: "motion",
  g_long: "motion",
  g_vert: "motion",
  yaw_rate_dps: "motion",
  pos_x: "motion",
  pos_y: "motion",
  pos_z: "motion",
  heading_rad: "motion",
  fuel_l: "energy",
  virtual_energy: "energy",
  battery_soc: "energy",
  regen_kw: "energy",
  brake_bias_rear: "electronics",
  tc: "electronics",
  abs: "electronics",
  motor_map: "electronics",
  tc_active: "electronics",
  abs_active: "electronics",
  front_ride_height_mm: "aero",
  rear_ride_height_mm: "aero",
  front_downforce_n: "aero",
  rear_downforce_n: "aero",
  drag_n: "aero",
  third_front_mm: "aero",
  third_rear_mm: "aero",
  in_pits: "track",
  in_pit_lane: "track",
  current_sector: "track",
  lap_invalidated: "track",
  track_c: "track",
  ambient_c: "track",
  rain: "track",
  wetness_avg: "track",
  susp_deflection_m: "susp",
  ride_height_m: "susp",
  susp_force_n: "susp",
  brake_temp_c: "brakes",
  brake_pressure: "brakes",
};

/** `temp_c_c_fl` → `{ base: "temp_c_c", wheel: 0 }`; `null` for scalar channels. */
export function splitWheel(name: string): { base: string; wheel: 0 | 1 | 2 | 3 } | null {
  for (let i = 0; i < WHEEL_SUFFIXES.length; i += 1) {
    const suffix = WHEEL_SUFFIXES[i] as string;
    if (name.endsWith(suffix)) {
      return { base: name.slice(0, -suffix.length), wheel: i as 0 | 1 | 2 | 3 };
    }
  }
  return null;
}

/** Human label of a channel (per-wheel channels get the wheel appended). */
export function channelLabel(name: string, lang: Lang): string {
  const w = splitWheel(name);
  const base = w ? w.base : name;
  const entry = LABELS[base];
  const label = entry ? entry[lang === "fr" ? 0 : 1] : base;
  return w ? `${label} ${WHEEL_LABELS[lang][w.wheel]}` : label;
}

/** Label of a per-wheel family without the wheel (`temp_c_c` → "T° centre"). */
export function baseLabel(base: string, lang: Lang): string {
  const entry = LABELS[base];
  return entry ? entry[lang === "fr" ? 0 : 1] : base;
}

export function categoryOf(name: string): Category {
  const w = splitWheel(name);
  if (w) return CATEGORY_OF[w.base] ?? "tyres";
  return CATEGORY_OF[name] ?? "other";
}

/** A pickable entry of the channel selector: a scalar channel or a per-wheel family. */
export interface PickableChannel {
  /** Scalar name, or the family base name (`temp_c_c`). */
  key: string;
  label: string;
  unit: string;
  category: Category;
  /** The concrete channel names this entry adds (1 or 4). */
  channels: string[];
  perWheel: boolean;
}

/** Groups the server's channel list into pickable entries, per-wheel families folded into one. */
export function pickableChannels(list: ChannelInfo[], lang: Lang): PickableChannel[] {
  const out: PickableChannel[] = [];
  const families = new Map<string, PickableChannel>();
  for (const ch of list) {
    if (AXIS_CHANNELS.has(ch.name)) continue;
    const w = splitWheel(ch.name);
    if (!w) {
      out.push({
        key: ch.name,
        label: channelLabel(ch.name, lang),
        unit: ch.unit,
        category: categoryOf(ch.name),
        channels: [ch.name],
        perWheel: false,
      });
      continue;
    }
    let fam = families.get(w.base);
    if (!fam) {
      fam = {
        key: w.base,
        label: baseLabel(w.base, lang),
        unit: ch.unit,
        category: categoryOf(ch.name),
        channels: [],
        perWheel: true,
      };
      families.set(w.base, fam);
      out.push(fam);
    }
    fam.channels.push(ch.name);
  }
  return out;
}

/** Unit shown for a channel on an axis (ride heights in metres are shown in millimetres). */
export function displayUnit(name: string, unit: string): string {
  if (splitWheel(name)?.base === "ride_height_m" || splitWheel(name)?.base === "susp_deflection_m")
    return "mm";
  if (name === "throttle" || name === "brake" || name === "clutch") return "%";
  return unit;
}

/** Scale factor applied before plotting (matches `displayUnit`). */
export function displayFactor(name: string): number {
  const base = splitWheel(name)?.base;
  if (base === "ride_height_m" || base === "susp_deflection_m") return 1000;
  if (name === "throttle" || name === "brake" || name === "clutch") return 100;
  return 1;
}

/** Channels whose values are integers / states: drawn as steps and interpolated by holding. */
export function isStepChannel(name: string): boolean {
  const base = splitWheel(name)?.base ?? name;
  return (
    base === "gear" ||
    base === "tc" ||
    base === "abs" ||
    base === "motor_map" ||
    base === "tc_active" ||
    base === "abs_active" ||
    base === "in_pits" ||
    base === "in_pit_lane" ||
    base === "current_sector" ||
    base === "lap_invalidated" ||
    base === "surface"
  );
}
