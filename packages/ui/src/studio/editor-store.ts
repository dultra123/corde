import { create } from "zustand";
import { type DashDefinition, type DashElement, validateDefinition } from "../dashdef/model";

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
  /** The element the inspector shows: the last one chosen (`null`: none, or several). */
  selected: string | null;
  /** Every element chosen (Maj + clic, a frame drawn on the page). */
  selection: string[];
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
  /** Adds or takes away one element of the selection (Maj + clic). */
  toggle(id: string): void;
  selectMany(ids: string[]): void;
  markSaved(def: DashDefinition): void;
  /** Back to the version saved on the PC; the draft is dropped. */
  discardDraft(): void;
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
  selection: [],
  gesture: null,
  open(def, saved) {
    set({
      h: historyOf(def),
      saved,
      pageId: def.pages[0]?.id ?? null,
      selected: null,
      selection: [],
      gesture: null,
    });
  },
  close() {
    set({ h: null, saved: null, pageId: null, selected: null, selection: [], gesture: null });
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
    // Elements that are gone leave the selection.
    const ids = new Set(next.pages.find((p) => p.id === page)?.elements.map((e) => e.id) ?? []);
    const selection = get().selection.filter((id) => ids.has(id));
    const selected =
      get().selected !== null && ids.has(get().selected as string) ? get().selected : null;
    set({ h: nh, pageId: page, selection, selected: selection.length > 1 ? null : selected });
  },
  begin() {
    // A gesture still open (a field being typed in when a drag starts): it ends here, as a
    // step of its own, so that nothing it changed is lost from the history.
    if (get().gesture) get().end();
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
    set({ h: sync(nh), pageId: page, selected: null, selection: [] });
  },
  redo() {
    const h = get().h;
    if (!h) return;
    const nh = redo(h);
    if (nh === h) return;
    const page = nh.present.pages.some((p) => p.id === get().pageId)
      ? get().pageId
      : (nh.present.pages[0]?.id ?? null);
    set({ h: sync(nh), pageId: page, selected: null, selection: [] });
  },
  setPage(id) {
    set({ pageId: id, selected: null, selection: [] });
  },
  select(id) {
    set({ selected: id, selection: id ? [id] : [] });
  },
  toggle(id) {
    const cur = get().selection;
    const selection = cur.includes(id) ? cur.filter((x) => x !== id) : [...cur, id];
    set({ selection, selected: selection.length === 1 ? (selection[0] as string) : null });
  },
  selectMany(ids) {
    const selection = [...new Set(ids)];
    set({ selection, selected: selection.length === 1 ? (selection[0] as string) : null });
  },
  markSaved(def) {
    clearDraft(def.id);
    set({ saved: def });
  },
  discardDraft() {
    const { saved, h } = get();
    if (!saved || !h) return;
    clearDraft(saved.id);
    const pageId = saved.pages.some((p) => p.id === get().pageId)
      ? get().pageId
      : (saved.pages[0]?.id ?? null);
    set({ h: historyOf(saved), pageId, selected: null, selection: [], gesture: null });
  },
}));

// ---------------------------------------------------------------- clipboard

/**
 * The elements copied with Ctrl + C: kept for this visit of the Studio, pasted on any page of
 * any dashboard (Ctrl + V), each copy with a new id.
 */
let clipboard: DashElement[] = [];

export function copyElements(elements: readonly DashElement[]): void {
  clipboard = elements.map((e) => structuredClone(e));
}

export function copiedElements(): DashElement[] {
  return clipboard.map((e) => structuredClone(e));
}

/** The changes are not on the PC yet. */
export function isDirty(s: Pick<EditorState, "h" | "saved">): boolean {
  if (!s.h) return false;
  if (!s.saved) return true;
  return JSON.stringify(s.h.present) !== JSON.stringify(s.saved);
}
