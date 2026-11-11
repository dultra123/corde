import type { VehicleClass } from "@corde/protocol";

/** Semantic colours, duplicated from tokens.css for use on canvases. */
export const COLORS = {
  cold: "#4ea1e6",
  optimal: "#37d08a",
  warm: "#f2b63c",
  hot: "#f0503c",
  faster: "#28d17c",
  slower: "#ff5a5a",
  best: "#b982ff",
  text: "#e9edf2",
  dim: "#93a0ae",
  faint: "#5d6977",
  accent: "#ffb454",
  bg: "#0b0e12",
  panel: "#131920",
  line: "#242c36",
} as const;

export const CLASS_COLORS: Record<VehicleClass, string> = {
  hypercar: "#e23b3f",
  lmp2: "#3d7fe8",
  lmp3: "#c35ad6",
  gte: "#2bb673",
  gt3: "#f29b1d",
  pace_car: "#9aa5b1",
  unknown: "#6f7b88",
};

export function classColor(c: VehicleClass): string {
  return CLASS_COLORS[c] ?? CLASS_COLORS.unknown;
}

type Rgb = [number, number, number];

function hexToRgb(hex: string): Rgb {
  const h = hex.replace("#", "");
  return [
    Number.parseInt(h.slice(0, 2), 16),
    Number.parseInt(h.slice(2, 4), 16),
    Number.parseInt(h.slice(4, 6), 16),
  ];
}

function rgbToHex([r, g, b]: Rgb): string {
  const c = (v: number) =>
    Math.round(Math.min(255, Math.max(0, v)))
      .toString(16)
      .padStart(2, "0");
  return `#${c(r)}${c(g)}${c(b)}`;
}

function lerpRgb(a: Rgb, b: Rgb, t: number): Rgb {
  return [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t];
}

export interface TempScale {
  /** Below this everything is "cold" blue. */
  cold: number;
  /** Start of the optimal (green) window. */
  optLow: number;
  /** End of the optimal window. */
  optHigh: number;
  /** Above this everything is "hot" red. */
  hot: number;
}

const C_COLD = hexToRgb(COLORS.cold);
const C_OPT = hexToRgb(COLORS.optimal);
const C_WARM = hexToRgb(COLORS.warm);
const C_HOT = hexToRgb(COLORS.hot);

/**
 * Cold blue → optimal green → warm amber → hot red.
 * The green plateau covers [optLow, optHigh]; amber sits halfway between optHigh and hot.
 */
export function tempColor(t: number, scale: TempScale): string {
  if (!Number.isFinite(t)) return COLORS.faint;
  if (t <= scale.cold) return COLORS.cold;
  if (t < scale.optLow) {
    return rgbToHex(lerpRgb(C_COLD, C_OPT, (t - scale.cold) / (scale.optLow - scale.cold)));
  }
  if (t <= scale.optHigh) return COLORS.optimal;
  const mid = (scale.optHigh + scale.hot) / 2;
  if (t < mid) return rgbToHex(lerpRgb(C_OPT, C_WARM, (t - scale.optHigh) / (mid - scale.optHigh)));
  if (t < scale.hot) return rgbToHex(lerpRgb(C_WARM, C_HOT, (t - mid) / (scale.hot - mid)));
  return COLORS.hot;
}

/** Tyre surface scale, centred on the compound's optimal temperature when known. */
export function tyreScale(optimal: number | null): TempScale {
  const opt = optimal ?? 90;
  return { cold: opt - 30, optLow: opt - 7, optHigh: opt + 7, hot: opt + 28 };
}

export const BRAKE_SCALE: TempScale = { cold: 180, optLow: 320, optHigh: 560, hot: 720 };

/** Colour for a delta: negative = faster (green), positive = slower (red). */
export function deltaColor(d: number | null | undefined): string {
  if (d === null || d === undefined || !Number.isFinite(d)) return COLORS.dim;
  if (Math.abs(d) < 0.0005) return COLORS.text;
  return d < 0 ? COLORS.faster : COLORS.slower;
}

/** Wear colour: plenty of tread → neutral, worn → amber → red. */
export function wearColor(remaining: number): string {
  if (remaining > 0.5) return COLORS.text;
  if (remaining > 0.25) return COLORS.warm;
  return COLORS.hot;
}
