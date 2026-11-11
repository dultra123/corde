import type {
  ClientMessage,
  GameEvent,
  LapRecord,
  ServerMessage,
  SourceStatus,
  TelemetryFrame,
} from "@corde/protocol";

/** The `type` tags we understand. Anything else is dropped (forward compatibility). */
const KNOWN_TYPES = new Set(["hello", "status", "frame", "lap", "event", "pong"]);

export type ParsedMessage =
  | { kind: "hello"; serverVersion: string; protocolVersion: number; status: SourceStatus }
  | { kind: "status"; status: SourceStatus }
  | { kind: "frame"; frame: TelemetryFrame }
  | { kind: "lap"; lap: LapRecord }
  | { kind: "event"; event: GameEvent }
  | { kind: "pong"; t: number };

const isObject = (v: unknown): v is Record<string, unknown> =>
  typeof v === "object" && v !== null && !Array.isArray(v);

/**
 * Parses one WebSocket text message into a typed payload.
 * Returns `null` for malformed or unknown messages instead of throwing: a dashboard must never
 * crash because the server (or a future version of it) sent something unexpected.
 */
export function parseServerMessage(raw: string | unknown): ParsedMessage | null {
  let data: unknown = raw;
  if (typeof raw === "string") {
    try {
      data = JSON.parse(raw);
    } catch {
      return null;
    }
  }
  if (!isObject(data)) return null;
  const type = data.type;
  if (typeof type !== "string" || !KNOWN_TYPES.has(type)) return null;
  const msg = data as unknown as ServerMessage;

  switch (msg.type) {
    case "hello": {
      if (!isObject(msg.status)) return null;
      return {
        kind: "hello",
        serverVersion: String(msg.server_version ?? ""),
        protocolVersion: Number(msg.protocol_version ?? 0),
        status: msg.status,
      };
    }
    case "status": {
      const { type: _t, ...status } = msg;
      if (typeof status.connected !== "boolean") return null;
      return { kind: "status", status: status as SourceStatus };
    }
    case "frame": {
      const { type: _t, ...frame } = msg;
      if (!isObject(frame.session) || !Array.isArray(frame.standings)) return null;
      if (frame.player !== null && !isObject(frame.player)) return null;
      return { kind: "frame", frame: frame as TelemetryFrame };
    }
    case "lap": {
      const { type: _t, ...lap } = msg;
      if (typeof lap.lap_time_s !== "number") return null;
      return { kind: "lap", lap: lap as LapRecord };
    }
    case "event": {
      const { type: _t, ...event } = msg;
      if (typeof (event as { kind?: unknown }).kind !== "string") return null;
      return { kind: "event", event: event as GameEvent };
    }
    case "pong":
      return { kind: "pong", t: Number(msg.t) };
    default:
      return null;
  }
}

export function encodeClientMessage(msg: ClientMessage): string {
  return JSON.stringify(msg);
}
