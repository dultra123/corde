import type {
  Electronics,
  Energy,
  LapRecord,
  PlayerTelemetry,
  SessionInfo,
  Standing,
} from "@corde/protocol";
import { fmtDelta, fmtGap } from "../lib/format";
import { estimateLaps } from "../lib/fuel";
import type { LiveView } from "../store/settings";

/**
 * The rules of the phone dashboard (`dash/`), without React: which view the Direct page shows,
 * the pages, the rev lights, the flags, the delta and the gaps, the autonomy. Everything is
 * computed from fields of the protocol; a value the game does not give is `null` (shown « — »),
 * never made up.
 */

// ---------------------------------------------------------------- pages

export const DASH_PAGES = ["course", "tyres", "energy", "timing"] as const;
export type DashPage = (typeof DASH_PAGES)[number];

/** Index of a page name (a stored one, a debug request's), the first page when unknown. */
export function pageIndex(name: string | null | undefined): number {
  return Math.max(0, DASH_PAGES.indexOf(name as DashPage));
}

/** The page `delta` pages away from `index`, held within the four (no wrap-around). */
export function stepPage(index: number, delta: number): number {
  const i = Math.round(index) + Math.round(delta);
  return Math.max(0, Math.min(DASH_PAGES.length - 1, Number.isFinite(i) ? i : 0));
}

// ---------------------------------------------------------------- which view

/** A phone: its short side is at most this, in CSS pixels. */
export const PHONE_MAX_SIDE = 500;

export interface ViewContext {
  /** The phone application (`APP_BUILD`). */
  app: boolean;
  /** The main pointer is a finger (`(pointer: coarse)`). */
  coarse: boolean;
  /** Size of the window, CSS pixels. */
  width: number;
  height: number;
  /** `#/dash`: always the phone dashboard. */
  forced: boolean;
}

/** A phone's browser: a touch screen whose short side is small. */
export function isPhone(ctx: Pick<ViewContext, "coarse" | "width" | "height">): boolean {
  return ctx.coarse && Math.min(ctx.width, ctx.height) <= PHONE_MAX_SIDE;
}

/** What the Direct page shows: the phone dashboard or the grid of widgets. */
export function chooseView(setting: LiveView, ctx: ViewContext): "dash" | "widgets" {
  if (ctx.forced) return "dash";
  if (setting === "dash" || setting === "widgets") return setting;
  return ctx.app || isPhone(ctx) ? "dash" : "widgets";
}

/**
 * How the dashboard is shown once chosen: full screen on a phone (and in the phone
 * application, and at `#/dash`), as in 1.1.0; elsewhere (a PC, a tablet's browser) inside the
 * Direct page, under its top bar, unless the driver asked for full screen (`full`).
 */
export function dashPresentation(ctx: ViewContext, full: boolean): "full" | "page" {
  return ctx.forced || ctx.app || isPhone(ctx) || full ? "full" : "page";
}

// ---------------------------------------------------------------- rev lights

export const LED_COUNT = 15;
/** The first light comes on above this fraction of the maximum revs… */
export const LED_START = 0.55;
/** …and all of them at this one. */
export const LED_FULL = 0.96;
/** Shift: the whole bar flashes from this fraction. */
export const SHIFT_AT = 0.97;

export type LedColor = "g" | "y" | "r";

/** Five green, five yellow, five red. */
export const LED_COLORS: readonly LedColor[] = Array.from({ length: LED_COUNT }, (_, i) =>
  i < 5 ? "g" : i < 10 ? "y" : "r",
);

export interface RevState {
  /** Lights on, 0..15. */
  lit: number;
  /** At or past the shift point. */
  shift: boolean;
  /** Pit limiter on: the bar alternates, no revs shown. */
  limiter: boolean;
}

export function revState(rpm: number, maxRpm: number, limiter = false): RevState {
  if (limiter) return { lit: 0, shift: false, limiter: true };
  if (!(maxRpm > 0) || !Number.isFinite(rpm)) return { lit: 0, shift: false, limiter: false };
  const f = rpm / maxRpm;
  const shift = f >= SHIFT_AT;
  let lit: number;
  if (f <= LED_START) lit = 0;
  else if (f >= LED_FULL) lit = LED_COUNT;
  else lit = Math.min(LED_COUNT, Math.ceil(((f - LED_START) / (LED_FULL - LED_START)) * LED_COUNT));
  return { lit, shift, limiter: false };
}

