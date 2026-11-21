import type { GameId } from "@corde/protocol";
import { create } from "zustand";
import { defaultLayouts, gameLayoutId, gameLayouts } from "../dashboard/defaults";
import { BUILTIN_ID } from "../dashdef/model";
import { useDashes } from "../dashdef/store";
import { gameName, PROFILE_GAMES, profileGame } from "../lib/games";
import { useDashboard } from "./dashboard";
import { type LiveView, useSettings } from "./settings";
import { useTelemetry } from "./telemetry";

/**
 * One profile per game, on this device: the layout of widgets (PC and phone), the race
 * dashboard and the kind of live view. When Corde detects a game, its profile is put back;
 * what is chosen afterwards is kept for that game — also once the game is closed: what is
 * shown is always the profile of the last game seen (`active`).
 *
 * - A game seen for the first time gets the layouts Corde proposes for it (`gameLayouts`:
 *   « Course » without what the game does not publish) and the dashboard « Corde ». Le
 *   Mans Ultimate keeps what was in use before the profiles existed.
 * - A field left empty means « what Corde proposes »: it is filled when the game is seen.
 * - The layouts are ordinary ones: they can be changed, renamed, deleted. A profile that
 *   points at a layout or a dashboard that is gone leaves the current one in place.
 * - Nothing here is sent anywhere: like the other choices of the device, it stays in the
 *   browser's storage.
 */
export interface GameProfile {
  desktop?: string;
  mobile?: string;
  dash?: string;
  liveView?: LiveView;
}

type Profiles = Partial<Record<GameId, GameProfile>>;

interface Persisted {
  enabled: boolean;
  profiles: Profiles;
  /** The last game seen: the profile in place (`null`: none seen yet on this device). */
  active: GameId | null;
}

const KEY = "corde.gameProfiles.v1";
const LIVE_VIEWS: readonly string[] = ["auto", "dash", "widgets"];

function text(v: unknown): string | undefined {
  return typeof v === "string" && v.length > 0 && v.length <= 200 ? v : undefined;
}

/** What was stored, cleaned: unknown games and odd values are left out. */
export function parseProfiles(raw: string | null): Persisted {
  const fallback: Persisted = { enabled: true, profiles: {}, active: null };
  if (!raw) return fallback;
  try {
    const parsed = JSON.parse(raw) as { enabled?: unknown; profiles?: unknown; active?: unknown };
    const profiles: Profiles = {};
    const src =
      parsed.profiles && typeof parsed.profiles === "object"
        ? (parsed.profiles as Record<string, unknown>)
        : {};
    for (const game of PROFILE_GAMES) {
      const p = src[game];
      if (!p || typeof p !== "object") continue;
      const o = p as Record<string, unknown>;
      const view = text(o.liveView);
      profiles[game] = {
        desktop: text(o.desktop),
        mobile: text(o.mobile),
        dash: text(o.dash),
        liveView: view && LIVE_VIEWS.includes(view) ? (view as LiveView) : undefined,
      };
    }
    const active = PROFILE_GAMES.find((g) => g === parsed.active) ?? null;
    return { enabled: parsed.enabled !== false, profiles, active };
  } catch {
    return fallback;
  }
}

function load(): Persisted {
  try {
    return parseProfiles(localStorage.getItem(KEY));
  } catch {
    return { enabled: true, profiles: {}, active: null };
  }
}

function save(s: Persisted): void {
  try {
    localStorage.setItem(
      KEY,
      JSON.stringify({ enabled: s.enabled, profiles: s.profiles, active: s.active }),
    );
  } catch {
    // storage unavailable: the profiles live for this visit only
  }
}

/** What this device shows now. */
function snapshot(): GameProfile {
  const { current } = useDashboard.getState();
  return {
    desktop: current.desktop || undefined,
    mobile: current.mobile || undefined,
    dash: useDashes.getState().selected,
    liveView: useSettings.getState().liveView,
  };
}

function same(a: GameProfile | undefined, b: GameProfile): boolean {
  return (
    !!a &&
    a.desktop === b.desktop &&
    a.mobile === b.mobile &&
    a.dash === b.dash &&
    a.liveView === b.liveView
  );
}

/** What Corde proposes for a game seen for the first time. */
function proposed(game: GameId): GameProfile {
  return {
    desktop: gameLayoutId(game, "desktop"),
    mobile: gameLayoutId(game, "mobile"),
    dash: BUILTIN_ID,
    liveView: useSettings.getState().liveView,
  };
}

/** `chosen`, its empty fields taken from `base`. */
function filled(chosen: GameProfile | undefined, base: GameProfile): GameProfile {
  return {
    desktop: chosen?.desktop ?? base.desktop,
    mobile: chosen?.mobile ?? base.mobile,
    dash: chosen?.dash ?? base.dash,
    liveView: chosen?.liveView ?? base.liveView,
  };
}

