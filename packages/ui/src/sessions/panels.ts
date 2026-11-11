import type { TranslationKey } from "../i18n";
import { COLORS } from "../lib/color";
import type { Lang } from "../store/settings";
import type { PanelScale, PanelSeries } from "./ChartPanel";
import {
  channelLabel,
  displayFactor,
  displayUnit,
  isStepChannel,
  SERIES_COLORS,
  splitWheel,
  WHEEL_COLORS,
  WHEEL_LABELS,
  WHEEL_SUFFIXES,
} from "./channels";
import { splitSign } from "./lib/delta";

export type PanelKind =
  | "speed"
  | "pedals"
  | "gearRpm"
  | "g"
  | "tyreTemp"
  | "pressure"
  | "brakeTemp"
  | "slip"
  | "rideHeight"
  | "delta"
  | "custom";

export interface PanelDef {
  id: string;
  kind: PanelKind;
  /** Tyre temperature panel: centre only, or the three zones of each tyre. */
  zones?: boolean;
  /** Custom panel: the channels picked by the user. */
  channels?: string[];
}

/** One loaded lap, already resampled on the page's shared x axis. */
export interface LapView {
  label: "A" | "B";
  lapNumber: number;
  /** Display values of a channel on the shared x (already scaled by `displayFactor`). */
  get(channel: string): (number | null)[] | undefined;
}

export interface BuiltPanel {
  title: string;
  subtitle?: string;
  height: number;
  series: PanelSeries[];
  scales: PanelScale[];
}

const B_DASH = [7, 4];

const wheels = (base: string) => WHEEL_SUFFIXES.map((s) => `${base}${s}`);

/** Channels a panel needs from the server. */
export function panelChannels(def: PanelDef): string[] {
  switch (def.kind) {
    case "speed":
      return ["speed_kmh"];
    case "pedals":
      return ["throttle", "brake", "steering"];
    case "gearRpm":
      return ["rpm", "gear"];
    case "g":
      return ["g_lat", "g_long"];
    case "tyreTemp":
      return def.zones
        ? [...wheels("temp_l_c"), ...wheels("temp_c_c"), ...wheels("temp_r_c")]
        : wheels("temp_c_c");
    case "pressure":
      return wheels("pressure_kpa");
    case "brakeTemp":
      return wheels("brake_temp_c");
    case "slip":
      return wheels("slip_ratio");
    case "rideHeight":
      return wheels("ride_height_m");
    case "delta":
      return [];
    case "custom":
      return def.channels ?? [];
  }
}

export const DEFAULT_PANELS: PanelDef[] = [
  { id: "speed", kind: "speed" },
  { id: "pedals", kind: "pedals" },
  { id: "gearRpm", kind: "gearRpm" },
  { id: "g", kind: "g" },
  { id: "tyreTemp", kind: "tyreTemp", zones: false },
  { id: "pressure", kind: "pressure" },
  { id: "brakeTemp", kind: "brakeTemp" },
  { id: "slip", kind: "slip" },
  { id: "rideHeight", kind: "rideHeight" },
];

export const panelTitleKey = (kind: Exclude<PanelKind, "custom">): TranslationKey =>
  `panel.${kind}` as TranslationKey;

interface Spec {
  channel: string;
  label: string;
  unit: string;
  color: string;
  scale: string;
  step?: boolean;
  width?: number;
  alpha?: number;
  /** Extra dash in single-lap mode (tyre zones). */
  dash?: number[];
  legendFmt?: (v: number) => string;
}

function expand(specs: Spec[], laps: LapView[]): PanelSeries[] {
  const out: PanelSeries[] = [];
  const compare = laps.length > 1;
  for (const spec of specs) {
    for (const lap of laps) {
      const values = lap.get(spec.channel);
      if (!values) continue;
      out.push({
        key: spec.channel,
        label: spec.label,
        unit: spec.unit,
        color: spec.color,
        scale: spec.scale,
        lap: compare ? lap.label : undefined,
        dash: lap.label === "B" ? B_DASH : spec.dash,
        width: spec.width,
        alpha: spec.alpha,
        step: spec.step,
        legendFmt: spec.legendFmt,
        values,
      });
    }
  }
  return out;
}

function perWheel(base: string, lang: Lang, scale: string, unit: string): Spec[] {
  return WHEEL_SUFFIXES.map((s, i) => ({
    channel: `${base}${s}`,
    label: WHEEL_LABELS[lang][i] as string,
    unit,
    color: WHEEL_COLORS[i] as string,
    scale,
  }));
}

