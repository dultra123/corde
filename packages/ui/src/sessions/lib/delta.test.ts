import { describe, expect, it } from "vitest";
import {
  deltaTime,
  interpolateAt,
  lapClock,
  lapStartTime,
  lowerIndex,
  monotonicIndices,
  pick,
  resample,
  splitSign,
} from "./delta";

describe("monotonicIndices / pick", () => {
  it("keeps only strictly increasing finite values", () => {
    const xs = [0, 10, 10, 9, 20, Number.NaN, 30];
    expect(monotonicIndices(xs)).toEqual([0, 1, 4, 6]);
    expect(pick(["a", "b", "c", "d", "e", "f", "g"], [0, 1, 4, 6])).toEqual(["a", "b", "e", "g"]);
  });
});

describe("lowerIndex / interpolateAt", () => {
  const xs = [0, 10, 20, 30];
  const ys = [0, 100, 200, 300];
  it("finds the bracketing sample", () => {
    expect(lowerIndex(xs, -1)).toBe(-1);
    expect(lowerIndex(xs, 0)).toBe(0);
    expect(lowerIndex(xs, 15)).toBe(1);
    expect(lowerIndex(xs, 30)).toBe(3);
    expect(lowerIndex(xs, 99)).toBe(3);
  });
  it("interpolates linearly, holds for steps, nulls outside and around gaps", () => {
    expect(interpolateAt(xs, ys, 15)).toBe(150);
    expect(interpolateAt(xs, ys, 10)).toBe(100);
    expect(interpolateAt(xs, ys, 30)).toBe(300);
    expect(interpolateAt(xs, ys, -5)).toBeNull();
    expect(interpolateAt(xs, ys, 15, "step")).toBe(100);
    expect(interpolateAt(xs, [0, null, 200, 300], 15)).toBeNull();
    expect(interpolateAt(xs, [0, null, 200, 300], 5)).toBeNull();
    expect(interpolateAt([], [], 5)).toBeNull();
  });
  it("resamples on a grid", () => {
    expect(resample(xs, ys, [5, 25, 40])).toEqual([50, 250, null]);
  });
});

describe("lapStartTime / lapClock", () => {
  it("extrapolates the start back to distance 0", () => {
    // 50 m/s: first sample 1 m after the line, 0.02 s after the start
    const dist = [1, 2, 3];
    const t = [100.02, 100.04, 100.06];
    expect(lapStartTime(dist, t)).toBeCloseTo(100, 9);
    expect(lapClock(dist, t)[0]).toBeCloseTo(0.02, 9);
  });
  it("falls back to the first stamp when it cannot extrapolate", () => {
    expect(lapStartTime([0, 1], [5, 6])).toBe(5);
    expect(lapStartTime([3], [5])).toBe(5);
    expect(lapStartTime([], [])).toBe(0);
  });
});

describe("deltaTime", () => {
  it("is positive when A is slower (behind) and negative when A is ahead", () => {
    // A: 10 m/s constant, B: 20 m/s constant over 100 m
    const a = { dist: [0, 50, 100], t: [0, 5, 10] };
    const b = { dist: [0, 25, 50, 100], t: [0, 1.25, 2.5, 5] };
    expect(deltaTime(a, b)).toEqual([0, 2.5, 5]);
    expect(deltaTime(b, a)).toEqual([0, -1.25, -2.5, -5]);
  });
  it("interpolates B between its samples and nulls where B has no data", () => {
    const a = { dist: [0, 10, 20, 30], t: [0, 1, 2, 3] };
    const b = { dist: [0, 20], t: [0, 1] }; // 20 m/s, stops at 20 m
    expect(deltaTime(a, b)).toEqual([0, 0.5, 1, null]);
  });
});

describe("splitSign", () => {
  it("separates the two halves and keeps gaps", () => {
    expect(splitSign([-1, 0, 2, null])).toEqual({
      neg: [-1, 0, null, null],
      pos: [null, 0, 2, null],
    });
  });
  it("closes both halves at zero across a sign change", () => {
    expect(splitSign([-2, -1, 1, 2])).toEqual({
      neg: [-2, -1, 0, null],
      pos: [null, 0, 1, 2],
    });
    expect(splitSign([1, -1])).toEqual({ neg: [0, -1], pos: [1, 0] });
  });
});
