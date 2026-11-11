import { describe, expect, it } from "vitest";
import {
  DEFAULT_OPTIONS,
  fitFactor,
  freeSpot,
  isOnTrack,
  isOverlayHash,
  layoutCanvas,
  OFFTRACK_GRACE_MS,
  optionsQuery,
  overlayCells,
  overlayHash,
  overlayShown,
  overlayUrl,
  parseOverlayOptions,
  parseOverlayTarget,
  recommendedLayoutSize,
  recommendedWidgetSize,
} from "./model";

describe("overlay addresses", () => {
  it("recognises the overlay pages, valid or not", () => {
    expect(isOverlayHash("#/overlay/widget/laps")).toBe(true);
    expect(isOverlayHash("#/overlay/layout/default-overlay?bg=none")).toBe(true);
    expect(isOverlayHash("#/overlay")).toBe(true);
    expect(isOverlayHash("#/overlay/nope")).toBe(true);
    expect(isOverlayHash("#/")).toBe(false);
    expect(isOverlayHash("")).toBe(false);
    expect(isOverlayHash("#/overlays")).toBe(false);
    expect(isOverlayHash("#/obs")).toBe(false);
  });

  it("parses the widget or layout named by the hash", () => {
    expect(parseOverlayTarget("#/overlay/widget/laps")).toEqual({ kind: "widget", id: "laps" });
    expect(parseOverlayTarget("#/overlay/widget/laps/")).toEqual({ kind: "widget", id: "laps" });
    expect(parseOverlayTarget("#/overlay/layout/layout-abc_1?bg=none")).toEqual({
      kind: "layout",
      id: "layout-abc_1",
    });
    expect(parseOverlayTarget("/overlay/layout/x")).toEqual({ kind: "layout", id: "x" });
    for (const bad of [
      "#/overlay",
      "#/overlay/widget",
      "#/overlay/thing/laps",
      "#/overlay/widget/laps/more",
      "#/overlay/layout/..",
      "#/overlay/layout/.hidden",
      "#/overlay/layout/a%2Fb",
      "#/overlay/layout/%E0%A4%A",
      `#/overlay/layout/${"a".repeat(65)}`,
    ]) {
      expect(parseOverlayTarget(bad), bad).toBeNull();
    }
  });

  it("builds the address to paste into OBS, defaults left out", () => {
    const base = "http://127.0.0.1:8471";
    expect(overlayUrl(base, { kind: "widget", id: "inputs" })).toBe(
      "http://127.0.0.1:8471/#/overlay/widget/inputs",
    );
    expect(overlayUrl(`${base}/`, { kind: "layout", id: "default-overlay" })).toBe(
      "http://127.0.0.1:8471/#/overlay/layout/default-overlay",
    );
    expect(
      overlayUrl(
        base,
        { kind: "widget", id: "standings" },
        {
          ...DEFAULT_OPTIONS,
          bg: "none",
          opacity: 0.8500001,
          scale: 1.25,
          hideOffTrack: true,
          lang: "en",
          title: false,
          widget: { mode: "full" },
        },
      ),
    ).toBe(
      "http://127.0.0.1:8471/#/overlay/widget/standings?bg=none&opacity=0.85&scale=1.25&hide=offtrack&lang=en&title=0&w.mode=full",
    );
    expect(overlayHash({ kind: "layout", id: "a b" })).toBe("#/overlay/layout/a%20b");
  });

  it("reads back what it writes", () => {
    const o = {
      ...DEFAULT_OPTIONS,
      bg: "solid" as const,
      opacity: 0.5,
      scale: 2,
      hideOffTrack: true,
      lang: "fr" as const,
      title: false,
      widget: { window: "20" },
    };
    const hash = overlayHash({ kind: "widget", id: "inputs" }, o);
    expect(parseOverlayOptions("", hash)).toEqual(o);
    expect(optionsQuery(DEFAULT_OPTIONS)).toBe("");
  });
});

describe("overlay options", () => {
  it("has quiet defaults", () => {
    expect(parseOverlayOptions("", "#/overlay/widget/laps")).toEqual(DEFAULT_OPTIONS);
    expect(DEFAULT_OPTIONS).toMatchObject({
      bg: "glass",
      opacity: 1,
      scale: 1,
      hideOffTrack: false,
      lang: null,
      title: true,
    });
  });

  it("reads every option, case and spaces tolerated", () => {
    const o = parseOverlayOptions(
      "",
      "#/overlay/widget/laps?bg=NONE&opacity=0,6&scale= 1.5 &hide=offtrack&lang=EN&title=off",
    );
    expect(o).toMatchObject({
      bg: "none",
      opacity: 0.6,
      scale: 1.5,
      hideOffTrack: true,
      lang: "en",
      title: false,
    });
  });

  it("ignores invalid values without failing", () => {
    const o = parseOverlayOptions(
      "",
      "#/overlay/widget/laps?bg=pink&opacity=2&scale=0&hide=always&lang=de&title=maybe&w.=x&w.mode=&w.Bad-Key=1",
    );
    expect(o).toEqual(DEFAULT_OPTIONS);
    for (const q of [
      "opacity=abc",
      "opacity=-1",
      "opacity=0.05",
      "scale=NaN",
      "scale=Infinity",
      "scale=4",
    ]) {
      expect(parseOverlayOptions("", `#/overlay/widget/x?${q}`), q).toEqual(DEFAULT_OPTIONS);
    }
    expect(parseOverlayOptions("", `#/overlay/widget/x?w.mode=${"x".repeat(33)}`).widget).toEqual(
      {},
    );
  });

  it("takes the page's query string too, the hash winning", () => {
    expect(
      parseOverlayOptions("?bg=none&scale=2", "#/overlay/widget/laps?scale=1.5"),
    ).toMatchObject({ bg: "none", scale: 1.5 });
    expect(
      parseOverlayOptions("?hide=offtrack", "#/overlay/widget/laps?hide=none").hideOffTrack,
    ).toBe(false);
  });

  it("keeps widget settings for the widget to check", () => {
    expect(parseOverlayOptions("", "#/overlay/widget/standings?w.mode=full").widget).toEqual({
      mode: "full",
    });
  });
});

