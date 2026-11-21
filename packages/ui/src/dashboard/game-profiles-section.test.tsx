import type { SourceStatus } from "@corde/protocol";
import { act, type ReactNode } from "react";
import { createRoot } from "react-dom/client";
import { beforeEach, describe, expect, it, vi } from "vitest";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

function render(node: ReactNode): { div: HTMLDivElement; unmount: () => void } {
  const div = document.createElement("div");
  document.body.append(div);
  const root = createRoot(div);
  act(() => root.render(node));
  return {
    div,
    unmount: () => {
      act(() => root.unmount());
      div.remove();
    },
  };
}

function pick(select: HTMLSelectElement | null | undefined, value: string): void {
  if (!select) throw new Error("no select");
  act(() => {
    select.value = value;
    select.dispatchEvent(new Event("change", { bubbles: true }));
  });
}

describe("« Profils par jeu » in the settings", () => {
  beforeEach(() => {
    localStorage.clear();
    vi.resetModules();
  });

  async function setup() {
    await import("../widgets");
    const { useDashboard } = await import("../store/dashboard");
    const { useSettings } = await import("../store/settings");
    const { useTelemetry } = await import("../store/telemetry");
    const { useGameProfiles, startGameProfiles } = await import("../store/game-profiles");
    const { GameProfilesSection } = await import("./GameProfilesSection");
    useDashboard.getState().setDevice("desktop");
    useSettings.setState({ lang: "fr" });
    startGameProfiles();
    const run = (game: SourceStatus["game"]) =>
      act(() =>
        useTelemetry.setState({ demo: "off", status: { connected: true, game } as SourceStatus }),
      );
    return { useDashboard, useGameProfiles, GameProfilesSection, run };
  }

  it("lists the games, shows which profile is in place and what each one uses", async () => {
    const { useDashboard, GameProfilesSection, run } = await setup();
    const { div, unmount } = render(<GameProfilesSection />);
    const games = () => [...div.querySelectorAll("fieldset legend")].map((n) => n.textContent);
    expect(games()).toEqual([
      "Le Mans Ultimate",
      "Assetto Corsa Competizione",
      "Assetto Corsa",
      "Assetto Corsa EVO",
      "Assetto Corsa Rally",
      "iRacing",
    ]);
    const layout = (game: string) =>
      div.querySelector<HTMLSelectElement>(`#profile-layout-${game}`);
    // nothing seen yet: Le Mans Ultimate shows what is in use, the others Corde's proposal
    expect(layout("lmu")?.value).toBe("default-desktop");
    expect(layout("iracing")?.value).toBe("");
    expect(layout("iracing")?.selectedOptions[0]?.textContent).toContain("Proposée par Corde");
    // only the layouts of this kind of screen are offered
    const offered = [...(layout("lmu")?.options ?? [])].map((o) => o.value);
    expect(offered).toEqual(["default-desktop"]);

    run("iracing");
    expect(games()).toContain("iRacingen place");
    expect(layout("iracing")?.value).toBe("game-iracing-desktop");
    expect(layout("iracing")?.selectedOptions[0]?.textContent).toBe("iRacing");
    expect(useDashboard.getState().current.desktop).toBe("game-iracing-desktop");
    unmount();
  });

  it("a choice is kept for its game and shown at once for the one in place", async () => {
    const { useDashboard, useGameProfiles, GameProfilesSection, run } = await setup();
    run("lmu");
    const sprint = useDashboard.getState().createLayout("Sprint", "desktop");
    act(() => useDashboard.getState().selectLayout("default-desktop"));
    const { div, unmount } = render(<GameProfilesSection />);
    pick(div.querySelector<HTMLSelectElement>("#profile-layout-acc"), sprint);
    expect(useGameProfiles.getState().profiles.acc?.desktop).toBe(sprint);
    expect(useDashboard.getState().current.desktop).toBe("default-desktop");
    pick(div.querySelector<HTMLSelectElement>("#profile-layout-lmu"), sprint);
    expect(useDashboard.getState().current.desktop).toBe(sprint);
    // back to Corde's proposal for a game not seen yet
    pick(div.querySelector<HTMLSelectElement>("#profile-layout-acc"), "default-desktop");
    expect(useGameProfiles.getState().profiles.acc?.desktop).toBe("default-desktop");
    unmount();
  });

  it("reset asks before rewriting a layout, and the switch hides the games", async () => {
    const { useDashboard, useGameProfiles, GameProfilesSection, run } = await setup();
    run("iracing");
    act(() => useDashboard.getState().addWidget("tyres"));
    const { div, unmount } = render(<GameProfilesSection />);
    const reset = () =>
      [...div.querySelectorAll("fieldset")]
        .find((f) => f.querySelector("legend")?.textContent?.startsWith("iRacing"))
        ?.querySelector<HTMLButtonElement>(".profile-reset");
    const hasTyres = () =>
      useDashboard
        .getState()
        .layouts.find((l) => l.id === "game-iracing-desktop")
        ?.widgets.some((w) => w.kind === "tyres");
    const answers: boolean[] = [false, true];
    const asked: string[] = [];
    window.confirm = (msg?: string) => {
      asked.push(String(msg));
      return answers.shift() ?? false;
    };
    act(() => reset()?.click());
    expect(asked).toHaveLength(1);
    expect(hasTyres()).toBe(true);
    act(() => reset()?.click());
    expect(hasTyres()).toBe(false);

    const toggle = div.querySelector<HTMLInputElement>(".switch-toggle");
    act(() => toggle?.click());
    expect(useGameProfiles.getState().enabled).toBe(false);
    expect(div.querySelectorAll("fieldset")).toHaveLength(0);
    unmount();
  });
});
