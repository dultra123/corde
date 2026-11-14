import { create } from "zustand";
import { DASH_PAGES } from "./model";

/**
 * The page shown last on this device, by its id (a convenience: lost with the browser's
 * storage). The pages of « Corde » keep the names 1.1.0 remembered (`course`…).
 */
const KEY = "corde.dash.page";

function stored(): string | null {
  try {
    return localStorage.getItem(KEY);
  } catch {
    return null;
  }
}

function clampIndex(i: number, count: number): number {
  const n = Math.round(i);
  return Math.max(0, Math.min(Math.max(0, count - 1), Number.isFinite(n) ? n : 0));
}

interface DashPageState {
  index: number;
  /** The ids of the pages of the dashboard shown. */
  pages: readonly string[];
  /** A dashboard is shown: its pages; back to the page remembered when it has it. */
  setPages(pages: readonly string[]): void;
  /** Shows page `index` (held within the pages) and remembers it. */
  show(index: number): void;
  /** The next (`1`) or previous (`-1`) page. */
  step(delta: number): void;
}

export const useDashPage = create<DashPageState>((set, get) => ({
  index: Math.max(0, DASH_PAGES.indexOf(stored() as (typeof DASH_PAGES)[number])),
  pages: DASH_PAGES,
  setPages(pages) {
    const cur = get().pages;
    if (cur.length === pages.length && cur.every((p, i) => p === pages[i])) return;
    const want = pages.indexOf(stored() ?? "");
    set({ pages, index: want >= 0 ? want : clampIndex(get().index, pages.length) });
  },
  show(index) {
    const i = clampIndex(index, get().pages.length);
    if (i === get().index) return;
    set({ index: i });
    try {
      const id = get().pages[i];
      if (id) localStorage.setItem(KEY, id);
    } catch {
      // storage unavailable (private mode): the page is remembered for this visit only
    }
  },
  step(delta) {
    get().show(get().index + Math.round(delta));
  },
}));

interface DashFullState {
  /** The driver asked for the dashboard in full screen (PC, tablet). Not remembered. */
  full: boolean;
  setFull(full: boolean): void;
}

/**
 * Full screen on request (PC, tablet): the dashboard covers the window, and the browser's own
 * full screen is asked for when it allows it. Esc or the same button leave it.
 */
export const useDashFull = create<DashFullState>((set) => ({
  full: false,
  setFull(full) {
    set({ full });
    if (typeof document === "undefined") return;
    try {
      if (full && !document.fullscreenElement) {
        void document.documentElement.requestFullscreen?.()?.catch(() => {});
      } else if (!full && document.fullscreenElement) {
        void document.exitFullscreen?.()?.catch(() => {});
      }
    } catch {
      // no Fullscreen API (some webviews): the dashboard still covers the window
    }
  },
}));
