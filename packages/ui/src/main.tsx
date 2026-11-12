// First: an overlay page (OBS) must be transparent and set its pixel ratio before anything loads.
import { BOOT_OPTIONS, OVERLAY_AT_BOOT } from "./overlay/boot";
import "@fontsource/barlow/400.css";
import "@fontsource/barlow/500.css";
import "@fontsource/barlow/600.css";
import "@fontsource/barlow-semi-condensed/500.css";
import "@fontsource/barlow-semi-condensed/600.css";
import "@fontsource/barlow-semi-condensed/700.css";
import "gridstack/dist/gridstack.min.css";
import "uplot/dist/uPlot.min.css";
import "./styles/tokens.css";
import "./styles/base.css";
import "./styles/dashboard.css";
import "./styles/widgets.css";
import "./styles/sessions.css";
import "./styles/profile.css";
import "./styles/engineer.css";
import "./styles/home.css";
import "./styles/races.css";
import "./styles/obs.css";
import "./styles/overlay.css";
import "./styles/hub.css";

import { createRoot } from "react-dom/client";
import { App } from "./App";
import { APP_BUILD } from "./mobile/env";
import { startAutoNav } from "./nav/auto";
import { isEmptyHash } from "./nav/routes";
import { connection } from "./net/connection";
import { isDemoForced } from "./net/url";
import { isOverlayHash } from "./overlay/model";
import { OverlayApp } from "./overlay/OverlayApp";
import { useSettings } from "./store/settings";
import "./widgets";

const root = document.getElementById("root");
if (!root) throw new Error("#root missing");

/** A preview of the OBS page (`?preview=1`): demo data, frozen after a few seconds. */
const PREVIEW_FREEZE_MS = 6000;

// The overlay and the app are two different pages: moving from one to the other reloads.
window.addEventListener("hashchange", () => {
  if (isOverlayHash(window.location.hash) !== OVERLAY_AT_BOOT) window.location.reload();
});

if (OVERLAY_AT_BOOT) {
  // `lang=` is for this page only, never saved.
  if (BOOT_OPTIONS?.lang) useSettings.setState({ lang: BOOT_OPTIONS.lang });
  const preview = new URLSearchParams(window.location.search).get("preview") === "1";
  connection.start({
    overlay: true,
    freezeDemoAfterMs: preview && isDemoForced() ? PREVIEW_FREEZE_MS : undefined,
  });
  createRoot(root).render(<OverlayApp />);
} else {
  const start = () => {
    // `?demo=1` is a request to see the demo race: open the dashboard rather than the home page.
    if (isDemoForced() && isEmptyHash(window.location.hash)) {
      window.history.replaceState(
        null,
        "",
        `${window.location.pathname}${window.location.search}#/`,
      );
    }
    // Before the first render, while the hash still is what the app was opened with.
    startAutoNav();
    connection.start({ noAutoDemo: APP_BUILD });
    createRoot(root).render(<App />);
  };
  if (import.meta.env.MODE === "mobile") {
    // The iPhone application (`vite build --mode mobile`): its own start first. In the browser
    // build this branch and the module it loads are left out.
    import("./mobile/boot")
      .then((m) => m.bootApp())
      .catch((e: unknown) => console.error("[corde] start of the application failed", e))
      .finally(start);
  } else {
    start();
  }
}
