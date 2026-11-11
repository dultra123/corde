import { useT } from "../i18n";
import { useDashboard } from "../store/dashboard";
import { listWidgets } from "../widgets";
import { IconPlus } from "./icons";

/** Bottom strip shown in edit mode: one chip per widget kind, tap to append it to the layout. */
export function Palette() {
  const { t } = useT();
  const addWidget = useDashboard((s) => s.addWidget);
  const setPaletteOpen = useDashboard((s) => s.setPaletteOpen);
  const device = useDashboard((s) => s.device);
  return (
    <div className="palette" role="dialog" aria-label={t("palette.title")}>
      <div className="palette-head">
        <span>{t("palette.title")}</span>
        <span className="dim palette-hint">{t("layout.editHint")}</span>
        <button
          type="button"
          className="btn is-icon"
          onClick={() => setPaletteOpen(false)}
          aria-label={t("palette.close")}
        >
          ×
        </button>
      </div>
      <div className="palette-list">
        {listWidgets().map((w) => {
          const size = device === "mobile" ? (w.mobileSize ?? w.defaultSize) : w.defaultSize;
          return (
            <button key={w.id} type="button" className="chip" onClick={() => addWidget(w.id)}>
              <IconPlus />
              <span>{t(w.titleKey)}</span>
              <small className="num dim">
                {size.w}×{size.h}
              </small>
            </button>
          );
        })}
      </div>
    </div>
  );
}
