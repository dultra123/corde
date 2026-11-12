import type { ClearCookieOptions, HttpOptions, HttpResponse } from "@capacitor/core";
import type { LocalNotificationsPlugin } from "@capacitor/local-notifications";
import type { PreferencesPlugin } from "@capacitor/preferences";

/**
 * The parts of the Capacitor plugins the application's logic uses, with the plugins' own
 * signatures (`import type` only: nothing of Capacitor is loaded by these modules). The real
 * plugins are passed by `mobile/platform.ts`; the tests pass doubles of the same shape.
 */
export type PrefsPort = Pick<PreferencesPlugin, "get" | "set" | "remove">;

/** `CapacitorHttp.get` (the plugin's interface type is not exported by `@capacitor/core`). */
export interface HttpPort {
  get(options: HttpOptions): Promise<HttpResponse>;
}

/** `CapacitorCookies.clearCookies`. */
export interface CookiesPort {
  clearCookies(options: ClearCookieOptions): Promise<void>;
}

export type NotificationsPort = Pick<
  LocalNotificationsPlugin,
  "checkPermissions" | "requestPermissions" | "schedule" | "getPending" | "cancel"
>;

/** `fetch`, for the calls to a Corde server (webview, CORS answered by the server). */
export type FetchPort = (input: string, init?: RequestInit) => Promise<Response>;

/** Diagnostic lines of the application (the native log in the app, nothing in the tests). */
export type LogPort = (line: string) => void;
