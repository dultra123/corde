import { type ReactNode, useEffect, useMemo, useRef } from "react";
import uPlot from "uplot";
import { IconClose } from "../dashboard/icons";
import { useT } from "../i18n";
import { COLORS } from "../lib/color";
import { useElementSize } from "../widgets/shared/hooks";
import { baseAxis, fmtTick, fmtValue } from "./uplot-theme";

export type XUnit = "m" | "s";

export interface PanelSeries {
  /** Channel key: series of lap A and lap B share it (one legend entry, two values). */
  key: string;
  label: string;
  unit: string;
  color: string;
  scale: string;
  /** "A" / "B" in comparison mode. */
  lap?: string;
  dash?: number[];
  width?: number;
  alpha?: number;
  step?: boolean;
  fill?: string;
  values: (number | null)[];
  /** Values shown in the legend when they differ from the plotted ones (delta halves). */
  legendValues?: (number | null)[];
  /** Legend number format (defaults to `fmtValue`). */
  legendFmt?: (v: number) => string;
  /** Leave this series out of the legend. */
  hideLegend?: boolean;
}

export interface PanelScale {
  key: string;
  side: "left" | "right";
  /** Fixed range, or a function of the data extent. */
  range?: [number, number] | ((min: number, max: number) => [number, number]);
  /** Tick formatter (defaults to `fmtTick`). */
  fmt?: (v: number) => string;
  /** Integer ticks only (gears). */
  integer?: boolean;
}

interface Props {
  title: string;
  subtitle?: string;
  x: number[];
  xUnit: XUnit;
  series: PanelSeries[];
  scales: PanelScale[];
  height: number;
  syncKey: string;
  showXAxis: boolean;
  /** Called with the zoomed x range, or `null` when back to the full extent. */
  onZoom?: (range: [number, number] | null) => void;
  /** Registry of live plots (for "reset zoom" across every panel). */
  register: (plot: uPlot, on: boolean) => void;
  onRemove?: () => void;
  controls?: ReactNode;
}

const LEFT_AXIS = 56;
const RIGHT_AXIS = 46;

const fmtX = (v: number, unit: XUnit) =>
  unit === "m" ? `${Math.round(v)} m` : `${v.toFixed(v < 10 ? 2 : 1)} s`;

function withAlpha(hex: string, alpha: number): string {
  const h = hex.replace("#", "");
  const r = Number.parseInt(h.slice(0, 2), 16);
  const g = Number.parseInt(h.slice(2, 4), 16);
  const b = Number.parseInt(h.slice(4, 6), 16);
  return `rgba(${r},${g},${b},${alpha})`;
}

/**
 * One stacked telemetry chart: shared x (distance or lap time), one or two y scales, a cursor
 * synchronised with every other panel of the page through `syncKey`, drag-to-zoom, legend in
 * the header updated on hover without re-rendering React.
 */
