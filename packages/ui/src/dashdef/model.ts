import type {
  DashBackground,
  DashCondition,
  DashDefinition,
  DashElement,
  DashFormat,
  DashPageDef,
  DashStyle,
  DashTheme,
  DashThreshold,
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

export type {
  DashBackground,
  DashCondition,
  DashDefinition,
  DashElement,
  DashFormat,
  DashPageDef,
  DashStyle,
  DashTheme,
  DashThreshold,
};

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
/** Comparisons of the conditions and colour rules (ADR 0016): a closed list. */
export const OPS = ["lt", "le", "ge", "gt"] as const;
export type Op = (typeof OPS)[number];
/** What « Afficher quand… » can test. */
export const CONDITIONS = ["limiter", "pits", "yellow", "blue", "field"] as const;
export type ConditionKind = (typeof CONDITIONS)[number];
/** The fonts of the numbers, all shipped with Corde (no download). */
export const FONTS = ["condensed", "barlow", "digital", "wide"] as const;
export type FontId = (typeof FONTS)[number];
/** How a picture fills its box. */
export const FITS = ["contain", "cover", "fill"] as const;
export type Fit = (typeof FITS)[number];
export const MAX_THRESHOLDS = 4;
export const MAX_RADIUS = 40;
export const MAX_BORDER_WIDTH = 8;
/** The dashboard shipped with Corde: never stored on the PC. */
export const BUILTIN_ID = "corde";

/** The grid of a new page. */
export const DEFAULT_COLS = 24;
export const DEFAULT_ROWS = 12;

/** The shapes of a page (ADR 0018), a closed list. Left out of a definition: `rect`. */
export const SHAPES = ["rect", "rounded", "round"] as const;
export type ShapeId = (typeof SHAPES)[number];

export type PresetId =
  | "phone-landscape"
  | "phone-portrait"
  | "tablet"
  | "screen-16-9"
  | "wheel-strip"
  | "wheel-round"
  | "wheel-moza"
  | "square"
  | "screen-5-3"
  | "screen-3-2"
  | "fill";

/**
 * The drawing formats offered (`custom` is any other size, any shape): the same list as
 * `PRESETS` of `corde-server/src/dashes.rs`. A preset is a name for these values, nothing more:
 * an id that is not here (a definition made by a later version) is drawn as a custom size.
 */
export const PRESETS: Record<PresetId, { width: number; height: number; shape?: ShapeId }> = {
  "phone-landscape": { width: 852, height: 393 },
  "phone-portrait": { width: 393, height: 852 },
  tablet: { width: 1024, height: 768 },
  "screen-16-9": { width: 1920, height: 1080 },
  "wheel-strip": { width: 800, height: 240 },
  "wheel-round": { width: 480, height: 480, shape: "round" },
  // The screen of a MOZA KS Pro or CS Pro wheel: what « Exporter pour un volant MOZA » writes.
  "wheel-moza": { width: 780, height: 248 },
  square: { width: 720, height: 720 },
  "screen-5-3": { width: 800, height: 480 },
  "screen-3-2": { width: 480, height: 320 },
  fill: { width: 852, height: 393 },
};

export const PRESET_IDS = Object.keys(PRESETS) as PresetId[];

/** The format of a preset, as a definition holds it. */
export function presetFormat(id: PresetId): DashFormat {
  return { preset: id, ...PRESETS[id] };
}

/** The preset of a format, `custom` for any id that is not in the list. */
export function presetOf(format: Pick<DashFormat, "preset">): PresetId | "custom" {
  return (PRESET_IDS as string[]).includes(format.preset) ? (format.preset as PresetId) : "custom";
}

/** The largest corner radius of a `rounded` page: half its short side. */
export function maxShapeRadius(width: number, height: number): number {
  return Math.floor(Math.min(width, height) / 2);
}

/**
 * The shape a format is drawn with: `rect` when it names none (every definition before ADR
 * 0018), when it names one this version does not know, or for `fill` (the whole screen).
 */
export function shapeOf(format: Pick<DashFormat, "preset" | "shape">): ShapeId {
  if (format.preset === "fill") return "rect";
  const s = format.shape;
  return typeof s === "string" && (SHAPES as readonly string[]).includes(s)
    ? (s as ShapeId)
    : "rect";
}

/** The corner radius of a `rounded` format, in pixels of its size, held within its bounds. */
export function radiusOf(format: DashFormat): number {
  if (shapeOf(format) !== "rounded") return 0;
  const r = typeof format.radius === "number" && Number.isFinite(format.radius) ? format.radius : 0;
  return Math.max(0, Math.min(maxShapeRadius(format.width, format.height), Math.round(r)));
}

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
const FIELD_ID = /^[a-z0-9_]{1,40}$/;
/** A media of the PC: the SHA-256 of its content and the extension of its type. */
const ASSET_ID = /^[0-9a-f]{64}\.(png|jpg|webp|gif|mp4|webm)$/;

/** The id of a picture or a video stored on the PC (`dash-assets`, ADR 0016). */
export function validAssetId(id: unknown): id is string {
  return typeof id === "string" && ASSET_ID.test(id);
}

/** A setting that cites a media (`asset`, `…Asset`). */
export function isAssetKey(k: string): boolean {
  return k === "asset" || k.endsWith("Asset");
}

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
    | "props"
    | "media"
    | "style"
    | "condition"
    | "archive";
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
    if (isAssetKey(k)) {
      if (!(val === null || val === "" || validAssetId(val))) fail("media", `${at}.${k}`);
    }
  }
  return out;
}

