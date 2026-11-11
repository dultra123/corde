import { isOverlayHash, type OverlayOptions, parseOverlayOptions } from "./model";

/**
 * Imported first by `main.tsx`, before any other module runs: an overlay page (OBS browser
 * source) must be transparent from its very first paint, and its pixel ratio must be set before
 * uPlot reads it at load time.
 *
 * `scale` enlarges the widgets with a CSS transform. Canvases (traces, map) would then be
 * stretched and blurry, so the page announces a pixel ratio multiplied by the same factor:
 * `fitCanvas` and uPlot size their backing stores for it and the scaled canvases land 1:1 on
 * the screen. Only overlay pages do this.
 */

/** Whether the page was opened as an overlay (decided once, at load time). */
export const OVERLAY_AT_BOOT: boolean =
  typeof window !== "undefined" && isOverlayHash(window.location.hash);

/** The options the page was opened with (`null` for the normal app). */
export const BOOT_OPTIONS: OverlayOptions | null = OVERLAY_AT_BOOT
  ? parseOverlayOptions(window.location.search, window.location.hash)
  : null;

if (OVERLAY_AT_BOOT && BOOT_OPTIONS) {
  const root = document.documentElement;
  root.classList.add("is-overlay");
  root.style.background = "transparent";
  const scale = BOOT_OPTIONS.scale;
  if (scale !== 1) {
    const real = window.devicePixelRatio || 1;
    try {
      Object.defineProperty(window, "devicePixelRatio", {
        configurable: true,
        get: () => real * scale,
      });
    } catch {
      // a browser that refuses: the scaled canvases are only a little softer
    }
  }
}
