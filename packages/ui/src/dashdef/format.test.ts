import { describe, expect, it } from "vitest";
import { buildArchive, importFile } from "./archive";
import { template } from "./builtin";
import {
  cellRatio,
  cellsRoughlySquare,
  cleanFormat,
  defaultRadius,
  gridFor,
  insideShape,
  sameFormat,
  shapePath,
  shapeRadiusCss,
  suggestGrid,
  swapped,
  withPreset,
  withRadius,
  withShape,
  withSize,
} from "./format";
import { exportText, importText } from "./io";
import {
  aspectOf,
  type DashDefinition,
  type DashFormat,
  maxShapeRadius,
  PRESET_IDS,
  PRESETS,
  presetFormat,
  presetOf,
  radiusOf,
  SHAPES,
  shapeOf,
  validateDefinition,
} from "./model";
import { adaptGrids, newDefinition, setFormat, setPreset } from "./ops";
import { boundCache } from "./store";

/**
 * The format of a dashboard and the shape of its page (ADR 0018). The validation cases are the
 * ones of `corde-server/src/dashes.rs` (`page_shapes_are_a_closed_list_with_a_bounded_radius`,
 * `every_preset_is_valid_and_an_unknown_one_is_a_custom_size`, `shapes_in_json`): what one side
 * accepts, the other accepts too.
 */

function dash(format: unknown, over: Partial<DashDefinition> = {}): DashDefinition {
  return {
    v: 1,
    id: "x",
    name: "Dash x",
    format: format as DashFormat,
    theme: {
      background: "#000000",
      panel: "#0d1116",
      line: "#1f2731",
      text: "#f2f5f8",
      dim: "#93a0ae",
      accent: "#ffb454",
    },
    pages: [
      {
        id: "p1",
        name: "Course",
        cols: 24,
        rows: 12,
        elements: [
          { id: "e1", type: "value", x: 0, y: 0, w: 6, h: 3, props: { field: "speed_kmh" } },
          { id: "e2", type: "corde.gear", x: 8, y: 1, w: 7, h: 9, props: {} },
        ],
      },
    ],
    updated_ms: 1,
    ...over,
  };
}

const fmt = (
  preset: string,
  width: number,
  height: number,
  shape?: unknown,
  radius?: unknown,
): Record<string, unknown> => ({
  preset,
  width,
  height,
  ...(shape !== undefined ? { shape } : {}),
  ...(radius !== undefined ? { radius } : {}),
});

const ok = (format: unknown) => validateDefinition(dash(format)).ok;

