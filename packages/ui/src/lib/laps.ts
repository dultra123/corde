import type { LapNote, LapRecord } from "@corde/protocol";

/**
 * Lap number as the game shows it. Recorded laps are 0-based (the number of
 * laps completed when the lap started); the game — its HUD, its result files —
 * counts from 1.
 */
export function lapNo(lapNumber: number): number {
  return lapNumber + 1;
}

/** What the status column says about a lap. */
export type LapStatus = "counted" | LapNote | "invalid";

type LapStatusInput = Pick<LapRecord, "valid" | "in_lap" | "out_lap"> &
  Partial<Pick<LapRecord, "note" | "timed_by_game">>;

/**
 * Why a lap counts or not. Rows written before notes existed only carry the
 * `valid` / `in_lap` / `out_lap` flags.
 */
export function lapStatus(lap: LapStatusInput): LapStatus {
  if (lap.note) return lap.note;
  if (lap.out_lap) return "out_lap";
  if (lap.in_lap) return "in_lap";
  if (lap.timed_by_game === false) return "not_timed";
  return lap.valid ? "counted" : "invalid";
}

/** i18n key of the short label of a lap status. */
export function lapStatusKey(status: LapStatus): `lapStatus.${LapStatus}` {
  return `lapStatus.${status}`;
}

/** i18n key of the sentence explaining a lap status. */
export function lapStatusHintKey(status: LapStatus): `lapStatus.hint.${LapStatus}` {
  return `lapStatus.hint.${status}`;
}