describe("advised source size", () => {
  const laps = { defaultSize: { w: 3, h: 6 } };
  it("uses the widget's desktop size on a 1080p screen by default", () => {
    expect(recommendedWidgetSize(laps)).toEqual({ width: 480, height: 384 });
  });
  it("prefers the widget's own overlay size", () => {
    expect(recommendedWidgetSize({ ...laps, overlaySize: { width: 400, height: 400 } })).toEqual({
      width: 400,
      height: 400,
    });
  });
  it("grows with the scale, in even pixels", () => {
    expect(recommendedWidgetSize(laps, 1.5)).toEqual({ width: 720, height: 576 });
    expect(recommendedWidgetSize({ ...laps, overlaySize: { width: 333, height: 201 } }, 1)).toEqual(
      { width: 334, height: 202 },
    );
    expect(recommendedWidgetSize(laps, Number.NaN)).toEqual({ width: 480, height: 384 });
  });
  it("is the canvas for a layout: 1920 × 1080 for an overlay layout", () => {
    const overlay = { device: "overlay" as const, columns: 48, widgets: [] };
    expect(recommendedLayoutSize(overlay)).toEqual({ width: 1920, height: 1080 });
    const desktop = {
      device: "desktop" as const,
      columns: 12,
      widgets: [{ id: "a", kind: "laps", x: 0, y: 12, w: 4, h: 4, config: {} }],
    };
    expect(recommendedLayoutSize(desktop)).toEqual({ width: 1920, height: 1024 });
  });
});

describe("overlay geometry", () => {
  it("puts an overlay layout on 40 px cells", () => {
    expect(layoutCanvas({ device: "overlay", columns: 48, widgets: [] })).toEqual({
      width: 1920,
      height: 1080,
      cellW: 40,
      cellH: 40,
    });
  });
  it("fits the canvas inside the source", () => {
    const c = { width: 1920, height: 1080 };
    expect(fitFactor(c, 1920, 1080)).toBe(1);
    expect(fitFactor(c, 1280, 720)).toBeCloseTo(2 / 3);
    expect(fitFactor(c, 1920, 540)).toBe(0.5);
    expect(fitFactor(c, 0, 0)).toBe(1);
  });
  it("turns pixel sizes into cells of the overlay grid", () => {
    expect(overlayCells({ width: 640, height: 384 }, { w: 3, h: 4 })).toEqual({ w: 16, h: 10 });
    expect(overlayCells({ width: 40, height: 40 }, { w: 3, h: 4 })).toEqual({ w: 3, h: 4 });
    expect(overlayCells({ width: 9999, height: 9999 }, { w: 1, h: 1 })).toEqual({ w: 48, h: 27 });
  });
  it("finds a free place on a bounded grid", () => {
    expect(freeSpot([], 10, 10, 48, 27)).toEqual({ x: 0, y: 0 });
    expect(freeSpot([{ x: 0, y: 0, w: 10, h: 10 }], 10, 10, 48, 27)).toEqual({ x: 10, y: 0 });
    expect(freeSpot([{ x: 0, y: 0, w: 48, h: 20 }], 10, 10, 48, 27)).toBeNull();
    expect(freeSpot([{ x: 0, y: 0, w: 48, h: 17 }], 10, 10, 48, 27)).toEqual({ x: 0, y: 17 });
  });
});

describe("hide=offtrack", () => {
  it("never hides without the option", () => {
    expect(overlayShown(false, false, null, 0)).toBe(true);
  });
  it("shows on track, hides at once when never seen on track", () => {
    expect(overlayShown(true, true, null, 0)).toBe(true);
    expect(overlayShown(true, false, null, 0)).toBe(false);
  });
  it("waits a moment after the driver left the track", () => {
    expect(overlayShown(true, false, 1000, 1000 + OFFTRACK_GRACE_MS - 1)).toBe(true);
    expect(overlayShown(true, false, 1000, 1000 + OFFTRACK_GRACE_MS)).toBe(false);
  });

  const frame = (realtime: boolean, player = true) => ({
    player: player ? {} : null,
    session: { in_realtime: realtime },
  });
  it("is on track when the game runs in real time with a car", () => {
    const live = { connected: true, in_realtime: true };
    expect(isOnTrack({ socket: "open", demo: "off", status: live, frame: frame(true) })).toBe(true);
    // the garage: the game is connected, not in real time
    expect(
      isOnTrack({
        socket: "open",
        demo: "off",
        status: { connected: true, in_realtime: false },
        frame: frame(false),
      }),
    ).toBe(false);
    // no game, no frame, no car, socket down
    expect(
      isOnTrack({
        socket: "open",
        demo: "off",
        status: { connected: false, in_realtime: false },
        frame: frame(true),
      }),
    ).toBe(false);
    expect(isOnTrack({ socket: "open", demo: "off", status: live, frame: null })).toBe(false);
    expect(
      isOnTrack({ socket: "open", demo: "off", status: live, frame: frame(true, false) }),
    ).toBe(false);
    expect(isOnTrack({ socket: "closed", demo: "off", status: live, frame: frame(true) })).toBe(
      false,
    );
    // the demo race (previews) is on track
    expect(isOnTrack({ socket: "closed", demo: "manual", status: null, frame: frame(true) })).toBe(
      true,
    );
  });
});
