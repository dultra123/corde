import { fetchRecording } from "../net/sessions-api";
import { useDashboard } from "../store/dashboard";
import { useSettings } from "../store/settings";
import { freshEvents, type TelemetryState, useTelemetry } from "../store/telemetry";
import { isEmptyHash, navigate, parseRoute, type Route, sameRoute } from "./routes";

/**
 * Automatic navigation — what makes the window behave like an app instead of a web page:
 *
 * - the driver **takes the track** → the live dashboard opens (`#/`);
 * - the driver has been **off the track for 10 s** and the stint left a recorded session with at
 *   least one lap → that session's page opens (`#/sessions/<id>`), or the home page when the
 *   summary option is off.
 *
 * Two per-device options (settings dialog, localStorage) switch each half off. The page never
 * jumps under the user's hand: nothing moves within 15 s of a manual page change, nor while the
 * dashboard is being edited, and a manual change made after the event cancels the jump.
 *
 * The rules live in {@link step}, a pure state machine; {@link createAutoNav} is the driver that
 * feeds it and applies its decisions (both tested); {@link startAutoNav} plugs the driver into
 * the browser and the stores.
 */

/** How long the driver must stay off the track before the summary opens. */
export const LEAVE_DELAY_MS = 10_000;
/** No automatic jump within this delay after the user changed page by hand. */
export const MANUAL_GRACE_MS = 15_000;

export interface AutoNavOptions {
  /** « Ouvrir le dashboard quand j'entre en piste ». */
  dashboardOnTrack: boolean;
  /** « Afficher le résumé de la session quand je rentre au garage ». */
  summaryAfterSession: boolean;
}

export const DEFAULT_OPTIONS: AutoNavOptions = {
  dashboardOnTrack: true,
  summaryAfterSession: true,
};

export interface AutoNavState {
  /** Last known answer to "is the driver on track?" — `null` until the server says. */
  onTrack: boolean | null;
  /**
   * The app was opened without naming a page (it shows the home page): if the driver turns out
   * to be on track already, the dashboard opens.
   */
  bootPending: boolean;
  /** The driver took the track at this time and the dashboard has not been opened yet. */
  enter: { at: number } | null;
  /** The driver left the track at this time and the summary has not been decided yet. */
  leave: { at: number } | null;
  /** Session recorded during the current stint (or the one that just ended). */
  recorded: { id: string; laps: number } | null;
  /** Last time the user changed page by hand. */
  manualAt: number | null;
}

export function initialState(bootPending: boolean): AutoNavState {
  return {
    onTrack: null,
    bootPending,
    enter: null,
    leave: null,
    recorded: null,
    manualAt: null,
  };
}

export type AutoNavEvent =
  /**
   * What the server says now. `linked` is false while there is no live server behind the page
   * (socket closed, demo mode): everything pending is dropped and nothing can happen.
   */
  | { type: "link"; now: number; linked: boolean; onTrack: boolean }
  /** A session is being (or was just) recorded, with that many completed laps. */
  | { type: "recorded"; id: string; laps: number }
  /** The user changed page by hand (tab, link, back button…). */
  | { type: "manual"; now: number }
  /** Time passes (once a second while something is pending). */
  | { type: "tick"; now: number };

export interface AutoNavContext {
  /** The page currently shown. */
  route: Route;
  /** The dashboard is in edit mode. */
  editing: boolean;
  options: AutoNavOptions;
}

export interface AutoNavStep {
  state: AutoNavState;
  /** Where to go now, or `null` to stay. */
  go: Route | null;
}

const DASHBOARD: Route = { page: "dashboard" };
const HOME: Route = { page: "home" };

function inGrace(state: AutoNavState, now: number): boolean {
  return state.manualAt !== null && now - state.manualAt < MANUAL_GRACE_MS;
}

