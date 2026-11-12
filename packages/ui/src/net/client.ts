import type { ClientMessage } from "@corde/protocol";
import { encodeClientMessage, type ParsedMessage, parseServerMessage } from "./messages";
import { RateMeter } from "./rate";

export type SocketState = "idle" | "connecting" | "open" | "closed";

export interface ClientEvents {
  onMessage(msg: ParsedMessage): void;
  onState(state: SocketState, attempt: number): void;
}

export interface ClientOptions {
  url: string;
  rateHz: number;
  /** Reconnection delays grow from `minDelayMs` to `maxDelayMs`. */
  minDelayMs?: number;
  maxDelayMs?: number;
  pingIntervalMs?: number;
  /**
   * WebSocket subprotocols, read at each (re)connection (iPhone application on the home
   * server: its device token, `net/auth`). `undefined`: none, as in the browser.
   */
  protocols?: () => string[] | undefined;
}

/**
 * WebSocket client for corde-server.
 *
 * - Reconnects with exponential backoff (+ jitter) until `close()` is called.
 * - Parses every `ServerMessage`, drops malformed ones silently (counted in `dropped`).
 * - Measures the incoming frame rate (`frameHz`) and the ping round trip (`rttMs`).
 * - Sends `set_rate` on connect and whenever `setRate()` changes it.
 */
export class CordeClient {
  private ws: WebSocket | null = null;
  private attempt = 0;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private pingTimer: ReturnType<typeof setInterval> | null = null;
  private stopped = false;
  private readonly meter = new RateMeter(2000);
  private readonly opts: Required<ClientOptions>;

  state: SocketState = "idle";
  dropped = 0;
  rttMs: number | null = null;

  constructor(
    opts: ClientOptions,
    private readonly events: ClientEvents,
  ) {
    this.opts = {
      minDelayMs: 500,
      maxDelayMs: 10_000,
      pingIntervalMs: 5_000,
      protocols: () => undefined,
      ...opts,
    };
  }

  get url(): string {
    return this.opts.url;
  }

  get frameHz(): number {
    return this.meter.hz();
  }

  connect(): void {
    this.stopped = false;
    this.open();
  }

  /** Stops reconnecting and closes the socket. */
  close(): void {
    this.stopped = true;
    this.clearTimers();
    if (this.ws) {
      const ws = this.ws;
      this.ws = null;
      ws.onopen = ws.onmessage = ws.onclose = ws.onerror = null;
      try {
        ws.close();
      } catch {
        // already closed
      }
    }
    this.setState("closed");
  }

  setRate(hz: number): void {
    const clamped = Math.max(1, Math.min(60, Math.round(hz)));
    this.opts.rateHz = clamped;
    this.send({ type: "set_rate", hz: clamped });
  }

  send(msg: ClientMessage): boolean {
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) return false;
    try {
      this.ws.send(encodeClientMessage(msg));
      return true;
    } catch {
      return false;
    }
  }

  private open(): void {
    if (this.stopped) return;
    this.clearTimers();
    this.setState("connecting");
    let ws: WebSocket;
    try {
      const protocols = this.opts.protocols();
      ws = protocols ? new WebSocket(this.opts.url, protocols) : new WebSocket(this.opts.url);
    } catch {
      this.scheduleReconnect();
      return;
    }
    this.ws = ws;
    ws.onopen = () => {
      if (this.ws !== ws) return;
      this.attempt = 0;
      this.meter.reset();
      this.setState("open");
      this.send({ type: "set_rate", hz: this.opts.rateHz });
      this.pingTimer = setInterval(
        () => this.send({ type: "ping", t: Date.now() }),
        this.opts.pingIntervalMs,
      );
    };
    ws.onmessage = (ev: MessageEvent) => {
      if (this.ws !== ws) return;
      const parsed = parseServerMessage(typeof ev.data === "string" ? ev.data : null);
      if (!parsed) {
        this.dropped += 1;
        return;
      }
      if (parsed.kind === "frame") this.meter.tick();
      if (parsed.kind === "pong" && Number.isFinite(parsed.t)) this.rttMs = Date.now() - parsed.t;
      this.events.onMessage(parsed);
    };
    ws.onerror = () => {
      // onclose always follows; nothing to do here
    };
    ws.onclose = () => {
      if (this.ws !== ws) return;
      this.ws = null;
      this.clearTimers();
      this.setState("closed");
      this.scheduleReconnect();
    };
  }

  private scheduleReconnect(): void {
    if (this.stopped) return;
    const { minDelayMs, maxDelayMs } = this.opts;
    const base = Math.min(maxDelayMs, minDelayMs * 2 ** this.attempt);
    const jitter = base * 0.2 * Math.random();
    this.attempt += 1;
    this.reconnectTimer = setTimeout(() => this.open(), base + jitter);
  }

  private clearTimers(): void {
    if (this.reconnectTimer) clearTimeout(this.reconnectTimer);
    if (this.pingTimer) clearInterval(this.pingTimer);
    this.reconnectTimer = null;
    this.pingTimer = null;
  }

  private setState(s: SocketState): void {
    if (this.state === s) return;
    this.state = s;
    this.events.onState(s, this.attempt);
  }
}
