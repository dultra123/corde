import { describe, expect, it } from "vitest";
import type { ResultRow } from "../net/profile-api";
import {
  bareAddress,
  homeLamps,
  homePhase,
  lastRaces,
  matchResult,
  ordinalSuffix,
  phoneState,
  RESULT_WINDOW_MS,
  sameTrack,
} from "./lib";

const T0 = Date.UTC(2026, 10, 9, 20, 4);

function result(over: Partial<ResultRow> = {}): ResultRow {
  return {
    id: 1,
    file_name: "2026_11_09_20_04_00-54R1.xml",
    file_mtime_ms: T0,
    imported_wall_ms: T0,
    game_version: "1.42",
    track_venue: "Circuit Paul Ricard",
    track_course: "Paul Ricard - 1A",
    track_length_m: 5770,
    session_kind: "race",
    session_name: "Race",
    started_wall_ms: T0,
    minutes: 30,
    laps_planned: null,
    setting: "Multiplayer",
    server_name: null,
    player_name: "alex martin",
    player_position: 3,
    player_class_position: 3,
    player_grid_pos: 5,
    player_class_grid_pos: 5,
    player_class: "GT3",
    player_vehicle: "Mercedes-AMG LMGT3",
    player_vehicle_name: "Corde Racing #15",
    player_best_lap_s: 118.375,
    player_laps: 15,
    player_finish: "finished",
    num_drivers: 14,
    num_class_drivers: 14,
    ...over,
  };
}

const session = {
  track_name: "Paul Ricard - 1A",
  session_type: "race" as const,
  started_wall_ms: T0 + 45_000,
};

describe("matchResult", () => {
  it("links the result of the same track and type that started within 10 min", () => {
    expect(matchResult(session, [result()])?.id).toBe(1);
    expect(matchResult(session, [result({ started_wall_ms: T0 - 9 * 60_000 })])?.id).toBe(1);
    expect(
      matchResult(session, [
        result({ started_wall_ms: session.started_wall_ms + RESULT_WINDOW_MS }),
      ])?.id,
    ).toBe(1);
  });

  it("refuses another type, another track or a start too far away", () => {
    expect(matchResult(session, [result({ session_kind: "qualify" })])).toBeNull();
    expect(
      matchResult(session, [
        result({ track_course: "Silverstone International", track_venue: "Silverstone Circuit" }),
      ]),
    ).toBeNull();
    expect(
      matchResult(session, [
        result({ started_wall_ms: session.started_wall_ms + RESULT_WINDOW_MS + 1 }),
      ]),
    ).toBeNull();
    expect(matchResult(session, [result({ started_wall_ms: T0 - 86_400_000 })])).toBeNull();
    expect(matchResult(session, [])).toBeNull();
  });

  it("accepts the venue name and ignores case", () => {
    const byVenue = { ...session, track_name: "circuit paul ricard" };
    expect(matchResult(byVenue, [result()])?.id).toBe(1);
    const shorter = { ...session, track_name: "Paul Ricard" };
    expect(matchResult(shorter, [result()])?.id).toBe(1);
  });

  it("picks the closest start when two results qualify (race 1 / race 2)", () => {
    const r1 = result({ id: 1, started_wall_ms: T0 - 8 * 60_000 });
    const r2 = result({ id: 2, started_wall_ms: T0 + 60_000 });
    expect(matchResult(session, [r1, r2])?.id).toBe(2);
    expect(matchResult(session, [r2, r1])?.id).toBe(2);
  });
});

describe("sameTrack", () => {
  it("never matches an empty name", () => {
    expect(sameTrack("", "Paul Ricard")).toBe(false);
    expect(sameTrack("  ", "")).toBe(false);
    expect(sameTrack("Le Mans", "le mans ")).toBe(true);
    expect(sameTrack("Le Mans", "Spa")).toBe(false);
  });
});

