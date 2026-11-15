import { useEffect, useState } from "react";
import { SettingsDialog } from "../dashboard/SettingsDialog";
import { TopBar } from "../dashboard/TopBar";
import { syncDashesWithServer, useDashes } from "../dashdef/store";
import { appSlots } from "../mobile/slots";
import { connection } from "../net/connection";
import { DashEditor } from "./Editor";
import { DashLibrary } from "./Library";

/**
 * The Studio (`#/dash-studio`, PC): the library of race dashboards, and the editor of one of
 * them (`#/dash-studio/<id>`). Made for a mouse and a keyboard; the dashboards are stored on the
 * PC and every device picks the one it shows.
 *
 * A section of the app like the others: the top bar of the app with its « Studio » tab, and
 * under it the bar of the library or of the editor (two rows: the editor's bar has too many
 * controls to share one with the tabs).
 */
export function StudioPage({ id }: { id?: string }) {
  const sync = useDashes((s) => s.sync);
  const [asked, setAsked] = useState(false);
  useEffect(() => {
    if (asked || sync === "ready" || sync === "loading") return;
    setAsked(true);
    void syncDashesWithServer(connection.endpoint.http);
  }, [asked, sync]);
  useEffect(() => {
    document.documentElement.classList.add("is-studio");
    return () => document.documentElement.classList.remove("is-studio");
  }, []);
  const [settingsOpen, setSettingsOpen] = useState(false);
  // Installed before the first render, never changes (`mobile/slots`).
  const Tabs = appSlots.BottomTabs;
  return (
    <div className="app is-desktop is-studio">
      <TopBar section="studio" onSettings={() => setSettingsOpen(true)} />
      {id ? <DashEditor id={id} /> : <DashLibrary />}
      {Tabs && <Tabs section="studio" />}
      <SettingsDialog open={settingsOpen} onClose={() => setSettingsOpen(false)} />
    </div>
  );
}
