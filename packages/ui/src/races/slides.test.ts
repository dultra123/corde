import { describe, expect, it } from "vitest";
import type { RacePick, RaceSettings } from "../net/races-api";
import type { Race } from "./parse";
import {
  autoplays,
  buildSlides,
  countdownLabel,
  followIndex,
  isSoon,
  MAX_SLIDES,
  nextStart,
  slideArt,
  swipeStep,
  trackMark,
  wrap,
} from "./slides";

const NOW = Date.UTC(2026, 10, 12, 18, 0);
const SEC = 1e3;
const MIN = 60e3;
const HOUR = 3600e3;
const DAY = 864e5;

function race(over: Partial<Race> & Pick<Race, "id">): Race {
  return {
    cat: "daily",
    src: "rc",
    name: over.id,
    level: "beginner",
    track: "Monza",
    duration: "32 min",
    image: null,
    classes: [],
    starts: [],
    ...over,
  };
}

const settings = (over: Partial<RaceSettings> = {}): RaceSettings => ({
  leads: [15, 5],
  levels: ["beginner", "intermediate", "advanced"],
  cats: ["daily", "weekly", "special", "champ"],
  hour_fix: 0,
  profile: { sr: "", dr: "" },
  req: {
    beginner: { sr: "", dr: "" },
    intermediate: { sr: "", dr: "" },
    advanced: { sr: "", dr: "" },
    weekly: { sr: "", dr: "" },
    special: { sr: "", dr: "" },
    champ: { sr: "", dr: "" },
  },
  ...over,
});

function pick(over: Partial<RacePick> & Pick<RacePick, "race_id" | "start_ms">): RacePick {
  return {
    id: `${over.race_id}@${over.start_ms}`,
    cat: "daily",
    src: "rc",
    name: over.race_id,
    level: "beginner",
    track: "Monza",
    duration: "32 min",
    image: null,
    ...over,
  };
}

const sebring = race({
  id: "sebring",
  cat: "special",
  src: "lt",
  level: null,
  track: "Sebring International Raceway",
  from: NOW + 3 * DAY,
  to: NOW + 4 * DAY,
  remindAt: NOW + 3 * DAY + 10 * HOUR,
});

const RACES: Race[] = [
  race({ id: "lmp3", starts: [NOW + 12 * MIN, NOW + 57 * MIN] }),
  race({ id: "proto", level: "intermediate", starts: [NOW + 7 * MIN, NOW + 67 * MIN] }),
  race({ id: "elms", level: "advanced", starts: [NOW + 40 * MIN] }),
  race({ id: "gt3w", cat: "weekly", src: "lt", level: null, starts: [NOW + 26 * HOUR] }),
  race({ id: "hyperw", cat: "weekly", src: "lt", level: null, starts: [NOW + 3 * DAY] }),
  race({ id: "rcpro", cat: "champ", src: "lt", level: null, starts: [NOW + 2 * HOUR] }),
  sebring,
];

