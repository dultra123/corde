/**
 * Hooks into the desktop shell (`apps/desktop`, Tauri 2).
 *
 * The shell loads this same UI from `http://127.0.0.1:<port>` with `withGlobalTauri`, which puts
 * `window.__TAURI__.core.invoke` on the page. In a plain browser — a phone, a second PC, the dev
 * server — that object is absent: `isDesktop()` is false and every call rejects, so callers hide
 * the "Application" controls and show an instruction instead.
 *
 * The command names and their arguments are the contract with the shell — do not rename them:
 *
 *   get_autostart                -> boolean   Corde starts with Windows (minimised to the tray)
 *   set_autostart { enabled }                 turn that on or off
 *   restart_app                               quit and start again (applies the network setting)
 *   app_version                  -> string    version of the desktop application
 *   update_status                -> UpdateStatus   updates: installed version, last check, …
 *   check_update                 -> UpdateStatus   look for a newer version now
 *   install_update                            download, then Corde quits and the installer
 *                                             restarts it (never settles on success; rejects
 *                                             with a message while the game runs / records)
 *   open_releases_page                        the releases page, in the default browser
 *   get_update_auto_check        -> boolean   check automatically (on by default)
 *   set_update_auto_check { enabled }
 *
 * The shell also emits the `update-status` event (payload: an `UpdateStatus`) on every change
 * of the update state; `onUpdateStatus` listens to it through `window.__TAURI__.event.listen`.
 *
 * No `@tauri-apps/api` dependency: only the minimal shape used here is declared.
 */

