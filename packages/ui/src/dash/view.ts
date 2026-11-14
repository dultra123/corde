import { useEffect, useState } from "react";
import { APP_BUILD } from "../mobile/env";
import type { Route } from "../nav/routes";
import { useSettings } from "../store/settings";
import { chooseView, type ViewContext } from "./model";

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
