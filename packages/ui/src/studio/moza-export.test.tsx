import { act, type ReactNode } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { template } from "../dashdef/builtin";
import type { DashDefinition } from "../dashdef/model";
import type { MzdashNote } from "../dashdef/mzdash";
import { useDashes } from "../dashdef/store";
import { translate } from "../i18n";
import { useSettings } from "../store/settings";
import { useTelemetry } from "../store/telemetry";
import { DashLibrary } from "./Library";
import { noteLines } from "./MozaExport";

/**
 * « Exporter pour le volant MOZA » in the library of the Studio (ADR 0021): where the button
 * is, what it hands to the browser, what the dialog says. The file itself: `mzdash.test.ts`.
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

function buttons(div: HTMLElement, text: string): HTMLButtonElement[] {
  return [...div.querySelectorAll<HTMLButtonElement>("button")].filter(
    (x) => x.textContent?.trim() === text,
  );
}

const WHEEL: DashDefinition = { ...template("moza-wheel"), id: "volant-moza", updated_ms: 5 };
const ENDURANCE: DashDefinition = { ...template("endurance"), id: "endurance", updated_ms: 6 };

beforeEach(() => {
  useSettings.setState({ lang: "fr", keepAwake: false });
});

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
  useTelemetry.getState().resetData();
  useDashes.setState({ defs: [], selected: "corde" });
});

describe("the library", () => {
  it("offers the export only for a dashboard in the wheel's format", () => {
    act(() => useDashes.setState({ defs: [ENDURANCE, WHEEL], sync: "ready", base: "http://pc" }));
    const { div, unmount } = render(<DashLibrary />);
    const cards = [...div.querySelectorAll<HTMLElement>(".studio-card")];
    expect(cards.map((c) => c.querySelector("h2")?.textContent)).toEqual([
      "Corde",
      "Endurance",
      "Volant MOZA",
    ]);
    expect(cards.map((c) => buttons(c, "Exporter pour le volant MOZA").length)).toEqual([0, 0, 1]);
    // The ordinary export stays on every card.
    expect(cards.map((c) => buttons(c, "Exporter").length)).toEqual([1, 1, 1]);
    unmount();
  });

  it("hands the file to the browser and says what it holds", async () => {
    const blobs: Blob[] = [];
    vi.spyOn(URL, "createObjectURL").mockImplementation((b) => {
      blobs.push(b as Blob);
      return "blob:corde";
    });
    vi.spyOn(URL, "revokeObjectURL").mockImplementation(() => {});
    const clicked: { download: string; href: string }[] = [];
    vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(function (
      this: HTMLAnchorElement,
    ) {
      clicked.push({ download: this.download, href: this.getAttribute("href") ?? "" });
    });
    const fetchMock = vi.fn();
    vi.stubGlobal("fetch", fetchMock);
    act(() => useDashes.setState({ defs: [WHEEL], sync: "ready", base: "http://pc" }));
    const { div, unmount } = render(<DashLibrary />);
    const dialog = div.querySelector<HTMLDialogElement>("dialog.studio-moza");
    expect(dialog?.getAttribute("aria-label")).toBe("Export pour l'écran du volant MOZA");
    expect(dialog?.open).toBe(false);
    expect(dialog?.textContent).toBe("");

    act(() => buttons(div, "Exporter pour le volant MOZA")[0]?.click());
    expect(clicked).toEqual([{ download: "Volant MOZA.mzdash", href: "blob:corde" }]);
    expect(blobs).toHaveLength(1);
    const file = JSON.parse(await (blobs[0] as Blob).text());
    expect(file.type).toBe("Window.qml");
    expect(file.name).toBe("Volant MOZA");
    expect(file.children).toHaveLength(3);
    expect(file.window.GUID).toMatch(
      /^\{[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}\}$/,
    );
    expect(Math.abs(file.lastModified - Date.now() / 1000)).toBeLessThan(60);
    // Nothing leaves the page: the file is made here and downloaded.
    expect(fetchMock).not.toHaveBeenCalled();

    expect(dialog?.open).toBe(true);
    const total = WHEEL.pages.reduce((n, p) => n + p.elements.length, 0);
    expect(dialog?.querySelector("p")?.textContent).toBe(
      `Fichier « Volant MOZA.mzdash » téléchargé : ${total} élément(s) écrit(s) sur ${total}.`,
    );
    // The whole model is written: nothing to report.
    expect(dialog?.querySelector(".studio-moza-notes")).toBeNull();
    expect(dialog?.querySelectorAll(".studio-moza-how li")).toHaveLength(3);
    expect(dialog?.textContent).toContain("pas de Corde");
    expect(dialog?.textContent).toContain("Expérimental");

    act(() => buttons(dialog as HTMLElement, "Fermer")[0]?.click());
    expect(dialog?.open).toBe(false);
    expect(dialog?.textContent).toBe("");
    unmount();
  });

  it("lists what it left out, each reason once", () => {
    vi.spyOn(URL, "createObjectURL").mockReturnValue("blob:corde");
    vi.spyOn(URL, "revokeObjectURL").mockImplementation(() => {});
    vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(() => {});
    const page = WHEEL.pages[0];
    if (!page) throw new Error("page");
    const mixed: DashDefinition = {
      ...WHEEL,
      id: "mixte",
      name: "Mixte",
      pages: [
        {
          ...page,
          elements: [
            { id: "a", type: "value", x: 0, y: 0, w: 4, h: 2, props: { field: "oil_c" } },
            { id: "b", type: "corde.fuel", x: 4, y: 0, w: 4, h: 4, props: {} },
            { id: "c", type: "corde.fuel", x: 8, y: 0, w: 4, h: 4, props: {} },
            { id: "d", type: "value", x: 12, y: 0, w: 4, h: 2, props: { field: "gear" } },
          ],
        },
      ],
    };
    act(() => useDashes.setState({ defs: [mixed], sync: "ready", base: "http://pc" }));
    const { div, unmount } = render(<DashLibrary />);
    act(() => buttons(div, "Exporter pour le volant MOZA")[0]?.click());
    const dialog = div.querySelector<HTMLDialogElement>("dialog.studio-moza");
    expect(dialog?.querySelector("p")?.textContent).toBe(
      "Fichier « Mixte.mzdash » téléchargé : 1 élément(s) écrit(s) sur 4.",
    );
    expect(
      [...(dialog?.querySelectorAll(".studio-moza-notes li") ?? [])].map((l) => l.textContent),
    ).toEqual([
      "Course · Valeur : le volant ne reçoit pas cette donnée (Huile), laissé de côté",
      "Course · Carburant : pas d'équivalent sur le volant, laissé de côté (× 2)",
    ]);
    unmount();
  });
});

describe("a note", () => {
  const t =
    (lang: "fr" | "en") =>
    (key: Parameters<typeof translate>[1], params?: Record<string, string | number>) =>
      translate(lang, key, params);
  const note = (kind: MzdashNote["kind"], element: string, what: string): MzdashNote => ({
    kind,
    page: "Course",
    element,
    what,
  });

  it("names the page, the element and the reason, in both languages", () => {
    const notes = [
      note("field", "value", "laps_left"),
      note("field", "clock", "clock:local"),
      note("condition", "alert", "rain"),
      note("media", "image", "image"),
      note("media", "background", "image"),
      note("partial", "corde.delta", "block"),
      note("partial", "arc", "arc"),
      note("overflow", "rpm", ""),
      note("element", "widget.inconnu", ""),
    ];
    expect(noteLines(notes, t("fr"), "fr")).toEqual([
      "Course · Valeur : le volant ne reçoit pas cette donnée (Tours restants), laissé de côté",
      "Course · Heure / temps de session : le volant ne reçoit pas cette donnée (cette horloge), laissé de côté",
      "Course · Bandeau d'alerte : sa condition d'affichage porte sur une donnée que le volant ne reçoit pas (Pluie), laissé de côté",
      "Course · Image (photo, logo) : les images et les vidéos ne sont pas transportées par cet export",
      "Course · Fond de la page : les images et les vidéos ne sont pas transportées par cet export",
      "Course · Écart au meilleur tour : écrit sous une forme plus simple",
      "Course · Jauge circulaire : l'arc n'a pas d'équivalent : la valeur seule est écrite",
      "Course · Rampe de régime réglable : plus de place dans le fichier, laissé de côté",
      "Course · widget.inconnu : pas d'équivalent sur le volant, laissé de côté",
    ]);
    const en = noteLines(notes, t("en"), "en");
    expect(en[0]).toBe("Course · Value: the wheel is not given this data (Laps to go), left out");
    expect(en).toHaveLength(notes.length);
    for (const line of en) expect(line).not.toMatch(/studio\.moza|\{what\}/);
  });
});
