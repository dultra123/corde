import type { Lang } from "../store/settings";
import { cordeDefinition, type TemplateId, template } from "./builtin";
import { getElementSpec, propsWithDefaults } from "./catalog";
import { cleanFormat, sameFormat, suggestGrid, withPreset } from "./format";
import {
  BUILTIN_ID,
  type DashBackground,
  type DashCondition,
  type DashDefinition,
  type DashElement,
  type DashFormat,
  type DashPageDef,
  type DashStyle,
  type DashThreshold,
  DEFAULT_COLS,
  DEFAULT_ROWS,
  type FontId,
  MAX_COLS,
  MAX_ELEMENTS,
  MAX_ID_LEN,
  MAX_NAME_LEN,
  MAX_PAGE_NAME_LEN,
  MAX_PAGES,
  MAX_ROWS,
  MIN_COLS,
  MIN_ROWS,
  type PresetId,
} from "./model";

/**
 * The operations of the Studio on a definition, without React: each returns a new definition
 * (or the same one when nothing changes), always within the rules of `model.ts`.
 */

/** A slug of a name, for an id: `GT3 nuit` → `gt3-nuit`. */
export function slug(name: string): string {
  const s = name
    .normalize("NFD")
    .replace(/[̀-ͯ]/g, "")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 24)
    .replace(/-+$/g, "");
  return s || "tableau";
}

/** Device names of Windows: `con.json` or `nul.json` there is the device, not a file. */
const WINDOWS_DEVICES = [
  "con",
  "prn",
  "aux",
  "nul",
  ...Array.from({ length: 10 }, (_, i) => `com${i}`),
  ...Array.from({ length: 10 }, (_, i) => `lpt${i}`),
];

/**
 * A new id `[a-z0-9-]{1,40}` not in `taken` (never `corde`, never a device name of Windows: the
 * server refuses those).
 */
export function freshId(base: string, taken: Iterable<string>): string {
  const used = new Set(taken);
  used.add(BUILTIN_ID);
  for (const name of WINDOWS_DEVICES) used.add(name);
  const root = slug(base).slice(0, MAX_ID_LEN - 4) || "x";
  if (!used.has(root)) return root;
  for (let i = 2; i < 10_000; i += 1) {
    const id = `${root}-${i}`;
    if (!used.has(id)) return id;
  }
  return `${root}-${Date.now().toString(36)}`.slice(0, MAX_ID_LEN);
}

/** A short id for an element or a page, unique within `taken`. */
export function freshLocalId(prefix: string, taken: Iterable<string>): string {
  const used = new Set(taken);
  for (let i = 1; i < 100_000; i += 1) {
    const id = `${prefix}${i}`;
    if (!used.has(id)) return id;
  }
  return `${prefix}${Date.now().toString(36)}`;
}

function clip(s: string, max: number): string {
  return [...s].slice(0, max).join("");
}

function touch(def: DashDefinition): DashDefinition {
  return { ...def, updated_ms: Date.now() };
}

/**
 * A new dashboard from a model, in the format of the model or in `format` (ADR 0018): its pages
 * then get the grid that suits it (`adaptGrids`: nothing of the model moves).
 */
export function newDefinition(
  from: TemplateId,
  taken: Iterable<string>,
  lang: Lang,
  format?: DashFormat,
): DashDefinition {
  const t = template(from, lang);
  const def: DashDefinition = { ...t, id: freshId(t.name, taken) };
  if (!format) return touch(def);
  return touch(adaptGrids({ ...def, format: cleanFormat(format) }));
}

/** A copy under a new id and name (« Corde » included: its copy can be modified). */
export function duplicateDefinition(
  def: DashDefinition,
  taken: Iterable<string>,
  lang: Lang,
): DashDefinition {
  const src = def.id === BUILTIN_ID ? cordeDefinition(lang) : def;
  const name = clip(`${src.name} (${lang === "en" ? "copy" : "copie"})`, MAX_NAME_LEN);
  const copy: DashDefinition = structuredClone(src);
  // The copy of « Corde » adapts to the screen like the original; its whole pages can be moved.
  return touch({ ...copy, id: freshId(name, taken), name });
}

export function renameDefinition(def: DashDefinition, name: string): DashDefinition {
  const n = clip(name.trim(), MAX_NAME_LEN);
  return n && n !== def.name ? touch({ ...def, name: n }) : def;
}

// ---------------------------------------------------------------- pages

