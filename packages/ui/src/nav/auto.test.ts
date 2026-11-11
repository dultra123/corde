import type { SourceStatus } from "@corde/protocol";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  type AutoNavEnv,
  type AutoNavEvent,
  type AutoNavOptions,
  type AutoNavState,
  createAutoNav,
  DEFAULT_OPTIONS,
  initialState,
  isAutoNavDisabled,
  isPending,
  LEAVE_DELAY_MS,
  linkOf,
  MANUAL_GRACE_MS,
  step,
} from "./auto";
import { type Route, routeHash } from "./routes";

const HOME: Route = { page: "home" };
const DASHBOARD: Route = { page: "dashboard" };
const SESSION: Route = { page: "session", id: "s1" };
const LAP: Route = { page: "lap", id: "old", lap: 3 };

/** A page with the machine behind it: applies the jumps like the real driver does. */
function app(init: { route?: Route; boot?: boolean; options?: Partial<AutoNavOptions> } = {}) {
  let state: AutoNavState = initialState(init.boot ?? false);
  const page = {
    route: init.route ?? HOME,
    editing: false,
    options: { ...DEFAULT_OPTIONS, ...init.options },
    get state() {
      return state;
    },
    send(event: AutoNavEvent): Route | null {
      const res = step(state, event, {
        route: page.route,
        editing: page.editing,
        options: page.options,
      });
      state = res.state;
      if (res.go) page.route = res.go;
      return res.go;
    },
    /** The server says where the driver is. */
    track(now: number, onTrack: boolean) {
      return page.send({ type: "link", now, linked: true, onTrack });
    },
    offline(now: number) {
      return page.send({ type: "link", now, linked: false, onTrack: false });
    },
    tick(now: number) {
      return page.send({ type: "tick", now });
    },
    /** The user clicks a tab or a link. */
    click(now: number, route: Route) {
      page.route = route;
      return page.send({ type: "manual", now });
    },
    recorded(id: string, laps: number) {
      return page.send({ type: "recorded", id, laps });
    },
  };
  return page;
}

/** In the garage at t = 0, on track at t = 1 s. */
function onTrackFrom(route: Route, options?: Partial<AutoNavOptions>) {
  const page = app({ route, options });
  page.track(0, false);
  page.track(1000, true);
  return page;
}

describe("taking the track", () => {
  it("opens the dashboard from the home page", () => {
    const page = app({ route: HOME });
    expect(page.track(0, false)).toBeNull();
    expect(page.track(5000, true)).toEqual(DASHBOARD);
    expect(page.route).toEqual(DASHBOARD);
    expect(isPending(page.state)).toBe(false);
  });

  it("opens the dashboard even from a lap being analysed, by default", () => {
    for (const route of [LAP, SESSION, { page: "engineer", id: "s1" } as Route]) {
      const page = app({ route });
      page.track(0, false);
      expect(page.track(1000, true)).toEqual(DASHBOARD);
    }
  });

  it("does nothing when the dashboard is already shown", () => {
    const page = app({ route: DASHBOARD });
    page.track(0, false);
    expect(page.track(1000, true)).toBeNull();
    expect(isPending(page.state)).toBe(false);
  });

  it("never opens the dashboard when the option is off", () => {
    for (const route of [LAP, SESSION, HOME]) {
      const page = app({ route, options: { dashboardOnTrack: false } });
      page.track(0, false);
      expect(page.track(1000, true)).toBeNull();
      expect(page.tick(60_000)).toBeNull();
      expect(page.route).toEqual(route);
    }
  });

  it("only reacts to the change, not to every status", () => {
    const page = app({ route: HOME });
    page.track(0, false);
    expect(page.track(1000, true)).toEqual(DASHBOARD);
    page.click(60_000, SESSION);
    // still on track 5 minutes later: the user's page is left alone
    expect(page.track(300_000, true)).toBeNull();
    expect(page.tick(301_000)).toBeNull();
    expect(page.route).toEqual(SESSION);
  });
});

