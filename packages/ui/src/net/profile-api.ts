import type { FinishStatus, SessionType } from "@corde/protocol";
import { request } from "./sessions-api";

/**
 * REST access to the driver profile built from the game's results files
 * (`UserData/Log/Results/*.xml`, imported by corde-server):
 *
 *   GET  /api/profile                                Profile (totals, tracks, vehicles, recent, import)
 *   POST /api/profile/import                         scan the results folder now → ImportStatus
 *   GET  /api/results?track=&kind=&vehicle=&limit=   ResultRow[] (most recent first)
 *   GET  /api/results/{id}                           ResultDetail (player laps + the whole parsed file)
 *
 * These types are hand-written mirrors of the server's JSON (`crates/corde-store/src/results.rs`,
 * `crates/corde-server/src/results_import.rs`, `crates/corde-lmu/src/results.rs`); they are not
 * part of the shared protocol package.
 */

export interface ResultRow {
  id: number;
  file_name: string;
  file_mtime_ms: number;
  imported_wall_ms: number;
  game_version: string | null;
  track_venue: string;
  track_course: string;
  track_length_m: number;
  session_kind: SessionType;
  session_name: string;
  started_wall_ms: number;
  minutes: number | null;
  laps_planned: number | null;
  setting: string | null;
  server_name: string | null;
  player_name: string;
  player_position: number | null;
  player_class_position: number | null;
  player_grid_pos: number | null;
  player_class_grid_pos: number | null;
  player_class: string;
  /** Car model (`Mercedes-AMG LMGT3`). */
  player_vehicle: string;
  /** Livery / entry name. */
  player_vehicle_name: string;
  player_best_lap_s: number | null;
  player_laps: number;
  player_finish: FinishStatus;
  num_drivers: number;
  num_class_drivers: number;
}

export interface ResultLapRow {
  result_id: number;
  driver_name: string;
  is_player: boolean;
  lap_number: number;
  position: number | null;
  elapsed_s: number | null;
  lap_time_s: number | null;
  s1: number | null;
  s2: number | null;
  s3: number | null;
  top_speed_kmh: number | null;
  fuel_fraction: number | null;
  front_compound: string | null;
  rear_compound: string | null;
  tyre_wear: [number, number, number, number] | null;
  pit: boolean;
}

/** `corde_lmu::results::ResultLap` — a lap of any driver inside the parsed file. */
export interface FileLap {
  num: number;
  position: number | null;
  elapsed_s: number | null;
  lap_time_s: number | null;
  s1: number | null;
  s2: number | null;
  s3: number | null;
  top_speed_kmh: number | null;
  fuel_fraction: number | null;
  front_compound: string | null;
  rear_compound: string | null;
  tyre_wear: [number, number, number, number] | null;
  pit: boolean;
}

export interface IncidentSummary {
  contacts: number;
  contacts_with_vehicles: number;
  track_limit_warnings: number;
  invalid_laps: number;
  penalties: number;
}

/** `corde_lmu::results::ResultDriver` — one entry of the classification. */
export interface FileDriver {
  name: string;
  vehicle_name: string;
  vehicle_file: string;
  car_type: string;
  car_class: string;
  category: string | null;
  car_number: string | null;
  team: string;
  is_player: boolean;
  connected: boolean | null;
  server_scored: boolean | null;
  grid_pos: number | null;
  position: number | null;
  class_grid_pos: number | null;
  class_position: number | null;
  points: number | null;
  class_points: number | null;
  finish_status: string;
  finish: FinishStatus;
  dnf_reason: string | null;
  best_lap_s: number | null;
  finish_time_s: number | null;
  laps_completed: number;
  pitstops: number;
  aids: { start_lap: number; end_lap: number; text: string }[];
  laps: FileLap[];
  incidents: IncidentSummary;
}

export interface StreamEvent {
  kind: "contact" | "track_limits" | "penalty";
  et_s: number;
  driver: string;
  lap: number | null;
  other: string | null;
  text: string;
}

/** `corde_lmu::results::RaceResultsFile` — the parsed XML, as stored. */
export interface ResultsFile {
  game_version: string | null;
  setting: string | null;
  server_name: string | null;
  player_file: string | null;
  file_date_time: number | null;
  track_venue: string;
  track_course: string;
  track_event: string | null;
  track_length_m: number;
  mod_name: string | null;
  session_name: string;
  session_kind: SessionType;
  date_time: number;
  time_string: string | null;
  minutes: number | null;
  laps_planned: number | null;
  most_laps_completed: number | null;
  drivers: FileDriver[];
  /** Only the player's events are kept. */
  events: StreamEvent[];
}

export interface ResultDetail extends ResultRow {
  laps: ResultLapRow[];
  file: ResultsFile;
  /** Recorded Corde session of the same track that started within ±10 min. */
  linked_session_id: string | null;
}

export interface ProfileTotals {
  sessions: number;
  races: number;
  qualifyings: number;
  practices: number;
  laps: number;
  km: number;
  wins: number;
  podiums: number;
  class_wins: number;
  class_podiums: number;
  dnfs: number;
}

export interface TrackSummary {
  track_venue: string;
  track_course: string;
  track_length_m: number;
  sessions: number;
  races: number;
  laps: number;
  km: number;
  best_lap_s: number | null;
  best_lap_vehicle: string | null;
  best_lap_date: number | null;
  best_lap_result_id: number | null;
  best_class_position: number | null;
  avg_class_position: number | null;
  last_date: number;
}

export interface VehicleSummary {
  vehicle_name: string;
  car_class: string;
  sessions: number;
  races: number;
  laps: number;
  km: number;
  best_lap_s: number | null;
  best_track: string | null;
  best_lap_date: number | null;
  best_lap_result_id: number | null;
  last_date: number;
}

export interface ImportStatus {
  enabled: boolean;
  results_dir: string | null;
  dir_source: "cli" | "game" | "steam" | null;
  last_scan_ms: number | null;
  scanning: boolean;
  files_seen: number;
  files_imported: number;
  total_imported: number;
  files_skipped: number;
  skipped: { file_name: string; reason: string }[];
  player_name: string | null;
  error: string | null;
}

export interface Profile {
  player_name: string | null;
  totals: ProfileTotals;
  tracks: TrackSummary[];
  vehicles: VehicleSummary[];
  recent: ResultRow[];
  import: ImportStatus;
}

export interface ResultsFilter {
  limit?: number;
  track?: string;
  kind?: SessionType | "";
  vehicle?: string;
}

export function fetchProfile(signal?: AbortSignal) {
  return request<Profile>("/api/profile", { signal });
}

/** Scans the results folder now (re-examining skipped files); can take a few seconds. */
export function importResults() {
  return request<ImportStatus>("/api/profile/import", { method: "POST" }, 120_000);
}

export function fetchResults(filter: ResultsFilter = {}, signal?: AbortSignal) {
  const q = new URLSearchParams();
  if (filter.limit) q.set("limit", String(filter.limit));
  if (filter.track) q.set("track", filter.track);
  if (filter.kind) q.set("kind", filter.kind);
  if (filter.vehicle) q.set("vehicle", filter.vehicle);
  const qs = q.toString();
  return request<ResultRow[]>(`/api/results${qs ? `?${qs}` : ""}`, { signal });
}

export function fetchResult(id: number, signal?: AbortSignal) {
  return request<ResultDetail>(`/api/results/${id}`, { signal }, 30_000);
}
