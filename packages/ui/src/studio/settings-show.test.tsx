import type { TelemetryFrame } from "@corde/protocol";
import { act, type ReactNode } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { EMPTY_PAGE, type PageProps } from "../dash/parts";
import { getElementSpec, listElementSpecs, propsWithDefaults } from "../dashdef/catalog";
import { MediaMotion } from "../dashdef/draw";
import type { DashDefinition, DashElement, DashStyle } from "../dashdef/model";
import { ElementView, Stage } from "../dashdef/Renderer";
import { useDashes } from "../dashdef/store";
import { DemoRace } from "../demo/generator";
import { useSettings } from "../store/settings";
import { useTelemetry } from "../store/telemetry";
import { DashEditor } from "./Editor";
import { useEditor } from "./editor-store";

/**
 * The lesson of 1.2.0 (fixed in 1.2.1): every setting shows at once in the editor, the game
 * closed and the demonstration stopped — no telemetry frame needed. Checked here for every
 * element type and every colour or style setting, then the elements of ADR 0016 one by one.
 */

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

function mount() {
  const div = document.createElement("div");
  document.body.append(div);
  const root = createRoot(div);
  return {
    div,
    show: (node: ReactNode) => act(() => root.render(node)),
    unmount: () => {
      act(() => root.unmount());
      div.remove();
    },
  };
}

const HASH = "ab".repeat(32);

function el(type: string, props: Record<string, unknown>, extra: Partial<DashElement> = {}) {
  const spec = getElementSpec(type);
  return {
    id: "e1",
    type,
    x: 0,
    y: 0,
    w: spec?.size.w ?? 4,
    h: spec?.size.h ?? 3,
    props,
    ...extra,
  };
}

beforeEach(() => {
  useSettings.setState({ lang: "fr", keepAwake: false });
});

afterEach(() => {
  useTelemetry.getState().resetData();
  useDashes.setState({ defs: [], selected: "corde", base: null });
  vi.restoreAllMocks();
});

describe("every setting shows at once, without a telemetry frame", () => {
  it("each colour setting of each element type changes the drawing", () => {
    const m = mount();
    let checked = 0;
    for (const spec of listElementSpecs()) {
      const base = propsWithDefaults(spec.type, { field: "throttle" });
      for (const p of spec.props.filter((x) => x.type.kind === "color")) {
        const draw = (c: string) => {
          m.show(
            <ElementView
              el={el(spec.type, { ...base, [p.key]: c })}
              data={EMPTY_PAGE}
              lang="fr"
              u={4}
            />,
          );
          return m.div.innerHTML;
        };
        const a = draw("#ff0000");
        const b = draw("#00ff00");
        expect(b, `${spec.type}.${p.key}`).not.toBe(a);
        checked += 1;
      }
    }
    // Valeur, Barre, Texte, Panneau ×2, Heure, Jauge, Rampe ×4, Bandeau ×2, Forme.
    expect(checked).toBeGreaterThanOrEqual(14);
    m.unmount();
  });

  it("each style setting of each element type changes the drawing", () => {
    const m = mount();
    const styles: [DashStyle, DashStyle][] = [
      [{ bg: "#112233" }, { bg: "#332211" }],
      [{ opacity: 0.5 }, { opacity: 0.2 }],
      [{ radius: 4 }, { radius: 12 }],
      [
        { border: "#ffffff", border_width: 2 },
        { border: "#ff0000", border_width: 2 },
      ],
      [
        { border: "#ffffff", border_width: 2 },
        { border: "#ffffff", border_width: 6 },
      ],
    ];
    for (const spec of listElementSpecs()) {
      for (const [s1, s2] of styles) {
        const draw = (style: DashStyle) => {
          m.show(
            <ElementView el={el(spec.type, {}, { style })} data={EMPTY_PAGE} lang="fr" u={4} />,
          );
          return (m.div.querySelector(".del-in") as HTMLElement).getAttribute("style");
        };
        expect(draw(s2), `${spec.type} ${JSON.stringify(s2)}`).not.toBe(draw(s1));
      }
    }
    m.unmount();
  });

  it("a colour rule shows as soon as it is set, from the data at hand", () => {
    const race = new DemoRace({ seed: 2 });
    let frame: TelemetryFrame | null = null;
    for (let i = 0; i < 200; i += 1) frame = race.step(0.05, true).frame ?? frame;
    act(() => useTelemetry.getState().pushFrame(frame as TelemetryFrame));
    const m = mount();
    const draw = (rules: DashElement["thresholds"]) => {
      m.show(
        <ElementView
          el={el("value", { field: "speed_kmh" }, { thresholds: rules })}
          data={EMPTY_PAGE}
          lang="fr"
          u={4}
        />,
      );
      return (m.div.querySelector(".del-v span") as HTMLElement).style.color;
    };
    const none = draw(undefined);
    const red = draw([{ op: "ge", value: -1000, color: "#ff0000" }]);
    expect(red).not.toBe(none);
    const blue = draw([{ op: "ge", value: -1000, color: "#0000ff" }]);
    expect(blue).not.toBe(red);
    // A rule that does not hold: back to the element's own colour.
    expect(draw([{ op: "lt", value: -1000, color: "#ff0000" }])).toBe(none);
    m.unmount();
  });
});

