import { describe, expect, it } from "vitest";
import { CORDE, cordeDefinition, TEMPLATE_IDS, template } from "./builtin";
import { ELEMENTS, getElementSpec } from "./catalog";
import { FIELDS, formatValue, getField } from "./fields";
import { exportText, fileName, importText } from "./io";
import { type DashDefinition, MAX_DASH_BYTES, parseDefinition, validateDefinition } from "./model";
import {
  addElement,
  addPage,
  clampRect,
  duplicateDefinition,
  duplicateElement,
  findFreeSpot,
  freshId,
  moveElement,
  movePage,
  newDefinition,
  removeElement,
  removePage,
  resizeElement,
  setGrid,
  slug,
} from "./ops";
import { boundCache, CACHE_MAX, resolveDash } from "./store";

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

function withProps(props: unknown): unknown {
  const d = dash() as unknown as { pages: { elements: { props: unknown }[] }[] };
  (d.pages[0] as { elements: { props: unknown }[] }).elements[0] = {
    ...(d.pages[0]?.elements[0] as object),
    props,
  };
  return d;
}

const ok = (v: unknown) => validateDefinition(v).ok;

describe("validation (same rules as the server)", () => {
  it("accepts a plain definition and the shipped ones", () => {
    expect(ok(dash())).toBe(true);
    expect(ok(CORDE)).toBe(true);
    for (const t of TEMPLATE_IDS) expect(ok({ ...template(t), id: "x" })).toBe(true);
  });

  it("ids", () => {
    for (const bad of ["", "A", "a_b", "a.b", "../x", "a/b", "a b", "é", "x".repeat(41)]) {
      expect(ok(dash({ id: bad })), bad).toBe(false);
    }
    expect(ok(dash({ id: "x".repeat(40) }))).toBe(true);
    const d = dash();
    (d.pages[0] as { id: string }).id = "../p";
    expect(ok(d)).toBe(false);
  });

  it("version, names, author", () => {
    expect(ok(dash({ v: 2 }))).toBe(false);
    for (const bad of [
      "",
      "   ",
      "<b>x</b>",
      "voir https://exemple.org",
      "www.exemple.org",
      "javascript:alert(1)",
      "a\u0000b",
      "n".repeat(61),
    ]) {
      expect(ok(dash({ name: bad })), bad).toBe(false);
    }
    expect(ok(dash({ name: "GT3 · nuit (v2) — Spa", author: "dultra__" }))).toBe(true);
    expect(ok(dash({ author: "x".repeat(61) }))).toBe(false);
  });

  it("format and colours", () => {
    expect(ok(dash({ format: { preset: "custom", width: 99, height: 400 } }))).toBe(false);
    expect(ok(dash({ format: { preset: "custom", width: 400, height: 4001 } }))).toBe(false);
    expect(ok(dash({ format: { preset: "custom", width: 4000, height: 400 } }))).toBe(false);
    expect(ok(dash({ format: { preset: "custom", width: 3200, height: 400 } }))).toBe(true);
    expect(ok(dash({ format: { preset: "Fill!", width: 852, height: 393 } }))).toBe(false);
    for (const bad of ["red", "#fff", "#12345g", "#1234567", "url(x)"]) {
      expect(ok(dash({ theme: { ...dash().theme, accent: bad } })), bad).toBe(false);
    }
  });

  it("pages and elements, each bound", () => {
    const p = dash().pages[0] as DashDefinition["pages"][number];
    expect(ok(dash({ pages: [] }))).toBe(false);
    const twelve = Array.from({ length: 12 }, (_, i) => ({ ...p, id: `p${i}` }));
    expect(ok(dash({ pages: twelve }))).toBe(true);
    expect(ok(dash({ pages: [...twelve, { ...p, id: "p12" }] }))).toBe(false);
    expect(ok(dash({ pages: [p, p] }))).toBe(false);
    const e = p.elements[0] as DashDefinition["pages"][number]["elements"][number];
    const eighty = Array.from({ length: 80 }, (_, i) => ({ ...e, id: `e${i}` }));
    expect(ok(dash({ pages: [{ ...p, elements: eighty }] }))).toBe(true);
    expect(ok(dash({ pages: [{ ...p, elements: [...eighty, { ...e, id: "e80" }] }] }))).toBe(false);
    expect(ok(dash({ pages: [{ ...p, elements: [e, e] }] }))).toBe(false);
    for (const [cols, rows, good] of [
      [4, 2, true],
      [48, 32, true],
      [3, 12, false],
      [49, 12, false],
      [24, 1, false],
      [24, 33, false],
    ] as const) {
      const el = { ...e, w: 1, h: 1 };
      expect(ok(dash({ pages: [{ ...p, cols, rows, elements: [el] }] })), `${cols}×${rows}`).toBe(
        good,
      );
    }
    for (const [x, y, w, h, good] of [
      [18, 9, 6, 3, true],
      [19, 0, 6, 3, false],
      [0, 10, 6, 3, false],
      [0, 0, 0, 3, false],
      [0, 0, 1.5, 3, false],
      [-1, 0, 2, 2, false],
    ] as const) {
      expect(ok(dash({ pages: [{ ...p, elements: [{ ...e, x, y, w, h }] }] }))).toBe(good);
    }
  });

  it("keeps unknown types and settings", () => {
    const d = dash();
    const el = d.pages[0]?.elements[0];
    if (!el) throw new Error();
    el.type = "gauge.round-v9";
    el.props = { futureThing: 3, mode: "x" };
    const c = validateDefinition(d);
    expect(c.ok && c.def.pages[0]?.elements[0]?.type).toBe("gauge.round-v9");
    expect(c.ok && c.def.pages[0]?.elements[0]?.props.futureThing).toBe(3);
  });

  it("settings are flat, bounded, inert", () => {
    expect(ok(withProps({}))).toBe(true);
    expect(ok(withProps({ list: ["a", 1, true, null] }))).toBe(true);
    for (const bad of [
      [],
      "x",
      { nested: { a: 1 } },
      { list: [[1]] },
      { list: Array.from({ length: 17 }, (_, i) => i) },
      { n: 1e7 },
      { n: Number.NaN },
      { n: Number.POSITIVE_INFINITY },
      { color: "red" },
      { color: "#abc" },
      { fillColor: "url(x)" },
      { text: "<img src=x onerror=alert(1)>" },
      { text: "https://exemple.org/logo.png" },
      { text: "data:image/png;base64,AAAA" },
      { text: "x".repeat(201) },
      { "1bad": 1 },
      { "bad-key": 1 },
      Object.fromEntries(Array.from({ length: 33 }, (_, i) => [`k${i}`, i])),
    ]) {
      expect(ok(withProps(bad)), JSON.stringify(bad).slice(0, 60)).toBe(false);
    }
  });

  it("refuses a definition over 256 KiB", () => {
    const big = Object.fromEntries(
      Array.from({ length: 30 }, (_, i) => [`k${i}`, "x".repeat(190)]),
    );
    const p = dash().pages[0] as DashDefinition["pages"][number];
    const els = (n: number) =>
      Array.from({ length: n }, (_, i) => ({
        id: `e${i}`,
        type: "text",
        x: 0,
        y: 0,
        w: 1,
        h: 1,
        props: big,
      }));
    expect(ok(dash({ pages: [{ ...p, elements: els(10) }] }))).toBe(true);
    const r = validateDefinition(dash({ pages: [{ ...p, elements: els(60) }] }));
    expect(r.ok).toBe(false);
    expect(!r.ok && r.problem.code).toBe("too-large");
    expect(MAX_DASH_BYTES).toBe(262144);
  });

  it("drops unknown keys of the definition itself", () => {
    const c = validateDefinition({ ...dash(), device: "PC-de-quelqu-un", server: "192.168.1.2" });
    expect(c.ok && Object.keys(c.def)).not.toContain("device");
  });

  it("malformed text", () => {
    expect(parseDefinition("{ not json").ok).toBe(false);
    const r = parseDefinition("{}");
    expect(!r.ok && r.problem.code).toBe("version");
  });
});

