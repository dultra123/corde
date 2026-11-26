import type { DashDefinition, DashElement, DashPageDef } from "@corde/protocol";
import { CLOSE_GAP_S, LED_FULL, LED_START, SHIFT_AT } from "../dash/model";
import type { Lang } from "../store/settings";
import { boolProp, numProp, strProp } from "./catalog";
import { type FieldSpec, getField, label, unitOf } from "./fields";
import type { DashCondition, DashThreshold } from "./model";
import { validColor } from "./model";

/**
 * A dashboard of the Studio written as a dashboard for the screen of a MOZA wheel (KS Pro,
 * CS Pro: a 780 × 248 strip), in the format of the maker's own editor — a `.mzdash` file,
 * which is JSON: a window, its screens (our pages) and, on each screen, texts, rectangles and
 * gauges whose properties may be tied to a small expression over named data
 * (`v1/gameData/Rpm`…).
 *
 * What this gives and what it does not (ADR 0021):
 * - Corde does not talk to the wheel. The file is loaded by the maker's software (or by any
 *   software that drives the wheel), and that software is also what feeds the screen with the
 *   data of the game: the numbers shown on the wheel are its numbers, not Corde's. A data of
 *   the Studio is therefore translated to the data of the same meaning there ([`CHANNELS`]);
 *   one without a sure counterpart is left out and reported ([`MzdashNote`]), never guessed.
 * - Only the plain elements have a counterpart: value, bar, text, panel, shape, alert, rev
 *   lights and the clock of the session. A handful of blocks of Corde are written in a simple
 *   form ([`STAND_INS`]); the other blocks, the whole pages, the widgets, the pictures and
 *   the videos are reported and left out.
 * - The format is not documented by its maker. It was learnt from dashboards made with the
 *   maker's editor: the output mirrors their shape, key for key (a block a kind of element
 *   does not carry there is not written here either). Experimental until seen on a wheel.
 *
 * Pure: the definition in, the text of the file and the notes out.
 */

/** The screen of the wheels this is made for. */
export const MOZA_WIDTH = 780;
export const MOZA_HEIGHT = 248;
export const MZDASH_SUFFIX = ".mzdash";
/** The format version the maker's editor writes. */
const MZDASH_VERSION = "1.1.1";
/** Ids are whole numbers under this, the window being 0. */
const MAX_NODE_ID = 10_000;
/** The most nodes one element can become (rev lights: 30). */
const NODES_PER_ELEMENT = 40;
/** Shipped with the maker's editor and used by most of its own dashboards. */
const FONT = "Roboto";
const CLEAR = "#00000000";
/** What a text shows while there is no data. */
const NO_VALUE = "-";
/** The unit of the Studio's drawing on this screen (`fitStage` of the renderer). */
const U = Math.min(MOZA_HEIGHT / 100, MOZA_WIDTH * 0.0046);
/** Half the gap the Studio leaves between two elements. */
const GAP = Math.round((U * 1.8) / 2);

/** The colours of `styles/tokens.css` a data takes by itself in the Studio. */
const TONE = {
  cold: "#4ea1e6",
  warm: "#f2b63c",
  hot: "#f0503c",
  faster: "#28d17c",
  slower: "#ff5a5a",
  best: "#b982ff",
} as const;

/**
 * The screens the file says it is made for: the strip of the KS Pro, and the same strip on
 * the CS Pro, as the maker's dashboards for these wheels name them.
 */
const DISPLAYS = [
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
] as const;

export type MzdashNoteKind =
  /** The kind of element has no counterpart on the wheel: left out. */
  | "element"
  /** The data has no sure counterpart in what the wheel is fed: left out. */
  | "field"
  /** « Shown only when… » over a data without counterpart: left out. */
  | "condition"
  /** A picture or a video: not carried by this export. */
  | "media"
  /** Written in a simpler form (`what`: `block`, `arc`). */
  | "partial"
  /** No room left in the file: left out. */
  | "overflow";

export interface MzdashNote {
  kind: MzdashNoteKind;
  page: string;
  /** The type of the element (`value`, `corde.gear`…), `background` for that of a page. */
  element: string;
  /** The data or the part concerned. */
  what: string;
}

export interface MzdashResult {
  /** The name of the dashboard, as the file and its folder must be called. */
  name: string;
  /** The text of the `.mzdash` file. */
  json: string;
  /** How many elements of the Studio were written. */
  written: number;
  /** How many the definition holds. */
  total: number;
  notes: MzdashNote[];
}

export interface MzdashOptions {
  lang: Lang;
  /** Unix seconds, written as the date of the file. */
  now: number;
  /** A fresh GUID (`xxxxxxxx-xxxx-…`), without braces. */
  guid: string;
}

// ---------------------------------------------------------------- the data

/**
 * The entry of a table under a key read from a definition: its own keys only (`constructor`
 * and the like are not entries).
 */
function entry<T>(table: Record<string, T>, key: unknown): T | undefined {
  return typeof key === "string" && Object.hasOwn(table, key) ? table[key] : undefined;
}

