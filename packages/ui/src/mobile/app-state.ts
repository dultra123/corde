import { create } from "zustand";
import { connection } from "../net/connection";
import { endpointFromHost, type ServerEndpoint } from "../net/url";
import { useHub } from "../store/hub";
import { useTelemetry } from "../store/telemetry";
import type { PhoneRaces } from "./phone-races";
import type { PermissionView } from "./reminders";
import type { PairResult, ProbeResult, ServerBook, ServerEntry } from "./servers";

/**
 * State of the iPhone application around the shared interface: the servers of « Mes
 * serveurs », whether the active one answers, and the notification permission. Driven by
 * {@link AppController}; read by the application's components (`mobile/ui`).
 */
export type ServerStatus =
  /** No server chosen. */
  "none" | "checking" | "ok" | "unpaired" | "refused" | "unreachable" | "not_corde" | "error";

export interface AppState {
  servers: ServerEntry[];
  activeId: string | null;
  /** Server ids this phone is paired with (never the tokens). */
  paired: string[];
  status: ServerStatus;
  /** The server's own words for `refused` / `error`. */
  statusMessage: string | null;
  version: string | null;
  permission: PermissionView | null;
  /** The keyboard is open (the tab bar hides). */
  keyboard: boolean;
}

export const useApp = create<AppState>(() => ({
  servers: [],
  activeId: null,
  paired: [],
  status: "none",
  statusMessage: null,
  version: null,
  permission: null,
  keyboard: false,
}));

/** `http://192.168.1.20:8471` → the endpoint of the shared connection layer. */
export function endpointOf(base: string): ServerEndpoint {
  const secure = base.startsWith("https://");
  const host = base.replace(/^https?:\/\//, "");
  return endpointFromHost(host, "setting", secure);
}

export function statusOf(r: ProbeResult): { status: ServerStatus; message: string | null } {
  switch (r.kind) {
    case "ok":
      return { status: "ok", message: null };
    case "unpaired":
      return { status: "unpaired", message: null };
    case "refused":
      return { status: "refused", message: r.message || null };
    case "not_corde":
      return { status: "not_corde", message: null };
    case "unreachable":
      return { status: "unreachable", message: null };
    case "error":
      return { status: "error", message: r.message || `HTTP ${r.status}` };
  }
}

/** Opens a QR code scanner; the text read, `null` when cancelled or unavailable. */
export type ScanFn = () => Promise<string | null>;

/**
 * Ties « Mes serveurs » ({@link ServerBook}) to the shared connection (`net/connection`,
 * `net/auth`) and to the home-server state (`store/hub`).
 */
export class AppController {
  private refreshing: Promise<void> | null = null;
  private dropTimer: ReturnType<typeof setTimeout> | null = null;

  constructor(
    readonly book: ServerBook,
    readonly races: PhoneRaces,
    readonly scan: ScanFn | null,
  ) {}

  /** The endpoint of the active server, for the connection layer. */
  endpoint(): ServerEndpoint | null {
    const a = this.book.active();
    return a ? endpointOf(a.base) : null;
  }

  private publish(): void {
    const servers = this.book.list();
    useApp.setState({
      servers,
      activeId: this.book.active()?.id ?? null,
      paired: servers.filter((s) => this.book.isPaired(s.id)).map((s) => s.id),
    });
  }

  /** Reads « Mes serveurs » (before the first render). */
  async load(): Promise<void> {
    await this.book.load();
    this.publish();
    if (this.book.active()) useApp.setState({ status: "checking" });
  }

  /** Tests the active server, then follows the connection. */
  async start(): Promise<void> {
    // A server that stops answering (PC switched off): test it again after a few seconds.
    useTelemetry.subscribe((s, prev) => {
      if (prev.socket === "open" && s.socket !== "open") {
        if (this.dropTimer) clearTimeout(this.dropTimer);
        this.dropTimer = setTimeout(() => {
          this.dropTimer = null;
          if (useTelemetry.getState().socket !== "open") void this.refresh();
        }, 4_000);
      }
    });
    await this.refresh();
  }

  /** Tests the active server and (re)connects to it, or explains why not. */
  refresh(): Promise<void> {
    if (this.refreshing) return this.refreshing;
    this.refreshing = this.doRefresh().finally(() => {
      this.refreshing = null;
    });
    return this.refreshing;
  }

  private async doRefresh(): Promise<void> {
    const active = this.book.active();
    if (!active) {
      connection.connect();
      useHub.setState({ role: "unknown", paired: false, device: null, checked: true });
      useApp.setState({ status: "none", statusMessage: null, version: null });
      this.publish();
      return;
    }
    useApp.setState({ status: "checking", statusMessage: null });
    const r = await this.book.probe(active.id);
    const { status, message } = statusOf(r);
    useApp.setState({
      status,
      statusMessage: message,
      version: r.kind === "ok" ? r.version : null,
    });
    this.publish();
    if (r.kind === "ok") {
      if (useTelemetry.getState().socket !== "open" || connection.endpoint.http !== active.base) {
        connection.connect();
      }
      await useHub.getState().check();
    } else {
      connection.disconnect();
      useTelemetry.getState().setSocket("closed", 0);
    }
  }

  async add(input: string, name: string) {
    const r = await this.book.add(input, name);
    this.publish();
    if (r.ok && this.book.active()?.id === r.entry.id) await this.refresh();
    return r;
  }

  async remove(id: string): Promise<void> {
    const wasActive = this.book.active()?.id === id;
    await this.book.remove(id);
    this.publish();
    if (wasActive) await this.refresh();
  }

  async rename(id: string, name: string): Promise<void> {
    await this.book.rename(id, name);
    this.publish();
  }

  async setActive(id: string): Promise<void> {
    await this.book.setActive(id);
    useHub.setState({ role: "unknown", paired: false, device: null, checked: false });
    this.publish();
    await this.refresh();
  }

  /** Tests any server of the list (the active one also reconnects). */
  async test(id: string): Promise<ProbeResult> {
    if (this.book.active()?.id === id) {
      await this.refresh();
    }
    const r = await this.book.probe(id);
    this.publish();
    return r;
  }

  async pair(id: string, code: string, deviceName: string): Promise<PairResult> {
    const r = await this.book.pair(id, code, deviceName);
    this.publish();
    if (r.ok && this.book.active()?.id === id) await this.refresh();
    return r;
  }

  async unpair(id: string) {
    const r = await this.book.unpair(id);
    this.publish();
    if (this.book.active()?.id === id) await this.refresh();
    return r;
  }

  /** The application came back to the front: the server may have changed, reminders too. */
  async resume(): Promise<void> {
    void this.races.replan();
    useApp.setState({ permission: await this.races.permission().catch(() => null) });
    if (useTelemetry.getState().socket !== "open") await this.refresh();
  }
}

let controller: AppController | null = null;

export function setController(c: AppController): void {
  controller = c;
}

/** The application's controller (installed by `mobile/boot.ts` before the first render). */
export function appController(): AppController {
  if (!controller) throw new Error("application controller not started");
  return controller;
}
