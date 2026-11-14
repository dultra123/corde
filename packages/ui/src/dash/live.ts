import { useEffect, useState } from "react";
import { type TelemetryState, useTelemetry } from "../store/telemetry";

/** How often the slow parts of the dashboard (positions, temperatures, fuel…) are redrawn. */
export const SLOW_HZ = 4;

/**
 * A value of the telemetry store, read again at most `hz` times a second: the dashboard's
 * React tree re-renders at that pace, whatever the frame rate of the server (up to 60 Hz).
 * The fast values (gear, speed, revs, delta, lap time) do not go through React at all: they
 * are written into the page by `useFrameDraw` (`widgets/shared/hooks.ts`), once per animation
 * frame that brought a new telemetry frame.
 */
export function useThrottled<T>(select: (s: TelemetryState) => T, hz = SLOW_HZ): T {
  const [value, setValue] = useState(() => select(useTelemetry.getState()));
  // biome-ignore lint/correctness/useExhaustiveDependencies: `select` is read at each change
  useEffect(() => {
    const period = 1000 / hz;
    let timer: ReturnType<typeof setTimeout> | null = null;
    let last = 0;
    const flush = () => {
      timer = null;
      last = performance.now();
      setValue(select(useTelemetry.getState()));
    };
    const unsub = useTelemetry.subscribe(() => {
      if (timer) return;
      timer = setTimeout(flush, Math.max(0, last + period - performance.now()));
    });
    flush();
    return () => {
      unsub();
      if (timer) clearTimeout(timer);
    };
  }, [hz]);
  return value;
}

/** Writes `text` into `el` only when it changed (no style or layout work otherwise). */
export function setText(el: HTMLElement | null, text: string): void {
  if (el && el.textContent !== text) el.textContent = text;
}

/** Sets `el.dataset[key]` only when it changed (the CSS colours follow the data attributes). */
export function setData(el: HTMLElement | null, key: string, value: string): void {
  if (el && el.dataset[key] !== value) el.dataset[key] = value;
}
