import type { LapTiming } from "@corde/protocol";

/**
 * The three sectors of the lap in progress and their colours, shared by the Laps widget and
 * the phone dashboard (`dash/`).
 */

export type SectorTone = "best" | "faster" | "slower" | "running" | "none";

export interface SectorView {
  value: number | null;
  tone: SectorTone;
}

/** Colour of a completed sector: purple = personal best sector, green/red vs the best lap's sector. */
function tone(
  value: number | null,
  bestLapSector: number | null,
  bestSector: number | null,
): SectorTone {
  if (value === null) return "none";
  if (bestSector !== null && value <= bestSector + 0.0005) return "best";
  if (bestLapSector === null) return "none";
  return value < bestLapSector ? "faster" : "slower";
}

/** The three sectors of the lap in progress (running sector shown live). */
export function currentSectors(lap: LapTiming): [SectorView, SectorView, SectorView] {
  const bestLapS3 =
    lap.best_lap_s !== null && lap.best_lap_s1 !== null && lap.best_lap_s2 !== null
      ? lap.best_lap_s - lap.best_lap_s1 - lap.best_lap_s2
      : null;
  const s1: SectorView =
    lap.cur_s1 !== null
      ? { value: lap.cur_s1, tone: tone(lap.cur_s1, lap.best_lap_s1, lap.best_s1) }
      : { value: lap.current_sector === 1 ? lap.current_lap_s : null, tone: "running" };
  const s2: SectorView =
    lap.cur_s2 !== null
      ? { value: lap.cur_s2, tone: tone(lap.cur_s2, lap.best_lap_s2, lap.best_s2) }
      : {
          value:
            lap.current_sector === 2 && lap.cur_s1 !== null ? lap.current_lap_s - lap.cur_s1 : null,
          tone: "running",
        };
  const s3: SectorView =
    lap.current_sector === 3 && lap.cur_s1 !== null && lap.cur_s2 !== null
      ? { value: lap.current_lap_s - lap.cur_s1 - lap.cur_s2, tone: "running" }
      : lap.current_sector === 1 && lap.last_s3 !== null
        ? // just crossed the line: keep showing the sector 3 that was completed
          { value: lap.last_s3, tone: tone(lap.last_s3, bestLapS3, lap.best_s3) }
        : { value: null, tone: "none" };
  return [s1, s2, s3];
}
