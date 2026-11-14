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

/** A Live Activity of the application, as the native side lists it. */
export interface NativeActivity {
  id: string;
  pickId: string;
  /** Start, ms since the epoch. */
  startMs: number;
}

/** What the native side shows in a Live Activity (`CordeRaceAttributes.swift`). */
export interface NativeActivityStart {
  pickId: string;
  cat: string;
  level: string | null;
  name: string;
  track: string;
  classes: string;
  startMs: number;
  lang: string;
}

/** The state of the native side, for the diagnosis shown on the Courses page. */
export interface NativeDiagnostics {
  /** `ok`, `empty` (nothing written yet), `error`. */
  store: "ok" | "empty" | "error";
  /** Last `OSStatus` of the shared keychain item (0 = success). */
  status: number;
  /** The keychain group could be read from the application's `Info.plist`. */
  group: boolean;
  /** Bytes of the block held, and when the application wrote it (ms, 0 when none). */
  bytes: number;
  writtenAt: number;
  /** Format version of the block held (0 when none or unreadable). */
  version: number;
  activitiesAllowed: boolean;
  activities: number;
  ios: string;
}

/**
 * The application's own native module (`apps/mobile/ios/App/App/CordeNativePlugin.swift`):
 * the block of the widgets in the shared keychain item, and the Live Activity.
 */
export interface NativePort {
  /** Writes the block and asks WidgetKit to reload the widgets. */
  setWidgetData(options: { json: string }): Promise<{ ok: boolean; status: number; bytes: number }>;
  /** Whether iOS allows Live Activities for Corde, and those running. */
  activities(): Promise<{ allowed: boolean; running: NativeActivity[] }>;
  startActivity(options: NativeActivityStart): Promise<{ id: string }>;
  /** Ends the activities given (all of them without `ids`), removed at once. */
  endActivities(options: { ids?: string[] }): Promise<void>;
  diagnostics(): Promise<NativeDiagnostics>;
}