describe("opening the app", () => {
  it("goes straight to the dashboard when the driver is already on track", () => {
    const page = app({ route: HOME, boot: true });
    // the socket is not open yet: nothing is known, the boot rule stays armed
    expect(page.offline(0)).toBeNull();
    expect(page.state.bootPending).toBe(true);
    expect(page.track(800, true)).toEqual(DASHBOARD);
  });

  it("stays on the home page when the driver is in the garage, then follows him", () => {
    const page = app({ route: HOME, boot: true });
    expect(page.track(800, false)).toBeNull();
    expect(page.state.bootPending).toBe(false);
    expect(page.track(30_000, true)).toEqual(DASHBOARD);
  });

  it("respects a page asked for explicitly (deep link, reload)", () => {
    for (const route of [SESSION, LAP, HOME]) {
      const page = app({ route, boot: false });
      expect(page.track(800, true)).toBeNull();
      expect(page.tick(20_000)).toBeNull();
      expect(page.route).toEqual(route);
    }
  });

  it("does not jump once the user has started browsing", () => {
    const page = app({ route: HOME, boot: true });
    page.offline(0);
    page.click(1000, { page: "sessions" });
    expect(page.track(2000, true)).toBeNull();
    expect(page.tick(60_000)).toBeNull();
    expect(page.route).toEqual({ page: "sessions" });
  });

  it("stays on the home page when the option is off", () => {
    const page = app({ route: HOME, boot: true, options: { dashboardOnTrack: false } });
    expect(page.track(800, true)).toBeNull();
    expect(page.route).toEqual(HOME);
  });
});

