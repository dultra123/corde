import { describe, expect, it } from "vitest";
import type { RacePick } from "../net/races-api";
import { ApiError } from "../net/sessions-api";
import { FakePrefs } from "./doubles";
import { KEYS } from "./kv";
import {
  applySettingsPatch,
  defaultRaceSettings,
  MAX_PICKS,
  RaceBook,
  validatePick,
} from "./race-book";

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

describe("the races kept on the phone", () => {
  it("validates picks like the server", () => {
    expect(() => validatePick(pick("ok-race", NOW + H))).not.toThrow();
    for (const bad of [
      pick("Bad Race", NOW),
      { ...pick("race", NOW), id: "race@1" },
      pick("race", NOW, { cat: "other" as never }),
      pick("race", NOW, { src: "xx" as never }),
      pick("race", NOW, { level: "pro" as never }),
      pick("race", NOW, { name: "" }),
      pick("race", NOW, { name: "x".repeat(121) }),
      pick("race", NOW, { track: "a\u0007b" }),
      pick("race", NOW, { image: "http://img.example/a.png" }),
      pick("race", NOW, { image: 'https://x/"onerror' }),
      pick("race", NOW, { eve_ms: NOW - H }),
      pick("race", NOW, { cat: "special", eve_ms: NOW + 1 }),
      pick("race", 1_000),
      { ...pick("race", NOW), extra: 1 },
    ]) {
      expect(() => validatePick(bad), JSON.stringify(bad)).toThrow();
    }
  });

  it("keeps at most 50 picks (409) and drops them 3 h after their start", async () => {
    const prefs = new FakePrefs();
    let now = NOW;
    const book = new RaceBook(prefs, () => now);
    for (let i = 0; i < MAX_PICKS; i += 1) await book.put(pick(`r${i}`, NOW + i * 60_000));
    await expect(book.put(pick("one-more", NOW))).rejects.toMatchObject({ status: 409 });
    // Replacing an existing one is fine.
    await expect(book.put(pick("r0", NOW))).resolves.toBeTruthy();
    await expect(book.put(pick("Bad", NOW))).rejects.toBeInstanceOf(ApiError);
    expect(await book.list()).toHaveLength(50);
    now = NOW + 3 * H + 30 * 60_000;
    expect((await book.list()).length).toBeLessThan(50);
    // Persisted, read back by a new book (a new start of the application).
    const again = new RaceBook(prefs, () => now);
    expect(await again.list()).toEqual(await book.list());
  });

  it("removes a pick, and ignores broken storage", async () => {
    const prefs = new FakePrefs();
    prefs.data.set(KEYS.picks, JSON.stringify([pick("good", NOW + H), { id: "x" }, 3]));
    prefs.data.set(KEYS.raceSettings, "{not json");
    const book = new RaceBook(prefs, () => NOW);
    expect((await book.list()).map((p) => p.id)).toEqual([`good@${NOW + H}`]);
    expect(await book.getSettings()).toEqual(defaultRaceSettings());
    expect(await book.remove(`good@${NOW + H}`)).toBe(true);
    expect(await book.remove("nothing@1")).toBe(false);
    expect(await book.list()).toEqual([]);
  });

  it("applies strict settings patches, and the hour correction moves racecontrol.gg daily picks", async () => {
    const s = applySettingsPatch(defaultRaceSettings(), {
      leads: [0, 60, 5, 5],
      hour_fix: -1,
      profile: { sr: "S1" },
      req: { champ: { dr: "S0" } },
      cats: ["champ", "daily"],
    });
    expect(s.leads).toEqual([60, 5, 0]);
    expect(s.hour_fix).toBe(-1);
    expect(s.profile).toEqual({ sr: "S1", dr: "" });
    expect(s.req.champ).toEqual({ sr: "B2", dr: "S0" });
    expect(s.cats).toEqual(["daily", "champ"]);
    for (const bad of [
      { leads: [7] },
      { hour_fix: 3 },
      { hour_fix: 0.5 },
      { cats: [] },
      { cats: ["x"] },
      { profile: { sr: "Z9" } },
      { req: { nope: {} } },
      { unknown: 1 },
    ]) {
      expect(() => applySettingsPatch(defaultRaceSettings(), bad), JSON.stringify(bad)).toThrow();
    }

    const book = new RaceBook(new FakePrefs(), () => NOW);
    await book.put(pick("daily-rc", NOW + H));
    await book.put(pick("weekly-rc", NOW + H, { cat: "weekly" }));
    await book.put(pick("daily-lt", NOW + H, { src: "lt" }));
    await book.patchSettings({ hour_fix: 1 });
    const ids = (await book.list()).map((p) => p.id).sort();
    expect(ids).toEqual(
      [`daily-lt@${NOW + H}`, `daily-rc@${NOW + 2 * H}`, `weekly-rc@${NOW + H}`].sort(),
    );
    await expect(book.patchSettings({ leads: [7] })).rejects.toMatchObject({ status: 400 });
  });
});