function bounded(v: unknown, at: string, code: DashProblem["code"]): number {
  if (typeof v !== "number" || !Number.isFinite(v) || Math.abs(v) > MAX_PROP_NUMBER) fail(code, at);
  return v;
}

function opacity(v: unknown, at: string, code: DashProblem["code"]): number {
  if (typeof v !== "number" || !Number.isFinite(v) || v < 0 || v > 1) fail(code, at);
  return v;
}

function optColor(v: unknown, at: string): string {
  if (!validColor(v)) fail("color", at);
  return v;
}

function style(v: unknown, at: string): DashStyle {
  if (!isObject(v)) fail("style", at);
  const out: DashStyle = {};
  if (v.bg != null) out.bg = optColor(v.bg, `${at}.bg`);
  if (v.border != null) out.border = optColor(v.border, `${at}.border`);
  if (v.opacity != null) out.opacity = opacity(v.opacity, `${at}.opacity`, "style");
  if (v.radius != null) out.radius = int(v.radius, `${at}.radius`, 0, MAX_RADIUS, "style");
  if (v.border_width != null)
    out.border_width = int(v.border_width, `${at}.border_width`, 0, MAX_BORDER_WIDTH, "style");
  return out;
}

function isOp(v: unknown): v is Op {
  return typeof v === "string" && (OPS as readonly string[]).includes(v);
}

function condition(v: unknown, at: string): DashCondition {
  if (!isObject(v)) fail("condition", at);
  const when = v.when;
  if (typeof when !== "string" || !(CONDITIONS as readonly string[]).includes(when))
    fail("condition", at);
  const has = (k: string) => v[k] !== undefined && v[k] !== null;
  if (when !== "field") {
    if (has("field") || has("op") || has("value")) fail("condition", at);
    return { when };
  }
  if (typeof v.field !== "string" || !FIELD_ID.test(v.field)) fail("condition", at);
  if (!isOp(v.op)) fail("condition", at);
  return { when, field: v.field, op: v.op, value: bounded(v.value, at, "condition") };
}

function thresholds(v: unknown, at: string): DashThreshold[] {
  if (!Array.isArray(v) || v.length > MAX_THRESHOLDS) fail("condition", at);
  return v.map((t, i) => {
    const a = `${at}[${i}]`;
    if (!isObject(t) || !isOp(t.op)) fail("condition", a);
    return { op: t.op, value: bounded(t.value, a, "condition"), color: optColor(t.color, a) };
  });
}

