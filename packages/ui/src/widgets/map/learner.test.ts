import { describe, expect, it } from "vitest";
import { carPoint, hasCoordinates, TrackLearner } from "./learner";

/** A round circuit of radius 100, driven once and a little more. */
function learnt(): TrackLearner {
  const learner = new TrackLearner(120);
  for (let i = 0; i <= 130; i += 1) {
    // The middle of each bin.
    const f = ((i + 0.5) / 120) % 1;
    const a = f * Math.PI * 2;
    learner.learn(100 * Math.cos(a), 100 * Math.sin(a), f, 1, false);
  }
  return learner;
}

describe("the cars on the map", () => {
  it("uses the coordinates the game gives", () => {
    const learner = new TrackLearner();
    expect(hasCoordinates([12, 0, -7])).toBe(true);
    expect(hasCoordinates([0, 3, 0])).toBe(false);
    expect(carPoint(learner, [12, 5, -7], 0.5)).toEqual([12, -7]);
  });

  it("places a car without coordinates by where it is on the lap, once the loop is known", () => {
    const fresh = new TrackLearner();
    expect(carPoint(fresh, [0, 0, 0], 0.25)).toBeNull();
    const learner = learnt();
    expect(learner.closed).toBe(true);
    const at = carPoint(learner, [0, 0, 0], 0.25);
    expect(at).not.toBeNull();
    // A quarter of the lap: the top of the circle, to a bin.
    expect(Math.abs(at?.[0] ?? 99)).toBeLessThan(5);
    expect(at?.[1]).toBeGreaterThan(99);
    expect(carPoint(learner, [0, 0, 0], Number.NaN)).toBeNull();
  });
});
