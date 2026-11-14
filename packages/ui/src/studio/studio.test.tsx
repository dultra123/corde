import type { LapRecord, TelemetryFrame } from "@corde/protocol";
import { act, type ReactNode } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { DashView } from "../dash/DashView";
import { EMPTY_PAGE, type PageProps } from "../dash/parts";
import { useDashPage } from "../dash/store";
import { template } from "../dashdef/builtin";
import { listElementSpecs } from "../dashdef/catalog";
import type { DashDefinition } from "../dashdef/model";
import { ElementView } from "../dashdef/Renderer";
import { useDashes } from "../dashdef/store";
import { DemoRace } from "../demo/generator";
import { useSettings } from "../store/settings";
import { useTelemetry } from "../store/telemetry";
import { DashEditor } from "./Editor";
import { commit, historyOf, redo, undo, useEditor } from "./editor-store";

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

function demo(): { frame: TelemetryFrame; laps: LapRecord[] } {
  const race = new DemoRace({ seed: 7 });
  const laps: LapRecord[] = [];
  let frame: TelemetryFrame | null = null;
  for (let i = 0; i < 2.2 * 115 * 20; i += 1) {
    const out = race.step(0.05, i % 20 === 0);
    laps.push(...out.laps);
    if (out.frame) frame = out.frame;
  }
  if (!frame) throw new Error("no frame");
  return { frame, laps };
}

const DEMO = demo();
const ENDURANCE: DashDefinition = { ...template("endurance"), id: "endurance", updated_ms: 5 };

beforeEach(() => {
  useSettings.setState({ lang: "fr", keepAwake: false });
});

afterEach(() => {
  useTelemetry.getState().resetData();
  useDashes.setState({ defs: [], selected: "corde" });
});

describe("every element", () => {
  const data: PageProps = {
    ...EMPTY_PAGE,
    frame: DEMO.frame,
    laps: DEMO.laps,
    standings: DEMO.frame.standings,
  };
  it("draws with data and without, never throws", () => {
    for (const spec of listElementSpecs()) {
      for (const d of [data, EMPTY_PAGE]) {
        const el = {
          id: "e1",
          type: spec.type,
          x: 0,
          y: 0,
          w: spec.size.w,
          h: spec.size.h,
          props: {},
        };
        const { div, unmount } = render(<ElementView el={el} data={d} lang="fr" u={4} />);
        expect(div.querySelector(".del-unknown"), spec.type).toBeNull();
        unmount();
      }
    }
  });

  it("a changed colour shows at once, with no telemetry frame arriving (game closed)", () => {
    const div = document.createElement("div");
    document.body.append(div);
    const root = createRoot(div);
    const show = (type: string, color?: string) =>
      act(() =>
        root.render(
          <ElementView
            el={{
              id: "e1",
              type,
              x: 0,
              y: 0,
              w: 4,
              h: 2,
              props: { field: "throttle", ...(color ? { color } : {}) },
            }}
            data={EMPTY_PAGE}
            lang="fr"
            u={4}
          />,
        ),
      );
    // Valeur : la couleur du texte suit le réglage sans attendre une trame.
    const text = () => (div.querySelector(".del-v span") as HTMLElement).style.color;
    show("value", "#ff0000");
    const red = text();
    expect(red).not.toBe("");
    show("value", "#00ff00");
    expect(text()).not.toBe("");
    expect(text()).not.toBe(red);
    show("value");
    expect(text()).toBe("");
    // Barre : la couleur de la jauge aussi.
    const fill = () => (div.querySelector(".del-track i") as HTMLElement).style.background;
    show("bar", "#ff0000");
    const redBar = fill();
    show("bar", "#00ff00");
    expect(fill()).not.toBe(redBar);
    act(() => root.unmount());
    div.remove();
  });

  it("an unknown type is shown as such, kept as data", () => {
    const el = { id: "e1", type: "gauge.from-the-future", x: 0, y: 0, w: 4, h: 3, props: {} };
    const { div, unmount } = render(<ElementView el={el} data={EMPTY_PAGE} lang="fr" u={4} />);
    expect(div.textContent).toContain("Élément inconnu");
    unmount();
  });

  it("puts every text of a definition in as text, never as markup", () => {
    const el = {
      id: "e1",
      type: "text",
      x: 0,
      y: 0,
      w: 4,
      h: 1,
      props: { text: "<img src=x onerror=alert(1)>", color: "red;background:url(x)" },
    };
    const { div, unmount } = render(<ElementView el={el} data={EMPTY_PAGE} lang="fr" u={4} />);
    expect(div.querySelector("img")).toBeNull();
    expect(div.textContent).toContain("<img");
    expect(div.innerHTML).not.toContain("url(x)");
    unmount();
  });
});