/** One transition of the machine. Pure: same inputs, same answer. */
export function step(prev: AutoNavState, event: AutoNavEvent, ctx: AutoNavContext): AutoNavStep {
  switch (event.type) {
    case "recorded": {
      const same = prev.recorded?.id === event.id;
      const laps = same ? Math.max(prev.recorded?.laps ?? 0, event.laps) : event.laps;
      return { state: { ...prev, recorded: { id: event.id, laps } }, go: null };
    }
    case "manual":
      // The user is driving the UI: whatever was about to happen is theirs to decide now.
      return {
        state: { ...prev, manualAt: event.now, bootPending: false, enter: null, leave: null },
        go: null,
      };
    case "tick":
      return decide(prev, event.now, ctx);
    case "link": {
      if (!event.linked) {
        // No live server: forget what was pending. The next answer is a new baseline, not a
        // transition — a page that reconnects must not jump because of what it missed.
        return { state: { ...prev, onTrack: null, enter: null, leave: null }, go: null };
      }
      let state = prev;
      const was = prev.onTrack;
      if (event.onTrack && was !== true) {
        // false → true: the driver takes the track. null → true: he was already there when the
        // page connected, which only counts for an app that just opened on its home page.
        const counts = was === false || (prev.bootPending && ctx.route.page === "home");
        state = {
          ...state,
          onTrack: true,
          enter: counts ? { at: event.now } : null,
          leave: null,
          recorded: was === false ? null : state.recorded,
        };
      } else if (!event.onTrack && was !== false) {
        state = {
          ...state,
          onTrack: false,
          enter: null,
          leave: was === true ? { at: event.now } : null,
        };
      }
      if (state.bootPending) state = { ...state, bootPending: false };
      return decide(state, event.now, ctx);
    }
  }
}

/** Acts on what is pending, if its time has come. */
function decide(prev: AutoNavState, now: number, ctx: AutoNavContext): AutoNavStep {
  let state = prev;

  if (state.enter) {
    if (!ctx.options.dashboardOnTrack || sameRoute(ctx.route, DASHBOARD)) {
      state = { ...state, enter: null };
    } else if (!inGrace(state, now)) {
      return { state: { ...state, enter: null }, go: DASHBOARD };
    }
    // else: the user changed page less than 15 s ago — wait for the grace period to end
  }

  if (state.leave && now - state.leave.at >= LEAVE_DELAY_MS) {
    const session = state.recorded && state.recorded.laps >= 1 ? state.recorded : null;
    const onLivePage = ctx.route.page === "dashboard" || ctx.route.page === "home";
    const editing = ctx.route.page === "dashboard" && ctx.editing;
    if (!session || !onLivePage || editing) {
      // nothing worth showing, or the user is busy elsewhere: stay
      state = { ...state, leave: null };
    } else if (!inGrace(state, now)) {
      const target: Route = ctx.options.summaryAfterSession
        ? { page: "session", id: session.id }
        : HOME;
      return {
        state: { ...state, leave: null },
        go: sameRoute(ctx.route, target) ? null : target,
      };
    }
  }

  return { state, go: null };
}

/** `true` while the machine waits for a delay to elapse (the driver then ticks once a second). */
export function isPending(state: AutoNavState): boolean {
  return state.enter !== null || state.leave !== null;
}

/* ------------------------------------------------------------------ driver */

/** `?autonav=0` (or `off` / `false`) pins the page: for an OBS browser source or a kiosk. */
export function isAutoNavDisabled(search: string = window.location.search): boolean {
  const v = new URLSearchParams(search).get("autonav");
  return v === "0" || v === "off" || v === "false";
}

type LinkInput = Pick<TelemetryState, "socket" | "status" | "demo" | "hello">;

/**
 * What the telemetry store says about the link to the server and the driver.
 *
 * Only the server's status counts, and only once this connection has said `hello`:
 * - waiting for a frame as well would make a page that just connected see "garage, then track"
 *   within a few milliseconds;
 * - right after a reconnection the store still holds the status of the previous connection
 *   (`staleHello` is the `hello` that was current while the socket was down).
 */
export function linkOf(
  s: LinkInput,
  staleHello: TelemetryState["hello"] = null,
): { linked: boolean; onTrack: boolean } {
  const linked =
    s.demo === "off" &&
    s.socket === "open" &&
    s.status !== null &&
    s.hello !== null &&
    s.hello !== staleHello;
  return { linked, onTrack: linked && !!s.status?.connected && !!s.status.in_realtime };
}

/** What the driver needs from the outside world (the browser and the stores; fakes in tests). */
export interface AutoNavEnv {
  now(): number;
  /** `window.location.hash`. */
  hash(): string;
  /** Shows a page (sets the hash). */
  go(route: Route): void;
  /** The dashboard is in edit mode. */
  editing(): boolean;
  options(): AutoNavOptions;
  /** The session the recorder holds open right now, with its completed laps. */
  recording(): Promise<{ id: string; laps: number } | null>;
}

export interface AutoNav {
  /** The server's answer changed (see {@link linkOf}). */
  link(linked: boolean, onTrack: boolean): void;
  /** A recording started (`laps` 0) or stopped. */
  recorded(id: string, laps: number): void;
  /** The browser announced a hash change. */
  hashChanged(): void;
  /** One second passed. */
  tick(): void;
  /** Something waits for a delay: keep ticking. */
  readonly pending: boolean;
  readonly state: AutoNavState;
}

