import { isDesktop, isLocalPage } from "../lib/desktop";
import {
  deletePick,
  fetchPicks,
  fetchRaceSettings,
  fetchRaceSources,
  type RacePick,
  type RaceSettings,
  type RaceSettingsPatch,
  type RaceSources,
  reportUnreadable,
  type SourceId,
  savePick,
  saveRaceSettings,
} from "../net/races-api";
import { fetchSettings, saveSettings } from "../net/system-api";
import { isHub, isHubAdmin } from "../store/hub";
import { useTelemetry } from "../store/telemetry";

/**
 * Where the « Courses » page and the home carousel get their races from:
 *
 * - **`server`** — the Corde server that serves the page (PC or home server): it reads the
 *   two public pages, keeps the picks and settings, and rings the reminders
 *   (`crates/corde-server/src/races/`). The browser, the desktop application.
 * - **`phone`** — the iPhone application itself (`mobile/phone-races.ts`): it reads the pages
 *   with the phone's native HTTP, keeps picks and settings on the phone and schedules the
 *   reminders as local notifications. No PC needed.
 *
 * The pages only talk to this interface: same answers (`RaceSources`, `RacePick`,
 * `RaceSettings`), same errors (`ApiError`, 409 beyond 50 picks), so the components are the
 * same for both.
 */
export interface RaceBackend {
  readonly kind: "server" | "phone";
  /** The two schedule pages as text (`force`: read them again, within the rate limits). */
  fetchSources(force: boolean): Promise<RaceSources>;
  /** A page had no race in it (server: keeps its copy for diagnosis). */
  reportUnreadable(id: SourceId): Promise<void>;
  fetchPicks(signal?: AbortSignal): Promise<RacePick[]>;
  savePick(pick: RacePick): Promise<RacePick>;
  deletePick(id: string): Promise<void>;
  fetchSettings(): Promise<RaceSettings>;
  saveSettings(patch: RaceSettingsPatch): Promise<RaceSettings>;
  /** The schedule is read at all (`online_schedule` on a server). */
  fetchEnabled(): Promise<boolean>;
  setEnabled(on: boolean): Promise<void>;
  /** The schedule may be turned on / off from here. */
  canSwitch(): boolean;
}

/**
 * The server's: on a PC only from the PC itself; on the home server from an administrator
 * device (the server says so too).
 */
export const serverRaces: RaceBackend = {
  kind: "server",
  fetchSources: (force) => fetchRaceSources(force),
  reportUnreadable: async (id) => {
    await reportUnreadable(id);
  },
  fetchPicks: (signal) => fetchPicks(signal),
  savePick: (p) => savePick(p),
  deletePick: (id) => deletePick(id),
  fetchSettings: () => fetchRaceSettings(),
  saveSettings: (patch) => saveRaceSettings(patch),
  fetchEnabled: async () => (await fetchSettings()).online_schedule === true,
  setEnabled: async (on) => {
    await saveSettings({ online_schedule: on });
  },
  canSwitch: () => (isHub() ? isHubAdmin() : isDesktop() || isLocalPage()),
};

let current: RaceBackend = serverRaces;

/** The application installs its own (`mobile/boot.ts`). */
export function setRaceBackend(backend: RaceBackend): void {
  current = backend;
}

export function raceBackend(): RaceBackend {
  return current;
}

/**
 * Whether the races can be asked for now: on a server, while it answers on the socket; on the
 * phone, always.
 */
export function useRacesReady(): boolean {
  const online = useTelemetry((s) => s.socket === "open");
  return current.kind === "phone" || online;
}
