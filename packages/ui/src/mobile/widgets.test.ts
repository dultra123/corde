import { describe, expect, it } from "vitest";
import type { RacePick, RaceSettings } from "../net/races-api";
import type { Race } from "../races/parse";
import {
  AFTER_START_MS,
  buildPayload,
  extendDaily,
  MAX_PAYLOAD_BYTES,
  payloadKey,
  picksAt,
  planActivity,
  racesAt,
  WIDGET_VERSION,
  widgetPicks,
  widgetRaces,
} from "./widgets";

const NOW = Date.UTC(2026, 10, 14, 18, 0);
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
    track: "Spa",
    duration: "20 min",
    image: null,
    eve_ms: null,
    ...over,
  };
}

describe("extendDaily", () => {
  it("goes on at a constant interval", () => {
    const starts = [NOW + 25 * MIN, NOW + 55 * MIN, NOW + 85 * MIN, NOW + 115 * MIN];
    const { s, x } = extendDaily(starts, NOW, 4 * HOUR);
    expect(s).toEqual(starts);
    expect(x[0]).toBe(NOW + 145 * MIN);
    expect(x.at(-1)).toBe(NOW + 235 * MIN);
    expect(x.every((t, i) => i === 0 || t - (x[i - 1] as number) === 30 * MIN)).toBe(true);
  });

  it("repeats every 24 h when the interval is not constant", () => {
    const starts = [NOW + 30 * MIN, NOW + 2 * HOUR];
    expect(extendDaily(starts, NOW, 48 * HOUR).x).toEqual([
      NOW + 30 * MIN + DAY,
      NOW + 2 * HOUR + DAY,
    ]);
    expect(extendDaily(starts, NOW, 72 * HOUR).x).toEqual([
      NOW + 30 * MIN + DAY,
      NOW + 2 * HOUR + DAY,
      NOW + 30 * MIN + 2 * DAY,
      NOW + 2 * HOUR + 2 * DAY,
    ]);
  });

  it("estimates nothing without a start, and drops past starts", () => {
    expect(extendDaily([], NOW)).toEqual({ s: [], x: [] });
    const { s } = extendDaily([NOW - 10 * MIN, NOW + 10 * MIN], NOW, HOUR);
    expect(s).toEqual([NOW + 10 * MIN]);
  });
});

describe("widgetRaces", () => {
  const races = [
    race({ id: "sprint-beginner", starts: [NOW + 10 * MIN, NOW + 40 * MIN, NOW + 70 * MIN] }),
    race({ id: "gt3-advanced", level: "advanced", starts: [NOW + 20 * MIN] }),
    race({
      id: "weekly-spa",
      cat: "weekly",
      src: "lt",
      level: null,
      classes: ["GT3", "LMP2"],
      starts: [NOW + 3 * HOUR, NOW + 9 * DAY],
    }),
    race({
      id: "special-6h",
      cat: "special",
      src: "lt",
      level: null,
      starts: [],
      from: NOW + 2 * DAY,
      to: NOW + 3 * DAY,
      remindAt: NOW + 2 * DAY + 10 * HOUR,
    }),
    race({
      id: "special-far",
      cat: "special",
      src: "lt",
      level: null,
      starts: [],
      from: NOW + 20 * DAY,
      to: NOW + 21 * DAY,
      remindAt: NOW + 20 * DAY,
    }),
  ];

  it("applies the filters, extends the daily races, limits the others to 7 days", () => {
    const out = widgetRaces(races, settings({ levels: ["beginner"] }), NOW);
    expect(out.map((r) => r.id)).toEqual(["sprint-beginner", "weekly-spa", "special-6h"]);
    const sprint = out[0];
    expect(sprint?.s).toHaveLength(3);
    expect(sprint?.x?.length).toBeGreaterThan(80);
    expect(out[1]?.s).toEqual([NOW + 3 * HOUR]);
    expect(out[1]?.cls).toEqual(["GT3", "LMP2"]);
    expect(out[2]).toMatchObject({ s: [NOW + 2 * DAY], to: NOW + 3 * DAY });
  });

  it("moves racecontrol.gg starts by the hour correction", () => {
    const out = widgetRaces([races[1] as Race], settings({ hour_fix: 1 }), NOW);
    expect(out[0]?.s).toEqual([NOW + 80 * MIN]);
  });
});