// ---------------------------------------------------------------- flags

export interface DashFlags {
  /** Yellow for the whole session (full course yellow) or in the driver's sector. */
  yellow: boolean;
  /** The driver took the chequered flag. */
  chequered: boolean;
  /** Blue flag shown to the driver (a faster car behind, as the game says). */
  blue: boolean;
}

export const NO_FLAGS: DashFlags = { yellow: false, chequered: false, blue: false };

/**
 * The flags the game gives: the session's yellow (`phase`, `yellow_flag`), the yellow of the
 * driver's sector (`sector_flags`, same order as `current_sector`, as the Session widget reads
 * it), the blue flag of the driver's standing, the chequered flag of their finish status.
 */
export function flagsOf(
  session: Pick<SessionInfo, "phase" | "yellow_flag" | "sector_flags"> | null,
  player: (Pick<PlayerTelemetry, "finish_status"> & { lap: { current_sector: number } }) | null,
  standings: readonly Pick<Standing, "is_player" | "flag_blue">[] = [],
): DashFlags {
  if (!session) return NO_FLAGS;
  const course =
    session.phase === "full_course_yellow" ||
    (session.yellow_flag !== "none" && session.yellow_flag !== "invalid");
  const sector = player ? (session.sector_flags[player.lap.current_sector - 1] ?? 0) > 0 : false;
  return {
    yellow: course || sector,
    chequered: player?.finish_status === "finished",
    blue: standings.some((s) => s.is_player && s.flag_blue),
  };
}

/** The colour of the frame: the most important flag wins. */
export function flagTone(f: DashFlags): "yellow" | "chequered" | "blue" | null {
  if (f.chequered) return "chequered";
  if (f.yellow) return "yellow";
  if (f.blue) return "blue";
  return null;
}

// ---------------------------------------------------------------- delta

/** The delta bar is full at this many seconds. */
export const DELTA_BAR_S = 1;

export type DeltaTone = "faster" | "slower" | "even" | "none";

export interface DeltaView {
  text: string;
  tone: DeltaTone;
  /** −1 (a second or more faster) .. +1 (slower). */
  frac: number;
}

/** Live delta to the best lap; `null` while there is no best lap to compare with. */
export function deltaView(delta: number | null): DeltaView {
  if (delta === null || !Number.isFinite(delta)) return { text: "—", tone: "none", frac: 0 };
  const tone: DeltaTone = Math.abs(delta) < 0.0005 ? "even" : delta < 0 ? "faster" : "slower";
  return {
    text: fmtDelta(delta, 3),
    tone,
    frac: Math.max(-1, Math.min(1, delta / DELTA_BAR_S)),
  };
}

/** The delta of a player, `null` until a best lap exists. */
export function playerDelta(p: Pick<PlayerTelemetry, "lap"> | null): number | null {
  if (!p || p.lap.best_lap_s === null) return null;
  return p.lap.delta_best_s;
}

// ---------------------------------------------------------------- gaps

/** Under this gap a car is « close »: green ahead (in reach), red behind (a threat). */
export const CLOSE_GAP_S = 1;

export interface GapView {
  text: string;
  tone: "close-ahead" | "close-behind" | "plain" | "none";
}

/**
 * Gap to the car ahead (`−1.8`) or behind (`+0.6`) in the race, the sign convention of the
 * Gaps and Standings widgets: negative ahead.
 */
export function gapView(seconds: number | null, side: "ahead" | "behind"): GapView {
  if (seconds === null || !Number.isFinite(seconds)) return { text: "—", tone: "none" };
  const close = Math.abs(seconds) < CLOSE_GAP_S;
  return {
    text: `${side === "ahead" ? "−" : "+"}${fmtGap(seconds)}`,
    tone: close ? (side === "ahead" ? "close-ahead" : "close-behind") : "plain",
  };
}

// ---------------------------------------------------------------- session

/** A real lap limit (timed sessions report 0 or a huge number). */
export function lapLimit(s: Pick<SessionInfo, "max_laps"> | null): number | null {
  if (!s) return null;
  return s.max_laps > 0 && s.max_laps < 10_000 ? s.max_laps : null;
}

