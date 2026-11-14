import type { RacePick, RaceSettings, RaceSources } from "../net/races-api";
import { buildSchedule } from "../races/model";
import type { Race } from "../races/parse";
import type { Lang } from "../store/settings";
import { readJson, writeJson } from "./kv";
import type {
  LogPort,
  NativeDiagnostics,
  NativeImagesResult,
  NativePort,
  PrefsPort,
} from "./ports";
import {
  type ActivityEvent,
  type ActivityPlan,
  buildPayload,
  imageUrls,
  payloadKey,
  planActivity,
  type WidgetImage,
  type WidgetPayload,
  widgetImages,
} from "./widgets";

/** Keys of this module in the phone's storage (`kv.ts` has the others). */
export const WIDGET_KEYS = {
  /** The last schedule read, for the widgets while the pages cannot be read (≤ 3 days). */
  schedule: "corde.widgets.schedule.v1",
  /** « Compte à rebours sur l'écran verrouillé et dans la Dynamic Island » (on by default). */
  liveActivity: "corde.races.liveActivity",
} as const;

/** A schedule kept for the widgets is used for 3 days at most. */
export const KEEP_SCHEDULE_MS = 3 * 864e5;
/** A picture that could not be downloaded is tried again after this (at the next update). */
export const IMAGE_RETRY_MS = 15 * 60e3;

export interface WidgetSyncOptions {
  prefs: PrefsPort;
  native: NativePort;
  /** What the « Courses » page works with (the phone's races). */
  picks: () => Promise<RacePick[]>;
  settings: () => Promise<RaceSettings>;
  enabled: () => Promise<boolean>;
  /** The last answer of the schedule (pages as text), `null` before the first reading. */
  sources: () => RaceSources | null;
  lang: () => Lang;
  now?: () => number;
  log?: LogPort;
  /** The Live Activity changed (the Courses page shows its state). */
  onActivity?: (state: ActivityView) => void;
}

export interface ActivityView {
  /** iOS allows Live Activities for Corde. */
  allowed: boolean;
  /** The pick the running activity follows, `null` for none. */
  pickId: string | null;
  enabled: boolean;
}

/** The last write of the block, for the diagnosis. */
export interface WriteView {
  ok: boolean;
  status: number;
  bytes: number;
  at: number;
}

interface KeptSchedule {
  at: number;
  races: Race[];
}

function isKept(v: unknown): v is KeptSchedule {
  if (typeof v !== "object" || v === null) return false;
  const o = v as Record<string, unknown>;
  return typeof o.at === "number" && Array.isArray(o.races);
}

/**
 * Keeps the widgets and the Live Activity in step with the phone's races. `update()` builds the
 * block of the widgets ({@link buildPayload}) and writes it through the native module only when
 * it changed; `activity(event)` applies the Live Activity rules ({@link planActivity}). Both run
 * one at a time (a call during a run runs once more after it). Nothing here reads the network:
 * the schedule is the one the Courses page already read; the pictures of the « Courses en
 * images » widget are downloaded by the native side of the application (`setWidgetImages`),
 * which is handed their list after each write of the block, only when it changed.
 */
export class WidgetSync {
  private lastKey: string | null = null;
  private updating: Promise<void> | null = null;
  private updateAgain = false;
  private acting: Promise<ActivityPlan | null> = Promise.resolve(null);
  lastWrite: WriteView | null = null;
  private lastImagesKey: string | null = null;
  private imagesRetryAt: number | null = null;
  /** The last answer of `setWidgetImages`, for the diagnosis. */
  lastImages: (NativeImagesResult & { at: number }) | null = null;
  private readonly now: () => number;

  constructor(private readonly opts: WidgetSyncOptions) {
    this.now = opts.now ?? Date.now;
  }

  /** The schedule for the widgets: the one just read, else the one kept (≤ 3 days). */
  private async schedule(enabled: boolean): Promise<Race[] | null> {
    if (!enabled) return null;
    const sources = this.opts.sources();
    const fresh = sources ? buildSchedule(sources).schedule : null;
    if (fresh && fresh.races.length > 0) {
      try {
        await writeJson(this.opts.prefs, WIDGET_KEYS.schedule, {
          at: this.now(),
          races: fresh.races,
        } satisfies KeptSchedule);
      } catch (e) {
        this.opts.log?.(`widgets: schedule not kept (${String(e)})`);
      }
      return fresh.races;
    }
    const kept = await readJson(this.opts.prefs, WIDGET_KEYS.schedule);
    if (isKept(kept) && this.now() - kept.at <= KEEP_SCHEDULE_MS) return kept.races;
    return null;
  }

  /** The block as it would be written now, and the pictures of its pages. */
  private async build(): Promise<{ payload: WidgetPayload; images: WidgetImage[] }> {
    const enabled = await this.opts.enabled();
    const [picks, settings, races] = await Promise.all([
      this.opts.picks(),
      this.opts.settings(),
      this.schedule(enabled),
    ]);
    const now = this.now();
    const payload = buildPayload({
      races,
      enabled,
      picks,
      settings,
      lang: this.opts.lang(),
      now,
    });
    return { payload, images: widgetImages(payload, imageUrls(races ?? [], picks), now) };
  }

