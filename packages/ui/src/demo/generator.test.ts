import { describe, expect, it } from "vitest";
import { DemoRace } from "./generator";

describe("DemoRace", () => {
  it("runs a lap in ~115 s within the speed envelope and fills every frame", () => {
    const race = new DemoRace({ seed: 3 });
    let laps = 0;
    let minKmh = Infinity;
    let maxKmh = 0;
    let lapTime = 0;
    const brakes = { min: Infinity, max: 0 };
    const tyres = { min: Infinity, max: 0 };
    // 2.5 laps worth of simulation at 20 Hz
    for (let i = 0; i < 2.5 * 115 * 20; i += 1) {
      const out = race.step(0.05, i % 10 === 0);
      for (const lap of out.laps) {
        laps += 1;
        lapTime = lap.lap_time_s;
        // the very first lap started mid-way (no S1 split): the runner drops it
        if (laps > 1) expect(lap.s1).not.toBeNull();
        expect(lap.fuel_used_l).toBeGreaterThan(0);
      }
      const f = out.frame;
      if (!f) continue;
      expect(f.player).not.toBeNull();
      expect(f.standings).toHaveLength(20);
      expect(f.session.track_length_m).toBeGreaterThan(5000);
      const p = f.player;
      if (!p) continue;
      minKmh = Math.min(minKmh, p.motion.speed_kmh);
      maxKmh = Math.max(maxKmh, p.motion.speed_kmh);
      for (const w of p.wheels) {
        brakes.min = Math.min(brakes.min, w.brake_temp_c);
        brakes.max = Math.max(brakes.max, w.brake_temp_c);
        for (const t of w.temp_c) {
          tyres.min = Math.min(tyres.min, t);
          tyres.max = Math.max(tyres.max, t);
        }
        expect(w.pressure_kpa).toBeGreaterThan(100);
        expect(w.wear_remaining).toBeLessThanOrEqual(1);
      }
      expect(p.engine.gear).toBeGreaterThanOrEqual(1);
      expect(p.engine.rpm).toBeLessThanOrEqual(p.engine.max_rpm);
    }
    expect(laps).toBeGreaterThanOrEqual(2);
    expect(lapTime).toBeGreaterThan(105);
    expect(lapTime).toBeLessThan(125);
    expect(minKmh).toBeGreaterThanOrEqual(75);
    expect(maxKmh).toBeLessThanOrEqual(305);
    expect(maxKmh).toBeGreaterThan(280);
    expect(brakes.max).toBeGreaterThan(400);
    expect(brakes.max).toBeLessThan(800);
    expect(brakes.min).toBeGreaterThan(100);
    expect(tyres.min).toBeGreaterThan(60);
    expect(tyres.max).toBeLessThan(115);
  });

  it("produces a live delta once a best lap exists", () => {
    const race = new DemoRace({ seed: 9 });
    let sawDelta = false;
    for (let i = 0; i < 3 * 115 * 20; i += 1) {
      const out = race.step(0.05, i % 20 === 0);
      const lap = out.frame?.player?.lap;
      if (lap?.best_lap_s && lap.delta_best_s !== 0) sawDelta = true;
    }
    expect(sawDelta).toBe(true);
  });

  it("keeps the standings consistent", () => {
    const race = new DemoRace({ seed: 5 });
    const f = race.step(0.05).frame;
    if (!f) throw new Error("frame");
    const places = f.standings.map((s) => s.place).sort((a, b) => a - b);
    expect(places).toEqual(Array.from({ length: 20 }, (_, i) => i + 1));
    expect(f.standings.filter((s) => s.is_player)).toHaveLength(1);
    const leader = f.standings.find((s) => s.place === 1);
    expect(leader?.time_behind_leader_s).toBe(0);
  });
});
