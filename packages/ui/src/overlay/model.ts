import type { DashboardLayout } from "@corde/protocol";

/**
 * The "overlay" mode: a widget, or a whole layout, alone on a transparent page, for an OBS
 * browser source. Everything here is pure (no DOM, no store) and unit-tested:
 *
 *   #/overlay/widget/<kind>[?options]     one widget of the registry, filling the source
 *   #/overlay/layout/<id>[?options]       one saved layout, widgets at their place
 *
 * Options (query string of the hash, or of the page — the hash wins):
 *
 *   bg=none|glass|solid   panel background (default glass: dark and translucent)
 *   opacity=0.1…1         opacity of the whole overlay (default 1)
 *   scale=0.5…3           size of texts and graphics (default 1)
 *   hide=offtrack         fade out while the driver is not on track
 *   lang=fr|en            language (default: the one saved on this browser)
 *   title=0               no widget title
 *   w.<key>=<value>       a widget setting (single widget only), among the values it offers
 *
 * Invalid values are ignored: the default applies.
 */

export type OverlayBg = "none" | "glass" | "solid";
export type OverlayLang = "fr" | "en";

export interface OverlayOptions {
  bg: OverlayBg;
  opacity: number;
  scale: number;
  hideOffTrack: boolean;
  lang: OverlayLang | null;
  title: boolean;
  /** Widget settings (`w.<key>`), validated later against the widget's own choices. */
  widget: Record<string, string>;
}

export const DEFAULT_OPTIONS: OverlayOptions = {
  bg: "glass",
  opacity: 1,
  scale: 1,
  hideOffTrack: false,
  lang: null,
  title: true,
  widget: {},
};

export const OPACITY_MIN = 0.1;
export const OPACITY_MAX = 1;
export const SCALE_MIN = 0.5;
export const SCALE_MAX = 3;

export type OverlayTarget = { kind: "widget"; id: string } | { kind: "layout"; id: string };

/** Widget kinds and layout ids: letters, digits, `-`, `_`, `.` (no leading dot), ≤ 64. */
const ID = /^[A-Za-z0-9_-][A-Za-z0-9._-]{0,63}$/;
const WIDGET_KEY = /^[a-z][a-zA-Z0-9_]{0,31}$/;

function splitHash(hash: string): { path: string; query: string } {
  let h = hash.startsWith("#") ? hash.slice(1) : hash;
  if (!h.startsWith("/")) h = `/${h}`;
  const q = h.indexOf("?");
  return q >= 0 ? { path: h.slice(0, q), query: h.slice(q + 1) } : { path: h, query: "" };
}

/** `true` when the hash names an overlay page (even an invalid one): the app then boots bare. */
export function isOverlayHash(hash: string): boolean {
  const { path } = splitHash(hash);
  return path === "/overlay" || path.startsWith("/overlay/");
}

/** The widget or layout named by an overlay hash, or `null`. */
export function parseOverlayTarget(hash: string): OverlayTarget | null {
  const { path } = splitHash(hash);
  const parts = path.split("/").filter((p) => p.length > 0);
  if (parts.length !== 3 || parts[0] !== "overlay") return null;
  let id: string;
  try {
    id = decodeURIComponent(parts[2] ?? "");
  } catch {
    return null;
  }
  if (!ID.test(id)) return null;
  if (parts[1] === "widget") return { kind: "widget", id };
  if (parts[1] === "layout") return { kind: "layout", id };
  return null;
}

function parseNumber(raw: string | null, min: number, max: number): number | null {
  if (raw === null || raw.trim() === "") return null;
  const v = Number(raw.trim().replace(",", "."));
  if (!Number.isFinite(v) || v < min || v > max) return null;
  return v;
}

