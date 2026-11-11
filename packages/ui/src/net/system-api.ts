import { request } from "./sessions-api";

/**
 * REST access to the server's own settings and network state:
 *
 *   GET /api/settings    ServerSettings — `<data-dir>/settings.json`
 *   PUT /api/settings    body { lan_access?, online_schedule? } → SavedSettings · 400 · 403 not from this PC
 *   GET /api/network     NetworkInfo
 *   GET /api/health      Health
 *
 * The listening address is chosen when the server starts: changing `lan_access` only takes
 * effect after a restart (`restart_required`).
 */

export interface ServerSettings {
  /** Listen on the local network (phone, other PC) instead of this PC only. */
  lan_access: boolean;
  /** Read the public race schedule pages (racecontrol.gg, lmu.tools) for the « Courses » page. */
  online_schedule: boolean;
}

export interface SavedSettings extends ServerSettings {
  /** `true` when the saved setting differs from what the running server is doing. */
  restart_required: boolean;
}

export interface NetworkInfo {
  /** The saved setting: what the next start will do. */
  lan_access_setting: boolean;
  /** What the running server does right now. */
  listening_on_lan: boolean;
  port: number;
  /** `http://<ip>:<port>` addresses to open on the phone; empty when listening on this PC only. */
  urls: string[];
}

export interface Health {
  ok: boolean;
  version: string;
  protocol_version: number;
  uptime_s: number;
}

export function fetchSettings(signal?: AbortSignal) {
  return request<ServerSettings>("/api/settings", { signal }, 5_000);
}

export function saveSettings(patch: Partial<ServerSettings>) {
  return request<SavedSettings>(
    "/api/settings",
    {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(patch),
    },
    5_000,
  );
}

export function fetchNetwork(signal?: AbortSignal) {
  return request<NetworkInfo>("/api/network", { signal }, 5_000);
}

export function fetchHealth(signal?: AbortSignal) {
  return request<Health>("/api/health", { signal }, 5_000);
}
