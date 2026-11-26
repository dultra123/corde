import { describe, expect, it } from "vitest";
import { template } from "./builtin";
import { FIELDS, formatValue, getField } from "./fields";
import {
  type DashCondition,
  type DashDefinition,
  type DashElement,
  type DashThreshold,
  DEFAULT_THEME,
  presetFormat,
  validateDefinition,
} from "./model";
import {
  MOZA_HEIGHT,
  MOZA_WIDTH,
  type MzdashNote,
  mozaHasField,
  mzdashName,
  toMzdash,
} from "./mzdash";

/**
 * ADR 0021: a dashboard of the Studio written for the screen of a MOZA wheel. The file is
 * checked for its shape (the one the maker's editor writes) and its expressions are run the
 * way the wheel's software runs them, over a stand-in for its data, and compared with what
 * the Studio shows for the same values.
 */

const OPTS = {
  lang: "fr" as const,
  now: 1_800_000_000,
  guid: "01234567-89ab-4cde-8f01-23456789abcd",
};

type Node = {
  id: number;
  type: string;
  name: string;
  children: Node[];
  binding: Record<string, { methods: string[]; type: string }>;
  general: Record<string, unknown> & { x: number; y: number; width: number; height: number };
  effect: Record<string, unknown>;
  text?: { text: string; fontColor: { color: string }; fontSize: number; fontWeight: number };
  linearGauge?: Record<string, unknown>;
  [key: string]: unknown;
};

function el(
  type: string,
  props: Record<string, unknown> = {},
  over: Partial<DashElement> = {},
): DashElement {
  return { id: "e1", type, x: 0, y: 0, w: 8, h: 4, props, ...over };
}

function dash(elements: DashElement[], over: Partial<DashDefinition> = {}): DashDefinition {
  return {
    v: 1,
    id: "volant",
    name: "Volant",
    format: presetFormat("wheel-moza"),
    theme: { ...DEFAULT_THEME },
    pages: [
      {
        id: "p1",
        name: "Course",
        cols: 24,
        rows: 8,
        elements: elements.map((e, i) => ({ ...e, id: `e${i + 1}` })),
      },
    ],
    updated_ms: 0,
    ...over,
  };
}

function build(def: DashDefinition, lang: "fr" | "en" = "fr") {
  const r = toMzdash(def, { ...OPTS, lang });
  return { ...r, root: JSON.parse(r.json) as Node & Record<string, unknown> };
}

function walk(n: Node, fn: (n: Node, parent: Node | null) => void, parent: Node | null = null) {
  fn(n, parent);
  for (const c of n.children) walk(c, fn, n);
}

function nodes(root: Node): Node[] {
  const out: Node[] = [];
  walk(root, (n) => out.push(n));
  return out;
}

/** Runs an expression of the file as the wheel's software does: over its `Telemetry`. */
function run(expr: string, data: Record<string, unknown>): unknown {
  const Telemetry = { get: (url: string) => ({ value: data[url.replace("v1/gameData/", "")] }) };
  return new Function("Telemetry", `return (${expr});`)(Telemetry);
}

/** The value of a bound property, the two steps of its chain run in turn. */
function bound(n: Node, prop: string, data: Record<string, unknown>): unknown {
  const b = n.binding[prop];
  if (!b) throw new Error(`no binding ${prop} on ${n.name}`);
  expect(b.type).toBe("METHOD_CHAINING");
  expect(b.methods).toHaveLength(2);
  expect(b.methods[1]).toBe("_result");
  return run(b.methods[0] as string, data);
}

/** A stand-in for the data that answers the same number whatever is asked. */
const every = (v: unknown) => new Proxy({}, { get: () => v }) as Record<string, unknown>;

/** The node that shows the value of the one element of a dashboard. */
function valueNode(def: DashDefinition, lang: "fr" | "en" = "fr"): Node {
  const { root } = build(def, lang);
  const found = nodes(root).filter((n) => n.binding["text.text"]);
  expect(found).toHaveLength(1);
  return found[0] as Node;
}

/** The Studio writes a true minus and a long dash; the wheel, plain ones. */
const plain = (s: string) => s.replace(/−/g, "-").replace(/—/g, "-");

