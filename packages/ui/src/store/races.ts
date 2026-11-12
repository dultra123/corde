import type { RaceReminder } from "@corde/protocol";
import { create } from "zustand";
import type { RacePick, RaceSettings, RaceSources } from "../net/races-api";

/**
 * State of the « Courses » feature shared by every page: the reminders waiting in the banner
 * (pushed by `net/connection` on `race_reminder`), the driver's picks (read from the server
 * by `races/usePicks`, shown on the Courses page and on the home page), and the schedule pages
 * and race settings read by `races/useRaceSources` / `useRaceSettings` — one copy for both
 * pages, so that going from one to the other does not ask the server again.
 */
export interface ShownReminder extends RaceReminder {
  /** `<pick_id>#<kind>[-<lead>]`, unique per reminder. */
  key: string;
  received_ms: number;
  /** The phone already rang it (notification of the iPhone application): no second sound. */
  quiet?: boolean;
}

interface RacesState {
  /** Newest last; at most three. */
  reminders: ShownReminder[];
  /**
   * The server's `race_reminder` messages are shown (the browser, the desktop application);
   * `false` in the iPhone application, which rings its own picks.
   */
  serverReminders: boolean;
  picks: RacePick[] | null;
  /** Bumped when the picks changed somewhere (reminder received, pick toggled). */
  picksRevision: number;
  /** `GET /api/races/sources`, the last answer. */
  sources: RaceSources | null;
  /** When it was received (`Date.now()`), 0 before. */
  sourcesAt: number;
  sourcesError: string | null;
  sourcesLoading: boolean;
  /** `GET /api/races/settings`, the last answer (or an optimistic update of it). */
  settings: RaceSettings | null;
  settingsAt: number;
  pushReminder(r: RaceReminder, quiet?: boolean): void;
  dismiss(key: string): void;
  setPicks(p: RacePick[]): void;
  bumpPicks(): void;
  update(patch: Partial<RacesState>): void;
}

export const reminderKey = (r: Pick<RaceReminder, "pick_id" | "kind" | "lead_min">): string =>
  `${r.pick_id}#${r.kind}${r.lead_min != null ? `-${r.lead_min}` : ""}`;

export const useRaces = create<RacesState>((set) => ({
  reminders: [],
  serverReminders: true,
  picks: null,
  picksRevision: 0,
  sources: null,
  sourcesAt: 0,
  sourcesError: null,
  sourcesLoading: false,
  settings: null,
  settingsAt: 0,
  pushReminder(r, quiet) {
    const key = reminderKey(r);
    set((s) => ({
      reminders: [
        ...s.reminders.filter((x) => x.key !== key && x.pick_id !== r.pick_id),
        { ...r, key, received_ms: Date.now(), ...(quiet ? { quiet: true } : {}) },
      ].slice(-3),
      picksRevision: s.picksRevision + 1,
    }));
  },
  dismiss(key) {
    set((s) => ({ reminders: s.reminders.filter((x) => x.key !== key) }));
  },
  setPicks(picks) {
    set({ picks });
  },
  bumpPicks() {
    set((s) => ({ picksRevision: s.picksRevision + 1 }));
  },
  update(patch) {
    set(patch);
  },
}));
