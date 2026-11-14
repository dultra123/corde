import { create } from "zustand";
import { type DashDefinition, validateDefinition } from "../dashdef/model";

/**
 * The dashboard being edited in the Studio: the current state, what « Annuler » and
 * « Rétablir » go back to, the selection. Every change is also kept as a draft on this device
 * (`corde.studio.draft.<id>`), so that leaving the Studio without saving loses nothing.
 */

export const HISTORY_MAX = 100;
const DRAFT_PREFIX = "corde.studio.draft.";

export interface History {
  past: DashDefinition[];
  present: DashDefinition;
  future: DashDefinition[];
}

export function historyOf(def: DashDefinition): History {
  return { past: [], present: def, future: [] };
}

/** A change that can be undone (nothing when the definition did not change). */
export function commit(h: History, next: DashDefinition): History {
  if (next === h.present) return h;
  return { past: [...h.past, h.present].slice(-HISTORY_MAX), present: next, future: [] };
}

export function undo(h: History): History {
  const prev = h.past[h.past.length - 1];
  if (!prev) return h;
  return { past: h.past.slice(0, -1), present: prev, future: [h.present, ...h.future] };
}

export function redo(h: History): History {
  const next = h.future[0];
  if (!next) return h;
  return { past: [...h.past, h.present], present: next, future: h.future.slice(1) };
}

// ---------------------------------------------------------------- draft

export function readDraft(id: string): DashDefinition | null {
  try {
    const raw = localStorage.getItem(DRAFT_PREFIX + id);
    if (!raw) return null;
    const c = validateDefinition(JSON.parse(raw));
    return c.ok && c.def.id === id ? c.def : null;
  } catch {
    return null;
  }
}

export function writeDraft(def: DashDefinition): void {
  try {
    localStorage.setItem(DRAFT_PREFIX + def.id, JSON.stringify(def));
  } catch {
    // storage full or unavailable: no draft
  }
}

export function clearDraft(id: string): void {
  try {
    localStorage.removeItem(DRAFT_PREFIX + id);
  } catch {
    // nothing to clear
  }
}

// ---------------------------------------------------------------- store

interface EditorState {
  h: History | null;
  /** The definition as last saved on the PC (to know what is not saved yet). */
  saved: DashDefinition | null;
  pageId: string | null;
  selected: string | null;
  /** A drag in progress: its changes are one step of the history, committed at the end. */
  gesture: DashDefinition | null;
  open(def: DashDefinition, saved: DashDefinition | null): void;
  close(): void;
  apply(fn: (d: DashDefinition) => DashDefinition): void;
  /** Starts a gesture: the next `preview` calls replace the state without a history step. */
  begin(): void;
  preview(fn: (d: DashDefinition) => DashDefinition): void;
  end(): void;
  undo(): void;
  redo(): void;
  setPage(id: string): void;
  select(id: string | null): void;
  markSaved(def: DashDefinition): void;
}

function sync(h: History) {
  writeDraft(h.present);
  return h;
}

export const useEditor = create<EditorState>((set, get) => ({
  h: null,
  saved: null,
  pageId: null,
  selected: null,
  gesture: null,
  open(def, saved) {
    set({
      h: historyOf(def),
      saved,
      pageId: def.pages[0]?.id ?? null,
      selected: null,
      gesture: null,
    });
  },
  close() {
    set({ h: null, saved: null, pageId: null, selected: null, gesture: null });
  },
  apply(fn) {
    const h = get().h;
    if (!h) return;
    const next = fn(h.present);
    if (next === h.present) return;
    const nh = sync(commit(h, next));
    const page = next.pages.some((p) => p.id === get().pageId)
      ? get().pageId
      : (next.pages[0]?.id ?? null);
    set({ h: nh, pageId: page });
  },
  begin() {
    const h = get().h;
    if (h) set({ gesture: h.present });
  },
  preview(fn) {
    const h = get().h;
    if (!h) return;
    const next = fn(h.present);
    if (next !== h.present) set({ h: { ...h, present: next } });
  },
  end() {
    const { h, gesture } = get();
    set({ gesture: null });
    if (!h || !gesture || gesture === h.present) return;
    // One step: from the state before the gesture to the state after it.
    set({
      h: sync({ past: [...h.past, gesture].slice(-HISTORY_MAX), present: h.present, future: [] }),
    });
  },
  undo() {
    const h = get().h;
    if (!h) return;
    const nh = undo(h);
    if (nh === h) return;
    const page = nh.present.pages.some((p) => p.id === get().pageId)
      ? get().pageId
      : (nh.present.pages[0]?.id ?? null);
    set({ h: sync(nh), pageId: page, selected: null });
  },
  redo() {
    const h = get().h;
    if (!h) return;
    const nh = redo(h);
    if (nh === h) return;
    const page = nh.present.pages.some((p) => p.id === get().pageId)
      ? get().pageId
      : (nh.present.pages[0]?.id ?? null);
    set({ h: sync(nh), pageId: page, selected: null });
  },
  setPage(id) {
    set({ pageId: id, selected: null });
  },
  select(id) {
    set({ selected: id });
  },
  markSaved(def) {
    clearDraft(def.id);
    set({ saved: def });
  },
}));

/** The changes are not on the PC yet. */
export function isDirty(s: Pick<EditorState, "h" | "saved">): boolean {
  if (!s.h) return false;
  if (!s.saved) return true;
  return JSON.stringify(s.h.present) !== JSON.stringify(s.saved);
}
