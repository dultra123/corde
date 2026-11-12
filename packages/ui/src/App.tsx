import { useEffect } from "react";
import { Dashboard } from "./dashboard/Dashboard";
import { PairingScreen } from "./hub/PairingScreen";
import { useRoute } from "./nav/routes";
import { ReminderBanner } from "./races/ReminderBanner";
import { SessionsShell } from "./sessions/SessionsShell";
import { useDashboard } from "./store/dashboard";
import { useHub } from "./store/hub";
import { useSettings } from "./store/settings";
import { useUpdates } from "./store/updates";

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
  // Until the server has said what it is, the usual app (a PC answers at once; a phone offline
  // keeps its offline state): the home server switches to the pairing screen when it answers.
  if (hubChecked && onHub && !paired) return <PairingScreen />;
  return (
    <>
      <ReminderBanner />
      {route.page === "dashboard" && !onHub ? <Dashboard /> : <SessionsShell route={route} />}
    </>
  );
}