/** A text literal of the expressions. */
function lit(s: string): string {
  return JSON.stringify(s);
}

/** The value of a named data, as a number. */
function tele(name: string): string {
  return `Number(Telemetry.get("v1/gameData/${name}").value)`;
}

/** A share between 0 and 1 from a data given in percent. */
const percent = (name: string) => `${tele(name)}/100`;

const CORNER: Record<string, { long: string; short: string }> = {
  fl: { long: "FrontLeft", short: "FL" },
  fr: { long: "FrontRight", short: "FR" },
  rl: { long: "RearLeft", short: "RL" },
  rr: { long: "RearRight", short: "RR" },
};

/**
 * The revs as a share of the car's maximum; when the game gives no maximum, the share the
 * software computed itself (a percentage).
 */
const RPM_SHARE = `((r,m,p)=>m>0?Math.min(1,Math.max(0,r/m)):p>=0?Math.min(1,p/100):0)(${tele("Rpm")},${tele("MaxRpm")},${tele("CarSettings_CurrentDisplayedRPMPercent")})`;

/**
 * Data of the Studio → an expression giving the same quantity in the Studio's own unit
 * (seconds, litres, °C, kPa, a share from 0 to 1…) over the data the wheel is fed, whose units
 * are those the maker's catalogue states. Only the data whose meaning and unit are sure are
 * here. Left out on purpose: the tyre wear (worn or left?), the time of day and the sector (no
 * unit given), the brake migration (described as something else), and everything the wheel is
 * not fed at all (oil, steering, class position, gap to the leader, laps left, rain…).
 */
const CHANNELS: Record<string, string> = {
  speed_kmh: tele("SpeedKmh"),
  gear: tele("Gear"),
  rpm: tele("Rpm"),
  rpm_fraction: RPM_SHARE,
  limiter: tele("PitLimiter"),
  water_c: tele("WaterTemp"),
  throttle: tele("Throttle"),
  brake: tele("Brake"),
  clutch: tele("Clutch"),
  lap_current: tele("CurrentLapTime"),
  lap_last: tele("LastLapTime"),
  lap_best: tele("BestLapTime"),
  lap_estimated: tele("EstimatedLapTime"),
  delta_best: tele("GAP"),
  lap_number: tele("CurrentLap"),
  lap_fraction: tele("patch/TrackPositionPercent"),
  position: tele("CurrentPos"),
  cars: tele("CurrentCarCount"),
  gap_ahead: tele("TimeGapPlaceAhead"),
  gap_behind: tele("TimeGapPlaceBehind"),
  session_left: tele("SessionTimeLeft"),
  track_name: `Telemetry.get("v1/gameData/patch/TrackName").value`,
  fuel_l: tele("Fuel"),
  fuel_fraction: percent("FuelRemainder"),
  fuel_per_lap: tele("FuelConsumeLap"),
  fuel_laps: tele("FuelLaps"),
  // Given as a share or as a percentage, depending on the game: both are read.
  virtual_energy: `((e)=>e>1?e/100:e)(${tele("VirtualEnergy")})`,
  battery: percent("BatteryPercent"),
  tc: tele("TCLevel"),
  abs: tele("ABSLevel"),
  motor_map: tele("ECUMap"),
  brake_bias: tele("BrakeBias"),
  track_c: tele("TrackTemp"),
  air_c: tele("AirTemp"),
  ...Object.fromEntries(
    Object.entries(CORNER).flatMap(([k, c]) => [
      [`tyre_temp_${k}`, tele(`TyreTemp${c.long}`)],
      [`tyre_pressure_${k}`, tele(`TyrePressure${c.short}&kpa`)],
      [`brake_temp_${k}`, tele(`BrakeTemp${c.long}`)],
    ]),
  ),
};

/** The data of the Studio that can be shown on the wheel. */
export function mozaHasField(id: string): boolean {
  return Object.hasOwn(CHANNELS, id);
}

/**
 * The colour a data takes by itself in the Studio (`color` of its `FieldSpec`), as an
 * expression of `v`; `base` is the expression of the colour it has otherwise.
 */
const OWN_COLOR: Record<string, (base: string) => string> = {
  throttle: () => lit(TONE.faster),
  brake: () => lit(TONE.slower),
  clutch: () => lit(TONE.cold),
  lap_best: () => lit(TONE.best),
  battery: () => lit(TONE.best),
  delta_best: (b) => `!(Math.abs(v)>=0.0005)?${b}:v<0?${lit(TONE.faster)}:${lit(TONE.slower)}`,
  rpm_fraction: (b) => `v>=${SHIFT_AT}?${lit(TONE.cold)}:v>=0.85?${lit(TONE.slower)}:${b}`,
  gap_ahead: (b) => `v>0&&v<${CLOSE_GAP_S}?${lit(TONE.faster)}:${b}`,
  gap_behind: (b) => `v>0&&v<${CLOSE_GAP_S}?${lit(TONE.slower)}:${b}`,
  fuel_fraction: (b) => `v<0.1?${lit(TONE.hot)}:v<0.2?${lit(TONE.warm)}:${b}`,
  virtual_energy: () => `v<0.1?${lit(TONE.hot)}:v<0.2?${lit(TONE.warm)}:${lit(TONE.cold)}`,
};

