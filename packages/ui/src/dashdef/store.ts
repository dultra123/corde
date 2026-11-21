import type { GameId } from "@corde/protocol";
import { useMemo } from "react";
import { create } from "zustand";
import { authHeaders } from "../net/auth";
import { useSettings } from "../store/settings";
import { useTelemetry } from "../store/telemetry";
import { cordeDefinition } from "./builtin";
import {
  BUILTIN_ID,
  byteSize,
  type DashDefinition,
  MAX_DASH_BYTES,
  validateDefinition,
} from "./model";

/**
 * The dashboards made in the Studio. They live on the PC (`/api/dashes`, ADR 0015); every device
 * keeps a bounded copy of the last list it received (`localStorage`), so that the phone keeps
 * showing its dashboard when the PC is off, and remembers which one it uses. Every definition,
 * from the server or from the copy, goes through `validateDefinition` first.
 */

const CACHE_KEY = "corde.dashes.cache.v1";
const SELECTED_KEY = "corde.dashes.selected";
/** The copy kept on a device: at most this many definitions… */
export const CACHE_MAX = 20;
/** …and this many bytes. */
export const CACHE_MAX_BYTES = 2 * 1024 * 1024;

export type DashSync = "idle" | "loading" | "ready" | "offline";

/** Keeps the valid definitions, most recent first, within the bounds of the copy. */
export function boundCache(defs: unknown[]): DashDefinition[] {
  const valid: DashDefinition[] = [];
  for (const d of defs) {
    const c = validateDefinition(d);
    if (c.ok && c.def.id !== BUILTIN_ID && !valid.some((v) => v.id === c.def.id)) valid.push(c.def);
  }
  valid.sort((a, b) => b.updated_ms - a.updated_ms);
  const out: DashDefinition[] = [];
  let bytes = 0;
  for (const d of valid) {
    if (out.length >= CACHE_MAX) break;
    const n = byteSize(d);
    if (bytes + n > CACHE_MAX_BYTES) continue;
    bytes += n;
    out.push(d);
  }
  return out;
}

function loadCache(): DashDefinition[] {
  try {
    const raw = localStorage.getItem(CACHE_KEY);
    if (!raw) return [];
    const parsed: unknown = JSON.parse(raw);
    return Array.isArray(parsed) ? boundCache(parsed) : [];
  } catch {
    return [];
  }
}

function saveCache(defs: DashDefinition[]): void {
  try {
    localStorage.setItem(CACHE_KEY, JSON.stringify(boundCache(defs)));
  } catch {
    // storage unavailable or full: the list lives for this visit only
  }
}

function loadSelected(): string {
  try {
    return localStorage.getItem(SELECTED_KEY) ?? BUILTIN_ID;
  } catch {
    return BUILTIN_ID;
  }
}

interface DashesState {
  /** The dashboards of the PC (the copy until the PC answers). */
  defs: DashDefinition[];
  /** The one this device uses (`corde` when none or unknown). */
  selected: string;
  sync: DashSync;
  base: string | null;
  select(id: string): void;
  setFromServer(defs: unknown[]): void;
  /** Local change after a save or a delete that the server accepted. */
  put(def: DashDefinition): void;
  drop(id: string): void;
}

export const useDashes = create<DashesState>((set, get) => ({
  defs: loadCache(),
  selected: loadSelected(),
  sync: "idle",
  base: null,
  select(id) {
    set({ selected: id });
    try {
      localStorage.setItem(SELECTED_KEY, id);
    } catch {
      // remembered for this visit only
    }
  },
  setFromServer(defs) {
    const clean = boundCache(defs);
    set({ defs: clean, sync: "ready" });
    saveCache(clean);
  },
  put(def) {
    const defs = [def, ...get().defs.filter((d) => d.id !== def.id)];
    set({ defs });
    saveCache(defs);
  },
  drop(id) {
    const defs = get().defs.filter((d) => d.id !== id);
    set({ defs, selected: get().selected === id ? BUILTIN_ID : get().selected });
    saveCache(defs);
  },
}));

/**
 * The dashboard to show: the selected one when it is known and valid, else « Corde » (its page
 * names in the language of the interface).
 */
export function resolveDash(
  defs: readonly DashDefinition[],
  selected: string,
  lang: "fr" | "en",
  game?: GameId | null,
): DashDefinition {
  if (selected !== BUILTIN_ID) {
    const d = defs.find((x) => x.id === selected);
    if (d && validateDefinition(d).ok) return d;
  }
  // « Corde » follows the game: only the pages it publishes something for.
  return cordeDefinition(lang, game);
}

export function useActiveDash(): DashDefinition {
  const defs = useDashes((s) => s.defs);
  const selected = useDashes((s) => s.selected);
  const lang = useSettings((s) => s.lang);
  // The game being read (a demonstration plays every page).
  const game = useTelemetry((s) =>
    s.demo === "off" && s.status?.connected ? s.status.game : null,
  );
  return useMemo(() => resolveDash(defs, selected, lang, game), [defs, selected, lang, game]);
}

// ---------------------------------------------------------------- server

async function call(
  base: string,
  path: string,
  init: RequestInit = {},
  timeoutMs = 5000,
): Promise<Response> {
  const ctrl = new AbortController();
  const timer = setTimeout(() => ctrl.abort(), timeoutMs);
  try {
    return await fetch(`${base}${path}`, {
      ...init,
      signal: ctrl.signal,
      headers: { Accept: "application/json", ...authHeaders(), ...(init.headers ?? {}) },
    });
  } finally {
    clearTimeout(timer);
  }
}

/** Called once a server said hello: reads its dashboards (the copy stays when it cannot). */
export async function syncDashesWithServer(base: string): Promise<void> {
  useDashes.setState({ base, sync: "loading" });
  try {
    const res = await call(base, "/api/dashes");
    if (!res.ok) throw new Error(String(res.status));
    const json: unknown = await res.json();
    if (!Array.isArray(json)) throw new Error("not a list");
    useDashes.getState().setFromServer(json);
  } catch {
    useDashes.setState({ sync: "offline" });
  }
}

export type SaveResult =
  | { ok: true }
  | { ok: false; reason: "offline" | "refused" | "full" | "pc-only" };

/** Stores a definition on the PC (validated here first, then by the server). */
export async function saveDash(def: DashDefinition): Promise<SaveResult> {
  const base = useDashes.getState().base;
  const checked = validateDefinition(def);
  if (!checked.ok || checked.def.id === BUILTIN_ID || byteSize(checked.def) > MAX_DASH_BYTES)
    return { ok: false, reason: "refused" };
  if (base === null) return { ok: false, reason: "offline" };
  try {
    const res = await call(base, `/api/dashes/${encodeURIComponent(checked.def.id)}`, {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(checked.def),
    });
    if (res.status === 409) return { ok: false, reason: "full" };
    if (!res.ok) return { ok: false, reason: "refused" };
    useDashes.getState().put(checked.def);
    return { ok: true };
  } catch {
    return { ok: false, reason: "offline" };
  }
}

/** Deletes a definition on the PC (allowed from the PC itself only, like the layouts). */
export async function deleteDash(id: string): Promise<SaveResult> {
  const base = useDashes.getState().base;
  if (base === null) return { ok: false, reason: "offline" };
  try {
    const res = await call(base, `/api/dashes/${encodeURIComponent(id)}`, { method: "DELETE" });
    if (res.status === 403) return { ok: false, reason: "pc-only" };
    if (!res.ok && res.status !== 404) return { ok: false, reason: "refused" };
    useDashes.getState().drop(id);
    return { ok: true };
  } catch {
    return { ok: false, reason: "offline" };
  }
}