describe("which races the carousel shows, in which order", () => {
  it("shows the picks first, then daily, weekly, special, championships, each by start", () => {
    const picks = [pick({ race_id: "elms", start_ms: NOW + 40 * MIN, level: "advanced" })];
    const s = buildSlides({ races: RACES, picks, settings: settings(), now: NOW });
    expect(s.map((x) => x.key)).toEqual(["elms", "proto", "lmp3", "gt3w", "sebring", "rcpro"]);
    expect(s[0]?.picked).toBe(true);
    expect(s[0]?.pick?.id).toBe(picks[0]?.id);
    expect(s.slice(1).every((x) => !x.picked && x.pick === null)).toBe(true);
    // the weekly race in 3 days is beyond the 48 h of the widget
    expect(s.some((x) => x.key === "hyperw")).toBe(false);
  });

  it("never shows the same race twice", () => {
    const picks = [
      pick({ race_id: "lmp3", start_ms: NOW + 57 * MIN }),
      pick({ race_id: "lmp3", start_ms: NOW + 12 * MIN }),
    ];
    const s = buildSlides({ races: RACES, picks, settings: settings(), now: NOW });
    expect(s.filter((x) => x.key === "lmp3")).toHaveLength(1);
    expect(s[0]?.key).toBe("lmp3");
    expect(s[0]?.start).toBe(NOW + 12 * MIN);
    expect(new Set(s.map((x) => x.key)).size).toBe(s.length);
  });

  it("follows the level and category filters for the schedule, not for the picks", () => {
    const only = settings({ levels: ["beginner"], cats: ["daily"] });
    const picks = [pick({ race_id: "rcpro", cat: "champ", level: null, start_ms: NOW + 2 * HOUR })];
    const s = buildSlides({ races: RACES, picks, settings: only, now: NOW });
    expect(s.map((x) => x.key)).toEqual(["rcpro", "lmp3"]);
  });

  it("keeps at most eight slides", () => {
    const many = Array.from({ length: 12 }, (_, i) =>
      race({ id: `d${i}`, starts: [NOW + (i + 1) * MIN * 5] }),
    );
    const s = buildSlides({ races: many, picks: [], settings: settings(), now: NOW });
    expect(s).toHaveLength(MAX_SLIDES);
    expect(s[0]?.key).toBe("d0");
  });

  it("drops a start once it has passed: a pick leaves, a race moves to its next start", () => {
    const picks = [pick({ race_id: "proto", start_ms: NOW + 7 * MIN, level: "intermediate" })];
    const before = buildSlides({ races: RACES, picks, settings: settings(), now: NOW });
    expect(before[0]?.key).toBe("proto");
    const later = NOW + 13 * MIN;
    const after = buildSlides({ races: RACES, picks, settings: settings(), now: later });
    // the pick is gone; proto comes back as a race of the schedule, on its next start
    expect(after.find((x) => x.key === "proto")?.picked).toBe(false);
    expect(after.find((x) => x.key === "proto")?.start).toBe(NOW + 67 * MIN);
    expect(after.find((x) => x.key === "lmp3")?.start).toBe(NOW + 57 * MIN);
    // a race without any further start leaves
    const late = buildSlides({
      races: RACES,
      picks: [],
      settings: settings(),
      now: NOW + 41 * MIN,
    });
    expect(late.some((x) => x.key === "elms")).toBe(false);
    // exactly at the start it is gone too (no « 0 s » slide)
    const atStart = buildSlides({ races: RACES, picks, settings: settings(), now: NOW + 7 * MIN });
    expect(atStart.find((x) => x.key === "proto")?.start).toBe(NOW + 67 * MIN);
  });

  it("shows a picked special event until its last day, by its dates", () => {
    const p = pick({
      race_id: "sebring",
      cat: "special",
      src: "lt",
      level: null,
      start_ms: sebring.remindAt as number,
    });
    const s = buildSlides({ races: RACES, picks: [p], settings: settings(), now: NOW });
    expect(s[0]?.key).toBe("sebring");
    expect(s[0]?.start).toBe(sebring.from);
    expect(s[0]?.end).toBe(sebring.to);
    const during = buildSlides({
      races: RACES,
      picks: [p],
      settings: settings(),
      now: NOW + 3 * DAY + 12 * HOUR,
    });
    expect(during[0]?.key).toBe("sebring");
    // a special event of the schedule is picked on its reminder of the day
    const free = buildSlides({ races: RACES, picks: [], settings: settings(), now: NOW });
    expect(free.find((x) => x.key === "sebring")?.pickId).toBe(`sebring@${sebring.remindAt}`);
  });

  it("applies the hour correction to racecontrol.gg starts", () => {
    const s = buildSlides({
      races: [race({ id: "lmp3", starts: [NOW + 12 * MIN] })],
      picks: [],
      settings: settings({ hour_fix: 1 }),
      now: NOW,
    });
    expect(s[0]?.start).toBe(NOW + 12 * MIN + HOUR);
    expect(s[0]?.pickId).toBe(`lmp3@${NOW + 12 * MIN + HOUR}`);
  });

  it("shows only the picks while the settings are not loaded, and nothing when empty", () => {
    const picks = [pick({ race_id: "x", start_ms: NOW + HOUR })];
    expect(
      buildSlides({ races: RACES, picks, settings: null, now: NOW }).map((x) => x.key),
    ).toEqual(["x"]);
    expect(buildSlides({ races: [], picks: [], settings: settings(), now: NOW })).toEqual([]);
  });

  it("borrows the race's picture for a pick and refuses anything but https", () => {
    const withImg = RACES.map((r) =>
      r.id === "lmp3" ? { ...r, image: "https://example.org/monza.webp" } : r,
    );
    const picks = [pick({ race_id: "lmp3", start_ms: NOW + 12 * MIN })];
    const s = buildSlides({ races: withImg, picks, settings: settings(), now: NOW });
    expect(s[0]?.image).toBe("https://example.org/monza.webp");
    const http = buildSlides({
      races: [race({ id: "a", image: "http://example.org/a.jpg", starts: [NOW + MIN * 3] })],
      picks: [],
      settings: settings(),
      now: NOW,
    });
    expect(http[0]?.image).toBeNull();
  });

  it("knows the next start of a race", () => {
    expect(nextStart(race({ id: "a", starts: [NOW - MIN, NOW + MIN] }), NOW)).toBe(NOW + MIN);
    expect(nextStart(race({ id: "a", starts: [NOW] }), NOW)).toBeNull();
    expect(nextStart(sebring, NOW)).toBe(sebring.from);
    expect(nextStart(sebring, NOW + 5 * DAY)).toBeNull();
  });
});

