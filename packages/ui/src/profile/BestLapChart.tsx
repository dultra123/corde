import { useEffect, useMemo, useRef, useState } from "react";
import uPlot from "uplot";
import { useT } from "../i18n";
import { COLORS } from "../lib/color";
import { fmtDate, fmtDateTime, fmtLapTime, pad2 } from "../lib/format";
import { navigate } from "../nav/routes";
import { baseAxis } from "../sessions/uplot-theme";
import { useElementSize } from "../widgets/shared/hooks";
import type { BestLapPoint } from "./lib";

interface Props {
  points: readonly BestLapPoint[];
  height?: number;
}

/**
 * Best lap of every session on one track, over time: races as filled accent dots, other
 * sessions as filled light dots, joined by a thin line, the all-time best as a dashed purple
 * line. Hovering shows the session under the chart; clicking opens its result.
 */
export function BestLapChart({ points, height = 200 }: Props) {
  const { t, lang } = useT();
  const hostRef = useRef<HTMLDivElement>(null);
  const plotRef = useRef<uPlot | null>(null);
  const size = useElementSize(hostRef);
  const [hover, setHover] = useState<BestLapPoint | null>(null);

  const data = useMemo<uPlot.AlignedData>(() => {
    const x = points.map((p) => p.t);
    const all = points.map((p) => p.bestLapS);
    const races = points.map((p) => (p.result.session_kind === "race" ? p.bestLapS : null));
    const others = points.map((p) => (p.result.session_kind === "race" ? null : p.bestLapS));
    return [x, all, others, races];
  }, [points]);
  const best = useMemo(
    () => (points.length ? Math.min(...points.map((p) => p.bestLapS)) : null),
    [points],
  );

  const yRange = useMemo<[number, number]>(() => {
    const vals = points.map((p) => p.bestLapS);
    if (!vals.length) return [0, 1];
    const lo = Math.min(...vals);
    const hi = Math.max(...vals);
    const span = Math.max(1, hi - lo);
    return [lo - span * 0.25, hi + span * 0.25];
  }, [points]);

  // At least a week on the x axis so that a lone session does not stretch over one day.
  const xRange = useMemo<[number, number]>(() => {
    const xs = points.map((p) => p.t);
    if (!xs.length) return [0, 1];
    const lo = Math.min(...xs);
    const hi = Math.max(...xs);
    const mid = (lo + hi) / 2;
    const half = Math.max(3.5 * 86_400, ((hi - lo) / 2) * 1.16);
    return [mid - half, mid + half];
  }, [points]);

  const dataRef = useRef(data);
  dataRef.current = data;
  const pointsRef = useRef(points);
  pointsRef.current = points;
  const yRef = useRef(yRange);
  yRef.current = yRange;
  const xRef = useRef(xRange);
  xRef.current = xRange;
  const sizeRef = useRef(size);
  sizeRef.current = size;
  const langRef = useRef(lang);
  langRef.current = lang;
  const bestRef = useRef(best);
  bestRef.current = best;

  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;
    const dot = (stroke: string, fill: string, size = 10) => ({
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
      padding: [10, 36, 0, 0],
      legend: { show: false },
      cursor: {
        y: false,
        drag: { x: false, y: false, setScale: false },
        points: { size: 16, width: 2, stroke: COLORS.accent, fill: "transparent" },
      },
      select: { show: false, left: 0, top: 0, width: 0, height: 0 },
      scales: {
        x: { time: true, range: () => xRef.current },
        y: { range: () => yRef.current },
      },
      axes: [
        {
          ...baseAxis(),
          size: 26,
          space: 90,
          values: (u, vals) => {
            // Ticks closer than a day apart need the time of day to be told apart.
            const span = (u.scales.x?.max ?? 0) - (u.scales.x?.min ?? 0);
            const withTime = span < 3 * 86_400;
            let lastDay = "";
            return vals.map((v) => {
              const d = new Date(v * 1000);
              const day = fmtDate(v * 1000, langRef.current);
              if (!withTime) return day;
              const time = `${pad2(d.getHours())}:${pad2(d.getMinutes())}`;
              if (day === lastDay) return time;
              lastDay = day;
              return `${day} ${time}`;
            });
          },
        },
        {
          ...baseAxis(),
          size: 60,
          values: (_u, vals) => vals.map((v) => fmtLapTime(v, { ms: 1 })),
        },
      ],
      series: [
        { label: "t" },
        {
          label: "all",
          stroke: COLORS.dim,
          width: 1,
          points: { show: false },
          spanGaps: true,
        },
        {
          label: "others",
          stroke: "transparent",
          width: 0,
          points: dot(COLORS.text, COLORS.text),
        },
        {
          label: "races",
          stroke: "transparent",
          width: 0,
          points: dot(COLORS.accent, COLORS.accent, 12),
        },
      ],
      hooks: {
        // The record as a dashed line across the plot (a series needs two points to draw).
        draw: [
          (u) => {
            const b = bestRef.current;
            if (b === null) return;
            const y = u.valToPos(b, "y", true);
            const { ctx } = u;
            ctx.save();
            ctx.strokeStyle = COLORS.best;
            ctx.lineWidth = 1;
            ctx.setLineDash([5, 4]);
            ctx.beginPath();
            ctx.moveTo(u.bbox.left, y);
            ctx.lineTo(u.bbox.left + u.bbox.width, y);
            ctx.stroke();
            ctx.restore();
          },
        ],
        setCursor: [
          (u) => {
            const idx = u.cursor.idx;
            setHover(idx === null || idx === undefined ? null : (pointsRef.current[idx] ?? null));
          },
        ],
      },
    };
    const plot = new uPlot(opts, dataRef.current, host);
    plotRef.current = plot;
    const onClick = () => {
      const idx = plot.cursor.idx;
      if (idx === null || idx === undefined) return;
      const p = pointsRef.current[idx];
      if (p) navigate({ page: "result", id: p.result.id });
    };
    plot.over.addEventListener("click", onClick);
    plot.over.style.cursor = "pointer";
    return () => {
      plot.over.removeEventListener("click", onClick);
      plot.destroy();
      plotRef.current = null;
    };
  }, [height]);

  // `best` changes with `data` and is read by the draw hook through `bestRef`.
  useEffect(() => {
    plotRef.current?.setData(data);
  }, [data]);

  useEffect(() => {
    const plot = plotRef.current;
    if (plot && size.width > 0) plot.setSize({ width: size.width, height });
  }, [size, height]);

  const last = points.at(-1) ?? null;
  const shown = hover ?? last;
  return (
    <div className="bestlap">
      <div className="laptimes-legend dim">
        <span>
          <i className="lg-dot is-race" /> {t("profile.chart.race")}
        </span>
        <span>
          <i className="lg-dot is-filled" /> {t("profile.chart.other")}
        </span>
        <span>
          <i className="lg-line is-best" /> {t("profile.chart.best")}
        </span>
      </div>
      <div className="laptimes-plot" ref={hostRef} />
      <div className="bestlap-hover num dim" aria-live="polite">
        {shown ? (
          <>
            <span>{fmtDateTime(shown.result.started_wall_ms, lang)}</span>
            <span className="is-strong" style={{ color: COLORS.best }}>
              {fmtLapTime(shown.bestLapS)}
            </span>
            <span>{shown.result.player_vehicle}</span>
            <span>{t(`session.type.${shown.result.session_kind}`)}</span>
            {hover && <span className="faint">{t("profile.chart.open")}</span>}
          </>
        ) : (
          <span>{t("profile.chart.empty")}</span>
        )}
      </div>
    </div>
  );
}
