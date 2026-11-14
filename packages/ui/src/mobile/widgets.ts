import type { RacePick, RaceSettings } from "../net/races-api";
import { CAT_ORDER, shownRaces } from "../races/model";
import { type Cat, type Level, type Race, viewRaces } from "../races/parse";
import type { Lang } from "../store/settings";

/**
 * What the iPhone application hands to its home-screen / lock-screen widgets and to its Live
 * Activity (`apps/mobile/ios/App/Shared/WidgetData.swift` decodes it). The widgets never read
 * the network: they only show this block, written by the application into a shared keychain
 * item (`SharedStore.swift`) each time the schedule, the picks, the filters or the language
 * change. Described in docs/IOS.md, « Widgets et Dynamic Island ».
 *
 * Format version 1 (`v: 1`), JSON, times in **milliseconds since the epoch (UTC)**; the widgets
 * show them in the phone's time zone. A field the widgets do not know is ignored; another
 * version is shown as « Ouvre Corde pour actualiser ». **Never a secret in here**: no server
 * address, no pairing token — only the public schedule, the driver's picks and the language.
 *
 * The daily races repeat: the pages list the next hours only, so the application extends them
 * ({@link extendDaily}) for the widgets to stay right for a couple of days without opening
 * Corde. Those starts are marked as estimated (`x`).
 */

export const WIDGET_VERSION = 1;
const MIN = 60e3;
const HOUR = 3600e3;
const DAY = 864e5;

/** Daily starts written ahead (known + estimated). */
export const DAILY_HORIZON_MS = 48 * HOUR;
/** Weekly races, championships, special events: written when they begin within this. */
export const OTHER_HORIZON_MS = 7 * DAY;
/** Largest block written (the widget extension is small; the keychain item too). */
export const MAX_PAYLOAD_BYTES = 48 * 1024;
/** Estimated starts per race at most (a race every 10 minutes for 48 h is 288). */
const MAX_PER_RACE = 300;

export type ScheduleState =
  /** The driver has not turned the online schedule on. */
  | "off"
  /** Read (now or from the last good copy). */
  | "on"
  /** Turned on, but nothing could be read yet. */
  | "none";

export interface WidgetRace {
  /** Race id of the schedule (`races/parse.ts`). */
  id: string;
  cat: Cat;
  lvl: Level | null;
  name: string;
  track: string;
  /** Car classes (`GT3`, `LMP2`…), may be empty. */
  cls: string[];
  /** `32 min`, `1 h 30`, may be empty. */
  dur: string;
  /** Known starts, sorted. Special event: `[first day 00:00]`. */
  s: number[];
  /** Estimated starts of a daily race (after the known ones), sorted. */
  x?: number[];
  /** Special event: its last day, 23:59. */
  to?: number;
}

export interface WidgetPick {
  /** Pick id `<race_id>@<start_ms>`. */
  id: string;
  race: string;
  cat: Cat;
  lvl: Level | null;
  name: string;
  track: string;
  cls: string[];
  dur: string;
  /** The start (special event: its reminder of the day, 10:00). */
  t: number;
}

export interface WidgetPayload {
  v: typeof WIDGET_VERSION;
  /** When the application wrote it. */
  at: number;
  lang: Lang;
  sched: ScheduleState;
  /**
   * After this, nothing in here is still to come: « Ouvre Corde pour actualiser ». `0` when the
   * block announces nothing (schedule off, no pick): then only its age counts (4 days).
   */
  until: number;
  /** The schedule through the driver's filters (categories, levels), by category then name. */
  races: WidgetRace[];
  /** The driver's picks still to come, by start. */
  mine: WidgetPick[];
}

const sorted = (xs: number[]) => [...xs].sort((a, b) => a - b);

/**
 * The starts of a daily race over `[now, now + horizon]`: the known ones, then estimated ones.
 * Three or more known starts at a constant interval (± 1 min) go on at that interval; anything
 * else repeats the known starts every 24 h. Nothing is estimated from fewer than one start.
 */
