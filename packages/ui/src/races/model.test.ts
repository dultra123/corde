import { describe, expect, it } from "vitest";
import { parseServerMessage } from "../net/messages";
import type { RacePick, RaceSources } from "../net/races-api";
import { reminderKey, useRaces } from "../store/races";
import lmuTools from "./fixtures/lmutools-assumed.html?raw";
import raceControl from "./fixtures/racecontrol-2026-11-11.html?raw";
import { relLabel } from "./format";
import {
  buildSchedule,
  countdown,
  eveOf,
  isUrgent,
  missing,
  nextRace,
  pickOf,
  rank,
  safeImage,
  upcomingPicks,
  utcLabel,
} from "./model";
import { parseLmuTools, parseRaceControl } from "./parse";
import { mergeSettings } from "./RacesPage";

const NOW = Date.UTC(2026, 10, 11, 20, 10);
const MIN = 60e3;

function sources(rc: string | null, lt: string | null): RaceSources {
  const one = (id: "racecontrol" | "lmutools", html: string | null) => ({
    id,
    url: id === "racecontrol" ? "https://racecontrol.gg/" : "https://lmu.tools/schedule",
    state: "ok" as const,
    fetched_at_ms: NOW,
    attempted_at_ms: NOW,
    error: null,
    html,
  });
  return { enabled: true, sources: [one("racecontrol", rc), one("lmutools", lt)] };
}

describe("schedule from the server's pages", () => {
  it("parses each page at the time it was read and puts them together", () => {
    const v = buildSchedule(sources(raceControl, lmuTools));
    expect(v.sources.map((s) => [s.id, s.count, s.unreadable])).toEqual([
      ["racecontrol", 4, false],
      ["lmutools", 7, false],
    ]);
    expect(v.schedule?.races).toHaveLength(9);
    expect(v.schedule?.offset).toBe(-4);
  });

  it("marks a page without races as unreadable and keeps the other one", () => {
    const v = buildSchedule(sources("<html><body>Maintenance</body></html>", lmuTools));
    expect(v.sources[0]?.unreadable).toBe(true);
    expect(v.sources[1]?.unreadable).toBe(false);
    expect(v.schedule?.races.filter((r) => r.cat === "daily")).toHaveLength(2);
  });

  it("has nothing when nothing was read", () => {
    const v = buildSchedule({ enabled: false, sources: [] });
    expect(v.schedule).toBeNull();
  });
});

describe("picks", () => {
  const rc = parseRaceControl(raceControl, NOW);
  const lt = parseLmuTools(lmuTools, NOW);

  it("picks one start of a daily race", () => {
    const race = rc.races[1];
    if (!race) throw new Error("no race");
    const start = race.starts[2] as number;
    const p = pickOf(race, start);
    expect(p).toEqual({
      id: `lmp3-fixed-beginner@${start}`,
      race_id: "lmp3-fixed-beginner",
      cat: "daily",
      src: "rc",
      name: "LMP3 Fixed",
      level: "beginner",
      track: "Monza",
      duration: "32 min",
      image: "https://img.example/rc/daily/lmp3-fixed-monza.webp?w=800",
      start_ms: start,
      eve_ms: null,
    });
  });

  it("picks a special event on its reminder day, with the evening before at 18:00", () => {
    const ev = lt.special[0];
    if (!ev) throw new Error("no event");
    const p = pickOf(ev, 0);
    expect(p.start_ms).toBe(ev.remindAt);
    expect(p.eve_ms).toBe(new Date(2026, 10, 20, 18, 0, 0).getTime());
    expect(p.src).toBe("lt");
    expect(eveOf(new Date(2026, 0, 1, 10).getTime())).toBe(new Date(2025, 11, 31, 18).getTime());
  });

  it("only sends https pictures", () => {
    expect(safeImage("http://a.example/x.png")).toBeNull();
    expect(safeImage("https://a.example/x.png")).toBe("https://a.example/x.png");
    expect(safeImage('https://a.example/x.png"onerror="x')).toBeNull();
    expect(safeImage(null)).toBeNull();
  });

  const pick = (id: string, start: number, cat: RacePick["cat"] = "daily"): RacePick => ({
    id,
    race_id: id,
    cat,
    src: "rc",
    name: id,
    level: null,
    track: "Monza",
    duration: "",
    image: null,
    start_ms: start,
  });

  it("lists the coming picks and finds the next race within 3 h", () => {
    const picks = [
      pick("c", NOW + 4 * 3600e3),
      pick("a", NOW - 6 * MIN),
      pick("b", NOW + 12 * MIN),
      pick("s", NOW + 5 * MIN, "special"),
    ];
    expect(upcomingPicks(picks, NOW).map((p) => p.id)).toEqual(["s", "b", "c"]);
    expect(nextRace(picks, NOW)?.id).toBe("b");
    expect(nextRace(picks, NOW + 13 * MIN)).toBeNull();
    expect(relLabel(NOW + 12 * MIN, NOW, "fr")).toBe("dans 12 min");
    expect(relLabel(NOW + 65 * MIN, NOW, "fr")).toBe("dans 1 h 05");
    expect(relLabel(NOW + 12 * MIN, NOW, "en")).toBe("in 12 min");
  });

  it("counts down", () => {
    expect(countdown(4 * MIN + 7000)).toBe("4:07");
    expect(countdown(3600e3 + 2 * MIN + 7000)).toBe("1:02:07");
    expect(countdown(-5000)).toBe("0:00");
  });
});