describe("the shape of the page (same rules as the server)", () => {
  it("is one of a closed list, with a bounded radius", () => {
    const cases: [string, number, number, string | null, number | null, boolean][] = [
      // No shape: a rectangle, as every definition before ADR 0018.
      ["custom", 480, 480, null, null, true],
      ["custom", 480, 480, "rect", null, true],
      // Round: the circle or the ellipse inscribed in the size, no radius.
      ["wheel-round", 480, 480, "round", null, true],
      ["custom", 800, 480, "round", null, true],
      ["custom", 480, 480, "round", 40, false],
      // Rounded corners: a radius from 1 to half the short side.
      ["custom", 800, 480, "rounded", 1, true],
      ["custom", 800, 480, "rounded", 240, true],
      ["custom", 480, 800, "rounded", 240, true],
      ["custom", 101, 300, "rounded", 50, true],
      ["custom", 101, 300, "rounded", 51, false],
      ["custom", 800, 480, "rounded", 241, false],
      ["custom", 800, 480, "rounded", 0, false],
      ["custom", 800, 480, "rounded", null, false],
      // A radius without rounded corners.
      ["custom", 800, 480, null, 20, false],
      ["custom", 800, 480, "rect", 20, false],
      // Not in the list.
      ["custom", 480, 480, "circle", null, false],
      ["custom", 480, 480, "Round", null, false],
      ["custom", 480, 480, "", null, false],
      ["custom", 480, 480, "polygon(0 0)", null, false],
      ["custom", 480, 480, "url(x)", null, false],
      // The whole screen has no shape of its own.
      ["fill", 852, 393, null, null, true],
      ["fill", 852, 393, "rect", null, true],
      ["fill", 852, 393, "round", null, false],
      ["fill", 852, 393, "rounded", 20, false],
      // A shape does not loosen the bounds of the size.
      ["custom", 99, 480, "round", null, false],
      ["custom", 4000, 400, "round", null, false],
    ];
    for (const [preset, w, h, shape, radius, good] of cases) {
      // `null` as in the JSON of the server's cases: the key left out, then the key at `null`.
      const left = fmt(preset, w, h, shape ?? undefined, radius ?? undefined);
      expect(ok(left), JSON.stringify(left)).toBe(good);
      expect(ok({ preset, width: w, height: h, shape, radius }), JSON.stringify(left)).toBe(good);
    }
    expect(maxShapeRadius(800, 480)).toBe(240);
    expect(maxShapeRadius(101, 300)).toBe(50);
    expect(SHAPES).toEqual(["rect", "rounded", "round"]);
  });

  it("every preset is valid; an unknown one is a custom size", () => {
    // The list of `PRESETS` in `corde-server/src/dashes.rs`, in the same order.
    expect(
      PRESET_IDS.map((id) => [
        id,
        PRESETS[id].width,
        PRESETS[id].height,
        PRESETS[id].shape ?? null,
      ]),
    ).toEqual([
      ["phone-landscape", 852, 393, null],
      ["phone-portrait", 393, 852, null],
      ["tablet", 1024, 768, null],
      ["screen-16-9", 1920, 1080, null],
      ["wheel-strip", 800, 240, null],
      ["wheel-round", 480, 480, "round"],
      ["square", 720, 720, null],
      ["screen-5-3", 800, 480, null],
      ["screen-3-2", 480, 320, null],
      ["fill", 852, 393, null],
    ]);
    for (const id of PRESET_IDS) expect(ok(presetFormat(id)), id).toBe(true);
    // A preset of a later version: kept, drawn at its size.
    const later = validateDefinition(dash(fmt("wheel-oval-2030", 600, 400)));
    expect(later.ok && later.def.format).toEqual({
      preset: "wheel-oval-2030",
      width: 600,
      height: 400,
    });
    expect(presetOf({ preset: "wheel-oval-2030" })).toBe("custom");
    expect(presetOf({ preset: "wheel-round" })).toBe("wheel-round");
  });

  it("refuses a shape or a radius that is not the right kind of value", () => {
    for (const bad of [
      fmt("custom", 800, 480, 3),
      fmt("custom", 800, 480, ["round"]),
      fmt("custom", 800, 480, "rounded", 12.5),
      fmt("custom", 800, 480, "rounded", -4),
      fmt("custom", 800, 480, "rounded", "12"),
      fmt("custom", 800, 480, "rounded", 70000),
    ]) {
      expect(ok(bad), JSON.stringify(bad)).toBe(false);
    }
    const r = validateDefinition(dash(fmt("custom", 480, 480, "circle")));
    expect(!r.ok && r.problem.code).toBe("format");
  });

  it("keeps the fields as they came, and none for a definition without a shape", () => {
    const round = validateDefinition(dash(fmt("wheel-round", 480, 480, "round")));
    expect(round.ok && round.def.format).toEqual({
      preset: "wheel-round",
      width: 480,
      height: 480,
      shape: "round",
    });
    const rounded = validateDefinition(dash(fmt("custom", 800, 480, "rounded", 24)));
    expect(rounded.ok && rounded.def.format).toEqual({
      preset: "custom",
      width: 800,
      height: 480,
      shape: "rounded",
      radius: 24,
    });
    // Before ADR 0018: read and written back without the new keys.
    const old = validateDefinition(dash(fmt("phone-landscape", 852, 393)));
    expect(old.ok && Object.keys(old.def.format)).toEqual(["preset", "width", "height"]);
    const nulls = validateDefinition(
      dash({ ...fmt("custom", 800, 480), shape: null, radius: null }),
    );
    expect(nulls.ok && Object.keys(nulls.def.format)).toEqual(["preset", "width", "height"]);
    // Unknown keys of the format are dropped, as everywhere in a definition.
    const extra = validateDefinition(dash({ ...fmt("custom", 800, 480), mask: "url(x)" }));
    expect(extra.ok && Object.keys(extra.def.format)).toEqual(["preset", "width", "height"]);
  });
});