const OPS: Record<string, string> = { lt: "<", le: "<=", ge: ">=", gt: ">" };

/** The colour rules as the head of a chain over `name`: `name<5?"#…":name>=9?"#…":`. */
function ruleChain(rules: readonly DashThreshold[] | undefined, name: string): string {
  return (rules ?? [])
    .filter((r) => Object.hasOwn(OPS, r.op) && Number.isFinite(r.value) && validColor(r.color))
    .map((r) => `${name}${OPS[r.op]}${r.value}?${lit(r.color)}:`)
    .join("");
}

/** The colour of an expression when it is one fixed colour, else `null`. */
function fixedColor(expr: string): string | null {
  return /^"#[0-9a-fA-F]{6}"$/.test(expr) ? (JSON.parse(expr) as string) : null;
}

/** `v` as the text the Studio would write, as an expression of `v`. */
function formatter(f: FieldSpec, decimals: number | null, lang: Lang): string {
  const none = lit(NO_VALUE);
  const fixed = (d: number) => `(isFinite(v)?v.toFixed(${d}):${none})`;
  const d = decimals !== null && decimals >= 0 ? Math.min(3, Math.round(decimals)) : null;
  const two = `(n)=>(n<10?"0":"")+n`;
  switch (f.format) {
    case "lap":
      return `(v>0?((t)=>Math.floor(t/60000)+":"+(${two})(Math.floor(t/1000)%60)+"."+String(t%1000).padStart(3,"0"))(Math.round(v*1000)):${none})`;
    case "sector":
      return `(v>0?((t)=>(t<60000?"":Math.floor(t/60000)+":")+(t<60000?String(Math.floor(t/1000)):(${two})(Math.floor(t/1000)%60))+"."+String(t%1000).padStart(3,"0"))(Math.round(v*1000)):${none})`;
    case "delta":
      return `(isFinite(v)?(v>0?"+":v<0?"-":"±")+Math.abs(v).toFixed(${d ?? 3}):${none})`;
    case "gap":
      return `(v>0?${lit(f.id === "gap_ahead" ? "-" : "+")}+v.toFixed(${d ?? 1}):${none})`;
    case "clock":
      return `(v>=0?((t,p)=>(t>=3600?Math.floor(t/3600)+":":"")+p(Math.floor(t%3600/60))+":"+p(t%60))(Math.floor(v),${two}):${none})`;
    case "gear":
      return `(isFinite(v)?(v<0?"R":v<1?"N":String(Math.round(v))):${none})`;
    case "place":
      return `(v>0?"P"+Math.round(v):${none})`;
    case "pct":
      return `(isFinite(v)?(v*100).toFixed(${d ?? 0}):${none})`;
    case "onoff":
      return `(v>0?${lit(lang === "en" ? "ON" : "OUI")}:${none})`;
    case "text":
      return `(v?String(v):${none})`;
    case "dec1":
      return fixed(d ?? 1);
    case "dec2":
      return fixed(d ?? 2);
    default:
      return fixed(d ?? 0);
  }
}

/** A text of this data at its longest, to size the letters. */
function sample(f: FieldSpec, decimals: number | null): string {
  const d = decimals !== null && decimals >= 0 ? decimals : null;
  const frac = (n: number) => (n > 0 ? `.${"0".repeat(n)}` : "");
  switch (f.format) {
    case "lap":
    case "sector":
      return "1:23.456";
    case "delta":
      return `+0${frac(d ?? 3)}`;
    case "gap":
      return `+00${frac(d ?? 1)}`;
    case "clock":
      return "1:23:45";
    case "gear":
      return "N";
    case "place":
      return "P12";
    case "pct":
      return `100${frac(d ?? 0)}`;
    case "onoff":
      return "OUI";
    case "text":
      return "Circuit de la Sarthe";
    case "dec1":
      return `000${frac(d ?? 1)}`;
    case "dec2":
      return `0${frac(d ?? 2)}`;
    default:
      return f.id === "rpm" ? "00000" : `000${frac(d ?? 0)}`;
  }
}

/** A property tied to an expression: the computation, then its result as it is. */
function bind(expr: string) {
  return { methods: [expr, "_result"], type: "METHOD_CHAINING" };
}

/**
 * « Shown only when… » as an expression: `undefined` without a condition, `null` when it has
 * no counterpart.
 */
function visibleWhen(c: DashCondition | undefined): string | null | undefined {
  if (!c) return undefined;
  switch (c.when) {
    case "limiter":
      return `${tele("PitLimiter")}>0`;
    case "pits":
      return `${tele("IsInPit")}>0||${tele("PitLane")}>0`;
    case "yellow":
      return `${tele("YellowFlag")}>0`;
    case "blue":
      return `${tele("BlueFlag")}>0`;
    case "field": {
      const expr = entry(CHANNELS, c.field);
      if (!expr || typeof c.op !== "string" || !Object.hasOwn(OPS, c.op)) return null;
      if (typeof c.value !== "number" || !Number.isFinite(c.value)) return null;
      return `((v)=>v${OPS[c.op]}${c.value})(${expr})`;
    }
    default:
      return null;
  }
}

