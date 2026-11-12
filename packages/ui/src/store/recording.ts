import { useEffect } from "react";
import { create } from "zustand";
import {
  ApiError,
  fetchRecording,
  type RecordingStatus,
  startRecording,
  stopRecording,
} from "../net/sessions-api";
import { isHub } from "./hub";
import { freshEvents, useTelemetry } from "./telemetry";

const POLL_MS = 2000;

interface RecordingState {
  status: RecordingStatus | null;
  /** Last fetch error (null when the server answers). */
  error: string | null;
  busy: boolean;
  /** Bumped on every `recording_started` / `recording_stopped` event: lists re-fetch on it. */
  revision: number;

  refresh(): Promise<void>;
  start(): Promise<void>;
  stop(): Promise<void>;
}

export const useRecording = create<RecordingState>((set) => ({
  status: null,
  error: null,
  busy: false,
  revision: 0,

  async refresh() {
    try {
      const status = await fetchRecording();
      set({ status, error: null });
    } catch (e) {
      set({ error: e instanceof ApiError ? e.message : String(e) });
    }
  },

  async start() {
    set({ busy: true });
    try {
      const status = await startRecording();
      set({ status, error: null });
    } catch (e) {
      set({ error: e instanceof ApiError ? e.message : String(e) });
    } finally {
      set({ busy: false });
    }
  },

  async stop() {
    set({ busy: true });
    try {
      const status = await stopRecording();
      set({ status, error: null });
    } catch (e) {
      set({ error: e instanceof ApiError ? e.message : String(e) });
    } finally {
      set({ busy: false });
    }
  },
}));

let watchers = 0;
let timer: ReturnType<typeof setInterval> | null = null;

// The server pushes `recording_started` / `recording_stopped` on the WebSocket (they land in
// the telemetry event list): refresh immediately instead of waiting for the next poll.
let seenEvents = useTelemetry.getState().events;
let seenSocket = useTelemetry.getState().socket;
useTelemetry.subscribe((s) => {
  if (s.socket !== seenSocket) {
    seenSocket = s.socket;
    // the server just answered: do not wait for the next poll to know what it records
    if (s.socket === "open" && timer) void useRecording.getState().refresh();
  }
  if (s.events === seenEvents) return;
  const fresh = freshEvents(seenEvents, s.events);
  seenEvents = s.events;
  if (fresh.some((e) => e.kind === "recording_started" || e.kind === "recording_stopped")) {
    useRecording.setState((st) => ({ revision: st.revision + 1 }));
    void useRecording.getState().refresh();
  }
});

/**
 * No point asking while no server answers (the socket's own reconnection already probes it, and
 * every failed request logs an error in the browser console). The manual demo closes the socket
 * on purpose: the server may well be there.
 */
function worthAsking(): boolean {
  // The home server records nothing (the PC does).
  if (isHub()) return false;
  const { socket, demo } = useTelemetry.getState();
  return socket === "open" || demo === "manual";
}

function startPolling(): void {
  if (timer) return;
  if (worthAsking()) void useRecording.getState().refresh();
  timer = setInterval(() => {
    if (document.visibilityState !== "hidden" && worthAsking()) {
      void useRecording.getState().refresh();
    }
  }, POLL_MS);
}

function stopPolling(): void {
  if (timer) clearInterval(timer);
  timer = null;
}

/** Polls `/api/recording` every 2 s while at least one component is mounted with this hook. */
export function useRecordingWatch(): void {
  useEffect(() => {
    watchers += 1;
    startPolling();
    return () => {
      watchers -= 1;
      if (watchers === 0) stopPolling();
    };
  }, []);
}
