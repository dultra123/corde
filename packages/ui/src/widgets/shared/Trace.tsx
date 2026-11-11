import { useEffect, useRef } from "react";
import uPlot from "uplot";
import { COLORS } from "../../lib/color";
import type { RingBuffer } from "../../lib/ring-buffer";
import { useElementSize, useFrameDraw } from "./hooks";

export interface TraceSeries {
  buffer: RingBuffer;
  color: string;
  width?: number;
  /** Maps raw sample values to the 0..1 plot scale. */
  map?: (v: number) => number;
  fill?: string;
}

interface Props {
  series: TraceSeries[];
  /** Window, seconds. */
  windowS: number;
  /** Horizontal guide lines on the 0..1 scale. */
  guides?: number[];
}

/**
 * A rolling uPlot strip chart over the trace ring buffers. Scales are fixed (0..1 vertically,
 * the last `windowS` seconds horizontally) so the chart never jumps.
 */
export function Trace({ series, windowS, guides = [0.5] }: Props) {
  const hostRef = useRef<HTMLDivElement>(null);
  const plotRef = useRef<uPlot | null>(null);
  const rangeRef = useRef<[number, number]>([0, windowS]);
  const size = useElementSize(hostRef);
  const seriesRef = useRef(series);
  seriesRef.current = series;
  const windowRef = useRef(windowS);
  windowRef.current = windowS;
  const guidesRef = useRef(guides);
  guidesRef.current = guides;
  // mount-time size only; later changes go through `setSize` below
  const sizeRef = useRef(size);
  sizeRef.current = size;

  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;
    const opts: uPlot.Options = {
      width: Math.max(10, sizeRef.current.width),
      height: Math.max(10, sizeRef.current.height),
      pxAlign: 0,
      cursor: { show: false },
      legend: { show: false },
      select: { show: false, left: 0, top: 0, width: 0, height: 0 },
      padding: [2, 2, 2, 2],
      axes: [{ show: false }, { show: false }],
      scales: {
        x: { time: false, range: () => rangeRef.current },
        y: { range: [0, 1] },
      },
      series: [
        {},
        ...seriesRef.current.map((s) => ({
          stroke: s.color,
          width: s.width ?? 1.5,
          fill: s.fill,
          points: { show: false },
          spanGaps: true,
        })),
      ],
      hooks: {
        drawClear: [
          (u) => {
            const ctx = u.ctx;
            ctx.save();
            ctx.strokeStyle = COLORS.line;
            ctx.lineWidth = 1;
            for (const g of guidesRef.current) {
              const y = u.valToPos(g, "y", true);
              ctx.beginPath();
              ctx.moveTo(u.bbox.left, y);
              ctx.lineTo(u.bbox.left + u.bbox.width, y);
              ctx.stroke();
            }
            ctx.restore();
          },
        ],
      },
    };
    const data: uPlot.AlignedData = [[], ...seriesRef.current.map(() => [])];
    const plot = new uPlot(opts, data, host);
    plotRef.current = plot;
    return () => {
      plot.destroy();
      plotRef.current = null;
    };
  }, []);

  useEffect(() => {
    const plot = plotRef.current;
    if (plot && size.width > 0 && size.height > 0) {
      plot.setSize({ width: size.width, height: size.height });
    }
  }, [size]);

  useFrameDraw(() => {
    const plot = plotRef.current;
    if (!plot) return;
    const list = seriesRef.current;
    const first = list[0];
    if (!first) return;
    const last = first.buffer.last();
    const now = last ? last.t : 0;
    const win = windowRef.current;
    const since = now - win;
    rangeRef.current = [since, now];
    const base = first.buffer.since(since);
    const cols: (number | null)[][] = [base.t];
    for (const s of list) {
      const { v } = s.buffer.since(since);
      const map = s.map;
      // buffers are pushed together, lengths match; guard anyway
      const arr: (number | null)[] = new Array(base.t.length);
      for (let i = 0; i < base.t.length; i += 1) {
        const raw = v[i];
        arr[i] = raw === undefined ? null : map ? map(raw) : raw;
      }
      cols.push(arr);
    }
    plot.setData(cols as uPlot.AlignedData, true);
  });

  return <div className="trace" ref={hostRef} />;
}