// ---------------------------------------------------------------- the nodes

interface Box {
  x: number;
  y: number;
  w: number;
  h: number;
}

type Node = Record<string, unknown>;

interface Look {
  fill?: string;
  border?: string;
  width?: number;
  radius?: number;
}

const solid = (color: string) => ({ color, type: "SOLID" });

/** `#rrggbb` (and an opacity from 0 to 1) as the format writes a fixed colour: `#AARRGGBB`. */
function argb(color: string, alpha = 1): string {
  const a = Math.round(Math.max(0, Math.min(1, alpha)) * 255);
  return `#${a.toString(16).padStart(2, "0")}${color.slice(1)}`.toUpperCase();
}

const px = (n: number) => Math.max(0, Math.round(n));

/** The outline and the corners, which the format writes twice (here and in `general`). */
function borderStyle(o: Look = {}) {
  const w = o.border ? px(o.width ?? 0) : 0;
  const r = px(o.radius ?? 0);
  return {
    allBorders: w,
    allCornerRadius: r,
    borderColor: solid(w > 0 && o.border ? argb(o.border) : CLEAR),
    bordersBottom: w,
    bordersLeft: w,
    bordersRight: w,
    bordersTop: w,
    radiusBottomLeft: r,
    radiusBottomRight: r,
    radiusTopLeft: r,
    radiusTopRight: r,
  };
}

function general(b: Box, o: Look = {}, fillAlpha = 1) {
  const w = o.border ? px(o.width ?? 0) : 0;
  return {
    backgroundColor: solid(o.fill ? argb(o.fill, fillAlpha) : CLEAR),
    borderColor: solid(w > 0 && o.border ? argb(o.border) : CLEAR),
    borderRadius: px(o.radius ?? 0),
    borderWidth: w,
    height: Math.max(1, Math.round(b.h)),
    locked: false,
    visible: true,
    width: Math.max(1, Math.round(b.w)),
    x: Math.round(b.x),
    y: Math.round(b.y),
  };
}

const OUTER_SHADOW = () => ({ blur: 0, color: solid(CLEAR), depth: 0, direction: 0 });

/** The effect block, which differs by kind of element (the shadows are not everywhere). */
function effect(kind: "text" | "rect" | "plain", blink = false) {
  return {
    blinkDelay: 250,
    blinkEnabled: blink,
    blurRadius: 0,
    gravitySensorEnabled: false,
    ...(kind === "rect" ? { innerShadow: { blur: 0, color: solid("#FFFFFF"), size: 0 } } : {}),
    opacity: 100,
    ...(kind === "plain" ? {} : { outerShadow: OUTER_SHADOW() }),
    rotation: 0,
  };
}

type HAlign = "AlignLeft" | "AlignCenter" | "AlignRight";

const ALIGN: Record<string, HAlign> = {
  left: "AlignLeft",
  center: "AlignCenter",
  right: "AlignRight",
};

class Writer {
  private next = 1;
  readonly notes: MzdashNote[] = [];
  written = 0;
  constructor(
    readonly def: DashDefinition,
    readonly opts: MzdashOptions,
  ) {}

  id(): number {
    return this.next++;
  }

  /** Whether one more element fits under the highest id. */
  room(): boolean {
    return this.next + NODES_PER_ELEMENT < MAX_NODE_ID;
  }

  private node(type: string, name: string, look: Look, rest: Node): Node {
    return {
      actions: {},
      binding: {},
      borderStyle: borderStyle(look),
      children: [],
      id: this.id(),
      macros: {},
      name: name.slice(0, 40),
      plugins: [],
      type,
      ...rest,
    };
  }

  text(
    name: string,
    b: Box,
    o: {
      text: string;
      size: number;
      color: string;
      bold?: boolean;
      h?: HAlign;
      binding?: Node;
      blink?: boolean;
    },
  ): Node {
    return this.node(
      "Text.qml",
      name,
      {},
      {
        binding: o.binding ?? {},
        effect: effect("text", o.blink === true),
        general: general(b),
        text: {
          fontColor: solid(argb(o.color)),
          fontFamily: FONT,
          fontSize: Math.max(8, Math.round(o.size)),
          fontWeight: o.bold === false ? 500 : 700,
          horizontalAlignment: o.h ?? "AlignCenter",
          paddingBottom: 0,
          paddingLeft: 0,
          paddingRight: 0,
          paddingTop: 0,
          text: o.text,
          verticalAlignment: "AlignCenter",
          wrapText: false,
        },
        textEffect: {
          shadowBlur: 0,
          shadowColor: solid(CLEAR),
          shadowDepth: 0,
          shadowDirection: 0,
        },
      },
    );
  }

