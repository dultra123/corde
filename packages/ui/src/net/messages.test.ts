import { describe, expect, it } from "vitest";
import { DemoRace } from "../demo/generator";
import { encodeClientMessage, parseServerMessage } from "./messages";
import { normaliseHost } from "./url";

const status = {
  connected: true,
  game: "lmu",
  source: "lmu-shm",
  input_rate_hz: 100,
  output_rate_hz: 20,
  in_realtime: true,
  game_version: "1.3",
  last_error: null,
  clients: 1,
  recording: false,
};

describe("parseServerMessage", () => {
  it("parses hello", () => {
    const m = parseServerMessage(
      JSON.stringify({ type: "hello", server_version: "0.1.0", protocol_version: 1, status }),
    );
    expect(m).toMatchObject({ kind: "hello", serverVersion: "0.1.0", protocolVersion: 1 });
    expect(m && m.kind === "hello" ? m.status.connected : null).toBe(true);
  });

  it("parses a flattened status", () => {
    const m = parseServerMessage(JSON.stringify({ type: "status", ...status }));
    expect(m?.kind).toBe("status");
    expect(m && m.kind === "status" ? m.status.game : null).toBe("lmu");
    expect(m && m.kind === "status" ? "type" in m.status : null).toBe(false);
  });

  it("parses a flattened frame from the demo generator", () => {
    const race = new DemoRace({ seed: 1 });
    const frame = race.step(0.05).frame;
    const wire = JSON.stringify({ type: "frame", ...frame });
    const m = parseServerMessage(wire);
    expect(m?.kind).toBe("frame");
    if (m?.kind !== "frame") throw new Error("frame expected");
    expect(m.frame.seq).toBe(frame?.seq);
    expect(m.frame.standings).toHaveLength(20);
    expect(m.frame.player?.wheels[0].temp_c).toHaveLength(3);
    expect("type" in m.frame).toBe(false);
  });

  it("parses a lap and an event", () => {
    const lap = parseServerMessage(
      JSON.stringify({ type: "lap", lap_number: 3, lap_time_s: 112.5, valid: true }),
    );
    expect(lap).toMatchObject({ kind: "lap", lap: { lap_number: 3 } });
    const ev = parseServerMessage(
      JSON.stringify({ type: "event", kind: "pit_entry", lap_number: 4 }),
    );
    expect(ev).toMatchObject({ kind: "event", event: { kind: "pit_entry", lap_number: 4 } });
    expect(parseServerMessage(JSON.stringify({ type: "pong", t: 12 }))).toEqual({
      kind: "pong",
      t: 12,
    });
  });

  it("drops junk instead of throwing", () => {
    expect(parseServerMessage("not json")).toBeNull();
    expect(parseServerMessage("[1,2]")).toBeNull();
    expect(parseServerMessage(JSON.stringify({ type: "future_thing", x: 1 }))).toBeNull();
    expect(parseServerMessage(JSON.stringify({ type: "frame" }))).toBeNull();
    expect(parseServerMessage(JSON.stringify({ type: "status" }))).toBeNull();
    expect(parseServerMessage(null)).toBeNull();
  });

  it("encodes client messages", () => {
    expect(JSON.parse(encodeClientMessage({ type: "set_rate", hz: 30 }))).toEqual({
      type: "set_rate",
      hz: 30,
    });
  });
});

describe("normaliseHost", () => {
  it("adds the default port and strips schemes / paths", () => {
    expect(normaliseHost("192.168.1.20")).toBe("192.168.1.20:8471");
    expect(normaliseHost("pc-jeu:9000")).toBe("pc-jeu:9000");
    expect(normaliseHost("http://pc:8471/")).toBe("pc:8471");
    expect(normaliseHost("ws://pc:8471/ws")).toBe("pc:8471");
    expect(normaliseHost("   ")).toBeNull();
  });
});