describe("back in the garage", () => {
  it("opens the recorded session after 10 s off the track", () => {
    const page = onTrackFrom(HOME);
    expect(page.route).toEqual(DASHBOARD);
    page.recorded("s1", 0);
    expect(page.track(100_000, false)).toBeNull();
    page.recorded("s1", 4); // what GET /api/recording answers when the driver stops
    expect(isPending(page.state)).toBe(true);
    expect(page.tick(100_000 + LEAVE_DELAY_MS - 1)).toBeNull();
    expect(page.tick(100_000 + LEAVE_DELAY_MS)).toEqual(SESSION);
    expect(isPending(page.state)).toBe(false);
    // decided once: nothing more happens
    expect(page.tick(200_000)).toBeNull();
  });

  it("goes back to the home page instead when the summary option is off", () => {
    const page = onTrackFrom(HOME, { summaryAfterSession: false });
    page.recorded("s1", 4);
    page.track(100_000, false);
    expect(page.tick(100_000 + LEAVE_DELAY_MS)).toEqual(HOME);
  });

  it("stays put when the dashboard never opened and the summary option is off", () => {
    const page = onTrackFrom(HOME, { dashboardOnTrack: false, summaryAfterSession: false });
    page.recorded("s1", 4);
    page.track(100_000, false);
    expect(page.tick(100_000 + LEAVE_DELAY_MS)).toBeNull();
    expect(page.route).toEqual(HOME);
    expect(isPending(page.state)).toBe(false);
  });

  it("shows the summary from the home page when only the dashboard option is off", () => {
    const page = onTrackFrom(HOME, { dashboardOnTrack: false });
    page.recorded("s1", 4);
    page.track(100_000, false);
    expect(page.tick(100_000 + LEAVE_DELAY_MS)).toEqual(SESSION);
  });

  it("ignores a quick return to the track (debounce)", () => {
    const page = onTrackFrom(HOME);
    page.recorded("s1", 4);
    page.track(100_000, false);
    expect(page.tick(105_000)).toBeNull();
    // Esc then straight back out: still the dashboard, no summary later
    expect(page.track(108_000, true)).toBeNull();
    expect(page.tick(100_000 + LEAVE_DELAY_MS)).toBeNull();
    expect(page.tick(130_000)).toBeNull();
    expect(page.route).toEqual(DASHBOARD);
    expect(isPending(page.state)).toBe(false);
  });

  it("needs a recorded session with at least one lap", () => {
    const none = onTrackFrom(HOME);
    none.track(100_000, false);
    expect(none.tick(100_000 + LEAVE_DELAY_MS)).toBeNull();
    expect(none.route).toEqual(DASHBOARD);
    expect(isPending(none.state)).toBe(false);

    const noLap = onTrackFrom(HOME);
    noLap.recorded("s1", 0);
    noLap.track(100_000, false);
    expect(noLap.tick(100_000 + LEAVE_DELAY_MS)).toBeNull();
    expect(noLap.route).toEqual(DASHBOARD);
    // the answer came too late: the moment has passed
    noLap.recorded("s1", 3);
    expect(noLap.tick(100_000 + 3 * LEAVE_DELAY_MS)).toBeNull();
  });

  it("uses the session the recorder closed during the stint", () => {
    const page = onTrackFrom(HOME);
    page.recorded("s1", 0); // recording_started
    page.recorded("s1", 6); // recording_stopped (the game was closed)
    page.recorded("s1", 2); // a stale, smaller count never wins
    page.track(100_000, false);
    expect(page.tick(100_000 + LEAVE_DELAY_MS)).toEqual(SESSION);
  });

  it("does not reuse the session of the previous stint", () => {
    const page = onTrackFrom(HOME);
    page.recorded("s1", 4);
    page.track(100_000, false);
    expect(page.tick(100_000 + LEAVE_DELAY_MS)).toEqual(SESSION);
    // out again (dashboard), straight back in without a recording this time
    expect(page.track(200_000, true)).toEqual(DASHBOARD);
    expect(page.state.recorded).toBeNull();
    page.track(210_000, false);
    expect(page.tick(210_000 + LEAVE_DELAY_MS)).toBeNull();
    expect(page.route).toEqual(DASHBOARD);
  });

  it("leaves the user alone on any other page", () => {
    const profile: Route = { page: "profile" };
    const page = onTrackFrom(HOME);
    page.recorded("s1", 4);
    page.click(50_000, profile);
    page.track(100_000, false);
    expect(page.tick(100_000 + LEAVE_DELAY_MS)).toBeNull();
    expect(page.tick(100_000 + MANUAL_GRACE_MS * 4)).toBeNull();
    expect(page.route).toEqual(profile);
    expect(isPending(page.state)).toBe(false);
  });

  it("does not leave a session page that is already the right one", () => {
    const page = onTrackFrom(HOME, { dashboardOnTrack: false });
    page.recorded("s1", 4);
    page.click(50_000, HOME);
    page.track(100_000, false);
    expect(page.tick(100_000 + LEAVE_DELAY_MS)).toEqual(SESSION);
    expect(page.track(200_000, true)).toBeNull();
    page.recorded("s1", 9);
    page.track(300_000, false);
    // the session page is not a "live" page: nothing moves
    expect(page.tick(300_000 + LEAVE_DELAY_MS)).toBeNull();
    expect(page.route).toEqual(SESSION);
  });
});