function withPage(
  def: DashDefinition,
  pageId: string,
  fn: (p: DashPageDef) => DashPageDef,
): DashDefinition {
  let changed = false;
  const pages = def.pages.map((p) => {
    if (p.id !== pageId) return p;
    const next = fn(p);
    if (next !== p) changed = true;
    return next;
  });
  return changed ? touch({ ...def, pages }) : def;
}

export function addPage(def: DashDefinition, name: string): DashDefinition {
  if (def.pages.length >= MAX_PAGES) return def;
  const last = def.pages[def.pages.length - 1];
  const page: DashPageDef = {
    id: freshLocalId(
      "p",
      def.pages.map((p) => p.id),
    ),
    name: clip(name.trim() || "Page", MAX_PAGE_NAME_LEN),
    cols: last?.cols ?? DEFAULT_COLS,
    rows: last?.rows ?? DEFAULT_ROWS,
    elements: [],
  };
  return touch({ ...def, pages: [...def.pages, page] });
}

export function renamePage(def: DashDefinition, pageId: string, name: string): DashDefinition {
  const n = clip(name.trim(), MAX_PAGE_NAME_LEN);
  if (!n) return def;
  return withPage(def, pageId, (p) => (p.name === n ? p : { ...p, name: n }));
}

/** Moves a page `delta` places (held within the pages). */
export function movePage(def: DashDefinition, pageId: string, delta: number): DashDefinition {
  const i = def.pages.findIndex((p) => p.id === pageId);
  const j = Math.max(0, Math.min(def.pages.length - 1, i + delta));
  if (i < 0 || i === j) return def;
  const pages = [...def.pages];
  const [p] = pages.splice(i, 1);
  pages.splice(j, 0, p as DashPageDef);
  return touch({ ...def, pages });
}

export function removePage(def: DashDefinition, pageId: string): DashDefinition {
  if (def.pages.length <= 1) return def;
  const pages = def.pages.filter((p) => p.id !== pageId);
  return pages.length === def.pages.length ? def : touch({ ...def, pages });
}

/** Changes the grid of a page; the elements are held within it (moved, then shrunk). */
export function setGrid(
  def: DashDefinition,
  pageId: string,
  cols: number,
  rows: number,
): DashDefinition {
  const c = Math.max(MIN_COLS, Math.min(MAX_COLS, Math.round(cols)));
  const r = Math.max(MIN_ROWS, Math.min(MAX_ROWS, Math.round(rows)));
  return withPage(def, pageId, (p) =>
    p.cols === c && p.rows === r
      ? p
      : { ...p, cols: c, rows: r, elements: p.elements.map((e) => clampRect(e, c, r)) },
  );
}

// ---------------------------------------------------------------- elements

export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

/** Held within the grid: the size first (at least one cell), then the place. */
export function clampRect<T extends Rect>(
  r: T,
  cols: number,
  rows: number,
  min = { w: 1, h: 1 },
): T {
  const w = Math.max(Math.min(min.w, cols), Math.min(cols, Math.round(r.w)));
  const h = Math.max(Math.min(min.h, rows), Math.min(rows, Math.round(r.h)));
  const x = Math.max(0, Math.min(cols - w, Math.round(r.x)));
  const y = Math.max(0, Math.min(rows - h, Math.round(r.y)));
  return { ...r, x, y, w, h };
}

function overlaps(a: Rect, b: Rect): boolean {
  return a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h;
}

/** The first free place for a `w × h` box, row by row; the top left corner when none is free. */
export function findFreeSpot(page: DashPageDef, w: number, h: number): { x: number; y: number } {
  const ww = Math.min(w, page.cols);
  const hh = Math.min(h, page.rows);
  for (let y = 0; y + hh <= page.rows; y += 1) {
    for (let x = 0; x + ww <= page.cols; x += 1) {
      const r = { x, y, w: ww, h: hh };
      if (!page.elements.some((e) => overlaps(r, e))) return { x, y };
    }
  }
  return { x: 0, y: 0 };
}

/** Adds an element of `type` (at `at`, else at the first free place); its id is returned too. */
export function addElement(
  def: DashDefinition,
  pageId: string,
  type: string,
  at?: { x: number; y: number },
  props: Record<string, unknown> = {},
): { def: DashDefinition; id: string | null } {
  const page = def.pages.find((p) => p.id === pageId);
  if (!page || page.elements.length >= MAX_ELEMENTS) return { def, id: null };
  const spec = getElementSpec(type);
  const size = spec?.size ?? { w: 4, h: 3 };
  const spot = at ?? findFreeSpot(page, size.w, size.h);
  const id = freshLocalId(
    "e",
    page.elements.map((e) => e.id),
  );
  const defaults = propsWithDefaults(type, {});
  delete defaults.scale;
  const el: DashElement = clampRect(
    { id, type, x: spot.x, y: spot.y, w: size.w, h: size.h, props: { ...defaults, ...props } },
    page.cols,
    page.rows,
    spec?.min,
  );
  if (spec?.defaultShow) el.show = { ...spec.defaultShow };
  return { def: withPage(def, pageId, (p) => ({ ...p, elements: [...p.elements, el] })), id };
}

