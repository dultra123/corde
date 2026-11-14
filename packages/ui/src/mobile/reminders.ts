import type { LocalNotificationSchema } from "@capacitor/local-notifications";
import type { RaceReminder } from "@corde/protocol";
import { translate } from "../i18n";
import type { RacePick } from "../net/races-api";
import { hm } from "../races/format";
import type { Lang } from "../store/settings";
import type { LogPort, NotificationsPort } from "./ports";

/**
 * Reminders of the picked races **as local notifications of the phone**: iOS delivers them at
 * their time, application closed or phone locked. Same reminders as the server's
 * (`crates/corde-server/src/races/reminders.rs`): one per delay of the settings for a race
 * (`start − delay`), and for a special event the evening before (`eve_ms`, 18:00) and the day
 * itself (`start_ms`, 10:00).
 *
 * iOS keeps at most **64** pending notifications per application: the nearest ones are
 * scheduled, the others wait for the next planning (each change of the picks or the delays,
 * each return to the application). Only future reminders are scheduled (iOS would deliver a
 * past one at once). Each reminder has a stable identifier derived from its key
 * (`<pick>#<delay|eve|day>`), so planning again never duplicates one: what is pending and
 * unchanged stays, what is no longer wanted is cancelled, what is missing is added.
 */
export const MAX_PENDING = 64;
/** A reminder due in less than this is not scheduled (it would be late). */
export const MIN_AHEAD_MS = 1_000;
/** Sound of the reminders: a file of the web assets (`apps/mobile/assets/corde-reminder.wav`). */
export const SOUND = "corde-reminder.wav";
/** Marks the notifications of this module among the application's pending ones. */
const MARK = "corde-race";
/**
 * Notification category (`UNNotificationCategory`, registered at start-up by `boot.ts` with
 * `registerActionTypes`): one action, « Voir Courses », which opens the application.
 */
export const CATEGORY = "corde-race";

/**
 * Android: the same sound as a raw resource of the application
 * (`apps/mobile/android/app/src/main/res/raw/corde_reminder.wav`: a resource name has no `-`).
 * On Android 8+ the sound belongs to the channel ({@link CHANNEL}), created with it.
 */
export const ANDROID_SOUND = "corde_reminder.wav";
/** Android: the notification channel of the reminders (created by `mobile/android.ts`). */
export const CHANNEL = "corde-races";
/** Android: the monochrome small icon (`res/drawable/ic_stat_corde.xml`). */
export const SMALL_ICON = "ic_stat_corde";
/** Android: the colour of the small icon (the accent of the interface). */
export const ICON_COLOR = "#ffb454";

/**
 * How the reminders are delivered on Android. `exact`: the driver allows exact alarms
 * (« Alarmes et rappels ») — the reminder rings at its minute, phone asleep included; without
 * it Android may deliver it a few minutes late. `undefined` on the iPhone: nothing changes.
 */
export interface AndroidStyle {
  exact: boolean;
}

export interface PlannedReminder {
  /** Notification identifier (32-bit, positive). */
  id: number;
  /** `<pick_id>#<15|eve|day>`. */
  key: string;
  /** When it rings (ms since the epoch). */
  at: number;
  reminder: RaceReminder;
}

/** FNV-1a of the key, 31 bits, never 0. */
export function notificationId(key: string): number {
  let h = 0x811c9dc5;
  for (let i = 0; i < key.length; i += 1) {
    h ^= key.charCodeAt(i);
    h = Math.imul(h, 0x01000193) >>> 0;
  }
  return h & 0x7fffffff || 1;
}

/** Every future reminder of `picks`, the nearest {@link MAX_PENDING} first. */
export function planReminders(
  picks: readonly RacePick[],
  leads: readonly number[],
  now: number,
  max: number = MAX_PENDING,
): PlannedReminder[] {
  const all: Omit<PlannedReminder, "id">[] = [];
  for (const p of picks) {
    const slots: [string, number, RaceReminder["kind"], number | null][] = [];
    if (p.cat === "special") {
      if (p.eve_ms != null) slots.push(["eve", p.eve_ms, "eve", null]);
      slots.push(["day", p.start_ms, "day", null]);
    } else {
      for (const lead of new Set(leads)) {
        slots.push([String(lead), p.start_ms - lead * 60_000, "lead", lead]);
      }
    }
    for (const [tag, at, kind, lead] of slots) {
      if (at < now + MIN_AHEAD_MS) continue;
      all.push({
        key: `${p.id}#${tag}`,
        at,
        reminder: {
          pick_id: p.id,
          race_id: p.race_id,
          name: p.name,
          track: p.track,
          cat: p.cat,
          level: p.level,
          start_ms: p.start_ms,
          kind,
          lead_min: lead,
          due_ms: at,
        },
      });
    }
  }
  all.sort((a, b) => a.at - b.at || (a.key < b.key ? -1 : a.key > b.key ? 1 : 0));
  const kept = all.slice(0, Math.max(0, max));
  // Identifiers: from the key; a (rare) collision takes the next free number, in key order so
  // that the same set always gets the same numbers.
  const used = new Set<number>();
  const ids = new Map<string, number>();
  for (const k of kept.map((r) => r.key).sort()) {
    let id = notificationId(k);
    while (used.has(id)) id = (id % 0x7fffffff) + 1;
    used.add(id);
    ids.set(k, id);
  }
  return kept.map((r) => ({ ...r, id: ids.get(r.key) as number }));
}