/** Reads the options from one query string; what is absent or invalid stays as in `base`. */
function readQuery(query: string, base: OverlayOptions): OverlayOptions {
  const p = new URLSearchParams(query);
  const out: OverlayOptions = { ...base, widget: { ...base.widget } };
  const bg = p.get("bg")?.trim().toLowerCase();
  if (bg === "none" || bg === "glass" || bg === "solid") out.bg = bg;
  const opacity = parseNumber(p.get("opacity"), OPACITY_MIN, OPACITY_MAX);
  if (opacity !== null) out.opacity = opacity;
  const scale = parseNumber(p.get("scale"), SCALE_MIN, SCALE_MAX);
  if (scale !== null) out.scale = scale;
  const hide = p.get("hide")?.trim().toLowerCase();
  if (hide === "offtrack") out.hideOffTrack = true;
  else if (hide === "none" || hide === "") out.hideOffTrack = false;
  const lang = p.get("lang")?.trim().toLowerCase();
  if (lang === "fr" || lang === "en") out.lang = lang;
  const title = p.get("title")?.trim().toLowerCase();
  if (title === "0" || title === "off" || title === "false") out.title = false;
  else if (title === "1" || title === "on" || title === "true") out.title = true;
  for (const [k, v] of p) {
    if (!k.startsWith("w.")) continue;
    const key = k.slice(2);
    if (WIDGET_KEY.test(key) && v.length > 0 && v.length <= 32) out.widget[key] = v;
  }
  return out;
}

/** Options of an overlay page: the page's query string first, then the hash's (which wins). */
export function parseOverlayOptions(search: string, hash: string): OverlayOptions {
  const fromPage = readQuery(search.startsWith("?") ? search.slice(1) : search, DEFAULT_OPTIONS);
  return readQuery(splitHash(hash).query, fromPage);
}

/** Rounds a slider value so that addresses stay short and stable (`0.85`, `1.25`). */
export function roundOption(v: number): number {
  return Math.round(v * 100) / 100;
}

/** The query string of the options (defaults left out), without the leading `?`. */
export function optionsQuery(o: OverlayOptions): string {
  const p = new URLSearchParams();
  if (o.bg !== DEFAULT_OPTIONS.bg) p.set("bg", o.bg);
  if (roundOption(o.opacity) !== DEFAULT_OPTIONS.opacity)
    p.set("opacity", String(roundOption(o.opacity)));
  if (roundOption(o.scale) !== DEFAULT_OPTIONS.scale) p.set("scale", String(roundOption(o.scale)));
  if (o.hideOffTrack) p.set("hide", "offtrack");
  if (o.lang) p.set("lang", o.lang);
  if (!o.title) p.set("title", "0");
  for (const [k, v] of Object.entries(o.widget).sort(([a], [b]) => a.localeCompare(b))) {
    if (WIDGET_KEY.test(k)) p.set(`w.${k}`, v);
  }
  return p.toString();
}

/** `#/overlay/widget/laps?bg=none` */
export function overlayHash(target: OverlayTarget, o: OverlayOptions = DEFAULT_OPTIONS): string {
  const q = optionsQuery(o);
  return `#/overlay/${target.kind}/${encodeURIComponent(target.id)}${q ? `?${q}` : ""}`;
}

/**
 * The address to paste into OBS: `http://127.0.0.1:8471/#/overlay/widget/laps?bg=none`.
 * `base` is the server (`http://host:port`, a trailing slash is tolerated).
 */
export function overlayUrl(
  base: string,
  target: OverlayTarget,
  o: OverlayOptions = DEFAULT_OPTIONS,
): string {
  return `${base.replace(/\/+$/, "")}/${overlayHash(target, o)}`;
}

/* ------------------------------------------------------------------ geometry */

/** Overlay layouts: a free grid of 48 × 27 cells of 40 px on a 1920 × 1080 canvas. */
export const OVERLAY_CANVAS = { width: 1920, height: 1080 } as const;
export const OVERLAY_COLUMNS = 48;
export const OVERLAY_ROWS = 27;
export const OVERLAY_CELL = 40;
/** Desktop and mobile layouts shown in an overlay: the row height of a 1080p screen. */
const DESKTOP_WIDTH = 1920;
const DESKTOP_ROW = 64;
const MOBILE_WIDTH = 480;
const MOBILE_ROW = 72;

export interface Canvas {
  width: number;
  height: number;
  cellW: number;
  cellH: number;
}

/** The logical canvas of a layout (what the source should measure at scale 1). */
export function layoutCanvas(
  layout: Pick<DashboardLayout, "device" | "columns" | "widgets">,
): Canvas {
  const columns = Math.max(1, layout.columns);
  if (layout.device === "overlay") {
    const cellW = OVERLAY_CANVAS.width / columns;
    return { ...OVERLAY_CANVAS, cellW, cellH: OVERLAY_CELL };
  }
  const width = layout.device === "mobile" ? MOBILE_WIDTH : DESKTOP_WIDTH;
  const cellH = layout.device === "mobile" ? MOBILE_ROW : DESKTOP_ROW;
  const rows = Math.max(1, ...layout.widgets.map((w) => w.y + w.h));
  return { width, height: rows * cellH, cellW: width / columns, cellH };
}

