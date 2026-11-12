import { useEffect } from "react";
import { useT } from "../i18n";
import { isDesktop } from "../lib/desktop";
import { noteLines, releaseDate, updateView } from "../lib/updates";
import { useUpdates } from "../store/updates";

/**
 * « Mises à jour » in the settings dialog. In the desktop application: installed version,
 * « Rechercher une mise à jour », the result (up to date / version X with its notes),
 * « Installer et redémarrer » with a progress bar, the automatic check switch and the releases
 * page. Elsewhere (browser, phone): one line saying updates happen on the PC.
 */
export function UpdatesSection({ open }: { open: boolean }) {
  const { t, lang } = useT();
  const desktop = isDesktop();
  const status = useUpdates((s) => s.status);
  const actionError = useUpdates((s) => s.actionError);
  const refresh = useUpdates((s) => s.refresh);
  const check = useUpdates((s) => s.check);
  const install = useUpdates((s) => s.install);
  const setAutoCheck = useUpdates((s) => s.setAutoCheck);
  const openReleases = useUpdates((s) => s.openReleases);

  useEffect(() => {
    if (open && desktop) {
      useUpdates.getState().start();
      void refresh();
    }
  }, [open, desktop, refresh]);

  const v = updateView(status, desktop);

  if (v.state === "browser") {
    return (
      <>
        <div className="sheet-section">{t("settings.section.updates")}</div>
        <div className="field">
          <span className="hint">{t("update.browser")}</span>
        </div>
      </>
    );
  }

  const lastCheck =
    v.lastCheckMs !== null
      ? new Date(v.lastCheckMs).toLocaleString(lang, {
          day: "numeric",
          month: "short",
          hour: "2-digit",
          minute: "2-digit",
        })
      : null;
  const date = v.available ? releaseDate(v.available.date, lang) : null;
  const notes = v.available ? noteLines(v.available.notes) : [];

  let result: string | null = null;
  switch (v.state) {
    case "loading":
      result = "…";
      break;
    case "never":
      result = t("update.never");
      break;
    case "checking":
      result = t("update.checking");
      break;
    case "upToDate":
      result = t("update.upToDate");
      break;
    case "checkFailed":
      result = null; // the notice says it
      break;
    case "available":
    case "downloading":
    case "installing":
      result = null; // the update block says it
      break;
  }

  return (
    <>
      <div className="sheet-section">{t("settings.section.updates")}</div>
      <div className="field is-switches update-section">
        <div className="sheet-app-row">
          <span className="hint">
            {t("update.current")} <b className="num">{v.current ?? "—"}</b>
          </span>
          <button type="button" className="btn" onClick={() => void check()} disabled={!v.canCheck}>
            {v.state === "checking" ? t("update.checking") : t("update.check")}
          </button>
        </div>

        {v.updatedFrom && (
          <span className="hint update-done">{t("update.justUpdated", { v: v.updatedFrom })}</span>
        )}
        {result && (
          <span className="hint" role="status">
            {result}
            {lastCheck && v.state !== "checking" && (
              <span className="dim"> · {t("update.lastCheck", { when: lastCheck })}</span>
            )}
          </span>
        )}

        {v.available && (
          <div className="update-available" role="status">
            <div className="update-title">
              <b>{t("update.available", { v: v.available.version })}</b>
              {date && <span className="dim"> · {t("update.publishedOn", { date })}</span>}
            </div>
            {notes.length > 0 && (
              <ul className="update-notes" aria-label={t("update.notes")}>
                {notes.map((line, i) => (
                  <li key={i}>{line}</li>
                ))}
              </ul>
            )}
            {v.state === "downloading" || v.state === "installing" ? (
              <div className="update-progress">
                <progress
                  max={100}
                  value={v.state === "installing" ? 100 : (v.percent ?? undefined)}
                  aria-label={t("update.progress")}
                />
                <span className="hint num">
                  {v.state === "installing"
                    ? t("update.installing")
                    : v.percent !== null
                      ? t("update.downloadingPct", { pct: v.percent })
                      : t("update.downloadingMb", { mb: v.downloadedMb ?? 0 })}
                </span>
              </div>
            ) : (
              <div className="sheet-app-row">
                <span className="hint">{t("update.installHint")}</span>
                <button
                  type="button"
                  className="btn is-active"
                  onClick={() => void install()}
                  disabled={!v.canInstall}
                >
                  {t("update.install")}
                </button>
              </div>
            )}
          </div>
        )}

        {v.notice && (
          <span className={`hint${v.noticeIsError ? " is-error" : ""}`} role="alert">
            {t(v.notice)}
          </span>
        )}
        {actionError && (
          <span className="hint is-error" role="alert">
            {t("settings.appError", { msg: actionError })}
          </span>
        )}

        <label className="switch is-wrap">
          <input
            className="switch-toggle"
            type="checkbox"
            checked={v.autoCheck}
            disabled={v.state === "loading"}
            onChange={(e) => void setAutoCheck(e.target.checked)}
          />
          {t("update.auto")}
        </label>
        <button
          type="button"
          className={`update-link${v.suggestReleasesPage ? " is-strong" : ""}`}
          onClick={() => void openReleases()}
        >
          {t("update.releases")}
        </button>
      </div>
    </>
  );
}
