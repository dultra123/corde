import { useEffect, useState } from "react";
import { APP_BUILD } from "../mobile/env";
import type { Route } from "../nav/routes";
import { useSettings } from "../store/settings";
import { chooseView, dashPresentation, type ViewContext } from "./model";
import { useDashFull } from "./store";

type Measured = Omit<ViewContext, "forced" | "app">;

function measure(): Measured {
  if (typeof window === "undefined") return { coarse: false, width: 1280, height: 800 };
  return {
    coarse: window.matchMedia?.("(pointer: coarse)").matches ?? false,
    width: window.innerWidth,
    height: window.innerHeight,
  };
}

/** The pointer and the size of the window, followed (resize, turn, pointer change). */
function useMeasured(): Measured {
  const [ctx, setCtx] = useState(measure);
  useEffect(() => {
    const on = () =>
      setCtx((prev) => {
        const next = measure();
        return prev.coarse === next.coarse &&
          prev.width === next.width &&
          prev.height === next.height
          ? prev
          : next;
      });
    window.addEventListener("resize", on);
    const mq = window.matchMedia?.("(pointer: coarse)");
    mq?.addEventListener?.("change", on);
    return () => {
      window.removeEventListener("resize", on);
      mq?.removeEventListener?.("change", on);
    };
  }, []);
  return ctx;
}

/**
 * What the Direct page shows on this device (`chooseView`): followed live, so that a window
 * resized or a tablet turned follows the rule.
 */
export function useLiveView(route: Route): "dash" | "widgets" {
  const setting = useSettings((s) => s.liveView);
  const ctx = useMeasured();
  const forced = route.page === "dashboard" && route.dash === true;
  return chooseView(setting, { ...ctx, app: APP_BUILD, forced });
}

/**
 * `true` where « Automatique » picks the phone dashboard: the grid of widgets then offers the
 * way back to it (a PC never sees that button).
 */
export function useAutoPicksDash(): boolean {
  const ctx = useMeasured();
  return chooseView("auto", { ...ctx, app: APP_BUILD, forced: false }) === "dash";
}

export type LiveMode = "widgets" | "dash-page" | "dash-full";

/**
 * What the Direct page shows and how: the grid of widgets, the dashboard inside the page (PC,
 * tablet: the top bar stays) or the dashboard in full screen (phone, `#/dash`, or asked for).
 */
export function useLiveMode(route: Route): LiveMode {
  const setting = useSettings((s) => s.liveView);
  const full = useDashFull((s) => s.full);
  const ctx = useMeasured();
  const forced = route.page === "dashboard" && route.dash === true;
  const vctx = { ...ctx, app: APP_BUILD, forced };
  if (chooseView(setting, vctx) === "widgets") return "widgets";
  return dashPresentation(vctx, full) === "full" ? "dash-full" : "dash-page";
}

/**
 * The full screen can be left (a PC or a tablet asked for it): not on a phone, not at `#/dash`.
 */
export function useFullToggle(route: Route): boolean {
  const ctx = useMeasured();
  const forced = route.page === "dashboard" && route.dash === true;
  return dashPresentation({ ...ctx, app: APP_BUILD, forced }, false) === "page";
}

/**
 * The menu of the Direct page (and the ☰ sheet) switch with the same setting as the Settings
 * (« Affichage du Direct »): one state, never two that disagree.
 */
export function showDash(): void {
  useSettings.getState().update({ liveView: "dash" });
}

export function showWidgets(): void {
  useDashFull.getState().setFull(false);
  useSettings.getState().update({ liveView: "widgets" });
}
