import type { GameEvent, LapRecord, SourceStatus, TelemetryFrame } from "@corde/protocol";
import { create } from "zustand";
import { useShallow } from "zustand/react/shallow";
import { createTraceBuffers, type TraceBuffers } from "../lib/ring-buffer";
import type { SocketState } from "../net/client";

export type DemoMode = "off" | "auto" | "manual";

/** What the top bar shows. */
export type ConnectionPhase =
  | "connecting"
  | "offline"
  | "waiting-game"
  | "monitor"
  | "live"
  | "demo"
  | "demo-auto";

const MAX_LAPS = 60;
const MAX_EVENTS = 30;

export interface TelemetryState {
  frame: TelemetryFrame | null;
  status: SourceStatus | null;
  hello: { serverVersion: string; protocolVersion: number } | null;
  socket: SocketState;
  reconnectAttempt: number;
  demo: DemoMode;
  /** Completed laps, chronological (oldest first). */
  laps: LapRecord[];
  events: GameEvent[];
  /** Measured incoming frame rate. */
  rxHz: number;
  rttMs: number | null;
  /** Mutable, non-reactive sample buffers for traces. */
  buffers: TraceBuffers;
  /** Bumped on every frame so canvas widgets can subscribe cheaply. */
  frameTick: number;

  pushFrame(frame: TelemetryFrame): void;
  pushLap(lap: LapRecord): void;
  pushEvent(event: GameEvent): void;
  setStatus(status: SourceStatus): void;
  setHello(hello: { serverVersion: string; protocolVersion: number; status: SourceStatus }): void;
  setSocket(state: SocketState, attempt: number): void;
  setDemo(mode: DemoMode): void;
  setRates(rxHz: number, rttMs: number | null): void;
  /** Clears frames, laps and buffers (switching between demo and live). */
  resetData(): void;
}

export const useTelemetry = create<TelemetryState>((set, get) => ({
  frame: null,
  status: null,
  hello: null,
  socket: "idle",
  reconnectAttempt: 0,
  demo: "off",
  laps: [],
  events: [],
  rxHz: 0,
  rttMs: null,
  buffers: createTraceBuffers(),
  frameTick: 0,

  pushFrame(frame) {
    const { buffers, frameTick } = get();
    const p = frame.player;
    if (p) {
      const t = frame.wall_ms / 1000;
      buffers.speed.push(t, p.motion.speed_kmh);
      buffers.throttle.push(t, p.inputs.throttle);
      buffers.brake.push(t, p.inputs.brake);
      buffers.steering.push(t, p.inputs.steering);
      buffers.gLat.push(t, p.motion.g_lat);
      buffers.gLong.push(t, p.motion.g_long);
      buffers.rpm.push(t, p.engine.rpm);
      const w = p.wheels;
      buffers.tyreFL.push(t, avg3(w[0].temp_c));
      buffers.tyreFR.push(t, avg3(w[1].temp_c));
      buffers.tyreRL.push(t, avg3(w[2].temp_c));
      buffers.tyreRR.push(t, avg3(w[3].temp_c));
    }
    set({ frame, frameTick: frameTick + 1 });
  },

  pushLap(lap) {
    set((s) => {
      // The server may re-send a lap (reconnect): replace by lap number.
      const without = s.laps.filter((l) => l.lap_number !== lap.lap_number);
      const laps = [...without, lap].sort((a, b) => a.lap_number - b.lap_number);
      return { laps: laps.slice(-MAX_LAPS) };
    });
  },

  pushEvent(event) {
    set((s) => ({ events: [...s.events, event].slice(-MAX_EVENTS) }));
  },

  setStatus(status) {
    set({ status });
  },

  setHello({ serverVersion, protocolVersion, status }) {
    set({ hello: { serverVersion, protocolVersion }, status });
  },

  setSocket(state, attempt) {
    set({ socket: state, reconnectAttempt: attempt });
  },

  setDemo(mode) {
    set({ demo: mode });
  },

  setRates(rxHz, rttMs) {
    set({ rxHz, rttMs });
  },

  resetData() {
    for (const b of Object.values(get().buffers)) b.clear();
    set({ frame: null, laps: [], events: [], frameTick: 0, rxHz: 0 });
  },
}));

/**
 * The events of `next` that `prev` did not hold yet. The list is capped (`MAX_EVENTS`), so its
 * length stops growing after a while: the last event already seen is looked up by identity.
 */
export function freshEvents(prev: readonly GameEvent[], next: readonly GameEvent[]): GameEvent[] {
  if (next === prev) return [];
  const last = prev[prev.length - 1];
  if (last === undefined) return [...next];
  const at = next.lastIndexOf(last);
  return at < 0 ? [...next] : next.slice(at + 1);
}

function avg3(v: readonly [number, number, number]): number {
  return (v[0] + v[1] + v[2]) / 3;
}

/** Derives the headline connection phase shown in the top bar. */
export function connectionPhase(
  s: Pick<TelemetryState, "socket" | "status" | "demo" | "frame">,
): ConnectionPhase {
  if (s.demo === "manual") return "demo";
  if (s.demo === "auto") return "demo-auto";
  if (s.socket !== "open")
    return s.socket === "connecting" || s.socket === "idle" ? "connecting" : "offline";
  if (!s.status?.connected) return "waiting-game";
  const inRealtime = s.status.in_realtime || (s.frame?.session.in_realtime ?? false);
  return inRealtime && s.frame?.player ? "live" : "monitor";
}

export function useConnectionPhase(): ConnectionPhase {
  return useTelemetry(
    useShallow((s) =>
      connectionPhase({ socket: s.socket, status: s.status, demo: s.demo, frame: s.frame }),
    ),
  );
}

/** Convenience selector hooks. */
export const usePlayer = () => useTelemetry((s) => s.frame?.player ?? null);
export const useSession = () => useTelemetry((s) => s.frame?.session ?? null);
export const useStandings = () => useTelemetry((s) => s.frame?.standings ?? EMPTY_STANDINGS);
export const useLaps = () => useTelemetry((s) => s.laps);

const EMPTY_STANDINGS: TelemetryFrame["standings"] = [];
