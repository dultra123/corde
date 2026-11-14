import { lazy, Suspense, useEffect } from "react";
import { DashView } from "./dash/lazy";
import { useLiveMode } from "./dash/view";
import { Dashboard } from "./dashboard/Dashboard";
import { PairingScreen } from "./hub/PairingScreen";
import { appSlots, useAppServerGate } from "./mobile/slots";
import { useRoute } from "./nav/routes";
import { ReminderBanner } from "./races/ReminderBanner";
import { SessionsShell } from "./sessions/SessionsShell";
import { useDashboard } from "./store/dashboard";
import { useHub } from "./store/hub";
import { useSettings } from "./store/settings";
import { useUpdates } from "./store/updates";

/** The Studio (editor of race dashboards, PC): its own chunk. */
const StudioPage = lazy(() =>
  import("./studio/StudioPage").then((m) => ({ default: m.StudioPage })),
);

/**
 * Five sections behind hash routes: the home page (`#/home`, shown when the app opens), the live
 * dashboard (`#/`), the recorded sessions (`#/sessions…`), the online races (`#/races`) and the
 * driver profile built from the game's results files (`#/profile`, `#/results/<id>`). The race
 * reminders (`ReminderBanner`) show over every page. The WebSocket connection lives outside
 * React (`net/connection`), so leaving the dashboard never interrupts the stream: the store keeps
 * filling up and the widgets pick it up again when the tab comes back. `nav/auto` moves between
 * the sections on its own when the driver takes the track or comes back to the garage.
 *
 * Served by the home server (« Corde serveur », `store/hub`), the same app opens on the pairing
 * screen until this device is paired, and the live dashboard gives way to a page that sends the
 * driver to the PC (the game, and so the live view, are there).
 *
 * In the iPhone application (`mobile/slots`, filled in before the first render) the pairing
 * happens in « Mes serveurs », the tabs are at the bottom, and the live dashboard needs an
 * active server (`useServerGate`): without one, the shell explains how to add it.
 *
 * On a phone (and in the phone application, and at `#/dash`), the live page is the race
 * dashboard (`dash/`, choice in `dash/view.ts`): full screen, without the bars of the app. On a
 * PC or a tablet it is chosen in the menu of the Direct page and shown inside the page, under
 * the top bar; full screen there only on request.
 */
export function App() {
  const lang = useSettings((s) => s.lang);
  const route = useRoute();
  const hubChecked = useHub((s) => s.checked);
  const onHub = useHub((s) => s.role === "hub");
  const paired = useHub((s) => s.paired);
  useEffect(() => {
    document.documentElement.lang = lang;
  }, [lang]);
  useEffect(() => {
    void useHub.getState().check();
  }, []);
  // Desktop application only (no-op in a browser): follow the update state of the shell, for
  // the settings badge and the home line.
  useEffect(() => {
    useUpdates.getState().start();
  }, []);
  // An overlay layout is edited on purpose (menu, OBS page): leaving the dashboard ends it, so
  // that the dashboard opened on track shows the screen layout again.
  const onDashboard = route.page === "dashboard";
  useEffect(() => {
    if (!onDashboard) useDashboard.getState().setView("screen");
  }, [onDashboard]);
  const Tabs = appSlots.BottomTabs;
  const gate = useAppServerGate(route);
  const mode = useLiveMode(route);
  // Until the server has said what it is, the usual app (a PC answers at once; a phone offline
  // keeps its offline state): the home server switches to the pairing screen when it answers.
  if (!Tabs && hubChecked && onHub && !paired) return <PairingScreen />;
  const dashboard = route.page === "dashboard" && !onHub && !gate;
  const dash = dashboard && mode === "dash-full";
  const studio = route.page === "studio" && !onHub;
  return (
    <>
      <ReminderBanner />
      {studio ? (
        <Suspense fallback={null}>
          <StudioPage id={route.page === "studio" ? route.id : undefined} />
        </Suspense>
      ) : dash ? (
        <Suspense fallback={null}>
          <DashView />
        </Suspense>
      ) : dashboard ? (
        <Dashboard dash={mode === "dash-page"} />
      ) : (
        <SessionsShell route={route} />
      )}
      {dashboard && !dash && Tabs && <Tabs section="dashboard" />}
    </>
  );
}
