import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { act, type ReactNode } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { DashView } from "../dash/DashView";
import { EMPTY_PAGE } from "../dash/parts";
import { template } from "../dashdef/builtin";
import { type DashDefinition, type DashFormat, presetFormat } from "../dashdef/model";
import { DashPage, fitStage, ShapeMask, Stage } from "../dashdef/Renderer";
import { useDashes } from "../dashdef/store";
import { translate } from "../i18n";
import { en } from "../i18n/en";
import { fr } from "../i18n/fr";
import { useSettings } from "../store/settings";
import { useTelemetry } from "../store/telemetry";
import { DashEditor } from "./Editor";
import { useEditor } from "./editor-store";
import { FormatFields, FormatGlyph, formatWords } from "./Format";
import { DashLibrary } from "./Library";
import { StudioPage } from "./StudioPage";

/**
 * The format of a dashboard and the shape of its page (ADR 0018): what is drawn, what the
 * editor's « Format » control does, how a dashboard is created in a format. The tab of the
 * Studio and its pages under the top bar: `section.test.tsx`.
 */

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

// The stylesheets as they are written (the test environment does not apply them).
const here = dirname(fileURLToPath(import.meta.url));
const css = readFileSync(join(here, "../styles/dashdef.css"), "utf8");
const dashCss = readFileSync(join(here, "../styles/dash.css"), "utf8");

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
const ROUND: DashDefinition = {
  ...template("endurance"),
  id: "rond",
  name: "Rond",
  format: presetFormat("wheel-round"),
  updated_ms: 6,
};
const ROUNDED: DashDefinition = {
  ...ROUND,
  id: "coins",
  format: { preset: "custom", width: 800, height: 480, shape: "rounded", radius: 40 },
};
const WITH_BG: DashDefinition = {
  ...ROUND,
  pages: ROUND.pages.map((p) => ({
    ...p,
    background: { color: "#102030", asset: `${"ab".repeat(32)}.mp4` },
  })),
};

const tr = (key: Parameters<typeof translate>[1], params?: Record<string, string | number>) =>
  translate("fr", key, params);

/** Types in a field the way React hears it. */
function type(input: HTMLInputElement, value: string) {
  const set = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")?.set;
  act(() => {
    set?.call(input, value);
    input.dispatchEvent(new Event("input", { bubbles: true }));
  });
}

function field(div: HTMLElement, label: string): HTMLInputElement {
  const lab = [...div.querySelectorAll("label")].find((l) => l.textContent === label);
  const el = lab ? div.ownerDocument.getElementById(lab.getAttribute("for") ?? "") : null;
  if (!(el instanceof HTMLInputElement)) throw new Error(`no field ${label}`);
  return el;
}

function button(div: HTMLElement, text: string): HTMLButtonElement {
  const b = [...div.querySelectorAll<HTMLButtonElement>("button")].find(
    (x) => x.textContent?.trim() === text || x.getAttribute("aria-label") === text,
  );
  if (!b) throw new Error(`no button ${text}`);
  return b;
}

const format = () => useEditor.getState().h?.present.format;

beforeEach(() => {
  useSettings.setState({ lang: "fr", keepAwake: false });
});

