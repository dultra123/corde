import type {
  DashDefinition,
  DashElement,
  DashFormat,
  DashPageDef,
  DashTheme,
} from "@corde/protocol";

/**
 * Race dashboards as data (ADR 0015): a `DashDefinition` is pages of elements placed on a grid,
 * drawn at the scale of the screen. The same rules as the server (`corde-server/src/dashes.rs`):
 * a definition that one side accepts, the other accepts too (the tests of both sides cover the
 * same cases).
 *
 * A definition is pure data. Nothing in it is ever run, nor written into the page as HTML (React
 * puts every string in as text); the rules below keep it inert anyway.
 */

export type { DashDefinition, DashElement, DashFormat, DashPageDef, DashTheme };

export const DASH_VERSION = 1;
export const MAX_DASH_BYTES = 256 * 1024;
export const MAX_DASHES = 50;
export const MAX_PAGES = 12;
export const MAX_ELEMENTS = 80;
export const MAX_ID_LEN = 40;
export const MAX_NAME_LEN = 60;
export const MAX_PAGE_NAME_LEN = 40;
export const MAX_PROP_KEYS = 32;
export const MAX_PROP_STRING_LEN = 200;
export const MAX_PROP_ARRAY_LEN = 16;
export const MAX_PROP_NUMBER = 1_000_000;
export const MIN_COLS = 4;
export const MAX_COLS = 48;
export const MIN_ROWS = 2;
export const MAX_ROWS = 32;
export const MIN_FORMAT_SIDE = 100;
export const MAX_FORMAT_SIDE = 4000;
export const MAX_FORMAT_RATIO = 8;
/** The dashboard shipped with Corde: never stored on the PC. */
export const BUILTIN_ID = "corde";

/** The grid of a new page. */
export const DEFAULT_COLS = 24;
export const DEFAULT_ROWS = 12;

export type PresetId =
  | "phone-landscape"
  | "phone-portrait"
  | "tablet"
  | "screen-16-9"
  | "wheel-strip"
  | "fill";

/** The drawing formats offered (`custom` is any other size). */
export const PRESETS: Record<PresetId, { width: number; height: number }> = {
  "phone-landscape": { width: 852, height: 393 },
  "phone-portrait": { width: 393, height: 852 },
  tablet: { width: 1024, height: 768 },
  "screen-16-9": { width: 1920, height: 1080 },
  "wheel-strip": { width: 800, height: 240 },
  fill: { width: 852, height: 393 },
};

export const PRESET_IDS = Object.keys(PRESETS) as PresetId[];

/** The colours of the dashboard shipped with Corde (`styles/dash.css`, `styles/tokens.css`). */
export const DEFAULT_THEME: DashTheme = {
  background: "#000000",
  panel: "#0d1116",
  line: "#1f2731",
  text: "#f2f5f8",
  dim: "#93a0ae",
  accent: "#ffb454",
};

export const THEME_KEYS = ["background", "panel", "line", "text", "dim", "accent"] as const;

// ---------------------------------------------------------------- checks

const ID = /^[a-z0-9-]{1,40}$/;
const TYPE = /^[a-z0-9.-]{1,40}$/;
const COLOR = /^#[0-9a-fA-F]{6}$/;
const PROP_KEY = /^[A-Za-z][A-Za-z0-9_]{0,31}$/;

export function validId(id: unknown): id is string {
  return typeof id === "string" && ID.test(id);
}

export function validColor(c: unknown): c is string {
  return typeof c === "string" && COLOR.test(c);
}

/** Why a definition was refused: a code for the message, and where. */
export interface DashProblem {
  code:
    | "not-json"
    | "too-large"
    | "version"
    | "shape"
    | "id"
    | "text"
    | "format"
    | "color"
    | "pages"
    | "elements"
    | "grid"
    | "outside"
    | "props";
  at: string;
}

export type Checked = { ok: true; def: DashDefinition } | { ok: false; problem: DashProblem };

