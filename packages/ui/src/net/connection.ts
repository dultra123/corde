import { DemoRunner } from "../demo/runner";
import { useRaces } from "../store/races";
import { useSettings } from "../store/settings";
import { useTelemetry } from "../store/telemetry";
import { wsProtocols } from "./auth";
import { CordeClient } from "./client";
import { syncLayoutsWithServer } from "./layout-sync";
import type { ParsedMessage } from "./messages";
import { isDemoForced, resolveEndpoint, type ServerEndpoint } from "./url";

const AUTO_DEMO_AFTER_MS = 3000;
const RATE_REFRESH_MS = 500;
/** What an overlay asks the server for: enough for a video at 30 or 60 fps, half of the 60 Hz cap. */
export const OVERLAY_RATE_HZ = 30;

export interface StartOptions {
  /**
   * OBS browser source (`#/overlay/…`): 30 Hz, never the demo race on its own (a stream must
   * not show invented data), never writes layouts back to the server, reconnects within 3 s.
   */
  overlay?: boolean;
  /** With the demo (`?demo=1`): stop it after this delay and keep the last picture (previews). */
  freezeDemoAfterMs?: number;
  /**
   * iPhone application: never start the demo race on its own (no server chosen is not a
   * reason to show invented data; the pages say how to add a server).
   */
  noAutoDemo?: boolean;
}

/**
 * Where to connect, when it is not the page's own server (`resolveEndpoint`): the iPhone
 * application gives the active server of « Mes serveurs », or `null` when there is none.
 */
export type EndpointSource = () => ServerEndpoint | null;

/**
 * Owns the WebSocket client and the demo generator and decides which one feeds the store:
 *
 * - `?demo=1` or the demo switch → demo only ("manual"), socket closed.
 * - otherwise connect; if no server answers within 3 s (or it drops for 3 s) → demo "auto",
 *   the socket keeps retrying in the background and the demo stops as soon as a `hello` arrives.
 */
class ConnectionManager {
  private client: CordeClient | null = null;
  private demo: DemoRunner | null = null;
  private autoDemoTimer: ReturnType<typeof setTimeout> | null = null;
  endpoint: ServerEndpoint = resolveEndpoint("");
  /** `false` only in the application while no server is chosen (`request` then fails at once). */
  hasEndpoint = true;
  private overlay = false;
  private noAutoDemo = false;
  private source: EndpointSource | null = null;

  /** The application's servers (`mobile/servers`); set before `start`. */
  setEndpointSource(source: EndpointSource | null): void {
    this.source = source;
  }

  start(opts: StartOptions = {}): void {
    this.overlay = opts.overlay ?? false;
    this.noAutoDemo = opts.noAutoDemo ?? false;
    setInterval(() => this.refreshRates(), RATE_REFRESH_MS);
    if (isDemoForced()) {
      this.setDemo(true);
      const freeze = opts.freezeDemoAfterMs;
      if (freeze !== undefined) setTimeout(() => this.demo?.stop(), freeze);
      return;
    }
    this.connect();
  }

  /** The page is an OBS overlay (see {@link StartOptions.overlay}). */
  get isOverlay(): boolean {
    return this.overlay;
  }

  /** (Re)connects to the address currently configured in settings. */
  connect(): void {
    this.disconnect();
    if (this.source) {
      const ep = this.source();
      this.hasEndpoint = ep !== null;
      if (!ep) {
        useTelemetry.getState().setSocket("closed", 0);
        return;
      }
      this.endpoint = ep;
    } else {
      this.endpoint = resolveEndpoint(useSettings.getState().server);
    }
    const telemetry = useTelemetry.getState();
    telemetry.setSocket("connecting", 0);
    this.client = new CordeClient(
      this.overlay
        ? { url: this.endpoint.ws, rateHz: OVERLAY_RATE_HZ, maxDelayMs: 3000 }
        : { url: this.endpoint.ws, rateHz: useSettings.getState().rateHz, protocols: wsProtocols },
      {
        onMessage: (m) => this.onMessage(m),
        onState: (state, attempt) => {
          useTelemetry.getState().setSocket(state, attempt);
          if (state === "open") this.clearAutoDemoTimer();
          else this.armAutoDemo();
        },
      },
    );
    this.client.connect();
    this.armAutoDemo();
  }