function background(v: unknown, at: string): DashBackground {
  if (!isObject(v)) fail("style", at);
  const out: DashBackground = {};
  if (v.color != null) out.color = optColor(v.color, `${at}.color`);
  if (v.asset != null) {
    if (!validAssetId(v.asset)) fail("media", `${at}.asset`);
    out.asset = v.asset;
  }
  if (v.fit != null) {
    if (typeof v.fit !== "string" || !(FITS as readonly string[]).includes(v.fit))
      fail("style", `${at}.fit`);
    out.fit = v.fit;
  }
  if (v.opacity != null) out.opacity = opacity(v.opacity, `${at}.opacity`, "style");
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
  const el: DashElement = { id: v.id, type: v.type, x, y, w, h, props: props(v.props ?? {}, at) };
  if (v.style != null) el.style = style(v.style, `${at}.style`);
  if (v.show != null) el.show = condition(v.show, `${at}.show`);
  if (v.thresholds != null) el.thresholds = thresholds(v.thresholds, `${at}.thresholds`);
  return el;
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
  const pg: DashPageDef = { id: v.id, name, cols, rows, elements };
  if (v.background != null) pg.background = background(v.background, `${at}.background`);
  return pg;
}

/** The format: its preset id, its size, its proportions, its shape (ADR 0018). */
function format(f: unknown): DashFormat {
  if (!isObject(f) || typeof f.preset !== "string" || f.preset.length > 24 || !ID.test(f.preset))
    fail("format", "format");
  const width = int(f.width, "format", MIN_FORMAT_SIDE, MAX_FORMAT_SIDE, "format");
  const height = int(f.height, "format", MIN_FORMAT_SIDE, MAX_FORMAT_SIDE, "format");
  const ratio = width / height;
  if (ratio < 1 / MAX_FORMAT_RATIO || ratio > MAX_FORMAT_RATIO) fail("format", "format");
  const out: DashFormat = { preset: f.preset, width, height };
  let shape: ShapeId = "rect";
  if (f.shape != null) {
    if (typeof f.shape !== "string" || !(SHAPES as readonly string[]).includes(f.shape))
      fail("format", "format.shape");
    shape = f.shape as ShapeId;
    out.shape = shape;
  }
  // The whole screen has no shape of its own.
  if (f.preset === "fill" && shape !== "rect") fail("format", "format.shape");
  if (shape === "rounded") {
    out.radius = int(f.radius, "format.radius", 1, maxShapeRadius(width, height), "format");
  } else if (f.radius != null) {
    // A radius only with rounded corners.
    fail("format", "format.radius");
  }
  return out;
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
  const fmt = format(v.format);
  const th = v.theme;
  if (!isObject(th)) fail("color", "theme");
  const theme = {} as DashTheme;
  for (const k of THEME_KEYS) {
    if (!validColor(th[k])) fail("color", `theme.${k}`);
    theme[k] = th[k] as string;
  }
  if (th.font != null) {
    if (typeof th.font !== "string" || !(FONTS as readonly string[]).includes(th.font))
      fail("style", "theme.font");
    theme.font = th.font;
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
    format: fmt,
    theme,
    pages,
    updated_ms: updated ? Math.max(0, Math.floor(v.updated_ms as number)) : 0,
  };
  if (byteSize(def) > MAX_DASH_BYTES) fail("too-large", "");
  return def;
}

/** The media a definition cites (its elements' settings, its pages' backgrounds). */
export function referencedAssets(def: DashDefinition): string[] {
  const out = new Set<string>();
  for (const p of def.pages) {
    if (validAssetId(p.background?.asset)) out.add(p.background.asset);
    for (const e of p.elements) {
      for (const [k, v] of Object.entries(e.props)) {
        if (isAssetKey(k) && validAssetId(v)) out.add(v);
      }
    }
  }
  return [...out];
}

/** UTF-8 size of the compact JSON of a value. */
export function byteSize(v: unknown): number {
  return new TextEncoder().encode(JSON.stringify(v)).length;
}

/**
 * Checks a definition against every rule (ADRs 0015, 0016, 0018) and returns a clean copy: only the known
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
