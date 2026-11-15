import {
  type DashFormat,
  type DashPageDef,
  DEFAULT_COLS,
  MAX_COLS,
  MAX_FORMAT_RATIO,
  MAX_FORMAT_SIDE,
  MAX_ROWS,
  MIN_COLS,
  MIN_FORMAT_SIDE,
  MIN_ROWS,
  maxShapeRadius,
  type PresetId,
  presetFormat,
  radiusOf,
  type ShapeId,
  shapeOf,
} from "./model";

/**
 * The format of a dashboard (ADR 0018), without React: its size, its shape, and what follows
 * from them — the outline that is drawn, the grid that suits it. Every function returns a
 * format within the rules of `model.ts` (or the same one when nothing changes): the Studio
 * edits a format with these, on a dashboard being edited as on one about to be created.
 */

/** Only the keys of a format: no shape for a rectangle, a radius only with rounded corners. */
export function cleanFormat(f: DashFormat): DashFormat {
  const out: DashFormat = { preset: f.preset, width: f.width, height: f.height };
  const shape = shapeOf(f);
  if (shape === "round") out.shape = "round";
  else if (shape === "rounded") {
    out.shape = "rounded";
    out.radius = Math.max(1, radiusOf(f));
  }
  return out;
}

export function sameFormat(a: DashFormat, b: DashFormat): boolean {
  return (
    a.preset === b.preset &&
    a.width === b.width &&
    a.height === b.height &&
    shapeOf(a) === shapeOf(b) &&
    radiusOf(a) === radiusOf(b)
  );
}

/** The proportions are within the bounds (8:1 at most, either way). */
export function ratioAllowed(width: number, height: number): boolean {
  const r = width / height;
  return r >= 1 / MAX_FORMAT_RATIO && r <= MAX_FORMAT_RATIO;
}

/** A side held within its bounds, a whole number. */
export function clampSide(v: number): number {
  return Math.max(MIN_FORMAT_SIDE, Math.min(MAX_FORMAT_SIDE, Math.round(v)));
}

/** A preset: its size and its shape. `custom`: the same size and shape, free to change. */
export function withPreset(f: DashFormat, preset: PresetId | "custom"): DashFormat {
  if (preset !== "custom") return f.preset === preset ? f : presetFormat(preset);
  if (f.preset === "custom") return f;
  // « Tout l'écran » has no shape of its own: its size is where a custom format starts.
  return cleanFormat({ ...f, preset: "custom" });
}

/**
 * Another size (a custom format, the shape kept, the radius held within the new size). The
 * same format when the size is out of bounds: sides from 100 to 4000, 8:1 at most.
 */
export function withSize(f: DashFormat, width: number, height: number): DashFormat {
  const w = Math.round(width);
  const h = Math.round(height);
  if (w !== clampSide(w) || h !== clampSide(h) || !ratioAllowed(w, h)) return f;
  if (w === f.width && h === f.height) return f;
  const shape = f.preset === "fill" ? "rect" : shapeOf(f);
  return cleanFormat({ ...f, preset: "custom", width: w, height: h, shape });
}

/** Width and height exchanged (landscape ↔ portrait). The same format for a square or `fill`. */
export function swapped(f: DashFormat): DashFormat {
  if (f.preset === "fill") return f;
  return withSize(f, f.height, f.width);
}

/** The radius a `rounded` page starts with: a twelfth of its short side. */
export function defaultRadius(width: number, height: number): number {
  return Math.max(
    1,
    Math.min(maxShapeRadius(width, height), Math.round(Math.min(width, height) / 12)),
  );
}

/** Another shape (a custom format). `fill` has none: the same format. */
export function withShape(f: DashFormat, shape: ShapeId): DashFormat {
  if (f.preset === "fill" || shapeOf(f) === shape) return f;
  const next: DashFormat = { preset: "custom", width: f.width, height: f.height };
  if (shape === "round") next.shape = "round";
  else if (shape === "rounded") {
    next.shape = "rounded";
    next.radius = defaultRadius(f.width, f.height);
  }
  return next;
}

/** Another corner radius, held from 1 to half the short side (only with rounded corners). */
export function withRadius(f: DashFormat, radius: number): DashFormat {
  if (shapeOf(f) !== "rounded" || !Number.isFinite(radius)) return f;
  const r = Math.max(1, Math.min(maxShapeRadius(f.width, f.height), Math.round(radius)));
  if (r === f.radius) return f;
  return { preset: "custom", width: f.width, height: f.height, shape: "rounded", radius: r };
}

// ---------------------------------------------------------------- drawing

/**
 * The `border-radius` of the shape, in percent of the drawing (`a% / b%`: the same radius on
 * both axes whatever the size it is drawn at), `null` for a rectangle.
 */
export function shapeRadiusCss(f: DashFormat): string | null {
  const shape = shapeOf(f);
  if (shape === "round") return "50%";
  if (shape !== "rounded") return null;
  const r = radiusOf(f);
  if (r <= 0) return null;
  const pct = (v: number) => `${Number(((r / v) * 100).toFixed(3))}%`;
  return `${pct(f.width)} / ${pct(f.height)}`;
}

