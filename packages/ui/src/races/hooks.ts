import { useEffect, useState } from "react";
import type { RacePick, RaceSettings, RaceSources } from "../net/races-api";
import { ApiError } from "../net/sessions-api";
import { errorMessage } from "../sessions/shared";
import { useRaces } from "../store/races";
import { raceBackend, useRacesReady } from "./backend";
import { pickOf } from "./model";
import type { Race } from "./parse";

/** The schedule is asked again after 5 minutes (the server keeps its own copy 10 min / 1 h). */
export const SOURCES_EVERY_MS = 5 * 60_000;
const SETTINGS_EVERY_MS = 60_000;

/** `Date.now()`, refreshed every `everyMs` while the page is visible. */
export function useNow(everyMs: number): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const id = setInterval(() => {
      if (document.visibilityState !== "hidden") setNow(Date.now());
    }, everyMs);
    return () => clearInterval(id);
  }, [everyMs]);
  return now;
}

/**
 * The driver's picks, shared by the PC and the phone through the server (or kept on the phone
 * by the iPhone application, `races/backend`): read when a page needs them, again every 30 s
 * and whenever something changed (a pick toggled, a reminder received).
 */
export function usePicks(): { picks: RacePick[] | null; error: string | null } {
  const online = useRacesReady();
  const revision = useRaces((s) => s.picksRevision);
  const picks = useRaces((s) => s.picks);
  const [error, setError] = useState<string | null>(null);
  const [tick, setTick] = useState(0);
  useEffect(() => {
    const id = setInterval(() => {
      if (document.visibilityState !== "hidden") setTick((n) => n + 1);
    }, 30_000);
    return () => clearInterval(id);
  }, []);
  // biome-ignore lint/correctness/useExhaustiveDependencies: revision and tick are reload triggers
  useEffect(() => {
    if (!online) return;
    const ctrl = new AbortController();
    raceBackend()
      .fetchPicks(ctrl.signal)
      .then((p) => {
        if (ctrl.signal.aborted) return;
        useRaces.getState().setPicks(p);
        setError(null);
      })
      .catch((e: unknown) => {
        if (!ctrl.signal.aborted) setError(errorMessage(e));
      });
    return () => ctrl.abort();
  }, [online, revision, tick]);
  return { picks, error };
}

/* ------------------------------------------------------------------ schedule pages */

let sourcesInFlight: Promise<void> | null = null;

/**
 * Asks the server for the schedule pages (`force`: it reads the sites again). Calls made while
 * one is on its way share it, unless `force` asks for a new reading.
 */
export function loadSources(force = false): Promise<void> {
  if (sourcesInFlight && !force) return sourcesInFlight;
  const store = useRaces.getState();
  store.update({ sourcesLoading: true });
  const p = raceBackend()
    .fetchSources(force)
    .then((d) => {
      useRaces.getState().update({ sources: d, sourcesAt: Date.now(), sourcesError: null });
    })
    .catch((e: unknown) => {
      useRaces.getState().update({ sourcesError: errorMessage(e) });
    })
    .finally(() => {
      if (sourcesInFlight === p) {
        sourcesInFlight = null;
        useRaces.getState().update({ sourcesLoading: false });
      }
    });
  sourcesInFlight = p;
  return p;
}

/** The cached schedule is older than {@link SOURCES_EVERY_MS} (or missing). */
export const sourcesStale = (at: number, now: number): boolean =>
  at === 0 || now - at >= SOURCES_EVERY_MS;

/**
 * The schedule pages read by the server, shared by the Courses page and the home page: asked
 * once when a page needs them and the copy is older than 5 minutes, then every 5 minutes while
 * the page is visible. Moving between the two pages does not ask again.
 */
export function useRaceSources(): {
  data: RaceSources | null;
  error: string | null;
  loading: boolean;
} {
  const online = useRacesReady();
  const data = useRaces((s) => s.sources);
  const error = useRaces((s) => s.sourcesError);
  const loading = useRaces((s) => s.sourcesLoading);
  useEffect(() => {
    if (!online) return;
    const due = () => {
      const { sourcesAt } = useRaces.getState();
      if (sourcesStale(sourcesAt, Date.now())) void loadSources(false);
    };
    due();
    const id = setInterval(() => {
      if (document.visibilityState !== "hidden") due();
    }, 30_000);
    return () => clearInterval(id);
  }, [online]);
  return { data, error, loading };
}

/* ------------------------------------------------------------------ race settings */

let settingsInFlight: Promise<RaceSettings | null> | null = null;

/** Reads the race settings (shared by the callers of the moment). */
export function loadRaceSettings(): Promise<RaceSettings | null> {
  if (settingsInFlight) return settingsInFlight;
  const p = raceBackend()
    .fetchSettings()
    .then((s) => {
      useRaces.getState().update({ settings: s, settingsAt: Date.now() });
      return s;
    })
    .finally(() => {
      if (settingsInFlight === p) settingsInFlight = null;
    });
  settingsInFlight = p;
  return p;
}

/** The race settings (filters, ratings, reminders), shared by the Courses and home pages. */
export function useRaceSettings(): { settings: RaceSettings | null; error: string | null } {
  const online = useRacesReady();
  const settings = useRaces((s) => s.settings);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    if (!online) return;
    if (Date.now() - useRaces.getState().settingsAt < SETTINGS_EVERY_MS) return;
    loadRaceSettings()
      .then(() => setError(null))
      .catch((e: unknown) => setError(errorMessage(e)));
  }, [online]);
  return { settings, error };
}

/** Replaces the shared race settings (optimistic update, or the server's answer). */
export function setRaceSettings(s: RaceSettings | ((prev: RaceSettings) => RaceSettings)): void {
  const prev = useRaces.getState().settings;
  if (typeof s === "function") {
    if (prev) useRaces.getState().update({ settings: s(prev) });
    return;
  }
  useRaces.getState().update({ settings: s, settingsAt: Date.now() });
}

/* ------------------------------------------------------------------ picks */

/** 409: the server keeps at most 50 picks. */
export const isFull = (e: unknown): boolean => e instanceof ApiError && e.status === 409;

/**
 * Adds or removes one start of a race in « Mes courses » (the Courses page and the home page
 * make the same calls): the list follows at once, the server's list replaces it after.
 */
export async function togglePick(race: Race, start: number): Promise<void> {
  const pick = pickOf(race, start);
  const store = useRaces.getState();
  const current = store.picks ?? [];
  const on = current.some((p) => p.id === pick.id);
  store.setPicks(on ? current.filter((p) => p.id !== pick.id) : [...current, pick]);
  try {
    if (on) await raceBackend().deletePick(pick.id);
    else await raceBackend().savePick(pick);
  } finally {
    useRaces.getState().bumpPicks();
  }
}

/** Removes a pick (already gone on the server: fine). */
export async function removePick(p: RacePick): Promise<void> {
  const store = useRaces.getState();
  store.setPicks((store.picks ?? []).filter((x) => x.id !== p.id));
  try {
    await raceBackend().deletePick(p.id);
  } catch (e) {
    if (!(e instanceof ApiError && e.status === 404)) throw e;
  } finally {
    useRaces.getState().bumpPicks();
  }
}