describe("the dashboard used", () => {
  it("shows the pages of the chosen definition, « Corde » when it is gone", () => {
    act(() => {
      useTelemetry.getState().pushFrame(DEMO.frame);
      useDashes.setState({ defs: [ENDURANCE], selected: "endurance" });
    });
    let r = render(<DashView />);
    expect(r.div.querySelectorAll(".dash-dots button")).toHaveLength(2);
    expect(r.div.querySelector(".dstage")).not.toBeNull();
    expect(useDashPage.getState().pages).toEqual(["p1", "p2"]);
    r.unmount();
    act(() => useDashes.setState({ defs: [], selected: "endurance" }));
    r = render(<DashView />);
    expect(r.div.querySelectorAll(".dash-dots button")).toHaveLength(4);
    expect(r.div.querySelector(".dc")).not.toBeNull();
    r.unmount();
  });
});

describe("undo / redo", () => {
  it("steps back and forth, a new change clears the future", () => {
    const a = ENDURANCE;
    const b = { ...a, name: "B" };
    const c = { ...a, name: "C" };
    let h = commit(commit(historyOf(a), b), c);
    h = undo(h);
    expect(h.present.name).toBe("B");
    h = undo(h);
    expect(h.present).toBe(a);
    expect(undo(h)).toBe(h);
    h = redo(h);
    expect(h.present.name).toBe("B");
    h = commit(h, { ...a, name: "D" });
    expect(h.future).toHaveLength(0);
    expect(commit(h, h.present)).toBe(h);
  });
});

describe("the editor", () => {
  it("adds from the palette, moves with the keys, undoes, keeps a draft", () => {
    act(() => useDashes.setState({ defs: [ENDURANCE], sync: "ready" }));
    const { div, unmount } = render(<DashEditor id="endurance" />);
    const count = () => useEditor.getState().h?.present.pages[0]?.elements.length ?? 0;
    const before = count();
    const add = [...div.querySelectorAll<HTMLButtonElement>(".pal-chip")].find((b) =>
      b.textContent?.startsWith("Texte"),
    );
    act(() => add?.click());
    expect(count()).toBe(before + 1);
    const id = useEditor.getState().selected;
    expect(id).not.toBeNull();
    const pos = () => useEditor.getState().h?.present.pages[0]?.elements.find((e) => e.id === id);
    const x0 = pos()?.x ?? 0;
    const box = div.querySelector<HTMLButtonElement>(".sel-box.is-on");
    act(() => {
      box?.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowRight", bubbles: true }));
    });
    expect(pos()?.x).toBe(Math.min(x0 + 1, 24 - (pos()?.w ?? 1)));
    act(() => {
      window.dispatchEvent(new KeyboardEvent("keydown", { key: "z", ctrlKey: true }));
    });
    expect(pos()?.x).toBe(x0);
    expect(localStorage.getItem("corde.studio.draft.endurance")).toContain('"type":"text"');
    // The unsaved change makes the button say « Enregistrer ».
    expect(div.textContent).toContain("Enregistrer");
    unmount();
    localStorage.removeItem("corde.studio.draft.endurance");
  });

  it("saves through the server and clears the draft", async () => {
    const fetchMock = vi.fn(
      async (_url: string, init?: RequestInit) => new Response(init?.body ?? "{}", { status: 200 }),
    );
    vi.stubGlobal("fetch", fetchMock);
    act(() => useDashes.setState({ defs: [ENDURANCE], sync: "ready", base: "http://pc" }));
    const { div, unmount } = render(<DashEditor id="endurance" />);
    act(() => useEditor.getState().apply((d) => ({ ...d, name: "Endurance 2" })));
    const save = [...div.querySelectorAll<HTMLButtonElement>("button")].find(
      (b) => b.textContent === "Enregistrer",
    );
    await act(async () => {
      save?.click();
    });
    expect(fetchMock).toHaveBeenCalledWith(
      "http://pc/api/dashes/endurance",
      expect.objectContaining({ method: "PUT" }),
    );
    expect(useDashes.getState().defs[0]?.name).toBe("Endurance 2");
    expect(localStorage.getItem("corde.studio.draft.endurance")).toBeNull();
    unmount();
    vi.unstubAllGlobals();
  });
});