describe("the countdown", () => {
  it("counts the seconds under a minute, then minutes, hours and days", () => {
    expect(countdownLabel(NOW + 42 * SEC, null, NOW, "fr")).toBe("dans 42 s");
    expect(countdownLabel(NOW + 41_200, null, NOW, "fr")).toBe("dans 42 s");
    expect(countdownLabel(NOW + 60 * SEC, null, NOW, "fr")).toBe("dans 60 s");
    expect(countdownLabel(NOW + 61 * SEC, null, NOW, "fr")).toBe("dans 2 min");
    expect(countdownLabel(NOW + 12 * MIN, null, NOW, "fr")).toBe("dans 12 min");
    expect(countdownLabel(NOW + 65 * MIN, null, NOW, "fr")).toBe("dans 1 h 05");
    expect(countdownLabel(NOW + 3 * DAY, null, NOW, "fr")).toBe("dans 3 j");
    expect(countdownLabel(NOW + 42 * SEC, null, NOW, "en")).toBe("in 42 s");
    expect(countdownLabel(NOW + 12 * MIN, null, NOW, "en")).toBe("in 12 min");
  });

  it("says a running special event is under way", () => {
    expect(countdownLabel(NOW - HOUR, NOW + DAY, NOW, "fr")).toBe("en cours");
    expect(countdownLabel(NOW, null, NOW, "fr")).toBe("maintenant");
  });

  it("turns urgent five minutes before a start (not for a special event)", () => {
    expect(isSoon({ start: NOW + 5 * MIN, cat: "daily" }, NOW)).toBe(true);
    expect(isSoon({ start: NOW + 6 * MIN, cat: "daily" }, NOW)).toBe(false);
    expect(isSoon({ start: NOW + MIN, cat: "special" }, NOW)).toBe(false);
  });
});

describe("the picture", () => {
  it("falls back to the drawn stand-in when there is none, it is unsafe, or it failed", () => {
    expect(slideArt("https://example.org/a.webp", false)).toBe("https://example.org/a.webp");
    expect(slideArt("https://example.org/a.webp", true)).toBeNull();
    expect(slideArt(null, false)).toBeNull();
    expect(slideArt("http://example.org/a.webp", false)).toBeNull();
    expect(slideArt('https://example.org/a.webp" onerror="x', false)).toBeNull();
    expect(trackMark("Circuit de la Sarthe (24h)")).toBe("Circuit de la Sarthe");
  });
});

describe("turning", () => {
  const free = { count: 4, hovered: false, focused: false, hidden: false, reducedMotion: false };
  it("turns by itself only when nothing holds it", () => {
    expect(autoplays(free)).toBe(true);
    expect(autoplays({ ...free, hovered: true })).toBe(false);
    expect(autoplays({ ...free, focused: true })).toBe(false);
    expect(autoplays({ ...free, hidden: true })).toBe(false);
    expect(autoplays({ ...free, reducedMotion: true })).toBe(false);
    expect(autoplays({ ...free, count: 1 })).toBe(false);
  });

  it("wraps around both ways", () => {
    expect(wrap(4, 4)).toBe(0);
    expect(wrap(-1, 4)).toBe(3);
    expect(wrap(2, 0)).toBe(0);
  });

  it("stays on the same race when the list changes", () => {
    expect(followIndex("b", 1, ["a", "b", "c"])).toBe(1);
    expect(followIndex("b", 1, ["b", "c"])).toBe(0);
    // the current race left: the one that took its place
    expect(followIndex("b", 1, ["a", "c", "d"])).toBe(1);
    expect(followIndex("z", 5, ["a", "c"])).toBe(1);
    expect(followIndex(null, 0, [])).toBe(0);
  });

  it("reads a horizontal swipe, not a vertical scroll", () => {
    expect(swipeStep(-80, 5)).toBe(1);
    expect(swipeStep(80, 5)).toBe(-1);
    expect(swipeStep(-20, 0)).toBe(0);
    expect(swipeStep(-60, 70)).toBe(0);
  });
});