describe("lastRaces", () => {
  it("keeps the races only, most recent first", () => {
    const rows = [
      result({ id: 1, started_wall_ms: T0 }),
      result({ id: 2, session_kind: "practice", started_wall_ms: T0 + 1 }),
      result({ id: 3, started_wall_ms: T0 + 2 }),
      result({ id: 4, session_kind: "qualify", started_wall_ms: T0 + 3 }),
      ...[5, 6, 7, 8, 9].map((id) => result({ id, started_wall_ms: T0 - id })),
    ];
    expect(lastRaces(rows).map((r) => r.id)).toEqual([3, 1, 5, 6, 7]);
    expect(lastRaces(rows, 2).map((r) => r.id)).toEqual([3, 1]);
    expect(lastRaces([])).toEqual([]);
  });
});

describe("ordinalSuffix", () => {
  it("speaks French and English", () => {
    expect([1, 2, 3, 12].map((n) => ordinalSuffix(n, "fr"))).toEqual(["er", "e", "e", "e"]);
    expect([1, 2, 3, 4, 11, 12, 13, 21, 22, 23].map((n) => ordinalSuffix(n, "en"))).toEqual([
      "st",
      "nd",
      "rd",
      "th",
      "th",
      "th",
      "th",
      "st",
      "nd",
      "rd",
    ]);
  });
});

describe("homePhase / homeLamps", () => {
  it("names where things stand", () => {
    expect(homePhase("connecting", false)).toBe("connecting");
    expect(homePhase("offline", false)).toBe("offline");
    // the automatic demo only means that no server answered
    expect(homePhase("demo-auto", true)).toBe("offline");
    expect(homePhase("demo", true)).toBe("demo");
    expect(homePhase("waiting-game", false)).toBe("waiting");
    expect(homePhase("monitor", false)).toBe("menu");
    expect(homePhase("monitor", true)).toBe("garage");
    expect(homePhase("live", true)).toBe("track");
  });

  it("lights the chain up to the step being waited for", () => {
    expect(homeLamps("connecting", null)).toEqual({
      server: "wait",
      game: "off",
      session: "off",
      track: "off",
      recording: "off",
    });
    expect(homeLamps("offline", null).server).toBe("error");
    expect(homeLamps("waiting", null)).toMatchObject({
      server: "on",
      game: "wait",
      session: "off",
    });
    expect(homeLamps("menu", null)).toMatchObject({ game: "on", session: "wait", track: "off" });
    expect(homeLamps("garage", null)).toMatchObject({ session: "on", track: "wait" });
    expect(homeLamps("track", null)).toMatchObject({
      server: "on",
      game: "on",
      session: "on",
      track: "on",
      recording: "off",
    });
  });

  it("shows the recorder: writing, paused or armed", () => {
    const rec = { recording: true, armed: false, idle_reason: null };
    expect(homeLamps("track", rec).recording).toBe("rec");
    expect(homeLamps("garage", { ...rec, idle_reason: "hors piste" }).recording).toBe("wait");
    expect(
      homeLamps("garage", { recording: false, armed: true, idle_reason: null }).recording,
    ).toBe("wait");
    expect(
      homeLamps("garage", { recording: false, armed: false, idle_reason: null }).recording,
    ).toBe("off");
    // a stale status must not light anything without a server
    expect(homeLamps("offline", rec).recording).toBe("off");
  });
});

describe("phoneState", () => {
  const net = { lan_access_setting: false, listening_on_lan: false, port: 8471, urls: [] };

  it("covers the four combinations of setting and listening address", () => {
    expect(phoneState(net)).toEqual({ kind: "off" });
    expect(phoneState({ ...net, lan_access_setting: true })).toEqual({ kind: "restart" });
    const urls = ["http://192.168.1.20:8471"];
    expect(phoneState({ ...net, lan_access_setting: true, listening_on_lan: true, urls })).toEqual({
      kind: "on",
      urls,
      stopping: false,
    });
    expect(phoneState({ ...net, listening_on_lan: true, urls })).toEqual({
      kind: "on",
      urls,
      stopping: true,
    });
  });

  it("says when the PC has no address to give", () => {
    expect(phoneState({ ...net, lan_access_setting: true, listening_on_lan: true })).toEqual({
      kind: "no-address",
      stopping: false,
    });
  });

  it("strips the scheme for display", () => {
    expect(bareAddress("http://192.168.1.20:8471")).toBe("192.168.1.20:8471");
    expect(bareAddress("http://[fd00::20]:8471/")).toBe("[fd00::20]:8471");
  });
});
