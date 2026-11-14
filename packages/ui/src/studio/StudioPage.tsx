import { useEffect, useState } from "react";
import { syncDashesWithServer, useDashes } from "../dashdef/store";
import { connection } from "../net/connection";
import { DashEditor } from "./Editor";
import { DashLibrary } from "./Library";

/**
 * The Studio (`#/dash-studio`, PC): the library of race dashboards, and the editor of one of
 * them (`#/dash-studio/<id>`). Made for a mouse and a keyboard; the dashboards are stored on the
 * PC and every device picks the one it shows.
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
  return id ? <DashEditor id={id} /> : <DashLibrary />;
}