describe("never under the user's hand", () => {
  it("waits 15 s after a manual page change before opening the dashboard", () => {
    const page = app({ route: HOME });
    page.track(0, false);
    page.click(10_000, { page: "engineer", id: "old" });
    // "Drive" 5 s after the click: not now…
    expect(page.track(15_000, true)).toBeNull();
    expect(isPending(page.state)).toBe(true);
    expect(page.tick(10_000 + MANUAL_GRACE_MS - 1)).toBeNull();
    // …but as soon as the 15 s have passed, since the driver is still out there
    expect(page.tick(10_000 + MANUAL_GRACE_MS)).toEqual(DASHBOARD);
  });

  it("gives up the pending jump when the user changes page again", () => {
    const page = app({ route: HOME });
    page.track(0, false);
    page.click(10_000, SESSION);
    expect(page.track(15_000, true)).toBeNull();
    page.click(20_000, LAP);
    expect(isPending(page.state)).toBe(false);
    expect(page.tick(10_000 + MANUAL_GRACE_MS)).toBeNull();
    expect(page.tick(20_000 + MANUAL_GRACE_MS)).toBeNull();
    expect(page.tick(120_000)).toBeNull();
    expect(page.route).toEqual(LAP);
  });

  it("gives up the pending jump when the driver comes back in before it happened", () => {
    const page = app({ route: HOME });
    page.track(0, false);
    page.click(10_000, SESSION);
    expect(page.track(15_000, true)).toBeNull();
    expect(page.track(18_000, false)).toBeNull();
    expect(page.tick(10_000 + MANUAL_GRACE_MS)).toBeNull();
    expect(page.tick(18_000 + LEAVE_DELAY_MS)).toBeNull();
    expect(page.route).toEqual(SESSION);
  });

  it("is immediate once the last manual change is older than 15 s", () => {
    const page = app({ route: HOME });
    page.track(0, false);
    page.click(10_000, SESSION);
    expect(page.track(10_000 + MANUAL_GRACE_MS, true)).toEqual(DASHBOARD);
  });

  it("delays the summary until 15 s after a manual change made just before stopping", () => {
    const page = app({ route: HOME, options: { dashboardOnTrack: false } });
    page.track(0, false);
    page.track(1000, true);
    page.recorded("s1", 4);
    page.click(98_000, DASHBOARD); // 2 s before leaving the track
    page.track(100_000, false);
    expect(page.tick(100_000 + LEAVE_DELAY_MS)).toBeNull(); // only 12 s since the click
    expect(isPending(page.state)).toBe(true);
    expect(page.tick(98_000 + MANUAL_GRACE_MS)).toEqual(SESSION);
  });

  it("drops the summary when the user changes page after the driver stopped", () => {
    const page = onTrackFrom(HOME);
    page.recorded("s1", 4);
    page.track(100_000, false);
    page.click(103_000, HOME);
    expect(isPending(page.state)).toBe(false);
    expect(page.tick(100_000 + LEAVE_DELAY_MS)).toBeNull();
    expect(page.tick(103_000 + MANUAL_GRACE_MS)).toBeNull();
    expect(page.route).toEqual(HOME);
  });

  it("never moves while the dashboard is being edited, even once editing is over", () => {
    const page = onTrackFrom(HOME);
    page.recorded("s1", 4);
    page.editing = true;
    page.track(100_000, false);
    expect(page.tick(100_000 + LEAVE_DELAY_MS)).toBeNull();
    expect(isPending(page.state)).toBe(false);
    page.editing = false;
    expect(page.tick(100_000 + 2 * LEAVE_DELAY_MS)).toBeNull();
    expect(page.route).toEqual(DASHBOARD);
  });

  it("ignores the edit flag left on while another page is shown", () => {
    const page = onTrackFrom(HOME, { dashboardOnTrack: false });
    page.editing = true; // edit mode was left on before leaving the dashboard
    page.recorded("s1", 4);
    page.track(100_000, false);
    expect(page.tick(100_000 + LEAVE_DELAY_MS)).toEqual(SESSION);
  });
});

describe("losing the server", () => {
  it("drops what was pending and does not jump on reconnection", () => {
    const page = onTrackFrom(HOME);
    page.recorded("s1", 4);
    page.track(100_000, false);
    expect(page.offline(103_000)).toBeNull();
    expect(isPending(page.state)).toBe(false);
    expect(page.tick(100_000 + LEAVE_DELAY_MS)).toBeNull();
    // back online with the driver on track: a new baseline, not a transition
    page.click(150_000, SESSION);
    expect(page.track(400_000, true)).toBeNull();
    expect(page.tick(500_000)).toBeNull();
    expect(page.route).toEqual(SESSION);
    // the next real transitions count again
    page.track(600_000, false);
    expect(page.track(700_000, true)).toEqual(DASHBOARD);
  });

  it("does not take a reconnection in the garage for a stint that just ended", () => {
    const page = onTrackFrom(HOME);
    page.recorded("s1", 4);
    page.offline(50_000);
    expect(page.track(60_000, false)).toBeNull();
    expect(isPending(page.state)).toBe(false);
    expect(page.tick(60_000 + LEAVE_DELAY_MS)).toBeNull();
    expect(page.route).toEqual(DASHBOARD);
  });
});

