import type { LapRecord } from "@corde/protocol";

export interface Consumption {
  /** Average per lap over the laps used. */
  perLap: number;
  /** Number of laps the average is based on. */
  samples: number;
  /** Whole laps still possible with `remaining`. */
  lapsLeft: number;
  /** Fractional laps, for a finer display. */
  lapsLeftExact: number;
}

/** Laps that are representative of race pace: complete, not in/out laps, with a measured use. */
function usable(laps: LapRecord[], pick: (l: LapRecord) => number | null): number[] {
  const out: number[] = [];
  for (const l of laps) {
    if (l.in_lap || l.out_lap) continue;
    const v = pick(l);
    if (v === null || !Number.isFinite(v) || v <= 0) continue;
    out.push(v);
  }
  return out;
}

/**
 * Estimates how many laps `remaining` covers from the average use of the last `window` laps.
 * Returns `null` while no usable lap exists.
 */
export function estimateLaps(
  remaining: number,
  laps: LapRecord[],
  pick: (l: LapRecord) => number | null,
  window = 5,
): Consumption | null {
  const values = usable(laps, pick).slice(-window);
  if (values.length === 0 || !Number.isFinite(remaining)) return null;
  const perLap = values.reduce((a, b) => a + b, 0) / values.length;
  if (perLap <= 0) return null;
  const exact = Math.max(0, remaining / perLap);
  return { perLap, samples: values.length, lapsLeft: Math.floor(exact), lapsLeftExact: exact };
}

export const fuelLaps = (fuelL: number, laps: LapRecord[]) =>
  estimateLaps(fuelL, laps, (l) => l.fuel_used_l);

export const energyLaps = (fraction: number, laps: LapRecord[]) =>
  estimateLaps(fraction, laps, (l) => l.virtual_energy_used);
