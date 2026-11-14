import { create } from "zustand";
import { DASH_PAGES, pageIndex, stepPage } from "./model";

/** The page shown last on this device (a convenience: lost with the browser's storage). */
const KEY = "corde.dash.page";

function load(): number {
  try {
    return pageIndex(localStorage.getItem(KEY));
  } catch {
    return 0;
  }
}

interface DashPageState {
  index: number;
  /** Shows page `index` (held within the four) and remembers it. */
  show(index: number): void;
  /** The next (`1`) or previous (`-1`) page. */
  step(delta: number): void;
}

export const useDashPage = create<DashPageState>((set, get) => ({
  index: load(),
  show(index) {
    const i = stepPage(index, 0);
    if (i === get().index) return;
    set({ index: i });
    try {
      localStorage.setItem(KEY, DASH_PAGES[i] ?? DASH_PAGES[0]);
    } catch {
      // storage unavailable (private mode): the page is remembered for this visit only
    }
  },
  step(delta) {
    get().show(stepPage(get().index, delta));
  },
}));
