import { createContext, useContext, useEffect, useLayoutEffect, useState } from "react";
import type { PageProps } from "../dash/parts";
import type { Lang } from "../store/settings";
import { useTelemetry } from "../store/telemetry";
import { useFrameDraw } from "../widgets/shared/hooks";
import type { FieldCtx } from "./fields";
import { type DashThreshold, validColor } from "./model";

/** Shared by the drawings of the elements (`elements.tsx`, `visuals.tsx`). */

export interface ElementProps {
  /** The type of the element (`widget.map`…). */
  type: string;
  props: Record<string, unknown>;
  data: PageProps;
  lang: Lang;
  /** The colour rules of the element (Valeur, Barre, Jauge circulaire). */
  thresholds?: readonly DashThreshold[];
}

/** A colour of the definition, or the fallback (`auto`, anything not `#rrggbb`). */
export function colorOr(v: unknown, fallback: string | undefined): string | undefined {
  return validColor(v) ? v : fallback;
}

export function readCtx(): FieldCtx {
  const s = useTelemetry.getState();
  return { frame: s.frame, laps: s.laps };
}

/**
 * `draw` on each new telemetry frame, and after every render of the element: a changed setting
 * (colour, data, decimals) shows at once, even while no frame arrives — the game closed, or the
 * Studio without the demonstration race. Without this, a new colour only showed after the
 * dashboard was opened again.
 */
export function useElementDraw(draw: () => void): void {
  useFrameDraw(draw);
  useLayoutEffect(() => {
    draw();
  });
}

/**
 * Whether a video may play here: `false` in the small previews of the library (a still
 * picture), `true` on a dashboard and in the editor.
 */
export const MediaMotion = createContext(true);

export function useMediaMotion(): boolean {
  return useContext(MediaMotion);
}

function query(q: string): boolean {
  return typeof window !== "undefined" && (window.matchMedia?.(q).matches ?? false);
}

/** `prefers-reduced-motion: reduce`, followed. */
export function useReducedMotion(): boolean {
  const [on, setOn] = useState(() => query("(prefers-reduced-motion: reduce)"));
  useEffect(() => {
    const m = window.matchMedia?.("(prefers-reduced-motion: reduce)");
    if (!m) return;
    const on2 = () => setOn(m.matches);
    m.addEventListener?.("change", on2);
    return () => m.removeEventListener?.("change", on2);
  }, []);
  return on;
}

/** The page is on screen (not a tab in the background, not an application sent away). */
export function usePageVisible(): boolean {
  const [on, setOn] = useState(() =>
    typeof document === "undefined" ? true : document.visibilityState !== "hidden",
  );
  useEffect(() => {
    const on2 = () => setOn(document.visibilityState !== "hidden");
    document.addEventListener("visibilitychange", on2);
    window.addEventListener("pagehide", on2);
    window.addEventListener("pageshow", on2);
    return () => {
      document.removeEventListener("visibilitychange", on2);
      window.removeEventListener("pagehide", on2);
      window.removeEventListener("pageshow", on2);
    };
  }, []);
  return on;
}