/** Uniform factor that fits a canvas inside the viewport (never 0). */
export function fitFactor(canvas: { width: number; height: number }, vw: number, vh: number) {
  if (vw <= 0 || vh <= 0) return 1;
  return Math.max(0.05, Math.min(vw / canvas.width, vh / canvas.height));
}

export interface PxSize {
  width: number;
  height: number;
}

/**
 * Size advised for the OBS source of one widget: the widget's own overlay size at scale 1, else
 * its desktop size on a 1080p screen (160 × 64 px cells); multiplied by `scale` so that a bigger
 * text keeps the same room. Rounded to even pixels.
 */
export function recommendedWidgetSize(
  def: { overlaySize?: PxSize; defaultSize: { w: number; h: number } },
  scale = 1,
): PxSize {
  const base = def.overlaySize ?? {
    width: def.defaultSize.w * 160,
    height: def.defaultSize.h * 64,
  };
  const s = Number.isFinite(scale) && scale > 0 ? scale : 1;
  const even = (v: number) => Math.max(2, Math.round((v * s) / 2) * 2);
  return { width: even(base.width), height: even(base.height) };
}

/** Size advised for the OBS source of a layout: its canvas (1920 × 1080 for an overlay layout). */
export function recommendedLayoutSize(
  layout: Pick<DashboardLayout, "device" | "columns" | "widgets">,
): PxSize {
  const c = layoutCanvas(layout);
  return { width: Math.round(c.width), height: Math.round(c.height) };
}

/** Overlay size in px → cells of the overlay grid (at least the widget's minimum). */
export function overlayCells(
  size: PxSize,
  min: { w: number; h: number },
): { w: number; h: number } {
  return {
    w: Math.min(OVERLAY_COLUMNS, Math.max(min.w, Math.round(size.width / OVERLAY_CELL))),
    h: Math.min(OVERLAY_ROWS, Math.max(min.h, Math.round(size.height / OVERLAY_CELL))),
  };
}

/**
 * First place where a `w × h` widget fits on a bounded grid without covering another one
 * (row by row, left to right); `null` when the grid is full.
 */
export function freeSpot(
  widgets: ReadonlyArray<{ x: number; y: number; w: number; h: number }>,
  w: number,
  h: number,
  columns: number,
  rows: number,
): { x: number; y: number } | null {
  const overlaps = (x: number, y: number) =>
    widgets.some((o) => x < o.x + o.w && o.x < x + w && y < o.y + o.h && o.y < y + h);
  for (let y = 0; y + h <= rows; y += 1) {
    for (let x = 0; x + w <= columns; x += 1) {
      if (!overlaps(x, y)) return { x, y };
    }
  }
  return null;
}

/* ------------------------------------------------------------------ off track */

/** How long the driver must be off track before a `hide=offtrack` overlay fades out. */
export const OFFTRACK_GRACE_MS = 2000;

/**
 * Is the overlay shown? Always without `hide=offtrack`; with it, while the driver is on track,
 * and for {@link OFFTRACK_GRACE_MS} after they left it (a reconnection or a short trip through
 * a menu does not blink the stream). `offSince` is when the driver was last seen leaving the
 * track (`null` = never seen on track since the page opened: hidden at once).
 */
export function overlayShown(
  hideOffTrack: boolean,
  onTrack: boolean,
  offSince: number | null,
  now: number,
): boolean {
  if (!hideOffTrack || onTrack) return true;
  if (offSince === null) return false;
  return now - offSince < OFFTRACK_GRACE_MS;
}

/** What counts as "on track" for the overlay: the game says it is in real time with a car. */
export function isOnTrack(s: {
  socket: string;
  demo: string;
  status: { connected: boolean; in_realtime: boolean } | null;
  frame: { player: unknown; session: { in_realtime: boolean } } | null;
}): boolean {
  if (!s.frame?.player) return false;
  if (s.demo !== "off") return true;
  if (s.socket !== "open" || !s.status?.connected) return false;
  return s.status.in_realtime || s.frame.session.in_realtime;
}