  disconnect(): void {
    this.clearAutoDemoTimer();
    if (this.client) {
      this.client.close();
      this.client = null;
    }
  }

  setRate(hz: number): void {
    if (!this.overlay) this.client?.setRate(hz);
  }

  /** Switches the demo on (manual) or off (reconnects to the server). */
  setDemo(on: boolean): void {
    if (on) {
      this.disconnect();
      useTelemetry.getState().setSocket("closed", 0);
      this.startDemo("manual");
    } else {
      this.stopDemo();
      this.connect();
    }
  }

  get demoRunning(): boolean {
    return this.demo?.running ?? false;
  }

  private onMessage(m: ParsedMessage): void {
    const s = useTelemetry.getState();
    switch (m.kind) {
      case "hello":
        if (s.demo === "auto") this.stopDemo();
        s.setHello({
          serverVersion: m.serverVersion,
          protocolVersion: m.protocolVersion,
          status: m.status,
        });
        // An overlay only reads the layout it shows (`overlay/OverlayApp.tsx`).
        if (!this.overlay) void syncLayoutsWithServer(this.endpoint.http);
        break;
      case "status":
        if (s.demo === "off") s.setStatus(m.status);
        break;
      case "frame":
        if (s.demo === "off") s.pushFrame(m.frame);
        break;
      case "lap":
        if (s.demo === "off") s.pushLap(m.lap);
        break;
      case "event":
        if (s.demo === "off") s.pushEvent(m.event);
        break;
      case "pong":
        break;
      case "race_reminder":
        // Shown whatever the page and the demo: it is about the driver's evening, not the stream.
        // The iPhone application has its own picks and reminders (local notifications): the
        // ones of a PC it is connected to are that PC's.
        if (useRaces.getState().serverReminders) useRaces.getState().pushReminder(m.reminder);
        break;
    }
  }

  private armAutoDemo(): void {
    if (this.noAutoDemo) return;
    if (this.overlay || this.autoDemoTimer || useTelemetry.getState().demo !== "off") return;
    this.autoDemoTimer = setTimeout(() => {
      this.autoDemoTimer = null;
      const s = useTelemetry.getState();
      if (s.socket !== "open" && s.demo === "off") this.startDemo("auto");
    }, AUTO_DEMO_AFTER_MS);
  }

  private clearAutoDemoTimer(): void {
    if (this.autoDemoTimer) clearTimeout(this.autoDemoTimer);
    this.autoDemoTimer = null;
  }

  private startDemo(mode: "auto" | "manual"): void {
    if (this.demo) {
      useTelemetry.getState().setDemo(mode);
      return;
    }
    const s = useTelemetry.getState();
    s.resetData();
    s.setDemo(mode);
    this.demo = new DemoRunner({
      onFrame: (f) => useTelemetry.getState().pushFrame(f),
      onLap: (l) => useTelemetry.getState().pushLap(l),
      onEvent: (e) => useTelemetry.getState().pushEvent(e),
      onStatus: (st) => useTelemetry.getState().setStatus(st),
    });
    this.demo.start();
  }

  private stopDemo(): void {
    if (!this.demo) return;
    this.demo.stop();
    this.demo = null;
    const s = useTelemetry.getState();
    s.resetData();
    s.setDemo("off");
    s.setStatus({
      connected: false,
      game: "unknown",
      source: "",
      input_rate_hz: 0,
      output_rate_hz: 0,
      in_realtime: false,
      game_version: null,
      last_error: null,
      clients: 0,
      recording: false,
    });
  }

  private refreshRates(): void {
    const s = useTelemetry.getState();
    if (this.demo?.running) {
      s.setRates(this.demo.hz, null);
    } else if (this.client) {
      s.setRates(this.client.state === "open" ? this.client.frameHz : 0, this.client.rttMs);
    }
  }
}

export const connection = new ConnectionManager();

/** Re-apply settings that affect the connection when the user changes them. */
useSettings.subscribe((cur, prev) => {
  if (cur.rateHz !== prev.rateHz) connection.setRate(cur.rateHz);
  if (cur.server !== prev.server && useTelemetry.getState().demo !== "manual") connection.connect();
});