describe("createAutoNav (the driver)", () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  /** A browser reduced to its hash, a clock and a recorder. */
  function browser(hash: string, recording: AutoNavEnv["recording"] = async () => null) {
    const world = {
      hash,
      now: 0,
      jumps: [] as string[],
      asked: 0,
    };
    const nav = createAutoNav({
      now: () => world.now,
      hash: () => world.hash,
      go: (route) => {
        world.hash = routeHash(route);
        world.jumps.push(world.hash);
      },
      editing: () => false,
      options: () => DEFAULT_OPTIONS,
      recording: () => {
        world.asked += 1;
        return recording();
      },
    });
    return { world, nav };
  }

  it("arms the start-up rule only when the app was opened without naming a page", () => {
    expect(browser("").nav.state.bootPending).toBe(true);
    expect(browser("#").nav.state.bootPending).toBe(true);
    expect(browser("#/home").nav.state.bootPending).toBe(false);
    expect(browser("#/").nav.state.bootPending).toBe(false);

    const { world, nav } = browser("");
    nav.link(false, false); // socket still connecting
    nav.link(true, true); // hello: already on track
    expect(world.jumps).toEqual(["#/"]);
  });

  it("does not take its own jumps for the user's", () => {
    const { world, nav } = browser("#/home");
    nav.link(true, false);
    world.now = 1000;
    nav.link(true, true);
    expect(world.jumps).toEqual(["#/"]);
    nav.hashChanged(); // the browser announces the jump we just made
    expect(nav.state.manualAt).toBeNull();
    // so the summary is not held back by a "manual" change that never happened
    world.now = 5000;
    nav.recorded("s1", 3);
    nav.link(true, false);
    world.now = 5000 + LEAVE_DELAY_MS;
    nav.tick();
    expect(world.jumps).toEqual(["#/", "#/sessions/s1"]);
  });

  it("sees a click before the browser has announced it", () => {
    const { world, nav } = browser("#/sessions/old");
    nav.link(true, false);
    // The user clicks « Profil »: the hash changes at once, `hashchange` comes a moment later —
    // and the server's "on track" lands in between.
    world.now = 60_000;
    world.hash = "#/profile";
    nav.link(true, true);
    expect(world.jumps).toEqual([]);
    expect(nav.state.manualAt).toBe(60_000);
    expect(nav.pending).toBe(true);
    nav.hashChanged(); // the late announcement changes nothing
    expect(nav.state.manualAt).toBe(60_000);
    world.now = 60_000 + MANUAL_GRACE_MS - 1;
    nav.tick();
    expect(world.jumps).toEqual([]);
    world.now = 60_000 + MANUAL_GRACE_MS;
    nav.tick();
    expect(world.jumps).toEqual(["#/"]);
    expect(nav.pending).toBe(false);
  });

  it("counts a hash change it did not make as manual", () => {
    const { world, nav } = browser("#/home");
    nav.link(true, false);
    world.now = 10_000;
    world.hash = "#/sessions";
    nav.hashChanged();
    expect(nav.state.manualAt).toBe(10_000);
    world.now = 12_000;
    nav.link(true, true);
    expect(world.jumps).toEqual([]); // within the grace period
  });

  it("asks the recorder once when the driver leaves the track", async () => {
    const { world, nav } = browser("#/", async () => ({ id: "s1", laps: 7 }));
    nav.link(true, true);
    expect(world.asked).toBe(0);
    world.now = 100_000;
    nav.link(true, false);
    expect(world.asked).toBe(1);
    await Promise.resolve();
    await Promise.resolve();
    expect(nav.state.recorded).toEqual({ id: "s1", laps: 7 });
    nav.tick();
    expect(world.asked).toBe(1);
    world.now = 100_000 + LEAVE_DELAY_MS;
    nav.tick();
    expect(world.jumps).toEqual(["#/sessions/s1"]);
  });

  it("stays put when the recorder holds nothing, and retries once when it fails", async () => {
    const idle = browser("#/");
    idle.nav.link(true, true);
    idle.world.now = 100_000;
    idle.nav.link(true, false);
    await Promise.resolve();
    idle.world.now = 100_000 + LEAVE_DELAY_MS;
    idle.nav.tick();
    expect(idle.world.jumps).toEqual([]);

    vi.useFakeTimers();
    let calls = 0;
    const flaky = browser("#/", async () => {
      calls += 1;
      if (calls === 1) throw new Error("server busy");
      return { id: "s2", laps: 4 };
    });
    flaky.nav.link(true, true);
    flaky.world.now = 100_000;
    flaky.nav.link(true, false);
    await vi.advanceTimersByTimeAsync(2100);
    expect(calls).toBe(2);
    expect(flaky.nav.state.recorded).toEqual({ id: "s2", laps: 4 });
    flaky.world.now = 100_000 + LEAVE_DELAY_MS;
    flaky.nav.tick();
    expect(flaky.world.jumps).toEqual(["#/sessions/s2"]);
  });
});