describe("what a reader makes of a format", () => {
  it("draws a rectangle when no shape is named, or for the whole screen", () => {
    expect(shapeOf({ preset: "custom" })).toBe("rect");
    expect(shapeOf({ preset: "custom", shape: "round" })).toBe("round");
    expect(shapeOf({ preset: "custom", shape: "rounded" })).toBe("rounded");
    expect(shapeOf({ preset: "custom", shape: "hexagon" })).toBe("rect");
    expect(shapeOf({ preset: "fill", shape: "round" })).toBe("rect");
    expect(
      radiusOf({ preset: "custom", width: 800, height: 480, shape: "rounded", radius: 24 }),
    ).toBe(24);
    expect(
      radiusOf({ preset: "custom", width: 800, height: 480, shape: "rounded", radius: 999 }),
    ).toBe(240);
    expect(radiusOf({ preset: "custom", width: 800, height: 480, radius: 24 })).toBe(0);
    // The proportions do not depend on the shape.
    expect(aspectOf({ format: presetFormat("wheel-round") })).toBe(1);
    expect(aspectOf({ format: presetFormat("fill") })).toBeNull();
  });

  it("the radius of the clip is in percent of the drawing, the same on any screen", () => {
    expect(shapeRadiusCss(presetFormat("phone-landscape"))).toBeNull();
    expect(shapeRadiusCss(presetFormat("fill"))).toBeNull();
    expect(shapeRadiusCss(presetFormat("wheel-round"))).toBe("50%");
    expect(shapeRadiusCss({ preset: "custom", width: 800, height: 480, shape: "round" })).toBe(
      "50%",
    );
    expect(
      shapeRadiusCss({ preset: "custom", width: 800, height: 480, shape: "rounded", radius: 40 }),
    ).toBe("5% / 8.333%");
    // Half the short side: the ends of the page are half circles.
    expect(
      shapeRadiusCss({ preset: "custom", width: 800, height: 400, shape: "rounded", radius: 200 }),
    ).toBe("25% / 50%");
  });

  it("knows what is inside the shape", () => {
    const round = presetFormat("wheel-round");
    expect(insideShape(round, 240, 240)).toBe(true);
    expect(insideShape(round, 240, 1)).toBe(true);
    expect(insideShape(round, 5, 5)).toBe(false);
    expect(insideShape(round, 475, 475)).toBe(false);
    expect(insideShape(round, 71, 71)).toBe(true); // 240 − 240/√2 ≈ 70.3
    expect(insideShape(round, 69, 69)).toBe(false);
    const oval: DashFormat = { preset: "custom", width: 800, height: 400, shape: "round" };
    expect(insideShape(oval, 400, 2)).toBe(true);
    expect(insideShape(oval, 20, 20)).toBe(false);
    const rounded: DashFormat = {
      preset: "custom",
      width: 800,
      height: 480,
      shape: "rounded",
      radius: 100,
    };
    expect(insideShape(rounded, 5, 5)).toBe(false);
    expect(insideShape(rounded, 30, 30)).toBe(true); // 100 − 100/√2 ≈ 29.3
    expect(insideShape(rounded, 400, 1)).toBe(true);
    expect(insideShape(rounded, 795, 475)).toBe(false);
    const rect = presetFormat("screen-5-3");
    expect(insideShape(rect, 0, 0)).toBe(true);
    expect(insideShape(rect, 801, 10)).toBe(false);
    // The outline drawn in the editor: closed paths in the pixels of the format.
    expect(shapePath(rect)).toBe("M0 0H800V480H0Z");
    expect(shapePath(round)).toBe("M240 0A240 240 0 1 1 240 480A240 240 0 1 1 240 0Z");
    expect(shapePath(rounded)).toContain("A100 100 0 0 1 800 100");
  });
});

