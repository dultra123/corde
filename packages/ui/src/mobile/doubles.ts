import type { HttpOptions, HttpResponse, PermissionState } from "@capacitor/core";
import type {
  Channel,
  LocalNotificationSchema,
  PendingLocalNotificationSchema,
} from "@capacitor/local-notifications";
import type { AndroidNotificationsPort } from "./android";
import type {
  HttpPort,
  NativeActivity,
  NativeActivityStart,
  NativeDiagnostics,
  NativeImageItem,
  NativeImagesResult,
  NativePort,
  NotificationsPort,
  PrefsPort,
} from "./ports";

/**
 * Doubles of the Capacitor plugins for the tests of `src/mobile/` (same method names and
 * shapes as the plugins: `Preferences`, `CapacitorHttp`, `LocalNotifications`).
 */
export class FakePrefs implements PrefsPort {
  readonly data = new Map<string, string>();
  async get({ key }: { key: string }) {
    return { value: this.data.get(key) ?? null };
  }
  async set({ key, value }: { key: string; value: string }) {
    this.data.set(key, value);
  }
  async remove({ key }: { key: string }) {
    this.data.delete(key);
  }
}

export type HttpHandler = (options: HttpOptions) => HttpResponse | Promise<HttpResponse>;

export class FakeHttp implements HttpPort {
  readonly calls: HttpOptions[] = [];
  constructor(public handler: HttpHandler) {}
  async get(options: HttpOptions): Promise<HttpResponse> {
    this.calls.push(options);
    return this.handler(options);
  }
}

export const page = (data: unknown, status = 200, headers: Record<string, string> = {}) =>
  ({ data, status, headers, url: "" }) as HttpResponse;

export class FakeNotifications implements NotificationsPort {
  permission: PermissionState = "prompt";
  /** What `requestPermissions` turns `prompt` into. */
  answer: PermissionState = "granted";
  requests = 0;
  pending: LocalNotificationSchema[] = [];
  scheduled: LocalNotificationSchema[][] = [];
  cancelled: number[][] = [];

  async checkPermissions() {
    return { display: this.permission };
  }
  async requestPermissions() {
    this.requests += 1;
    if (this.permission === "prompt") this.permission = this.answer;
    return { display: this.permission };
  }
  async schedule({ notifications }: { notifications: LocalNotificationSchema[] }) {
    // The real plugin asks for the permission itself when it was never asked.
    if (this.permission === "prompt") await this.requestPermissions();
    this.scheduled.push(notifications);
    for (const n of notifications) {
      this.pending = this.pending.filter((p) => p.id !== n.id);
      this.pending.push(n);
    }
    return { notifications: notifications.map((n) => ({ id: n.id })) };
  }
  async getPending() {
    // As the plugin returns them: id, title, body, schedule, extra.
    const notifications: PendingLocalNotificationSchema[] = this.pending.map((n) => ({
      id: n.id,
      title: n.title,
      body: n.body,
      schedule: n.schedule,
      extra: JSON.parse(JSON.stringify(n.extra ?? null)),
    }));
    return { notifications };
  }
  async cancel({ notifications }: { notifications: { id: number }[] }) {
    const ids = notifications.map((n) => n.id);
    this.cancelled.push(ids);
    this.pending = this.pending.filter((p) => !ids.includes(p.id));
  }
}

/** Double of the Android-only methods of `LocalNotifications` (`mobile/android.ts`). */
export class FakeAndroidNotifications implements AndroidNotificationsPort {
  channels: Channel[] = [];
  /** « Alarmes et rappels » for Corde. */
  exact: PermissionState = "denied";
  /** What the driver does on the « Alarmes et rappels » screen. */
  exactAnswer: PermissionState = "granted";
  /** Times that screen was opened. */
  screens = 0;
  failCheck = false;

  async createChannel(channel: Channel) {
    this.channels = this.channels.filter((c) => c.id !== channel.id);
    this.channels.push({ ...channel });
  }
  async checkExactNotificationSetting() {
    if (this.failCheck) throw new Error("not implemented");
    return { exact_alarm: this.exact };
  }
  async changeExactNotificationSetting() {
    this.screens += 1;
    this.exact = this.exactAnswer;
    return { exact_alarm: this.exact };
  }
}

/** Double of the application's native module (`CordeNative`). */
export class FakeNative implements NativePort {
  written: string[] = [];
  /** `OSStatus` the next writes answer (0 = success). */
  status = 0;
  allowed = true;
  running: NativeActivity[] = [];
  started: NativeActivityStart[] = [];
  ended: string[][] = [];
  /** Each list of pictures received. */
  imageCalls: NativeImageItem[][] = [];
  /** The pictures kept: key → size. */
  images = new Map<string, number>();
  /** Addresses whose download fails. */
  failing = new Set<string>();
  /** Size of a reduced picture. */
  imageSize = 30_000;
  private next = 1;

  async setWidgetData({ json }: { json: string }) {
    if (this.status !== 0) return { ok: false, status: this.status, bytes: 0 };
    this.written.push(json);
    return { ok: true, status: 0, bytes: new TextEncoder().encode(json).length };
  }
  async setWidgetImages({ items }: { items: NativeImageItem[] }): Promise<NativeImagesResult> {
    this.imageCalls.push(items.map((i) => ({ ...i })));
    const list = items.slice(0, 8);
    const wanted = new Set(list.map((i) => i.key));
    let removed = 0;
    for (const key of [...this.images.keys()]) {
      if (!wanted.has(key)) {
        this.images.delete(key);
        removed += 1;
      }
    }
    let written = 0;
    let kept = 0;
    let failed = 0;
    for (const i of list) {
      if (this.images.has(i.key)) kept += 1;
      else if (this.failing.has(i.url) || !i.url.startsWith("https://")) failed += 1;
      else {
        this.images.set(i.key, this.imageSize);
        written += 1;
      }
    }
    const bytes = [...this.images.values()].reduce((a, b) => a + b, 0);
    return { written, kept, failed, removed, bytes, status: 0 };
  }
  async activities() {
    return { allowed: this.allowed, running: this.running.map((a) => ({ ...a })) };
  }
  async startActivity(o: NativeActivityStart) {
    this.started.push(o);
    const id = `A${this.next++}`;
    this.running.push({ id, pickId: o.pickId, startMs: o.startMs });
    return { id };
  }
  async endActivities({ ids }: { ids?: string[] }) {
    const gone = ids ?? this.running.map((a) => a.id);
    this.ended.push(gone);
    this.running = this.running.filter((a) => !gone.includes(a.id));
  }
  async diagnostics(): Promise<NativeDiagnostics> {
    const last = this.written.at(-1);
    return {
      store: this.status !== 0 ? "error" : last ? "ok" : "empty",
      status: this.status,
      group: true,
      bytes: last ? new TextEncoder().encode(last).length : 0,
      writtenAt: last ? (JSON.parse(last) as { at: number }).at : 0,
      version: last ? 1 : 0,
      activitiesAllowed: this.allowed,
      activities: this.running.length,
      ios: "18.1",
      images: this.images.size,
      imageBytes: [...this.images.values()].reduce((a, b) => a + b, 0),
      imageStatus: 0,
    };
  }
}
