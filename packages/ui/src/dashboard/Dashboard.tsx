import { useEffect, useState } from "react";
import { useT } from "../i18n";
import { useCurrentLayout, useDashboard } from "../store/dashboard";
import { MOBILE_MAX_WIDTH } from "./defaults";
import { GridCanvas } from "./GridCanvas";
import { Palette } from "./Palette";
import { SettingsDialog } from "./SettingsDialog";
import { TopBar } from "./TopBar";

const DESKTOP_ROWS = 16;
const MOBILE_CELL = 72;

/** Desktop: 16 rows fill the viewport; mobile: fixed rows, the page scrolls. */
function computeCellHeight(device: "desktop" | "mobile"): number {
  if (device === "mobile") return MOBILE_CELL;
  const topbar = 38;
  const avail = window.innerHeight - topbar - 6;
  return Math.max(44, Math.floor(avail / DESKTOP_ROWS));
}

function useCellHeight(device: "desktop" | "mobile"): number {
  const [h, setH] = useState(() => computeCellHeight(device));
  useEffect(() => {
    const on = () => setH(computeCellHeight(device));
    on();
    window.addEventListener("resize", on);
    return () => window.removeEventListener("resize", on);
  }, [device]);
  return h;
}

export function Dashboard() {
  const { t } = useT();
  const device = useDashboard((s) => s.device);
  const setDevice = useDashboard((s) => s.setDevice);
  const editing = useDashboard((s) => s.editing);
  const paletteOpen = useDashboard((s) => s.paletteOpen);
  const layout = useCurrentLayout();
  const cellHeight = useCellHeight(device);
  const [settingsOpen, setSettingsOpen] = useState(false);

  useEffect(() => {
    const mq = window.matchMedia(`(max-width: ${MOBILE_MAX_WIDTH - 1}px)`);
    const apply = () => setDevice(mq.matches ? "mobile" : "desktop");
    apply();
    mq.addEventListener("change", apply);
    return () => mq.removeEventListener("change", apply);
  }, [setDevice]);

  return (
    <div className={`app is-${device}${editing ? " is-editing" : ""}`}>
      <TopBar onSettings={() => setSettingsOpen(true)} />
      <main className="board">
        {layout ? (
          <>
            <GridCanvas key={layout.id} layout={layout} editing={editing} cellHeight={cellHeight} />
            {layout.widgets.length === 0 && (
              <div className="board-empty dim">{t("layout.empty")}</div>
            )}
          </>
        ) : (
          <div className="board-empty dim">{t("layout.empty")}</div>
        )}
      </main>
      {editing && paletteOpen && <Palette />}
      <SettingsDialog open={settingsOpen} onClose={() => setSettingsOpen(false)} />
    </div>
  );
}