describe("the file", () => {
  const model = { ...template("moza-wheel", "fr"), id: "volant-moza" };
  const { root, json, name, written, total, notes } = build(model);

  it("writes all of the model drawn for the wheel", () => {
    expect(validateDefinition(model).ok).toBe(true);
    expect(model.format).toEqual({ preset: "wheel-moza", width: MOZA_WIDTH, height: MOZA_HEIGHT });
    expect(notes).toEqual([]);
    expect(written).toBe(total);
    expect(total).toBe(model.pages.reduce((n, p) => n + p.elements.length, 0));
    expect(name).toBe("Volant MOZA");
    for (const lang of ["fr", "en"] as const)
      for (const p of template("moza-wheel", lang).pages)
        for (const e of p.elements) {
          const field = e.props?.field;
          if (typeof field === "string") expect(mozaHasField(field), field).toBe(true);
        }
  });

  it("is a window of the wheel's size holding one screen per page", () => {
    expect(root.type).toBe("Window.qml");
    expect(root.id).toBe(0);
    expect(root.version).toBe("1.1.1");
    expect(root.name).toBe("Volant MOZA");
    expect(root.lastModified).toBe(1_800_000_000);
    expect(root.imageResources).toEqual([]);
    expect(root.effect).toEqual({ gravitySensorEnabled: false, rotation: 0 });
    expect(root.general).toMatchObject({ x: 0, y: 0, width: 780, height: 248 });
    expect(root.window).toEqual({
      GUID: "{01234567-89ab-4cde-8f01-23456789abcd}",
      defaultScreenId: 0,
      idealDeviceInfos: [
        {
          deviceId: 16,
          hardwareVersion: "RS21-W17-HW RGB-DU-V11",
          networkId: 1,
          productType: "W18 Display",
        },
        {
          deviceId: 8,
          hardwareVersion: "RS21-W17-HW RGB-DU-V11",
          networkId: 1,
          productType: "W17 Display",
        },
      ],
    });
    expect(root.children.map((s) => [s.type, s.name])).toEqual([
      ["Screen.qml", "Course"],
      ["Screen.qml", "Pneus & freins"],
      ["Screen.qml", "Carburant & énergie"],
    ]);
    for (const s of root.children) {
      expect(s.general).toMatchObject({ x: 0, y: 0, width: 780, height: 248 });
      expect(s.guides).toEqual([]);
      expect(s.image).toEqual({ src: "" });
    }
  });

  it("is written as the maker's editor writes: four spaces, sorted keys, no mark", () => {
    expect(json.startsWith('{\n    "actions": {},\n    "binding": {},\n    "borderStyle": {')).toBe(
      true,
    );
    expect(json.charCodeAt(0)).toBe(0x7b);
    const sortedKeys = (v: unknown): boolean => {
      if (Array.isArray(v)) return v.every(sortedKeys);
      if (v === null || typeof v !== "object") return true;
      const keys = Object.keys(v);
      return keys.join("\n") === [...keys].sort().join("\n") && Object.values(v).every(sortedKeys);
    };
    expect(sortedKeys(root)).toBe(true);
  });

  it("gives every node the blocks its kind carries, and no other", () => {
    const envelope = ["actions", "binding", "borderStyle", "children", "effect", "general"];
    const own: Record<string, string[]> = {
      "Window.qml": ["imageResources", "lastModified", "version", "window"],
      "Screen.qml": ["guides", "image"],
      "Text.qml": ["text", "textEffect"],
      "Rectangle.qml": [],
      "LinearGauge.qml": ["linearGauge"],
    };
    const kinds = new Set<string>();
    for (const n of nodes(root)) {
      kinds.add(n.type);
      const expected = [
        ...envelope,
        "id",
        "macros",
        "name",
        "plugins",
        "type",
        ...(own[n.type] ?? ["?"]),
      ];
      expect(Object.keys(n).sort(), `${n.type} ${n.name}`).toEqual(expected.sort());
      if (n.type === "Window.qml") continue;
      // The shadows: both on a rectangle, the outer one on a text, none elsewhere.
      expect("innerShadow" in n.effect, n.type).toBe(n.type === "Rectangle.qml");
      expect("outerShadow" in n.effect, n.type).toBe(
        n.type === "Rectangle.qml" || n.type === "Text.qml",
      );
      expect(n.effect.opacity).toBe(100);
    }
    expect([...kinds].sort()).toEqual([
      "LinearGauge.qml",
      "Rectangle.qml",
      "Screen.qml",
      "Text.qml",
      "Window.qml",
    ]);
  });

  it("numbers the nodes once each, in order, under the limit", () => {
    const ids = nodes(root).map((n) => n.id);
    expect(ids).toEqual(ids.map((_, i) => i));
    expect(Math.max(...ids)).toBeLessThan(10_000);
  });

  it("keeps everything on the screen, on whole pixels", () => {
    for (const n of nodes(root)) {
      const g = n.general;
      for (const v of [g.x, g.y, g.width, g.height]) expect(Number.isInteger(v), n.name).toBe(true);
      expect(g.x).toBeGreaterThanOrEqual(0);
      expect(g.y).toBeGreaterThanOrEqual(0);
      expect(g.x + g.width, n.name).toBeLessThanOrEqual(MOZA_WIDTH);
      expect(g.y + g.height, n.name).toBeLessThanOrEqual(MOZA_HEIGHT);
      if (n.text) {
        expect(n.text.fontSize).toBeGreaterThanOrEqual(8);
        expect(n.text.fontSize).toBeLessThanOrEqual(g.height);
      }
    }
  });

  it("runs every expression, with data and without", () => {
    const targets = new Set<string>();
    for (const data of [every(42), every(0), every(undefined), every("texte"), every(-3.5)])
      for (const n of nodes(root))
        for (const prop of Object.keys(n.binding)) {
          targets.add(`${n.type} ${prop}`);
          const v = bound(n, prop, data);
          if (prop === "text.text") expect(typeof v, n.name).toBe("string");
          else if (prop === "general.visible") expect(typeof v).toBe("boolean");
          else if (prop === "linearGauge.value") {
            expect(v).toBeGreaterThanOrEqual(0);
            expect(v).toBeLessThanOrEqual(100);
          } else expect(v, `${n.name} ${prop}`).toMatch(/^#[0-9a-fA-F]{6}$/);
        }
    // Only the properties seen bound in dashboards made with the maker's editor.
    expect([...targets].sort()).toEqual([
      "LinearGauge.qml linearGauge.gaugeColor",
      "LinearGauge.qml linearGauge.value",
      "Rectangle.qml general.backgroundColor",
      "Rectangle.qml general.visible",
      "Text.qml general.visible",
      "Text.qml text.fontColor",
      "Text.qml text.text",
    ]);
  });

  it("reads nothing but named data of the game", () => {
    const urls = new Set(json.match(/v1\/gameData\/[^\\"]+/g));
    for (const u of urls) expect(u).toMatch(/^v1\/gameData\/(patch\/)?[A-Za-z_]+(&kpa)?$/);
    expect(urls.size).toBeGreaterThan(20);
    // No address, no script but the expressions, nothing of the PC.
    expect(json).not.toMatch(/https?:|file:|[A-Za-z]:\\\\|import\(|require\(|eval\(/);
  });
});

describe("a value", () => {
  const SAMPLES = [0.004, 0.5, 1, 7, 12.34, 83.456, 123.456, 3725.9];
  /** Read as they are: the wheel's data is in the Studio's own unit. */
  const direct = FIELDS.filter(
    (f) =>
      mozaHasField(f.id) &&
      !["rpm_fraction", "fuel_fraction", "virtual_energy", "battery", "track_name"].includes(
        f.id,
      ) &&
      f.format !== "onoff",
  );

  it("is written as the Studio writes it", () => {
    expect(direct.length).toBeGreaterThan(35);
    for (const f of direct) {
      const n = valueNode(dash([el("value", { field: f.id })]));
      // A gear is a whole number.
      for (const v of f.format === "gear" ? [1, 7] : SAMPLES)
        expect(bound(n, "text.text", every(v)), `${f.id} ${v}`).toBe(plain(formatValue(f, v)));
      expect(bound(n, "text.text", every(undefined)), f.id).toBe("-");
      expect(n.text?.text).toBe("-");
    }
  });

  it("follows the decimals asked for", () => {
    for (const decimals of [0, 1, 2, 3])
      for (const id of ["fuel_l", "delta_best", "gap_behind", "speed_kmh"]) {
        const f = getField(id);
        if (!f) throw new Error(id);
        const n = valueNode(dash([el("value", { field: id, decimals })]));
        expect(bound(n, "text.text", every(12.3456)), `${id} ${decimals}`).toBe(
          plain(formatValue(f, 12.3456, decimals)),
        );
      }
  });

  it("writes the signs, the gears and the places", () => {
    const text = (field: string, v: number, lang: "fr" | "en" = "fr") =>
      bound(valueNode(dash([el("value", { field })]), lang), "text.text", every(v));
    expect(text("delta_best", -0.3124)).toBe("-0.312");
    expect(text("delta_best", 1.04)).toBe("+1.040");
    expect(text("delta_best", 0)).toBe("±0.000");
    expect(text("gap_ahead", 1.26)).toBe("-1.3");
    expect(text("gap_behind", 1.26)).toBe("+1.3");
    expect(text("gap_ahead", 0)).toBe("-");
    expect(text("gear", -1)).toBe("R");
    expect(text("gear", 0)).toBe("N");
    expect(text("gear", 6)).toBe("6");
    expect(text("position", 3)).toBe("P3");
    expect(text("position", 0)).toBe("-");
    expect(text("lap_current", 83.4567)).toBe("1:23.457");
    expect(text("lap_best", 0)).toBe("-");
    expect(text("limiter", 1)).toBe("OUI");
    expect(text("limiter", 1, "en")).toBe("ON");
    expect(text("limiter", 0)).toBe("-");
  });

  it("brings the percentages of the wheel back to the Studio's shares", () => {
    const text = (field: string, data: Record<string, unknown>) =>
      bound(valueNode(dash([el("value", { field })])), "text.text", data);
    expect(text("fuel_fraction", { FuelRemainder: 37 })).toBe("37");
    expect(text("fuel_fraction", { FuelRemainder: 0.6 })).toBe("1");
    expect(text("battery", { BatteryPercent: 82.4 })).toBe("82");
    // Given as a share by some games and as a percentage by others.
    expect(text("virtual_energy", { VirtualEnergy: 0.64 })).toBe("64");
    expect(text("virtual_energy", { VirtualEnergy: 64 })).toBe("64");
    expect(text("rpm_fraction", { Rpm: 6000, MaxRpm: 8000 })).toBe("75");
    expect(text("rpm_fraction", { Rpm: 9000, MaxRpm: 8000 })).toBe("100");
    // No maximum from the game: the share the software computed itself.
    expect(
      text("rpm_fraction", { Rpm: 6000, MaxRpm: 0, CarSettings_CurrentDisplayedRPMPercent: 71 }),
    ).toBe("71");
    expect(text("rpm_fraction", {})).toBe("0");
    expect(text("track_name", { "patch/TrackName": "Spa" })).toBe("Spa");
    expect(text("track_name", {})).toBe("-");
    expect(text("tyre_pressure_fl", { "TyrePressureFL&kpa": 172.4 })).toBe("172");
    expect(text("tyre_temp_rr", { TyreTempRearRight: 88.6 })).toBe("89");
    expect(text("brake_temp_fr", { BrakeTempFrontRight: 412 })).toBe("412");
  });

  it("is labelled, with its unit, and sized to its box", () => {
    const labels = (props: Record<string, unknown>, lang: "fr" | "en" = "fr") =>
      nodes(build(dash([el("value", props)]), lang).root)
        .filter((n) => n.type === "Text.qml" && !n.binding["text.text"])
        .map((n) => n.text?.text);
    expect(labels({ field: "speed_kmh" })).toEqual(["VITESSE (km/h)"]);
    expect(labels({ field: "speed_kmh" }, "en")).toEqual(["SPEED (km/h)"]);
    expect(labels({ field: "speed_kmh", unit: false })).toEqual(["VITESSE"]);
    expect(labels({ field: "water_c" })).toEqual(["EAU"]);
    expect(labels({ field: "fuel_laps" })).toEqual(["AUTONOMIE (tours)"]);
    expect(labels({ field: "fuel_laps" }, "en")).toEqual(["RANGE (laps)"]);
    expect(labels({ field: "tyre_pressure_fl" })).toEqual(["AV G KPA"]);
    expect(labels({ field: "gear", label: "Vitesse engagée" })).toEqual(["VITESSE ENGAGÉE"]);
    // No room for the unit beside the name: the name alone, in smaller letters if need be.
    const narrow = nodes(
      build(dash([el("value", { field: "fuel_laps" }, { w: 3, h: 4 })])).root,
    ).filter((n) => n.type === "Text.qml" && !n.binding["text.text"]);
    expect(narrow.map((n) => n.text?.text)).toEqual(["AUTONOMIE"]);
    expect(narrow[0]?.text?.fontSize).toBeLessThanOrEqual(15);
    expect(narrow[0]?.text?.fontSize).toBeGreaterThanOrEqual(9);
    // Too low for a label: the value alone.
    const low = build(dash([el("value", { field: "gear" }, { h: 1 })])).root;
    expect(nodes(low).filter((n) => n.type === "Text.qml")).toHaveLength(1);
    // A frame by default, none when asked.
    const rects = (props: Record<string, unknown>) =>
      nodes(build(dash([el("value", props)])).root).filter((n) => n.type === "Rectangle.qml")
        .length;
    expect(rects({ field: "gear" })).toBe(1);
    expect(rects({ field: "gear", framed: false })).toBe(0);
  });

  it("takes the colour of its rules, then its own, then the data's", () => {
    const color = (
      props: Record<string, unknown>,
      v: number,
      thresholds?: DashThreshold[],
    ): string => {
      const n = valueNode(dash([el("value", props, thresholds ? { thresholds } : {})]));
      return n.binding["text.fontColor"]
        ? (bound(n, "text.fontColor", every(v)) as string)
        : `#${(n.text?.fontColor.color ?? "").slice(3).toLowerCase()}`;
    };
    // The data's own colours, as in the Studio.
    expect(color({ field: "delta_best" }, -0.2)).toBe("#28d17c");
    expect(color({ field: "delta_best" }, 0.2)).toBe("#ff5a5a");
    expect(color({ field: "delta_best" }, 0)).toBe(DEFAULT_THEME.text);
    expect(color({ field: "delta_best" }, Number.NaN)).toBe(DEFAULT_THEME.text);
    expect(color({ field: "gap_ahead" }, 0.6)).toBe("#28d17c");
    expect(color({ field: "gap_ahead" }, 2.6)).toBe(DEFAULT_THEME.text);
    expect(color({ field: "gap_behind" }, 0.6)).toBe("#ff5a5a");
    expect(color({ field: "lap_best" }, 80)).toBe("#b982ff");
    expect(color({ field: "speed_kmh" }, 80)).toBe(DEFAULT_THEME.text);
    // A colour chosen by the driver wins over the data's own.
    expect(color({ field: "delta_best", color: "#00ffaa" }, 0.2)).toBe("#00ffaa");
    // And a rule wins over both: the first that holds.
    const rules: DashThreshold[] = [
      { op: "ge", value: 110, color: "#ff0000" },
      { op: "ge", value: 95, color: "#ffaa00" },
      { op: "lt", value: 60, color: "#0000ff" },
    ];
    expect(color({ field: "water_c", color: "#00ffaa" }, 120, rules)).toBe("#ff0000");
    expect(color({ field: "water_c", color: "#00ffaa" }, 100, rules)).toBe("#ffaa00");
    expect(color({ field: "water_c", color: "#00ffaa" }, 40, rules)).toBe("#0000ff");
    expect(color({ field: "water_c", color: "#00ffaa" }, 80, rules)).toBe("#00ffaa");
    expect(color({ field: "water_c" }, 80, rules)).toBe(DEFAULT_THEME.text);
  });

  it("is left out, and said so, when the wheel is not given its data", () => {
    const missing = FIELDS.filter((f) => !mozaHasField(f.id)).map((f) => f.id);
    // What the wheel has no sure counterpart for (ADR 0021).
    for (const id of ["oil_c", "steering_deg", "class_position", "gap_leader", "laps_left"])
      expect(missing).toContain(id);
    for (const id of ["tyre_wear_fl", "time_of_day", "sector", "brake_migration"])
      expect(missing).toContain(id);
    const r = build(dash([el("value", { field: "oil_c" }), el("value", { field: "nope" })]));
    expect(r.written).toBe(0);
    expect(r.total).toBe(2);
    expect(r.root.children[0]?.children).toEqual([]);
    expect(r.notes).toEqual<MzdashNote[]>([
      { kind: "field", page: "Course", element: "value", what: "oil_c" },
      { kind: "field", page: "Course", element: "value", what: "nope" },
    ]);
  });
});

describe("the other elements", () => {
  const first = (def: DashDefinition, type: string) => {
    const n = nodes(build(def).root).find((x) => x.type === type);
    if (!n) throw new Error(`no ${type}`);
    return n;
  };

  it("draws a bar as a gauge from 0 to 100, over its track", () => {
    const r = build(dash([el("bar", { field: "throttle" }, { w: 12, h: 2 })]));
    const gauge = first(
      dash([el("bar", { field: "throttle" }, { w: 12, h: 2 })]),
      "LinearGauge.qml",
    );
    expect(gauge.linearGauge).toMatchObject({
      minimum: 0,
      maximum: 100,
      value: 0,
      gaugeOrientation: "Horizontal",
      gaugeAlignment: "AlignLeft",
      gaugeColor: { color: "#FF28D17C", type: "SOLID" },
    });
    expect(bound(gauge, "linearGauge.value", { Throttle: 0.42 })).toBeCloseTo(42);
    expect(bound(gauge, "linearGauge.value", { Throttle: 3 })).toBe(100);
    expect(bound(gauge, "linearGauge.value", {})).toBe(0);
    // The frame, the label, the value, the track and the gauge.
    expect(
      nodes(r.root)
        .map((n) => n.type)
        .slice(2),
    ).toEqual(["Rectangle.qml", "Text.qml", "Text.qml", "Rectangle.qml", "LinearGauge.qml"]);
    const value = nodes(r.root).find((n) => n.binding["text.text"]);
    expect(value && bound(value, "text.text", { Throttle: 0.424 })).toBe("42 %");
    // The gauge sits exactly on its track.
    const track = nodes(r.root)[5];
    expect(track?.general).toMatchObject({
      x: gauge.general.x,
      y: gauge.general.y,
      width: gauge.general.width,
      height: gauge.general.height,
    });
  });

  it("colours a bar by rules on the percentage shown", () => {
    const thresholds: DashThreshold[] = [{ op: "lt", value: 15, color: "#ff0000" }];
    const gauge = first(
      dash([el("bar", { field: "fuel_fraction", color: "#00ff00" }, { thresholds })]),
      "LinearGauge.qml",
    );
    expect(bound(gauge, "linearGauge.gaugeColor", { FuelRemainder: 10 })).toBe("#ff0000");
    expect(bound(gauge, "linearGauge.gaugeColor", { FuelRemainder: 40 })).toBe("#00ff00");
    // Without a colour of its own: the data's (low fuel turns warm, then hot).
    const auto = first(dash([el("bar", { field: "fuel_fraction" })]), "LinearGauge.qml");
    expect(bound(auto, "linearGauge.gaugeColor", { FuelRemainder: 5 })).toBe("#f0503c");
    expect(bound(auto, "linearGauge.gaugeColor", { FuelRemainder: 15 })).toBe("#f2b63c");
    expect(bound(auto, "linearGauge.gaugeColor", { FuelRemainder: 60 })).toBe(DEFAULT_THEME.text);
    // Upright: the same gauge, turned (it fills from the bottom).
    const up = first(
      dash([el("bar", { field: "brake", vertical: true }, { w: 2, h: 8 })]),
      "LinearGauge.qml",
    );
    expect(up.linearGauge).toMatchObject({
      gaugeOrientation: "Vertical",
      gaugeAlignment: "AlignLeft",
    });
    // A bar only shows a share: any other data is refused, as in the Studio.
    expect(build(dash([el("bar", { field: "speed_kmh" })])).notes).toEqual<MzdashNote[]>([
      { kind: "field", page: "Course", element: "bar", what: "speed_kmh" },
    ]);
  });

  it("lights the revs like the Studio's ramp", async () => {
    const { rampLit } = await import("./visuals");
    const segments = 12;
    const r = build(dash([el("rpm", { segments, start: 0.6 }, { w: 24, h: 1 })]));
    const leds = nodes(r.root).filter((n) => n.name.startsWith("Régime"));
    expect(leds).toHaveLength(segments);
    const off = `${DEFAULT_THEME.line}`;
    for (const rpm of [0, 4000, 4801, 5500, 6500, 7000, 7600, 7679, 7700, 7999]) {
      const lit = leds.filter(
        (n) => bound(n, "general.backgroundColor", { Rpm: rpm, MaxRpm: 8000 }) !== off,
      ).length;
      // At the top every light takes the shift colour.
      expect(lit, String(rpm)).toBe(
        rpm / 8000 >= 0.97 ? segments : rampLit(rpm, 8000, segments, 0.6),
      );
    }
    const top = leds.map((n) => bound(n, "general.backgroundColor", { Rpm: 7900, MaxRpm: 8000 }));
    expect(new Set(top)).toEqual(new Set(["#3d8bff"]));
    const mid = leds.map((n) => bound(n, "general.backgroundColor", { Rpm: 7600, MaxRpm: 8000 }));
    expect(mid[0]).toBe("#2fe07f");
    expect(mid[5]).toBe("#ffc93c");
    expect(mid[10]).toBe("#ff4b3e");
    // Side by side, without overlap, across the whole element.
    for (let i = 1; i < leds.length; i += 1) {
      const a = leds[i - 1]?.general;
      const b = leds[i]?.general;
      if (!a || !b) throw new Error("led");
      expect(b.x).toBeGreaterThan(a.x + a.width - 1);
    }
    const last = leds[leds.length - 1]?.general;
    expect((last?.x ?? 0) + (last?.width ?? 0)).toBeGreaterThan(MOZA_WIDTH - 6);
  });

  it("shows an element only when its condition holds", () => {
    const show = (c: DashCondition, data: Record<string, unknown>) => {
      const r = build(dash([el("alert", { text: "Limiteur" }, { show: c })]));
      const all = nodes(r.root).slice(2);
      expect(all.length).toBe(2);
      const seen = all.map((n) => {
        // Hidden until the software says otherwise.
        expect(n.general.visible).toBe(false);
        return bound(n, "general.visible", data);
      });
      expect(new Set(seen).size).toBe(1);
      return seen[0];
    };
    expect(show({ when: "limiter" }, { PitLimiter: 1 })).toBe(true);
    expect(show({ when: "limiter" }, { PitLimiter: 0 })).toBe(false);
    expect(show({ when: "limiter" }, {})).toBe(false);
    expect(show({ when: "pits" }, { PitLane: 1 })).toBe(true);
    expect(show({ when: "pits" }, { IsInPit: 1 })).toBe(true);
    expect(show({ when: "pits" }, { IsInPit: 0, PitLane: 0 })).toBe(false);
    expect(show({ when: "yellow" }, { YellowFlag: 1 })).toBe(true);
    expect(show({ when: "blue" }, { BlueFlag: 1 })).toBe(true);
    expect(show({ when: "field", field: "fuel_laps", op: "lt", value: 2 }, { FuelLaps: 1.5 })).toBe(
      true,
    );
    expect(show({ when: "field", field: "fuel_laps", op: "lt", value: 2 }, { FuelLaps: 2 })).toBe(
      false,
    );
    expect(
      show({ when: "field", field: "fuel_fraction", op: "le", value: 0.1 }, { FuelRemainder: 10 }),
    ).toBe(true);
    // A condition over data the wheel is not given: the element is left out, not always shown.
    const r = build(
      dash([
        el(
          "alert",
          { text: "Pluie" },
          { show: { when: "field", field: "rain", op: "gt", value: 0 } },
        ),
      ]),
    );
    expect(r.written).toBe(0);
    expect(r.notes).toEqual<MzdashNote[]>([
      { kind: "condition", page: "Course", element: "alert", what: "rain" },
    ]);
    // Without a condition nothing is bound and everything is shown.
    const always = nodes(build(dash([el("alert", { text: "Stand" })])).root).slice(2);
    for (const n of always) {
      expect(n.general.visible).toBe(true);
      expect(n.binding).toEqual({});
      expect(n.effect.blinkEnabled).toBe(true);
    }
    const still = nodes(build(dash([el("alert", { text: "Stand", blink: false })])).root).slice(2);
    for (const n of still) expect(n.effect.blinkEnabled).toBe(false);
  });

  it("writes texts, panels, shapes and the session clock", () => {
    const text = first(dash([el("text", { text: "Relais 2", align: "center" })]), "Text.qml");
    expect(text.text).toMatchObject({
      text: "RELAIS 2",
      fontFamily: "Roboto",
      fontWeight: 700,
      horizontalAlignment: "AlignCenter",
      verticalAlignment: "AlignCenter",
      wrapText: false,
    });
    expect(
      first(dash([el("text", { text: "Relais", upper: false, bold: false })]), "Text.qml").text,
    ).toMatchObject({ text: "Relais", fontWeight: 500, horizontalAlignment: "AlignLeft" });
    expect(build(dash([el("text", { text: "  " })])).written).toBe(0);

    const panel = nodes(build(dash([el("panel", { title: "Pneus" })])).root).slice(2);
    expect(panel.map((n) => n.type)).toEqual(["Rectangle.qml", "Text.qml"]);
    expect(panel[0]?.general).toMatchObject({ borderRadius: 6, borderWidth: 1 });
    // The outline and the corners are written twice, as the format has them.
    expect(panel[0]?.borderStyle).toMatchObject({
      allBorders: 1,
      bordersTop: 1,
      allCornerRadius: 6,
      radiusTopLeft: 6,
      radiusBottomRight: 6,
    });
    expect(panel[1]?.text?.text).toBe("PNEUS");

    const line = first(
      dash([el("shape", { shape: "line-h", thickness: 4 }, { w: 24, h: 2 })]),
      "Rectangle.qml",
    );
    expect(line.general.height).toBeLessThanOrEqual(3);
    expect(line.general.width).toBeGreaterThan(760);
    const circle = first(
      dash([el("shape", { shape: "circle", fillColor: "#112233" }, { w: 4, h: 4 })]),
      "Rectangle.qml",
    );
    expect(circle.general.borderRadius).toBe(
      Math.round(Math.min(circle.general.width, circle.general.height) / 2),
    );
    expect(circle.general.backgroundColor).toEqual({ color: "#FF112233", type: "SOLID" });

    const clock = valueNode(dash([el("clock", { mode: "left" })]));
    expect(bound(clock, "text.text", { SessionTimeLeft: 3725 })).toBe("1:02:05");
    expect(bound(clock, "text.text", { SessionTimeLeft: 307 })).toBe("05:07");
    expect(bound(clock, "text.text", {})).toBe("-");
    for (const mode of ["local", "elapsed", "game"])
      expect(build(dash([el("clock", { mode })])).notes).toEqual<MzdashNote[]>([
        { kind: "field", page: "Course", element: "clock", what: `clock:${mode}` },
      ]);
  });

  it("gives an element its own look, behind it", () => {
    const style = { bg: "#102030", opacity: 0.5, radius: 8, border: "#ffffff", border_width: 4 };
    const all = nodes(
      build(dash([el("value", { field: "gear", framed: false }, { style })])).root,
    ).slice(2);
    expect(all.map((n) => n.name)).toEqual(["Style", "Libellé", "Rapport"]);
    expect(all[0]?.general).toMatchObject({
      backgroundColor: { color: "#FF102030", type: "SOLID" },
      borderColor: { color: "#FFFFFFFF", type: "SOLID" },
      borderRadius: 5,
      borderWidth: 2,
    });
    // The opacity is that of the whole element.
    for (const n of all) expect(n.effect.opacity).toBe(50);
  });

  it("takes the background colour of the page, else that of the theme", () => {
    const plainPage = build(dash([])).root.children[0];
    expect(plainPage?.general.backgroundColor).toEqual({
      color: `#FF${DEFAULT_THEME.background.slice(1).toUpperCase()}`,
      type: "SOLID",
    });
    const def = dash([]);
    const page = def.pages[0];
    if (!page) throw new Error("page");
    page.background = { color: "#203040", asset: `${"ab".repeat(32)}.png` };
    const r = build(def);
    expect(r.root.children[0]?.general.backgroundColor).toEqual({
      color: "#FF203040",
      type: "SOLID",
    });
    expect(r.notes).toEqual<MzdashNote[]>([
      { kind: "media", page: "Course", element: "background", what: "image" },
    ]);
  });

  it("writes a few blocks of Corde in a simple form, and says which it leaves", () => {
    const r = build(
      dash([
        el("corde.gear"),
        el("corde.rev", {}, { w: 24, h: 1 }),
        el("corde.delta"),
        el("corde.fuel"),
        el("corde.page.course"),
        el("arc", { field: "speed_kmh" }),
        el("image", { asset: `${"ab".repeat(32)}.png` }),
        el("video", { asset: `${"ab".repeat(32)}.mp4` }),
        el("widget.unknown"),
      ]),
    );
    expect(r.total).toBe(9);
    expect(r.written).toBe(4);
    expect(r.notes).toEqual<MzdashNote[]>([
      { kind: "partial", page: "Course", element: "corde.gear", what: "block" },
      { kind: "partial", page: "Course", element: "corde.rev", what: "block" },
      { kind: "partial", page: "Course", element: "corde.delta", what: "block" },
      { kind: "element", page: "Course", element: "corde.fuel", what: "" },
      { kind: "element", page: "Course", element: "corde.page.course", what: "" },
      { kind: "partial", page: "Course", element: "arc", what: "arc" },
      { kind: "media", page: "Course", element: "image", what: "image" },
      { kind: "media", page: "Course", element: "video", what: "video" },
      { kind: "element", page: "Course", element: "widget.unknown", what: "" },
    ]);
    // The gear block: the gear alone, large, without label or frame — which is all of it
    // when the block does not show the speed.
    expect(build(dash([el("corde.gear", { speed: false })])).notes).toEqual([]);
    const gear = nodes(build(dash([el("corde.gear")])).root).slice(2);
    expect(gear.map((n) => n.type)).toEqual(["Text.qml"]);
    expect(gear[0] && bound(gear[0], "text.text", { Gear: 4 })).toBe("4");
    expect(gear[0]?.text?.fontSize).toBeGreaterThan(80);
  });

  it("stops before the file holds more nodes than its format numbers", () => {
    const page = (i: number) => ({
      id: `p${i}`,
      name: `Page ${i}`,
      cols: 48,
      rows: 32,
      elements: Array.from({ length: 80 }, (_, k) => ({
        ...el("rpm", { segments: 30 }, { x: 0, y: k % 32, w: 48, h: 1 }),
        id: `e${k}`,
      })),
    });
    const def = dash([], { pages: Array.from({ length: 12 }, (_, i) => page(i + 1)) });
    expect(validateDefinition(def).ok).toBe(true);
    const r = build(def);
    const ids = nodes(r.root).map((n) => n.id);
    expect(Math.max(...ids)).toBeLessThan(10_000);
    expect(r.written).toBeLessThan(r.total);
    expect(r.written).toBeGreaterThan(300);
    expect(r.notes.filter((n) => n.kind === "overflow")).toHaveLength(r.total - r.written);
    // The pages that came after the file was full hold nothing: they are not written.
    expect(r.root.children).toHaveLength(5);
  });
});

describe("what a definition says", () => {
  it("never reaches an expression, and a strange key is no entry of a table", () => {
    // A name that every object answers to is not a data, an alignment or a block.
    const r = build(
      dash([
        el(
          "alert",
          { text: "A" },
          { show: { when: "field", field: "constructor", op: "gt", value: 0 } },
        ),
        el(
          "alert",
          { text: "B" },
          { show: { when: "field", field: "__proto__", op: "gt", value: 0 } },
        ),
        el("value", { field: "toString" }),
        el("value", { field: "gear", align: "constructor" }),
        el("constructor"),
        el("text", { text: 'x");Telemetry.get("v1/gameData/Rpm' }),
        // A label that would be code if it were ever pasted into an expression.
        el("value", { field: "gear", label: `${"$"}{1+1}\`"\\` }),
      ]),
    );
    expect(r.written).toBe(3);
    expect(r.notes.map((n) => [n.kind, n.element, n.what])).toEqual([
      ["condition", "alert", "constructor"],
      ["condition", "alert", "__proto__"],
      ["field", "value", "toString"],
      ["element", "constructor", ""],
    ]);
    const texts = nodes(r.root).filter((n) => n.type === "Text.qml");
    for (const n of texts) expect(typeof n.text?.text).toBe("string");
    expect(texts.every((n) => "horizontalAlignment" in (n.text ?? {}))).toBe(true);
    // The words of the driver are texts of the file, never part of what is computed.
    const exprs = nodes(r.root).flatMap((n) => Object.values(n.binding).map((b) => b.methods[0]));
    expect(exprs.length).toBeGreaterThan(0);
    for (const e of exprs) expect(e).not.toMatch(/1\+1|x"\)/);
    expect(texts.map((n) => n.text?.text)).toContain('X");TELEMETRY.GET("V1/GAMEDATA/RPM');
  });

  it("only writes numbers and colours it has checked into the rules", () => {
    const thresholds = [
      { op: "ge", value: 1e21, color: "#ff0000" },
      { op: "lt", value: -5, color: "#00ff00" },
      { op: "ge", value: Number.NaN, color: "#0000ff" },
      { op: "ge", value: 3, color: 'red";alert(1);"' },
      { op: "nope", value: 3, color: "#123456" },
    ] as DashThreshold[];
    const n = valueNode(dash([el("value", { field: "speed_kmh" }, { thresholds })]));
    const expr = n.binding["text.fontColor"]?.methods[0] ?? "";
    expect(expr).not.toMatch(/alert|NaN|nope|123456|0000ff/);
    expect(bound(n, "text.fontColor", every(-9))).toBe("#00ff00");
    expect(bound(n, "text.fontColor", every(50))).toBe(DEFAULT_THEME.text);
    expect(bound(n, "text.fontColor", every(2e21))).toBe("#ff0000");
  });
});

describe("the pages", () => {
  it("leaves out a page nothing could be written of, but never all of them", () => {
    const page = (id: string, name: string, elements: DashElement[]) => ({
      id,
      name,
      cols: 24,
      rows: 8,
      elements,
    });
    const def = dash([], {
      pages: [
        page("p1", "Blocs", [el("corde.fuel")]),
        page("p2", "Course", [el("value", { field: "gear" })]),
        page("p3", "Vide", []),
      ],
    });
    const r = build(def);
    expect(r.root.children.map((s) => s.name)).toEqual(["Course"]);
    expect(r.root.children[0]?.id).toBeGreaterThan(0);
    expect(r.written).toBe(1);
    expect(r.total).toBe(2);
    expect(r.notes).toEqual<MzdashNote[]>([
      { kind: "element", page: "Blocs", element: "corde.fuel", what: "" },
    ]);
    // Nothing at all to write: one empty screen, so that the file stays a dashboard.
    const none = build(dash([el("corde.fuel")]));
    expect(none.root.children).toHaveLength(1);
    expect(none.root.children[0]?.children).toEqual([]);
    expect(none.written).toBe(0);
    // The model « Endurance », drawn for a phone: its second page is made of blocks only.
    const endurance = build({ ...template("endurance", "fr"), id: "endurance" });
    expect(endurance.root.children.map((s) => s.name)).toEqual(["Relais"]);
    expect(endurance.written).toBeGreaterThan(15);
    expect(endurance.written).toBeLessThan(endurance.total);
  });
});

describe("the name of the file", () => {
  it("keeps plain letters, digits, spaces, dashes and underscores", () => {
    expect(mzdashName("Volant MOZA")).toBe("Volant MOZA");
    expect(mzdashName("  Été à Spa — relais n°2  ")).toBe("Ete a Spa relais n 2");
    expect(mzdashName("GT3_endurance-24h")).toBe("GT3_endurance-24h");
    expect(mzdashName('..\\..\\Windows/<x>:"|?*')).toBe("Windows x");
    expect(mzdashName("日本語")).toBe("Corde");
    expect(mzdashName("")).toBe("Corde");
    expect(mzdashName("a".repeat(80))).toHaveLength(40);
    expect(build(dash([], { name: "Été / Spa" })).root.name).toBe("Ete Spa");
  });
});