class Refused extends Error {
  constructor(readonly problem: DashProblem) {
    super(`${problem.code} at ${problem.at}`);
  }
}

function fail(code: DashProblem["code"], at: string): never {
  throw new Refused({ code, at });
}

function isObject(v: unknown): v is Record<string, unknown> {
  return typeof v === "object" && v !== null && !Array.isArray(v);
}

/** Characters, like Rust's `chars().count()`. */
function length(s: string): number {
  return [...s].length;
}

/** A text of a definition: bounded, no control character, no markup, no address. */
export function textProblem(s: string, min: number, max: number): boolean {
  const n = length(s);
  if (n < min || n > max) return true;
  // biome-ignore lint/suspicious/noControlCharactersInRegex: control characters are refused
  if (/[\u0000-\u001f\u007f-\u009f]/.test(s)) return true;
  if (s.includes("<") || s.includes(">")) return true;
  const lower = s.toLowerCase();
  if (lower.includes("://") || lower.includes("www.")) return true;
  const start = lower.trimStart();
  return ["javascript:", "data:", "vbscript:", "file:"].some((p) => start.startsWith(p));
}

function text(v: unknown, at: string, min: number, max: number): string {
  if (typeof v !== "string" || textProblem(v, min, max)) fail("text", at);
  return v;
}

function int(v: unknown, at: string, min: number, max: number, code: DashProblem["code"]): number {
  if (typeof v !== "number" || !Number.isInteger(v) || v < min || v > max) fail(code, at);
  return v;
}

function scalar(v: unknown, at: string): unknown {
  if (v === null || typeof v === "boolean") return v;
  if (typeof v === "number") {
    if (!Number.isFinite(v) || Math.abs(v) > MAX_PROP_NUMBER) fail("props", at);
    return v;
  }
  if (typeof v === "string") {
    if (textProblem(v, 0, MAX_PROP_STRING_LEN)) fail("props", at);
    if (v.startsWith("#") && !validColor(v)) fail("color", at);
    return v;
  }
  return fail("props", at);
}

function props(v: unknown, at: string): Record<string, unknown> {
  if (!isObject(v)) fail("props", at);
  const keys = Object.keys(v);
  if (keys.length > MAX_PROP_KEYS) fail("props", at);
  const out: Record<string, unknown> = {};
  for (const k of keys) {
    if (!PROP_KEY.test(k)) fail("props", `${at}.${k}`);
    const val = v[k];
    if (Array.isArray(val)) {
      if (val.length > MAX_PROP_ARRAY_LEN) fail("props", `${at}.${k}`);
      out[k] = val.map((x) => scalar(x, `${at}.${k}`));
    } else {
      out[k] = scalar(val, `${at}.${k}`);
    }
    if (k === "color" || k.endsWith("Color")) {
      if (!(val === null || val === "auto" || validColor(val))) fail("color", `${at}.${k}`);
    }
  }
  return out;
}

function element(v: unknown, at: string, cols: number, rows: number): DashElement {
  if (!isObject(v)) fail("shape", at);
  if (!validId(v.id)) fail("id", at);
  if (typeof v.type !== "string" || !TYPE.test(v.type)) fail("elements", at);
  const x = int(v.x, at, 0, 255, "outside");
  const y = int(v.y, at, 0, 255, "outside");
  const w = int(v.w, at, 1, 255, "outside");
  const h = int(v.h, at, 1, 255, "outside");
  if (x + w > cols || y + h > rows) fail("outside", at);
  return { id: v.id, type: v.type, x, y, w, h, props: props(v.props ?? {}, at) };
}

function page(v: unknown, at: string): DashPageDef {
  if (!isObject(v)) fail("shape", at);
  if (!validId(v.id)) fail("id", at);
  const name = text(v.name, `${at}.name`, 1, MAX_PAGE_NAME_LEN);
  const cols = int(v.cols, at, MIN_COLS, MAX_COLS, "grid");
  const rows = int(v.rows, at, MIN_ROWS, MAX_ROWS, "grid");
  if (!Array.isArray(v.elements) || v.elements.length > MAX_ELEMENTS) fail("elements", at);
  const ids = new Set<string>();
  const elements = v.elements.map((e, i) => {
    const el = element(e, `${at}.elements[${i}]`, cols, rows);
    if (ids.has(el.id)) fail("id", `${at}.elements[${i}]`);
    ids.add(el.id);
    return el;
  });
  return { id: v.id, name, cols, rows, elements };
}