/** The outline of the shape as an SVG path, in the pixels of the format (0,0 → width,height). */
export function shapePath(f: DashFormat): string {
  const { width: w, height: h } = f;
  const shape = shapeOf(f);
  if (shape === "round") {
    const rx = w / 2;
    const ry = h / 2;
    return `M${rx} 0A${rx} ${ry} 0 1 1 ${rx} ${h}A${rx} ${ry} 0 1 1 ${rx} 0Z`;
  }
  const r = shape === "rounded" ? radiusOf(f) : 0;
  if (r <= 0) return `M0 0H${w}V${h}H0Z`;
  return (
    `M${r} 0H${w - r}A${r} ${r} 0 0 1 ${w} ${r}V${h - r}A${r} ${r} 0 0 1 ${w - r} ${h}` +
    `H${r}A${r} ${r} 0 0 1 0 ${h - r}V${r}A${r} ${r} 0 0 1 ${r} 0Z`
  );
}

/** Whether a point of the format (in its pixels) is inside its shape. */
export function insideShape(f: DashFormat, x: number, y: number): boolean {
  const { width: w, height: h } = f;
  if (x < 0 || y < 0 || x > w || y > h) return false;
  const shape = shapeOf(f);
  if (shape === "round") {
    const dx = (x - w / 2) / (w / 2);
    const dy = (y - h / 2) / (h / 2);
    return dx * dx + dy * dy <= 1;
  }
  const r = shape === "rounded" ? radiusOf(f) : 0;
  if (r <= 0) return true;
  // Only the four corner squares can be outside: there, the distance to the corner's centre.
  const cx = Math.max(r, Math.min(w - r, x));
  const cy = Math.max(r, Math.min(h - r, y));
  return (x - cx) ** 2 + (y - cy) ** 2 <= r * r;
}

// ---------------------------------------------------------------- grid

/**
 * Elements are placed in cells, so a page keeps its layout when the format changes: it is
 * drawn with the new proportions. The cells follow: on a square page a 24 × 12 grid has cells
 * twice as tall as wide. Above this width / height (or under its inverse) the cells are no
 * longer « roughly square » and the Studio offers a grid that suits the format.
 */
export const CELL_RATIO_MAX = 1.6;

type Grid = Pick<DashPageDef, "cols" | "rows">;

/** Width over height of one cell of a page drawn in this format (1 for `fill`: no size). */
export function cellRatio(f: DashFormat, page: Grid): number {
  if (f.preset === "fill") return 1;
  return f.width / page.cols / (f.height / page.rows);
}

export function cellsRoughlySquare(f: DashFormat, page: Grid): boolean {
  const r = cellRatio(f, page);
  return r <= CELL_RATIO_MAX && r >= 1 / CELL_RATIO_MAX;
}

/** The grid of an empty page of this format: 24 columns (fewer on a tall page), square cells. */
export function gridFor(f: DashFormat): Grid {
  const ratio = f.preset === "fill" ? 2 : f.width / f.height;
  let cols = DEFAULT_COLS;
  let rows = Math.round(cols / ratio);
  if (rows > MAX_ROWS) {
    rows = MAX_ROWS;
    cols = Math.round(rows * ratio);
  }
  return {
    cols: Math.max(MIN_COLS, Math.min(MAX_COLS, cols)),
    rows: Math.max(MIN_ROWS, Math.min(MAX_ROWS, rows)),
  };
}

/**
 * A grid that suits the format better than the page's, `null` when its cells are roughly
 * square already (or nothing better exists).
 *
 * - An empty page takes the grid of a new page of this format.
 * - A page with elements only gets a **finer** grid: its columns or its rows cut in 2, 3 or 4,
 *   so that every element keeps exactly its place and its size (its cells are multiplied the
 *   same way). Nothing is ever moved, shrunk or rounded.
 */
export function suggestGrid(f: DashFormat, page: DashPageDef): Grid | null {
  if (f.preset === "fill" || cellsRoughlySquare(f, page)) return null;
  if (page.elements.length === 0) {
    const g = gridFor(f);
    return g.cols === page.cols && g.rows === page.rows ? null : g;
  }
  const r = cellRatio(f, page);
  const off = (x: number) => Math.abs(Math.log(x));
  let best: Grid | null = null;
  let bestOff = off(r);
  for (let k = 2; k <= 4; k += 1) {
    // Cells too wide: more columns. Too tall: more rows.
    const g =
      r > 1 ? { cols: page.cols * k, rows: page.rows } : { cols: page.cols, rows: page.rows * k };
    if (g.cols > MAX_COLS || g.rows > MAX_ROWS) break;
    const o = off(cellRatio(f, g));
    if (o < bestOff - 1e-9) {
      best = g;
      bestOff = o;
    }
  }
  return best;
}
