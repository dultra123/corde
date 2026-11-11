import { useEffect, useMemo, useRef } from "react";
import uPlot from "uplot";
import { useT } from "../i18n";
import { COLORS } from "../lib/color";
import { fmtLapTime } from "../lib/format";
import { lapNo } from "../lib/laps";
import { useElementSize } from "../widgets/shared/hooks";
import { isCounted, type LapLike } from "./lib/stats";
import { AXIS_FONT, baseAxis } from "./uplot-theme";

interface Props {
  laps: readonly LapLike[];
  bestLapS: number | null;
  selected?: readonly number[];
  onToggle?: (lapNumber: number) => void;
  height?: number;
  /**
   * A small read-only version (home page): no legend, no axis title, no cursor, no click —
   * just the shape of the session.
   */
  compact?: boolean;
}

const NO_SELECTION: readonly number[] = [];

/**
 * Lap times over the session: counted laps as filled dots joined by a thin line, invalid / pit
 * laps as hollow dots, the best lap as a dashed purple line, selected laps in the accent colour.
 * Clicking a dot (de)selects the lap.
 */
export function LapTimesChart({
  laps,
  bestLapS,
  selected = NO_SELECTION,
  onToggle,
  height = 190,
  compact = false,
}: Props) {
  const { t } = useT();
  const hostRef = useRef<HTMLDivElement>(null);
  const plotRef = useRef<uPlot | null>(null);
  const size = useElementSize(hostRef);
  const toggleRef = useRef(onToggle);
  toggleRef.current = onToggle;

  const ordered = useMemo(() => [...laps].sort((a, b) => a.lap_number - b.lap_number), [laps]);

  const data = useMemo<uPlot.AlignedData>(() => {
    const x = ordered.map((l) => lapNo(l.lap_number));
    const counted = ordered.map((l) => (isCounted(l) ? l.lap_time_s : null));
    const other = ordered.map((l) => (isCounted(l) ? null : l.lap_time_s));
    const sel = ordered.map((l) => (selected.includes(l.lap_number) ? l.lap_time_s : null));
    const best = ordered.map(() => bestLapS);
    return [x, counted, other, sel, best];
  }, [ordered, bestLapS, selected]);

  // y range: pad around the counted laps so a 20 s pit lap does not flatten everything
  const yRange = useMemo<[number, number]>(() => {
    const vals = ordered.filter(isCounted).map((l) => l.lap_time_s);
    const all = ordered.map((l) => l.lap_time_s).filter((v) => v > 0);
    const src = vals.length ? vals : all;
    if (!src.length) return [0, 1];
    const lo = Math.min(...src);
    const hi = Math.max(...src);
    const span = Math.max(0.5, hi - lo);
    const top = Math.min(Math.max(...all), hi + span * 1.5);
    return [lo - span * 0.3, Math.max(top, hi) + span * 0.3];
  }, [ordered]);

  const dataRef = useRef(data);
  dataRef.current = data;
  const orderedRef = useRef(ordered);
  orderedRef.current = ordered;
  const rangeRef = useRef(yRange);
  rangeRef.current = yRange;
  const sizeRef = useRef(size);
  sizeRef.current = size;

  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;
    const dot = (stroke: string, fill: string, size = compact ? 7 : 9) => ({
      show: true,
      size,
      width: 1.5,
      stroke,
      fill,
      space: 0,
    });
    const opts: uPlot.Options = {
      width: Math.max(10, sizeRef.current.width),
      height,
      pxAlign: 0,
      padding: compact ? [8, 10, 0, 0] : [10, 12, 0, 0],
      legend: { show: false },
      cursor: compact
        ? { show: false }
        : {
            y: false,
            drag: { x: false, y: false, setScale: false },
            points: { size: 14, width: 2, stroke: COLORS.accent, fill: "transparent" },
          },
      select: { show: false, left: 0, top: 0, width: 0, height: 0 },
      scales: {
        x: { time: false, range: (_u, min, max) => [min - 0.5, max + 0.5] },
        y: { range: () => rangeRef.current },
      },
      axes: [
        {
          ...baseAxis(),
          size: compact ? 22 : 26,
          incrs: [1, 2, 5, 10, 20, 50],
          values: (_u, vals) => vals.map((v) => (Number.isInteger(v) ? String(v) : "")),
          ...(compact ? {} : { label: t("session.col.lap"), labelFont: AXIS_FONT, labelSize: 14 }),
        },
        {
          ...baseAxis(),
          size: compact ? 52 : 60,
          ...(compact ? { space: 26 } : {}),
          values: (_u, vals) => vals.map((v) => fmtLapTime(v, { ms: 1 })),
        },
      ],
      series: [
        { label: "lap" },
        {
          label: "counted",
          stroke: COLORS.dim,
          width: 1,
          points: dot(COLORS.text, COLORS.text),
          spanGaps: true,
        },
        {
          label: "other",
          stroke: "transparent",
          width: 0,
          points: dot(COLORS.faint, COLORS.panel),
        },
        {
          label: "selected",
          stroke: "transparent",
          width: 0,
          points: dot(COLORS.accent, COLORS.accent, 12),
        },
        {
          label: "best",
          stroke: COLORS.best,
          width: 1,
          dash: [5, 4],
          points: { show: false },
        },
      ],
    };
    const plot = new uPlot(opts, dataRef.current, host);
    plotRef.current = plot;
    const onClick = () => {
      const idx = plot.cursor.idx;
      if (idx === null || idx === undefined) return;
      const lap = orderedRef.current[idx];
      if (lap !== undefined) toggleRef.current?.(lap.lap_number);
    };
    if (!compact) plot.over.addEventListener("click", onClick);
    return () => {
      plot.over.removeEventListener("click", onClick);
      plot.destroy();
      plotRef.current = null;
    };
  }, [height, t, compact]);

  useEffect(() => {
    plotRef.current?.setData(data);
  }, [data]);

  useEffect(() => {
    const plot = plotRef.current;
    if (plot && size.width > 0) plot.setSize({ width: size.width, height });
  }, [size, height]);

  if (compact) {
    return (
      <div className="laptimes is-compact" aria-hidden="true">
        <div className="laptimes-plot" ref={hostRef} />
      </div>
    );
  }

  return (
    <div className="laptimes">
      <div className="laptimes-legend dim">
        <span>
          <i className="lg-dot is-filled" /> {t("session.legend.valid")}
        </span>
        <span>
          <i className="lg-dot is-hollow" /> {t("session.legend.invalid")}
        </span>
        <span>
          <i className="lg-line is-best" /> {t("session.legend.bestLine")}
        </span>
      </div>
      <div className="laptimes-plot" ref={hostRef} />
    </div>
  );
}
