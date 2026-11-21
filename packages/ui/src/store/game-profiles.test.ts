import type { GameId, SourceStatus } from "@corde/protocol";
import { beforeEach, describe, expect, it, vi } from "vitest";

function status(game: GameId, connected = true): SourceStatus {
  return { connected, game } as SourceStatus;
}

describe("one profile per game", () => {
  beforeEach(() => {
    localStorage.clear();
    vi.resetModules();
  });

  async function stores(device: "desktop" | "mobile" = "desktop") {
    await import("../widgets");
    const { useDashboard } = await import("./dashboard");
    const { useDashes } = await import("../dashdef/store");
    const { useSettings } = await import("./settings");
    const { useTelemetry } = await import("./telemetry");
    const profiles = await import("./game-profiles");
    useDashboard.getState().setDevice(device);
    profiles.startGameProfiles();
    const run = (game: GameId | null) =>
      useTelemetry.setState(
        game ? { demo: "off", status: status(game) } : { status: status("unknown", false) },
      );
    return { useDashboard, useDashes, useSettings, useTelemetry, run, ...profiles };
  }

  it("Le Mans Ultimate keeps what was in use; another game gets its own interface", async () => {
    const { useDashboard, useDashes, useGameProfiles, run } = await stores();
    const mine = useDashboard.getState().createLayout("Endurance", "desktop");
    run("lmu");
    expect(useGameProfiles.getState().active).toBe("lmu");
    expect(useGameProfiles.getState().profiles.lmu?.desktop).toBe(mine);
    expect(useDashboard.getState().current.desktop).toBe(mine);

    run("iracing");
    const s = useDashboard.getState();
    expect(s.current.desktop).toBe("game-iracing-desktop");
    expect(s.current.mobile).toBe("game-iracing-mobile");
    const layout = s.layouts.find((l) => l.id === "game-iracing-desktop");
    expect(layout?.name).toBe("iRacing");
    // what the game does not publish while driving is not in what Corde proposes
    const kinds = layout?.widgets.map((w) => w.kind) ?? [];
    expect(kinds).not.toContain("tyres");
    expect(kinds).not.toContain("brakes");
    expect(kinds).toEqual(expect.arrayContaining(["laps", "inputs", "map", "standings", "fuel"]));
    expect(useDashes.getState().selected).toBe("corde");

    // back to the first game: its layout comes back
    run("lmu");
    expect(useDashboard.getState().current.desktop).toBe(mine);
    // the game closed: nothing moves, and the profile in place stays the last game's
    run(null);
    expect(useDashboard.getState().current.desktop).toBe(mine);
    expect(useGameProfiles.getState().active).toBe("lmu");
  });

  it("a first game other than Le Mans Ultimate leaves what was in use to Le Mans Ultimate", async () => {
    const { useDashboard, useGameProfiles, run } = await stores();
    run("acc");
    const p = useGameProfiles.getState().profiles;
    expect(p.lmu?.desktop).toBe("default-desktop");
    expect(p.acc?.desktop).toBe("game-acc-desktop");
    const kinds =
      useDashboard
        .getState()
        .layouts.find((l) => l.id === "game-acc-desktop")
        ?.widgets.map((w) => w.kind) ?? [];
    // Competizione names no other driver, but gives tyres and brakes
    expect(kinds).not.toContain("standings");
    expect(kinds).toEqual(expect.arrayContaining(["tyres", "brakes", "gaps", "electronics"]));
  });

  it("what is chosen stays with the game in place, also once it is closed", async () => {
    const { useDashboard, useDashes, useSettings, useGameProfiles, run } = await stores();
    run("lmu");
    run("acc");
    const other = useDashboard.getState().createLayout("Sprint", "desktop");
    useDashes.getState().select("my-dash");
    useSettings.getState().update({ liveView: "dash" });
    run(null);
    // chosen with no game running: still Competizione's
    const late = useDashboard.getState().createLayout("Nuit", "desktop");
    expect(useGameProfiles.getState().profiles.acc).toMatchObject({
      desktop: late,
      dash: "my-dash",
      liveView: "dash",
    });
    expect(other).not.toBe(late);

    run("lmu");
    expect(useDashboard.getState().current.desktop).toBe("default-desktop");
    expect(useDashes.getState().selected).toBe("corde");
    expect(useSettings.getState().liveView).toBe("auto");
    // putting Le Mans Ultimate's profile back did not write into Competizione's
    expect(useGameProfiles.getState().profiles.acc?.desktop).toBe(late);

    run("acc");
    expect(useDashboard.getState().current.desktop).toBe(late);
    expect(useDashes.getState().selected).toBe("my-dash");
    expect(useSettings.getState().liveView).toBe("dash");
  });

  it("is kept across a restart, and a demonstration is not a game", async () => {
    const first = await stores();
    first.run("lmu");
    first.run("iracing");
    vi.resetModules();
    const { useDashboard, useGameProfiles, useTelemetry, run } = await stores();
    expect(useGameProfiles.getState().active).toBe("iracing");
    expect(useDashboard.getState().current.desktop).toBe("game-iracing-desktop");
    // a demonstration plays Le Mans Ultimate: nothing changes
    useTelemetry.setState({ demo: "manual", status: status("lmu") });
    expect(useDashboard.getState().current.desktop).toBe("game-iracing-desktop");
    useTelemetry.setState({ demo: "off", status: status("lmu", false) });
    run("lmu");
    expect(useDashboard.getState().current.desktop).toBe("default-desktop");
    // the simulator of the developers counts as Le Mans Ultimate
    run("iracing");
    run("sim");
    expect(useGameProfiles.getState().active).toBe("lmu");
  });

  it("a choice from the settings shows at once for the game in place only", async () => {
    const { useDashboard, useDashes, useGameProfiles, run } = await stores();
    run("lmu");
    const sprint = useDashboard.getState().createLayout("Sprint", "desktop");
    useDashboard.getState().selectLayout("default-desktop");
    // for a game never seen: kept, nothing shown, its other choices stay Corde's proposal
    useGameProfiles.getState().choose("iracing", { desktop: sprint });
    expect(useDashboard.getState().current.desktop).toBe("default-desktop");
    useGameProfiles.getState().choose("lmu", { dash: "my-dash" });
    expect(useDashes.getState().selected).toBe("my-dash");

    run("iracing");
    expect(useDashboard.getState().current.desktop).toBe(sprint);
    expect(useDashboard.getState().current.mobile).toBe("game-iracing-mobile");
    expect(useDashes.getState().selected).toBe("corde");
    // only the proposed layout the profile points at was made
    const ids = useDashboard.getState().layouts.map((l) => l.id);
    expect(ids).toContain("game-iracing-mobile");
    expect(ids).not.toContain("game-iracing-desktop");
    // … and choosing the proposal makes it
    useGameProfiles.getState().choose("iracing", { desktop: "game-iracing-desktop" });
    expect(useDashboard.getState().current.desktop).toBe("game-iracing-desktop");
  });

  it("reset puts a game back as proposed", async () => {
    const { useDashboard, useGameProfiles, run } = await stores();
    run("iracing");
    useDashboard.getState().addWidget("tyres");
    const changed = useDashboard.getState().layouts.find((l) => l.id === "game-iracing-desktop");
    expect(changed?.widgets.some((w) => w.kind === "tyres")).toBe(true);
    const sprint = useDashboard.getState().createLayout("Sprint", "desktop");
    expect(useGameProfiles.getState().profiles.iracing?.desktop).toBe(sprint);

    useGameProfiles.getState().reset("iracing");
    const again = useDashboard.getState().layouts.find((l) => l.id === "game-iracing-desktop");
    expect(again?.widgets.some((w) => w.kind === "tyres")).toBe(false);
    expect(again?.updated_ms).toBeGreaterThan(0);
    expect(useDashboard.getState().current.desktop).toBe("game-iracing-desktop");

    // Le Mans Ultimate goes back to « Course », made again when it was deleted
    useDashboard.getState().deleteLayout("default-desktop");
    useGameProfiles.getState().reset("lmu");
    expect(useDashboard.getState().layouts.some((l) => l.id === "default-desktop")).toBe(true);
    expect(useGameProfiles.getState().profiles.lmu?.desktop).toBe("default-desktop");
    // not the game in place: nothing shown
    expect(useDashboard.getState().current.desktop).toBe("game-iracing-desktop");
  });

  it("a proposed layout that is only added does not win over the same one changed elsewhere", async () => {
    const { useDashboard, run } = await stores();
    run("acc");
    const made = useDashboard.getState().layouts.filter((l) => l.id.startsWith("game-acc-"));
    expect(made).toHaveLength(2);
    for (const l of made) expect(l.updated_ms).toBe(0);
  });

  it("switched off, nothing is put back nor kept; switched on, the running game's profile comes back", async () => {
    const { useDashboard, useGameProfiles, run } = await stores();
    run("lmu");
    run("iracing");
    useGameProfiles.getState().setEnabled(false);
    expect(useGameProfiles.getState().active).toBeNull();
    useDashboard.getState().selectLayout("default-desktop");
    run("lmu");
    run("iracing");
    expect(useDashboard.getState().current.desktop).toBe("default-desktop");
    expect(useGameProfiles.getState().profiles.iracing?.desktop).toBe("game-iracing-desktop");
    useGameProfiles.getState().setEnabled(true);
    expect(useGameProfiles.getState().active).toBe("iracing");
    expect(useDashboard.getState().current.desktop).toBe("game-iracing-desktop");
  });

  it("a profile whose layout is gone leaves what is shown", async () => {
    const { useDashboard, useGameProfiles, run } = await stores();
    run("lmu");
    const gone = useDashboard.getState().createLayout("Éphémère", "desktop");
    run("acc");
    // deleted on another device: the list comes back from the server without it
    const { layouts, replaceLayouts } = useDashboard.getState();
    replaceLayouts(
      layouts.filter((l) => l.id !== gone),
      true,
    );
    run("lmu");
    expect(useGameProfiles.getState().active).toBe("lmu");
    expect(useDashboard.getState().current.desktop).toBe("game-acc-desktop");
  });

  it("reads what was stored with care", async () => {
    const { parseProfiles } = await stores();
    expect(parseProfiles(null)).toEqual({ enabled: true, profiles: {}, active: null });
    expect(parseProfiles("{nope")).toEqual({ enabled: true, profiles: {}, active: null });
    const odd = parseProfiles(
      JSON.stringify({
        enabled: false,
        active: "rfactor",
        profiles: {
          acc: { desktop: "a", mobile: 3, dash: "", liveView: "sideways" },
          nope: { desktop: "x" },
          lmu: "text",
        },
      }),
    );
    expect(odd.enabled).toBe(false);
    expect(odd.active).toBeNull();
    expect(Object.keys(odd.profiles)).toEqual(["acc"]);
    expect(odd.profiles.acc).toEqual({
      desktop: "a",
      mobile: undefined,
      dash: undefined,
      liveView: undefined,
    });
    expect(parseProfiles(JSON.stringify({ active: "iracing" })).active).toBe("iracing");
  });
});
