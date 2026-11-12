import type {
  RacePick,
  RaceSettings,
  RaceSettingsPatch,
  RaceSources,
  SourceId,
} from "../net/races-api";
import { ApiError } from "../net/sessions-api";
import type { RaceBackend } from "../races/backend";
import type { Lang } from "../store/settings";
import { KEYS, readJson, writeJson } from "./kv";
import type { LogPort, NotificationsPort, PrefsPort } from "./ports";
import type { RaceBook } from "./race-book";
import {
  cancelAllReminders,
  ensurePermission,
  type PermissionView,
  planReminders,
  type SyncResult,
  syncReminders,
} from "./reminders";
import type { PhoneSchedule } from "./schedule";

export interface PhoneRacesOptions {
  prefs: PrefsPort;
  schedule: PhoneSchedule;
  book: RaceBook;
  notifications: NotificationsPort;
  lang: () => Lang;
  now?: () => number;
  log?: LogPort;
  /** The permission state changed (the Courses page shows it). */
  onPermission?: (p: PermissionView) => void;
}

/**
 * The races of the iPhone application ({@link RaceBackend} `phone`): the schedule read by the
 * phone ({@link PhoneSchedule}), picks and settings on the phone ({@link RaceBook}), reminders
 * as local notifications (`reminders.ts`), planned again after every change of the picks or
 * of the delays, and when the application comes back to the front.
 *
 * The notification permission is asked **at the first pick**, never at start-up: before that,
 * nothing is scheduled (and nothing could be — `schedule` would ask by itself, which this
 * module never lets happen without a pick).
 */
export class PhoneRaces implements RaceBackend {
  readonly kind = "phone" as const;
  private enabled: boolean | null = null;
  private planning: Promise<SyncResult | null> | null = null;
  private again = false;
  private readonly now: () => number;

  constructor(private readonly opts: PhoneRacesOptions) {
    this.now = opts.now ?? Date.now;
  }

  private async isEnabled(): Promise<boolean> {
    if (this.enabled === null)
      this.enabled = (await readJson(this.opts.prefs, KEYS.racesEnabled)) === true;
    return this.enabled;
  }

  fetchEnabled(): Promise<boolean> {
    return this.isEnabled();
  }

  async setEnabled(on: boolean): Promise<void> {
    this.enabled = on;
    await writeJson(this.opts.prefs, KEYS.racesEnabled, on);
  }

  canSwitch(): boolean {
    return true;
  }

  async fetchSources(force: boolean): Promise<RaceSources> {
    return this.opts.schedule.snapshot(await this.isEnabled(), force);
  }

  /** Nothing to keep: the phone has no diagnostic folder (the page says where to look). */
  async reportUnreadable(_id: SourceId): Promise<void> {}

  async fetchPicks(signal?: AbortSignal): Promise<RacePick[]> {
    const picks = await this.opts.book.list();
    if (signal?.aborted) throw new ApiError(0, "aborted");
    return picks;
  }

  async savePick(pick: RacePick): Promise<RacePick> {
    const saved = await this.opts.book.put(pick);
    // The first pick is the moment to ask for the notifications (the system asks only once).
    const asked = (await readJson(this.opts.prefs, KEYS.notifAsked)) === true;
    if (!asked) {
      await writeJson(this.opts.prefs, KEYS.notifAsked, true);
      try {
        this.opts.onPermission?.(await ensurePermission(this.opts.notifications));
      } catch (e) {
        this.opts.log?.(`reminders: permission request failed (${String(e)})`);
      }
    }
    void this.replan();
    return saved;
  }

  async deletePick(id: string): Promise<void> {
    await this.opts.book.remove(id);
    void this.replan();
  }

  fetchSettings(): Promise<RaceSettings> {
    return this.opts.book.getSettings();
  }

  async saveSettings(patch: RaceSettingsPatch): Promise<RaceSettings> {
    const s = await this.opts.book.patchSettings(patch);
    if (patch.leads !== undefined || patch.hour_fix !== undefined) void this.replan();
    return s;
  }

  /**
   * Plans the reminders again (one planning at a time; a call during one runs once more after
   * it, with the latest picks). `null` when the notifications cannot be read.
   */
  replan(): Promise<SyncResult | null> {
    if (this.planning) {
      this.again = true;
      return this.planning;
    }
    const run = async (): Promise<SyncResult | null> => {
      let result: SyncResult | null = null;
      do {
        this.again = false;
        try {
          const picks = await this.opts.book.list();
          const { leads } = await this.opts.book.getSettings();
          if (picks.length === 0) {
            await cancelAllReminders(this.opts.notifications);
            result = { blocked: false, scheduled: 0, cancelled: 0, kept: 0 };
          } else {
            const plan = planReminders(picks, leads, this.now());
            result = await syncReminders(
              this.opts.notifications,
              plan,
              this.opts.lang(),
              this.opts.log,
            );
          }
        } catch (e) {
          this.opts.log?.(
            `reminders: planning failed (${e instanceof Error ? e.message : String(e)})`,
          );
          result = null;
        }
      } while (this.again);
      return result;
    };
    this.planning = run().finally(() => {
      this.planning = null;
    });
    return this.planning;
  }

  /** The permission as the system sees it now. */
  async permission(): Promise<PermissionView> {
    const d = (await this.opts.notifications.checkPermissions()).display;
    return d === "granted" ? "granted" : d === "denied" ? "denied" : "prompt";
  }

  /** « Autoriser les notifications » (when the system has not asked yet). */
  async askPermission(): Promise<PermissionView> {
    await writeJson(this.opts.prefs, KEYS.notifAsked, true);
    const p = await ensurePermission(this.opts.notifications);
    this.opts.onPermission?.(p);
    if (p === "granted") void this.replan();
    return p;
  }
}
