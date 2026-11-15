import type { TelemetryFrame } from "@corde/protocol";
import { describe, expect, it } from "vitest";
import { DemoRace } from "../demo/generator";
import { exportText, importText } from "./io";
import {
  type DashCondition,
  type DashDefinition,
  FONTS,
  referencedAssets,
  validAssetId,
  validateDefinition,
} from "./model";
import {
  addElement,
  alignGuides,
  elementsIn,
  moveElements,
  pasteElements,
  removeElements,
  setBackground,
  setFont,
  setShow,
  setStyle,
  setThresholds,
} from "./ops";
import { compare, conditionHolds, numeric, thresholdColor } from "./rules";

/**
 * ADR 0016: the new fields of a definition (style, « afficher quand… », colour rules, page
 * backgrounds, font), the same rules as the server (`corde-server/src/dashes.rs`, same cases).
 */

const HASH = "0123456789abcdef".repeat(4);
const asset = (ext: string) => `${HASH}.${ext}`;

function dash(over: Partial<DashDefinition> = {}): DashDefinition {
  return {
    v: 1,
    id: "gt3",
    name: "GT3",
    format: { preset: "phone-landscape", width: 852, height: 393 },
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
        ],
      },
    ],
    updated_ms: 1,
    ...over,
  };
}

/** A definition with its first element changed. */
function withEl(patch: Record<string, unknown>): unknown {
  const d = dash();
  const p = d.pages[0] as DashDefinition["pages"][number];
  return { ...d, pages: [{ ...p, elements: [{ ...p.elements[0], ...patch }] }] };
}

function withPage(patch: Record<string, unknown>): unknown {
  const d = dash();
  return { ...d, pages: [{ ...d.pages[0], ...patch }] };
}

const ok = (v: unknown) => validateDefinition(v).ok;

describe("media ids", () => {
  it("are a SHA-256 and the extension of a type, nothing else", () => {
    for (const e of ["png", "jpg", "webp", "gif", "mp4", "webm"])
      expect(validAssetId(asset(e))).toBe(true);
    for (const bad of [
      asset("svg"),
      asset("PNG"),
      asset("jpeg"),
      HASH,
      `${"a".repeat(63)}.png`,
      `${"A".repeat(64)}.png`,
      `../${"a".repeat(61)}.png`,
      "con.png",
      `${HASH}.png.exe`,
      "",
      3,
      null,
    ])
      expect(validAssetId(bad), String(bad)).toBe(false);
  });

  it("a setting that cites a media holds an id, never an address or a path", () => {
    for (const good of [
      { asset: asset("png") },
      { asset: asset("webm"), fit: "cover" },
      { asset: "" },
      { asset: null },
      { posterAsset: asset("jpg") },
    ])
      expect(ok(withEl({ props: good })), JSON.stringify(good)).toBe(true);
    for (const bad of [
      { asset: "https://exemple.org/logo.png" },
      { asset: "file:///C:/Users/x/logo.png" },
      { asset: "C:\\Users\\x\\logo.png" },
      { asset: "../dashes/x.json" },
      { asset: "logo.png" },
      { asset: asset("svg") },
      { asset: "data:image/png;base64,AAAA" },
      { asset: 3 },
      { asset: true },
      { posterAsset: "x" },
    ]) {
      const r = validateDefinition(withEl({ props: bad }));
      expect(r.ok, JSON.stringify(bad)).toBe(false);
    }
  });

  it("lists the media a definition cites", () => {
    const d = dash();
    const p = d.pages[0] as DashDefinition["pages"][number];
    const def: DashDefinition = {
      ...d,
      pages: [
        {
          ...p,
          background: { asset: asset("mp4") },
          elements: [
            ...p.elements,
            { id: "e2", type: "image", x: 0, y: 4, w: 4, h: 4, props: { asset: asset("gif") } },
            { id: "e3", type: "image", x: 4, y: 4, w: 4, h: 4, props: { asset: asset("gif") } },
          ],
        },
      ],
    };
    expect(validateDefinition(def).ok).toBe(true);
    expect(referencedAssets(def).sort()).toEqual([asset("gif"), asset("mp4")]);
  });
});

describe("page backgrounds", () => {
  it("a colour, a media, a fit and an opacity, all bounded", () => {
    for (const [bg, good] of [
      [{}, true],
      [{ color: "#000000" }, true],
      [{ opacity: 0 }, true],
      [{ color: "#101820", asset: asset("mp4"), fit: "contain", opacity: 0.5 }, true],
      [{ color: "red" }, false],
      [{ asset: "https://x.org/a.png" }, false],
      [{ asset: asset("svg") }, false],
      [{ fit: "tile" }, false],
      [{ opacity: 1.5 }, false],
      [{ opacity: -0.1 }, false],
    ] as const)
      expect(ok(withPage({ background: bg })), JSON.stringify(bg)).toBe(good);
  });
});

