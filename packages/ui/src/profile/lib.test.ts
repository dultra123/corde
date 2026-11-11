import { describe, expect, it } from "vitest";
import type { FileDriver, FileLap, ResultRow } from "../net/profile-api";
import { bestLapSeries, classKeyOf, classLabel, fmtPos, gained, lapTones, standings } from "./lib";

function driver(over: Partial<FileDriver>): FileDriver {
  return {
    name: "x",
    vehicle_name: "",
    vehicle_file: "",
    car_type: "",
    car_class: "GT3",
    category: null,
    car_number: null,
    team: "",
    is_player: true,
    connected: true,
    server_scored: true,
    grid_pos: null,
    position: null,
    class_grid_pos: null,
    class_position: null,
    points: null,
    class_points: null,
    finish_status: "Finished Normally",
    finish: "finished",
    dnf_reason: null,
    best_lap_s: null,
    finish_time_s: null,
    laps_completed: 0,
    pitstops: 0,
    aids: [],
    laps: [],
    incidents: {
      contacts: 0,
      contacts_with_vehicles: 0,
      track_limit_warnings: 0,
      invalid_laps: 0,
      penalties: 0,
    },
    ...over,
  };
}

function lap(num: number, over: Partial<FileLap>): FileLap {
  return {
    num,
    position: null,
    elapsed_s: null,
    lap_time_s: null,
    s1: null,
    s2: null,
    s3: null,
    top_speed_kmh: null,
    fuel_fraction: null,
    front_compound: "Medium",
    rear_compound: "Medium",
    tyre_wear: null,
    pit: false,
    ...over,
  };
}

describe("classKeyOf / classLabel", () => {
  it("maps the file's class strings onto protocol classes", () => {
    expect(classKeyOf("GT3")).toBe("gt3");
    expect(classKeyOf("LMP2_ELMS")).toBe("lmp2");
    expect(classKeyOf("Hypercar")).toBe("hypercar");
    expect(classKeyOf("LMGTE")).toBe("gte");
    expect(classKeyOf("")).toBe("unknown");
    expect(classLabel("LMP2_ELMS")).toBe("LMP2 ELMS");
    expect(classLabel("")).toBe("—");
  });
});

describe("standings", () => {
  // The GT3 class of the Paul Ricard race, reduced: winner on 16 laps, player one lap down,
  // a finisher on the same lap, a DNF.
  const drivers = [
    driver({
      name: "me",
      position: 23,
      class_position: 12,
      laps_completed: 15,
      finish_time_s: 1851.414,
      best_lap_s: 118.3745,
    }),
    driver({
      name: "Dylan",
      position: 11,
      class_position: 1,
      laps_completed: 16,
      finish_time_s: 1841.8375,
      best_lap_s: 113.2231,
    }),
    driver({
      name: "Domnu",
      position: 12,
      class_position: 2,
      laps_completed: 16,
      finish_time_s: 1847.3776,
      best_lap_s: 114.2438,
    }),
    driver({
      name: "Walt",
      position: 36,
      class_position: 16,
      laps_completed: 10,
      finish: "dnf",
      finish_status: "DNF",
      dnf_reason: "DNF",
      best_lap_s: 113.4227,
    }),
    driver({
      name: "P2 guy",
      car_class: "LMP2_ELMS",
      position: 1,
      class_position: 1,
      laps_completed: 18,
      finish_time_s: 1800,
      best_lap_s: 99,
    }),
  ];

  it("orders a class by class position and computes race gaps", () => {
    const rows = standings(drivers, "GT3", true);
    expect(rows.map((r) => r.driver.name)).toEqual(["Dylan", "Domnu", "me", "Walt"]);
    expect(rows.map((r) => r.gap)).toEqual([
      { kind: "leader" },
      { kind: "time", s: 1847.3776 - 1841.8375 },
      { kind: "laps", n: 1 },
      { kind: "dnf", reason: "DNF" },
    ]);
    expect(rows[2]?.classPos).toBe(12);
  });

  it("orders everybody by overall position", () => {
    const rows = standings(drivers, null, true);
    expect(rows.slice(0, 2).map((r) => [r.driver.name, r.classPos])).toEqual([
      ["P2 guy", 1],
      ["Dylan", 11],
    ]);
  });

  it("uses best-lap gaps outside of a race", () => {
    const rows = standings(drivers, "GT3", false);
    expect(rows[0]?.gap).toEqual({ kind: "leader" });
    expect(rows[2]?.gap).toEqual({ kind: "time", s: 118.3745 - 113.2231 });
    const noTime = standings([driver({ name: "a" })], "GT3", false);
    expect(noTime[0]?.gap).toEqual({ kind: "none" });
    expect(noTime[0]?.classPos).toBe(1);
  });
});

describe("lapTones", () => {
  it("marks the best lap and sectors, mutes untimed and pit laps", () => {
    const l1 = lap(1, {});
    const l2 = lap(2, { lap_time_s: 121.1, s1: 32.6, s2: 31.8, s3: 56.7 });
    const l3 = lap(3, { lap_time_s: 118.4, s1: 31.1, s2: 31.3, s3: 56.0 });
    const l4 = lap(4, { lap_time_s: 130.0, s1: 30.9, s2: 40.0, s3: 59.1, pit: true });
    const tones = lapTones([l1, l2, l3, l4]);
    expect(tones.get(l1)).toEqual({ time: "muted", s1: "muted", s2: "muted", s3: "muted" });
    expect(tones.get(l2)?.time).toBe("normal");
    expect(tones.get(l3)).toEqual({ time: "best", s1: "best", s2: "best", s3: "best" });
    // A pit lap never counts as best, even with the fastest S1.
    expect(tones.get(l4)?.time).toBe("muted");
    expect(tones.get(l4)?.s1).toBe("normal");
  });
});

describe("bestLapSeries / fmtPos / gained", () => {
  const row = (id: number, course: string, started: number, best: number | null): ResultRow =>
    ({
      id,
      track_course: course,
      started_wall_ms: started,
      player_best_lap_s: best,
    }) as ResultRow;

  it("keeps one track, oldest first, without missing bests", () => {
    const pts = bestLapSeries(
      [
        row(3, "A", 3000, 100),
        row(1, "A", 1000, 102),
        row(2, "B", 2000, 50),
        row(4, "A", 4000, null),
      ],
      "A",
    );
    expect(pts.map((p) => p.result.id)).toEqual([1, 3]);
    expect(pts[0]).toMatchObject({ t: 1, bestLapS: 102 });
  });

  it("formats positions and grid gains", () => {
    expect(fmtPos(12, 19)).toBe("12 / 19");
    expect(fmtPos(4)).toBe("4");
    expect(fmtPos(null, 10)).toBe("—");
    expect(fmtPos(0, 10)).toBe("—");
    expect(gained(35, 23)).toBe(12);
    expect(gained(2, 5)).toBe(-3);
    expect(gained(null, 5)).toBeNull();
  });
});