  rect(name: string, b: Box, o: Look & { binding?: Node; blink?: boolean; alpha?: number }): Node {
    return this.node("Rectangle.qml", name, o, {
      binding: o.binding ?? {},
      effect: effect("rect", o.blink === true),
      general: general(b, o, o.alpha),
    });
  }

  gauge(name: string, b: Box, o: { color: string; vertical: boolean; binding: Node }): Node {
    return this.node(
      "LinearGauge.qml",
      name,
      {},
      {
        binding: o.binding,
        effect: effect("plain"),
        general: general(b),
        linearGauge: {
          alternateStyle: {
            gaugeColor: solid("#03A200"),
            gaugeImage: "",
            useAlternateStyle: false,
          },
          backgroundImage: "",
          // The only spellings there are: on a vertical gauge it fills from the bottom.
          gaugeAlignment: "AlignLeft",
          gaugeColor: solid(argb(o.color)),
          gaugeImage: "",
          gaugeOrientation: o.vertical ? "Vertical" : "Horizontal",
          maximum: 100,
          minimum: 0,
          value: 0,
        },
      },
    );
  }
}

/** The largest letters that let `text` fit in `w` × `h` (a digit is about 0.58 em wide). */
function letterSize(text: string, w: number, h: number): number {
  return Math.max(8, Math.min(h * 0.86, w / (Math.max(1, text.length) * 0.58)));
}

/** The letters of a label of this height, smaller when it would not fit the width. */
function labelSize(text: string, w: number, h: number): number {
  return Math.max(9, Math.min(Math.max(11, h * 0.78), w / (Math.max(1, text.length) * 0.62)));
}

function inset(b: Box, by: number): Box {
  return { x: b.x + by, y: b.y + by, w: Math.max(1, b.w - 2 * by), h: Math.max(1, b.h - 2 * by) };
}

/** An element written in the place of a block of Corde. */
interface StandIn {
  type: string;
  props: Record<string, unknown>;
  /** No label and no frame, as the block is drawn. */
  bare?: boolean;
  /** The block shows nothing more than this: no note. */
  same?: boolean;
}

/** The blocks of Corde that have an obvious plain form, from their own settings. */
const STAND_INS: Record<string, (props: Record<string, unknown>) => StandIn> = {
  "corde.rev": () => ({ type: "rpm", props: {} }),
  // The gear alone: the same thing when the block was told not to show the speed.
  "corde.gear": (props) => ({
    type: "value",
    props: { field: "gear", align: "center" },
    bare: true,
    same: props.speed === false,
  }),
  "corde.delta": () => ({ type: "value", props: { field: "delta_best", align: "center" } }),
  "corde.position": () => ({ type: "value", props: { field: "position", align: "center" } }),
  "corde.lap-current": () => ({ type: "value", props: { field: "lap_current", align: "center" } }),
};

