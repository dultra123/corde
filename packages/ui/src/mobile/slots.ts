import type { ComponentType, ReactNode } from "react";
import type { TranslationKey } from "../i18n";
import type { Route } from "../nav/routes";
import type { RacePick } from "../net/races-api";

/**
 * What the iPhone application adds to the shared pages, filled in by `mobile/boot.ts` before
 * the first render. Empty in the browser build (which never loads `boot.ts`): the shared pages
 * only test whether a slot is there, so none of the application's code reaches the browser.
 *
 * Hooks in here are installed once, before the first render, and never change: calling them
 * where the slot exists keeps React's rules (same hooks, same order, every render).
 */
export interface AppSlots {
  /** The tab bar at the bottom of the screen. */
  BottomTabs?: ComponentType<{ section: string }>;
  /** `#/servers` — « Mes serveurs ». */
  ServersPage?: ComponentType;
  /**
   * For a page that needs a Corde server (Direct, Sessions, Engineer, Profile): what to show
   * instead while no server is active and reachable, `null` when the page can be shown.
   */
  useServerGate?: (route: Route) => ReactNode | null;
  /** On the home page: the active server, or how to add one. */
  HomeServerCard?: ComponentType;
  /** On the home page: the cards that need a server are shown. */
  useServerOnline?: () => boolean;
  /** On the Courses page, under the reminder delays: the notifications of the phone. */
  ReminderStatus?: ComponentType;
  /** On the Courses page, after the reminders: the Live Activity setting, the widgets' state. */
  RemindersExtra?: ComponentType;
  /** On a row of « Mes courses »: the button that starts the countdown on the lock screen. */
  PickAction?: ComponentType<{ pick: RacePick; now: number }>;
  /** A sentence that names the phone's system: its key for this phone (`mobile/device.ts`). */
  phoneText?: (key: TranslationKey) => TranslationKey;
}

export const appSlots: AppSlots = {};

const noGate = (_route: Route): ReactNode | null => null;

/**
 * The gate of {@link AppSlots.useServerGate}, `null` in the browser. A hook of its own, called
 * unconditionally by the shell: the slot is fixed before the first render.
 */
export function useAppServerGate(route: Route): ReactNode | null {
  const gate = appSlots.useServerGate ?? noGate;
  return gate(route);
}

/** {@link AppSlots.useServerOnline}, or `fallback` in the browser. */
export function useAppServerOnline(fallback: boolean): boolean {
  const online = appSlots.useServerOnline;
  return online ? online() : fallback;
}