describe("linkOf", () => {
  const status = (over: Partial<SourceStatus> = {}): SourceStatus => ({
    connected: true,
    game: "lmu",
    source: "lmu",
    input_rate_hz: 100,
    output_rate_hz: 20,
    in_realtime: true,
    game_version: null,
    last_error: null,
    clients: 1,
    recording: false,
    ...over,
  });

  const hello = { serverVersion: "0.2.0", protocolVersion: 1 };
  const open = { socket: "open", demo: "off", hello } as const;

  it("is on track when the server says the game is in real time", () => {
    // no frame needed: `hello` already carries the answer, before the first frame arrives
    expect(linkOf({ ...open, status: status() })).toEqual({ linked: true, onTrack: true });
    expect(linkOf({ ...open, status: status({ in_realtime: false }) })).toEqual({
      linked: true,
      onTrack: false,
    });
    // game closed: whatever the last frame said no longer counts
    expect(linkOf({ ...open, status: status({ connected: false, in_realtime: true }) })).toEqual({
      linked: true,
      onTrack: false,
    });
  });

  it("is not linked without a server or in demo mode", () => {
    expect(linkOf({ ...open, socket: "connecting", status: status() }).linked).toBe(false);
    expect(linkOf({ ...open, socket: "closed", status: status() }).linked).toBe(false);
    expect(linkOf({ ...open, status: null }).linked).toBe(false);
    for (const demo of ["auto", "manual"] as const) {
      expect(linkOf({ ...open, socket: "closed", demo, status: status() })).toEqual({
        linked: false,
        onTrack: false,
      });
    }
  });

  it("waits for this connection's hello before believing the status", () => {
    // socket open, nothing said yet
    expect(linkOf({ ...open, hello: null, status: status() }).linked).toBe(false);
    // reconnected: the store still holds the hello and the status of the previous connection
    const stale = status({ in_realtime: false });
    expect(linkOf({ ...open, status: stale }, hello)).toEqual({ linked: false, onTrack: false });
    // the new hello arrives with the real status
    const fresh = { serverVersion: "0.2.0", protocolVersion: 1 };
    expect(linkOf({ ...open, hello: fresh, status: status() }, hello)).toEqual({
      linked: true,
      onTrack: true,
    });
  });
});

describe("isAutoNavDisabled", () => {
  it("reads ?autonav=0 (OBS browser source, kiosk)", () => {
    expect(isAutoNavDisabled("")).toBe(false);
    expect(isAutoNavDisabled("?server=192.168.1.20:8471")).toBe(false);
    expect(isAutoNavDisabled("?autonav=1")).toBe(false);
    expect(isAutoNavDisabled("?autonav=0")).toBe(true);
    expect(isAutoNavDisabled("?server=pc:8471&autonav=off")).toBe(true);
    expect(isAutoNavDisabled("?autonav=false")).toBe(true);
  });
});