export function updateElement(
  def: DashDefinition,
  pageId: string,
  id: string,
  patch: Partial<Rect>,
): DashDefinition {
  return withPage(def, pageId, (p) => {
    let changed = false;
    const elements = p.elements.map((e) => {
      if (e.id !== id) return e;
      const min = getElementSpec(e.type)?.min;
      const next = clampRect({ ...e, ...patch }, p.cols, p.rows, min);
      if (next.x === e.x && next.y === e.y && next.w === e.w && next.h === e.h) return e;
      changed = true;
      return next;
    });
    return changed ? { ...p, elements } : p;
  });
}

/** Moves by whole cells, held within the grid. */
export function moveElement(
  def: DashDefinition,
  pageId: string,
  id: string,
  dx: number,
  dy: number,
): DashDefinition {
  const e = def.pages.find((p) => p.id === pageId)?.elements.find((x) => x.id === id);
  return e ? updateElement(def, pageId, id, { x: e.x + dx, y: e.y + dy }) : def;
}

/** Resizes by whole cells, never under the element's minimum. */
export function resizeElement(
  def: DashDefinition,
  pageId: string,
  id: string,
  dw: number,
  dh: number,
): DashDefinition {
  const page = def.pages.find((p) => p.id === pageId);
  const e = page?.elements.find((x) => x.id === id);
  if (!page || !e) return def;
  const min = getElementSpec(e.type)?.min ?? { w: 1, h: 1 };
  const w = Math.max(min.w, Math.min(page.cols - e.x, e.w + dw));
  const h = Math.max(min.h, Math.min(page.rows - e.y, e.h + dh));
  return updateElement(def, pageId, id, { w, h });
}

export function setProps(
  def: DashDefinition,
  pageId: string,
  id: string,
  patch: Record<string, unknown>,
): DashDefinition {
  return withPage(def, pageId, (p) => ({
    ...p,
    elements: p.elements.map((e) => (e.id === id ? { ...e, props: { ...e.props, ...patch } } : e)),
  }));
}

export function removeElement(def: DashDefinition, pageId: string, id: string): DashDefinition {
  return withPage(def, pageId, (p) => {
    const elements = p.elements.filter((e) => e.id !== id);
    return elements.length === p.elements.length ? p : { ...p, elements };
  });
}

/** A copy next to the original (to its right, else below, else on it); its id is returned. */
export function duplicateElement(
  def: DashDefinition,
  pageId: string,
  id: string,
): { def: DashDefinition; id: string | null } {
  const page = def.pages.find((p) => p.id === pageId);
  const e = page?.elements.find((x) => x.id === id);
  if (!page || !e || page.elements.length >= MAX_ELEMENTS) return { def, id: null };
  const nid = freshLocalId(
    "e",
    page.elements.map((x) => x.id),
  );
  const right = e.x + 2 * e.w <= page.cols;
  const below = e.y + 2 * e.h <= page.rows;
  const copy: DashElement = {
    ...structuredClone(e),
    id: nid,
    x: right ? e.x + e.w : e.x,
    y: right ? e.y : below ? e.y + e.h : e.y,
  };
  return {
    def: withPage(def, pageId, (p) => ({ ...p, elements: [...p.elements, copy] })),
    id: nid,
  };
}

/** Brings an element to the front (drawn last) or sends it to the back. */
export function reorderElement(
  def: DashDefinition,
  pageId: string,
  id: string,
  where: "front" | "back",
): DashDefinition {
  return withPage(def, pageId, (p) => {
    const e = p.elements.find((x) => x.id === id);
    if (!e) return p;
    const rest = p.elements.filter((x) => x.id !== id);
    return { ...p, elements: where === "front" ? [...rest, e] : [e, ...rest] };
  });
}

// ---------------------------------------------------------------- format, theme

/**
 * Another format (`dashdef/format.ts` builds it: a preset, a size, a shape). The pages and their
 * elements are untouched: they are placed in cells, and drawn with the new proportions.
 */
export function setFormat(def: DashDefinition, format: DashFormat): DashDefinition {
  const next = cleanFormat(format);
  if (sameFormat(def.format, next)) return def;
  return touch({ ...def, format: next });
}

