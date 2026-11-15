import { act, type ReactNode } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { TopBar } from "../dashboard/TopBar";
import { template } from "../dashdef/builtin";
import type { DashDefinition } from "../dashdef/model";
import { useDashes } from "../dashdef/store";
import { en } from "../i18n/en";
import { fr } from "../i18n/fr";
import { SessionsShell } from "../sessions/SessionsShell";
import { useHub } from "../store/hub";
import { useSettings } from "../store/settings";
import { useTelemetry } from "../store/telemetry";
import { StudioPage } from "./StudioPage";

/**
 * The Studio as a section of the app (ADR 0018): its tab in the top bar — on a PC, not on a
 * phone, not on the home server — and its pages under that bar, nothing of their own bars lost.
 */

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

const ENDURANCE: DashDefinition = { ...template("endurance"), id: "endurance", updated_ms: 5 };

beforeEach(() => {
  useSettings.setState({ lang: "fr", keepAwake: false });
  useHub.setState({ role: "desktop" });
});

afterEach(() => {
  useTelemetry.getState().resetData();
  useDashes.setState({ defs: [], selected: "corde", sync: "idle", base: null });
  useHub.setState({ role: "unknown" });
  localStorage.clear();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

describe("the Studio tab of the top bar", () => {
  const tabs = (div: HTMLElement) =>
    [...div.querySelectorAll<HTMLAnchorElement>(".navtab")].map((a) => a.textContent);

  it("is the sixth tab on a PC, current on the pages of the Studio", () => {
    const { div, unmount } = render(<TopBar section="studio" onSettings={() => {}} />);
    expect(tabs(div)).toEqual(["Accueil", "Direct", "Sessions", "Courses", "Profil", "Studio"]);
    const studio = div.querySelector<HTMLAnchorElement>(".navtab.is-current");
    expect(studio?.textContent).toBe("Studio");
    expect(studio?.getAttribute("aria-current")).toBe("page");
    expect(studio?.getAttribute("href")).toBe("#/dash-studio");
    expect(studio?.querySelector("svg")).not.toBeNull();
    // The controls of the Direct page stay there.
    expect(div.querySelector(".layoutmenu")).toBeNull();
    unmount();
    // On another section the tab is there, not current.
    const live = render(<TopBar section="dashboard" onSettings={() => {}} />);
    expect(tabs(live.div)).toContain("Studio");
    expect(live.div.querySelector(".navtab.is-current")?.textContent).toBe("Direct");
    live.unmount();
  });

  it("is not shown on the home server", () => {
    act(() => useHub.setState({ role: "hub" }));
    const { div, unmount } = render(<TopBar section="home" onSettings={() => {}} />);
    expect(tabs(div)).toEqual(["Accueil", "Direct", "Sessions", "Courses", "Profil"]);
    unmount();
  });

  it("is not shown on a phone (the rule of the phone dashboard: a finger, a small screen)", () => {
    vi.stubGlobal("matchMedia", (q: string) => ({
      matches: q.includes("pointer: coarse"),
      media: q,
      addEventListener: () => {},
      removeEventListener: () => {},
    }));
    vi.stubGlobal("innerWidth", 393);
    vi.stubGlobal("innerHeight", 852);
    const phone = render(<TopBar section="home" onSettings={() => {}} />);
    expect(tabs(phone.div)).toEqual(["Accueil", "Direct", "Sessions", "Courses", "Profil"]);
    phone.unmount();
    // A tablet (a finger, a large screen) keeps it, as it keeps the widgets.
    vi.stubGlobal("innerWidth", 1024);
    vi.stubGlobal("innerHeight", 768);
    const tablet = render(<TopBar section="home" onSettings={() => {}} />);
    expect(tabs(tablet.div)).toContain("Studio");
    tablet.unmount();
  });

  it("the home server has no Studio: its address shows what the Direct tab shows there", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => new Response("{}", { status: 404 })),
    );
    act(() => useHub.setState({ role: "hub" }));
    const { div, unmount } = render(<SessionsShell route={{ page: "studio" }} />);
    await act(async () => {});
    expect(div.querySelector(".studio")).toBeNull();
    expect(div.querySelector(".hub-direct")).not.toBeNull();
    expect(tabs(div)).not.toContain("Studio");
    expect(div.querySelector(".navtab.is-current")?.textContent).toBe("Direct");
    unmount();
  });

  it("has its name in both languages", () => {
    expect(fr["nav.studio"]).toBe("Studio");
    expect(en["nav.studio"]).toBe("Studio");
  });
});

describe("the Studio as a section of the app", () => {
  it("the library sits under the top bar, the tab current, no way out lost", () => {
    act(() => useDashes.setState({ defs: [ENDURANCE], sync: "ready" }));
    const { div, unmount } = render(<StudioPage />);
    expect(div.querySelector(".app.is-studio > .topbar.is-studio")).not.toBeNull();
    expect(div.querySelector(".navtab.is-current")?.textContent).toBe("Studio");
    expect(div.querySelector(".app.is-studio > .studio .studio-lib")).not.toBeNull();
    // The other sections are one click away (the tabs), the settings too.
    const hrefs = [...div.querySelectorAll<HTMLAnchorElement>(".navtab")].map((a) =>
      a.getAttribute("href"),
    );
    expect(hrefs).toEqual(["#/home", "#/", "#/sessions", "#/races", "#/profile", "#/dash-studio"]);
    expect(div.querySelector('[aria-label="Réglages"]')).not.toBeNull();
    // The bar of the library: its title, « Nouveau », « Importer » — and no second way to the
    // Direct page (the tab is that way).
    const bar = div.querySelector(".studio > .studio-bar");
    expect(bar?.querySelector(".studio-title")?.textContent).toContain("Studio");
    expect([...(bar?.querySelectorAll("button") ?? [])].map((b) => b.textContent)).toEqual([
      "Nouveau",
      "Importer un fichier…",
    ]);
    expect(bar?.querySelector("a")).toBeNull();
    expect(div.querySelectorAll(".studio-card")).toHaveLength(2);
    unmount();
  });

  it("the editor keeps every control of its bar, under the top bar", () => {
    act(() => useDashes.setState({ defs: [ENDURANCE], sync: "ready" }));
    const { div, unmount } = render(<StudioPage id="endurance" />);
    expect(div.querySelector(".app.is-studio > .topbar")).not.toBeNull();
    expect(div.querySelector(".navtab.is-current")?.textContent).toBe("Studio");
    const bar = div.querySelector(".studio.is-editor > .studio-bar");
    const labels = [...(bar?.querySelectorAll("button") ?? [])].map((b) => b.textContent?.trim());
    for (const l of [
      "← Bibliothèque",
      "Annuler",
      "Rétablir",
      "Course de démonstration",
      "Aperçu plein écran",
      "Exporter",
      "Enregistré",
    ]) {
      expect(labels, l).toContain(l);
    }
    expect(bar?.querySelector('input[aria-label="Nom du tableau de bord"]')).not.toBeNull();
    expect(div.querySelector(".studio-tabs")).not.toBeNull();
    expect(div.querySelector(".studio-edit .studio-canvas")).not.toBeNull();
    unmount();
  });
});