/** One element of the Studio, in its cell, as nodes of the wheel's screen. */
function convert(w: Writer, page: DashPageDef, source: DashElement, cell: Box): Node[] {
  const { def, opts } = w;
  const theme = def.theme;
  const note = (kind: MzdashNoteKind, what: string) =>
    w.notes.push({ kind, page: page.name, element: source.type, what });
  const stand = entry(STAND_INS, source.type)?.(source.props ?? {});
  if (stand && !stand.same) note("partial", "block");
  const type = stand?.type ?? source.type;
  const props = stand ? stand.props : (source.props ?? {});
  const bare = stand?.bare === true;
  const own = (key: string, fallback: string) => {
    const v = props[key];
    return validColor(v) ? v : fallback;
  };
  const out: Node[] = [];
  const box = inset(cell, GAP);
  const pad = Math.max(3, Math.min(box.w, box.h) * 0.07);
  const inner = inset(box, pad);
  const frame = () =>
    out.push(w.rect("Cadre", box, { fill: theme.panel, border: theme.line, width: 1, radius: 6 }));
  /** The label on top, and the room left under it. */
  const labelled = (text: string, align: HAlign): Box => {
    const lh = Math.max(13, Math.min(inner.h * 0.3, 20));
    if (bare || !text || inner.h < 34) return inner;
    out.push(
      w.text(
        "Libellé",
        { x: inner.x, y: inner.y, w: inner.w, h: lh },
        {
          text,
          size: labelSize(text, inner.w, lh),
          color: theme.dim,
          bold: false,
          h: align,
        },
      ),
    );
    return { x: inner.x, y: inner.y + lh, w: inner.w, h: inner.h - lh };
  };
  const nameOf = (f: FieldSpec) =>
    strProp(props, "label", "").trim() || label(f.short ?? f.label, opts.lang);
  /**
   * The label of a value: its name and, when there is room for both, its unit (a degree is
   * not said: a temperature reads as one).
   */
  const labelOf = (f: FieldSpec) => {
    const name = nameOf(f).toUpperCase();
    const unit = boolProp(props, "unit", true) ? unitOf(f, opts.lang) : "";
    if (!unit || unit === "°" || name.toLowerCase().includes(unit.toLowerCase())) return name;
    const full = `${name} (${unit})`;
    return inner.w / (full.length * 0.62) >= 11 ? full : name;
  };

  switch (type) {
    case "value":
    case "arc": {
      const f = getField(props.field);
      const expr = f ? entry(CHANNELS, f.id) : undefined;
      if (!f || !expr) {
        note("field", String(props.field ?? ""));
        return [];
      }
      // The arc itself has no counterpart: the number alone is written.
      if (type === "arc") note("partial", "arc");
      const asked = numProp(props, "decimals", -1, -1, 3);
      const decimals = asked < 0 ? null : asked;
      const align =
        type === "arc"
          ? "AlignCenter"
          : (entry(ALIGN, strProp(props, "align", "left")) ?? "AlignLeft");
      if (!bare && (type === "arc" || boolProp(props, "framed", true))) frame();
      const area = labelled(labelOf(f), align);
      const base = lit(own("color", theme.text));
      const tail = validColor(props.color) ? base : (entry(OWN_COLOR, f.id)?.(base) ?? base);
      const color = `${ruleChain(source.thresholds, "v")}${tail}`;
      const still = fixedColor(color);
      out.push(
        w.text(nameOf(f), area, {
          text: NO_VALUE,
          size: letterSize(sample(f, decimals), area.w, area.h),
          color: still ?? own("color", theme.text),
          h: align,
          binding: {
            "text.text": bind(`((v)=>${formatter(f, decimals, opts.lang)})(${expr})`),
            ...(still ? {} : { "text.fontColor": bind(`((v)=>${color})(${expr})`) }),
          },
        }),
      );
      return out;
    }
    case "bar": {
      const f = getField(props.field);
      const expr = f?.fraction ? entry(CHANNELS, f.id) : undefined;
      if (!f || !expr) {
        note("field", String(props.field ?? ""));
        return [];
      }
      const vertical = boolProp(props, "vertical", false);
      if (boolProp(props, "framed", true)) frame();
      const showValue = boolProp(props, "showValue", true);
      const ownLabel = strProp(props, "label", "").trim();
      const pct = `((v)=>isFinite(v)?Math.min(100,Math.max(0,v*100)):0)(${expr})`;
      let area = inner;
      if (!vertical && (showValue || ownLabel) && inner.h >= 30) {
        const lh = Math.max(13, Math.min(inner.h * 0.45, 22));
        const split = showValue ? 0.68 : 1;
        out.push(
          w.text(
            "Libellé",
            { x: inner.x, y: inner.y, w: inner.w * split, h: lh },
            {
              text: nameOf(f).toUpperCase(),
              size: labelSize(nameOf(f), inner.w * split, lh),
              color: theme.dim,
              bold: false,
              h: "AlignLeft",
            },
          ),
        );
        if (showValue)
          out.push(
            w.text(
              "Valeur",
              { x: inner.x + inner.w * split, y: inner.y, w: inner.w * (1 - split), h: lh },
              {
                text: NO_VALUE,
                size: labelSize("100 %", inner.w * (1 - split), lh),
                color: theme.text,
                h: "AlignRight",
                binding: { "text.text": bind(`((p)=>p.toFixed(0)+" %")(${pct})`) },
              },
            ),
          );
        area = { x: inner.x, y: inner.y + lh + 2, w: inner.w, h: inner.h - lh - 2 };
      }
      // The rules compare the percentage shown; the data's own colour reads its share.
      const base = lit(own("color", theme.text));
      const tail = validColor(props.color) ? base : (entry(OWN_COLOR, f.id)?.(base) ?? base);
      const color = `${ruleChain(source.thresholds, "p")}${tail}`;
      const still = fixedColor(color);
      out.push(
        w.rect("Fond de barre", area, {
          fill: theme.line,
          radius: Math.min(4, Math.min(area.w, area.h) / 2),
        }),
      );
      out.push(
        w.gauge(nameOf(f), area, {
          color: still ?? own("color", theme.text),
          vertical,
          binding: {
            "linearGauge.value": bind(pct),
            ...(still
              ? {}
              : { "linearGauge.gaugeColor": bind(`((v)=>((p)=>${color})(v*100))(${expr})`) }),
          },
        }),
      );
      return out;
    }
    case "text": {
      const raw = strProp(props, "text", "");
      if (!raw.trim()) return [];
      if (boolProp(props, "framed", false)) frame();
      const text = boolProp(props, "upper", true) ? raw.toUpperCase() : raw;
      out.push(
        w.text("Texte", inner, {
          text,
          size: letterSize(text, inner.w, inner.h),
          color: own("color", theme.text),
          bold: boolProp(props, "bold", true),
          h: entry(ALIGN, strProp(props, "align", "left")) ?? "AlignLeft",
        }),
      );
      return out;
    }
    case "panel": {
      const style = strProp(props, "style", "panel");
      const line = own("color", theme.line);
      if (style === "line-h")
        return [
          w.rect("Ligne", { x: box.x, y: box.y + box.h / 2 - 1, w: box.w, h: 2 }, { fill: line }),
        ];
      if (style === "line-v")
        return [
          w.rect("Ligne", { x: box.x + box.w / 2 - 1, y: box.y, w: 2, h: box.h }, { fill: line }),
        ];
      out.push(
        w.rect("Panneau", box, {
          fill: own("fillColor", theme.panel),
          border: line,
          width: 1,
          radius: 6,
        }),
      );
      const title = strProp(props, "title", "").trim();
      if (title) {
        const lh = Math.max(13, Math.min(inner.h * 0.3, 20));
        out.push(
          w.text(
            "Titre",
            { x: inner.x, y: inner.y, w: inner.w, h: lh },
            {
              text: title.toUpperCase(),
              size: labelSize(title, inner.w, lh),
              color: theme.dim,
              bold: false,
              h: "AlignLeft",
            },
          ),
        );
      }
      return out;
    }
    case "shape": {
      const shape = strProp(props, "shape", "rect");
      const fill = own("fillColor", theme.accent);
      const thick = Math.max(1, numProp(props, "thickness", 3, 1, 20) * 0.25 * U);
      if (shape === "line-h")
        return [
          w.rect(
            "Ligne",
            { x: box.x, y: box.y + (box.h - thick) / 2, w: box.w, h: thick },
            { fill },
          ),
        ];
      if (shape === "line-v")
        return [
          w.rect(
            "Ligne",
            { x: box.x + (box.w - thick) / 2, y: box.y, w: thick, h: box.h },
            { fill },
          ),
        ];
      // An oval: a rectangle whose corners are as round as they can be.
      const radius = shape === "circle" ? Math.min(box.w, box.h) / 2 : 0;
      return [w.rect("Forme", box, { fill, radius })];
    }
    case "alert": {
      const raw = strProp(props, "text", "").trim();
      const blink = boolProp(props, "blink", true);
      out.push(w.rect("Alerte", box, { fill: own("fillColor", "#e5484d"), radius: 6, blink }));
      if (raw)
        out.push(
          w.text("Alerte", inner, {
            text: raw.toUpperCase(),
            size: letterSize(raw, inner.w, inner.h),
            color: own("color", "#ffffff"),
            blink,
          }),
        );
      return out;
    }
    case "clock": {
      // Only the time left in the session: the wheel is not given the other clocks.
      const mode = strProp(props, "mode", "local");
      const f = mode === "left" ? getField("session_left") : undefined;
      if (!f) {
        note("field", `clock:${mode}`);
        return [];
      }
      const align = entry(ALIGN, strProp(props, "align", "left")) ?? "AlignLeft";
      if (boolProp(props, "framed", true)) frame();
      const area = labelled(
        (strProp(props, "label", "").trim() || label(f.label, opts.lang)).toUpperCase(),
        align,
      );
      out.push(
        w.text("Horloge", area, {
          text: NO_VALUE,
          size: letterSize(sample(f, null), area.w, area.h),
          color: own("color", theme.text),
          h: align,
          binding: {
            "text.text": bind(`((v)=>${formatter(f, null, opts.lang)})(${CHANNELS.session_left})`),
          },
        }),
      );
      return out;
    }
    case "rpm": {
      // The rev lights: one rectangle each, lit from its own share of the rev range, all of
      // them in the shift colour at the top (`rampLit` and `SHIFT_AT` of the renderer).
      const n = Math.round(numProp(props, "segments", 15, 3, 30));
      const start = numProp(props, "start", LED_START, 0.3, 0.9);
      const tones = [
        own("lowColor", "#2fe07f"),
        own("midColor", "#ffc93c"),
        own("highColor", "#ff4b3e"),
      ];
      const shift = own("shiftColor", "#3d8bff");
      const off = theme.line;
      const gap = Math.max(1, Math.round(box.w * 0.006));
      const width = (box.w - gap * (n - 1)) / n;
      for (let i = 0; i < n; i += 1) {
        const from = start + ((LED_FULL - start) * i) / n;
        const on = tones[Math.min(2, Math.floor((i * 3) / n))] ?? shift;
        const x0 = Math.round(box.x + i * (width + gap));
        const x1 = Math.round(box.x + i * (width + gap) + width);
        out.push(
          w.rect(
            `Régime ${i + 1}`,
            { x: x0, y: box.y, w: x1 - x0, h: box.h },
            {
              fill: off,
              radius: Math.min(3, width / 2),
              binding: {
                "general.backgroundColor": bind(
                  `((f)=>f>=${SHIFT_AT}?${lit(shift)}:f>${Number(from.toFixed(4))}?${lit(on)}:${lit(off)})(${RPM_SHARE})`,
                ),
              },
            },
          ),
        );
      }
      return out;
    }
    case "image":
    case "video":
      note("media", type);
      return [];
    default:
      note("element", "");
      return [];
  }
}

