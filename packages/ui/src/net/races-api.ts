import { request } from "./sessions-api";

/**
 * REST access to the « Courses » page (`crates/corde-server/src/races/`):
 *
 *   GET    /api/races/sources?force=1             RaceSources — the two public schedule pages, as text
 *   POST   /api/races/sources/{id}/unreadable     the page had no race: the server keeps its copy
 *   GET    /api/races/picks                       Pick[]
 *   PUT    /api/races/picks/{id}                  body Pick → Pick · 400 invalid · 409 more than 50
 *   DELETE /api/races/picks/{id}                  204 · 404
 *   GET    /api/races/settings                    RaceSettings
 *   PUT    /api/races/settings                    body Partial<RaceSettings> → RaceSettings
 *
 * Hand-written mirrors of the server's JSON. The HTML of a source is only ever given to the
 * parser (`races/parse.ts`), never to the DOM.
 */

export type SourceId = "racecontrol" | "lmutools";

export interface SourceSnapshot {
  id: SourceId;
  url: string;
  /** `disabled` (setting off) · `ok` · `error` (last attempt failed; `html` = last good copy). */
  state: "disabled" | "ok" | "error";
  fetched_at_ms: number | null;
  attempted_at_ms: number | null;
  error: string | null;
  html: string | null;
}

export interface RaceSources {
  enabled: boolean;
  sources: SourceSnapshot[];
}

export interface RacePick {
  /** `<race_id>@<start_ms>`. */
  id: string;
  race_id: string;
  cat: "daily" | "weekly" | "special" | "champ";
  src: "rc" | "lt";
  name: string;
  level: "beginner" | "intermediate" | "advanced" | null;
  track: string;
  duration: string;
  image: string | null;
  /** The start; for a special event, the reminder of the day (10:00 local). */
  start_ms: number;
  /** Special events: the reminder of the evening before (18:00 local). */
  eve_ms?: number | null;
}

export interface Ratings {
  sr: string;
  dr: string;
}

export type ReqKey = "beginner" | "intermediate" | "advanced" | "weekly" | "special" | "champ";

export interface RaceSettings {
  /** Minutes before the start, largest first. */
  leads: number[];
  levels: string[];
  cats: string[];
  /** Manual correction of the racecontrol.gg times, hours (−2 … +2). */
  hour_fix: number;
  profile: Ratings;
  req: Record<ReqKey, Ratings>;
}

/** What `PUT /api/races/settings` accepts: any key, ratings field by field. */
export type RaceSettingsPatch = Omit<Partial<RaceSettings>, "req" | "profile"> & {
  profile?: Partial<Ratings>;
  req?: Partial<Record<ReqKey, Partial<Ratings>>>;
};

export function fetchRaceSources(force = false, signal?: AbortSignal) {
  // The server may wait for the two sites (20 s each, side by side).
  return request<RaceSources>(`/api/races/sources${force ? "?force=1" : ""}`, { signal }, 45_000);
}

export function reportUnreadable(id: SourceId) {
  return request<{ kept: boolean; file: string }>(
    `/api/races/sources/${id}/unreadable`,
    { method: "POST" },
    10_000,
  );
}

export function fetchPicks(signal?: AbortSignal) {
  return request<RacePick[]>("/api/races/picks", { signal }, 10_000);
}

export function savePick(pick: RacePick) {
  return request<RacePick>(
    `/api/races/picks/${encodeURIComponent(pick.id)}`,
    {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(pick),
    },
    10_000,
  );
}

export function deletePick(id: string) {
  return request<void>(`/api/races/picks/${encodeURIComponent(id)}`, { method: "DELETE" }, 10_000);
}

export function fetchRaceSettings(signal?: AbortSignal) {
  return request<RaceSettings>("/api/races/settings", { signal }, 10_000);
}

export function saveRaceSettings(patch: RaceSettingsPatch) {
  return request<RaceSettings>(
    "/api/races/settings",
    {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(patch),
    },
    10_000,
  );
}