describe("element styles", () => {
  const style = { bg: "#112233", opacity: 0.8, radius: 40, border: "#ffffff", border_width: 8 };
  it("colours #rrggbb, an opacity 0..1, corners ≤ 40, an outline ≤ 8", () => {
    expect(ok(withEl({ style }))).toBe(true);
    for (const bad of [
      { bg: "rgba(0,0,0,0)" },
      { border: "url(x)" },
      { opacity: 2 },
      { opacity: Number.NaN },
      { radius: 41 },
      { radius: 2.5 },
      { border_width: 9 },
    ])
      expect(ok(withEl({ style: { ...style, ...bad } })), JSON.stringify(bad)).toBe(false);
  });
});

describe("« Afficher quand… »", () => {
  it("a closed list, or a field of the catalogue compared with a bounded number", () => {
    for (const c of [
      { when: "limiter" },
      { when: "pits" },
      { when: "yellow" },
      { when: "blue" },
      { when: "field", field: "fuel_laps", op: "lt", value: 2 },
      { when: "field", field: "tyre_temp_fl", op: "ge", value: -1e6 },
    ])
      expect(ok(withEl({ show: c })), JSON.stringify(c)).toBe(true);
    for (const c of [
      { when: "always" },
      { when: "script" },
      { when: "LIMITER" },
      { when: "limiter", field: "rpm" },
      { when: "field", op: "lt", value: 2 },
      { when: "field", field: "fuel_laps", value: 2 },
      { when: "field", field: "fuel_laps", op: "lt" },
      { when: "field", field: "fuel_laps", op: "<", value: 2 },
      { when: "field", field: "fuel_laps", op: "eq", value: 2 },
      { when: "field", field: "fuel laps", op: "lt", value: 2 },
      { when: "field", field: "x.y", op: "lt", value: 2 },
      { when: "field", field: "fuel_laps", op: "lt", value: 1e7 },
      { when: "field", field: "fuel_laps", op: "lt", value: Number.POSITIVE_INFINITY },
    ])
      expect(ok(withEl({ show: c })), JSON.stringify(c)).toBe(false);
  });
});

describe("colour rules", () => {
  const rule = (op: string, value: number, color: string) => ({ op, value, color });
  it("at most 4, a comparison of the list, a bounded number, a #rrggbb colour", () => {
    const four = [
      rule("lt", 1, "#ff0000"),
      rule("le", 2, "#ff8800"),
      rule("ge", 100, "#00ff00"),
      rule("gt", 120, "#0000ff"),
    ];
    expect(ok(withEl({ thresholds: four }))).toBe(true);
    expect(ok(withEl({ thresholds: [...four, rule("lt", 0, "#000000")] }))).toBe(false);
    for (const bad of [
      rule(">", 1, "#ff0000"),
      rule("lt", 1, "red"),
      rule("lt", 1e9, "#ff0000"),
      rule("lt", Number.NaN, "#ff0000"),
    ])
      expect(ok(withEl({ thresholds: [bad] })), JSON.stringify(bad)).toBe(false);
  });

  it("the first rule that holds gives the colour", () => {
    const rules = [rule("lt", 2, "#ff0000"), rule("lt", 5, "#ffb454"), rule("ge", 5, "#00ff00")];
    expect(thresholdColor(rules, 1)).toBe("#ff0000");
    expect(thresholdColor(rules, 3)).toBe("#ffb454");
    expect(thresholdColor(rules, 9)).toBe("#00ff00");
    expect(thresholdColor(rules, null)).toBeNull();
    expect(thresholdColor(rules, "P3")).toBeNull();
    expect(thresholdColor([], 1)).toBeNull();
    expect(thresholdColor(undefined, 1)).toBeNull();
    expect(compare("le", 2, 2)).toBe(true);
    expect(compare("lt", 2, 2)).toBe(false);
    expect(compare("gt", 3, 2)).toBe(true);
    expect(compare("ge", 2, 2)).toBe(true);
    expect(compare("eq", 2, 2)).toBe(false);
    expect(numeric(true)).toBe(1);
    expect(numeric(Number.NaN)).toBeNull();
  });
});

