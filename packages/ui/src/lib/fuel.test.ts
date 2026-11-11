import type { LapRecord } from "@corde/protocol";
import { describe, expect, it } from "vitest";
import { energyLaps, fuelLaps } from "./fuel";

function lap(n: number, fuel: number, extra: Partial<LapRecord> = {}): LapRecord {
  return {
    lap_number: n,
    lap_time_s: 115,
    s1: 40,
    s2: 40,
    s3: 35,
    valid: true,
    in_lap: false,
    out_lap: false,
    fuel_used_l: fuel,
    virtual_energy_used: 0.032,
    top_speed_kmh: 300,
    front_compound: "Medium",
    rear_compound: "Medium",
    avg_tyre_temp_c: [90, 90, 88, 88],
    avg_tyre_pressure_kpa: [185, 185, 182, 182],
    max_brake_temp_c: [500, 500, 400, 400],
    wear_remaining_end: [0.9, 0.9, 0.9, 0.9],
    track_c: 30,
    ambient_c: 22,
    rain: 0,
    session_elapsed_end_s: 1000,
    timed_by_game: true,
    note: null,
    ...extra,
  };
}

describe("fuelLaps", () => {
  it("needs at least one usable lap", () => {
    expect(fuelLaps(50, [])).toBeNull();
    expect(fuelLaps(50, [lap(1, 0)])).toBeNull();
    expect(fuelLaps(50, [lap(1, 3.2, { out_lap: true })])).toBeNull();
  });

  it("averages the last five race laps", () => {
    const laps = [lap(1, 9), lap(2, 3.0), lap(3, 3.2), lap(4, 3.4), lap(5, 3.0), lap(6, 3.4)];
    const r = fuelLaps(32, laps);
    expect(r).not.toBeNull();
    expect(r?.perLap).toBeCloseTo(3.2, 5);
    expect(r?.samples).toBe(5);
    expect(r?.lapsLeft).toBe(10);
    expect(r?.lapsLeftExact).toBeCloseTo(10, 5);
  });

  it("ignores in/out laps and never goes negative", () => {
    const laps = [lap(1, 3.0), lap(2, 1.0, { in_lap: true }), lap(3, 0.5, { out_lap: true })];
    expect(fuelLaps(6, laps)?.lapsLeft).toBe(2);
    expect(fuelLaps(0, laps)?.lapsLeft).toBe(0);
  });

  it("estimates virtual energy the same way", () => {
    const r = energyLaps(0.8, [lap(1, 3), lap(2, 3)]);
    expect(r?.lapsLeft).toBe(25);
    expect(energyLaps(0.8, [lap(1, 3, { virtual_energy_used: null })])).toBeNull();
  });
});