/** The part of `window.__TAURI__` this UI relies on. */
export interface TauriGlobal {
  core?: {
    invoke?: (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;
  };
  event?: {
    listen?: (event: string, handler: (event: { payload: unknown }) => void) => Promise<() => void>;
  };
}

declare global {
  interface Window {
    __TAURI__?: TauriGlobal;
  }
}

export const DESKTOP_COMMANDS = {
  getAutostart: "get_autostart",
  setAutostart: "set_autostart",
  restartApp: "restart_app",
  appVersion: "app_version",
  updateStatus: "update_status",
  checkUpdate: "check_update",
  installUpdate: "install_update",
  openReleasesPage: "open_releases_page",
  getUpdateAutoCheck: "get_update_auto_check",
  setUpdateAutoCheck: "set_update_auto_check",
} as const;

/** Event emitted by the shell on every change of the update state (payload: `UpdateStatus`). */
export const UPDATE_EVENT = "update-status";

/** A shell command that failed, was refused, or answered something unexpected. */
export class DesktopError extends Error {
  constructor(
    readonly command: string,
    message: string,
  ) {
    super(message);
    this.name = "DesktopError";
  }
}

type Invoke = (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;

function invoker(): Invoke | null {
  if (typeof window === "undefined") return null;
  try {
    const core = window.__TAURI__?.core;
    const invoke = core?.invoke;
    return typeof invoke === "function" ? invoke.bind(core) : null;
  } catch {
    return null;
  }
}

/** `true` inside the desktop application, `false` in a browser. */
export function isDesktop(): boolean {
  return invoker() !== null;
}

/** Tauri rejects with whatever the Rust command returned as an error: usually a string. */
function reason(e: unknown): string {
  if (e instanceof Error) return e.message;
  if (typeof e === "string") return e;
  if (typeof e === "object" && e !== null && "message" in e) {
    const m = (e as { message: unknown }).message;
    if (typeof m === "string") return m;
  }
  try {
    return JSON.stringify(e) ?? String(e);
  } catch {
    return String(e);
  }
}

async function call(command: string, args?: Record<string, unknown>): Promise<unknown> {
  const invoke = invoker();
  if (!invoke) throw new DesktopError(command, "not running in the desktop application");
  try {
    return await invoke(command, args);
  } catch (e) {
    throw new DesktopError(command, reason(e));
  }
}

/** Does Corde start with Windows (minimised to the notification area)? */
export async function getAutostart(): Promise<boolean> {
  const v = await call(DESKTOP_COMMANDS.getAutostart);
  if (typeof v !== "boolean") {
    throw new DesktopError(DESKTOP_COMMANDS.getAutostart, `unexpected answer: ${reason(v)}`);
  }
  return v;
}

export async function setAutostart(enabled: boolean): Promise<void> {
  await call(DESKTOP_COMMANDS.setAutostart, { enabled });
}

/** Quits and restarts the application. The page goes away: the promise may never settle. */
export async function restartApp(): Promise<void> {
  await call(DESKTOP_COMMANDS.restartApp);
}

/** Version of the desktop application (not of the embedded server). */
export async function appVersion(): Promise<string> {
  const v = await call(DESKTOP_COMMANDS.appVersion);
  if (typeof v !== "string" || !v.trim()) {
    throw new DesktopError(DESKTOP_COMMANDS.appVersion, `unexpected answer: ${reason(v)}`);
  }
  return v.trim();
}

/**
 * `true` when the page was opened on the PC that runs Corde (desktop shell, or a browser on
 * `127.0.0.1` / `localhost`). Only that PC may change the server settings.
 */
export function isLocalPage(hostname: string = window.location.hostname): boolean {
  const h = hostname.toLowerCase().replace(/^\[|\]$/g, "");
  return (
    h === "localhost" || h === "::1" || h === "tauri.localhost" || /^127(\.\d{1,3}){3}$/.test(h)
  );
}

/* ---------------------------------------------------------------- updates */

export type UpdatePhase = "idle" | "checking" | "downloading" | "installing";

/** Why the last update action failed or was refused (`updates::ErrorKind` in the shell). */
export type UpdateErrorKind =
  | "check"
  | "download"
  | "install"
  | "game_running"
  | "recording"
  | "activity_unknown"
  | "busy"
  | "nothing_to_install";

/** The update state of the desktop shell (`updates::Snapshot`, snake_case as sent). */
export interface UpdateStatus {
  current_version: string;
  auto_check: boolean;
  phase: UpdatePhase;
  /** End of the last check (successful or not), ms since 1970. */
  last_check_ms: number | null;
  last_check_ok: boolean | null;
  available: { version: string; date: string | null; notes: string | null } | null;
  progress: { downloaded: number; total: number | null } | null;
  error: { kind: UpdateErrorKind; message: string } | null;
  /** Version before the update, on the first start after an update. */
  updated_from: string | null;
}

const PHASES: readonly string[] = ["idle", "checking", "downloading", "installing"];
const ERROR_KINDS: readonly string[] = [
  "check",
  "download",
  "install",
  "game_running",
  "recording",
  "activity_unknown",
  "busy",
  "nothing_to_install",
];

function isRecord(v: unknown): v is Record<string, unknown> {
  return typeof v === "object" && v !== null && !Array.isArray(v);
}

function text(v: unknown): string | null {
  return typeof v === "string" && v.trim() ? v.trim() : null;
}

function count(v: unknown): number | null {
  return typeof v === "number" && Number.isFinite(v) && v >= 0 ? v : null;
}

/**
 * Validates what the shell sent. `null` when it is not an update state at all; unknown values
 * inside (from a newer shell) are degraded rather than rejected.
 */
export function parseUpdateStatus(v: unknown): UpdateStatus | null {
  if (!isRecord(v)) return null;
  const current = text(v.current_version);
  if (!current) return null;
  const phase = typeof v.phase === "string" && PHASES.includes(v.phase) ? v.phase : "idle";
  let available: UpdateStatus["available"] = null;
  if (isRecord(v.available)) {
    const version = text(v.available.version);
    if (version) {
      available = { version, date: text(v.available.date), notes: text(v.available.notes) };
    }
  }
  let progress: UpdateStatus["progress"] = null;
  if (isRecord(v.progress)) {
    const downloaded = count(v.progress.downloaded);
    if (downloaded !== null) progress = { downloaded, total: count(v.progress.total) || null };
  }
  let error: UpdateStatus["error"] = null;
  if (isRecord(v.error)) {
    const kind =
      typeof v.error.kind === "string" && ERROR_KINDS.includes(v.error.kind)
        ? v.error.kind
        : "check";
    error = {
      kind: kind as UpdateErrorKind,
      message: typeof v.error.message === "string" ? v.error.message : "",
    };
  }
  return {
    current_version: current,
    auto_check: v.auto_check !== false,
    phase: phase as UpdatePhase,
    last_check_ms: count(v.last_check_ms),
    last_check_ok: typeof v.last_check_ok === "boolean" ? v.last_check_ok : null,
    available,
    progress,
    error,
    updated_from: text(v.updated_from),
  };
}

async function statusCall(command: string): Promise<UpdateStatus> {
  const v = await call(command);
  const status = parseUpdateStatus(v);
  if (!status) throw new DesktopError(command, `unexpected answer: ${reason(v)}`);
  return status;
}

/** Current update state of the desktop application. */
export function updateStatus(): Promise<UpdateStatus> {
  return statusCall(DESKTOP_COMMANDS.updateStatus);
}

/** Looks for a newer version now. A failed check is described in the returned state. */
export function checkUpdate(): Promise<UpdateStatus> {
  return statusCall(DESKTOP_COMMANDS.checkUpdate);
}

/**
 * Downloads and installs the known update. On success Corde quits and the installer restarts
 * it: the promise never settles. Rejects with the shell's message when refused.
 */
export async function installUpdate(): Promise<void> {
  await call(DESKTOP_COMMANDS.installUpdate);
}

/** Opens the releases page (fixed address, chosen by the shell) in the default browser. */
export async function openReleasesPage(): Promise<void> {
  await call(DESKTOP_COMMANDS.openReleasesPage);
}

/** Does the application look for updates on its own (on by default)? */
export async function getUpdateAutoCheck(): Promise<boolean> {
  const v = await call(DESKTOP_COMMANDS.getUpdateAutoCheck);
  if (typeof v !== "boolean") {
    throw new DesktopError(DESKTOP_COMMANDS.getUpdateAutoCheck, `unexpected answer: ${reason(v)}`);
  }
  return v;
}

export async function setUpdateAutoCheck(enabled: boolean): Promise<void> {
  await call(DESKTOP_COMMANDS.setUpdateAutoCheck, { enabled });
}

/**
 * Listens to the shell's `update-status` event. Resolves to an unsubscribe function, or to
 * `null` when events are not available (browser, refused permission): callers then poll.
 */
export async function onUpdateStatus(
  handler: (status: UpdateStatus) => void,
): Promise<(() => void) | null> {
  if (typeof window === "undefined" || !isDesktop()) return null;
  try {
    const events = window.__TAURI__?.event;
    const listen = events?.listen;
    if (typeof listen !== "function") return null;
    const unlisten = await listen.call(events, UPDATE_EVENT, (e) => {
      const status = parseUpdateStatus(e?.payload);
      if (status) handler(status);
    });
    return typeof unlisten === "function" ? unlisten : null;
  } catch {
    return null;
  }
}
