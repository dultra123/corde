import type { RacePick } from "../net/races-api";

/**
 * « Mes courses » shared between devices (ADR 0019): the rule that merges two copies of the
 * picks. The same rule as the server's (`crates/corde-server/src/races/merge.rs`), checked
 * against the same cases (`merge-vectors.json`, next to the server's rule):
 *
 * - for one identifier, **the latest change wins**; at the same instant a removal wins over a
 *   pick, and between two picks a fixed order of their fields decides;
 * - what started more than three hours ago is dropped, picks and removals;
 * - at most 50 picks (the earliest starts) and 200 removals (the latest ones).
 *
 * The order of the two copies does not matter and merging twice changes nothing.
 */
export const KEEP_AFTER_START_MS = 3 * 3_600_000;
export const MAX_PICKS = 50;
export const MAX_REMOVED = 200;

/** A pick and the instant of its last change (0: picked before the picks were shared). */
export interface StampedPick {
  pick: RacePick;
  changed_ms: number;
}

/** A pick that was removed, and when. */
export interface RemovedPick {
  id: string;
  removed_ms: number;
}

/** One copy of « Mes courses »: what `POST /api/races/picks/sync` receives and answers. */
export interface PickSet {
  /** By start, then identifier. */
  picks: StampedPick[];
  /** By identifier. */
  removed: RemovedPick[];
}

export const emptyPickSet = (): PickSet => ({ picks: [], removed: [] });

/** The start written in a pick identifier (`<race_id>@<start_ms>`). */
export function startOf(id: string): number | null {
  const at = id.lastIndexOf("@");
  if (at < 0) return null;
  const digits = id.slice(at + 1);
  return /^\d{1,14}$/.test(digits) ? Number(digits) : null;
}

/**
 * Compares two texts by code point, like the server (which compares UTF-8 bytes): the `<` of
 * JavaScript compares UTF-16 units, and puts an emoji before « ～ ».
 */
export function compareText(a: string, b: string): number {
  let i = 0;
  let j = 0;
  while (i < a.length && j < b.length) {
    const x = a.codePointAt(i) ?? 0;
    const y = b.codePointAt(j) ?? 0;
    if (x !== y) return x < y ? -1 : 1;
    i += x > 0xffff ? 2 : 1;
    j += y > 0xffff ? 2 : 1;
  }
  return Math.sign(a.length - i - (b.length - j));
}

/** The order that decides between two picks changed at the same instant. */
function compareTie(a: RacePick, b: RacePick): number {
  return (
    compareText(a.name, b.name) ||
    compareText(a.track, b.track) ||
    compareText(a.duration, b.duration) ||
    compareText(a.image ?? "", b.image ?? "") ||
    compareText(a.cat, b.cat) ||
    compareText(a.src, b.src) ||
    compareText(a.level ?? "", b.level ?? "") ||
    Math.sign((a.eve_ms ?? -1) - (b.eve_ms ?? -1))
  );
}

type Entry = { live: StampedPick } | { gone: number };

const stampOf = (e: Entry): number => ("live" in e ? e.live.changed_ms : e.gone);

/** `next` replaces `old` for the same identifier. */
function wins(next: Entry, old: Entry): boolean {
  const a = stampOf(next);
  const b = stampOf(old);
  if (a !== b) return a > b;
  if ("gone" in next) return "live" in old;
  return "live" in old && compareTie(next.live.pick, old.live.pick) > 0;
}

function fold(into: Map<string, Entry>, set: PickSet): void {
  const offer = (id: string, e: Entry) => {
    const old = into.get(id);
    if (!old || wins(e, old)) into.set(id, e);
  };
  for (const s of set.picks) offer(s.pick.id, { live: s });
  for (const r of set.removed) offer(r.id, { gone: r.removed_ms });
}

/** The two copies as one, as it stands at `now`. Neither copy is modified. */
export function mergePickSets(a: PickSet, b: PickSet, now: number): PickSet {
  const all = new Map<string, Entry>();
  fold(all, a);
  fold(all, b);
  const limit = now - KEEP_AFTER_START_MS;
  let picks: StampedPick[] = [];
  let removed: RemovedPick[] = [];
  for (const [id, e] of all) {
    if ("live" in e) {
      if (e.live.pick.start_ms > limit) picks.push(e.live);
    } else {
      const start = startOf(id);
      if (start !== null && start > limit) removed.push({ id, removed_ms: e.gone });
    }
  }
  picks.sort((x, y) => x.pick.start_ms - y.pick.start_ms || compareText(x.pick.id, y.pick.id));
  picks = picks.slice(0, MAX_PICKS);
  if (removed.length > MAX_REMOVED) {
    removed.sort((x, y) => y.removed_ms - x.removed_ms || compareText(x.id, y.id));
    removed = removed.slice(0, MAX_REMOVED);
  }
  removed.sort((x, y) => compareText(x.id, y.id));
  return { picks, removed };
}

/** Two copies hold the same picks, removals and instants (both as {@link mergePickSets} gives them). */
export function samePickSet(a: PickSet, b: PickSet): boolean {
  return JSON.stringify(canonical(a)) === JSON.stringify(canonical(b));
}

function canonical(set: PickSet): unknown {
  return {
    picks: set.picks.map((s) => [
      s.pick.id,
      s.changed_ms,
      s.pick.race_id,
      s.pick.cat,
      s.pick.src,
      s.pick.name,
      s.pick.level ?? null,
      s.pick.track,
      s.pick.duration,
      s.pick.image ?? null,
      s.pick.start_ms,
      s.pick.eve_ms ?? null,
    ]),
    removed: set.removed.map((r) => [r.id, r.removed_ms]),
  };
}
