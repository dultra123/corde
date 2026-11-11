import type { VehicleClass } from "@corde/protocol";
import type { FileDriver, FileLap, ResultLapRow, ResultRow } from "../net/profile-api";

/** Pure helpers of the profile pages (classification, gaps, lap colouring, chart series). */

/** Car class string of a results file (`GT3`, `LMP2_ELMS`, `Hypercar`) → protocol class. */
export function classKeyOf(carClass: string): VehicleClass {
  const n = carClass.trim().toLowerCase();
  if (!n) return "unknown";
  if (n.includes("hyper") || n.includes("lmh") || n.includes("lmdh")) return "hypercar";
  if (n.includes("lmp2") || n.includes("p2")) return "lmp2";
  if (n.includes("lmp3") || n.includes("p3")) return "lmp3";
  if (n.includes("gte")) return "gte";
  if (n.includes("gt3")) return "gt3";
  if (n.includes("pace") || n.includes("safety")) return "pace_car";
  return "unknown";
}

/** `LMP2_ELMS` → `LMP2 ELMS`. */
export function classLabel(carClass: string): string {
  return carClass.replaceAll("_", " ").trim() || "—";
}

export type Gap =
  | { kind: "leader" }
  | { kind: "time"; s: number }
  | { kind: "laps"; n: number }
  | { kind: "dnf"; reason: string | null }
  | { kind: "dq" }
  | { kind: "none" };

export interface StandingRow {
  driver: FileDriver;
  /** Position within the class (1-based, from the file, else by order). */
  classPos: number;
  gap: Gap;
}

/**
 * Classification of one class (or of everybody when `carClass` is `null`), best first, with the
 * gap to the (class) winner: finish-time difference on the same lap, laps down otherwise; in a
 * practice / qualifying session the gap is the best-lap difference.
 */
export function standings(
  drivers: readonly FileDriver[],
  carClass: string | null,
  race: boolean,
): StandingRow[] {
  const inClass = drivers.filter((d) => carClass === null || d.car_class === carClass);
  const posOf = (d: FileDriver) =>
    carClass === null ? d.position : (d.class_position ?? d.position);
  const sorted = [...inClass].sort((a, b) => {
    const pa = posOf(a) ?? Number.MAX_SAFE_INTEGER;
    const pb = posOf(b) ?? Number.MAX_SAFE_INTEGER;
    if (pa !== pb) return pa - pb;
    return (a.position ?? Number.MAX_SAFE_INTEGER) - (b.position ?? Number.MAX_SAFE_INTEGER);
  });
  const leader = sorted[0];
  const bestLap = Math.min(...sorted.map((d) => d.best_lap_s ?? Number.POSITIVE_INFINITY));
  return sorted.map((d, i) => {
    let gap: Gap = { kind: "none" };
    if (race) {
      if (d.finish === "dnf") gap = { kind: "dnf", reason: d.dnf_reason };
      else if (d.finish === "dq") gap = { kind: "dq" };
      else if (leader && d === leader) gap = { kind: "leader" };
      else if (leader) {
        const lapsDown = leader.laps_completed - d.laps_completed;
        if (lapsDown > 0) gap = { kind: "laps", n: lapsDown };
        else if (d.finish_time_s !== null && leader.finish_time_s !== null)
          gap = { kind: "time", s: Math.max(0, d.finish_time_s - leader.finish_time_s) };
      }
    } else if (d.best_lap_s !== null && Number.isFinite(bestLap)) {
      gap =
        d.best_lap_s <= bestLap + 1e-6
          ? { kind: "leader" }
          : { kind: "time", s: d.best_lap_s - bestLap };
    }
    return { driver: d, classPos: posOf(d) ?? i + 1, gap };
  });
}

export type LapTone = "best" | "normal" | "muted";

export interface LapTones {
  time: LapTone;
  s1: LapTone;
  s2: LapTone;
  s3: LapTone;
}

type AnyLap = Pick<ResultLapRow, "lap_time_s" | "s1" | "s2" | "s3" | "pit"> | FileLap;

/** Best lap / best sectors in purple, laps without a time (or through the pits) muted. */
export function lapTones(laps: readonly AnyLap[]): Map<AnyLap, LapTones> {
  const timed = laps.filter((l) => l.lap_time_s !== null && !l.pit);
  const min = (pick: (l: AnyLap) => number | null) => {
    let best: number | null = null;
    for (const l of timed) {
      const v = pick(l);
      if (v !== null && (best === null || v < best)) best = v;
    }
    return best;
  };
  const bestTime = min((l) => l.lap_time_s);
  const bestS1 = min((l) => l.s1);
  const bestS2 = min((l) => l.s2);
  const bestS3 = min((l) => l.s3);
  const tone = (v: number | null, best: number | null, muted: boolean): LapTone => {
    if (muted || v === null) return "muted";
    return best !== null && Math.abs(v - best) < 5e-4 ? "best" : "normal";
  };
  const out = new Map<AnyLap, LapTones>();
  for (const l of laps) {
    const muted = l.lap_time_s === null || l.pit;
    out.set(l, {
      time: tone(l.lap_time_s, bestTime, muted),
      s1: tone(l.s1, bestS1, muted && l.s1 === null),
      s2: tone(l.s2, bestS2, muted && l.s2 === null),
      s3: tone(l.s3, bestS3, muted && l.s3 === null),
    });
  }
  return out;
}

export interface BestLapPoint {
  /** Session start, Unix seconds (uPlot time scale). */
  t: number;
  bestLapS: number;
  result: ResultRow;
}

/** Best lap of every session on one track, oldest first — the progression chart's data. */
export function bestLapSeries(results: readonly ResultRow[], trackCourse: string): BestLapPoint[] {
  return results
    .filter((r) => r.track_course === trackCourse && r.player_best_lap_s !== null)
    .map((r) => ({
      t: Math.floor(r.started_wall_ms / 1000),
      bestLapS: r.player_best_lap_s as number,
      result: r,
    }))
    .sort((a, b) => a.t - b.t);
}

/** `12 / 19` style position, `—` when unknown. */
export function fmtPos(pos: number | null | undefined, total?: number | null): string {
  if (pos === null || pos === undefined || pos <= 0) return "—";
  return total ? `${pos} / ${total}` : String(pos);
}

/** Positions gained (positive) or lost from the grid; `null` without a grid. */
export function gained(grid: number | null, finish: number | null): number | null {
  if (grid === null || finish === null || grid <= 0 || finish <= 0) return null;
  return grid - finish;
}