/** Makes the layouts Corde proposes for `game` that `profile` points at and that are missing. */
function makeProposed(game: GameId, profile: GameProfile): void {
  const wanted = gameLayouts(game, gameName(game)).filter(
    (l) => l.id === profile.desktop || l.id === profile.mobile,
  );
  if (wanted.length > 0) useDashboard.getState().ensureLayouts(wanted);
}

/** `true` while a profile is being put back: those changes are not the pilot's. */
let applying = false;

function apply(profile: GameProfile): void {
  applying = true;
  try {
    const layouts = useDashboard.getState();
    if (profile.desktop) layouts.showLayout(profile.desktop);
    if (profile.mobile) layouts.showLayout(profile.mobile);
    // A dashboard that is gone shows as « Corde » (`resolveDash`): nothing to check here.
    if (profile.dash && useDashes.getState().selected !== profile.dash)
      useDashes.getState().select(profile.dash);
    if (profile.liveView && useSettings.getState().liveView !== profile.liveView)
      useSettings.getState().update({ liveView: profile.liveView });
  } finally {
    applying = false;
  }
}

interface GameProfilesState extends Persisted {
  setEnabled(on: boolean): void;
  /** A game is running (`null`, the simulator's and unknown ones: see `profileGame`). */
  detect(game: GameId | null | undefined): void;
  /** Something was chosen on this device: kept for the game in place. */
  remember(): void;
  /** A choice made for a game from the settings; shown at once when it is the one in place. */
  choose(game: GameId, patch: GameProfile): void;
  /** Back to what Corde proposes for this game (its layouts are made again). */
  reset(game: GameId): void;
}

export const useGameProfiles = create<GameProfilesState>((set, get) => ({
  ...load(),

  setEnabled(on) {
    // Switched off: what is shown stays, and is no longer tied to a game. Switched on: the
    // profile of the game that is running comes back.
    set({ enabled: on, active: null });
    save(get());
    if (on) get().detect(detected());
  },

  detect(raw) {
    const game = profileGame(raw);
    const s = get();
    if (!s.enabled || !game || game === s.active) return;
    const profiles: Profiles = { ...s.profiles };
    // What was in use before a first other game is Le Mans Ultimate's.
    if (game !== "lmu") profiles.lmu = filled(profiles.lmu, snapshot());
    const profile = filled(profiles[game], game === "lmu" ? snapshot() : proposed(game));
    profiles[game] = profile;
    if (game !== "lmu") makeProposed(game, profile);
    set({ active: game, profiles });
    save(get());
    apply(profile);
  },

  remember() {
    const s = get();
    if (!s.enabled || applying || !s.active) return;
    const now = snapshot();
    if (same(s.profiles[s.active], now)) return;
    set({ profiles: { ...s.profiles, [s.active]: now } });
    save(get());
  },

  choose(game, patch) {
    if (!PROFILE_GAMES.includes(game)) return;
    const s = get();
    const profile = { ...s.profiles[game], ...patch };
    set({ profiles: { ...s.profiles, [game]: profile } });
    save(get());
    if (!s.enabled || s.active !== game) return;
    if (game !== "lmu") makeProposed(game, profile);
    apply(profile);
  },

  reset(game) {
    if (!PROFILE_GAMES.includes(game)) return;
    const s = get();
    let profile: GameProfile;
    if (game === "lmu") {
      // Le Mans Ultimate's layouts are the ones Corde starts with: they are not rewritten
      // (made again when they were deleted).
      useDashboard.getState().ensureLayouts(defaultLayouts());
      profile = {
        desktop: "default-desktop",
        mobile: "default-mobile",
        dash: BUILTIN_ID,
        liveView: useSettings.getState().liveView,
      };
    } else {
      useDashboard.getState().ensureLayouts(gameLayouts(game, gameName(game)), true);
      profile = proposed(game);
    }
    set({ profiles: { ...s.profiles, [game]: profile } });
    save(get());
    if (s.enabled && s.active === game) apply(profile);
  },
}));

/** The game Corde is reading now. A demonstration is not a game. */
function detected(): GameId | null {
  const t = useTelemetry.getState();
  return t.demo === "off" && t.status?.connected ? t.status.game : null;
}

let started = false;

/**
 * Follows the game Corde detects and what is chosen on this device. Called once when the
 * interface starts.
 */
export function startGameProfiles(): void {
  if (started) return;
  started = true;
  let last: GameId | null = null;
  const follow = () => {
    const game = detected();
    if (game === last) return;
    last = game;
    useGameProfiles.getState().detect(game);
  };
  useTelemetry.subscribe(follow);
  follow();
  const remember = () => useGameProfiles.getState().remember();
  useDashboard.subscribe((s, prev) => {
    if (s.current !== prev.current) remember();
  });
  useDashes.subscribe((s, prev) => {
    if (s.selected !== prev.selected) remember();
  });
  useSettings.subscribe((s, prev) => {
    if (s.liveView !== prev.liveView) remember();
  });
}