export function extendDaily(
  starts: number[],
  now: number,
  horizon = DAILY_HORIZON_MS,
): { s: number[]; x: number[] } {
  const known = sorted(starts).filter((t) => t > now - MIN && t <= now + horizon);
  const all = sorted(starts);
  const x: number[] = [];
  const last = all.at(-1);
  if (last === undefined) return { s: known, x };
  const end = now + horizon;
  const gaps = all.slice(1).map((t, i) => t - (all[i] as number));
  const step = gaps[0];
  const periodic =
    all.length >= 3 &&
    step !== undefined &&
    step >= 5 * MIN &&
    step <= 12 * HOUR &&
    gaps.every((g) => Math.abs(g - step) <= MIN);
  if (periodic) {
    for (let t = last + step; t <= end && x.length < MAX_PER_RACE; t += step) {
      if (t > now) x.push(t);
    }
  } else {
    for (let k = 1; k <= 3 && x.length < MAX_PER_RACE; k++) {
      for (const t of all) {
        const u = t + k * DAY;
        if (u > last && u > now && u <= end) x.push(u);
      }
    }
  }
  return { s: known, x: sorted([...new Set(x)]) };
}

const catRank = (c: Cat) => {
  const i = CAT_ORDER.indexOf(c);
  return i < 0 ? CAT_ORDER.length : i;
};

/**
 * The schedule as the widgets get it: the hour correction applied, the driver's filters
 * applied, past starts gone, daily races extended, the others limited to the next 7 days.
 */
export function widgetRaces(
  races: Race[],
  settings: RaceSettings,
  now: number,
  dailyHorizon = DAILY_HORIZON_MS,
): WidgetRace[] {
  const out: WidgetRace[] = [];
  for (const r of shownRaces(viewRaces(races, settings.hour_fix, now), settings)) {
    const base = {
      id: r.id,
      cat: r.cat,
      lvl: r.level,
      name: r.name,
      track: r.track,
      cls: r.classes.slice(0, 6),
      dur: r.duration,
    };
    if (r.cat === "special") {
      const from = r.from as number;
      const to = r.to as number;
      if (to > now && from - now < OTHER_HORIZON_MS) out.push({ ...base, s: [from], to });
      continue;
    }
    if (r.cat === "daily") {
      const { s, x } = extendDaily(r.starts, now, dailyHorizon);
      if (s.length || x.length) out.push(x.length ? { ...base, s, x } : { ...base, s });
      continue;
    }
    const s = r.starts.filter((t) => t > now && t - now < OTHER_HORIZON_MS);
    if (s.length) out.push({ ...base, s });
  }
  return out.sort(
    (a, b) => catRank(a.cat) - catRank(b.cat) || (a.id < b.id ? -1 : a.id > b.id ? 1 : 0),
  );
}

/** The picks still to come (a special event: until its last day), by start. */
export function widgetPicks(picks: RacePick[], races: Race[], now: number): WidgetPick[] {
  const byId = new Map(races.map((r) => [r.id, r]));
  return picks
    .filter((p) => {
      if (p.cat !== "special") return p.start_ms > now;
      const to = byId.get(p.race_id)?.to;
      return typeof to === "number" ? to > now : p.start_ms > now;
    })
    .sort((a, b) => a.start_ms - b.start_ms || (a.id < b.id ? -1 : 1))
    .map((p) => {
      const r = byId.get(p.race_id);
      return {
        id: p.id,
        race: p.race_id,
        cat: p.cat,
        lvl: p.level,
        name: p.name,
        track: p.track,
        cls: (r?.classes ?? []).slice(0, 6),
        dur: p.duration,
        t: p.start_ms,
      };
    });
}

export interface PayloadInput {
  /** The schedule (`buildSchedule(...).schedule.races`), or `null` when none could be read. */
  races: Race[] | null;
  /** The driver turned the schedule on. */
  enabled: boolean;
  picks: RacePick[];
  settings: RaceSettings;
  lang: Lang;
  now: number;
}

/** Last instant at which something of the block is still to come (0: nothing at all). */
function lastInstant(races: WidgetRace[], mine: WidgetPick[]): number {
  let until = 0;
  for (const r of races) {
    const last = Math.max(r.s.at(-1) ?? 0, r.x?.at(-1) ?? 0, r.to ?? 0);
    if (last > until) until = last;
  }
  for (const p of mine) if (p.t > until) until = p.t;
  return until;
}