describe("fonts", () => {
  it("a closed list of fonts shipped with Corde", () => {
    for (const f of FONTS) {
      const d = dash();
      expect(ok({ ...d, theme: { ...d.theme, font: f } }), f).toBe(true);
    }
    for (const f of ["Comic Sans", "url(x)", "", "digital2"]) {
      const d = dash();
      expect(ok({ ...d, theme: { ...d.theme, font: f } }), f).toBe(false);
    }
  });
});

describe("a definition of 1.2", () => {
  it("reads the same and is written back without the new fields", () => {
    const d = dash();
    const r = validateDefinition(d);
    expect(r.ok && r.def).toEqual(d);
    const text = exportText(d);
    const file = JSON.parse(text);
    for (const k of ["style", "show", "thresholds"])
      expect(file.pages[0].elements[0]).not.toHaveProperty(k);
    expect(file.pages[0]).not.toHaveProperty("background");
    expect(file.theme).not.toHaveProperty("font");
  });

  it("the new fields survive a clean copy and a file", () => {
    const d = withEl({
      style: { bg: "#112233", radius: 4 },
      show: { when: "field", field: "fuel_laps", op: "lt", value: 2 },
      thresholds: [{ op: "lt", value: 2, color: "#ff0000" }],
    }) as DashDefinition;
    const full: DashDefinition = {
      ...d,
      theme: { ...d.theme, font: "digital" },
      pages: [
        { ...(d.pages[0] as DashDefinition["pages"][number]), background: { color: "#101820" } },
      ],
    };
    const r = validateDefinition(full);
    expect(r.ok && r.def).toEqual(full);
    const back = importText(exportText(full), []);
    expect(back.ok && { ...back.def, updated_ms: 1 }).toEqual(full);
  });
});

// ---------------------------------------------------------------- conditions on frames

function demoFrame(): TelemetryFrame {
  const race = new DemoRace({ seed: 3 });
  let frame: TelemetryFrame | null = null;
  for (let i = 0; i < 400; i += 1) {
    const out = race.step(0.05, i % 20 === 0);
    if (out.frame) frame = out.frame;
  }
  if (!frame) throw new Error("no frame");
  return frame;
}

describe("conditions on the data", () => {
  const base = demoFrame();
  const ctx = (f: TelemetryFrame | null) => ({ frame: f, laps: [] });
  const clone = () =>
    structuredClone(base) as TelemetryFrame & { player: NonNullable<TelemetryFrame["player"]> };

  it("always without a condition, never without a frame", () => {
    expect(conditionHolds(undefined, ctx(null))).toBe(true);
    for (const when of ["limiter", "pits", "yellow", "blue"] as const)
      expect(conditionHolds({ when }, ctx(null))).toBe(false);
  });

  it("the limiter, the pits, the flags", () => {
    const f = clone();
    f.player.engine.speed_limiter = false;
    f.player.in_pits = false;
    f.player.in_garage = false;
    f.player.lap.in_pit_lane = false;
    expect(conditionHolds({ when: "limiter" }, ctx(f))).toBe(false);
    expect(conditionHolds({ when: "pits" }, ctx(f))).toBe(false);
    f.player.engine.speed_limiter = true;
    f.player.lap.in_pit_lane = true;
    expect(conditionHolds({ when: "limiter" }, ctx(f))).toBe(true);
    expect(conditionHolds({ when: "pits" }, ctx(f))).toBe(true);
    f.session.yellow_flag = "none";
    f.session.phase = "green";
    f.session.sector_flags = [0, 0, 0];
    expect(conditionHolds({ when: "yellow" }, ctx(f))).toBe(false);
    f.session.phase = "full_course_yellow";
    expect(conditionHolds({ when: "yellow" }, ctx(f))).toBe(true);
    for (const s of f.standings) s.flag_blue = s.is_player;
    expect(conditionHolds({ when: "blue" }, ctx(f))).toBe(true);
  });

  it("a field compared with a number; an unknown field never holds", () => {
    const f = clone();
    f.player.engine.rpm = 7000;
    const c = (op: "lt" | "ge", value: number): DashCondition => ({
      when: "field",
      field: "rpm",
      op,
      value,
    });
    expect(conditionHolds(c("ge", 6500), ctx(f))).toBe(true);
    expect(conditionHolds(c("lt", 6500), ctx(f))).toBe(false);
    expect(
      conditionHolds({ when: "field", field: "from_the_future", op: "lt", value: 1 }, ctx(f)),
    ).toBe(false);
  });
});

// ---------------------------------------------------------------- operations

