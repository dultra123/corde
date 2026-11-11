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
 *
 * No `@tauri-apps/api` dependency: only the minimal shape used here is declared.
 */

/** The part of `window.__TAURI__` this UI relies on. */
export interface TauriGlobal {
  core?: {
    invoke?: (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;
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
} as const;

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
