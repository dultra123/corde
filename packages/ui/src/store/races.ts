import type { RaceReminder } from "@corde/protocol";
import { create } from "zustand";
import type { RacePick } from "../net/races-api";

/**
 * State of the « Courses » feature shared by every page: the reminders waiting in the banner
 * (pushed by `net/connection` on `race_reminder`) and the driver's picks (read from the server
 * by `races/usePicks`, shown on the Courses page and on the home page).
 */
export interface ShownReminder extends RaceReminder {
  /** `<pick_id>#<kind>[-<lead>]`, unique per reminder. */
  key: string;
  received_ms: number;
}

interface RacesState {
  /** Newest last; at most three. */
  reminders: ShownReminder[];
  picks: RacePick[] | null;
  /** Bumped when the picks changed somewhere (reminder received, pick toggled). */
  picksRevision: number;
  pushReminder(r: RaceReminder): void;
  dismiss(key: string): void;
  setPicks(p: RacePick[]): void;
  bumpPicks(): void;
}

export const reminderKey = (r: Pick<RaceReminder, "pick_id" | "kind" | "lead_min">): string =>
  `${r.pick_id}#${r.kind}${r.lead_min != null ? `-${r.lead_min}` : ""}`;

export const useRaces = create<RacesState>((set) => ({
  reminders: [],
  picks: null,
  picksRevision: 0,
  pushReminder(r) {
    const key = reminderKey(r);
    set((s) => ({
      reminders: [
        ...s.reminders.filter((x) => x.key !== key && x.pick_id !== r.pick_id),
        { ...r, key, received_ms: Date.now() },
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
}));
