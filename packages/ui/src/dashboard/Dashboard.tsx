import { useEffect, useState } from "react";
import { useT } from "../i18n";
import { routeHash } from "../nav/routes";
import { OVERLAY_COLUMNS, OVERLAY_ROWS } from "../overlay/model";
import { useCurrentLayout, useDashboard } from "../store/dashboard";
import { MOBILE_MAX_WIDTH } from "./defaults";
import { GridCanvas } from "./GridCanvas";
import { Palette } from "./Palette";
import { SettingsDialog } from "./SettingsDialog";
import { TopBar } from "./TopBar";

const DESKTOP_ROWS = 16;
const MOBILE_CELL = 72;
const TOPBAR = 38;
/** Room kept under an overlay canvas for its caption. */
const OVERLAY_CAPTION = 30;

/** Desktop: 16 rows fill the viewport; mobile: fixed rows, the page scrolls. */
function computeCellHeight(device: "desktop" | "mobile"): number {
  if (device === "mobile") return MOBILE_CELL;
  const avail = window.innerHeight - TOPBAR - 6;
  return Math.max(44, Math.floor(avail / DESKTOP_ROWS));
}

/**
 * Overlay layouts: square cells, the whole 1920 × 1080 canvas (48 × 27 cells) visible at once —
 * what OBS will show, scaled down to the window.
 */
function computeOverlayCell(editing: boolean): number {
  const width = window.innerWidth - 6;
  const height = window.innerHeight - TOPBAR - 6 - OVERLAY_CAPTION - (editing ? 120 : 0);
  return Math.max(8, Math.min(width / OVERLAY_COLUMNS, height / OVERLAY_ROWS));
}

function useCellHeight(device: "desktop" | "mobile", overlay: boolean, editing: boolean): number {
  const compute = () => (overlay ? computeOverlayCell(editing) : computeCellHeight(device));
  const [h, setH] = useState(compute);
  // biome-ignore lint/correctness/useExhaustiveDependencies: `compute` reads exactly these
  useEffect(() => {
    const on = () => setH(compute());
    on();
    window.addEventListener("resize", on);
    return () => window.removeEventListener("resize", on);
  }, [device, overlay, editing]);
  return h;
}

export function Dashboard() {
  const { t } = useT();
  const device = useDashboard((s) => s.device);
  const setDevice = useDashboard((s) => s.setDevice);
  const editing = useDashboard((s) => s.editing);
  const paletteOpen = useDashboard((s) => s.paletteOpen);
  const layout = useCurrentLayout();
  const overlay = layout?.device === "overlay";
  const cellHeight = useCellHeight(device, overlay, editing);
  const [settingsOpen, setSettingsOpen] = useState(false);

  useEffect(() => {
    const mq = window.matchMedia(`(max-width: ${MOBILE_MAX_WIDTH - 1}px)`);
    const apply = () => setDevice(mq.matches ? "mobile" : "desktop");
    apply();
    mq.addEventListener("change", apply);
    return () => mq.removeEventListener("change", apply);
  }, [setDevice]);

  const grid = layout && (
    <GridCanvas
      key={layout.id}
      layout={layout}
      editing={editing}
      cellHeight={cellHeight}
      maxRow={overlay ? OVERLAY_ROWS : undefined}
    />
  );

  return (
    <div className={`app is-${device}${editing ? " is-editing" : ""}`}>
      <TopBar onSettings={() => setSettingsOpen(true)} />
      <main className={`board${overlay ? " is-overlay" : ""}`}>
        {layout && overlay ? (
          <div className="overlay-stage">
            <div
              className="overlay-frame checkerboard"
              style={{
                width: cellHeight * OVERLAY_COLUMNS,
                height: cellHeight * OVERLAY_ROWS,
              }}
            >
              {grid}
            </div>
            <p className="overlay-caption dim">
              {t("overlay.editorCaption")}{" "}
              <a className="link" href={routeHash({ page: "obs" })}>
                {t("overlay.editorLink")}
              </a>
            </p>
          </div>
        ) : (
          grid
        )}
        {(!layout || layout.widgets.length === 0) && (
          <div className="board-empty dim">{t("layout.empty")}</div>
        )}
      </main>
      {editing && paletteOpen && <Palette />}
      <SettingsDialog open={settingsOpen} onClose={() => setSettingsOpen(false)} />
    </div>
  );
}