const PAGE: DashDefinition["pages"][number] = {
  id: "p1",
  name: "Course",
  cols: 24,
  rows: 12,
  elements: [el("alert", { text: "LIMITEUR" }, { show: { when: "limiter" } })],
};

const DEF: DashDefinition = {
  v: 1,
  id: "media",
  name: "Médias",
  format: { preset: "phone-landscape", width: 852, height: 393 },
  theme: {
    background: "#000000",
    panel: "#0d1116",
    line: "#1f2731",
    text: "#f2f5f8",
    dim: "#93a0ae",
    accent: "#ffb454",
  },
  pages: [PAGE],
  updated_ms: 1,
};

describe("« Afficher quand… »", () => {
  it("hides the element on the dashboard, keeps it faint in the editor", () => {
    const m = mount();
    m.show(<Stage def={DEF} page={PAGE} data={EMPTY_PAGE} lang="fr" />);
    expect(m.div.querySelector(".del-in")?.getAttribute("data-off")).toBe("1");
    m.show(<Stage def={DEF} page={PAGE} data={EMPTY_PAGE} lang="fr" editing />);
    expect(m.div.querySelector(".dstage.is-editing .del-in[data-off='1']")).not.toBeNull();
    // The limiter on: shown.
    const race = new DemoRace({ seed: 2 });
    let frame: TelemetryFrame | null = null;
    for (let i = 0; i < 50; i += 1) frame = race.step(0.05, true).frame ?? frame;
    const f = structuredClone(frame) as TelemetryFrame;
    if (f.player) f.player.engine.speed_limiter = true;
    act(() => useTelemetry.getState().pushFrame(f));
    m.show(<Stage def={DEF} page={PAGE} data={EMPTY_PAGE} lang="fr" />);
    expect(m.div.querySelector(".del-in")?.getAttribute("data-off")).toBe("0");
    expect(m.div.textContent).toContain("LIMITEUR");
    m.unmount();
  });
});

