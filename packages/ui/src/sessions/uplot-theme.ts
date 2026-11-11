import type uPlot from "uplot";
import { COLORS } from "../lib/color";

/** Shared uPlot look for the analysis pages: recessive grid, dim labels, condensed numerals. */
export const AXIS_FONT = '11px "Barlow Semi Condensed", "Barlow", system-ui, sans-serif';

export function baseAxis(): uPlot.Axis {
  return {
    stroke: COLORS.dim,
    font: AXIS_FONT,
    labelFont: AXIS_FONT,
    gap: 4,
    ticks: { show: true, stroke: COLORS.line, width: 1, size: 4 },
    grid: { show: true, stroke: COLORS.line, width: 1 },
    border: { show: false },
  };
}

/** Compact number for axis ticks and legends: `1,234` → `1234`, `0.1234` → `0.123`. */
export function fmtTick(v: number, unit = ""): string {
  if (!Number.isFinite(v)) return "";
  const abs = Math.abs(v);
  const s =
    abs >= 1000
      ? v.toFixed(0)
      : abs >= 100
        ? v.toFixed(0)
        : abs >= 10
          ? v.toFixed(1)
          : v.toFixed(2);
  const trimmed = s.includes(".") ? s.replace(/\.?0+$/, "") : s;
  return unit ? `${trimmed} ${unit}` : trimmed;
}

/** Value shown in a legend: a couple of significant decimals, unit appended. */
export function fmtValue(v: number | null | undefined, unit = ""): string {
  if (v === null || v === undefined || !Number.isFinite(v)) return "—";
  const abs = Math.abs(v);
  const s = abs >= 100 ? v.toFixed(0) : abs >= 10 ? v.toFixed(1) : v.toFixed(2);
  return unit ? `${s} ${unit}` : s;
}