/** The background, the outline and the corners an element was given, as a rectangle behind. */
function styled(w: Writer, el: DashElement, cell: Box): Node | null {
  const s = el.style;
  if (!s) return null;
  const fill = validColor(s.bg) ? s.bg : undefined;
  const width = typeof s.border_width === "number" ? s.border_width * 0.25 * U : 0;
  const border = validColor(s.border) && width > 0 ? s.border : undefined;
  if (!fill && !border) return null;
  return w.rect("Style", inset(cell, GAP), {
    fill,
    border,
    width: Math.max(1, width),
    radius: typeof s.radius === "number" ? s.radius * 0.25 * U : 0,
  });
}

function screen(w: Writer, page: DashPageDef, index: number): Node {
  const { def } = w;
  const id = w.id();
  const cw = MOZA_WIDTH / Math.max(1, page.cols);
  const ch = MOZA_HEIGHT / Math.max(1, page.rows);
  const children: Node[] = [];
  const bg = page.background;
  if (bg?.asset)
    w.notes.push({ kind: "media", page: page.name, element: "background", what: "image" });
  for (const el of page.elements) {
    const skip = (kind: MzdashNoteKind, what: string) =>
      w.notes.push({ kind, page: page.name, element: el.type, what });
    if (!w.room()) {
      skip("overflow", "");
      continue;
    }
    const visible = visibleWhen(el.show);
    if (visible === null) {
      skip("condition", String(el.show?.field ?? el.show?.when ?? ""));
      continue;
    }
    const cell: Box = { x: el.x * cw, y: el.y * ch, w: el.w * cw, h: el.h * ch };
    const nodes = convert(w, page, el, cell);
    if (nodes.length === 0) continue;
    const back = styled(w, el, cell);
    const all = back ? [back, ...nodes] : nodes;
    const opacity = el.style?.opacity;
    for (const n of all) {
      if (typeof opacity === "number" && opacity >= 0 && opacity < 1)
        (n.effect as Node).opacity = Math.round(opacity * 100);
      if (visible !== undefined) {
        // Hidden until the condition holds, as it is in the Studio without a game.
        (n.binding as Node)["general.visible"] = bind(visible);
        (n.general as Node).visible = false;
      }
    }
    children.push(...all);
    w.written += 1;
  }
  const fill = bg && validColor(bg.color) ? bg.color : def.theme.background;
  return {
    actions: {},
    binding: {},
    borderStyle: borderStyle(),
    children,
    effect: effect("plain"),
    general: general({ x: 0, y: 0, w: MOZA_WIDTH, h: MOZA_HEIGHT }, { fill }),
    guides: [],
    id,
    image: { src: "" },
    macros: {},
    name: (page.name || `Page ${index + 1}`).slice(0, 40),
    plugins: [],
    type: "Screen.qml",
  };
}

