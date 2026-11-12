import { useEffect, useState } from "react";
import { MOBILE_MAX_WIDTH } from "../dashboard/defaults";
import { SettingsDialog } from "../dashboard/SettingsDialog";
import { TopBar } from "../dashboard/TopBar";
import { EngineerPage } from "../engineer/EngineerPage";
import { HomePage } from "../home/HomePage";
import { DevicesPage, HubDirectPage, HubHomePage } from "../hub/HubPages";
import { appSlots, useAppServerGate } from "../mobile/slots";
import { type Route, sectionOf } from "../nav/routes";
import { ObsPage } from "../overlay/ObsPage";
import { ProfilePage } from "../profile/ProfilePage";
import { ResultPage } from "../profile/ResultPage";
import { RacesPage } from "../races/RacesPage";
import { useHub } from "../store/hub";
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
 * Shell of every section but the dashboard — `#/home`, `#/sessions…`, `#/races`, `#/obs`,
 * `#/profile`, `#/results/…`: top bar, scrolling page, settings dialog. On the home server it
 * also shows `#/` (the live view is on the PC) and `#/devices`. In the iPhone application
 * (`mobile/slots`), the tabs move to the bottom of the screen, `#/servers` lists the Corde
 * servers, and the pages that need a server explain how to get one while none answers.
 */
export function SessionsShell({ route }: { route: Route }) {
  const [settingsOpen, setSettingsOpen] = useState(false);
  const mobile = useIsMobile();
  const onHub = useHub((s) => s.role === "hub");
  useRecordingWatch();
  const section = sectionOf(route);
  // Installed before the first render, never changes (`mobile/slots`).
  const gate = useAppServerGate(route);
  const app = appSlots.BottomTabs !== undefined;
  const Tabs = appSlots.BottomTabs;
  const Servers = appSlots.ServersPage;

  let page: React.ReactNode;
  switch (route.page) {
    case "home":
      page =
        onHub && !app ? <HubHomePage /> : <HomePage onSettings={() => setSettingsOpen(true)} />;
      break;
    case "servers":
      page = Servers ? <Servers /> : <HomePage onSettings={() => setSettingsOpen(true)} />;
      break;
    case "dashboard":
      page = <HubDirectPage />;
      break;
    case "devices":
      page = onHub ? <DevicesPage /> : <HomePage onSettings={() => setSettingsOpen(true)} />;
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
    case "obs":
      page = onHub ? <HubDirectPage /> : <ObsPage />;
      break;
    case "profile":
      page = <ProfilePage track={route.track} />;
      break;
    case "result":
      page = <ResultPage id={route.id} />;
      break;
  }
  if (gate) page = gate;

  return (
    <div className={`app is-${mobile ? "mobile" : "desktop"} is-sessions is-${section}`}>
      <TopBar section={section} onSettings={() => setSettingsOpen(true)} />
      <main className="page">{page}</main>
      {Tabs && <Tabs section={section} />}
      <SettingsDialog open={settingsOpen} onClose={() => setSettingsOpen(false)} />
    </div>
  );
}