describe("changing a format", () => {
  const landscape = presetFormat("phone-landscape");

  it("a preset is its size and its shape; custom keeps both, free to change", () => {
    const round = withPreset(landscape, "wheel-round");
    expect(round).toEqual({ preset: "wheel-round", width: 480, height: 480, shape: "round" });
    expect(withPreset(round, "wheel-round")).toBe(round);
    // To a rectangle preset: the shape goes with the preset left.
    expect(withPreset(round, "screen-5-3")).toEqual({
      preset: "screen-5-3",
      width: 800,
      height: 480,
    });
    expect(withPreset(round, "custom")).toEqual({
      preset: "custom",
      width: 480,
      height: 480,
      shape: "round",
    });
    expect(withPreset(presetFormat("fill"), "custom")).toEqual({
      preset: "custom",
      width: 852,
      height: 393,
    });
  });

  it("a size: any, within the bounds; the shape is kept", () => {
    expect(withSize(landscape, 1000, 500)).toEqual({ preset: "custom", width: 1000, height: 500 });
    expect(withSize(landscape, 852, 393)).toBe(landscape);
    for (const [w, h] of [
      [99, 400],
      [400, 4001],
      [4000, 400],
      [100, 801],
    ]) {
      expect(withSize(landscape, w as number, h as number), `${w}×${h}`).toBe(landscape);
    }
    expect(withSize(landscape, 3200, 400).width).toBe(3200);
    expect(withSize(landscape, 100, 100)).toEqual({ preset: "custom", width: 100, height: 100 });
    expect(withSize(landscape, 4000, 4000).height).toBe(4000);
    const round = presetFormat("wheel-round");
    expect(withSize(round, 600, 600)).toEqual({
      preset: "custom",
      width: 600,
      height: 600,
      shape: "round",
    });
    // A radius is held within the new size.
    const rounded: DashFormat = {
      preset: "custom",
      width: 800,
      height: 480,
      shape: "rounded",
      radius: 240,
    };
    expect(withSize(rounded, 800, 200)).toEqual({
      preset: "custom",
      width: 800,
      height: 200,
      shape: "rounded",
      radius: 100,
    });
  });

  it("swaps width and height", () => {
    expect(swapped(landscape)).toEqual({ preset: "custom", width: 393, height: 852 });
    const round = presetFormat("wheel-round");
    expect(swapped(round)).toBe(round);
    const fill = presetFormat("fill");
    expect(swapped(fill)).toBe(fill);
    const oval: DashFormat = { preset: "custom", width: 800, height: 480, shape: "round" };
    expect(swapped(oval)).toEqual({ preset: "custom", width: 480, height: 800, shape: "round" });
  });

  it("a shape and its radius", () => {
    const five = presetFormat("screen-5-3");
    expect(withShape(five, "rect")).toBe(five);
    expect(withShape(five, "round")).toEqual({
      preset: "custom",
      width: 800,
      height: 480,
      shape: "round",
    });
    const rounded = withShape(five, "rounded");
    expect(rounded).toEqual({
      preset: "custom",
      width: 800,
      height: 480,
      shape: "rounded",
      radius: 40,
    });
    expect(defaultRadius(800, 480)).toBe(40);
    expect(defaultRadius(100, 100)).toBe(8);
    expect(withShape(rounded, "rect")).toEqual({ preset: "custom", width: 800, height: 480 });
    expect(withRadius(rounded, 1000).radius).toBe(240);
    expect(withRadius(rounded, 0).radius).toBe(1);
    expect(withRadius(rounded, 40)).toBe(rounded);
    expect(withRadius(five, 20)).toBe(five);
    expect(withRadius(rounded, Number.NaN)).toBe(rounded);
    // The whole screen has no shape.
    const fill = presetFormat("fill");
    expect(withShape(fill, "round")).toBe(fill);
    // Every format these make is a valid one.
    for (const f of [
      rounded,
      withRadius(rounded, 1000),
      withShape(five, "round"),
      swapped(rounded),
    ]) {
      expect(ok(f), JSON.stringify(f)).toBe(true);
    }
  });

  it("cleans a format: only its keys, no shape for a rectangle", () => {
    expect(cleanFormat({ preset: "custom", width: 480, height: 480, shape: "rect" })).toEqual({
      preset: "custom",
      width: 480,
      height: 480,
    });
    expect(cleanFormat({ preset: "custom", width: 480, height: 480, radius: 30 })).toEqual({
      preset: "custom",
      width: 480,
      height: 480,
    });
    expect(
      cleanFormat({ preset: "custom", width: 480, height: 480, shape: "rounded", radius: 999 }),
    ).toEqual({ preset: "custom", width: 480, height: 480, shape: "rounded", radius: 240 });
    expect(
      sameFormat(presetFormat("square"), {
        preset: "square",
        width: 720,
        height: 720,
        shape: "rect",
      }),
    ).toBe(true);
    expect(
      sameFormat(presetFormat("square"), {
        preset: "square",
        width: 720,
        height: 720,
        shape: "round",
      }),
    ).toBe(false);
  });
});