const symmetric = (min: number, max: number): [number, number] => {
  const m = Math.max(Math.abs(min), Math.abs(max), 0.1) * 1.08;
  return [-m, m];
};

export function buildPanel(
  def: PanelDef,
  laps: LapView[],
  lang: Lang,
  t: (k: TranslationKey, p?: Record<string, string | number>) => string,
  delta?: (number | null)[] | null,
): BuiltPanel | null {
  const title = def.kind === "custom" ? customTitle(def, lang) : t(panelTitleKey(def.kind));
  switch (def.kind) {
    case "speed":
      return {
        title,
        subtitle: "km/h",
        height: 220,
        scales: [{ key: "y", side: "left", range: (_min, max) => [0, Math.max(50, max) * 1.05] }],
        series: expand(
          [
            {
              channel: "speed_kmh",
              label: channelLabel("speed_kmh", lang),
              unit: "km/h",
              color: COLORS.cold,
              scale: "y",
            },
          ],
          laps,
        ),
      };
    case "pedals":
      return {
        title,
        subtitle: "%",
        height: 170,
        scales: [
          { key: "pct", side: "left", range: [0, 102] },
          { key: "steer", side: "right", range: [-1.05, 1.05], fmt: (v) => v.toFixed(1) },
        ],
        series: expand(
          [
            {
              channel: "throttle",
              label: t("inputs.throttle"),
              unit: "%",
              color: COLORS.optimal,
              scale: "pct",
            },
            {
              channel: "brake",
              label: t("inputs.brake"),
              unit: "%",
              color: COLORS.hot,
              scale: "pct",
            },
            {
              channel: "steering",
              label: t("inputs.steering"),
              unit: "",
              color: "#e9edf2",
              scale: "steer",
              width: 1,
              alpha: 0.6,
            },
          ],
          laps,
        ),
      };
    case "gearRpm":
      return {
        title,
        subtitle: "tr/min",
        height: 170,
        scales: [
          { key: "rpm", side: "left", range: (_min, max) => [0, max * 1.05] },
          {
            key: "gear",
            side: "right",
            range: (_min, max) => [0, Math.max(1, Math.ceil(max)) + 1],
            integer: true,
            fmt: (v) => String(v),
          },
        ],
        series: expand(
          [
            {
              channel: "rpm",
              label: t("inputs.rpm"),
              unit: "",
              color: COLORS.accent,
              scale: "rpm",
            },
            {
              channel: "gear",
              label: t("inputs.gear"),
              unit: "",
              color: "#e9edf2",
              scale: "gear",
              step: true,
              width: 1.2,
              alpha: 0.8,
              legendFmt: (v) => String(Math.round(v)),
            },
          ],
          laps,
        ),
      };
    case "g":
      return {
        title,
        subtitle: "g",
        height: 170,
        scales: [{ key: "g", side: "left", range: symmetric }],
        series: expand(
          [
            {
              channel: "g_lat",
              label: channelLabel("g_lat", lang),
              unit: "g",
              color: COLORS.best,
              scale: "g",
            },
            {
              channel: "g_long",
              label: channelLabel("g_long", lang),
              unit: "g",
              color: COLORS.optimal,
              scale: "g",
            },
          ],
          laps,
        ),
      };
    case "tyreTemp": {
      const specs: Spec[] = def.zones
        ? WHEEL_SUFFIXES.flatMap((s, i) => [
            {
              channel: `temp_l_c${s}`,
              label: `${WHEEL_LABELS[lang][i]} ${lang === "fr" ? "G" : "L"}`,
              unit: "°C",
              color: WHEEL_COLORS[i] as string,
              scale: "y",
              width: 1,
              alpha: 0.6,
              dash: [2, 3],
            },
            {
              channel: `temp_c_c${s}`,
              label: `${WHEEL_LABELS[lang][i]} C`,
              unit: "°C",
              color: WHEEL_COLORS[i] as string,
              scale: "y",
            },
            {
              channel: `temp_r_c${s}`,
              label: `${WHEEL_LABELS[lang][i]} ${lang === "fr" ? "D" : "R"}`,
              unit: "°C",
              color: WHEEL_COLORS[i] as string,
              scale: "y",
              width: 1,
              alpha: 0.6,
            },
          ])
        : perWheel("temp_c_c", lang, "y", "°C");
      return {
        title,
        subtitle: `°C · ${def.zones ? t("lap.tyreZones") : t("lap.tyreCentre")}`,
        height: 180,
        scales: [{ key: "y", side: "left" }],
        series: expand(specs, laps),
      };
    }
    case "pressure":
      return {
        title,
        subtitle: "kPa",
        height: 160,
        scales: [{ key: "y", side: "left" }],
        series: expand(perWheel("pressure_kpa", lang, "y", "kPa"), laps),
      };
    case "brakeTemp":
      return {
        title,
        subtitle: "°C",
        height: 170,
        scales: [{ key: "y", side: "left" }],
        series: expand(perWheel("brake_temp_c", lang, "y", "°C"), laps),
      };
    case "slip":
      return {
        title,
        height: 160,
        scales: [{ key: "y", side: "left", range: symmetric }],
        series: expand(perWheel("slip_ratio", lang, "y", ""), laps),
      };
    case "rideHeight":
      return {
        title,
        subtitle: "mm",
        height: 160,
        scales: [{ key: "y", side: "left" }],
        series: expand(perWheel("ride_height_m", lang, "y", "mm"), laps),
      };
    case "delta": {
      if (!delta) return null;
      const { neg, pos } = splitSign(delta);
      return {
        title,
        subtitle: `s · ${t("lap.deltaHint")}`,
        height: 150,
        scales: [{ key: "d", side: "left", range: symmetric, fmt: (v) => v.toFixed(2) }],
        series: [
          {
            key: "delta",
            label: "Δ",
            unit: "s",
            color: COLORS.faster,
            scale: "d",
            fill: "rgba(40,209,124,0.22)",
            values: neg,
            legendValues: delta,
            legendFmt: (v) => `${v > 0 ? "+" : v < 0 ? "−" : ""}${Math.abs(v).toFixed(3)}`,
            width: 1.4,
          },
          {
            key: "delta",
            label: "Δ",
            unit: "s",
            color: COLORS.slower,
            scale: "d",
            fill: "rgba(255,90,90,0.22)",
            values: pos,
            hideLegend: true,
            width: 1.4,
          },
        ],
      };
    }
    case "custom": {
      const channels = def.channels ?? [];
      // one scale per display unit; first unit on the left, second on the right, the rest left
      const units = [...new Set(channels.map((c) => displayUnit(c, unitHint(c))))];
      const scaleOf = (u: string) => (units.indexOf(u) === 1 ? "right" : "left");
      const scales: PanelScale[] = units.slice(0, 2).map((_u, i) => ({
        key: `u${i}`,
        side: i === 1 ? "right" : "left",
      }));
      if (scales.length === 0) scales.push({ key: "u0", side: "left" });
      const specs: Spec[] = channels.map((c, i) => {
        const u = displayUnit(c, unitHint(c));
        const wheel = splitWheel(c);
        const color =
          wheel && channels.length <= 4
            ? (WHEEL_COLORS[wheel.wheel] as string)
            : (SERIES_COLORS[i % SERIES_COLORS.length] as string);
        return {
          channel: c,
          label: channelLabel(c, lang),
          unit: u,
          color,
          scale: scaleOf(u) === "right" ? "u1" : "u0",
          step: isStepChannel(c),
        };
      });
      return {
        title,
        subtitle: units.filter(Boolean).join(" · ") || undefined,
        height: 180,
        scales,
        series: expand(specs, laps),
      };
    }
  }
}

/** Unit of a channel as the catalogue knows it (filled at runtime from `/api/channels`). */
const unitTable = new Map<string, string>();
export function rememberUnits(list: { name: string; unit: string }[]): void {
  for (const c of list) unitTable.set(c.name, c.unit);
}
export function unitHint(channel: string): string {
  return unitTable.get(channel) ?? "";
}

function customTitle(def: PanelDef, lang: Lang): string {
  const channels = def.channels ?? [];
  const bases = [...new Set(channels.map((c) => splitWheel(c)?.base ?? c))];
  const labels = bases.map((b) => channelLabel(b, lang));
  return labels.length <= 3
    ? labels.join(" · ")
    : `${labels.slice(0, 3).join(" · ")} +${labels.length - 3}`;
}

/** Scale factor applied to raw values before display, exposed for the data pipeline. */
export { displayFactor };
