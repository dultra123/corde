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

describe("a rally stage: a line, not a loop", () => {
  /** A stage that goes east: x from 0 to 1000 as the place on the stage goes from 0 to 1. */
  function run(learner: TrackLearner, from = 0.02, to = 1): void {
    for (let i = 0; i <= 500; i += 1) {
      const f = from + ((to - from) * i) / 500;
      learner.learn(1000 * f, 50, f, 1, false);
    }
  }

  it("is known after a run and the start of the next, its two ends left apart", () => {
    const learner = new TrackLearner(100);
    learner.setOpen(true);
    run(learner);
    expect(learner.closed).toBe(false);
    // The run is over (the car out of the run, in the stop zone): the stage is known.
    learner.learn(1030, 50, 0, 1, true);
    expect(learner.closed).toBe(true);
    learner.learn(0, 50, 0, 1, false);
    learner.learn(5, 50, 0.005, 1, false);
    const path = learner.path();
    expect(path).toHaveLength(100);
    // In order from the start to the finish, never back to the start.
    for (let i = 1; i < path.length; i += 1)
      expect(path[i]?.[0] ?? 0).toBeGreaterThanOrEqual(path[i - 1]?.[0] ?? 0);
    expect(path[0]?.[0]).toBeLessThan(30);
    expect(path[99]?.[0]).toBe(1000);
    // The finish line is the end of the line, not the start line again.
    expect(learner.at(1)?.[0]).toBe(1000);
    expect(learner.at(0)?.[0]).toBeLessThan(30);
    expect(Math.abs((learner.at(0.5)?.[0] ?? 0) - 500)).toBeLessThan(15);
    expect(learner.at(7)?.[0]).toBe(1000);
  });

  it("a part never driven at one end takes the nearest known point", () => {
    const learner = new TrackLearner(100);
    learner.setOpen(true);
    // Joined a little after the start: the first tenth was never seen.
    run(learner, 0.1, 1);
    learner.learn(0, 50, 0.0, 1, true);
    learner.learn(110, 50, 0.11, 1, false);
    expect(learner.closed).toBe(true);
    expect(Math.abs((learner.at(0)?.[0] ?? 0) - 100)).toBeLessThan(15);
    expect(Math.abs((learner.at(0.05)?.[0] ?? 0) - 100)).toBeLessThan(15);
  });

  it("a loop still goes round, and changing kind forgets what was learnt", () => {
    const learner = new TrackLearner(100);
    run(learner, 0, 0.999);
    learner.learn(0, 50, 0.001, 1, false);
    expect(learner.closed).toBe(true);
    // Around the loop: the place 1 is the start line.
    expect(learner.at(1)?.[0]).toBeLessThan(30);
    learner.setOpen(true);
    expect(learner.closed).toBe(false);
    expect(learner.path()).toHaveLength(0);
    learner.setOpen(true);
    run(learner);
    expect(learner.progress).toBeGreaterThan(0.9);
  });
});