describe("the format of a dashboard being edited", () => {
  it("changes without touching a single element", () => {
    const d = dash(presetFormat("phone-landscape"));
    const round = setPreset(d, "wheel-round");
    expect(round.format).toEqual({
      preset: "wheel-round",
      width: 480,
      height: 480,
      shape: "round",
    });
    expect(round.pages).toBe(d.pages);
    expect(validateDefinition(round).ok).toBe(true);
    expect(setPreset(round, "wheel-round")).toBe(round);
    const free = setFormat(round, withSize(round.format, 500, 500));
    expect(free.format).toEqual({ preset: "custom", width: 500, height: 500, shape: "round" });
    expect(free.pages).toBe(d.pages);
    expect(setFormat(free, free.format)).toBe(free);
  });

  it("offers a finer grid when the cells are far from square, and only then", () => {
    const d = dash(presetFormat("phone-landscape"));
    const page = d.pages[0] as DashDefinition["pages"][number];
    // 852 × 393 on 24 × 12: cells of 35.5 × 32.75.
    expect(cellRatio(d.format, page)).toBeCloseTo(1.084, 3);
    expect(cellsRoughlySquare(d.format, page)).toBe(true);
    expect(suggestGrid(d.format, page)).toBeNull();
    expect(adaptGrids(d)).toBe(d);
    // The same page on a square: cells twice as tall as wide.
    const round = presetFormat("wheel-round");
    expect(cellRatio(round, page)).toBe(0.5);
    expect(cellsRoughlySquare(round, page)).toBe(false);
    expect(suggestGrid(round, page)).toEqual({ cols: 24, rows: 24 });
    // A wide strip: more columns.
    expect(suggestGrid(presetFormat("wheel-strip"), page)).toEqual({ cols: 48, rows: 12 });
    // Formats of the usual proportions keep the grid.
    for (const id of ["tablet", "screen-16-9", "screen-5-3", "screen-3-2"] as const) {
      expect(suggestGrid(presetFormat(id), page), id).toBeNull();
    }
    // The whole screen has no proportions of its own.
    expect(cellRatio(presetFormat("fill"), page)).toBe(1);
    expect(suggestGrid(presetFormat("fill"), page)).toBeNull();
    // Portrait: the rows cut in two (three would pass the 32 rows); still tall, nothing finer.
    const portrait = presetFormat("phone-portrait");
    expect(suggestGrid(portrait, page)).toEqual({ cols: 24, rows: 24 });
    expect(suggestGrid(portrait, { ...page, rows: 24 })).toBeNull();
  });

  it("adapting the grid moves nothing: every element keeps its place and its size", () => {
    const d = setPreset(dash(presetFormat("phone-landscape")), "wheel-round");
    const next = adaptGrids(d);
    const before = d.pages[0] as DashDefinition["pages"][number];
    const after = next.pages[0] as DashDefinition["pages"][number];
    expect([after.cols, after.rows]).toEqual([24, 24]);
    expect(after.elements).toHaveLength(before.elements.length);
    for (const [i, e] of before.elements.entries()) {
      const a = after.elements[i] as DashDefinition["pages"][number]["elements"][number];
      // The same fractions of the page, exactly.
      expect([a.x / after.cols, a.w / after.cols]).toEqual([e.x / before.cols, e.w / before.cols]);
      expect([a.y / after.rows, a.h / after.rows]).toEqual([e.y / before.rows, e.h / before.rows]);
      expect([a.id, a.type, a.props]).toEqual([e.id, e.type, e.props]);
    }
    expect(validateDefinition(next).ok).toBe(true);
    expect(cellsRoughlySquare(next.format, after)).toBe(true);
    expect(adaptGrids(next)).toBe(next);
  });

  it("an empty page takes the grid of its format", () => {
    expect(gridFor(presetFormat("wheel-round"))).toEqual({ cols: 24, rows: 24 });
    expect(gridFor(presetFormat("phone-portrait"))).toEqual({ cols: 15, rows: 32 });
    expect(gridFor(presetFormat("wheel-strip"))).toEqual({ cols: 24, rows: 7 });
    expect(gridFor({ preset: "custom", width: 3200, height: 400 })).toEqual({ cols: 24, rows: 3 });
    expect(gridFor({ preset: "custom", width: 400, height: 3200 })).toEqual({ cols: 4, rows: 32 });
    const empty = dash(presetFormat("phone-portrait"), {
      pages: [{ id: "p1", name: "Page 1", cols: 24, rows: 12, elements: [] }],
    });
    const next = adaptGrids(empty);
    expect([next.pages[0]?.cols, next.pages[0]?.rows]).toEqual([15, 32]);
    expect(validateDefinition(next).ok).toBe(true);
  });

  it("a new dashboard is created in the format chosen, its pages with a grid that suits it", () => {
    const plain = newDefinition("blank", [], "fr");
    expect(plain.format).toEqual(presetFormat("phone-landscape"));
    expect([plain.pages[0]?.cols, plain.pages[0]?.rows]).toEqual([24, 12]);
    const round = newDefinition("blank", [], "fr", presetFormat("wheel-round"));
    expect(round.format).toEqual({
      preset: "wheel-round",
      width: 480,
      height: 480,
      shape: "round",
    });
    expect([round.pages[0]?.cols, round.pages[0]?.rows]).toEqual([24, 24]);
    expect(validateDefinition(round).ok).toBe(true);
    // A model with elements: the same drawing on a finer grid.
    const model = template("endurance", "fr");
    const endurance = newDefinition("endurance", [], "fr", presetFormat("wheel-round"));
    expect(endurance.pages.map((p) => [p.cols, p.rows])).toEqual([
      [24, 24],
      [24, 24],
    ]);
    expect(endurance.pages[0]?.elements.map((e) => [e.x, e.y / 2, e.w, e.h / 2])).toEqual(
      model.pages[0]?.elements.map((e) => [e.x, e.y, e.w, e.h]),
    );
    expect(validateDefinition(endurance).ok).toBe(true);
    // The same format as the model: nothing changes.
    const same = newDefinition("endurance", [], "fr", presetFormat("phone-landscape"));
    expect(same.pages.map((p) => p.elements)).toEqual(model.pages.map((p) => p.elements));
  });
});

describe("a format with a shape travels with the definition", () => {
  const round = dash(presetFormat("wheel-round"), { id: "rond", name: "Rond" });
  const rounded = dash(
    { preset: "custom", width: 800, height: 480, shape: "rounded", radius: 36 },
    { id: "coins", name: "Coins" },
  );

  it("in a file (.cordedash.json)", () => {
    for (const d of [round, rounded]) {
      const text = exportText(d);
      expect(JSON.parse(text).format).toEqual(d.format);
      const back = importText(text, []);
      expect(back.ok && back.def.format).toEqual(d.format);
    }
    // A file with a shape that is not in the list is refused, with the reason.
    const bad = importText(exportText(round).replace('"round"', '"star"'), []);
    expect(!bad.ok && bad.problem.code).toBe("format");
  });

  it("in an archive (.cordedash)", async () => {
    for (const d of [round, rounded]) {
      const built = await buildArchive(d, async () => null);
      const back = await importFile(built.bytes, []);
      expect(back.ok && back.def.format).toEqual(d.format);
    }
  });

  it("in the copy a device keeps", () => {
    const kept = boundCache([round, rounded]);
    expect(kept.map((d) => d.format)).toEqual(
      expect.arrayContaining([round.format, rounded.format]),
    );
  });
});
