import { translate } from "../i18n";
import type { RacePick, RaceSettings } from "../net/races-api";
import type { Lang } from "../store/settings";
import { relLabel } from "./format";
import { CAT_ORDER, pickId, safeImage, shownRaces } from "./model";
import { type Cat, type Level, type Race, viewRaces } from "./parse";

/**
 * The « Courses en ligne » carousel of the home page: which races it shows, in which order, and
 * the small rules around it (countdown wording, picture fallback, when it may turn by itself).
 * Same choice as the owner's iPhone widget — the driver's own races first, then the next starts
 * of the schedule through the same filters — with the order of the categories made explicit.
 * Pure functions: the component only renders what comes out of here.
 */

const SEC = 1e3;
const MIN = 60e3;
const HOUR = 3600e3;
const DAY = 864e5;

/** At most this many slides. */
export const MAX_SLIDES = 8;
/** One slide every 8 s while nothing holds the carousel. */
export const SLIDE_MS = 8_000;
/** Weekly races and championships: shown when they start within 48 h (as in the widget). */
export const TIMED_HORIZON_MS = 48 * HOUR;
/** Special events: shown when they begin within 7 days, or while they run (as in the widget). */
export const SPECIAL_HORIZON_MS = 7 * DAY;

export interface Slide {
  /** The race id: one slide per race, never two. */
  key: string;
  cat: Cat;
  level: Level | null;
  name: string;
  track: string;
  duration: string;
  /** A safe `https://` picture, or null (the drawn stand-in is used). */
  image: string | null;
  /** The start shown; for a special event, its first day (or its reminder when unknown). */
  start: number;
  /** Special events: their last day. */
  end: number | null;
  /** The race of the schedule, when known (needed to pick a start). */
  race: Race | null;
  /** The driver's pick shown on this slide (« Mes courses »). */
  pick: RacePick | null;
  /** The pick id the slide's button adds or removes. */
  pickId: string;
  /** The shown start is one of the driver's picks. */
  picked: boolean;
}

export interface SlideInput {
  /** The schedule as parsed (`buildSchedule(...).schedule.races`); empty when off or not read. */
  races: Race[];
  picks: RacePick[];
  /** Filters, hour correction; null while not loaded (only the picks are shown then). */
  settings: RaceSettings | null;
  now: number;
}

const catRank = (c: Cat): number => {
  const i = CAT_ORDER.indexOf(c);
  return i < 0 ? CAT_ORDER.length : i;
};

/** The next start of a race after `now` (special event: its first day), or null. */
export function nextStart(r: Race, now: number): number | null {
  if (r.cat === "special") return (r.to as number) > now ? (r.from as number) : null;
  return r.starts.find((s) => s > now) ?? null;
}

/** Within the widget's horizon for its category. */
function inHorizon(r: Race, start: number, now: number): boolean {
  if (r.cat === "daily") return true;
  if (r.cat === "special") return start - now < SPECIAL_HORIZON_MS;
  return start - now < TIMED_HORIZON_MS;
}

function fromPick(p: RacePick, race: Race | null): Slide {
  const special = p.cat === "special";
  return {
    key: p.race_id,
    cat: p.cat,
    level: p.level,
    name: race?.name || p.name,
    track: race?.track || p.track,
    duration: race?.duration || p.duration,
    image: safeImage(race?.image) ?? safeImage(p.image),
    start: special && race?.from != null ? race.from : p.start_ms,
    end: special && race?.to != null ? race.to : null,
    race,
    pick: p,
    pickId: p.id,
    picked: true,
  };
}

function fromRace(r: Race, start: number, picked: Set<string>): Slide {
  const special = r.cat === "special";
  const id = pickId(r.id, special ? (r.remindAt as number) : start);
  return {
    key: r.id,
    cat: r.cat,
    level: r.level,
    name: r.name,
    track: r.track,
    duration: r.duration,
    image: safeImage(r.image),
    start,
    end: special ? (r.to as number) : null,
    race: r,
    pick: null,
    pickId: id,
    picked: picked.has(id),
  };
}

/**
 * The slides, in order: the driver's upcoming picks by start, then the next starts of the
 * schedule — daily races first, then weekly, special events, championships, each by its next
 * start — through the driver's category and level filters. One slide per race, at most
 * {@link MAX_SLIDES}. A start that has passed is gone: a pick leaves, a race moves on to its
 * next start or leaves.
 */
