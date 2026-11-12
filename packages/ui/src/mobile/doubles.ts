import type { HttpOptions, HttpResponse, PermissionState } from "@capacitor/core";
import type {
  LocalNotificationSchema,
  PendingLocalNotificationSchema,
} from "@capacitor/local-notifications";
import type { HttpPort, NotificationsPort, PrefsPort } from "./ports";

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
