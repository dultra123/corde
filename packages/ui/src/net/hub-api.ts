import { request } from "./sessions-api";

/**
 * REST access to the home server (« Corde serveur », `corde-server --role hub`, ADR 0010) and,
 * on the PC, to the link with it:
 *
 *   Home server
 *   GET    /api/hub/pairing          HubPairing — public (the only route with POST /api/hub/pair)
 *   POST   /api/hub/pair             {code, name} → 201 {device} + cookie · 400 · 429
 *   GET    /api/hub/status           HubStatus
 *   GET    /api/hub/devices          {devices: HubDevice[]}
 *   PATCH  /api/hub/devices/{id}     {name?, admin?}
 *   DELETE /api/hub/devices/{id}     204 (immediate)
 *
 *   PC
 *   GET    /api/hub-link             HubLinkStatus (never the token)
 *   PUT    /api/hub-link             {url}
 *   POST   /api/hub-link/pair        {url, code, name?}
 *   POST   /api/hub-link/sync        202 · POST /api/hub-link/resend 202
 *   DELETE /api/hub-link             {revoked_on_server}
 *
 * On a PC the `/api/hub/*` routes do not exist (404): that is how the page tells the two apart.
 */

export interface HubDeviceRef {
  id: string;
  name: string;
  admin: boolean;
}

export interface HubPairing {
  role: "hub";
  version: string;
  paired: boolean;
  device: HubDeviceRef | null;
}

export interface HubDevice {
  id: string;
  name: string;
  kind: "browser" | "app";
  admin: boolean;
  created_ms: number;
  last_used_ms: number | null;
  /** This very device. */
  current: boolean;
}

export interface LastPc {
  device_id: string;
  device_name: string;
  version: string;
  /** Where that PC serves the live view on the home network (empty: on itself only). */
  urls: string[];
  synced_ms: number;
}

export interface HubStatus {
  version: string;
  schema_version: number;
  sync_version: number;
  uptime_s: number;
  sessions: number;
  results: number;
  devices: number;
  last_pc: LastPc | null;
  you: HubDeviceRef;
}

export type HubLinkState =
  | "unpaired"
  | "connected"
  | "syncing"
  | "unreachable"
  | "server_outdated"
  | "pc_outdated"
  | "revoked"
  | "error";

export interface SyncReport {
  results_sent: number;
  sessions_sent: number;
  files_sent: number;
  chunks_sent: number;
  bytes_sent: number;
  feedback_pushed: number;
  feedback_pulled: number;
}

export interface HubLinkStatus {
  url: string | null;
  paired: boolean;
  device_name: string | null;
  state: HubLinkState;
  message: string | null;
  last_sync_ms: number | null;
  last_attempt_ms: number | null;
  pending_sessions: number | null;
  pending_results: number | null;
  server_version: string | null;
  last_report: SyncReport | null;
}

const JSON_HEADERS = { "Content-Type": "application/json" };

export function fetchHubPairing(signal?: AbortSignal) {
  return request<HubPairing>("/api/hub/pairing", { signal }, 5_000);
}

export function pairThisDevice(code: string, name: string) {
  return request<{ device: HubDeviceRef }>(
    "/api/hub/pair",
    { method: "POST", headers: JSON_HEADERS, body: JSON.stringify({ code, name }) },
    10_000,
  );
}

export function fetchHubStatus(signal?: AbortSignal) {
  return request<HubStatus>("/api/hub/status", { signal }, 5_000);
}

export function fetchDevices(signal?: AbortSignal) {
  return request<{ devices: HubDevice[] }>("/api/hub/devices", { signal }, 5_000);
}

export function updateDevice(id: string, patch: { name?: string; admin?: boolean }) {
  return request<HubDevice>(
    `/api/hub/devices/${encodeURIComponent(id)}`,
    { method: "PATCH", headers: JSON_HEADERS, body: JSON.stringify(patch) },
    5_000,
  );
}

export function revokeDevice(id: string) {
  return request<void>(`/api/hub/devices/${encodeURIComponent(id)}`, { method: "DELETE" }, 5_000);
}

export function fetchHubLink(signal?: AbortSignal) {
  return request<HubLinkStatus>("/api/hub-link", { signal }, 5_000);
}

export function setHubUrl(url: string) {
  return request<HubLinkStatus>(
    "/api/hub-link",
    { method: "PUT", headers: JSON_HEADERS, body: JSON.stringify({ url }) },
    5_000,
  );
}

export function pairHubLink(url: string, code: string) {
  return request<HubLinkStatus>(
    "/api/hub-link/pair",
    { method: "POST", headers: JSON_HEADERS, body: JSON.stringify({ url, code }) },
    20_000,
  );
}

export function syncHubLink() {
  return request<HubLinkStatus>("/api/hub-link/sync", { method: "POST" }, 5_000);
}

export function resendHubLink() {
  return request<HubLinkStatus>("/api/hub-link/resend", { method: "POST" }, 5_000);
}

export function unpairHubLink() {
  return request<{ revoked_on_server: boolean }>("/api/hub-link", { method: "DELETE" }, 10_000);
}

/**
 * A default name for this device, from the browser (a product name, sent to the server only);
 * `null` when unknown (the page then proposes a translated generic name).
 */
export function guessDeviceName(ua: string = navigator.userAgent): string | null {
  if (/iPhone/i.test(ua)) return "iPhone";
  if (/iPad/i.test(ua)) return "iPad";
  if (/Android/i.test(ua)) return "Android";
  if (/Macintosh/i.test(ua)) return "Mac";
  if (/Windows/i.test(ua)) return "Windows";
  if (/Linux/i.test(ua)) return "Linux";
  return null;
}

/** Keeps the digits of a typed code: `1234 5678`, `1234-5678` → `12345678`. */
export function codeDigits(input: string): string {
  return input.replace(/\D/g, "").slice(0, 8);
}

/** `12345678` → `1234 5678` (while typing too). */
export function formatCode(input: string): string {
  const d = codeDigits(input);
  return d.length > 4 ? `${d.slice(0, 4)} ${d.slice(4)}` : d;
}