const size = (p: WidgetPayload) => new TextEncoder().encode(JSON.stringify(p)).length;

/**
 * The block for the widgets. Kept under {@link MAX_PAYLOAD_BYTES}: estimated starts are
 * written for a shorter time first, then the longest lists are cut.
 */
export function buildPayload(input: PayloadInput): WidgetPayload {
  const { now } = input;
  const sched: ScheduleState = !input.enabled ? "off" : input.races ? "on" : "none";
  const all = input.enabled && input.races ? input.races : [];
  const mine = widgetPicks(input.picks, all, now).slice(0, 50);
  const make = (races: WidgetRace[]): WidgetPayload => ({
    v: WIDGET_VERSION,
    at: now,
    lang: input.lang,
    sched,
    until: lastInstant(races, mine),
    races,
    mine,
  });
  for (const horizon of [DAILY_HORIZON_MS, 24 * HOUR, 12 * HOUR, 6 * HOUR]) {
    const p = make(widgetRaces(all, input.settings, now, horizon));
    if (size(p) <= MAX_PAYLOAD_BYTES) return p;
  }
  // Still too large (a page with hundreds of races): the first races only.
  let races = widgetRaces(all, input.settings, now, 6 * HOUR);
  while (races.length > 0 && size(make(races)) > MAX_PAYLOAD_BYTES) {
    races = races.slice(0, Math.floor(races.length * 0.8));
  }
  return make(races);
}

/** The block without its writing time: two blocks that compare equal show the same thing. */
export const payloadKey = (p: WidgetPayload): string => JSON.stringify({ ...p, at: 0 });

// ------------------------------------------------------------------ what a widget shows

export interface ListedRace {
  race: WidgetRace;
  /** The start shown (special event: its first day). */
  start: number;
  /** An estimated start of a daily race. */
  estimated: boolean;
  /** A special event under way. */
  ongoing: boolean;
}

/**
 * What the « Courses » widget lists at the instant `t`: each race once, at its next start
 * after `t` (a special event: its first day, or « en cours » while it runs), by start; the
 * special events under way last. **Mirrored by `WidgetData.swift` (`races(at:)`)**: change
 * both together.
 */
export function racesAt(p: WidgetPayload, t: number, max: number): ListedRace[] {
  const next: ListedRace[] = [];
  const ongoing: ListedRace[] = [];
  for (const race of p.races) {
    if (race.cat === "special") {
      const from = race.s[0];
      if (from === undefined) continue;
      if (from > t) next.push({ race, start: from, estimated: false, ongoing: false });
      else if ((race.to ?? 0) > t)
        ongoing.push({ race, start: from, estimated: false, ongoing: true });
      continue;
    }
    const known = race.s.find((s) => s > t);
    const guess = race.x?.find((s) => s > t);
    if (known !== undefined) next.push({ race, start: known, estimated: false, ongoing: false });
    else if (guess !== undefined)
      next.push({ race, start: guess, estimated: true, ongoing: false });
  }
  next.sort(
    (a, b) =>
      a.start - b.start ||
      catRank(a.race.cat) - catRank(b.race.cat) ||
      (a.race.id < b.race.id ? -1 : 1),
  );
  return [...next, ...ongoing].slice(0, max);
}

/** The picks still to come at `t` (`WidgetData.swift`, `picks(at:)`). */
export function picksAt(p: WidgetPayload, t: number): WidgetPick[] {
  return p.mine.filter((x) => x.t > t);
}

// ------------------------------------------------------------------ Live Activity rules

/** Automatic start: the next picked start is within this. */
export const AUTO_LEAD_MS = 2 * HOUR;
/** The button on a pick works this long before its start. */
export const MANUAL_LEAD_MS = 8 * HOUR;
/** An activity stays this long after its start (« Départ donné »), then ends at the next opening. */
export const AFTER_START_MS = 2 * MIN;

