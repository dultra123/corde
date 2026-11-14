import { act, type ReactNode } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { Dashboard } from "../dashboard/Dashboard";
import { LayoutMenu } from "../dashboard/LayoutMenu";
import { useDashboard } from "../store/dashboard";
import { useSettings } from "../store/settings";
import { useTelemetry } from "../store/telemetry";
import { DashView } from "./DashView";
import { useDashFull } from "./store";
import { showWidgets } from "./view";

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

function openMenu(div: HTMLElement): HTMLElement {
  act(() => div.querySelector<HTMLButtonElement>(".layoutmenu-btn")?.click());
  const menu = div.querySelector<HTMLElement>(".menu");
  if (!menu) throw new Error("menu not open");
  return menu;
}

function itemNamed(menu: HTMLElement, name: string): HTMLButtonElement {
  const b = [...menu.querySelectorAll<HTMLButtonElement>("button.menu-item")].find((x) =>
    x.textContent?.startsWith(name),
  );
  if (!b) throw new Error(`no item ${name}`);
  return b;
}

beforeEach(() => {
  // A PC: a mouse and a large window (happy-dom's defaults).
  useSettings.setState({ lang: "fr", liveView: "auto", keepAwake: false });
  useDashFull.setState({ full: false });
  useDashboard.getState().setEditing(false);
});

afterEach(() => {
  useTelemetry.getState().resetData();
  useSettings.setState({ liveView: "auto" });
});

describe("the menu of the Direct page on a PC", () => {
  it("lists the dashboards next to the layouts, one click each way", () => {
    const { div, unmount } = render(<LayoutMenu />);
    let menu = openMenu(div);
    const sections = [...menu.querySelectorAll(".menu-section")].map((s) => s.textContent);
    expect(sections).toContain("Tableaux de bord");
    // To the dashboard…
    act(() => itemNamed(menu, "Corde").click());
    expect(useSettings.getState().liveView).toBe("dash");
    expect(div.querySelector(".layoutmenu-name")?.textContent).toBe("Corde");
    // …and back to a layout of widgets.
    menu = openMenu(div);
    expect(itemNamed(menu, "Corde").getAttribute("aria-checked")).toBe("true");
    const layout = useDashboard.getState().layouts.find((l) => l.device === "desktop");
    if (!layout) throw new Error("no desktop layout");
    act(() => itemNamed(menu, layout.name).click());
    expect(useSettings.getState().liveView).toBe("widgets");
    expect(div.querySelector(".layoutmenu-name")?.textContent).toBe(layout.name);
    unmount();
  });

  it("hides the actions of the hidden layout while the dashboard is shown", () => {
    useSettings.setState({ liveView: "dash" });
    const { div, unmount } = render(<LayoutMenu />);
    const menu = openMenu(div);
    expect(menu.textContent).not.toContain("Renommer");
    expect(menu.textContent).not.toContain("Supprimer");
    unmount();
  });
});

describe("the dashboard inside the Direct page", () => {
  it("keeps the top bar, without widget editing", () => {
    const { div, unmount } = render(<Dashboard dash />);
    expect(div.querySelector(".topbar")).not.toBeNull();
    expect(div.querySelector(".layoutmenu")).not.toBeNull();
    expect(div.querySelector('[aria-label="Modifier"]')).toBeNull();
    unmount();
  });

  it("goes full screen with its button, Esc leaves", () => {
    const { div, unmount } = render(<DashView embedded />);
    expect(div.querySelector(".dash.is-embedded")).not.toBeNull();
    const full = div.querySelector<HTMLButtonElement>(".dash-full");
    expect(full?.getAttribute("aria-label")).toBe("Plein écran");
    act(() => full?.click());
    expect(useDashFull.getState().full).toBe(true);
    act(() => {
      window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
    });
    expect(useDashFull.getState().full).toBe(false);
    unmount();
  });

  it("back to the widgets leaves the full screen too", () => {
    act(() => useDashFull.getState().setFull(true));
    act(() => showWidgets());
    expect(useDashFull.getState().full).toBe(false);
    expect(useSettings.getState().liveView).toBe("widgets");
  });
});