/**
 * The name of the file and of its folder: the wheel takes the name it shows from them, so
 * only plain letters, digits, spaces, `-` and `_` are kept (an accent loses its mark).
 */
export function mzdashName(name: string): string {
  const clean = name
    .normalize("NFD")
    .replace(/\p{M}/gu, "")
    .replace(/[^A-Za-z0-9 _-]/g, " ")
    .replace(/\s+/g, " ")
    .trim()
    .slice(0, 40)
    .trim();
  return clean || "Corde";
}

/** The keys of every object in alphabetical order, as the maker's editor writes them. */
function sorted(v: unknown): unknown {
  if (Array.isArray(v)) return v.map(sorted);
  if (v === null || typeof v !== "object") return v;
  const o = v as Record<string, unknown>;
  return Object.fromEntries(
    Object.keys(o)
      .sort()
      .map((k) => [k, sorted(o[k])]),
  );
}

export function toMzdash(def: DashDefinition, opts: MzdashOptions): MzdashResult {
  const w = new Writer(def, opts);
  // A page nothing could be written of would be a blank screen on the wheel: it is left out
  // (its elements are in the notes), but the file always holds a screen.
  const all = def.pages.map((p, i) => screen(w, p, i));
  const filled = all.filter((s) => (s.children as Node[]).length > 0);
  const screens = filled.length > 0 ? filled : all.slice(0, 1);
  const name = mzdashName(def.name);
  const root = {
    actions: {},
    binding: {},
    borderStyle: borderStyle(),
    children: screens,
    effect: { gravitySensorEnabled: false, rotation: 0 },
    general: general({ x: 0, y: 0, w: MOZA_WIDTH, h: MOZA_HEIGHT }, { fill: "#000000" }),
    id: 0,
    imageResources: [],
    lastModified: Math.floor(opts.now),
    macros: {},
    name,
    plugins: [],
    type: "Window.qml",
    version: MZDASH_VERSION,
    window: {
      GUID: `{${opts.guid}}`,
      defaultScreenId: 0,
      idealDeviceInfos: DISPLAYS.map((d) => ({ ...d })),
    },
  };
  return {
    name,
    // Four spaces and no byte-order mark, as the maker's editor writes its files.
    json: JSON.stringify(sorted(root), null, 4),
    written: w.written,
    total: def.pages.reduce((n, p) => n + p.elements.length, 0),
    notes: w.notes,
  };
}
