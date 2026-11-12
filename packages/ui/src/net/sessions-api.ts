import type { GameId, LapRecord, SessionType, VehicleClass, Weather } from "@corde/protocol";
import { authHeaders } from "./auth";
import { notifyUnauthorized } from "./auth-events";
import { connection } from "./connection";

/**
 * REST access to the recorder (`crates/corde-store`, served by corde-server):
 *
 *   GET    /api/sessions?limit=&track=&type=           SessionRow[] (most recent first)
 *   GET    /api/sessions/{id}                          SessionDetail (laps, events, setups, files)
 *   GET    /api/sessions/{id}/laps/{n}/samples?channels=&points=   LapSamples
 *   DELETE /api/sessions/{id}                          204 · 409 while being recorded
 *   GET    /api/recording · POST /api/recording/start|stop        RecordingStatus
 *   GET    /api/storage                                StorageStats
 *   GET    /api/channels                               ChannelInfo[]
 *
 * Every call throws an `ApiError` (with the HTTP status and the server's `error` text) so the
 * pages can show a real message instead of a blank screen.
 */

export interface SessionRow {
  id: string;
  game: GameId;
  track_name: string;
  track_length_m: number;
  session_type: SessionType;
  session_index: number;
  started_wall_ms: number;
  ended_wall_ms: number | null;
  player_name: string;
  vehicle_name: string;
  vehicle_model: string;
  vehicle_class: VehicleClass;
  class_name: string;
  server_name: string;
  is_multiplayer: boolean;
  game_version: string | null;
  source: string;
  record_rate_hz: number;
  sample_count: number;
  lap_count: number;
  best_lap_s: number | null;
  weather: Weather;
  notes: string | null;
}

export interface LapRow extends LapRecord {
  session_id: string;
  sample_file: string;
  sample_row_start: number;
  sample_row_end: number;
  /** `live` (read while driving) or `results` (official times from the game's result file). */
  time_source?: "live" | "results";
}

export interface EventRow {
  id: number;
  session_id: string;
  wall_ms: number;
  session_elapsed_s: number;
  lap_number: number;
  kind: string;
  payload: Record<string, unknown>;
}

export interface SetupRow {
  id: number;
  session_id: string;
  captured_wall_ms: number;
  source: string;
  name: string;
  content: unknown;
}

export interface SampleFileRow {
  session_id: string;
  file_index: number;
  name: string;
  row_start: number;
  row_end: number;
  bytes: number;
  closed: boolean;
}

export interface SessionDetail extends SessionRow {
  laps: LapRow[];
  events: EventRow[];
  setups: SetupRow[];
  files: SampleFileRow[];
}

export interface ChannelSeries {
  name: string;
  unit: string;
  values: (number | null)[];
}

export interface LapSamples {
  session_id: string;
  lap_number: number;
  total_rows: number;
  points: number;
  /** Session time, seconds (absolute — subtract the first value for a lap clock). */
  t_s: number[];
  lap_dist_m: number[];
  channels: ChannelSeries[];
}

export interface RecordingStatus {
  recording: boolean;
  auto: boolean;
  armed: boolean;
  session_id: string | null;
  started_wall_ms: number | null;
  rows: number;
  laps: number;
  bytes: number;
  files: number;
  record_rate_hz: number;
  last_flush_wall_ms: number | null;
  idle_reason: string | null;
  last_stop_reason: string | null;
}

export interface StorageStats {
  path: string;
  sessions: number;
  files: number;
  db_bytes: number;
  samples_bytes: number;
  total_bytes: number;
}

export interface ChannelInfo {
  name: string;
  unit: string;
  kind: string;
}

export interface SessionsFilter {
  limit?: number;
  track?: string;
  type?: SessionType | "";
}

export class ApiError extends Error {
  constructor(
    readonly status: number,
    message: string,
  ) {
    super(message);
    this.name = "ApiError";
  }
}

function base(): string {
  return connection.endpoint.http;
}

/** One JSON call to the server; shared with `profile-api`. */
export async function request<T>(path: string, init?: RequestInit, timeoutMs = 15_000): Promise<T> {
  // iPhone application with no active server: nothing to call.
  if (!connection.hasEndpoint) throw new ApiError(0, "no server");
  const ctrl = new AbortController();
  const timer = setTimeout(() => ctrl.abort(), timeoutMs);
  const outer = init?.signal;
  const onOuterAbort = () => ctrl.abort();
  outer?.addEventListener("abort", onOuterAbort);
  try {
    const res = await fetch(`${base()}${path}`, {
      ...init,
      signal: ctrl.signal,
      headers: { Accept: "application/json", ...authHeaders(), ...(init?.headers ?? {}) },
    });
    if (!res.ok) {
      // Home server: this device is not paired (any more) → the pairing screen.
      if (res.status === 401) notifyUnauthorized();
      let msg = `${res.status} ${res.statusText}`;
      try {
        const body = (await res.json()) as { error?: unknown };
        if (typeof body.error === "string" && body.error) msg = body.error;
      } catch {
        // no JSON body
      }
      throw new ApiError(res.status, msg);
    }
    if (res.status === 204) return undefined as T;
    return (await res.json()) as T;
  } catch (e) {
    if (e instanceof ApiError) throw e;
    if (e instanceof DOMException && e.name === "AbortError") {
      throw new ApiError(0, outer?.aborted ? "aborted" : "timeout");
    }
    throw new ApiError(0, e instanceof Error ? e.message : String(e));
  } finally {
    clearTimeout(timer);
    outer?.removeEventListener("abort", onOuterAbort);
  }
}

export function fetchSessions(filter: SessionsFilter = {}, signal?: AbortSignal) {
  const q = new URLSearchParams();
  if (filter.limit) q.set("limit", String(filter.limit));
  if (filter.track) q.set("track", filter.track);
  if (filter.type) q.set("type", filter.type);
  const qs = q.toString();
  return request<SessionRow[]>(`/api/sessions${qs ? `?${qs}` : ""}`, { signal });
}

export function fetchSession(id: string, signal?: AbortSignal) {
  return request<SessionDetail>(`/api/sessions/${encodeURIComponent(id)}`, { signal });
}

export function fetchLapSamples(
  id: string,
  lap: number,
  channels: readonly string[],
  points: number,
  signal?: AbortSignal,
) {
  const q = new URLSearchParams({ channels: channels.join(","), points: String(points) });
  return request<LapSamples>(
    `/api/sessions/${encodeURIComponent(id)}/laps/${lap}/samples?${q.toString()}`,
    { signal },
    30_000,
  );
}

export function deleteSession(id: string) {
  return request<void>(`/api/sessions/${encodeURIComponent(id)}`, { method: "DELETE" });
}

export function fetchRecording(signal?: AbortSignal) {
  return request<RecordingStatus>("/api/recording", { signal }, 5_000);
}

export function startRecording() {
  return request<RecordingStatus>("/api/recording/start", { method: "POST" }, 5_000);
}

export function stopRecording() {
  return request<RecordingStatus>("/api/recording/stop", { method: "POST" }, 5_000);
}

export function fetchStorage(signal?: AbortSignal) {
  return request<StorageStats>("/api/storage", { signal }, 5_000);
}

let channelsCache: Promise<ChannelInfo[]> | null = null;

/** The Parquet column list; cached for the page's lifetime (it only changes with the server). */
export function fetchChannels(): Promise<ChannelInfo[]> {
  if (!channelsCache) {
    channelsCache = request<ChannelInfo[]>("/api/channels").catch((e) => {
      channelsCache = null;
      throw e;
    });
  }
  return channelsCache;
}