  /** The block as it would be written now. */
  async payload(): Promise<WidgetPayload> {
    return (await this.build()).payload;
  }

  /**
   * Hands the list of pictures to the native side when it changed (the same pictures in another
   * order is no change), or again 15 minutes after a failed download. The native side downloads
   * what it does not keep yet, deletes what is no longer listed.
   */
  private async syncImages(images: WidgetImage[]): Promise<void> {
    const key = images
      .map((i) => `${i.key}=${i.url}`)
      .sort()
      .join("\n");
    const now = this.now();
    if (key === this.lastImagesKey && (this.imagesRetryAt === null || now < this.imagesRetryAt)) {
      return;
    }
    try {
      const r = await this.opts.native.setWidgetImages({ items: images });
      this.lastImages = { ...r, at: now };
      this.lastImagesKey = key;
      this.imagesRetryAt = r.failed > 0 ? now + IMAGE_RETRY_MS : null;
      this.opts.log?.(
        `widgets: pictures ${r.written} written, ${r.kept} kept, ${r.failed} failed, ${r.removed} removed (${r.bytes} bytes, OSStatus ${r.status})`,
      );
    } catch (e) {
      // Not available (an older application) or refused: tried again at the next change.
      this.lastImagesKey = null;
      this.opts.log?.(`widgets: pictures: ${e instanceof Error ? e.message : String(e)}`);
    }
  }

  /** Writes the block when it changed (`force`: even when not). */
  update(force = false): Promise<void> {
    if (this.updating) {
      this.updateAgain = true;
      if (force) this.lastKey = null;
      return this.updating;
    }
    if (force) this.lastKey = null;
    const run = async () => {
      do {
        this.updateAgain = false;
        try {
          const { payload: p, images } = await this.build();
          const key = payloadKey(p);
          if (key !== this.lastKey) {
            const json = JSON.stringify(p);
            const r = await this.opts.native.setWidgetData({ json });
            this.lastWrite = { ok: r.ok, status: r.status, bytes: r.bytes, at: p.at };
            if (r.ok) {
              this.lastKey = key;
              this.opts.log?.(
                `widgets: ${r.bytes} bytes written (${p.races.length} races, ${p.mine.length} picks, schedule ${p.sched})`,
              );
            } else {
              this.opts.log?.(`widgets: shared store refused the block (OSStatus ${r.status})`);
              continue;
            }
          }
          // After the block (it names the pictures): the pictures of its pages.
          await this.syncImages(images);
        } catch (e) {
          this.opts.log?.(`widgets: ${e instanceof Error ? e.message : String(e)}`);
        }
      } while (this.updateAgain);
    };
    this.updating = run().finally(() => {
      this.updating = null;
    });
    return this.updating;
  }

  async liveActivityEnabled(): Promise<boolean> {
    return (await readJson(this.opts.prefs, WIDGET_KEYS.liveActivity)) !== false;
  }

  async setLiveActivityEnabled(on: boolean): Promise<void> {
    await writeJson(this.opts.prefs, WIDGET_KEYS.liveActivity, on);
    await this.activity({ kind: "setting" });
  }

  /** Applies the Live Activity rules for `event` (one at a time, in order). */
  activity(event: ActivityEvent): Promise<ActivityPlan | null> {
    const next = this.acting.then(() => this.doActivity(event));
    this.acting = next.catch(() => null);
    return next;
  }

  private async doActivity(event: ActivityEvent): Promise<ActivityPlan | null> {
    try {
      const enabled = await this.liveActivityEnabled();
      const { allowed, running } = await this.opts.native.activities();
      const picks = await this.opts.picks();
      const plan = planActivity({
        event,
        picks,
        running: running.map((a) => ({ id: a.id, pickId: a.pickId, start: a.startMs })),
        enabled,
        allowed,
        now: this.now(),
      });
      if (plan.end.length > 0) await this.opts.native.endActivities({ ids: plan.end });
      let pickId: string | null = running.find((a) => !plan.end.includes(a.id))?.pickId ?? null;
      if (plan.start) {
        const p = plan.start;
        const races = await this.schedule(await this.opts.enabled()).catch(() => null);
        const race = races?.find((r) => r.id === p.race_id);
        await this.opts.native.startActivity({
          pickId: p.id,
          cat: p.cat,
          level: p.level,
          name: p.name,
          track: p.track,
          classes: (race?.classes ?? []).slice(0, 4).join(" · "),
          startMs: p.start_ms,
          lang: this.opts.lang(),
        });
        pickId = p.id;
      }
      if (plan.start || plan.end.length > 0) {
        this.opts.log?.(
          `live activity (${event.kind}): ${plan.reason}, ${plan.end.length} ended${plan.start ? `, started for ${plan.start.id}` : ""}`,
        );
      }
      this.opts.onActivity?.({ allowed, pickId, enabled });
      return plan;
    } catch (e) {
      this.opts.log?.(`live activity: ${e instanceof Error ? e.message : String(e)}`);
      return null;
    }
  }

  diagnostics(): Promise<NativeDiagnostics> {
    return this.opts.native.diagnostics();
  }
}
