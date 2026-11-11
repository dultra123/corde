/** Session-level statistics and lap-table colouring, pure functions over `LapRecord`-like rows. */

export interface LapLike {
  lap_number: number;
  lap_time_s: number;
  s1: number | null;
  s2: number | null;
  s3: number | null;
  valid: boolean;
  in_lap: boolean;
  out_lap: boolean;
  fuel_used_l: number;
  top_speed_kmh: number;
}

/** A lap that counts for bests and averages: valid, not an in/out lap, with a real time. */
export function isCounted(l: LapLike): boolean {
  return l.valid && !l.in_lap && !l.out_lap && Number.isFinite(l.lap_time_s) && l.lap_time_s > 0;
}

export interface LapStats {
  /** Fastest counted lap. */
  best: LapLike | null;
  /** Mean of the counted lap times, seconds. */
  mean: number | null;
  /** Population standard deviation of the counted lap times, seconds (needs ≥ 2 laps). */
  stddev: number | null;
  /** Mean fuel used per lap over laps that consumed something (counted laps first). */
  fuelPerLap: number | null;
  /** Highest top speed over every lap. */
  topSpeed: number | null;
  counted: number;
  total: number;
  /** Best sector times among counted laps. */
  bestSectors: [number | null, number | null, number | null];
}

const minOf = (vals: number[]): number | null => (vals.length ? Math.min(...vals) : null);

export function lapStats(laps: readonly LapLike[]): LapStats {
  const counted = laps.filter(isCounted);
  const times = counted.map((l) => l.lap_time_s);
  const mean = times.length ? times.reduce((a, b) => a + b, 0) / times.length : null;
  let stddev: number | null = null;
  if (mean !== null && times.length >= 2) {
    const v = times.reduce((acc, t) => acc + (t - mean) ** 2, 0) / times.length;
    stddev = Math.sqrt(v);
  }
  let best: LapLike | null = null;
  for (const l of counted) if (!best || l.lap_time_s < best.lap_time_s) best = l;

  const fuelSource = (counted.length ? counted : laps).filter(
    (l) => Number.isFinite(l.fuel_used_l) && l.fuel_used_l > 0,
  );
  const fuelPerLap = fuelSource.length
    ? fuelSource.reduce((a, l) => a + l.fuel_used_l, 0) / fuelSource.length
    : null;

  const speeds = laps.map((l) => l.top_speed_kmh).filter((v) => Number.isFinite(v) && v > 0);
  const topSpeed = speeds.length ? Math.max(...speeds) : null;

  const sector = (k: "s1" | "s2" | "s3") =>
    minOf(counted.map((l) => l[k]).filter((v): v is number => v !== null && v > 0));

  return {
    best,
    mean,
    stddev,
    fuelPerLap,
    topSpeed,
    counted: counted.length,
    total: laps.length,
    bestSectors: [sector("s1"), sector("s2"), sector("s3")],
  };
}

/**
 * Colour of a time in the lap table:
 * - `best`: the session's best (purple),
 * - `pb`: faster than every counted lap before it (green),
 * - `normal`: a counted lap that improves nothing,
 * - `muted`: invalid / in-lap / out-lap (grey).
 */
export type Tone = "best" | "pb" | "normal" | "muted";

export interface LapTones {
  time: Tone;
  s1: Tone;
  s2: Tone;
  s3: Tone;
}

const EPS = 0.0005;

/** Tones per lap number, computed in chronological order (lap numbers ascending). */
export function lapTones(laps: readonly LapLike[]): Map<number, LapTones> {
  const ordered = [...laps].sort((a, b) => a.lap_number - b.lap_number);
  const stats = lapStats(ordered);
  const out = new Map<number, LapTones>();
  let runBest = Number.POSITIVE_INFINITY;
  const runSector = [Number.POSITIVE_INFINITY, Number.POSITIVE_INFINITY, Number.POSITIVE_INFINITY];
  const sessionBest = stats.best?.lap_time_s ?? null;

  const toneOf = (v: number | null, sessionBestV: number | null, running: number): Tone => {
    if (v === null || !(v > 0)) return "muted";
    if (sessionBestV !== null && Math.abs(v - sessionBestV) < EPS) return "best";
    if (v < running - EPS) return "pb";
    return "normal";
  };

  for (const l of ordered) {
    if (!isCounted(l)) {
      out.set(l.lap_number, { time: "muted", s1: "muted", s2: "muted", s3: "muted" });
      continue;
    }
    const sectors = [l.s1, l.s2, l.s3];
    const tones: LapTones = {
      time: toneOf(l.lap_time_s, sessionBest, runBest),
      s1: toneOf(sectors[0] ?? null, stats.bestSectors[0], runSector[0] ?? 0),
      s2: toneOf(sectors[1] ?? null, stats.bestSectors[1], runSector[1] ?? 0),
      s3: toneOf(sectors[2] ?? null, stats.bestSectors[2], runSector[2] ?? 0),
    };
    out.set(l.lap_number, tones);
    runBest = Math.min(runBest, l.lap_time_s);
    sectors.forEach((s, i) => {
      if (s !== null && s > 0) runSector[i] = Math.min(runSector[i] ?? 0, s);
    });
  }
  return out;
}

/** Elapsed seconds between two wall-clock stamps, `null` while the session is still open. */
export function sessionDurationS(startedMs: number, endedMs: number | null, nowMs = Date.now()) {
  const end = endedMs ?? nowMs;
  return Math.max(0, (end - startedMs) / 1000);
}