describe("the elements of 1.3", () => {
  const data: PageProps = EMPTY_PAGE;

  it("a picture: from the PC by its id, a discreet place when it is not there", () => {
    const m = mount();
    m.show(<ElementView el={el("image", { asset: `${HASH}.png` })} data={data} lang="fr" u={4} />);
    // No server known (PC off): an empty place, no error.
    expect(m.div.querySelector("img")).toBeNull();
    expect(m.div.querySelector(".del-media-missing.is-cited")).not.toBeNull();
    act(() => useDashes.setState({ base: "http://pc:8471" }));
    m.show(<ElementView el={el("image", { asset: `${HASH}.png` })} data={data} lang="fr" u={4} />);
    expect(m.div.querySelector("img")?.getAttribute("src")).toBe(
      `http://pc:8471/api/dash-assets/${HASH}.png`,
    );
    // Never an address of a definition: a text that is not an id draws nothing.
    m.show(
      <ElementView
        el={el("image", { asset: "https://exemple.org/x.png" })}
        data={data}
        lang="fr"
        u={4}
      />,
    );
    expect(m.div.querySelector("img")).toBeNull();
    m.unmount();
  });

  it("a video: muted, looped, no controls; played only on the page shown and the page visible", () => {
    act(() => useDashes.setState({ base: "http://pc:8471" }));
    const play = vi
      .spyOn(HTMLMediaElement.prototype, "play")
      .mockImplementation(() => Promise.resolve());
    const pause = vi.spyOn(HTMLMediaElement.prototype, "pause").mockImplementation(() => {});
    const m = mount();
    const video = el("video", { asset: `${HASH}.webm` });
    m.show(<ElementView el={video} data={{ ...data, active: true }} lang="fr" u={4} />);
    const v = m.div.querySelector("video") as HTMLVideoElement;
    expect(v).not.toBeNull();
    expect(v.muted).toBe(true);
    expect(v.loop).toBe(true);
    expect(v.controls).toBe(false);
    expect(v.hasAttribute("playsinline")).toBe(true);
    expect(play).toHaveBeenCalled();
    // Another page shown: paused.
    pause.mockClear();
    m.show(<ElementView el={video} data={{ ...data, active: false }} lang="fr" u={4} />);
    expect(pause).toHaveBeenCalled();
    // The application sent to the background: paused too.
    m.show(<ElementView el={video} data={{ ...data, active: true }} lang="fr" u={4} />);
    pause.mockClear();
    const state = vi.spyOn(document, "visibilityState", "get").mockReturnValue("hidden");
    act(() => document.dispatchEvent(new Event("visibilitychange")));
    expect(pause).toHaveBeenCalled();
    state.mockRestore();
    // A small preview of the library: its first image only.
    play.mockClear();
    m.show(
      <MediaMotion.Provider value={false}>
        <ElementView el={video} data={{ ...data, active: true }} lang="fr" u={4} />
      </MediaMotion.Provider>,
    );
    expect(m.div.querySelector("video")?.getAttribute("src")).toContain("#t=0.001");
    expect(play).not.toHaveBeenCalled();
    m.unmount();
  });

  it("a page background: its colour, its picture under every element", () => {
    act(() => useDashes.setState({ base: "http://pc:8471" }));
    const page = { ...PAGE, background: { color: "#102030", asset: `${HASH}.jpg`, opacity: 0.5 } };
    const m = mount();
    m.show(<Stage def={{ ...DEF, pages: [page] }} page={page} data={data} lang="fr" />);
    const bg = m.div.querySelector(".dstage-bg") as HTMLElement;
    expect(bg.style.background).not.toBe("");
    expect(bg.querySelector("img")?.getAttribute("src")).toContain(`${HASH}.jpg`);
    expect((bg.querySelector("img") as HTMLElement).style.opacity).toBe("0.5");
    // Drawn first: under the elements.
    expect(m.div.querySelector(".dstage")?.firstElementChild?.className).toBe("dstage-bg");
    m.unmount();
  });

  it("the font of the numbers follows the theme", () => {
    const m = mount();
    m.show(
      <Stage
        def={{ ...DEF, theme: { ...DEF.theme, font: "digital" } }}
        page={PAGE}
        data={data}
        lang="fr"
      />,
    );
    const stage = m.div.querySelector(".dstage") as HTMLElement;
    expect(stage.dataset.font).toBe("digital");
    expect(stage.style.getPropertyValue("--font-num")).toContain("Share Tech Mono");
    m.unmount();
  });

  it("a round gauge, a shape, a banner, a rev ramp of its own", () => {
    const m = mount();
    m.show(
      <ElementView
        el={el("arc", { field: "speed_kmh", color: "#00ff00" })}
        data={data}
        lang="fr"
        u={4}
      />,
    );
    expect(m.div.querySelectorAll(".del-arc path")).toHaveLength(2);
    m.show(
      <ElementView
        el={el("shape", { shape: "circle", fillColor: "#ff0000" })}
        data={data}
        lang="fr"
        u={4}
      />,
    );
    expect(m.div.querySelector(".del-shape.is-circle")).not.toBeNull();
    m.show(<ElementView el={el("alert", { text: "PIT" })} data={data} lang="fr" u={4} />);
    expect(m.div.querySelector(".del-alert")?.textContent).toBe("PIT");
    m.show(<ElementView el={el("rpm", { segments: 8 })} data={data} lang="fr" u={4} />);
    expect(m.div.querySelectorAll(".del-rpm i")).toHaveLength(8);
    m.show(<ElementView el={el("rpm", { segments: 20 })} data={data} lang="fr" u={4} />);
    expect(m.div.querySelectorAll(".del-rpm i")).toHaveLength(20);
    m.unmount();
  });
});