describe("ratings and reminders", () => {
  it("knows what the driver lacks", () => {
    expect(rank("")).toBe(0);
    expect(rank("S2")).toBeGreaterThan(rank("B3"));
    expect(missing({ sr: "S2", dr: "" }, { sr: "B3", dr: "" })).toEqual(["SR S2"]);
    expect(missing({ sr: "S2", dr: "B2" }, { sr: "G0", dr: "B1" })).toEqual(["DR B2"]);
    // A driver who did not say: nothing is locked.
    expect(missing({ sr: "S2", dr: "B2" }, { sr: "", dr: "" })).toEqual([]);
  });

  it("rings louder from 5 minutes", () => {
    expect(isUrgent({ kind: "lead", lead_min: 15 })).toBe(false);
    expect(isUrgent({ kind: "lead", lead_min: 5 })).toBe(true);
    expect(isUrgent({ kind: "lead", lead_min: 0 })).toBe(true);
    expect(isUrgent({ kind: "eve", lead_min: null })).toBe(false);
    expect(isUrgent({ kind: "day", lead_min: null })).toBe(true);
    expect(utcLabel(-4)).toBe("UTC−4");
    expect(utcLabel(5.5)).toBe("UTC+5:30");
  });

  it("reads race_reminder from the socket and queues it once", () => {
    const raw = JSON.stringify({
      type: "race_reminder",
      pick_id: "lmp3-fixed-beginner@1762893000000",
      race_id: "lmp3-fixed-beginner",
      name: "LMP3 Fixed",
      track: "Monza",
      cat: "daily",
      level: "beginner",
      start_ms: 1762893000000,
      kind: "lead",
      lead_min: 5,
      due_ms: 1762892700000,
    });
    const m = parseServerMessage(raw);
    expect(m?.kind).toBe("race_reminder");
    if (m?.kind !== "race_reminder") return;
    expect(m.reminder.lead_min).toBe(5);
    expect(parseServerMessage('{"type":"race_reminder","name":3}')).toBeNull();
    const store = useRaces.getState();
    store.pushReminder(m.reminder);
    store.pushReminder(m.reminder);
    expect(useRaces.getState().reminders).toHaveLength(1);
    expect(useRaces.getState().reminders[0]?.key).toBe(reminderKey(m.reminder));
    // The 0-min reminder of the same pick replaces the 5-min one.
    store.pushReminder({ ...m.reminder, lead_min: 0 });
    expect(useRaces.getState().reminders.map((r) => r.lead_min)).toEqual([0]);
    store.dismiss(useRaces.getState().reminders[0]?.key ?? "");
    expect(useRaces.getState().reminders).toHaveLength(0);
  });

  it("merges a settings patch field by field", () => {
    const base = {
      leads: [15, 5],
      levels: ["beginner"],
      cats: ["daily"],
      hour_fix: 0,
      profile: { sr: "B1", dr: "" },
      req: {
        beginner: { sr: "B0", dr: "" },
        intermediate: { sr: "S0", dr: "" },
        advanced: { sr: "G0", dr: "" },
        weekly: { sr: "S2", dr: "" },
        special: { sr: "B2", dr: "" },
        champ: { sr: "B2", dr: "" },
      },
    };
    const next = mergeSettings(base, { req: { weekly: { dr: "S0" } }, profile: { dr: "B3" } });
    expect(next.req.weekly).toEqual({ sr: "S2", dr: "S0" });
    expect(next.profile).toEqual({ sr: "B1", dr: "B3" });
    expect(next.leads).toEqual([15, 5]);
  });
});