export function buildSlides({ races, picks, settings, now }: SlideInput): Slide[] {
  const out: Slide[] = [];
  const seen = new Set<string>();
  const view = viewRaces(races, settings?.hour_fix ?? 0, now);
  const byId = new Map(view.map((r) => [r.id, r]));

  const mine = picks
    .filter((p) => {
      if (p.cat !== "special") return p.start_ms > now;
      const race = byId.get(p.race_id);
      return race ? (race.to as number) > now : p.start_ms > now;
    })
    .sort((a, b) => a.start_ms - b.start_ms || a.id.localeCompare(b.id));
  for (const p of mine) {
    if (out.length >= MAX_SLIDES) return out;
    if (seen.has(p.race_id)) continue;
    seen.add(p.race_id);
    out.push(fromPick(p, byId.get(p.race_id) ?? null));
  }

  if (!settings) return out;
  const picked = new Set(picks.map((p) => p.id));
  const next = shownRaces(view, settings)
    .map((r) => ({ r, start: nextStart(r, now) }))
    .filter(
      (x): x is { r: Race; start: number } => x.start !== null && inHorizon(x.r, x.start, now),
    )
    .sort(
      (a, b) =>
        catRank(a.r.cat) - catRank(b.r.cat) || a.start - b.start || a.r.id.localeCompare(b.r.id),
    );
  for (const { r, start } of next) {
    if (out.length >= MAX_SLIDES) break;
    if (seen.has(r.id)) continue;
    seen.add(r.id);
    out.push(fromRace(r, start, picked));
  }
  return out;
}

/**
 * The live countdown: `dans 42 s` under a minute, then `dans 12 min`, `dans 1 h 05`, `dans 3 j`;
 * a special event that has begun is `en cours`.
 */
export function countdownLabel(start: number, end: number | null, now: number, lang: Lang): string {
  const left = start - now;
  if (left <= 0) {
    return translate(lang, end !== null && end > now ? "races.rel.started" : "races.rel.now");
  }
  if (left <= MIN) return translate(lang, "races.rel.sec", { n: Math.ceil(left / SEC) });
  return relLabel(start, now, lang);
}

/** Starting within 5 minutes: the countdown turns urgent. */
export const isSoon = (s: Pick<Slide, "start" | "cat">, now: number): boolean =>
  s.cat !== "special" && s.start - now <= 5 * MIN;

/** The picture to show, or null for the drawn stand-in (none, unsafe, or failed to load). */
export function slideArt(image: string | null | undefined, failed: boolean): string | null {
  if (failed) return null;
  return safeImage(image);
}

/** The word written large on the drawn stand-in: the track without its parentheses. */
export const trackMark = (track: string): string => track.replace(/\(.*?\)/g, "").trim();

export interface AutoplayState {
  count: number;
  hovered: boolean;
  focused: boolean;
  hidden: boolean;
  reducedMotion: boolean;
}

/**
 * The carousel turns by itself only when there is something to turn to and nothing holds it:
 * no pointer over it, no keyboard focus in it, the tab visible, and no request for less motion.
 */
export const autoplays = (s: AutoplayState): boolean =>
  s.count > 1 && !s.hovered && !s.focused && !s.hidden && !s.reducedMotion;

/** `i` brought back into `0 … n-1` (wraps both ways). */
export const wrap = (i: number, n: number): number => (n > 0 ? ((i % n) + n) % n : 0);

/**
 * Which slide to show after the list changed: the same race if it is still there, otherwise the
 * one that took its place (the same position, or the last one).
 */
export function followIndex(currentKey: string | null, lastIndex: number, keys: string[]): number {
  if (keys.length === 0) return 0;
  const i = currentKey === null ? -1 : keys.indexOf(currentKey);
  if (i >= 0) return i;
  return Math.min(Math.max(lastIndex, 0), keys.length - 1);
}

/** A horizontal swipe: `1` next, `-1` previous, `0` none (too short, or mostly vertical). */
export function swipeStep(dx: number, dy: number, threshold = 40): -1 | 0 | 1 {
  if (Math.abs(dx) < threshold || Math.abs(dx) <= Math.abs(dy) * 1.2) return 0;
  return dx < 0 ? 1 : -1;
}