export function setPreset(def: DashDefinition, preset: PresetId | "custom"): DashDefinition {
  return setFormat(def, withPreset(def.format, preset));
}

/** A page with a finer grid: each column cut in `kc`, each row in `kr`, every element with it. */
function refinePage(p: DashPageDef, kc: number, kr: number): DashPageDef {
  return {
    ...p,
    cols: p.cols * kc,
    rows: p.rows * kr,
    elements: p.elements.map((e) => ({
      ...e,
      x: e.x * kc,
      y: e.y * kr,
      w: e.w * kc,
      h: e.h * kr,
    })),
  };
}

/**
 * Gives each page the grid that suits the format (`suggestGrid`): a finer one where elements
 * are placed (every element keeps exactly its place and its size), a fresh one on an empty
 * page. The same definition when every grid suits already.
 */
export function adaptGrids(def: DashDefinition): DashDefinition {
  let changed = false;
  const pages = def.pages.map((p) => {
    const g = suggestGrid(def.format, p);
    if (!g) return p;
    changed = true;
    if (p.elements.length === 0) return { ...p, cols: g.cols, rows: g.rows };
    return refinePage(p, g.cols / p.cols, g.rows / p.rows);
  });
  return changed ? touch({ ...def, pages }) : def;
}

export function setThemeColor(
  def: DashDefinition,
  key: keyof DashDefinition["theme"],
  color: string,
): DashDefinition {
  if (!/^#[0-9a-fA-F]{6}$/.test(color) || def.theme[key] === color) return def;
  return touch({ ...def, theme: { ...def.theme, [key]: color.toLowerCase() } });
}

export function setAuthor(def: DashDefinition, author: string): DashDefinition {
  const a = clip(author.trim(), MAX_NAME_LEN);
  const next = { ...def };
  if (a) next.author = a;
  else delete next.author;
  return touch(next);
}

// ---------------------------------------------------------------- style, conditions (ADR 0016)

function withElement(
  def: DashDefinition,
  pageId: string,
  id: string,
  fn: (e: DashElement) => DashElement,
): DashDefinition {
  return withPage(def, pageId, (p) => {
    let changed = false;
    const elements = p.elements.map((e) => {
      if (e.id !== id) return e;
      const next = fn(e);
      if (next !== e) changed = true;
      return next;
    });
    return changed ? { ...p, elements } : p;
  });
}

/** Drops the keys left `undefined`; `undefined` when nothing is left. */
function compact<T extends object>(o: T): T | undefined {
  const out = Object.fromEntries(Object.entries(o).filter(([, v]) => v !== undefined)) as T;
  return Object.keys(out).length ? out : undefined;
}

/** Changes the look of an element (a key `undefined`: back to the element's own). */
export function setStyle(
  def: DashDefinition,
  pageId: string,
  id: string,
  patch: Partial<Record<keyof DashStyle, number | string | undefined>>,
): DashDefinition {
  return withElement(def, pageId, id, (e) => {
    const style = compact({ ...(e.style ?? {}), ...patch } as DashStyle);
    const next: DashElement = { ...e };
    if (style) next.style = style;
    else delete next.style;
    return next;
  });
}

/** « Afficher quand… » (`null`: always). */
export function setShow(
  def: DashDefinition,
  pageId: string,
  id: string,
  show: DashCondition | null,
): DashDefinition {
  return withElement(def, pageId, id, (e) => {
    const next: DashElement = { ...e };
    if (show) next.show = { ...show };
    else delete next.show;
    return next;
  });
}

/** The colour rules of an element (at most 4; none: the key goes). */
export function setThresholds(
  def: DashDefinition,
  pageId: string,
  id: string,
  rules: DashThreshold[],
): DashDefinition {
  return withElement(def, pageId, id, (e) => {
    const next: DashElement = { ...e };
    if (rules.length) next.thresholds = rules.slice(0, 4).map((r) => ({ ...r }));
    else delete next.thresholds;
    return next;
  });
}

/** The background of a page (a key `undefined`: removed; nothing left: no background). */
export function setBackground(
  def: DashDefinition,
  pageId: string,
  patch: Partial<Record<keyof DashBackground, number | string | undefined>>,
): DashDefinition {
  return withPage(def, pageId, (p) => {
    const bg = compact({ ...(p.background ?? {}), ...patch } as DashBackground);
    const next: DashPageDef = { ...p };
    if (bg) next.background = bg;
    else delete next.background;
    return next;
  });
}

export function setFont(def: DashDefinition, font: FontId): DashDefinition {
  const cur = def.theme.font ?? "condensed";
  if (cur === font) return def;
  const theme = { ...def.theme };
  if (font === "condensed") delete theme.font;
  else theme.font = font;
  return touch({ ...def, theme });
}

// ---------------------------------------------------------------- several elements

/** Moves several elements by whole cells, together: held so that none leaves the grid. */
export function moveElements(
  def: DashDefinition,
  pageId: string,
  ids: readonly string[],
  dx: number,
  dy: number,
): DashDefinition {
  const page = def.pages.find((p) => p.id === pageId);
  if (!page) return def;
  const set = new Set(ids);
  const moving = page.elements.filter((e) => set.has(e.id));
  if (moving.length === 0) return def;
  const minX = Math.min(...moving.map((e) => e.x));
  const minY = Math.min(...moving.map((e) => e.y));
  const maxX = Math.max(...moving.map((e) => e.x + e.w));
  const maxY = Math.max(...moving.map((e) => e.y + e.h));
  const ddx = Math.max(-minX, Math.min(page.cols - maxX, Math.round(dx)));
  const ddy = Math.max(-minY, Math.min(page.rows - maxY, Math.round(dy)));
  if (ddx === 0 && ddy === 0) return def;
  return withPage(def, pageId, (p) => ({
    ...p,
    elements: p.elements.map((e) => (set.has(e.id) ? { ...e, x: e.x + ddx, y: e.y + ddy } : e)),
  }));
}

export function removeElements(
  def: DashDefinition,
  pageId: string,
  ids: readonly string[],
): DashDefinition {
  const set = new Set(ids);
  return withPage(def, pageId, (p) => {
    const elements = p.elements.filter((e) => !set.has(e.id));
    return elements.length === p.elements.length ? p : { ...p, elements };
  });
}

/**
 * Puts copies of `elements` (from this page or another) on a page, each with a new id: at the
 * same place when no element of this page sits exactly there, else one cell lower and to the
 * right (held in the grid). Returns the ids of the copies (within the 80 elements of a page).
 */
export function pasteElements(
  def: DashDefinition,
  pageId: string,
  elements: readonly DashElement[],
): { def: DashDefinition; ids: string[] } {
  const page = def.pages.find((p) => p.id === pageId);
  if (!page || elements.length === 0) return { def, ids: [] };
  const room = MAX_ELEMENTS - page.elements.length;
  const list = elements.slice(0, Math.max(0, room));
  if (list.length === 0) return { def, ids: [] };
  const taken = new Set(page.elements.map((e) => e.id));
  const free = list.every(
    (c) => !page.elements.some((e) => e.x === c.x && e.y === c.y && e.w === c.w && e.h === c.h),
  );
  const shift = free ? 0 : 1;
  const ids: string[] = [];
  const copies = list.map((c) => {
    const id = freshLocalId("e", taken);
    taken.add(id);
    ids.push(id);
    const min = getElementSpec(c.type)?.min;
    return clampRect(
      { ...structuredClone(c), id, x: c.x + shift, y: c.y + shift },
      page.cols,
      page.rows,
      min,
    );
  });
  return {
    def: withPage(def, pageId, (p) => ({ ...p, elements: [...p.elements, ...copies] })),
    ids,
  };
}

/** The ids of the elements that a rectangle of cells touches. */
export function elementsIn(page: DashPageDef, r: Rect): string[] {
  return page.elements.filter((e) => overlaps(r, e)).map((e) => e.id);
}

/**
 * The guides of a move: the columns and rows where an edge or the centre of the moving box
 * meets an edge or the centre of another element (or the middle of the page).
 */
export function alignGuides(
  page: DashPageDef,
  box: Rect,
  moving: ReadonlySet<string>,
): { x: number[]; y: number[] } {
  const others = page.elements.filter((e) => !moving.has(e.id));
  const marksX = new Set([
    page.cols / 2,
    ...others.flatMap((e) => [e.x, e.x + e.w / 2, e.x + e.w]),
  ]);
  const marksY = new Set([
    page.rows / 2,
    ...others.flatMap((e) => [e.y, e.y + e.h / 2, e.y + e.h]),
  ]);
  const xs = [box.x, box.x + box.w / 2, box.x + box.w].filter(
    (m) => marksX.has(m) && m > 0 && m < page.cols,
  );
  const ys = [box.y, box.y + box.h / 2, box.y + box.h].filter(
    (m) => marksY.has(m) && m > 0 && m < page.rows,
  );
  return { x: [...new Set(xs)], y: [...new Set(ys)] };
}
