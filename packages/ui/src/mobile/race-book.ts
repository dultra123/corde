import { t } from "../i18n";
import type { RacePick, RaceSettings, RaceSettingsPatch, Ratings } from "../net/races-api";
import { ApiError } from "../net/sessions-api";
import { KEYS, readJson, writeJson } from "./kv";
import type { PrefsPort } from "./ports";

/**
 * The driver's races **on the phone**: picks and settings of the « Courses » page, kept in the
 * native storage. Same rules as the server's store (`crates/corde-server/src/races/store.rs`):
 * validated picks, at most 50, dropped 3 h after their start; strict settings patches; a change
 * of the hour correction moves the racecontrol.gg daily picks with it. Whatever is unreadable
 * in storage falls back to the defaults (or is dropped), never the other way round.
 */
export const MAX_PICKS = 50;
export const KEEP_AFTER_START_MS = 3 * 3_600_000;
export const LEAD_CHOICES = [60, 30, 15, 10, 5, 2, 0] as const;
const LEVELS = ["beginner", "intermediate", "advanced"] as const;
const CATS = ["daily", "weekly", "special", "champ"] as const;
const RATINGS = [
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
const REQ_KEYS = ["beginner", "intermediate", "advanced", "weekly", "special", "champ"] as const;
const MAX_NAME = 120;
const MAX_ID = 120;
const MAX_DURATION = 40;
const MAX_IMAGE = 500;
const MIN_START_MS = 1_577_836_800_000;
const MAX_START_MS = 4_102_444_800_000;

export function defaultRaceSettings(): RaceSettings {
  const r = (sr: string): Ratings => ({ sr, dr: "" });
  return {
    leads: [15, 5],
    levels: [...LEVELS],
    cats: [...CATS],
    hour_fix: 0,
    profile: { sr: "", dr: "" },
    req: {
      beginner: r("B0"),
      intermediate: r("S0"),
      advanced: r("G0"),
      weekly: r("S2"),
      special: r("B2"),
      champ: r("B2"),
    },
  };
}

/** A deep copy (`structuredClone` is missing before iOS 15.4). */
const clone = <T>(v: T): T => JSON.parse(JSON.stringify(v)) as T;

class Invalid extends Error {}

function bad(msg: string): never {
  throw new Invalid(msg);
}

// biome-ignore lint/suspicious/noControlCharactersInRegex: control characters are what is refused
const CONTROL = /[\u0000-\u001f\u007f-\u009f]/;

function checkText(field: string, v: unknown, max: number, required: boolean): string {
  if (typeof v !== "string") bad(`\`${field}\``);
  if (required && !v.trim()) bad(`\`${field}\` est vide`);
  if ([...v].length > max) bad(`\`${field}\` dépasse ${max} caractères`);
  if (CONTROL.test(v)) bad(`\`${field}\` contient un caractère de contrôle`);
  return v;
}

export const validRaceId = (id: string): boolean =>
  id.length > 0 && id.length <= MAX_ID && /^[a-z0-9-]+$/.test(id);

export const validImage = (url: string): boolean =>
  url.startsWith("https://") &&
  url.length <= MAX_IMAGE &&
  !/[\s"'<>\\]/.test(url) &&
  !CONTROL.test(url);

/** A pick read from storage or made by the page; throws on anything the server would refuse. */
export function validatePick(v: unknown): RacePick {
  if (typeof v !== "object" || v === null) bad("choix invalide");
  const o = v as Record<string, unknown>;
  const allowed = new Set([
    "id",
    "race_id",
    "cat",
    "src",
    "name",
    "level",
    "track",
    "duration",
    "image",
    "start_ms",
    "eve_ms",
  ]);
  for (const k of Object.keys(o)) if (!allowed.has(k)) bad(`champ inconnu \`${k}\``);
  const raceId = o.race_id;
  if (typeof raceId !== "string" || !validRaceId(raceId)) bad("`race_id` invalide");
  const start = o.start_ms;
  if (typeof start !== "number" || !Number.isInteger(start)) bad("`start_ms` invalide");
  if (start < MIN_START_MS || start > MAX_START_MS) bad("`start_ms` invalide");
  if (o.id !== `${raceId}@${start}`) bad("`id` doit valoir <race_id>@<start_ms>");
  const cat = o.cat;
  if (typeof cat !== "string" || !(CATS as readonly string[]).includes(cat)) bad("`cat` invalide");
  if (o.src !== "rc" && o.src !== "lt") bad("`src` invalide");
  const level = o.level ?? null;
  if (
    level !== null &&
    (typeof level !== "string" || !(LEVELS as readonly string[]).includes(level))
  )
    bad("`level` invalide");
  const name = checkText("name", o.name, MAX_NAME, true);
  const track = checkText("track", o.track, MAX_NAME, true);
  const duration = checkText("duration", o.duration ?? "", MAX_DURATION, false);
  const image = o.image ?? null;
  if (image !== null && (typeof image !== "string" || !validImage(image))) bad("`image` invalide");
  const eve = o.eve_ms ?? null;
  if (eve !== null) {
    if (cat !== "special") bad("`eve_ms` est réservé aux événements spéciaux");
    if (typeof eve !== "number" || eve >= start || start - eve > 2 * 86_400_000)
      bad("`eve_ms` invalide");
  }
  return {
    id: o.id as string,
    race_id: raceId,
    cat: cat as RacePick["cat"],
    src: o.src as RacePick["src"],
    name,
    level: level as RacePick["level"],
    track,
    duration,
    image: image as string | null,
    start_ms: start,
    eve_ms: eve as number | null,
  };
}

function rating(field: string, v: unknown): string {
  if (typeof v === "string" && (RATINGS as readonly string[]).includes(v)) return v;
  return bad(`\`${field}\` : note invalide (B0 … P3 ou vide)`);
}

function ratingsPatch(field: string, v: unknown, into: Ratings): Ratings {
  if (typeof v !== "object" || v === null || Array.isArray(v))
    bad(`\`${field}\` : objet {sr, dr} attendu`);
  const out = { ...into };
  for (const [k, x] of Object.entries(v as Record<string, unknown>)) {
    if (k === "sr") out.sr = rating(`${field}.sr`, x);
    else if (k === "dr") out.dr = rating(`${field}.dr`, x);
    else bad(`\`${field}.${k}\` inconnu`);
  }
  return out;
}

function subset(field: string, v: unknown, allowed: readonly string[]): string[] {
  if (!Array.isArray(v)) bad(`\`${field}\` : liste attendue`);
  const out: string[] = [];
  for (const it of v) {
    if (typeof it !== "string" || !allowed.includes(it)) bad(`\`${field}\` : valeur inconnue`);
    if (!out.includes(it)) out.push(it);
  }
  if (out.length === 0) bad(`\`${field}\` : garde au moins une valeur`);
  return out.sort((a, b) => allowed.indexOf(a) - allowed.indexOf(b));
}

/** The settings with a patch applied; throws on an unknown key or value (nothing applied). */
export function applySettingsPatch(s: RaceSettings, patch: unknown): RaceSettings {
  if (typeof patch !== "object" || patch === null || Array.isArray(patch))
    bad("réglages invalides");
  const next: RaceSettings = clone(s);
  for (const [key, v] of Object.entries(patch as Record<string, unknown>)) {
    switch (key) {
      case "leads": {
        if (!Array.isArray(v)) bad("`leads` : liste attendue");
        const set = new Set<number>();
        for (const n of v) {
          if (typeof n !== "number" || !(LEAD_CHOICES as readonly number[]).includes(n))
            bad("`leads` : délai non proposé");
          set.add(n);
        }
        next.leads = [...set].sort((a, b) => b - a);
        break;
      }
      case "levels":
        next.levels = subset("levels", v, LEVELS);
        break;
      case "cats":
        next.cats = subset("cats", v, CATS);
        break;
      case "hour_fix":
        if (typeof v !== "number" || !Number.isInteger(v) || v < -2 || v > 2)
          bad("`hour_fix` : entier de -2 à 2 attendu");
        next.hour_fix = v;
        break;
      case "profile":
        next.profile = ratingsPatch("profile", v, next.profile);
        break;
      case "req": {
        if (typeof v !== "object" || v === null || Array.isArray(v)) bad("`req` : objet attendu");
        for (const [k, r] of Object.entries(v as Record<string, unknown>)) {
          if (!(REQ_KEYS as readonly string[]).includes(k)) bad(`\`req.${k}\` inconnu`);
          const cur = next.req[k as keyof RaceSettings["req"]] ?? { sr: "", dr: "" };
          next.req[k as keyof RaceSettings["req"]] = ratingsPatch(`req.${k}`, r, cur);
        }
        break;
      }
      default:
        bad(`réglage inconnu : \`${key}\``);
    }
  }
  return next;
}

/** Settings read from storage: each unreadable key falls back to its default. */
export function settingsFromStorage(v: unknown): RaceSettings {
  let s = defaultRaceSettings();
  if (typeof v !== "object" || v === null) return s;
  for (const [k, x] of Object.entries(v as Record<string, unknown>)) {
    try {
      s = applySettingsPatch(s, { [k]: x });
    } catch {
      // ignored: default kept
    }
  }
  return s;
}

const byStart = (a: RacePick, b: RacePick) =>
  a.start_ms - b.start_ms || (a.id < b.id ? -1 : a.id > b.id ? 1 : 0);

export class RaceBook {
  private picks: RacePick[] = [];
  private settings: RaceSettings = defaultRaceSettings();
  private loaded = false;

  constructor(
    private readonly prefs: PrefsPort,
    private readonly now: () => number = Date.now,
  ) {}

  async load(): Promise<void> {
    if (this.loaded) return;
    const raw = await readJson(this.prefs, KEYS.picks);
    const picks: RacePick[] = [];
    if (Array.isArray(raw)) {
      for (const p of raw) {
        try {
          const v = validatePick(p);
          if (!picks.some((x) => x.id === v.id)) picks.push(v);
        } catch {
          // dropped
        }
      }
    }
    this.picks = picks.slice(0, MAX_PICKS);
    this.settings = settingsFromStorage(await readJson(this.prefs, KEYS.raceSettings));
    this.loaded = true;
  }

  /** Drops the picks started more than 3 h ago; `true` when something changed. */
  private prune(): boolean {
    const before = this.picks.length;
    const limit = this.now() - KEEP_AFTER_START_MS;
    this.picks = this.picks.filter((p) => p.start_ms > limit).sort(byStart);
    return before !== this.picks.length;
  }

  private async savePicks(): Promise<void> {
    await writeJson(this.prefs, KEYS.picks, this.picks);
  }

  /** The picks still useful, by start. */
  async list(): Promise<RacePick[]> {
    await this.load();
    if (this.prune()) await this.savePicks();
    return this.picks.map((p) => ({ ...p }));
  }

  /** Adds or replaces a pick. 400 when invalid, 409 beyond 50 (same answers as the server). */
  async put(pick: RacePick): Promise<RacePick> {
    await this.load();
    let p: RacePick;
    try {
      p = validatePick(pick);
    } catch (e) {
      throw new ApiError(400, e instanceof Error ? e.message : String(e));
    }
    this.prune();
    const i = this.picks.findIndex((x) => x.id === p.id);
    if (i >= 0) this.picks[i] = p;
    else {
      if (this.picks.length >= MAX_PICKS) {
        throw new ApiError(409, t("races.error.full"));
      }
      this.picks.push(p);
    }
    this.picks.sort(byStart);
    await this.savePicks();
    return { ...p };
  }

  /** Removes a pick; `false` when there was none. */
  async remove(id: string): Promise<boolean> {
    await this.load();
    const before = this.picks.length;
    this.picks = this.picks.filter((p) => p.id !== id);
    const removed = before !== this.picks.length;
    this.prune();
    if (removed) await this.savePicks();
    return removed;
  }

  async getSettings(): Promise<RaceSettings> {
    await this.load();
    return clone(this.settings);
  }

  /** Applies a patch (400 when invalid); a new hour correction moves the racecontrol.gg picks. */
  async patchSettings(patch: RaceSettingsPatch): Promise<RaceSettings> {
    await this.load();
    let next: RaceSettings;
    try {
      next = applySettingsPatch(this.settings, patch);
    } catch (e) {
      throw new ApiError(400, e instanceof Error ? e.message : String(e));
    }
    const delta = next.hour_fix - this.settings.hour_fix;
    this.settings = next;
    await writeJson(this.prefs, KEYS.raceSettings, next);
    if (delta !== 0) {
      this.picks = this.picks.map((p) => {
        if (p.src !== "rc" || p.cat !== "daily") return p;
        const start = p.start_ms + delta * 3_600_000;
        return { ...p, start_ms: start, id: `${p.race_id}@${start}` };
      });
      this.prune();
      await this.savePicks();
    }
    return clone(next);
  }
}