/** A running activity, as the native side lists it. */
export interface RunningActivity {
  id: string;
  pickId: string;
  start: number;
}

export type ActivityEvent =
  /** The application came to the front (launch, return). */
  | { kind: "foreground" }
  /** The picks changed (added, removed, hour correction). */
  | { kind: "picks" }
  /** The setting was turned on or off. */
  | { kind: "setting" }
  /** « Compte à rebours » on a pick. */
  | { kind: "manual"; pickId: string };

export interface ActivityInput {
  event: ActivityEvent;
  picks: RacePick[];
  running: RunningActivity[];
  /** The driver's setting (on by default). */
  enabled: boolean;
  /** iOS allows Live Activities for Corde (`areActivitiesEnabled`). */
  allowed: boolean;
  now: number;
}

export interface ActivityPlan {
  /** Activities to end (ids). */
  end: string[];
  /** The pick to show (a new activity), `null` for none. */
  start: RacePick | null;
  /** Why (logged, and shown by the manual button when nothing starts). */
  reason:
    | "off"
    | "not_allowed"
    | "kept"
    | "started"
    | "none"
    | "manual_too_early"
    | "manual_unknown"
    | "manual_past";
}

/** Picks a countdown can follow: real starts (not the reminder of a special event), to come. */
const countable = (picks: RacePick[], now: number) =>
  picks
    .filter((p) => p.cat !== "special" && p.start_ms > now)
    .sort((a, b) => a.start_ms - b.start_ms || (a.id < b.id ? -1 : 1));

/**
 * The single Live Activity of the application: for the nearest picked start, started when the
 * application is in front and that start is within 2 h (by hand: within 8 h), ended when the
 * pick goes, the setting goes off, or the application comes back after start + 2 min. Never two
 * at once. Pure: the native side only carries out the plan.
 */
export function planActivity(input: ActivityInput): ActivityPlan {
  const { event, picks, running, now } = input;
  const allIds = running.map((a) => a.id);
  if (!input.enabled) return { end: allIds, start: null, reason: "off" };
  if (!input.allowed) return { end: allIds, start: null, reason: "not_allowed" };

  const pickIds = new Set(picks.map((p) => p.id));
  // Running activities still worth showing: their pick is still there, and their start is not
  // more than 2 min ago. The nearest is kept, every other one ends.
  const alive = running
    .filter((a) => pickIds.has(a.pickId) && a.start + AFTER_START_MS > now)
    .sort((a, b) => a.start - b.start);
  const queue = countable(picks, now);

  let target: RacePick | null = null;
  let reason: ActivityPlan["reason"] = "none";
  if (event.kind === "manual") {
    const p = picks.find((x) => x.id === event.pickId);
    if (!p || p.cat === "special") reason = "manual_unknown";
    else if (p.start_ms <= now) reason = "manual_past";
    else if (p.start_ms - now > MANUAL_LEAD_MS) reason = "manual_too_early";
    else target = p;
  } else {
    const auto = queue[0];
    if (auto && auto.start_ms - now <= AUTO_LEAD_MS) target = auto;
  }

  if (!target) {
    // Nothing new: keep the nearest live one (a countdown started by hand, or « Départ
    // donné » for 2 minutes), unless a nearer pick is due.
    if (event.kind === "manual") {
      const keep = alive[0];
      return { end: allIds.filter((id) => id !== keep?.id), start: null, reason };
    }
    const keep = alive[0];
    if (keep) return { end: allIds.filter((id) => id !== keep.id), start: null, reason: "kept" };
    return { end: allIds, start: null, reason: "none" };
  }

  // The automatic target is not nearer than a countdown still running (the start just given
  // stays shown for its 2 minutes): keep that one.
  const current = alive[0];
  if (current && event.kind !== "manual" && current.start <= target.start_ms) {
    return { end: allIds.filter((id) => id !== current.id), start: null, reason: "kept" };
  }
  const same = running.find((a) => a.pickId === target.id && a.start === target.start_ms);
  if (same) return { end: allIds.filter((id) => id !== same.id), start: null, reason: "kept" };
  return { end: allIds, start: target, reason: "started" };
}
