import type { RaceReminder } from "@corde/protocol";
import type {
  RacePick,
  RaceSettings,
  RaceSources,
  Ratings,
  ReqKey,
  SourceId,
} from "../net/races-api";
import {
  type Cat,
  combine,
  type Level,
  parseLmuTools,
  parseRaceControl,
  type Race,
  type Schedule,
} from "./parse";

/**
 * The model of the « Courses » page around the parser: categories, ratings and access
 * requirements, picks, reminders. Same rules as the owner's « LMU Courses » script.
 */

export const CAT_ORDER: Cat[] = ["daily", "weekly", "special", "champ"];
export const LEVELS: Level[] = ["beginner", "intermediate", "advanced"];
export const LEAD_CHOICES = [60, 30, 15, 10, 5, 2, 0] as const;
export const RATINGS = [
  "",
  "B0",
  "B1",
  "B2",
  "B3",
  "S0",
  "S1",
  "S2",
  "S3",
  "G0",
  "G1",
  "G2",
  "G3",
  "P0",
  "P1",
  "P2",
  "P3",
] as const;
export const REQ_KEYS: ReqKey[] = [
  "beginner",
  "intermediate",
  "advanced",
  "weekly",
  "special",
  "champ",
];

const MIN = 60e3;
const HOUR = 3600e3;
const DAY = 864e5;

/** Which access requirement applies: the level of a daily race, the category otherwise. */
export function reqKey(r: { cat: Cat; level: Level | null }): ReqKey {
  return (r.cat === "daily" ? (r.level ?? "beginner") : r.cat) as ReqKey;
}

export const rank = (v: string): number => (v ? (RATINGS as readonly string[]).indexOf(v) : 0);

/** `["SR S2", "DR B2"]`: what the driver lacks for this requirement (nothing when unknown). */
export function missing(req: Ratings | undefined, profile: Ratings): string[] {
  const out: string[] = [];
  if (!req) return out;
  if (profile.sr && req.sr && rank(profile.sr) < rank(req.sr)) out.push(`SR ${req.sr}`);
  if (profile.dr && req.dr && rank(profile.dr) < rank(req.dr)) out.push(`DR ${req.dr}`);
  return out;
}

export const pickId = (raceId: string, start: number): string => `${raceId}@${start}`;

/** 18:00 local time, the day before `dayMs`. */
export function eveOf(dayMs: number): number {
  const d = new Date(dayMs - DAY);
  d.setHours(18, 0, 0, 0);
  return d.getTime();
}

/** The pick of one start of a race; a special event is picked on its reminder of the day. */
export function pickOf(race: Race, start: number): RacePick {
  const special = race.cat === "special";
  const at = special ? (race.remindAt as number) : start;
  return {
    id: pickId(race.id, at),
    race_id: race.id,
    cat: race.cat,
    src: race.src,
    name: race.name.slice(0, 120),
    level: race.level,
    track: (race.track || "?").slice(0, 120),
    duration: race.duration.slice(0, 40),
    image: safeImage(race.image),
    start_ms: at,
    eve_ms: special ? eveOf(at) : null,
  };
}

/** Only `https://` pictures are shown (and sent to the server). */
export function safeImage(url: string | null | undefined): string | null {
  if (!url?.startsWith("https://") || url.length > 500) return null;
  return /[\s"'<>\\]/.test(url) ? null : url;
}

/** Picks still worth showing (started less than 5 min ago), by start. */
export function upcomingPicks(picks: RacePick[], now: number): RacePick[] {
  return picks.filter((p) => p.start_ms > now - 5 * MIN).sort((a, b) => a.start_ms - b.start_ms);
}

/** The next race (not a special-event reminder) starting within `withinMs`. */
export function nextRace(picks: RacePick[], now: number, withinMs = 3 * HOUR): RacePick | null {
  return (
    upcomingPicks(picks, now).find(
      (p) => p.cat !== "special" && p.start_ms > now && p.start_ms - now <= withinMs,
    ) ?? null
  );
}

/** `4:07` / `1:02:07` — time left, never negative. */
export function countdown(ms: number): string {
  const s = Math.max(0, Math.floor(ms / 1000));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const x = s % 60;
  const pad = (n: number) => String(n).padStart(2, "0");
  return h ? `${h}:${pad(m)}:${pad(x)}` : `${m}:${pad(x)}`;
}

/** Minutes left, rounded up (`1` for anything under a minute, `0` once started). */
export function minutesLeft(start: number, now: number): number {
  return start <= now ? 0 : Math.ceil((start - now) / MIN);
}

/** A reminder that deserves the insistent sound and the red banner (≤ 5 min, the start, the day). */
export function isUrgent(r: Pick<RaceReminder, "kind" | "lead_min">): boolean {
  if (r.kind === "eve") return false;
  if (r.kind === "day") return true;
  return (r.lead_min ?? 0) <= 5;
}

/** `UTC−4`, `UTC+5:30`. */
export function utcLabel(offsetHours: number): string {
  const sign = offsetHours < 0 ? "−" : "+";
  const abs = Math.abs(offsetHours);
  const h = Math.floor(abs);
  const m = Math.round((abs - h) * 60);
  return `UTC${sign}${h}${m ? `:${String(m).padStart(2, "0")}` : ""}`;
}

export interface SourceView {
  id: SourceId;
  url: string;
  state: "disabled" | "ok" | "error";
  fetchedAt: number | null;
  error: string | null;
  /** A page was read but no race could be recognised in it. */
  unreadable: boolean;
  /** Races read from it. */
  count: number;
}

export interface ScheduleView {
  schedule: Schedule | null;
  sources: SourceView[];
}

/**
 * The schedule from the pages the server read: each one is parsed at the time it was read (the
 * racecontrol.gg time zone is guessed from « now »), then both are put together.
 */
export function buildSchedule(data: RaceSources): ScheduleView {
  const sources: SourceView[] = [];
  let rc: ReturnType<typeof parseRaceControl> | null = null;
  let lt: ReturnType<typeof parseLmuTools> | null = null;
  for (const s of data.sources) {
    const view: SourceView = {
      id: s.id,
      url: s.url,
      state: s.state,
      fetchedAt: s.fetched_at_ms,
      error: s.error,
      unreadable: false,
      count: 0,
    };
    if (s.html && s.fetched_at_ms) {
      try {
        if (s.id === "racecontrol") {
          rc = parseRaceControl(s.html, s.fetched_at_ms);
          view.count = rc.races.length;
        } else {
          lt = parseLmuTools(s.html, s.fetched_at_ms);
          view.count = lt.weekly.length + lt.daily.length + lt.champ.length + lt.special.length;
        }
      } catch {
        // RaceParseError, or anything a changed page may throw: the page is unreadable.
        view.unreadable = true;
      }
    }
    sources.push(view);
  }
  return { schedule: rc || lt ? combine(rc, lt, null) : null, sources };
}

/** The races a settings object lets through (categories and daily levels shown). */
export function shownRaces(races: Race[], settings: RaceSettings): Race[] {
  return races.filter(
    (r) =>
      settings.cats.includes(r.cat) &&
      (r.cat !== "daily" || !r.level || settings.levels.includes(r.level)),
  );
}
