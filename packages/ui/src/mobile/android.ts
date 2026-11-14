import type { LocalNotificationsPlugin } from "@capacitor/local-notifications";
import type { RaceReminder } from "@corde/protocol";
import { translate } from "../i18n";
import type { Lang } from "../store/settings";
import { KEYS, readJson, writeJson } from "./kv";
import type { LogPort, PrefsPort } from "./ports";
import {
  ANDROID_SOUND,
  type AndroidStyle,
  CHANNEL,
  notificationOf,
  type PermissionView,
} from "./reminders";

/**
 * What only the Android application does (`apps/mobile/android`, ADR 0013): the notification
 * channel of the reminders, exact alarms, the back button, and the entry of the CI's smoke
 * test. Loaded by `mobile/boot.ts` like the rest of `src/mobile/`, used only when
 * `Capacitor.getPlatform()` is `android` (`device.ts`).
 */

/** The parts of the notifications plugin that only exist on Android. */
export type AndroidNotificationsPort = Pick<
  LocalNotificationsPlugin,
  "createChannel" | "checkExactNotificationSetting" | "changeExactNotificationSetting"
>;

/** « Alarmes et rappels » (`SCHEDULE_EXACT_ALARM`): allowed or not. */
export type ExactView = "granted" | "denied";

export interface AndroidRemindersOptions {
  port: AndroidNotificationsPort;
  prefs: PrefsPort;
  lang: () => Lang;
  log?: LogPort;
  /** `window.confirm` in the application (a dialog of the system in the webview). */
  confirm: (text: string) => boolean;
  /** The state of exact alarms changed (the Courses page shows it). */
  onExact?: (v: ExactView) => void;
}

/**
 * The reminders on Android. Corde declares `SCHEDULE_EXACT_ALARM` (granted by the driver in
 * « Alarmes et rappels », refused by default on Android 14 and later), not `USE_EXACT_ALARM`
 * (Play Store: alarm clocks and calendars only). With it, a reminder rings at its minute,
 * phone asleep included (`setExactAndAllowWhileIdle`); without it, the plugin falls back to an
 * inexact alarm that Android may deliver a few minutes late (`setAndAllowWhileIdle`).
 */
export class AndroidReminders {
  private last: ExactView | null = null;

  constructor(private readonly opts: AndroidRemindersOptions) {}

  /**
   * The channel « Rappels des courses »: high importance (banner), Corde's sound, vibration,
   * shown on the lock screen. Created again at each start and language change (Android keeps
   * the sound and the driver's own changes, and updates the name).
   */
  async createChannel(): Promise<void> {
    const lang = this.opts.lang();
    await this.opts.port.createChannel({
      id: CHANNEL,
      name: translate(lang, "mobile.channel.name"),
      description: translate(lang, "mobile.channel.description"),
      importance: 4,
      visibility: 1,
      sound: ANDROID_SOUND,
      vibration: true,
      lights: false,
    });
  }

  /** Whether exact alarms are allowed now (`denied` when the plugin cannot tell). */
  async exact(): Promise<ExactView> {
    let v: ExactView = "denied";
    try {
      const r = await this.opts.port.checkExactNotificationSetting();
      v = r.exact_alarm === "granted" ? "granted" : "denied";
    } catch (e) {
      this.opts.log?.(`reminders: exact alarms unknown (${String(e)})`);
    }
    if (v !== this.last) {
      this.last = v;
      this.opts.log?.(`reminders: exact alarms ${v}`);
      this.opts.onExact?.(v);
    }
    return v;
  }

  /** How the next planning delivers the reminders. */
  async style(): Promise<AndroidStyle> {
    return { exact: (await this.exact()) === "granted" };
  }

  /**
   * After the notification permission (first pick): when it is granted and exact alarms are
   * not, Corde explains why and offers to open « Alarmes et rappels » — once. Refused: the
   * reminders stay inexact, the Courses page keeps a button to allow them later.
   */
  async offerExact(p: PermissionView): Promise<void> {
    if (p !== "granted") return;
    if ((await this.exact()) === "granted") return;
    if ((await readJson(this.opts.prefs, KEYS.exactAsked)) === true) return;
    await writeJson(this.opts.prefs, KEYS.exactAsked, true);
    if (!this.opts.confirm(translate(this.opts.lang(), "mobile.exact.confirm"))) return;
    await this.askExact();
  }

  /** Opens « Alarmes et rappels » for Corde; the state when the driver comes back. */
  async askExact(): Promise<ExactView> {
    try {
      await this.opts.port.changeExactNotificationSetting();
    } catch (e) {
      this.opts.log?.(`reminders: exact alarms screen (${String(e)})`);
    }
    return this.exact();
  }
}

/**
 * The back button / gesture of Android: back in the interface's history (the previous page),
 * and out of the application when there is nothing to go back to. (Without a listener the
 * `App` plugin goes back but never leaves.)
 */
export function backAction(canGoBack: boolean): "back" | "exit" {
  return canGoBack ? "back" : "exit";
}

/**
 * Entry of the CI's smoke test, sent only by the Debug build (`DebugHooks.java`, absent from the
 * Release build) as a `cordeDebug` event of the page:
 *
 * - `reminder` — a test reminder in `delaySec` seconds (10 to 600), built like a real one
 *   (channel, sound, icon, exact alarm when allowed) but marked apart: the planning of the
 *   races never cancels it;
 * - `fetch` — `fetch(url)` from the page (`http://` only, the address of the CI's machine as the
 *   emulator sees it), to check clear-text traffic and the page's origin;
 * - `scan` — opens the QR code scanner.
 *
 * Each one only writes to the log; none changes the driver's data.
 */
export type DebugRequest =
  | { action: "reminder"; delaySec: number }
  | { action: "fetch"; url: string }
  | { action: "scan" };

export function parseDebugRequest(ev: unknown): DebugRequest | null {
  if (typeof ev !== "object" || ev === null) return null;
  const o = ev as Record<string, unknown>;
  const d =
    typeof o.detail === "object" && o.detail !== null ? (o.detail as Record<string, unknown>) : o;
  const action = d.action ?? o.action;
  if (action === "reminder") {
    const n = Number(d.delaySec ?? o.delaySec ?? 60);
    const delaySec = Number.isFinite(n) ? Math.min(600, Math.max(10, Math.round(n))) : 60;
    return { action, delaySec };
  }
  if (action === "fetch") {
    const url = d.url ?? o.url;
    if (typeof url !== "string" || !/^http:\/\/[0-9a-z.[\]:-]+(\/[\w./-]*)?$/i.test(url)) {
      return null;
    }
    return { action, url };
  }
  if (action === "scan") return { action };
  return null;
}

/** Notification identifier of the test reminder (far from the FNV ones of `reminders.ts`). */
export const DEBUG_REMINDER_ID = 1;

/** The test reminder of the smoke test, `delaySec` from `now`. */
export function debugReminder(now: number, delaySec: number, lang: Lang, style: AndroidStyle) {
  const at = now + delaySec * 1000;
  const reminder: RaceReminder = {
    pick_id: "corde-debug",
    race_id: "corde-debug",
    name: "Corde",
    track: "CI",
    cat: "daily",
    level: null,
    start_ms: at,
    kind: "lead",
    lead_min: 0,
    due_ms: at,
  };
  const n = notificationOf(
    { id: DEBUG_REMINDER_ID, key: "corde-debug", at, reminder },
    lang,
    style,
  );
  // Not a reminder of the planning (`isReminderExtra` is false): never cancelled by it.
  return { ...n, extra: { mark: "corde-debug", at } };
}
