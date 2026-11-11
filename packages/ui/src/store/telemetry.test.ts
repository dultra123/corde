import type { GameEvent } from "@corde/protocol";
import { describe, expect, it } from "vitest";
import { freshEvents, useTelemetry } from "./telemetry";

const pit = (lap: number): GameEvent => ({ kind: "pit_entry", lap_number: lap });

describe("freshEvents", () => {
  it("returns what was appended", () => {
    const a = pit(1);
    const b = pit(2);
    const c = pit(3);
    expect(freshEvents([], [a])).toEqual([a]);
    expect(freshEvents([a], [a, b, c])).toEqual([b, c]);
    const same = [a, b];
    expect(freshEvents(same, same)).toEqual([]);
    expect(freshEvents([a, b], [a, b])).toEqual([]);
  });

  it("starts over after a reset", () => {
    const a = pit(1);
    const b = pit(2);
    expect(freshEvents([a], [])).toEqual([]);
    expect(freshEvents([a], [b])).toEqual([b]);
  });

  it("still sees new events once the store's list is full", () => {
    const store = useTelemetry.getState();
    store.resetData();
    for (let i = 0; i < 40; i += 1) useTelemetry.getState().pushEvent(pit(i));
    const before = useTelemetry.getState().events;
    // the list is capped: its length no longer tells what is new
    const stop: GameEvent = { kind: "recording_stopped", session_id: "s1", laps: 4, samples: 9 };
    useTelemetry.getState().pushEvent(stop);
    const after = useTelemetry.getState().events;
    expect(after.length).toBe(before.length);
    expect(freshEvents(before, after)).toEqual([stop]);
    useTelemetry.getState().resetData();
  });
});
