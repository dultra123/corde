import { describe, expect, it } from "vitest";
import { isCounted, type LapLike, lapStats, lapTones, sessionDurationS } from "./stats";

function lap(
  n: number,
  time: number,
  extra: Partial<LapLike> & { sectors?: [number, number, number] } = {},
): LapLike {
  const { sectors, ...rest } = extra;
  return {
    lap_number: n,
    lap_time_s: time,
    s1: sectors ? sectors[0] : time * 0.3,
    s2: sectors ? sectors[1] : time * 0.3,
    s3: sectors ? sectors[2] : time * 0.4,
    valid: true,
    in_lap: false,
    out_lap: false,
    fuel_used_l: 2.5,
    top_speed_kmh: 280,
    ...rest,
  };
}

describe("lapStats", () => {
  it("ignores invalid, in and out laps for bests and averages", () => {
    const laps = [
      lap(1, 95, { out_lap: true }),
      lap(2, 92.0),
      lap(3, 90.0),
      lap(4, 85.0, { valid: false }),
      lap(5, 94.0, { top_speed_kmh: 301 }),
      lap(6, 97, { in_lap: true, fuel_used_l: 0 }),
    ];
    const s = lapStats(laps);
    expect(laps.filter(isCounted).map((l) => l.lap_number)).toEqual([2, 3, 5]);
    expect(s.best?.lap_number).toBe(3);
    expect(s.mean).toBeCloseTo(92, 6);
    // population std-dev of [92, 90, 94] = sqrt((0 + 4 + 4) / 3)
    expect(s.stddev).toBeCloseTo(Math.sqrt(8 / 3), 6);
    expect(s.counted).toBe(3);
    expect(s.total).toBe(6);
    expect(s.topSpeed).toBe(301);
    expect(s.fuelPerLap).toBeCloseTo(2.5, 6);
    expect(s.bestSectors[0]).toBeCloseTo(27, 6);
  });

  it("handles empty and single-lap sessions", () => {
    expect(lapStats([])).toMatchObject({
      best: null,
      mean: null,
      stddev: null,
      fuelPerLap: null,
      topSpeed: null,
    });
    const one = lapStats([lap(1, 90)]);
    expect(one.best?.lap_number).toBe(1);
    expect(one.mean).toBe(90);
    expect(one.stddev).toBeNull();
  });
});

describe("lapTones", () => {
  it("colours the session best purple, running personal bests green, the rest grey/normal", () => {
    const laps = [
      lap(1, 95, { out_lap: true }),
      lap(2, 92.0, { sectors: [30, 30, 32] }),
      lap(3, 93.0, { sectors: [29, 31, 33] }),
      lap(4, 91.0, { sectors: [30, 29, 32] }),
      lap(5, 90.0, { sectors: [28, 30, 32] }),
      lap(6, 91.5, { valid: false }),
    ];
    const tones = lapTones(laps);
    expect(tones.get(1)?.time).toBe("muted");
    expect(tones.get(2)?.time).toBe("pb"); // first counted lap is a PB
    expect(tones.get(3)?.time).toBe("normal");
    expect(tones.get(4)?.time).toBe("pb");
    expect(tones.get(5)?.time).toBe("best");
    expect(tones.get(6)).toEqual({ time: "muted", s1: "muted", s2: "muted", s3: "muted" });
    // sectors: S1 best = 28 (lap 5), lap 3's 29 was a PB at the time
    expect(tones.get(3)?.s1).toBe("pb");
    expect(tones.get(5)?.s1).toBe("best");
    expect(tones.get(4)?.s2).toBe("best");
    expect(tones.get(2)?.s3).toBe("best"); // 32 ties the best S3 → purple
    expect(tones.get(4)?.s3).toBe("best");
  });
});

describe("sessionDurationS", () => {
  it("uses the end stamp or now", () => {
    expect(sessionDurationS(1000, 61_000)).toBe(60);
    expect(sessionDurationS(1000, null, 31_000)).toBe(30);
  });
});