describe("catalogue", () => {
  it("has each block of the pages, the generic elements and sane sizes", () => {
    for (const type of [
      "value",
      "bar",
      "text",
      "panel",
      "clock",
      "corde.rev",
      "corde.gear",
      "corde.delta",
      "corde.tyre",
      "corde.fuel",
      "corde.around",
      "corde.page.course",
    ]) {
      expect(getElementSpec(type), type).toBeDefined();
    }
    for (const e of ELEMENTS) {
      expect(e.min.w <= e.size.w && e.min.h <= e.size.h, e.type).toBe(true);
      expect(e.size.w <= 24 && e.size.h <= 12, e.type).toBe(true);
    }
  });

  it("about sixty fields, unique, each with a label in both languages", () => {
    expect(FIELDS.length).toBeGreaterThanOrEqual(55);
    expect(new Set(FIELDS.map((f) => f.id)).size).toBe(FIELDS.length);
    for (const f of FIELDS) expect(f.label.fr && f.label.en, f.id).toBeTruthy();
  });

  it("a value the game does not give is « — »", () => {
    for (const f of FIELDS) {
      expect(formatValue(f, f.read({ frame: null, laps: [] })), f.id).toBe("—");
    }
    const speed = getField("speed_kmh");
    if (!speed) throw new Error();
    expect(formatValue(speed, 241.6)).toBe("242");
    expect(formatValue(speed, 241.6, 1)).toBe("241.6");
    const delta = getField("delta_best");
    if (!delta) throw new Error();
    expect(delta.color?.(-0.2, { frame: null, laps: [] })).toBe("var(--faster)");
  });
});

