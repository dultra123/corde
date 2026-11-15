import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import type { RacePick } from "../net/races-api";
import {
  compareText,
  KEEP_AFTER_START_MS,
  MAX_PICKS,
  MAX_REMOVED,
  mergePickSets,
  type PickSet,
  samePickSet,
  startOf,
} from "./picks-merge";

const here = dirname(fileURLToPath(import.meta.url));
/** The cases shared with the server's rule (`crates/corde-server/src/races/merge.rs`). */
const VECTORS = JSON.parse(
  readFileSync(join(here, "../../../../crates/corde-server/src/races/merge-vectors.json"), "utf8"),
) as {
  keep_after_start_ms: number;
  max_picks: number;
  max_removed: number;
  cases: { name: string; now: number; a: PickSet; b: PickSet; want: PickSet }[];
};

const NOW = 1_762_891_800_000; // 2026-11-11 20:10 UTC
const H = 3_600_000;

function pick(race: string, start: number, extra: Partial<RacePick> = {}): RacePick {
  return {
    id: `${race}@${start}`,
    race_id: race,
    cat: "daily",
    src: "rc",
    name: "LMP3 Fixed",
    level: "beginner",
    track: "Monza",
    duration: "32 min",
    image: null,
    start_ms: start,
    eve_ms: null,
    ...extra,
  };
}

const live = (race: string, start: number, changed_ms: number) => ({
  pick: pick(race, start),
  changed_ms,
});
const gone = (race: string, start: number, removed_ms: number) => ({
  id: `${race}@${start}`,
  removed_ms,
});

describe("merging two copies of « Mes courses »", () => {
  it("uses the server's limits", () => {
    expect(KEEP_AFTER_START_MS).toBe(VECTORS.keep_after_start_ms);
    expect(MAX_PICKS).toBe(VECTORS.max_picks);
    expect(MAX_REMOVED).toBe(VECTORS.max_removed);
    expect(VECTORS.cases.length).toBeGreaterThanOrEqual(10);
  });

  for (const c of VECTORS.cases) {
    it(`gives the server's result: ${c.name}`, () => {
      const ab = mergePickSets(c.a, c.b, c.now);
      expect(ab, "(a, b)").toEqual(c.want);
      expect(mergePickSets(c.b, c.a, c.now), "(b, a)").toEqual(c.want);
      expect(mergePickSets(ab, c.a, c.now), "(again, a)").toEqual(c.want);
      expect(mergePickSets(ab, c.b, c.now), "(again, b)").toEqual(c.want);
      expect(samePickSet(ab, c.want)).toBe(true);
    });
  }

  it("merges three copies the same in any order", () => {
    const a: PickSet = {
      picks: [live("one", NOW + H, 10), live("two", NOW + 2 * H, 50)],
      removed: [gone("three", NOW + 3 * H, 70)],
    };
    const b: PickSet = {
      picks: [live("three", NOW + 3 * H, 60), live("four", NOW + 4 * H, 5)],
      removed: [gone("one", NOW + H, 20)],
    };
    const c: PickSet = {
      picks: [live("one", NOW + H, 30), live("three", NOW + 3 * H, 70)],
      removed: [gone("two", NOW + 2 * H, 40), gone("four", NOW + 4 * H, 5)],
    };
    const m = (x: PickSet, y: PickSet) => mergePickSets(x, y, NOW);
    const want = m(m(a, b), c);
    expect(m(a, m(b, c))).toEqual(want);
    expect(m(m(c, a), b)).toEqual(want);
    expect(m(m(b, c), a)).toEqual(want);
    expect(want.picks.map((s) => s.pick.race_id)).toEqual(["one", "two"]);
    expect(want.removed.map((r) => r.id)).toEqual([`four@${NOW + 4 * H}`, `three@${NOW + 3 * H}`]);
  });

  it("keeps the earliest picks and the latest removals beyond the limits", () => {
    const picks = Array.from({ length: MAX_PICKS + 5 }, (_, i) =>
      live("race", NOW + H + i * 60_000, i),
    );
    const removed = Array.from({ length: MAX_REMOVED + 5 }, (_, i) =>
      gone("gone", NOW + H + i * 60_000, i),
    );
    const m = mergePickSets({ picks, removed }, { picks: [], removed: [] }, NOW);
    expect(m.picks).toHaveLength(MAX_PICKS);
    expect(m.picks[0]?.pick.start_ms).toBe(NOW + H);
    expect(m.picks.at(-1)?.pick.start_ms).toBe(NOW + H + (MAX_PICKS - 1) * 60_000);
    expect(m.removed).toHaveLength(MAX_REMOVED);
    expect(m.removed.every((r) => r.removed_ms >= 5)).toBe(true);
  });

  it("does not modify the copies it is given", () => {
    const a: PickSet = { picks: [live("one", NOW + H, 10)], removed: [] };
    const b: PickSet = { picks: [], removed: [gone("one", NOW + H, 20)] };
    const before = JSON.stringify([a, b]);
    mergePickSets(a, b, NOW);
    expect(JSON.stringify([a, b])).toBe(before);
  });

  it("reads the start of an identifier and compares texts by code point", () => {
    expect(startOf("one-stint-sprint@1762892700000")).toBe(1_762_892_700_000);
    expect(startOf("one")).toBeNull();
    expect(startOf("one@x")).toBeNull();
    expect(startOf("one@-5")).toBeNull();
    expect(compareText("a", "b")).toBe(-1);
    expect(compareText("abc", "ab")).toBe(1);
    expect(compareText("same", "same")).toBe(0);
    // An emoji (beyond the basic plane) comes after « ～ », as in UTF-8.
    expect(compareText("\u{1F3C1}", "～")).toBe(1);
    expect("\u{1F3C1}" < "～").toBe(true);
  });
});
