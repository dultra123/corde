import { useEffect } from "react";
import { Dashboard } from "./dashboard/Dashboard";
import { useRoute } from "./nav/routes";
import { ReminderBanner } from "./races/ReminderBanner";
import { SessionsShell } from "./sessions/SessionsShell";
import { useSettings } from "./store/settings";

/**
 * Five sections behind hash routes: the home page (`#/home`, shown when the app opens), the live
 * dashboard (`#/`), the recorded sessions (`#/sessions…`), the online races (`#/races`) and the
 * driver profile built from the game's results files (`#/profile`, `#/results/<id>`). The race
 * reminders (`ReminderBanner`) show over every page. The WebSocket connection lives outside
 * React (`net/connection`), so leaving the dashboard never interrupts the stream: the store keeps
 * filling up and the widgets pick it up again when the tab comes back. `nav/auto` moves between
 * the sections on its own when the driver takes the track or comes back to the garage.
 */
export function App() {
  const lang = useSettings((s) => s.lang);
  const route = useRoute();
  useEffect(() => {
    document.documentElement.lang = lang;
  }, [lang]);
  return (
    <>
      <ReminderBanner />
      {route.page === "dashboard" ? <Dashboard /> : <SessionsShell route={route} />}
    </>
  );
}