describe("grid operations", () => {
  const base = () => dash();

  it("adds at the first free place, within the grid", () => {
    const r = addElement(base(), "p1", "value");
    const el = r.def.pages[0]?.elements.find((e) => e.id === r.id);
    expect(el).toMatchObject({ x: 6, y: 0, w: 4, h: 3 });
    expect(
      findFreeSpot({ ...(base().pages[0] as DashDefinition["pages"][number]) }, 24, 12),
    ).toEqual({
      x: 0,
      y: 0,
    });
    const placed = addElement(base(), "p1", "corde.rev", { x: 10, y: 11 });
    const rev = placed.def.pages[0]?.elements.find((e) => e.id === placed.id);
    expect(rev).toMatchObject({ x: 0, y: 11, w: 24, h: 1 });
  });

  it("moves and resizes by cells, never out of the page nor under the minimum", () => {
    let d = moveElement(base(), "p1", "e1", -3, 2);
    expect(d.pages[0]?.elements[0]).toMatchObject({ x: 0, y: 2 });
    d = moveElement(d, "p1", "e1", 100, 100);
    expect(d.pages[0]?.elements[0]).toMatchObject({ x: 18, y: 9 });
    d = resizeElement(d, "p1", "e1", 10, 10);
    expect(d.pages[0]?.elements[0]).toMatchObject({ w: 6, h: 3 });
    d = resizeElement(d, "p1", "e1", -10, -10);
    expect(d.pages[0]?.elements[0]).toMatchObject({ w: 2, h: 1 });
    const same = base();
    expect(moveElement(same, "p1", "e1", 0, 0)).toBe(same);
  });

  it("duplicates beside, removes", () => {
    const r = duplicateElement(base(), "p1", "e1");
    expect(r.def.pages[0]?.elements.find((e) => e.id === r.id)).toMatchObject({ x: 6, y: 0 });
    expect(removeElement(r.def, "p1", "e1").pages[0]?.elements).toHaveLength(1);
  });

  it("pages: add (≤ 12), move, remove (never the last), grid change keeps elements inside", () => {
    let d = base();
    for (let i = 0; i < 20; i += 1) d = addPage(d, `P${i}`);
    expect(d.pages).toHaveLength(12);
    const second = d.pages[1]?.id as string;
    d = movePage(d, second, -1);
    expect(d.pages[0]?.id).toBe(second);
    let one = base();
    one = removePage(one, "p1");
    expect(one.pages).toHaveLength(1);
    const g = setGrid(moveElement(base(), "p1", "e1", 18, 9), "p1", 8, 4);
    expect(g.pages[0]?.elements[0]).toMatchObject({ x: 2, y: 1, w: 6, h: 3 });
    expect(clampRect({ x: 5, y: 5, w: 50, h: 1 }, 24, 12)).toEqual({ x: 0, y: 5, w: 24, h: 1 });
  });

  it("new ids never collide and never take « corde »", () => {
    expect(slug("GT3 · Nuit à Spa !")).toBe("gt3-nuit-a-spa");
    expect(freshId("Corde", [])).toBe("corde-2");
    expect(freshId("GT3", ["gt3", "gt3-2"])).toBe("gt3-3");
    const d = newDefinition("blank", [], "fr");
    expect(ok(d)).toBe(true);
    const copy = duplicateDefinition(CORDE, [d.id], "fr");
    expect(copy.id).not.toBe("corde");
    expect(copy.name).toBe("Corde (copie)");
    expect(ok(copy)).toBe(true);
  });
});

describe("files", () => {
  it("export holds the definition only, import validates and renames a taken id", () => {
    const d = dash({ author: "dultra__" });
    const text = exportText(d);
    expect(fileName(d)).toBe("gt3.cordedash.json");
    expect(JSON.parse(text)).toEqual(d);
    const again = importText(text, ["gt3"]);
    expect(again.ok && again.def.id).toBe("gt3-2");
    const fresh = importText(text, []);
    expect(fresh.ok && fresh.def.id).toBe("gt3");
    const corde = importText(exportText(cordeDefinition()), []);
    expect(corde.ok && corde.def.id).not.toBe("corde");
    const bad = importText(text.replace('"GT3"', '"<script>x</script>"'), []);
    expect(!bad.ok && bad.problem.code).toBe("text");
    expect(importText("PK\u0003\u0004 not json", []).ok).toBe(false);
  });

  it("an export without author has no author key", () => {
    expect(Object.keys(JSON.parse(exportText(dash())))).not.toContain("author");
  });
});

describe("the dashboard used on a device", () => {
  it("falls back on « Corde » when the selected one is missing or invalid", () => {
    expect(resolveDash([], "gt3", "fr").id).toBe("corde");
    expect(resolveDash([dash()], "gt3", "fr").id).toBe("gt3");
    expect(resolveDash([dash({ theme: { ...dash().theme, text: "white" } })], "gt3", "fr").id).toBe(
      "corde",
    );
    expect(resolveDash([], "corde", "en").pages[0]?.name).toBe("Race");
  });

  it("keeps a bounded copy, valid only, never « corde »", () => {
    const many = Array.from({ length: 30 }, (_, i) => dash({ id: `d${i}`, updated_ms: i }));
    const kept = boundCache([...many, { junk: true }, CORDE]);
    expect(kept).toHaveLength(CACHE_MAX);
    expect(kept[0]?.id).toBe("d29");
    expect(kept.some((d) => d.id === "corde")).toBe(false);
  });
});