/** The session is timed: its remaining time is worth showing. */
export function isTimed(s: Pick<SessionInfo, "end_s"> | null): boolean {
  return !!s && s.end_s > 0;
}

/** Laps left in a lap-limited session (the one in progress included), `null` when timed. */
export function lapsLeft(
  s: Pick<SessionInfo, "max_laps"> | null,
  p: Pick<PlayerTelemetry, "lap"> | null,
): number | null {
  const limit = lapLimit(s);
  if (limit === null || !p) return null;
  return Math.max(0, limit - p.lap.laps_completed);
}

/**
 * Laps that still fit in a timed session, from the average of the recent laps: an estimate,
 * shown with « ≈ ». The lap in progress counts as one (it will be finished).
 */
export function lapsLeftTimed(
  s: Pick<SessionInfo, "end_s" | "time_remaining_s"> | null,
  laps: readonly LapRecord[],
): number | null {
  if (!isTimed(s) || !s) return null;
  const pace = recentPace(laps);
  if (pace === null) return null;
  return Math.max(0, Math.ceil(s.time_remaining_s / pace));
}

/** Average of the last five complete laps, in/out laps left out. */
export function recentPace(laps: readonly LapRecord[], window = 5): number | null {
  const times = laps
    .filter((l) => !l.in_lap && !l.out_lap && l.lap_time_s > 0 && Number.isFinite(l.lap_time_s))
    .slice(-window)
    .map((l) => l.lap_time_s);
  if (times.length === 0) return null;
  return times.reduce((a, b) => a + b, 0) / times.length;
}

// ---------------------------------------------------------------- energy

export interface Autonomy {
  /** Use per lap (litres, or a fraction for the virtual energy). */
  perLap: number;
  /** Laps that the remaining amount covers. */
  laps: number;
}

export function fuelAutonomy(e: Pick<Energy, "fuel_l">, laps: LapRecord[]): Autonomy | null {
  const c = estimateLaps(e.fuel_l, laps, (l) => l.fuel_used_l);
  return c ? { perLap: c.perLap, laps: c.lapsLeftExact } : null;
}

export function energyAutonomy(
  e: Pick<Energy, "virtual_energy">,
  laps: LapRecord[],
): Autonomy | null {
  if (e.virtual_energy === null) return null;
  const c = estimateLaps(e.virtual_energy, laps, (l) => l.virtual_energy_used);
  return c ? { perLap: c.perLap, laps: c.lapsLeftExact } : null;
}

/**
 * Fuel still missing to finish (positive) or spare (negative), litres: laps left × use per
 * lap − fuel on board. `null` while either is unknown.
 */
export function fuelToFinish(
  fuelL: number,
  perLap: number | null,
  laps: number | null,
): number | null {
  if (perLap === null || laps === null || !Number.isFinite(fuelL)) return null;
  return laps * perLap - fuelL;
}

/** The car has a hybrid system the dashboard can show. */
export function hasHybrid(e: Pick<Energy, "battery_soc" | "motor_state">): boolean {
  return e.battery_soc !== null || e.motor_state !== "unavailable";
}

// ---------------------------------------------------------------- electronics

/**
 * A setting the car has, else `null`: its cell is left out. Le Mans Ultimate gives the
 * maximum of each setting (above zero when the car has it); other games give the level
 * without its maximum: a level above zero is then a setting the car has.
 */
export function setting(value: number, max: number): number | null {
  return max > 0 || value > 0 ? value : null;
}

/** A brake temperature, `null` when the game publishes none (it then reads 0). */
export function brakeTemp(temp: number | null | undefined): number | null {
  return temp !== null && temp !== undefined && Number.isFinite(temp) && temp > 0 ? temp : null;
}

/** Front share of the braking, percent. */
export function frontBias(e: Pick<Electronics, "brake_bias_rear">): number | null {
  const v = (1 - e.brake_bias_rear) * 100;
  return Number.isFinite(v) && e.brake_bias_rear >= 0 && e.brake_bias_rear <= 1 ? v : null;
}

// ---------------------------------------------------------------- standings

/** Number of classes on track: the class position is only worth showing with two or more. */
export function classCount(standings: readonly Pick<Standing, "class_name">[]): number {
  return new Set(standings.map((s) => s.class_name)).size;
}