describe("buildPayload", () => {
  const races = [race({ id: "sprint-beginner", starts: [NOW + 10 * MIN, NOW + 40 * MIN] })];

  it("version, schedule state, picks, end of validity", () => {
    const p = buildPayload({
      races,
      enabled: true,
      picks: [pick({ race_id: "sprint-beginner", start_ms: NOW + 40 * MIN })],
      settings: settings(),
      lang: "en",
      now: NOW,
    });
    expect(p.v).toBe(WIDGET_VERSION);
    expect(p.sched).toBe("on");
    expect(p.lang).toBe("en");
    expect(p.mine).toHaveLength(1);
    expect(p.mine[0]).toMatchObject({ race: "sprint-beginner", t: NOW + 40 * MIN });
    expect(p.until).toBe(NOW + 40 * MIN + DAY);
    expect(JSON.stringify(p)).not.toMatch(/token|https?:\/\//);
  });

  it("off and none", () => {
    const base = { picks: [], settings: settings(), lang: "fr" as const, now: NOW };
    expect(buildPayload({ ...base, races, enabled: false }).sched).toBe("off");
    expect(buildPayload({ ...base, races, enabled: false }).races).toEqual([]);
    expect(buildPayload({ ...base, races: null, enabled: true }).sched).toBe("none");
    // Nothing announced: `until` is 0, and the block does not change with time (no rewrite).
    const off = buildPayload({ ...base, races, enabled: false });
    expect(off.until).toBe(0);
    expect(payloadKey(buildPayload({ ...base, now: NOW + 5000, races, enabled: false }))).toBe(
      payloadKey(off),
    );
  });

  it("stays under the size limit with a huge schedule", () => {
    const many = Array.from({ length: 80 }, (_, i) =>
      race({
        id: `race-${i}-with-a-rather-long-identifier`,
        name: `A long race name number ${i} for the size test`,
        track: "Circuit de la Sarthe (24h layout)",
        starts: Array.from({ length: 12 }, (_, k) => NOW + (k + 1) * 10 * MIN),
      }),
    );
    const p = buildPayload({
      races: many,
      enabled: true,
      picks: [],
      settings: settings(),
      lang: "fr",
      now: NOW,
    });
    expect(new TextEncoder().encode(JSON.stringify(p)).length).toBeLessThanOrEqual(
      MAX_PAYLOAD_BYTES,
    );
    expect(p.races.length).toBeGreaterThan(0);
  });

  it("the key ignores the writing time", () => {
    const input = { races, enabled: true, picks: [], settings: settings(), lang: "fr" as const };
    const a = buildPayload({ ...input, now: NOW });
    const b = { ...a, at: NOW + 5 };
    expect(payloadKey(a)).toBe(payloadKey(b));
  });
});

describe("racesAt (mirrored by WidgetData.swift)", () => {
  const p = buildPayload({
    races: [
      race({ id: "a", starts: [NOW + 10 * MIN, NOW + 40 * MIN, NOW + 70 * MIN] }),
      race({ id: "b", level: "advanced", starts: [NOW + 20 * MIN] }),
      race({
        id: "special-now",
        cat: "special",
        src: "lt",
        level: null,
        starts: [],
        from: NOW - HOUR,
        to: NOW + DAY,
        remindAt: NOW - HOUR,
      }),
    ],
    enabled: true,
    picks: [],
    settings: settings(),
    lang: "fr",
    now: NOW,
  });

  it("each race once, at its next start, the special event under way last", () => {
    const list = racesAt(p, NOW, 6);
    expect(list.map((x) => [x.race.id, x.start - NOW, x.ongoing])).toEqual([
      ["a", 10 * MIN, false],
      ["b", 20 * MIN, false],
      ["special-now", -HOUR, true],
    ]);
  });

  it("the list moves on by itself, then estimated starts take over", () => {
    expect(racesAt(p, NOW + 15 * MIN, 1)[0]?.race.id).toBe("b");
    const later = racesAt(p, NOW + 3 * HOUR, 6).find((x) => x.race.id === "a");
    expect(later?.estimated).toBe(true);
    expect(later?.start).toBe(NOW + 190 * MIN);
    const b = racesAt(p, NOW + 3 * HOUR, 6).find((x) => x.race.id === "b");
    expect(b).toMatchObject({ start: NOW + 20 * MIN + DAY, estimated: true });
  });

  it("picksAt drops the starts gone", () => {
    const q = {
      ...p,
      mine: widgetPicks([pick({ race_id: "a", start_ms: NOW + 10 * MIN })], [], NOW),
    };
    expect(picksAt(q, NOW)).toHaveLength(1);
    expect(picksAt(q, NOW + 10 * MIN)).toHaveLength(0);
  });
});

describe("planActivity", () => {
  const soon = pick({ race_id: "a", start_ms: NOW + 90 * MIN });
  const later = pick({ race_id: "b", start_ms: NOW + 5 * HOUR });
  const base = { picks: [soon, later], running: [], enabled: true, allowed: true, now: NOW };

  it("starts for the nearest pick within 2 h", () => {
    const plan = planActivity({ ...base, event: { kind: "foreground" } });
    expect(plan.start?.id).toBe(soon.id);
    expect(plan.end).toEqual([]);
  });

  it("nothing beyond 2 h, unless by hand within 8 h", () => {
    const only = { ...base, picks: [later] };
    expect(planActivity({ ...only, event: { kind: "foreground" } }).start).toBeNull();
    expect(planActivity({ ...only, event: { kind: "manual", pickId: later.id } }).start?.id).toBe(
      later.id,
    );
    const far = pick({ race_id: "c", start_ms: NOW + 9 * HOUR });
    const plan = planActivity({ ...base, picks: [far], event: { kind: "manual", pickId: far.id } });
    expect(plan).toMatchObject({ start: null, reason: "manual_too_early" });
  });

  it("keeps the one running for the same start, never two", () => {
    const running = [
      { id: "A1", pickId: soon.id, start: soon.start_ms },
      { id: "A2", pickId: later.id, start: later.start_ms },
    ];
    const plan = planActivity({ ...base, running, event: { kind: "foreground" } });
    expect(plan.start).toBeNull();
    expect(plan.end).toEqual(["A2"]);
  });

  it("a manual countdown far away gives way to a nearer start", () => {
    const running = [{ id: "A1", pickId: later.id, start: later.start_ms }];
    const plan = planActivity({ ...base, running, event: { kind: "picks" } });
    expect(plan.end).toEqual(["A1"]);
    expect(plan.start?.id).toBe(soon.id);
  });

  it("a manual countdown stays while nothing nearer is due", () => {
    const running = [{ id: "A1", pickId: later.id, start: later.start_ms }];
    const plan = planActivity({ ...base, picks: [later], running, event: { kind: "foreground" } });
    expect(plan).toMatchObject({ end: [], start: null, reason: "kept" });
  });

  it("ends when the pick goes, when off, when not allowed", () => {
    const running = [{ id: "A1", pickId: soon.id, start: soon.start_ms }];
    expect(planActivity({ ...base, picks: [], running, event: { kind: "picks" } }).end).toEqual([
      "A1",
    ]);
    expect(
      planActivity({ ...base, running, enabled: false, event: { kind: "setting" } }),
    ).toMatchObject({ end: ["A1"], start: null, reason: "off" });
    expect(
      planActivity({ ...base, running, allowed: false, event: { kind: "foreground" } }).end,
    ).toEqual(["A1"]);
  });

  it("« Départ donné » for 2 minutes, then the next start", () => {
    const gone = pick({ race_id: "g", start_ms: NOW - MIN });
    const next = pick({ race_id: "n", start_ms: NOW + HOUR });
    const running = [{ id: "A1", pickId: gone.id, start: gone.start_ms }];
    const during = planActivity({
      ...base,
      picks: [gone, next],
      running,
      event: { kind: "foreground" },
    });
    expect(during).toMatchObject({ end: [], start: null, reason: "kept" });
    const after = planActivity({
      ...base,
      picks: [gone, next],
      running,
      now: gone.start_ms + AFTER_START_MS + 1,
      event: { kind: "foreground" },
    });
    expect(after.end).toEqual(["A1"]);
    expect(after.start?.id).toBe(next.id);
  });

  it("never for a special event", () => {
    const sp = pick({ race_id: "s", cat: "special", start_ms: NOW + 30 * MIN });
    expect(planActivity({ ...base, picks: [sp], event: { kind: "foreground" } }).start).toBeNull();
    expect(
      planActivity({ ...base, picks: [sp], event: { kind: "manual", pickId: sp.id } }).reason,
    ).toBe("manual_unknown");
  });
});
