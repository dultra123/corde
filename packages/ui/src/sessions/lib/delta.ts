/**
 * Resampling and time-delta maths for lap comparison. Everything is pure and index based so it
 * can be unit tested without uPlot.
 */

export type Series = ArrayLike<number | null | undefined>;

/**
 * Indices `i` such that `xs[i]` is finite and strictly greater than the previous kept value:
 * the monotonic backbone needed for interpolation and for uPlot's x axis.
 */
export function monotonicIndices(xs: ArrayLike<number>): number[] {
  const out: number[] = [];
  let last = Number.NEGATIVE_INFINITY;
  for (let i = 0; i < xs.length; i += 1) {
    const x = xs[i];
    if (x === undefined || !Number.isFinite(x) || x <= last) continue;
    out.push(i);
    last = x;
  }
  return out;
}

/** `arr` restricted to `indices`. */
export function pick<T>(arr: ArrayLike<T>, indices: readonly number[]): T[] {
  const out = new Array<T>(indices.length);
  for (let k = 0; k < indices.length; k += 1) out[k] = arr[indices[k] as number] as T;
  return out;
}

/** Largest `i` with `xs[i] <= x` (−1 when `x` is before the first sample). `xs` ascending. */
export function lowerIndex(xs: ArrayLike<number>, x: number): number {
  let lo = 0;
  let hi = xs.length - 1;
  if (hi < 0 || x < (xs[0] as number)) return -1;
  if (x >= (xs[hi] as number)) return hi;
  while (hi - lo > 1) {
    const mid = (lo + hi) >> 1;
    if ((xs[mid] as number) <= x) lo = mid;
    else hi = mid;
  }
  return lo;
}

export type Interp = "linear" | "step";

/**
 * Value of the sampled function `(xs, ys)` at `x`. Linear between neighbours (or the previous
 * value for `step`), `null` outside the sampled range or next to a missing value. `xs` must be
 * strictly ascending (see `monotonicIndices`).
 */
export function interpolateAt(
  xs: ArrayLike<number>,
  ys: Series,
  x: number,
  mode: Interp = "linear",
): number | null {
  const n = xs.length;
  if (n === 0 || !Number.isFinite(x)) return null;
  const i = lowerIndex(xs, x);
  if (i < 0) return null;
  if (i >= n - 1 && x > (xs[n - 1] as number)) return null;
  const y0 = ys[i];
  if (y0 === null || y0 === undefined || !Number.isFinite(y0)) return null;
  if (i >= n - 1 || mode === "step") return y0;
  const x0 = xs[i] as number;
  const x1 = xs[i + 1] as number;
  if (x === x0) return y0;
  const y1 = ys[i + 1];
  if (y1 === null || y1 === undefined || !Number.isFinite(y1)) return null;
  const f = (x - x0) / (x1 - x0);
  return y0 + (y1 - y0) * f;
}

/** `(xs, ys)` resampled on `grid`. */
export function resample(
  xs: ArrayLike<number>,
  ys: Series,
  grid: ArrayLike<number>,
  mode: Interp = "linear",
): (number | null)[] {
  const out = new Array<number | null>(grid.length);
  for (let k = 0; k < grid.length; k += 1) out[k] = interpolateAt(xs, ys, grid[k] as number, mode);
  return out;
}

/**
 * Time at which the lap really started: `t[0]` extrapolated back to distance 0 using the first
 * two samples, so laps whose first sample lands a few metres after the line stay comparable.
 */
export function lapStartTime(dist: ArrayLike<number>, t: ArrayLike<number>): number {
  const t0 = t[0];
  if (t0 === undefined) return 0;
  const d0 = dist[0];
  const d1 = dist[1];
  const t1 = t[1];
  if (d0 === undefined || d1 === undefined || t1 === undefined || d0 <= 0 || d1 <= d0) return t0;
  const speed = (d1 - d0) / (t1 - t0); // m/s
  if (!(speed > 0)) return t0;
  return t0 - d0 / speed;
}

/** `t` rebased so the lap starts at 0 (see `lapStartTime`). */
export function lapClock(dist: ArrayLike<number>, t: ArrayLike<number>): number[] {
  const start = lapStartTime(dist, t);
  const out = new Array<number>(t.length);
  for (let i = 0; i < t.length; i += 1) out[i] = (t[i] as number) - start;
  return out;
}

export interface LapTrace {
  /** Strictly ascending distance along the lap, metres. */
  dist: ArrayLike<number>;
  /** Lap clock at each distance, seconds from the start of the lap. */
  t: ArrayLike<number>;
}

/**
 * Time delta A − B at each of A's sample distances: positive when A reached that point later
 * than B (A behind), negative when A is ahead. `null` where B has no sample.
 */
export function deltaTime(a: LapTrace, b: LapTrace): (number | null)[] {
  const out = new Array<number | null>(a.dist.length);
  for (let i = 0; i < a.dist.length; i += 1) {
    const tb = interpolateAt(b.dist, b.t, a.dist[i] as number, "linear");
    out[i] = tb === null ? null : (a.t[i] as number) - tb;
  }
  return out;
}

/**
 * Splits a signed series into its non-positive and non-negative halves for two-colour fills.
 * Each half is `null` where the other sign applies, except at a sign change where both halves
 * touch zero so the two strokes meet instead of leaving a hole.
 */
export function splitSign(values: readonly (number | null)[]): {
  neg: (number | null)[];
  pos: (number | null)[];
} {
  const n = values.length;
  const neg = new Array<number | null>(n).fill(null);
  const pos = new Array<number | null>(n).fill(null);
  let prev: number | null = null;
  for (let i = 0; i < n; i += 1) {
    const v = values[i];
    if (v === null || v === undefined || !Number.isFinite(v)) {
      prev = null;
      continue;
    }
    if (v <= 0) neg[i] = v;
    if (v >= 0) pos[i] = v;
    if (prev !== null && prev < 0 !== v < 0 && prev !== 0 && v !== 0) {
      // sign change between i-1 and i: close both halves at zero on the boundary samples
      if (v < 0) {
        neg[i - 1] = 0;
        pos[i] = 0;
      } else {
        pos[i - 1] = 0;
        neg[i] = 0;
      }
    }
    prev = v;
  }
  return { neg, pos };
}
