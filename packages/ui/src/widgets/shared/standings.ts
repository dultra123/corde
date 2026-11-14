import type { Standing } from "@corde/protocol";

/**
 * The rows of a classification around the player, shared by the Standings widget and the
 * phone dashboard (`dash/`).
 */

export interface StandingRow {
  s: Standing;
  /** Seconds to the player (relative) or to the leader (full); negative = ahead. */
  gapS: number;
  /** Whole laps of difference (same sign convention). */
  gapLaps: number;
  isClassLeaderOutsideWindow?: boolean;
}

/** 3 cars ahead and 3 behind the player (by position) plus the class leader when out of view. */
export function relativeRows(standings: Standing[], around = 3): StandingRow[] {
  const sorted = [...standings].filter((s) => !s.in_garage).sort((a, b) => a.place - b.place);
  const pi = sorted.findIndex((s) => s.is_player);
  if (pi < 0)
    return sorted
      .slice(0, around * 2 + 1)
      .map((s) => ({ s, gapS: s.time_behind_leader_s, gapLaps: s.laps_behind_leader }));
  const me = sorted[pi] as Standing;
  const rows: StandingRow[] = [];
  // cumulative intervals from the player outwards
  let acc = 0;
  const ahead: StandingRow[] = [];
  for (let i = pi; i > 0 && ahead.length < around; i -= 1) {
    const car = sorted[i - 1] as Standing;
    acc += (sorted[i] as Standing).time_behind_next_s;
    ahead.unshift({
      s: car,
      gapS: -acc,
      gapLaps: -(me.laps_behind_leader - car.laps_behind_leader),
    });
  }
  acc = 0;
  const behind: StandingRow[] = [];
  for (let i = pi + 1; i < sorted.length && behind.length < around; i += 1) {
    const car = sorted[i] as Standing;
    acc += car.time_behind_next_s;
    behind.push({ s: car, gapS: acc, gapLaps: car.laps_behind_leader - me.laps_behind_leader });
  }
  rows.push(...ahead, { s: me, gapS: 0, gapLaps: 0 }, ...behind);
  const leader = sorted.find((s) => s.class_name === me.class_name && s.class_place === 1);
  if (leader && !rows.some((r) => r.s.id === leader.id)) {
    let gap = 0;
    for (let i = sorted.indexOf(leader) + 1; i <= pi; i += 1)
      gap += (sorted[i] as Standing).time_behind_next_s;
    rows.unshift({
      s: leader,
      gapS: -gap,
      gapLaps: -(me.laps_behind_leader - leader.laps_behind_leader),
      isClassLeaderOutsideWindow: true,
    });
  }
  return rows;
}

export function fullRows(standings: Standing[]): StandingRow[] {
  return [...standings]
    .filter((s) => !s.in_garage)
    .sort((a, b) => a.place - b.place)
    .map((s) => ({ s, gapS: s.time_behind_leader_s, gapLaps: s.laps_behind_leader }));
}
