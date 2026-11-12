import { useEffect, useRef, useState } from "react";
import { useT } from "../i18n";
import { appVersion, getAutostart, isDesktop, restartApp, setAutostart } from "../lib/desktop";
import { fmtNum } from "../lib/format";
import { appSlots } from "../mobile/slots";
import { isAutoNavDisabled } from "../nav/auto";
import { routeHash } from "../nav/routes";
import { connection } from "../net/connection";
import { type Lang, useSettings } from "../store/settings";
import { useConnectionPhase, useTelemetry } from "../store/telemetry";
import { HubDeviceSection, HubLinkSection } from "./HubSection";
import { UpdatesSection } from "./UpdatesSection";

interface Props {
  open: boolean;
  onClose: () => void;
}

const RATES = [5, 10, 20, 30, 60];

function reasonOf(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

/**
 * « Application » — only inside the desktop shell (`lib/desktop`): start with Windows, restart,
 * version. These act at once (they are system actions, not preferences of this page); a command
 * that fails is reported under the control and changes nothing.
 */
function DesktopSection({ open }: { open: boolean }) {
  const { t } = useT();
  const [autostart, setAuto] = useState<boolean | null>(null);
  const [version, setVersion] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open || !isDesktop()) return;
    let cancelled = false;
    setError(null);
    getAutostart()
      .then((v) => {
        if (!cancelled) setAuto(v);
      })
      .catch((e: unknown) => {
        if (!cancelled) setError(reasonOf(e));
      });
    appVersion()
      .then((v) => {
        if (!cancelled) setVersion(v);
      })
      .catch(() => {
        // the version is a nicety: the autostart error above is the one worth showing
      });
    return () => {
      cancelled = true;
    };
  }, [open]);

  if (!isDesktop()) return null;

  const toggle = async (enabled: boolean) => {
    const before = autostart;
    setBusy(true);
    setError(null);
    setAuto(enabled);
    try {
      await setAutostart(enabled);
    } catch (e) {
      setAuto(before);
      setError(reasonOf(e));
    } finally {
      setBusy(false);
    }
  };

  const restart = async () => {
    setBusy(true);
    setError(null);
    try {
      await restartApp();
    } catch (e) {
      setError(reasonOf(e));
      setBusy(false);
    }
  };

  return (
    <>
      <div className="sheet-section">{t("settings.section.app")}</div>
      <div className="field is-switches">
        <label className="switch is-wrap">
          <input
            className="switch-toggle"
            type="checkbox"
            checked={autostart === true}
            disabled={busy || autostart === null}
            onChange={(e) => void toggle(e.target.checked)}
          />
          {t("settings.autostart")}
        </label>
        <div className="sheet-app-row">
          <span className="hint">
            {t("settings.appVersion")} <b className="num">{version ?? "—"}</b>
          </span>
          <button type="button" className="btn" onClick={() => void restart()} disabled={busy}>
            {t("settings.restart")}
          </button>
        </div>
        {error && (
          <span className="hint is-error" role="alert">
            {t("settings.appError", { msg: error })}
          </span>
        )}
      </div>
    </>
  );
}

