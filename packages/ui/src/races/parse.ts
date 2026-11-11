/**
 * Reading the online race schedule of Le Mans Ultimate out of two public web pages:
 *
 * - `https://racecontrol.gg/` — the official site: the daily races (three levels, several starts
 *   an hour), section « Upcoming Daily Races »;
 * - `https://lmu.tools/schedule` — a community site: weekly races, special events and the
 *   RaceControl Pro championships (and the daily races again, used to complete them).
 *
 * « Porté du script LMU Courses v2.2 du propriétaire du projet » (Scriptable, iPhone): ported
 * from the project owner's script, where these functions
 * were tuned on the real pages. The port is deliberately literal — same regular expressions, same
 * time-zone heuristics, same object shapes — and is checked against the original script on the
 * same sample pages (see docs/STATUS.md, « Courses »). Do not "improve" the logic here without
 * real pages to test it on: when a site changes, the server keeps the page that could not be read
 * in `<data-dir>/diagnostics/races/<source>.html`, which is what an adaptation should start from.
 *
 * Pure functions: text in, objects out. The HTML is never put in the DOM.
 */

export type Level = "beginner" | "intermediate" | "advanced";
export type Cat = "daily" | "weekly" | "special" | "champ";

export const LEVEL_WORDS: Record<Level, string[]> = {
  beginner: ["beginner", "debutant", "rookie", "novice"],
  intermediate: ["intermediate", "intermediaire", "inter"],
  advanced: ["advanced", "avance", "expert"],
};
export const LEVEL_ORDER: Level[] = ["beginner", "intermediate", "advanced"];

export interface RaceFormat {
  p?: number;
  q?: number;
  r?: number;
}

/**
 * One race of the schedule. The fields present depend on where it comes from, exactly as in the
 * original script: daily races of racecontrol.gg, online races of lmu.tools (`fmt`, `setup`,
 * multipliers) and special events (`from`, `to`, `remindAt`, no `starts`).
 */
export interface Race {
  id: string;
  cat: Cat;
  src: "rc" | "lt";
  name: string;
  level: Level | null;
  track: string;
  duration: string;
  image: string | null;
  classes: string[];
  /** Start times (ms since the epoch), sorted. Empty for a special event. */
  starts: number[];
  fmt?: RaceFormat;
  setup?: "fixed" | "open" | null;
  srMult?: number | null;
  drMult?: number | null;
  hours?: number | null;
  /** Special events: first day 00:00 and last day 23:59 (local time), reminder day 10:00. */
  from?: number;
  to?: number;
  remindAt?: number;
}

export interface RaceControlSchedule {
  fetchedAt: number;
  /** `horodatage`: absolute times found in the page; `auto`: time zone guessed (`offset`). */
  method: "horodatage" | "auto";
  offset: number | null;
  races: Race[];
}

export interface LmuToolsSchedule {
  fetchedAt: number;
  offset: number;
  weekly: Race[];
  champ: Race[];
  daily: Race[];
  special: Race[];
}

export interface Schedule {
  fetchedAt: number;
  method: "horodatage" | "auto" | null | undefined;
  offset: number | null | undefined;
  extrasAt: number;
  races: Race[];
}

/** A page from which no race could be read (the site changed, or answered something else). */
export class RaceParseError extends Error {
  constructor(readonly source: "racecontrol" | "lmutools") {
    super(
      source === "racecontrol"
        ? "no race recognised on racecontrol.gg"
        : "unreadable schedule on lmu.tools",
    );
    this.name = "RaceParseError";
  }
}

export const norm = (s: unknown): string =>
  String(s || "")
    .toLowerCase()
    .normalize("NFD")
    .replace(/[̀-ͯ]/g, "");

const MIN = 60e3;
const HOUR = 3600e3;
const DAY = 864e5;

/** `55` → `55 min`, `73` → `1 h 13`, `120` → `2 h`; empty for 0 / missing. */
export function fmtMin(m: number | undefined | null): string {
  if (!m) return "";
  if (m < 60) return `${m} min`;
  const h = Math.floor(m / 60);
  const r = m % 60;
  return r ? `${h} h ${String(r).padStart(2, "0")}` : `${h} h`;
}