/**
 * The driver: feeds {@link step} and applies its decisions.
 *
 * It tells its own jumps from the user's by remembering the hash it left the page on. That is
 * checked before every event and not only on `hashchange`, because the browser announces a
 * change a moment after it happened: a status message landing in between must already see the
 * click that preceded it.
 */
export function createAutoNav(env: AutoNavEnv): AutoNav {
  let state = initialState(isEmptyHash(env.hash()));
  let knownHash = env.hash();

  const apply = (event: AutoNavEvent): void => {
    const hadLeave = state.leave !== null;
    const res = step(state, event, {
      route: parseRoute(env.hash()),
      editing: env.editing(),
      options: env.options(),
    });
    state = res.state;
    if (res.go) {
      env.go(res.go);
      knownHash = env.hash();
    }
    if (!hadLeave && state.leave !== null) void askRecorder();
  };

  const noticeManual = (): void => {
    const hash = env.hash();
    if (hash === knownHash) return;
    knownHash = hash;
    apply({ type: "manual", now: env.now() });
  };

  const send = (event: AutoNavEvent): void => {
    noticeManual();
    apply(event);
  };

  /**
   * The driver just left the track: which session was being recorded, and with how many laps?
   * The recorder keeps the session open for a minute after the car stops, so it still knows;
   * when it has already closed it, the `recording_stopped` event said it.
   */
  const askRecorder = async (): Promise<void> => {
    for (let attempt = 0; attempt < 2; attempt += 1) {
      try {
        const rec = await env.recording();
        if (rec) send({ type: "recorded", id: rec.id, laps: rec.laps });
        return;
      } catch {
        await new Promise((r) => setTimeout(r, 2000));
      }
    }
  };

  return {
    link: (linked, onTrack) => send({ type: "link", now: env.now(), linked, onTrack }),
    recorded: (id, laps) => send({ type: "recorded", id, laps }),
    hashChanged: noticeManual,
    tick: () => send({ type: "tick", now: env.now() }),
    get pending() {
      return isPending(state);
    },
    get state() {
      return state;
    },
  };
}

let started = false;

/**
 * Starts the automatic navigation (once, outside React — like the connection). Call it before
 * the first render, while `location.hash` is still what the app was opened with.
 */
export function startAutoNav(): void {
  if (started || typeof window === "undefined" || isAutoNavDisabled()) return;
  started = true;

  const nav = createAutoNav({
    now: () => Date.now(),
    hash: () => window.location.hash,
    go: navigate,
    editing: () => useDashboard.getState().editing,
    options: () => {
      const { autoDashboard, autoSummary } = useSettings.getState();
      return { dashboardOnTrack: autoDashboard, summaryAfterSession: autoSummary };
    },
    recording: async () => {
      const rec = await fetchRecording();
      return rec.session_id ? { id: rec.session_id, laps: rec.laps } : null;
    },
  });

  // one tick a second, only while a delay is running
  let timer: ReturnType<typeof setInterval> | null = null;
  const syncTimer = (): void => {
    if (nav.pending && !timer) {
      timer = setInterval(() => {
        nav.tick();
        syncTimer();
      }, 1000);
    } else if (!nav.pending && timer) {
      clearInterval(timer);
      timer = null;
    }
  };

  window.addEventListener("hashchange", () => {
    nav.hashChanged();
    syncTimer();
  });

  let last: { linked: boolean; onTrack: boolean } | null = null;
  let staleHello: TelemetryState["hello"] = null;
  let seenEvents = useTelemetry.getState().events;
  const onTelemetry = (s: TelemetryState): void => {
    if (s.events !== seenEvents) {
      const fresh = freshEvents(seenEvents, s.events);
      seenEvents = s.events;
      for (const e of fresh) {
        if (e.kind === "recording_started") nav.recorded(e.session_id, 0);
        else if (e.kind === "recording_stopped") nav.recorded(e.session_id, e.laps);
      }
    }
    if (s.socket !== "open") staleHello = s.hello;
    const link = linkOf(s, staleHello);
    if (!last || last.linked !== link.linked || last.onTrack !== link.onTrack) {
      last = link;
      nav.link(link.linked, link.onTrack);
    }
    syncTimer();
  };
  useTelemetry.subscribe(onTelemetry);
  onTelemetry(useTelemetry.getState());
}