export function SettingsDialog({ open, onClose }: Props) {
  const { t } = useT();
  const ref = useRef<HTMLDialogElement>(null);
  const settings = useSettings();
  const phase = useConnectionPhase();
  const hello = useTelemetry((s) => s.hello);
  const status = useTelemetry((s) => s.status);
  const rttMs = useTelemetry((s) => s.rttMs);
  const demo = useTelemetry((s) => s.demo);
  const [server, setServer] = useState(settings.server);
  const [rate, setRate] = useState(settings.rateHz);
  const [lang, setLang] = useState<Lang>(settings.lang);
  const [demoOn, setDemoOn] = useState(demo === "manual");
  const [autoDashboard, setAutoDashboard] = useState(settings.autoDashboard);
  const [autoSummary, setAutoSummary] = useState(settings.autoSummary);
  const pinned = isAutoNavDisabled();

  useEffect(() => {
    const d = ref.current;
    if (!d) return;
    if (open && !d.open) {
      setServer(settings.server);
      setRate(settings.rateHz);
      setLang(settings.lang);
      setDemoOn(demo === "manual");
      setAutoDashboard(settings.autoDashboard);
      setAutoSummary(settings.autoSummary);
      d.showModal();
    } else if (!open && d.open) d.close();
  }, [
    open,
    settings.server,
    settings.rateHz,
    settings.lang,
    settings.autoDashboard,
    settings.autoSummary,
    demo,
  ]);

  const save = () => {
    settings.update({ server: server.trim(), rateHz: rate, lang, autoDashboard, autoSummary });
    if (demoOn !== (demo === "manual")) connection.setDemo(demoOn);
    onClose();
  };

  const phaseLabel: Record<typeof phase, string> = {
    connecting: t("status.connecting"),
    offline: t("status.offline"),
    "waiting-game": t("status.waitingGame"),
    monitor: t("status.monitor"),
    live: t("status.live"),
    demo: t("status.demo"),
    "demo-auto": t("status.demoAuto"),
  };

  return (
    <dialog className="sheet" ref={ref} onClose={onClose} aria-label={t("settings.title")}>
      <form
        method="dialog"
        onSubmit={(e) => {
          e.preventDefault();
          save();
        }}
      >
        <div className="sheet-head">{t("settings.title")}</div>
        <div className="sheet-body">
          {appSlots.ServersPage ? (
            // iPhone application: the servers are chosen in « Mes serveurs ».
            <div className="field">
              <span className="hint">
                <a className="link" href={routeHash({ page: "servers" })} onClick={onClose}>
                  {t("mobile.home.server.manage")}
                </a>
              </span>
            </div>
          ) : (
            <div className="field">
              <label htmlFor="set-server">{t("settings.server")}</label>
              <input
                id="set-server"
                className="input num"
                value={server}
                onChange={(e) => setServer(e.target.value)}
                placeholder={t("settings.serverDefault")}
                inputMode="url"
                autoCapitalize="off"
                autoCorrect="off"
                spellCheck={false}
              />
              <span className="hint">{t("settings.serverHint")}</span>
            </div>
          )}
          <div className="field">
            <label htmlFor="set-rate">{t("settings.rate")}</label>
            <select
              id="set-rate"
              className="select"
              value={rate}
              onChange={(e) => setRate(Number(e.target.value))}
            >
              {RATES.map((r) => (
                <option key={r} value={r}>
                  {r} Hz
                </option>
              ))}
            </select>
          </div>
          <div className="field">
            <label htmlFor="set-lang">{t("settings.language")}</label>
            <select
              id="set-lang"
              className="select"
              value={lang}
              onChange={(e) => setLang(e.target.value as Lang)}
            >
              <option value="fr">Français</option>
              <option value="en">English</option>
            </select>
          </div>
          <div className="field">
            <label className="switch">
              <input
                type="checkbox"
                checked={demoOn}
                onChange={(e) => setDemoOn(e.target.checked)}
              />
              {t("settings.demo")}
            </label>
            <span className="hint">{t("settings.demoHint")}</span>
          </div>
          <div className="sheet-section">{t("settings.section.autoNav")}</div>
          <div className="field is-switches">
            <label className="switch is-wrap">
              <input
                className="switch-toggle"
                type="checkbox"
                checked={autoDashboard && !pinned}
                disabled={pinned}
                onChange={(e) => setAutoDashboard(e.target.checked)}
              />
              {t("settings.autoDashboard")}
            </label>
            <label className="switch is-wrap">
              <input
                className="switch-toggle"
                type="checkbox"
                checked={autoSummary && !pinned}
                disabled={pinned}
                onChange={(e) => setAutoSummary(e.target.checked)}
              />
              {t("settings.autoSummary")}
            </label>
            <span className="hint">
              {pinned ? t("settings.autoNavPinned") : t("settings.autoNavHint")}
            </span>
          </div>
          <DesktopSection open={open} />
          <UpdatesSection open={open} />
          <HubLinkSection open={open} />
          <HubDeviceSection />
          <div className="sheet-info">
            <span>{t("settings.connectionState")}</span>
            <b>{phaseLabel[phase]}</b>
            <span>{t("settings.address")}</span>
            <b className="num">{connection.endpoint.ws}</b>
            <span>{t("settings.serverVersion")}</span>
            <b>
              {hello ? `${hello.serverVersion} (v${hello.protocolVersion})` : "—"}
              {status?.game_version ? ` · ${status.game} ${status.game_version}` : ""}
            </b>
            <span>{t("settings.ping")}</span>
            <b className="num">{rttMs === null ? "—" : `${fmtNum(rttMs, 0)} ms`}</b>
          </div>
        </div>
        <div className="sheet-foot">
          <button type="button" className="btn" onClick={onClose}>
            {t("settings.cancel")}
          </button>
          <button type="submit" className="btn is-active">
            {t("settings.save")}
          </button>
        </div>
      </form>
    </dialog>
  );
}
