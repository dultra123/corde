import { type RefObject, useEffect, useRef, useState } from "react";
import { useTelemetry } from "../../store/telemetry";

/**
 * Runs `draw` on every animation frame in which a new telemetry frame arrived (and once on
 * mount). Canvas / uPlot widgets use this instead of React state so they never re-render.
 */
export function useFrameDraw(draw: (tick: number) => void): void {
  const drawRef = useRef(draw);
  drawRef.current = draw;
  useEffect(() => {
    let raf = 0;
    let pending = useTelemetry.getState().frameTick;
    let drawn = -1;
    const loop = () => {
      raf = 0;
      if (pending !== drawn) {
        drawn = pending;
        drawRef.current(pending);
      }
    };
    const schedule = () => {
      if (!raf) raf = requestAnimationFrame(loop);
    };
    const unsub = useTelemetry.subscribe((s) => {
      if (s.frameTick !== pending) {
        pending = s.frameTick;
        schedule();
      }
    });
    schedule();
    return () => {
      unsub();
      if (raf) cancelAnimationFrame(raf);
    };
  }, []);
}

export interface Size {
  width: number;
  height: number;
}

/** Observed content box size of an element (0×0 until measured). */
export function useElementSize<T extends HTMLElement>(ref: RefObject<T | null>): Size {
  const [size, setSize] = useState<Size>({ width: 0, height: 0 });
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const ro = new ResizeObserver((entries) => {
      const e = entries[0];
      if (!e) return;
      const { width, height } = e.contentRect;
      setSize((prev) =>
        Math.abs(prev.width - width) < 0.5 && Math.abs(prev.height - height) < 0.5
          ? prev
          : { width, height },
      );
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, [ref]);
  return size;
}

/** Device pixel ratio aware canvas sizing. Returns the 2D context or null. */
export function fitCanvas(canvas: HTMLCanvasElement, size: Size): CanvasRenderingContext2D | null {
  const dpr = Math.min(3, window.devicePixelRatio || 1);
  const w = Math.max(1, Math.round(size.width * dpr));
  const h = Math.max(1, Math.round(size.height * dpr));
  if (canvas.width !== w || canvas.height !== h) {
    canvas.width = w;
    canvas.height = h;
  }
  const ctx = canvas.getContext("2d");
  if (!ctx) return null;
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  return ctx;
}