/** Title and text of a reminder, in the language of the application. */
export function reminderText(r: RaceReminder, lang: Lang): { title: string; body: string } {
  const tr = (k: Parameters<typeof translate>[1], p?: Record<string, string | number>) =>
    translate(lang, k, p);
  if (r.kind === "eve") return { title: tr("mobile.notif.eve"), body: `${r.name} — ${r.track}` };
  if (r.kind === "day") return { title: tr("mobile.notif.day"), body: `${r.name} — ${r.track}` };
  const title =
    (r.lead_min ?? 0) > 0 ? tr("mobile.notif.in", { n: r.lead_min ?? 0 }) : tr("mobile.notif.now");
  return { title, body: `${r.name} — ${r.track}, ${hm(r.start_ms)}` };
}

/** What `extra` of a notification of this module carries. */
export interface ReminderExtra {
  mark: typeof MARK;
  key: string;
  at: number;
  /** Title + text: a change of language or name schedules it again. */
  sig: string;
  reminder: RaceReminder;
  /** Android: scheduled as an exact alarm (a change of the permission schedules it again). */
  exact?: boolean;
}

export function isReminderExtra(v: unknown): v is ReminderExtra {
  if (typeof v !== "object" || v === null) return false;
  const o = v as Record<string, unknown>;
  return o.mark === MARK && typeof o.key === "string" && typeof o.at === "number";
}

export function notificationOf(
  p: PlannedReminder,
  lang: Lang,
  android?: AndroidStyle,
): LocalNotificationSchema {
  const { title, body } = reminderText(p.reminder, lang);
  const extra: ReminderExtra = {
    mark: MARK,
    key: p.key,
    at: p.at,
    sig: `${title}\n${body}`,
    reminder: p.reminder,
  };
  if (android) {
    // The channel carries the sound; `allowWhileIdle`: delivered in Doze too. Never
    // `isExactNotification: true` without the permission: the plugin would then open the
    // « Alarmes et rappels » screen by itself at every planning.
    return {
      id: p.id,
      title,
      body,
      schedule: { at: new Date(p.at), allowWhileIdle: true },
      sound: ANDROID_SOUND,
      channelId: CHANNEL,
      smallIcon: SMALL_ICON,
      iconColor: ICON_COLOR,
      isExactNotification: android.exact,
      actionTypeId: CATEGORY,
      extra: { ...extra, exact: android.exact },
    };
  }
  return {
    id: p.id,
    title,
    body,
    schedule: { at: new Date(p.at) },
    sound: SOUND,
    actionTypeId: CATEGORY,
    threadIdentifier: "corde-races",
    extra,
  };
}

export interface SyncResult {
  /** Permission not granted: nothing scheduled (what was pending is still cancelled). */
  blocked: boolean;
  scheduled: number;
  cancelled: number;
  kept: number;
}

export interface SyncOptions {
  /** Android ({@link AndroidStyle}); absent on the iPhone. */
  android?: AndroidStyle;
  /** Now (ms), for the Android pending list. */
  now?: number;
}

/**
 * Makes the pending notifications of this module match `plan`. Never asks for the permission
 * (the page asks it at the first pick, `ensurePermission`): without it nothing is scheduled.
 *
 * On Android the plugin's pending list also holds the reminders already delivered (kept for
 * its own queries; cancelling one does not remove it from the list): those, due before `now`,
 * are left alone.
 */
export async function syncReminders(
  port: NotificationsPort,
  plan: readonly PlannedReminder[],
  lang: Lang,
  log?: LogPort,
  opts: SyncOptions = {},
): Promise<SyncResult> {
  const { android } = opts;
  const now = opts.now ?? Date.now();
  const pending = (await port.getPending()).notifications;
  const wanted = new Map(plan.map((p) => [p.id, notificationOf(p, lang, android)]));
  const cancel: number[] = [];
  const keep = new Set<number>();
  for (const n of pending) {
    const extra: unknown = n.extra;
    if (!isReminderExtra(extra)) continue;
    if (android && extra.at <= now) continue;
    const w = wanted.get(n.id);
    const same =
      w !== undefined &&
      isReminderExtra(w.extra) &&
      w.extra.key === extra.key &&
      w.extra.at === extra.at &&
      w.extra.sig === extra.sig &&
      w.extra.exact === extra.exact;
    if (same && !keep.has(n.id)) keep.add(n.id);
    else cancel.push(n.id);
  }
  if (cancel.length > 0) await port.cancel({ notifications: cancel.map((id) => ({ id })) });
  const perm = await port.checkPermissions();
  const toAdd = [...wanted.values()].filter((n) => !keep.has(n.id));
  if (perm.display !== "granted") {
    log?.(`reminders: permission ${perm.display}, ${toAdd.length} not scheduled`);
    return { blocked: true, scheduled: 0, cancelled: cancel.length, kept: keep.size };
  }
  if (toAdd.length > 0) await port.schedule({ notifications: toAdd });
  log?.(`reminders: ${toAdd.length} scheduled, ${cancel.length} cancelled, ${keep.size} kept`);
  return { blocked: false, scheduled: toAdd.length, cancelled: cancel.length, kept: keep.size };
}

/** Cancels every pending reminder of this module (schedule turned off, nothing picked). */
export async function cancelAllReminders(port: NotificationsPort): Promise<number> {
  const pending = (await port.getPending()).notifications;
  const ids = pending.filter((n) => isReminderExtra(n.extra)).map((n) => n.id);
  if (ids.length > 0) await port.cancel({ notifications: ids.map((id) => ({ id })) });
  return ids.length;
}

export type PermissionView = "granted" | "denied" | "prompt";

/** Asks for the permission when the system has not asked yet; returns the state after. */
export async function ensurePermission(port: NotificationsPort): Promise<PermissionView> {
  const cur = (await port.checkPermissions()).display;
  if (cur === "granted" || cur === "denied") return cur;
  const after = (await port.requestPermissions()).display;
  return after === "granted" ? "granted" : after === "denied" ? "denied" : "prompt";
}