afterEach(() => {
  useTelemetry.getState().resetData();
  useDashes.setState({ defs: [], selected: "corde", sync: "idle", base: null });
  localStorage.clear();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

describe("a format in words", () => {
  it("says the size, and the shape when the page has one", () => {
    expect(formatWords(presetFormat("phone-landscape"), tr)).toBe("852 × 393");
    expect(formatWords(presetFormat("wheel-round"), tr)).toBe("Rond 480 × 480");
    expect(formatWords({ preset: "custom", width: 800, height: 480, shape: "round" }, tr)).toBe(
      "Ovale 800 × 480",
    );
    expect(
      formatWords({ preset: "custom", width: 800, height: 480, shape: "rounded", radius: 40 }, tr),
    ).toBe("Coins arrondis 800 × 480");
    expect(formatWords(presetFormat("fill"), tr)).toBe("Tout l'écran");
    expect(formatWords(presetFormat("wheel-round"), (k, p) => translate("en", k, p))).toBe(
      "Round 480 × 480",
    );
  });

  it("is said on each card of the library and in the bar of the editor", () => {
    act(() => useDashes.setState({ defs: [ENDURANCE, ROUND], sync: "ready" }));
    const lib = render(<StudioPage />);
    const metas = [...lib.div.querySelectorAll(".studio-card-meta")].map((m) => m.textContent);
    expect(metas[0]).toContain("Tout l'écran");
    expect(metas.some((m) => m?.includes("852 × 393"))).toBe(true);
    expect(metas.some((m) => m?.includes("Rond 480 × 480"))).toBe(true);
    lib.unmount();
    const ed = render(<StudioPage id="rond" />);
    const bar = ed.div.querySelector(".studio.is-editor > .studio-bar");
    expect(bar?.querySelector(".fmt-btn")?.textContent).toBe("FormatRond 480 × 480");
    ed.unmount();
  });

  it("no label of a format names a brand: a kind of screen, its size, its shape", () => {
    const keys = (Object.keys(fr) as (keyof typeof fr)[]).filter(
      (k) =>
        k.startsWith("studio.preset.") ||
        k.startsWith("studio.format") ||
        k.startsWith("studio.shape."),
    );
    expect(keys.length).toBeGreaterThan(25);
    for (const k of keys) {
      expect(en[k], k).toBeTruthy();
      expect(`${fr[k]} ${en[k]}`, k).toMatch(/^[\p{L}\p{N}\s.,:;'’«»“”(){}×·…/!?+-]+$/u);
    }
  });
});

describe("the page is clipped to its shape, wherever it is drawn", () => {
  const page = (d: DashDefinition) => d.pages[0] as DashDefinition["pages"][number];

  it("a rectangle (every definition before the shapes) is drawn as before", () => {
    const { div, unmount } = render(
      <Stage def={ENDURANCE} page={page(ENDURANCE)} data={EMPTY_PAGE} lang="fr" />,
    );
    const stage = div.querySelector<HTMLElement>(".dstage");
    expect(stage?.dataset.shape).toBe("rect");
    expect(stage?.style.getPropertyValue("--shape-r")).toBe("");
    expect(div.querySelector(".dstage-wrap.is-shaped")).toBeNull();
    expect(div.querySelector(".dstage-mask")).toBeNull();
    unmount();
  });

  it("round: the circle inscribed in the size, black around", () => {
    const { div, unmount } = render(
      <Stage def={ROUND} page={page(ROUND)} data={EMPTY_PAGE} lang="fr" />,
    );
    const stage = div.querySelector<HTMLElement>(".dstage");
    expect(stage?.dataset.shape).toBe("round");
    expect(stage?.style.getPropertyValue("--shape-r")).toBe("50%");
    expect(div.querySelector(".dstage-wrap.is-shaped")).not.toBeNull();
    // Every element is inside the clipped box, nothing is drawn beside it.
    expect(stage?.querySelectorAll(".del").length).toBe(page(ROUND).elements.length);
    expect(div.querySelectorAll(".dstage-wrap > *")).toHaveLength(1);
    expect(div.querySelector(".dstage-mask")).toBeNull();
    unmount();
  });

  it("rounded corners: the radius follows the size of the drawing", () => {
    const { div, unmount } = render(
      <Stage def={ROUNDED} page={page(ROUNDED)} data={EMPTY_PAGE} lang="fr" />,
    );
    const stage = div.querySelector<HTMLElement>(".dstage");
    expect(stage?.dataset.shape).toBe("rounded");
    expect(stage?.style.getPropertyValue("--shape-r")).toBe("5% / 8.333%");
    unmount();
  });

  it("the background of the page (colour, picture, video) is inside the clipped box too", () => {
    act(() => useDashes.setState({ base: "http://pc:8471" }));
    const { div, unmount } = render(
      <Stage def={WITH_BG} page={page(WITH_BG)} data={EMPTY_PAGE} lang="fr" />,
    );
    const bg = div.querySelector(".dstage[data-shape='round'] > .dstage-bg");
    expect(bg).not.toBeNull();
    expect(bg?.querySelector("video")).not.toBeNull();
    unmount();
  });

  it("the stylesheet clips the shapes, and only outside the editor", () => {
    const rule = css.match(
      /\.dstage\[data-shape="rounded"\]:not\(\.is-editing\),\s*\.dstage\[data-shape="round"\]:not\(\.is-editing\)\s*\{([^}]*)\}/,
    );
    expect(rule?.[1]).toContain("border-radius: var(--shape-r)");
    expect(rule?.[1]).toContain("clip-path: inset(0 round var(--shape-r))");
    expect(css).toMatch(/\.dstage\s*\{[^}]*overflow: hidden/);
    expect(css).toMatch(/\.dstage-wrap\.is-shaped\s*\{[^}]*background: #000/);
  });

  it("the blocks of a drawing never follow the orientation of the screen that shows it", () => {
    // A query on the orientation skips the drawing (a container of its width only) and would
    // be answered by the page around it: that page is not a `dpage` when it holds a drawing.
    expect(css).toMatch(/\.dash-page:has\(> \.dstage-wrap\)\s*\{\s*container-name: dstage-host;/);
    expect(dashCss).toMatch(/\.dash-page\s*\{[^}]*container: dpage \/ size/);
    expect(dashCss).toContain("@container dpage (orientation: portrait)");
  });

  it("with a mouse and in full screen, a format of its own takes the whole window", () => {
    const block = dashCss.slice(dashCss.indexOf(".dash.is-format:not(.is-embedded) .dash-page"));
    expect(block).toMatch(/^\.dash\.is-format:not\(\.is-embedded\) \.dash-page\s*\{\s*padding: 0;/);
    const before = dashCss.slice(
      0,
      dashCss.indexOf(".dash.is-format:not(.is-embedded) .dash-page"),
    );
    expect(before.lastIndexOf("@media (hover: hover) and (pointer: fine)")).toBeGreaterThan(
      before.lastIndexOf("}\n}"),
    );
    expect(dashCss).toMatch(/\.dash\.is-shaped\s*\{\s*background: #000;/);
  });

  it("the same proportions as a rectangle of its size: centred, never stretched", () => {
    expect(fitStage(480, 480, 1)).toMatchObject({ width: 480, height: 480 });
    expect(fitStage(800, 480, 1)).toMatchObject({ width: 480, height: 480 });
    expect(fitStage(480, 800, 1)).toMatchObject({ width: 480, height: 480 });
    expect(fitStage(1280, 720, 800 / 480)).toMatchObject({ width: 1200, height: 720 });
    // The unit of a square drawing: the rule of a page taller than wide or square.
    expect(fitStage(480, 480, 1).u).toBeCloseTo(2.208, 3);
  });

  it("the library, a preview, the dashboard of a device: the same clipped page", () => {
    const thumb = render(<DashPage def={ROUND} page={page(ROUND)} data={EMPTY_PAGE} lang="fr" />);
    expect(thumb.div.querySelector(".dstage[data-shape='round']")).not.toBeNull();
    thumb.unmount();
    act(() => useDashes.setState({ defs: [ROUND], selected: "rond" }));
    const view = render(<DashView />);
    expect(view.div.querySelector(".dash.is-custom.is-format.is-shaped")).not.toBeNull();
    expect(view.div.querySelector(".dash-page .dstage[data-shape='round']")).not.toBeNull();
    view.unmount();
    // A rectangle of the Studio has a format of its own, no shape; « Corde » has neither.
    act(() => useDashes.setState({ defs: [ENDURANCE], selected: "endurance" }));
    const rect = render(<DashView />);
    expect(rect.div.querySelector(".dash")?.className).toBe("dash is-custom is-format");
    rect.unmount();
    act(() => useDashes.setState({ defs: [], selected: "corde" }));
    const corde = render(<DashView />);
    expect(corde.div.querySelector(".dash")?.className).toBe("dash");
    corde.unmount();
  });
});

describe("the shape in the editor", () => {
  it("nothing is clipped: the outside is hatched, the shape outlined, the handles on top", () => {
    act(() => useDashes.setState({ defs: [ROUND], sync: "ready" }));
    const { div, unmount } = render(<DashEditor id="rond" />);
    const stage = div.querySelector<HTMLElement>(".studio-canvas .dstage.is-editing");
    expect(stage?.dataset.shape).toBe("round");
    expect(div.querySelector(".dstage-wrap.is-shaped")).toBeNull();
    const mask = stage?.querySelector(".dstage-mask");
    expect(mask?.getAttribute("viewBox")).toBe("0 0 480 480");
    expect(mask?.getAttribute("aria-hidden")).toBe("true");
    // The outside: the whole page minus the shape (even-odd); the outline: the shape.
    const dim = mask?.querySelector(".dstage-mask-dim");
    expect(dim?.getAttribute("fill-rule")).toBe("evenodd");
    expect(dim?.getAttribute("d")).toBe(
      "M0 0H480V480H0ZM240 0A240 240 0 1 1 240 480A240 240 0 1 1 240 0Z",
    );
    expect(mask?.querySelector(".dstage-mask-edge")?.getAttribute("d")).toBe(
      "M240 0A240 240 0 1 1 240 480A240 240 0 1 1 240 0Z",
    );
    // Drawn after the elements and before the handles: an element can still be placed anywhere.
    const kids = [...(stage?.children ?? [])].map((c) => c.getAttribute("class") ?? "");
    expect(kids.indexOf("dstage-mask")).toBeGreaterThan(kids.lastIndexOf("del"));
    expect(kids.indexOf("sel-layer")).toBeGreaterThan(kids.indexOf("dstage-mask"));
    expect(stage?.querySelectorAll(".sel-box").length).toBe(ROUND.pages[0]?.elements.length);
    unmount();
  });

  it("a rectangle has no mask", () => {
    const { div, unmount } = render(<ShapeMask format={presetFormat("screen-5-3")} />);
    expect(div.querySelector(".dstage-mask-dim")?.getAttribute("d")).toBe(
      "M0 0H800V480H0ZM0 0H800V480H0Z",
    );
    unmount();
    act(() => useDashes.setState({ defs: [ENDURANCE], sync: "ready" }));
    const ed = render(<DashEditor id="endurance" />);
    expect(ed.div.querySelector(".dstage-mask")).toBeNull();
    ed.unmount();
  });
});

describe("the « Format » control of the editor", () => {
  function open() {
    act(() => useDashes.setState({ defs: [ENDURANCE], sync: "ready" }));
    const r = render(<DashEditor id="endurance" />);
    const btn = r.div.querySelector<HTMLButtonElement>(".fmt-btn");
    if (!btn) throw new Error("no format button");
    act(() => btn.click());
    const panel = r.div.querySelector<HTMLElement>(".fmt-panel");
    if (!panel) throw new Error("panel not open");
    return { ...r, btn, panel };
  }

  it("says the current format in words and opens a labelled panel", () => {
    act(() => useDashes.setState({ defs: [ENDURANCE], sync: "ready" }));
    const { div, unmount } = render(<DashEditor id="endurance" />);
    const btn = div.querySelector<HTMLButtonElement>(".fmt-btn");
    expect(btn?.textContent).toBe("Format852 × 393");
    expect(btn?.getAttribute("aria-haspopup")).toBe("dialog");
    expect(btn?.getAttribute("aria-expanded")).toBe("false");
    expect(div.querySelector(".fmt-panel")).toBeNull();
    act(() => btn?.click());
    const panel = div.querySelector(".fmt-panel");
    expect(btn?.getAttribute("aria-expanded")).toBe("true");
    expect(panel?.getAttribute("role")).toBe("dialog");
    expect(panel?.getAttribute("aria-label")).toBe("Format du tableau de bord");
    expect(btn?.getAttribute("aria-controls")).toBe(panel?.id);
    // The presets, each with its shape drawn, its name and its size; the one in use is pressed.
    const presets = [...(panel?.querySelectorAll<HTMLButtonElement>(".fmt-preset") ?? [])];
    expect(presets.map((p) => p.dataset.preset)).toEqual([
      "phone-landscape",
      "phone-portrait",
      "tablet",
      "screen-16-9",
      "wheel-strip",
      "wheel-round",
      "square",
      "screen-5-3",
      "screen-3-2",
      "fill",
      "custom",
    ]);
    expect(presets.filter((p) => p.getAttribute("aria-pressed") === "true")).toHaveLength(1);
    expect(presets[0]?.getAttribute("aria-pressed")).toBe("true");
    expect(presets[5]?.textContent).toBe("Écran rond de volant480 × 480 · rond");
    expect(presets[7]?.textContent).toBe("Petit écran 5:3800 × 480");
    for (const p of presets)
      expect(p.querySelector("svg.fmt-glyph"), p.dataset.preset).not.toBeNull();
    // The keyboard goes to the format in use; every field has its label.
    expect(document.activeElement).toBe(presets[0]);
    expect(field(div, "Largeur").value).toBe("852");
    expect(field(div, "Hauteur").value).toBe("393");
    const shapes = [...(panel?.querySelectorAll<HTMLInputElement>(".fmt-shape input") ?? [])];
    expect(shapes.map((s) => [s.type, s.value, s.checked])).toEqual([
      ["radio", "rect", true],
      ["radio", "rounded", false],
      ["radio", "round", false],
    ]);
    expect(new Set(shapes.map((s) => s.name)).size).toBe(1);
    expect([...(panel?.querySelectorAll(".fmt-shape") ?? [])].map((l) => l.textContent)).toEqual([
      "Rectangle",
      "Coins arrondis",
      "Rond",
    ]);
    expect(button(div, "Inverser largeur et hauteur")).not.toBeNull();
    unmount();
  });

  it("a preset changes the format at once, keeps every element, and « Annuler » goes back", () => {
    const { div, panel, btn, unmount } = open();
    const before = useEditor.getState().h?.present.pages;
    act(() => panel.querySelector<HTMLButtonElement>('[data-preset="wheel-round"]')?.click());
    expect(format()).toEqual({ preset: "wheel-round", width: 480, height: 480, shape: "round" });
    expect(useEditor.getState().h?.present.pages).toBe(before);
    expect(btn.textContent).toBe("FormatRond 480 × 480");
    expect(div.querySelector(".dstage[data-shape='round'] .dstage-mask")).not.toBeNull();
    expect(div.querySelector<HTMLInputElement>(".fmt-shape input[value='round']")?.checked).toBe(
      true,
    );
    // A draft is kept, like any other change.
    expect(localStorage.getItem("corde.studio.draft.endurance")).toContain('"shape":"round"');
    act(() => useEditor.getState().undo());
    expect(format()).toEqual(presetFormat("phone-landscape"));
    expect(btn.textContent).toBe("Format852 × 393");
    act(() => useEditor.getState().redo());
    expect(format()?.shape).toBe("round");
    unmount();
  });

  it("any size: typed freely, applied when it is one, one step of « Annuler » for the entry", () => {
    const { div, unmount } = open();
    const width = field(div, "Largeur");
    const past = () => useEditor.getState().h?.past.length ?? 0;
    const steps = past();
    act(() => width.focus());
    // « 1 », « 12 » are not sizes yet: nothing changes, the field keeps what is typed.
    type(width, "1");
    expect(format()?.width).toBe(852);
    expect(width.value).toBe("1");
    expect(width.getAttribute("aria-invalid")).toBe("true");
    type(width, "12");
    expect(format()?.width).toBe(852);
    type(width, "120");
    expect(format()).toEqual({ preset: "custom", width: 120, height: 393 });
    type(width, "1200");
    expect(format()).toEqual({ preset: "custom", width: 1200, height: 393 });
    expect(width.getAttribute("aria-invalid")).toBeNull();
    // Still one entry: nothing in the history until the field is left.
    expect(past()).toBe(steps);
    act(() => width.blur());
    expect(past()).toBe(steps + 1);
    act(() => useEditor.getState().undo());
    expect(format()).toEqual(presetFormat("phone-landscape"));
    // Out of proportions (8:1 at most): refused, said, the format stays.
    act(() => useEditor.getState().redo());
    const height = field(div, "Hauteur");
    act(() => height.focus());
    type(height, "100");
    expect(format()?.height).toBe(393);
    expect(height.getAttribute("aria-invalid")).toBe("true");
    expect(div.querySelector(".fmt-hint.is-bad")?.textContent).toContain("8 fois");
    act(() => height.blur());
    expect(height.value).toBe("393");
    expect(div.querySelector(".fmt-hint.is-bad")).toBeNull();
    // 4001 is out of bounds.
    act(() => width.focus());
    type(width, "4001");
    expect(format()?.width).toBe(1200);
    act(() => width.blur());
    expect(width.value).toBe("1200");
    unmount();
  });

  it("an entry being typed is never lost when something else starts (a drag, the panel closed)", () => {
    const { div, unmount } = open();
    const past = () => useEditor.getState().h?.past.map((d) => d.format.width) ?? [];
    const width = field(div, "Largeur");
    act(() => width.focus());
    type(width, "1000");
    expect(format()?.width).toBe(1000);
    expect(past()).toEqual([]);
    // A press outside the panel (on the page, where a drag may start): heard first, the entry
    // is a step of its own before the drag begins.
    act(() => {
      div
        .querySelector(".studio-canvas")
        ?.dispatchEvent(new Event("pointerdown", { bubbles: true }));
    });
    expect(div.querySelector(".fmt-panel")).toBeNull();
    expect(past()).toEqual([852]);
    expect(useEditor.getState().gesture).toBeNull();
    // A gesture that begins while another is open closes it as a step too.
    act(() => useEditor.getState().begin());
    act(() =>
      useEditor.getState().preview((d) => ({ ...d, format: { ...d.format, width: 1100 } })),
    );
    act(() => useEditor.getState().begin());
    expect(past()).toEqual([852, 1000]);
    act(() => useEditor.getState().end());
    expect(past()).toEqual([852, 1000]);
    act(() => useEditor.getState().undo());
    act(() => useEditor.getState().undo());
    expect(format()).toEqual(presetFormat("phone-landscape"));
    unmount();
  });

  it("a size typed digit by digit does not shrink the corners on the way", () => {
    act(() => useDashes.setState({ defs: [ROUNDED], sync: "ready" }));
    const { div, unmount } = render(<DashEditor id="coins" />);
    act(() => div.querySelector<HTMLButtonElement>(".fmt-btn")?.click());
    act(() =>
      useEditor.getState().apply((d) => ({
        ...d,
        format: { preset: "custom", width: 800, height: 480, shape: "rounded", radius: 200 },
      })),
    );
    const width = field(div, "Largeur");
    act(() => width.focus());
    // « 100 » is a size (its corners could be 50 at most), then « 1000 ».
    for (const typed of ["1", "10", "100"]) type(width, typed);
    expect(format()).toMatchObject({ width: 100, radius: 50 });
    type(width, "1000");
    expect(format()).toEqual({
      preset: "custom",
      width: 1000,
      height: 480,
      shape: "rounded",
      radius: 200,
    });
    act(() => width.blur());
    unmount();
  });

  it("swaps the orientation, picks a shape and its radius", () => {
    const { div, panel, btn, unmount } = open();
    act(() => button(div, "Inverser largeur et hauteur").click());
    expect(format()).toEqual({ preset: "custom", width: 393, height: 852 });
    expect(panel.querySelector('[data-preset="custom"]')?.getAttribute("aria-pressed")).toBe(
      "true",
    );
    act(() => panel.querySelector<HTMLInputElement>(".fmt-shape input[value='rounded']")?.click());
    expect(format()).toEqual({
      preset: "custom",
      width: 393,
      height: 852,
      shape: "rounded",
      radius: 33,
    });
    expect(btn.textContent).toBe("FormatCoins arrondis 393 × 852");
    const radius = field(div, "Rayon des coins (pixels)");
    expect(radius.value).toBe("33");
    expect(radius.max).toBe("196");
    act(() => radius.focus());
    type(radius, "80");
    act(() => radius.blur());
    expect(format()?.radius).toBe(80);
    type(radius, "500");
    expect(format()?.radius).toBe(80);
    const slider = panel.querySelector<HTMLInputElement>('input[type="range"]');
    expect(slider?.getAttribute("aria-label")).toBe("Rayon des coins (pixels)");
    expect([slider?.min, slider?.max, slider?.value]).toEqual(["1", "196", "80"]);
    act(() => panel.querySelector<HTMLInputElement>(".fmt-shape input[value='round']")?.click());
    expect(format()).toEqual({ preset: "custom", width: 393, height: 852, shape: "round" });
    expect(btn.textContent).toBe("FormatOvale 393 × 852");
    expect(panel.querySelector('input[type="range"]')).toBeNull();
    act(() => panel.querySelector<HTMLInputElement>(".fmt-shape input[value='rect']")?.click());
    expect(format()).toEqual({ preset: "custom", width: 393, height: 852 });
    // The definition is a valid one at every step (it is saved as it is).
    unmount();
  });

  it("the whole screen has neither size nor shape to set", () => {
    const { div, panel, unmount } = open();
    act(() => panel.querySelector<HTMLButtonElement>('[data-preset="fill"]')?.click());
    expect(format()).toEqual(presetFormat("fill"));
    expect(field(div, "Largeur").closest("fieldset")?.disabled).toBe(true);
    expect(panel.querySelector(".fmt-shape input")?.closest("fieldset")?.disabled).toBe(true);
    expect(panel.textContent).toContain("la page prend la taille et la forme de l'écran");
    unmount();
  });

  it("offers to adapt the grid when the cells are far from square — never on its own", () => {
    const { div, panel, unmount } = open();
    expect(panel.querySelector(".fmt-advice")).toBeNull();
    act(() => panel.querySelector<HTMLButtonElement>('[data-preset="wheel-round"]')?.click());
    const grid = () => useEditor.getState().h?.present.pages.map((p) => [p.cols, p.rows]);
    // Nothing was changed: the grid and the elements are as they were.
    expect(grid()).toEqual([
      [24, 12],
      [24, 12],
    ]);
    const advice = panel.querySelector(".fmt-advice");
    expect(advice?.textContent).toContain("les cases de la grille sont allongées sur 2 pages");
    expect(advice?.textContent).toContain("rien n'est supprimé");
    const before = useEditor.getState().h?.present.pages[0]?.elements ?? [];
    act(() => button(div, "Adapter la grille (24 × 24)").click());
    expect(grid()).toEqual([
      [24, 24],
      [24, 24],
    ]);
    const after = useEditor.getState().h?.present.pages[0]?.elements ?? [];
    expect(after.map((e) => [e.id, e.x, e.y / 2, e.w, e.h / 2])).toEqual(
      before.map((e) => [e.id, e.x, e.y, e.w, e.h]),
    );
    expect(panel.querySelector(".fmt-advice")).toBeNull();
    // One step back: the grid of before; one more: the format of before.
    act(() => useEditor.getState().undo());
    expect(grid()?.[0]).toEqual([24, 12]);
    expect(format()?.shape).toBe("round");
    act(() => useEditor.getState().undo());
    expect(format()).toEqual(presetFormat("phone-landscape"));
    unmount();
  });

  it("closes with Échap (the keyboard goes back to the button) and with « Fermer »", () => {
    const { div, btn, unmount } = open();
    act(() => {
      document.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
    });
    expect(div.querySelector(".fmt-panel")).toBeNull();
    expect(document.activeElement).toBe(btn);
    act(() => btn.click());
    act(() => button(div.querySelector(".fmt-panel") as HTMLElement, "Fermer").click());
    expect(div.querySelector(".fmt-panel")).toBeNull();
    // A click outside closes it too.
    act(() => btn.click());
    act(() => {
      document.body.dispatchEvent(new Event("pointerdown", { bubbles: true }));
    });
    expect(div.querySelector(".fmt-panel")).toBeNull();
    unmount();
  });

  it("is reached from the properties too (« Changer le format… »)", () => {
    act(() => useDashes.setState({ defs: [ROUND], sync: "ready" }));
    const { div, unmount } = render(<DashEditor id="rond" />);
    const side = div.querySelector(".studio-side.is-right");
    expect(side?.querySelector(".ins-format")?.textContent).toContain("Rond 480 × 480");
    expect(div.querySelector(".fmt-panel")).toBeNull();
    act(() => button(side as HTMLElement, "Changer le format…").click());
    expect(div.querySelector(".fmt-panel")).not.toBeNull();
    unmount();
  });

  it("is saved with the dashboard", async () => {
    const fetchMock = vi.fn(
      async (_url: string, init?: RequestInit) => new Response(init?.body ?? "{}", { status: 200 }),
    );
    vi.stubGlobal("fetch", fetchMock);
    act(() => useDashes.setState({ defs: [ENDURANCE], sync: "ready", base: "http://pc" }));
    const { div, unmount } = render(<DashEditor id="endurance" />);
    act(() => div.querySelector<HTMLButtonElement>(".fmt-btn")?.click());
    act(() => div.querySelector<HTMLButtonElement>('[data-preset="wheel-round"]')?.click());
    await act(async () => {
      button(div, "Enregistrer").click();
    });
    const body = JSON.parse(String(fetchMock.mock.calls[0]?.[1]?.body));
    expect(body.format).toEqual({ preset: "wheel-round", width: 480, height: 480, shape: "round" });
    expect(useDashes.getState().defs[0]?.format.shape).toBe("round");
    unmount();
  });
});

/** What `FormatFields` told its owner, in order. */
const seen: DashFormat[] = [];
function Host() {
  return <FormatFields format={presetFormat("tablet")} onChange={(f) => seen.push(f)} />;
}

describe("the fields of a format on their own", () => {
  it("tell every change to their owner, a valid format each time", () => {
    const { div, unmount } = render(<Host />);
    act(() => div.querySelector<HTMLButtonElement>('[data-preset="square"]')?.click());
    expect(seen.at(-1)).toEqual({ preset: "square", width: 720, height: 720 });
    unmount();
  });

  it("draws a format small: its proportions and its shape", () => {
    const { div, unmount } = render(
      <>
        <FormatGlyph format={presetFormat("wheel-round")} />
        <FormatGlyph format={presetFormat("fill")} />
      </>,
    );
    const [round, fill] = [...div.querySelectorAll("svg")];
    expect(round?.querySelector("path.is-shape")?.getAttribute("d")).toContain("A240 240");
    expect(fill?.querySelector("rect.is-free")).not.toBeNull();
    expect(round?.getAttribute("aria-hidden")).toBe("true");
    unmount();
  });
});

describe("a new dashboard", () => {
  it("is created in the format chosen, from the model chosen", async () => {
    const fetchMock = vi.fn(
      async (_url: string, init?: RequestInit) => new Response(init?.body ?? "{}", { status: 200 }),
    );
    vi.stubGlobal("fetch", fetchMock);
    act(() => useDashes.setState({ defs: [], sync: "ready", base: "http://pc" }));
    const { div, unmount } = render(<DashLibrary />);
    const dialog = div.querySelector<HTMLDialogElement>("dialog.studio-new");
    expect(dialog?.getAttribute("aria-label")).toBe("Nouveau tableau de bord");
    expect(dialog?.querySelector("form")).toBeNull();
    act(() => button(div, "Nouveau").click());
    // The three models, « Vide » first; the format of the models by default.
    const models = [
      ...(dialog?.querySelectorAll<HTMLInputElement>('.sheet-body input[type="radio"]') ?? []),
    ].filter((r) => ["blank", "corde-blocks", "endurance"].includes(r.value));
    expect(models.map((m) => [m.value, m.checked])).toEqual([
      ["blank", true],
      ["corde-blocks", false],
      ["endurance", false],
    ]);
    expect(
      dialog?.querySelector('[data-preset="phone-landscape"]')?.getAttribute("aria-pressed"),
    ).toBe("true");
    act(() => models[2]?.click());
    act(() => dialog?.querySelector<HTMLButtonElement>('[data-preset="wheel-round"]')?.click());
    const create = dialog?.querySelector<HTMLButtonElement>('button[type="submit"]');
    expect(create?.textContent).toBe("Créer · Rond 480 × 480");
    await act(async () => {
      create?.click();
    });
    expect(fetchMock).toHaveBeenCalledWith(
      "http://pc/api/dashes/endurance",
      expect.objectContaining({ method: "PUT" }),
    );
    const body = JSON.parse(String(fetchMock.mock.calls[0]?.[1]?.body));
    expect(body.format).toEqual({ preset: "wheel-round", width: 480, height: 480, shape: "round" });
    // The pages of the model, on a grid that suits a square.
    expect(body.pages.map((p: { cols: number; rows: number }) => [p.cols, p.rows])).toEqual([
      [24, 24],
      [24, 24],
    ]);
    expect(body.pages[0].elements).toHaveLength(ENDURANCE.pages[0]?.elements.length ?? 0);
    expect(window.location.hash).toBe("#/dash-studio/endurance");
    window.location.hash = "";
    unmount();
  });

  it("« Annuler » creates nothing", () => {
    const fetchMock = vi.fn();
    vi.stubGlobal("fetch", fetchMock);
    act(() => useDashes.setState({ defs: [], sync: "ready", base: "http://pc" }));
    const { div, unmount } = render(<DashLibrary />);
    act(() => button(div, "Nouveau").click());
    const dialog = div.querySelector("dialog.studio-new") as HTMLElement;
    act(() => button(dialog, "Annuler").click());
    expect(dialog.querySelector("form")).toBeNull();
    expect(fetchMock).not.toHaveBeenCalled();
    unmount();
  });
});
