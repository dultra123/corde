import { lazy } from "react";

/** The race dashboard: its own chunk, loaded the first time it is shown (never on most PCs). */
export const DashView = lazy(() => import("./DashView").then((m) => ({ default: m.DashView })));
