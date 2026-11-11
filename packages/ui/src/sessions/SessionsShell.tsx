import { useEffect, useState } from "react";
import { MOBILE_MAX_WIDTH } from "../dashboard/defaults";
import { SettingsDialog } from "../dashboard/SettingsDialog";
import { TopBar } from "../dashboard/TopBar";
import { EngineerPage } from "../engineer/EngineerPage";
import { HomePage } from "../home/HomePage";
import { type Route, sectionOf } from "../nav/routes";
import { ProfilePage } from "../profile/ProfilePage";
import { ResultPage } from "../profile/ResultPage";
import { RacesPage } from "../races/RacesPage";
import { useRecordingWatch } from "../store/recording";
import { LapPage } from "./LapPage";
import { SessionPage } from "./SessionPage";
import { SessionsPage } from "./SessionsPage";

function useIsMobile(): boolean {
  const [mobile, setMobile] = useState(
    () => typeof window !== "undefined" && window.innerWidth < MOBILE_MAX_WIDTH,
  );
  useEffect(() => {
    const mq = window.matchMedia(`(max-width: ${MOBILE_MAX_WIDTH - 1}px)`);
    const apply = () => setMobile(mq.matches);
    apply();
    mq.addEventListener("change", apply);
    return () => mq.removeEventListener("change", apply);
  }, []);
  return mobile;
}

/**
 * Shell of every section but the dashboard — `#/home`, `#/sessions…`, `#/races`, `#/profile`,
 * `#/results/…`: top bar, scrolling page, settings dialog.
 */
export function SessionsShell({ route }: { route: Exclude<Route, { page: "dashboard" }> }) {
  const [settingsOpen, setSettingsOpen] = useState(false);
  const mobile = useIsMobile();
  useRecordingWatch();
  const section = sectionOf(route);

  let page: React.ReactNode;
  switch (route.page) {
    case "home":
      page = <HomePage onSettings={() => setSettingsOpen(true)} />;
      break;
    case "sessions":
      page = <SessionsPage />;
      break;
    case "session":
      page = <SessionPage id={route.id} />;
      break;
    case "lap":
      page = <LapPage id={route.id} a={route.lap} b={null} />;
      break;
    case "compare":
      page = <LapPage id={route.id} a={route.a} b={route.b} />;
      break;
    case "engineer":
      page = <EngineerPage id={route.id} />;
      break;
    case "races":
      page = <RacesPage />;
      break;
    case "profile":
      page = <ProfilePage track={route.track} />;
      break;
    case "result":
      page = <ResultPage id={route.id} />;
      break;
  }

  return (
    <div className={`app is-${mobile ? "mobile" : "desktop"} is-sessions is-${section}`}>
      <TopBar section={section} onSettings={() => setSettingsOpen(true)} />
      <main className="page">{page}</main>
      <SettingsDialog open={settingsOpen} onClose={() => setSettingsOpen(false)} />
    </div>
  );
}