describe("operations of 1.3", () => {
  it("style, condition, rules, background and font are set and taken away", () => {
    let d = dash();
    d = setStyle(d, "p1", "e1", { bg: "#112233", radius: 6 });
    expect(d.pages[0]?.elements[0]?.style).toEqual({ bg: "#112233", radius: 6 });
    d = setStyle(d, "p1", "e1", { bg: undefined, radius: undefined });
    expect(d.pages[0]?.elements[0]).not.toHaveProperty("style");
    d = setShow(d, "p1", "e1", { when: "limiter" });
    expect(d.pages[0]?.elements[0]?.show).toEqual({ when: "limiter" });
    d = setShow(d, "p1", "e1", null);
    expect(d.pages[0]?.elements[0]).not.toHaveProperty("show");
    d = setThresholds(d, "p1", "e1", [{ op: "lt", value: 2, color: "#ff0000" }]);
    expect(d.pages[0]?.elements[0]?.thresholds).toHaveLength(1);
    d = setThresholds(d, "p1", "e1", []);
    expect(d.pages[0]?.elements[0]).not.toHaveProperty("thresholds");
    d = setBackground(d, "p1", { color: "#101820", asset: asset("png") });
    expect(d.pages[0]?.background).toEqual({ color: "#101820", asset: asset("png") });
    d = setBackground(d, "p1", { color: undefined, asset: undefined });
    expect(d.pages[0]).not.toHaveProperty("background");
    d = setFont(d, "wide");
    expect(d.theme.font).toBe("wide");
    d = setFont(d, "condensed");
    expect(d.theme).not.toHaveProperty("font");
    expect(validateDefinition(d).ok).toBe(true);
  });

  it("an alert banner comes with its condition (the limiter)", () => {
    const r = addElement(dash(), "p1", "alert", undefined, { text: "LIMITEUR" });
    const el = r.def.pages[0]?.elements.find((e) => e.id === r.id);
    expect(el?.show).toEqual({ when: "limiter" });
    expect(el?.props.text).toBe("LIMITEUR");
    expect(validateDefinition(r.def).ok).toBe(true);
  });

  it("several elements move together, held in the grid", () => {
    let d = dash();
    d = addElement(d, "p1", "value", { x: 10, y: 0 }).def;
    const ids = d.pages[0]?.elements.map((e) => e.id) ?? [];
    const moved = moveElements(d, "p1", ids, 100, 2);
    const els = moved.pages[0]?.elements ?? [];
    // The rightmost one stops at the edge; the other keeps its distance.
    expect(els.map((e) => [e.x, e.y])).toEqual([
      [10, 2],
      [20, 2],
    ]);
    expect(moveElements(d, "p1", ids, -5, 0)).toBe(d);
    expect(removeElements(d, "p1", ids).pages[0]?.elements).toHaveLength(0);
  });

  it("pastes copies with new ids, shifted when the place is taken, within 80", () => {
    const d = dash();
    const src = d.pages[0]?.elements ?? [];
    const r = pasteElements(d, "p1", src);
    const els = r.def.pages[0]?.elements ?? [];
    expect(r.ids).toHaveLength(1);
    expect(r.ids[0]).not.toBe("e1");
    expect(els[1]).toMatchObject({ x: 1, y: 1, type: "value" });
    // On another page (empty there): the same place.
    const two: DashDefinition = {
      ...d,
      pages: [...d.pages, { id: "p2", name: "Deux", cols: 24, rows: 12, elements: [] }],
    };
    const r2 = pasteElements(two, "p2", src);
    expect(r2.def.pages[1]?.elements[0]).toMatchObject({ x: 0, y: 0 });
    const full: DashDefinition = {
      ...d,
      pages: [
        {
          ...(d.pages[0] as DashDefinition["pages"][number]),
          elements: Array.from({ length: 80 }, (_, i) => ({
            id: `e${i}`,
            type: "text",
            x: 0,
            y: 0,
            w: 1,
            h: 1,
            props: {},
          })),
        },
      ],
    };
    expect(pasteElements(full, "p1", src).ids).toHaveLength(0);
  });

  it("a frame picks what it touches; guides show where edges line up", () => {
    let d = dash();
    d = addElement(d, "p1", "value", { x: 10, y: 6 }).def;
    const page = d.pages[0] as DashDefinition["pages"][number];
    expect(elementsIn(page, { x: 0, y: 0, w: 2, h: 2 })).toEqual(["e1"]);
    expect(elementsIn(page, { x: 0, y: 0, w: 24, h: 12 })).toHaveLength(2);
    expect(elementsIn(page, { x: 20, y: 0, w: 4, h: 2 })).toEqual([]);
    // e2 moved to x = 6: its left edge meets the right edge of e1 (x 0..6).
    const g = alignGuides(page, { x: 6, y: 6, w: 4, h: 3 }, new Set(["e2"]));
    expect(g.x).toContain(6);
  });
});