describe("the editor of 1.3", () => {
  const MANY: DashDefinition = {
    ...DEF,
    id: "many",
    pages: [
      {
        ...PAGE,
        elements: [
          el("value", { field: "speed_kmh" }, { id: "a", x: 0, y: 0, w: 4, h: 3 }),
          el("value", { field: "gear" }, { id: "b", x: 6, y: 0, w: 4, h: 3 }),
        ],
      },
      { id: "p2", name: "Deux", cols: 24, rows: 12, elements: [] },
    ],
  };

  it("several elements: Maj + clic, moved together, copied to another page", () => {
    act(() => useDashes.setState({ defs: [MANY], sync: "ready" }));
    const m = mount();
    m.show(<DashEditor id="many" />);
    const boxes = () => [...m.div.querySelectorAll<HTMLButtonElement>(".sel-box")];
    const down = (b: HTMLElement, shift: boolean) =>
      act(() => {
        b.dispatchEvent(
          new PointerEvent("pointerdown", { bubbles: true, button: 0, shiftKey: shift }),
        );
      });
    down(boxes()[0] as HTMLElement, true);
    down(boxes()[1] as HTMLElement, true);
    expect(useEditor.getState().selection).toEqual(["a", "b"]);
    expect(m.div.textContent).toContain("2 éléments sélectionnés");
    act(() => {
      boxes()[0]?.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowDown", bubbles: true }));
    });
    const els = () => useEditor.getState().h?.present.pages[0]?.elements ?? [];
    expect(els().map((e) => e.y)).toEqual([1, 1]);
    // Copied, then pasted on the second page.
    act(() => {
      window.dispatchEvent(new KeyboardEvent("keydown", { key: "c", ctrlKey: true }));
    });
    act(() => useEditor.getState().setPage("p2"));
    act(() => {
      window.dispatchEvent(new KeyboardEvent("keydown", { key: "v", ctrlKey: true }));
    });
    const two = useEditor.getState().h?.present.pages[1]?.elements ?? [];
    expect(two.map((e) => [e.type, e.x, e.y])).toEqual([
      ["value", 0, 1],
      ["value", 6, 1],
    ]);
    expect(useEditor.getState().selection).toHaveLength(2);
    m.unmount();
    localStorage.removeItem("corde.studio.draft.many");
  });

  it("the search narrows the list of elements", () => {
    act(() => useDashes.setState({ defs: [MANY], sync: "ready" }));
    const m = mount();
    m.show(<DashEditor id="many" />);
    const search = m.div.querySelector<HTMLInputElement>(".pal-search") as HTMLInputElement;
    const set = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")?.set;
    act(() => {
      set?.call(search, "video");
      search.dispatchEvent(new Event("input", { bubbles: true }));
    });
    const chips = [...m.div.querySelectorAll(".pal-chip")].map((c) => c.textContent);
    expect(chips).toHaveLength(1);
    expect(chips[0]).toContain("Vidéo");
    m.unmount();
  });

  it("« Abandonner le brouillon » goes back to the saved version", () => {
    const draft = { ...MANY, name: "Brouillon" };
    localStorage.setItem("corde.studio.draft.many", JSON.stringify(draft));
    act(() => useDashes.setState({ defs: [MANY], sync: "ready" }));
    window.confirm = () => true;
    const m = mount();
    m.show(<DashEditor id="many" />);
    expect(useEditor.getState().h?.present.name).toBe("Brouillon");
    const discard = [...m.div.querySelectorAll<HTMLButtonElement>("button")].find(
      (b) => b.textContent === "Abandonner le brouillon",
    );
    expect(discard).toBeDefined();
    act(() => discard?.click());
    expect(useEditor.getState().h?.present.name).toBe("Médias");
    expect(localStorage.getItem("corde.studio.draft.many")).toBeNull();
    m.unmount();
  });
});
