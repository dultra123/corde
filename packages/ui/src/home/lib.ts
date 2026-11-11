import type { ResultRow } from "../net/profile-api";
import type { RecordingStatus, SessionRow } from "../net/sessions-api";
import type { NetworkInfo } from "../net/system-api";
import type { ConnectionPhase } from "../store/telemetry";

/** Pure helpers of the home page. */

/* ------------------------------------------------------------------ status */

/** Where things stand, in the words of the home page. */
export type HomePhase =
  /** Looking for the Corde server. */
  | "connecting"
  /** No server answers (the demo may be running in the background). */
  | "offline"
  /** The server is there, the game is not. */
  | "waiting"
  /** The game runs but no session is loaded (menus). */
  | "menu"
  /** A session is loaded, the driver is not on track. */
  | "garage"
  /** The driver is on track. */
  | "track"
  /** Demo mode asked for by the user. */
  | "demo";

export function homePhase(phase: ConnectionPhase, hasSession: boolean): HomePhase {
  switch (phase) {
    case "connecting":
      return "connecting";
    case "offline":
    case "demo-auto":
      return "offline";
    case "demo":
      return "demo";
    case "waiting-game":
      return "waiting";
    case "monitor":
      return hasSession ? "garage" : "menu";
    case "live":
      return "track";
  }
}

/** One light of the status row: done, the step being waited for, broken, or not there yet. */
export type Lamp = "on" | "wait" | "error" | "rec" | "off";

export interface Lamps {
  server: Lamp;
  game: Lamp;
  session: Lamp;
  track: Lamp;
  recording: Lamp;
}

type RecLike = Pick<RecordingStatus, "recording" | "armed" | "idle_reason">;

/**
 * The chain that must light up for Corde to do its job: server → game → session → on track →
 * recording. The first step that is missing is the one being waited for.
 */
export function homeLamps(phase: HomePhase, rec: RecLike | null): Lamps {
  const reached: Record<HomePhase, number> = {
    connecting: 0,
    offline: 0,
    demo: 0,
    waiting: 1,
    menu: 2,
    garage: 3,
    track: 4,
  };
  const n = reached[phase];
  const lamp = (index: number): Lamp => (index < n ? "on" : index === n ? "wait" : "off");
  let recording: Lamp = "off";
  if (n > 0 && rec) {
    if (rec.recording) recording = rec.idle_reason ? "wait" : "rec";
    else if (rec.armed) recording = "wait";
  }
  return {
    server: phase === "offline" ? "error" : phase === "demo" ? "off" : lamp(0),
    game: phase === "demo" ? "off" : lamp(1),
    session: lamp(2),
    track: lamp(3),
    recording,
  };
}

/* ------------------------------------------------------------------ last session ↔ result */

/** A game result belongs to a recorded session when it started within this window. */
export const RESULT_WINDOW_MS = 10 * 60_000;

/** Same circuit, however each side spells it (`Paul Ricard - 1A` / `Circuit Paul Ricard`). */
export function sameTrack(a: string, b: string): boolean {
  const x = a.trim().toLowerCase();
  const y = b.trim().toLowerCase();
  if (!x || !y) return false;
  return x === y || x.includes(y) || y.includes(x);
}

/**
 * The game result of a recorded session: same circuit, same session type, started within
 * ±10 min (the closest wins). `null` when the game's result file has not been imported — or
 * when the session was not finished.
 */
export function matchResult(
  session: Pick<SessionRow, "track_name" | "session_type" | "started_wall_ms">,
  results: readonly ResultRow[],
): ResultRow | null {
  let best: ResultRow | null = null;
  let bestGap = Number.POSITIVE_INFINITY;
  for (const r of results) {
    if (r.session_kind !== session.session_type) continue;
    if (
      !sameTrack(r.track_course, session.track_name) &&
      !sameTrack(r.track_venue, session.track_name)
    )
      continue;
    const gap = Math.abs(r.started_wall_ms - session.started_wall_ms);
    if (gap > RESULT_WINDOW_MS || gap >= bestGap) continue;
    best = r;
    bestGap = gap;
  }
  return best;
}

/** The last `n` race results, most recent first. */
export function lastRaces(results: readonly ResultRow[], n = 5): ResultRow[] {
  return results
    .filter((r) => r.session_kind === "race")
    .sort((a, b) => b.started_wall_ms - a.started_wall_ms)
    .slice(0, n);
}

/** French ordinal suffix (`1` → `er`, others → `e`), English one (`st`, `nd`, `rd`, `th`). */
export function ordinalSuffix(n: number, lang: "fr" | "en"): string {
  if (lang === "fr") return n === 1 ? "er" : "e";
  const rem10 = n % 10;
  const rem100 = n % 100;
  if (rem10 === 1 && rem100 !== 11) return "st";
  if (rem10 === 2 && rem100 !== 12) return "nd";
  if (rem10 === 3 && rem100 !== 13) return "rd";
  return "th";
}

/* ------------------------------------------------------------------ phone access */

export type PhoneState =
  /** Reachable from the LAN at these addresses. `stopping`: switched off, until the restart. */
  | { kind: "on"; urls: string[]; stopping: boolean }
  /** Listening on the LAN but this PC has no usable IPv4 address (cable unplugged, Wi-Fi off). */
  | { kind: "no-address"; stopping: boolean }
  /** Switched on, applied at the next start. */
  | { kind: "restart" }
  /** This PC only. */
  | { kind: "off" };

export function phoneState(net: NetworkInfo): PhoneState {
  if (net.listening_on_lan) {
    const stopping = !net.lan_access_setting;
    return net.urls.length > 0
      ? { kind: "on", urls: net.urls, stopping }
      : { kind: "no-address", stopping };
  }
  return net.lan_access_setting ? { kind: "restart" } : { kind: "off" };
}

/** `http://192.168.1.20:8471` → `192.168.1.20:8471` (what is shown in large type). */
export function bareAddress(url: string): string {
  return url.replace(/^https?:\/\//i, "").replace(/\/+$/, "");
}