export function ChartPanel({
  title,
  subtitle,
  x,
  xUnit,
  series,
  scales,
  height,
  syncKey,
  showXAxis,
  onZoom,
  register,
  onRemove,
  controls,
}: Props) {
  const { t } = useT();
  const hostRef = useRef<HTMLDivElement>(null);
  const plotRef = useRef<uPlot | null>(null);
  const size = useElementSize(hostRef);
  const sizeRef = useRef(size);
  sizeRef.current = size;
  const legendEls = useRef(new Map<string, HTMLElement>());
  const xEl = useRef<HTMLElement>(null);
  const onZoomRef = useRef(onZoom);
  onZoomRef.current = onZoom;
  const registerRef = useRef(register);
  registerRef.current = register;

  const data = useMemo<uPlot.AlignedData>(() => [x, ...series.map((s) => s.values)], [x, series]);
  const dataRef = useRef(data);
  dataRef.current = data;
  const seriesRef = useRef(series);
  seriesRef.current = series;

  // Legend entries: one per channel key, in first-seen order.
  const legend = useMemo(() => {
    const items: { key: string; label: string; unit: string; color: string; laps: string[] }[] = [];
    for (const s of series) {
      if (s.hideLegend) continue;
      let item = items.find((i) => i.key === s.key);
      if (!item) {
        item = { key: s.key, label: s.label, unit: s.unit, color: s.color, laps: [] };
        items.push(item);
      }
      item.laps.push(s.lap ?? "");
    }
    return items;
  }, [series]);

  // Structural signature: a change recreates the plot (series count, scales, axis, dashes…).
  const signature = [
    xUnit,
    height,
    showXAxis,
    scales.map((s) => `${s.key}:${s.side}:${s.integer ? 1 : 0}`).join("|"),
    series
      .map(
        (s) =>
          `${s.key}:${s.lap ?? ""}:${s.scale}:${s.color}:${s.dash?.join(".") ?? ""}:${s.step ? 1 : 0}:${s.fill ?? ""}`,
      )
      .join("|"),
  ].join("#");

  // biome-ignore lint/correctness/useExhaustiveDependencies: the signature captures what matters
  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;
    const hasRight = scales.some((s) => s.side === "right");
    const scaleOpts: Record<string, uPlot.Scale> = { x: { time: false } };
    for (const s of scales) {
      scaleOpts[s.key] = {
        range:
          s.range === undefined
            ? (_u, min, max) => padRange(min, max)
            : typeof s.range === "function"
              ? (_u, min, max) => (s.range as (a: number, b: number) => [number, number])(min, max)
              : s.range,
      };
    }
    const axes: uPlot.Axis[] = [
      {
        ...baseAxis(),
        show: showXAxis,
        size: 28,
        values: (_u, vals) => vals.map((v) => fmtX(v, xUnit)),
      },
      ...scales.map<uPlot.Axis>((s) => ({
        ...baseAxis(),
        scale: s.key,
        side: s.side === "right" ? 1 : 3,
        size: s.side === "right" ? RIGHT_AXIS : LEFT_AXIS,
        grid: { show: s.side === "left", stroke: COLORS.line, width: 1 },
        incrs: s.integer ? [1, 2, 5, 10] : undefined,
        values: (_u, vals) => vals.map((v) => (s.fmt ? s.fmt(v) : fmtTick(v))),
      })),
    ];
    const opts: uPlot.Options = {
      width: Math.max(10, sizeRef.current.width),
      height,
      pxAlign: 0,
      // same plot width with or without a right axis, so cursors line up across panels
      padding: [8, hasRight ? 0 : RIGHT_AXIS, showXAxis ? 0 : 4, 0],
      legend: { show: false },
      cursor: {
        sync: { key: syncKey, setSeries: false, scales: ["x", null] },
        drag: { x: true, y: false, setScale: true, dist: 4 },
        points: { size: 7, width: 1.5, fill: (u, i) => u.series[i]?.stroke as string },
        y: false,
      },
      scales: scaleOpts,
      axes,
      series: [
        {},
        ...seriesRef.current.map<uPlot.Series>((s) => ({
          label: s.label,
          scale: s.scale,
          stroke: s.alpha !== undefined ? withAlpha(s.color, s.alpha) : s.color,
          width: s.width ?? (s.dash ? 1.4 : 1.6),
          dash: s.dash,
          fill: s.fill,
          points: { show: false },
          spanGaps: false,
          paths: s.step ? uPlot.paths.stepped?.({ align: 1 }) : undefined,
        })),
      ],
      hooks: {
        setCursor: [
          (u) => {
            const idx = u.cursor.idx;
            const els = legendEls.current;
            if (xEl.current)
              xEl.current.textContent =
                idx === null || idx === undefined ? "" : fmtX(u.data[0][idx] as number, xUnit);
            seriesRef.current.forEach((s, i) => {
              if (s.hideLegend) return;
              const el = els.get(`${s.key}:${s.lap ?? ""}`);
              if (!el) return;
              const src = s.legendValues ?? (u.data[i + 1] as (number | null)[] | undefined);
              const v = idx === null || idx === undefined ? null : (src?.[idx] ?? null);
              el.textContent =
                v === null || !Number.isFinite(v)
                  ? "—"
                  : s.legendFmt
                    ? `${s.legendFmt(v)}${s.unit ? ` ${s.unit}` : ""}`
                    : fmtValue(v, s.unit);
            });
          },
        ],
        setScale: [
          (u, key) => {
            if (key !== "x") return;
            const xs = u.data[0];
            const n = xs.length;
            if (n < 2) return;
            const sx = u.scales.x;
            const min = sx?.min;
            const max = sx?.max;
            if (min === undefined || max === undefined || min === null || max === null) return;
            const full = min <= (xs[0] as number) && max >= (xs[n - 1] as number);
            onZoomRef.current?.(full ? null : [min, max]);
          },
        ],
      },
    };
    const plot = new uPlot(opts, dataRef.current, host);
    plotRef.current = plot;
    registerRef.current(plot, true);
    return () => {
      registerRef.current(plot, false);
      plot.destroy();
      plotRef.current = null;
    };
  }, [signature, syncKey]);

  // New values (finer resolution, other lap…): keep the current zoom.
  useEffect(() => {
    const plot = plotRef.current;
    if (!plot) return;
    const sx = plot.scales.x;
    const xs = data[0];
    const curMin = sx?.min ?? null;
    const curMax = sx?.max ?? null;
    const old = plot.data[0];
    const wasZoomed =
      curMin !== null &&
      curMax !== null &&
      old.length > 1 &&
      (curMin > (old[0] as number) || curMax < (old[old.length - 1] as number));
    plot.setData(data, !wasZoomed);
    if (wasZoomed && xs.length > 1) {
      const lo = Math.max(xs[0] as number, curMin);
      const hi = Math.min(xs[xs.length - 1] as number, curMax);
      if (hi > lo) plot.setScale("x", { min: lo, max: hi });
    }
  }, [data]);

  useEffect(() => {
    const plot = plotRef.current;
    if (plot && size.width > 0) plot.setSize({ width: size.width, height });
  }, [size, height]);

  const compare = legend.some((i) => i.laps.length > 1);

  return (
    <section className="cp">
      <header className="cp-head">
        <div className="cp-titles">
          <h3 className="cp-title">{title}</h3>
          {subtitle && <span className="cp-sub faint">{subtitle}</span>}
        </div>
        <div className="cp-side">
          <span className="cp-x num dim" ref={xEl} />
          {controls}
          {onRemove && (
            <button
              type="button"
              className="btn is-icon cp-remove"
              onClick={onRemove}
              aria-label={t("lap.removePanel")}
              title={t("lap.removePanel")}
            >
              <IconClose />
            </button>
          )}
        </div>
        <div className="cp-legend">
          {legend.map((item) => (
            <span key={item.key} className="cp-item" title={item.label}>
              <i className="cp-swatch" style={{ background: item.color }} />
              <span className="cp-item-label">{item.label}</span>
              {item.laps.map((lap) => (
                <b
                  key={lap}
                  className={`cp-val num${lap === "B" ? " is-b" : ""}`}
                  ref={(el) => {
                    const k = `${item.key}:${lap}`;
                    if (el) legendEls.current.set(k, el);
                    else legendEls.current.delete(k);
                  }}
                >
                  —
                </b>
              ))}
            </span>
          ))}
          {compare && (
            <span className="cp-ab faint">
              <i className="cp-swatch is-solid" /> A <i className="cp-swatch is-dash" /> B
            </span>
          )}
        </div>
      </header>
      <div className="cp-plot" ref={hostRef} style={{ height }} />
    </section>
  );
}

/** Auto range with 6 % headroom (and a non-zero span for flat channels). */
function padRange(min: number, max: number): [number, number] {
  if (!Number.isFinite(min) || !Number.isFinite(max)) return [0, 1];
  if (min === max) {
    const d = Math.abs(min) * 0.1 || 1;
    return [min - d, max + d];
  }
  const pad = (max - min) * 0.06;
  return [min - pad, max + pad];
}