function check(v: unknown): DashDefinition {
  if (!isObject(v)) fail("shape", "");
  if (v.v !== DASH_VERSION) fail("version", "v");
  if (!validId(v.id)) fail("id", "id");
  const name = text(v.name, "name", 1, MAX_NAME_LEN);
  if (name.trim() === "") fail("text", "name");
  let author: string | undefined;
  if (v.author !== undefined && v.author !== null) {
    author = text(v.author, "author", 0, MAX_NAME_LEN);
  }
  const f = v.format;
  if (!isObject(f) || typeof f.preset !== "string" || f.preset.length > 24 || !ID.test(f.preset))
    fail("format", "format");
  const width = int(f.width, "format", MIN_FORMAT_SIDE, MAX_FORMAT_SIDE, "format");
  const height = int(f.height, "format", MIN_FORMAT_SIDE, MAX_FORMAT_SIDE, "format");
  const ratio = width / height;
  if (ratio < 1 / MAX_FORMAT_RATIO || ratio > MAX_FORMAT_RATIO) fail("format", "format");
  const th = v.theme;
  if (!isObject(th)) fail("color", "theme");
  const theme = {} as DashTheme;
  for (const k of THEME_KEYS) {
    if (!validColor(th[k])) fail("color", `theme.${k}`);
    theme[k] = th[k] as string;
  }
  if (!Array.isArray(v.pages) || v.pages.length < 1 || v.pages.length > MAX_PAGES)
    fail("pages", "pages");
  const ids = new Set<string>();
  const pages = v.pages.map((p, i) => {
    const pg = page(p, `pages[${i}]`);
    if (ids.has(pg.id)) fail("id", `pages[${i}]`);
    ids.add(pg.id);
    return pg;
  });
  const updated = typeof v.updated_ms === "number" && Number.isFinite(v.updated_ms);
  const def: DashDefinition = {
    v: DASH_VERSION,
    id: v.id,
    name,
    ...(author ? { author } : {}),
    format: { preset: f.preset, width, height },
    theme,
    pages,
    updated_ms: updated ? Math.max(0, Math.floor(v.updated_ms as number)) : 0,
  };
  if (byteSize(def) > MAX_DASH_BYTES) fail("too-large", "");
  return def;
}

/** UTF-8 size of the compact JSON of a value. */
export function byteSize(v: unknown): number {
  return new TextEncoder().encode(JSON.stringify(v)).length;
}

/**
 * Checks a definition against every rule (ADR 0015) and returns a clean copy: only the known
 * keys of the definition, its pages and its elements (the settings of an element are kept
 * whole, unknown ones included, once checked).
 */
export function validateDefinition(v: unknown): Checked {
  try {
    return { ok: true, def: check(v) };
  } catch (e) {
    if (e instanceof Refused) return { ok: false, problem: e.problem };
    return { ok: false, problem: { code: "shape", at: "" } };
  }
}

/** Parses then validates a text (a file being imported). */
export function parseDefinition(json: string): Checked {
  if (new TextEncoder().encode(json).length > MAX_DASH_BYTES * 2) {
    return { ok: false, problem: { code: "too-large", at: "" } };
  }
  let raw: unknown;
  try {
    raw = JSON.parse(json);
  } catch {
    return { ok: false, problem: { code: "not-json", at: "" } };
  }
  return validateDefinition(raw);
}

/** The aspect ratio a definition is drawn at, `null` for `fill` (the whole screen). */
export function aspectOf(def: Pick<DashDefinition, "format">): number | null {
  if (def.format.preset === "fill") return null;
  return def.format.width / def.format.height;
}