// ------------------------------------------------------------------ HTML reading tools

const IMG_URL_RE = /https?:\/\/[^"'\s)]+?\.(?:webp|jpe?g|png)(?:\?[^"'\s)]*)?/i;

export function decode(s: string): string {
  return s
    .replace(/&nbsp;|&#160;/g, " ")
    .replace(/&quot;/g, '"')
    .replace(/&#39;|&apos;/g, "'")
    .replace(/&lt;/g, "<")
    .replace(/&gt;/g, ">")
    .replace(/&#(\d+);/g, (_, n: string) => String.fromCharCode(+n))
    .replace(/&#x([0-9a-f]+);/gi, (_, n: string) => String.fromCharCode(Number.parseInt(n, 16)))
    .replace(/&amp;/g, "&");
}

/**
 * The text of a page, one line per element.
 * mode `img`: race pictures become `§IMG url`; mode `href`: links become `§HREF url`.
 */
export function htmlLines(html: string, mode: "img" | "href"): string[] {
  const stripped = html
    .replace(/<!--[\s\S]*?-->/g, "")
    .replace(/<script[\s\S]*?<\/script>/gi, "")
    .replace(/<style[\s\S]*?<\/style>/gi, "")
    .replace(/<[^>]+>/g, (tag) => {
      if (mode === "href") {
        const h = tag.match(/^<a\b[^>]*\bhref\s*=\s*["']([^"']+)["']/i);
        return h ? `\n§HREF ${h[1]}\n` : "\n";
      }
      const m = tag.match(IMG_URL_RE);
      return m && !/\/assets\/|logo|avatar|icon/i.test(m[0]) ? `\n§IMG ${m[0]}\n` : "\n";
    });
  return decode(stripped)
    .split("\n")
    .map((s) => s.replace(/\s+/g, " ").trim())
    .filter(Boolean);
}

export const slug = (s: string): string =>
  norm(s)
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-|-$/g, "");

/** A short key that is the same for « Circuit de la Sarthe » and « Le Mans (WEC) »-like names. */
export function trackKey(t: string): string {
  return norm(t)
    .replace(/\(.*?\)/g, " ")
    .replace(
      /\b(wec|elms|rc|endurance|international|circuit|speedway|motorsport|motor|park|raceway|autodromo|nazionale|de|la|du|of|the|gp|grand|prix|\d{4})\b/g,
      " ",
    )
    .replace(/[^a-z]/g, "")
    .slice(0, 8);
}

/** Car classes named in the lines (`HYPERCAR`, `LMP2`, `LMGT3`…), in order of appearance. */
export function classTags(lines: string[] | null | undefined): string[] {
  const out: string[] = [];
  for (const l of lines || []) {
    const re = /hyp\w*|lmgt3|gt3|lmp2|lmp3|lmgte|gte|lmdh/gi;
    let m: RegExpExecArray | null = re.exec(l);
    while (m) {
      const n = m[0].toLowerCase();
      const tag =
        n.startsWith("hyp") || n === "lmdh"
          ? "HYPERCAR"
          : n.includes("gt3")
            ? "LMGT3"
            : n.includes("gte")
              ? "LMGTE"
              : n.toUpperCase();
      if (!out.includes(tag)) out.push(tag);
      m = re.exec(l);
    }
  }
  return out;
}

// ------------------------------------------------------------------ racecontrol.gg (daily)

const MONTH_PREFIX: [string, number][] = [
  ["juil", 6],
  ["juin", 5],
  ["jan", 0],
  ["fev", 1],
  ["feb", 1],
  ["mar", 2],
  ["apr", 3],
  ["avr", 3],
  ["may", 4],
  ["mai", 4],
  ["jun", 5],
  ["jul", 6],
  ["aug", 7],
  ["aou", 7],
  ["sep", 8],
  ["oct", 9],
  ["nov", 10],
  ["dec", 11],
  ["dez", 11],
];

export function monthIndex(word: string): number | null {
  const n = norm(word);
  for (const [p, i] of MONTH_PREFIX) if (n.startsWith(p)) return i;
  return null;
}

export function levelKey(line: string): Level | null {
  const n = norm(line).trim();
  for (const k of LEVEL_ORDER) if (LEVEL_WORDS[k].includes(n)) return k;
  return null;
}

/** « 9 Nov at 7:10pm » (English) or « 9 novembre à 14h30 » (French). */
const TIME_RE =
  /(\d{1,2})\s+([A-Za-zÀ-ÿ]{3,9})\.?\s+(?:at\s+|à\s+|a\s+|um\s+)?(\d{1,2})[:h](\d{2})\s*([ap]\.?m\.?)?/gi;

interface CardTime {
  d: number;
  mon: number;
  h: number;
  mi: number;
  ap: string;
}

interface Card {
  level: Level;
  name: string;
  track: string;
  duration: string;
  times: CardTime[];
  image: string | null;
  naive?: number[];
  abs?: number[];
}

interface DailySection {
  chunk: string;
  game: "lmu" | "rf2" | "?";
}

export function dailySections(html: string): DailySection[] {
  const idx: number[] = [];
  const re = /Upcoming Daily Races|Courses quotidiennes à venir/gi;
  let m: RegExpExecArray | null = re.exec(html);
  while (m) {
    idx.push(m.index);
    m = re.exec(html);
  }
  if (!idx.length) return [{ chunk: html, game: "?" }];
  return idx.map((start, k) => {
    let end = k + 1 < idx.length ? (idx[k + 1] as number) : html.length;
    for (const stop of [
      "Featured Community Events",
      "Latest Event Listings",
      "TRENDING LIVERIES",
    ]) {
      const j = html.indexOf(stop, start);
      if (j > start && j < end) end = j;
    }
    const chunk = html.slice(start, end);
    const head = chunk.slice(0, 3000).toLowerCase();
    const game = /lmu_logo|le mans ultimate/.test(head)
      ? "lmu"
      : /rc_logo|rfactor|rf2_logo/.test(head)
        ? "rf2"
        : "?";
    return { chunk, game };
  });
}

/** Each card: level → picture → name → Duration → Track → start times. */
function parseCards(chunk: string): Card[] {
  const cards: Card[] = [];
  let cur: Card | null = null;
  let expect: "name" | "duration" | "track" | null = null;
  let pendingImg: string | null = null;
  for (const L of htmlLines(chunk, "img")) {
    if (L.startsWith("§IMG ")) {
      const u = L.slice(5);
      if (cur && !cur.image && !cur.times.length) cur.image = u;
      else pendingImg = u;
      continue;
    }
    const lk = levelKey(L);
    if (lk) {
      cur = { level: lk, name: "", track: "", duration: "", times: [], image: pendingImg };
      pendingImg = null;
      cards.push(cur);
      expect = "name";
      continue;
    }
    if (!cur) continue;
    const n = norm(L);
    if (/^(duration|duree|dauer)\s*:?$/.test(n)) {
      expect = "duration";
      continue;
    }
    if (/^(track|piste|circuit|strecke)\s*:?$/.test(n)) {
      expect = "track";
      continue;
    }
    if (/^[*\-–—_\s]+$/.test(L)) continue;

    let found = false;
    TIME_RE.lastIndex = 0;
    let m: RegExpExecArray | null = TIME_RE.exec(L);
    while (m) {
      const mon = monthIndex(m[2] as string);
      if (mon != null) {
        found = true;
        cur.times.push({
          d: +(m[1] as string),
          mon,
          h: +(m[3] as string),
          mi: +(m[4] as string),
          ap: (m[5] || "").replace(/\./g, "").toLowerCase(),
        });
      }
      m = TIME_RE.exec(L);
    }
    if (found) {
      expect = null;
      continue;
    }

    if (expect === "name") cur.name = L;
    else if (expect === "duration") cur.duration = L;
    else if (expect === "track") cur.track = L;
    expect = null;
  }
  return cards.filter((c) => c.name && c.times.length);
}

function naiveMs(t: CardTime, nowMs: number): number {
  let h = t.h;
  if (t.ap === "pm" && h < 12) h += 12;
  if (t.ap === "am" && h === 12) h = 0;
  const y = new Date(nowMs).getUTCFullYear();
  let ms = Date.UTC(y, t.mon, t.d, h, t.mi);
  if (ms - nowMs > 180 * DAY) ms = Date.UTC(y - 1, t.mon, t.d, h, t.mi);
  if (nowMs - ms > 180 * DAY) ms = Date.UTC(y + 1, t.mon, t.d, h, t.mi);
  return ms;
}

/**
 * The site does not say its time zone. It lists upcoming starts only: keep the offset that puts
 * the very first start just after « now ».
 */
function detectOffset(cards: Card[], nowMs: number): number {
  const earliest = Math.min(...cards.map((c) => Math.min(...(c.naive as number[]))));
  for (let off = 14; off >= -12; off--) {
    if (earliest - off * HOUR - nowMs >= -10 * MIN) return off;
  }
  return -new Date(nowMs).getTimezoneOffset() / 60;
}

export function explicitStamps(chunk: string): number[] {
  const out: number[] = [];
  const iso =
    /(?:datetime|data-[\w-]+)="(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}(?::\d{2}(?:\.\d+)?)?(?:Z|[+-]\d{2}:?\d{2}))"/gi;
  let m: RegExpExecArray | null = iso.exec(chunk);
  while (m) {
    const t = Date.parse(m[1] as string);
    if (!Number.isNaN(t)) out.push(t);
    m = iso.exec(chunk);
  }
  if (out.length) return out;
  const unix = /data-[\w-]+="(1[6-9]\d{8}(?:\d{3})?)"/gi;
  m = unix.exec(chunk);
  while (m) {
    const n = +(m[1] as string);
    out.push(n < 1e12 ? n * 1000 : n);
    m = unix.exec(chunk);
  }
  return out;
}

/** `55 minutes` → `55 min`, `1h 13 minutes` → `1 h 13`. */
export function frDuration(s: string): string {
  if (!s) return "";
  return s
    .replace(
      /(\d+)\s*h(?:ours?)?\s*(\d+)\s*min(?:ute)?s?/i,
      (_, h: string, m: string) => `${h} h ${m.padStart(2, "0")}`,
    )
    .replace(/(\d+)\s*min(?:ute)?s?/i, "$1 min")
    .replace(/(\d+)\s*hours?/i, "$1 h");
}

/** The daily races of racecontrol.gg. Throws [`RaceParseError`] when no race is recognised. */
export function parseRaceControl(html: string, nowMs: number): RaceControlSchedule {
  const secs = dailySections(html);
  let chosen = secs.filter((s) => s.game === "lmu");
  if (!chosen.length) chosen = secs.filter((s) => s.game !== "rf2");
  if (!chosen.length) chosen = [secs[0] as DailySection];

  let cards: Card[] = [];
  let stamps: number[] = [];
  for (const s of chosen) {
    cards = cards.concat(parseCards(s.chunk));
    stamps = stamps.concat(explicitStamps(s.chunk));
  }
  if (!cards.length) throw new RaceParseError("racecontrol");

  const flat: CardTime[] = [];
  for (const c of cards) for (const t of c.times) flat.push(t);
  let method: "horodatage" | "auto";
  let offset: number | null = null;
  if (
    stamps.length === flat.length &&
    stamps.every((ms, i) => new Date(ms).getUTCMinutes() === (flat[i] as CardTime).mi)
  ) {
    method = "horodatage";
    let k = 0;
    for (const c of cards) c.abs = c.times.map(() => stamps[k++] as number);
  } else {
    for (const c of cards) c.naive = c.times.map((t) => naiveMs(t, nowMs));
    const off = detectOffset(cards, nowMs);
    offset = off;
    method = "auto";
    for (const c of cards) c.abs = (c.naive as number[]).map((n) => n - off * HOUR);
  }

  const byId: Record<string, Race> = {};
  const races: Race[] = [];
  for (const c of cards) {
    const id = `${slug(c.name)}-${c.level}`;
    const known = byId[id];
    if (known) {
      known.starts = Array.from(new Set(known.starts.concat(c.abs as number[]))).sort(
        (a, b) => a - b,
      );
      if (!known.image) known.image = c.image;
      continue;
    }
    const r: Race = {
      id,
      cat: "daily",
      src: "rc",
      name: c.name,
      level: c.level,
      track: c.track || "?",
      duration: frDuration(c.duration),
      image: c.image || null,
      classes: [],
      starts: (c.abs as number[]).slice().sort((a, b) => a - b),
    };
    byId[id] = r;
    races.push(r);
  }
  return { fetchedAt: nowMs, method, offset, races };
}

// ------------------------------------------------------------------ lmu.tools (weekly, special, championships)

const WD: Record<string, number> = { sun: 0, mon: 1, tue: 2, wed: 3, thu: 4, fri: 5, sat: 6 };

interface Delta {
  ms: number;
  unit: string;
}

export function parseDelta(s: string): Delta {
  let ms = 0;
  let unit = "m";
  const re = /(\d+)\s*([dhm])/gi;
  let m: RegExpExecArray | null = re.exec(s);
  while (m) {
    const u = (m[2] as string).toLowerCase();
    ms += +(m[1] as string) * (u === "d" ? DAY : u === "h" ? HOUR : MIN);
    unit = u;
    m = re.exec(s);
  }
  return { ms, unit };
}

interface WeekTime {
  wd: number;
  h: number;
  mi: number;
  delta: Delta | null;
}

interface OnlineEntry {
  name: string;
  track: string;
  classes: string[];
  fmt: RaceFormat;
  srMult: number | null;
  drMult: number | null;
  setup: "fixed" | "open" | null;
  times: WeekTime[];
}

export function parseOnlineEntry(buf: string[]): OnlineEntry | null {
  const lines = buf.filter((l) => !l.startsWith("§HREF "));
  const iP = lines.findIndex((l) => /^(practice|practise|qualifying|race)$/i.test(l));
  if (iP < 2) return null;
  const fmt: RaceFormat = {};
  for (let i = iP; i < lines.length - 1; i++) {
    const k = norm(lines[i]);
    const v = (lines[i + 1] as string).match(/^(\d+)\s*min/i);
    if (!v) continue;
    if (k === "practice" || k === "practise") fmt.p = +(v[1] as string);
    else if (k === "qualifying") fmt.q = +(v[1] as string);
    else if (k === "race") fmt.r = +(v[1] as string);
  }
  const rest = lines.slice(iP).join(" · ");
  const sr = rest.match(/SR\s*([\d.]+)\s*x/i);
  const dr = rest.match(/DR\s*([\d.]+)\s*x/i);
  const times: WeekTime[] = [];
  const re =
    /\b(Mon|Tue|Wed|Thu|Fri|Sat|Sun)[a-z]*\.?\s+(\d{1,2}):(\d{2})(?:[\s·]+in\s+((?:\d+\s*[dhm]\s*)+))?/gi;
  let m: RegExpExecArray | null = re.exec(rest);
  while (m) {
    times.push({
      wd: WD[(m[1] as string).slice(0, 3).toLowerCase()] as number,
      h: +(m[2] as string),
      mi: +(m[3] as string),
      delta: m[4] ? parseDelta(m[4]) : null,
    });
    m = re.exec(rest);
  }
  if (!times.length) return null;
  return {
    name: lines[0] as string,
    track: lines[1] as string,
    classes: classTags(lines.slice(2, iP)),
    fmt,
    srMult: sr ? +(sr[1] as string) : null,
    drMult: dr ? +(dr[1] as string) : null,
    setup: /fixed setup/i.test(rest) ? "fixed" : /open setup/i.test(rest) ? "open" : null,
    times,
  };
}

function urlParam(u: string | undefined, k: string): string | null {
  const m = u?.match(new RegExp(`[?&]${k}=([^&#]*)`));
  return m ? decodeURIComponent((m[1] as string).replace(/\+/g, " ")) : null;
}

interface SpecialEntry {
  title: string | null;
  track: string;
  hours: number | null;
  from: string;
  to: string;
  classes: string[];
}

export function parseSpecialEntry(buf: string[]): SpecialEntry | null {
  const hrefs = buf.filter((l) => l.startsWith("§HREF ")).map((l) => l.slice(6));
  const lines = buf.filter(
    (l) =>
      !l.startsWith("§HREF ") && !/^(next up|plan this race|create availability poll)/i.test(l),
  );
  const poll = hrefs.find((h) => /poll\/new/.test(h));
  const stint = hrefs.find((h) => /stint-planner/.test(h));
  const from = urlParam(poll, "from");
  const to = urlParam(poll, "to") || from;
  if (!from || !/^\d{4}-\d{2}-\d{2}$/.test(from)) return null;
  const minutes = +(urlParam(stint, "minutes") || 0);
  const hoursLine = lines.find((l) => /^\d+(?:[.,]\d+)?\s*h(?:rs?|ours?|eures?)?$/i.test(l));
  const hours = minutes
    ? minutes / 60
    : hoursLine
      ? Number.parseFloat(hoursLine.replace(",", "."))
      : null;
  const dateIdx = lines.findIndex((l) => /\b(mon|tue|wed|thu|fri|sat|sun)\w*\b.*\d/i.test(l));
  const before = lines.filter((l, i) => l !== hoursLine && (dateIdx < 0 || i < dateIdx));
  let track = before[before.length - 1] || "TBA";
  if (/track tba|^tba$/i.test(track)) track = "Circuit à annoncer";
  const classes = classTags(dateIdx >= 0 ? lines.slice(dateIdx + 1) : []);
  return { title: urlParam(poll, "title"), track, hours, from, to: to as string, classes };
}

/** The site shows « Mon 15:00 in 1h 5m »: its time zone is found again from the « in … ». */
function detectToolsOffset(entries: OnlineEntry[], nowMs: number): number {
  const samples: WeekTime[] = [];
  for (const e of entries)
    for (const t of e.times) if (t.delta && t.delta.unit === "m") samples.push(t);
  if (!samples.length) return 0;
  let best = 0;
  let bestScore = -1;
  for (let off = -12; off <= 14; off++) {
    let score = 0;
    for (const t of samples) {
      const w = new Date(nowMs + (t.delta as Delta).ms + off * HOUR);
      const diff = Math.abs(w.getUTCHours() * 60 + w.getUTCMinutes() - (t.h * 60 + t.mi));
      if (w.getUTCDay() === t.wd && Math.min(diff, 1440 - diff) <= 3) score++;
    }
    if (score > bestScore || (score === bestScore && off === 0)) {
      best = off;
      bestScore = score;
    }
  }
  return bestScore > 0 ? best : 0;
}

function nextOccurrence(
  ref: number,
  wd: number,
  h: number,
  mi: number,
  off: number,
): number | null {
  const w = new Date(ref + off * HOUR);
  for (let k = 0; k <= 8; k++) {
    const c = Date.UTC(w.getUTCFullYear(), w.getUTCMonth(), w.getUTCDate() + k, h, mi) - off * HOUR;
    if (c >= ref && new Date(c + off * HOUR).getUTCDay() === wd) return c;
  }
  return null;
}

function resolveTimes(times: WeekTime[], nowMs: number, off: number): number[] {
  const out: number[] = [];
  let prev: number | null = null;
  for (const t of times) {
    let abs: number | null = null;
    if (t.delta) {
      const target = nowMs + t.delta.ms;
      const w = new Date(target + off * HOUR);
      for (let k = -2; k <= 2; k++) {
        const c =
          Date.UTC(w.getUTCFullYear(), w.getUTCMonth(), w.getUTCDate() + k, t.h, t.mi) - off * HOUR;
        if (new Date(c + off * HOUR).getUTCDay() !== t.wd) continue;
        if (abs == null || Math.abs(c - target) < Math.abs(abs - target)) abs = c;
      }
    } else {
      abs = nextOccurrence(prev != null ? prev + MIN : nowMs - 30 * MIN, t.wd, t.h, t.mi, off);
    }
    if (abs != null) {
      out.push(abs);
      prev = abs;
    }
  }
  return out.sort((a, b) => a - b);
}

/** `2026-11-14` at `h` o'clock, local time of this device. */
function localDay(iso: string, h: number): number {
  const [y, m, d] = iso.split("-").map(Number) as [number, number, number];
  return new Date(y, m - 1, d, h || 0, 0, 0).getTime();
}

/** Weekly races, special events and championships of lmu.tools. Throws [`RaceParseError`]. */
export function parseLmuTools(html: string, nowMs: number): LmuToolsSchedule {
  type Sec = "special" | "weekly" | "daily" | "champ";
  const sections: {
    special: SpecialEntry[];
    weekly: OnlineEntry[];
    daily: OnlineEntry[];
    champ: OnlineEntry[];
  } = { special: [], weekly: [], daily: [], champ: [] };
  let sec: Sec | null = null;
  let buf: string[] = [];
  const flush = () => {
    if (sec && buf.length) {
      if (sec === "special") {
        const e = parseSpecialEntry(buf);
        if (e) sections.special.push(e);
      } else {
        const e = parseOnlineEntry(buf);
        if (e) sections[sec].push(e);
      }
    }
    buf = [];
  };
  for (const L of htmlLines(html, "href")) {
    const n = norm(L);
    if (/^special events$/.test(n)) {
      buf = [];
      sec = "special";
      continue;
    }
    if (/^weekly( races?)?$/.test(n)) {
      buf = [];
      sec = "weekly";
      continue;
    }
    if (/^daily( races?)?$/.test(n)) {
      buf = [];
      sec = "daily";
      continue;
    }
    if (/^championships?( races?)?$/.test(n)) {
      buf = [];
      sec = "champ";
      continue;
    }
    if (/^online races$/.test(n)) {
      buf = [];
      sec = null;
      continue;
    }
    if (!sec) continue;
    if (
      /^(january|february|march|april|may|june|july|august|september|october|november|december) \d{4}$/.test(
        n,
      )
    )
      continue;
    if (/^source:|^updated /.test(n)) {
      buf = [];
      continue;
    }
    if (sec === "special") {
      buf.push(L);
      if (L.startsWith("§HREF ") && /poll\/new/.test(L)) flush();
      continue;
    }
    if (/fuel-calculator/.test(L) && L.startsWith("§HREF ")) {
      flush();
      continue;
    }
    if (/^fuel calculator/.test(n)) {
      if (buf.length) flush();
      continue;
    }
    buf.push(L);
  }
  flush();

  const online = sections.weekly.concat(sections.daily, sections.champ);
  if (!online.length && !sections.special.length) throw new RaceParseError("lmutools");
  const off = detectToolsOffset(online, nowMs);
  const mk = (cat: Cat, e: OnlineEntry): Race => ({
    id: `${cat}-${slug(e.name)}`,
    cat,
    src: "lt",
    name: e.name,
    level: null,
    track: e.track,
    classes: e.classes,
    fmt: e.fmt,
    duration: fmtMin(e.fmt.r),
    setup: e.setup,
    srMult: e.srMult,
    drMult: e.drMult,
    image: null,
    starts: resolveTimes(e.times, nowMs, off),
  });
  const daily = sections.daily.map((e) => {
    const r = mk("daily", e);
    r.level =
      e.srMult == null
        ? "beginner"
        : e.srMult <= 1.05
          ? "beginner"
          : e.srMult <= 1.45
            ? "intermediate"
            : "advanced";
    r.id = `${slug(e.name)}-${r.level}`;
    return r;
  });
  const special = sections.special.map((e): Race => {
    const name = e.title || `${e.hours || ""}h ${e.track}`.trim();
    return {
      id: `special-${slug(`${name}-${e.from}`)}`,
      cat: "special",
      src: "lt",
      name,
      level: null,
      track: e.track,
      classes: e.classes,
      hours: e.hours,
      duration: e.hours ? fmtMin(Math.round(e.hours * 60)) : "",
      image: null,
      starts: [],
      from: localDay(e.from, 0),
      to: localDay(e.to, 23) + 59 * MIN,
      remindAt: localDay(e.from, 10),
    };
  });
  return {
    fetchedAt: nowMs,
    offset: off,
    weekly: sections.weekly.map((e) => mk("weekly", e)),
    champ: sections.champ.map((e) => mk("champ", e)),
    daily,
    special,
  };
}

// ------------------------------------------------------------------ putting the schedule together

const nameTokens = (s: string): string[] =>
  norm(s)
    .replace(/[^a-z0-9]+/g, " ")
    .split(" ")
    .filter((w) => w && !/^(the|de|la|le|winter|summer|spring|autumn|fall)$/.test(w));

/** Same race on both sites: most name words in common, and not on two different tracks. */
export function sameRace(
  a: Pick<Race, "name" | "track">,
  b: Pick<Race, "name" | "track">,
): boolean {
  const A = nameTokens(a.name);
  const B = nameTokens(b.name);
  if (!A.length || !B.length) return false;
  const common = A.filter((w) => B.includes(w)).length;
  const score = common / Math.min(A.length, B.length);
  const tk = trackKey(a.track) && trackKey(b.track) && trackKey(a.track) !== trackKey(b.track);
  return score >= 0.75 && !tk;
}

/**
 * The whole schedule: the daily races of racecontrol.gg completed by lmu.tools (classes, format,
 * multipliers), then lmu.tools' weekly races, special events and championships. `prev` is the
 * previous result, used when a source is missing.
 */
export function combine(
  rc: RaceControlSchedule | null,
  lt: LmuToolsSchedule | null,
  prev: Schedule | null,
): Schedule {
  const prevRaces = prev?.races || [];
  let daily: Race[] = rc
    ? rc.races
    : prevRaces.filter((r) => (r.cat || "daily") === "daily" && r.src !== "lt");
  if (!daily.length && lt) daily = lt.daily;
  if (lt) {
    daily = daily.map((r) => {
      if (r.src === "lt") return r;
      const m = lt.daily.find((x) => sameRace(r, x));
      return m
        ? Object.assign({}, r, {
            classes: m.classes,
            fmt: m.fmt,
            setup: m.setup,
            srMult: m.srMult,
            drMult: m.drMult,
          })
        : r;
    });
  }
  const extras = lt
    ? lt.weekly.concat(lt.special, lt.champ)
    : prevRaces.filter((r) => r.cat && r.cat !== "daily");
  // Pictures: the other categories borrow the picture of a daily race on the same track.
  const pool: Record<string, string> = {};
  for (const r of prevRaces.concat(daily)) if (r.image) pool[trackKey(r.track)] = r.image;
  const races = daily
    .concat(extras)
    .map((r) => (r.image ? r : Object.assign({}, r, { image: pool[trackKey(r.track)] || null })));
  return {
    fetchedAt: rc ? rc.fetchedAt : prev?.fetchedAt || Date.now(),
    method: rc ? rc.method : prev ? prev.method : null,
    offset: rc ? rc.offset : prev ? prev.offset : null,
    extrasAt: lt ? lt.fetchedAt : prev?.extrasAt || 0,
    races,
  };
}

/**
 * What the page shows: the manual hour correction applied to the racecontrol.gg starts, the past
 * starts removed (a start stays one minute), the races without a start left out.
 */
export function viewRaces(races: Race[], hourFix: number, nowMs: number): Race[] {
  const fix = (hourFix || 0) * HOUR;
  return races
    .map((r) => {
      const cat = r.cat || "daily";
      if (cat === "special") return Object.assign({}, r, { cat });
      const f = r.src === "lt" ? 0 : fix;
      return Object.assign({}, r, {
        cat,
        starts: r.starts.map((t) => t + f).filter((t) => t > nowMs - MIN),
      });
    })
    .filter((r) => (r.cat === "special" ? (r.to as number) > nowMs : r.starts.length));
}

/** When a race comes next: its first start, or the first day of a special event. */
export const nextOf = (r: Race): number =>
  r.cat === "special" ? (r.from as number) : (r.starts[0] as number);
