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

import { createRoot } from "react-dom/client";
import { App } from "./App";
import { startAutoNav } from "./nav/auto";
import { isEmptyHash } from "./nav/routes";
import { connection } from "./net/connection";
import { isDemoForced } from "./net/url";
import "./widgets";

const root = document.getElementById("root");
if (!root) throw new Error("#root missing");

// `?demo=1` is a request to see the demo race: open the dashboard rather than the home page.
if (isDemoForced() && isEmptyHash(window.location.hash)) {
  window.history.replaceState(null, "", `${window.location.pathname}${window.location.search}#/`);
}

// Before the first render, while the hash still is what the app was opened with.
startAutoNav();
connection.start();
createRoot(root).render(<App />);
