import { flagsOf } from "../dash/model";
import { type FieldCtx, getField } from "./fields";
import type { DashCondition, DashThreshold, Op } from "./model";

/**
 * « Afficher quand… » and the colour rules of an element (ADR 0016): facts of the frame from a
 * closed list, or a field of the catalogue compared with a number. Nothing here is a formula:
 * the definition only picks among these.
 */

export function compare(op: Op | string, v: number, x: number): boolean {
  switch (op) {
    case "lt":
      return v < x;
    case "le":
      return v <= x;
    case "ge":
      return v >= x;
    case "gt":
      return v > x;
    default:
      return false;
  }
}

/** A value that can be compared: a finite number, or a yes / no as 1 / 0. */
export function numeric(v: unknown): number | null {
  if (typeof v === "number") return Number.isFinite(v) ? v : null;
  if (typeof v === "boolean") return v ? 1 : 0;
  return null;
}

/** The colour of the first rule that holds for `v`, `null` when none does. */
export function thresholdColor(
  rules: readonly DashThreshold[] | undefined,
  v: unknown,
): string | null {
  const n = numeric(v);
  if (!rules || n === null) return null;
  for (const r of rules) if (compare(r.op, n, r.value)) return r.color;
  return null;
}

/** Whether an element shows now (`undefined`: always). */
export function conditionHolds(c: DashCondition | undefined, ctx: FieldCtx): boolean {
  if (!c) return true;
  const frame = ctx.frame;
  const p = frame?.player ?? null;
  switch (c.when) {
    case "limiter":
      return p?.engine.speed_limiter === true;
    case "pits":
      return !!p && (p.in_pits || p.in_garage || p.lap.in_pit_lane);
    case "yellow":
    case "blue": {
      if (!frame) return false;
      const f = flagsOf(frame.session, p, frame.standings);
      return c.when === "yellow" ? f.yellow : f.blue;
    }
    case "field": {
      const f = getField(c.field);
      if (!f || c.op === undefined || c.value === undefined) return false;
      const n = numeric(f.read(ctx));
      return n !== null && compare(c.op, n, c.value);
    }
    default:
      return false;
  }
}
