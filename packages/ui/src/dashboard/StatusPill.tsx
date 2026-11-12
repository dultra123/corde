import { useT } from "../i18n";
import { fmtNum } from "../lib/format";
import { useApp } from "../mobile/app-state";
import { APP_BUILD } from "../mobile/env";
import { useHub } from "../store/hub";
import { type ConnectionPhase, useConnectionPhase, useTelemetry } from "../store/telemetry";

const LABEL: Record<
  ConnectionPhase,
  | "status.connecting"
  | "status.offline"
  | "status.waitingGame"
  | "status.monitor"
  | "status.live"
  | "status.demo"
  | "status.demoAuto"
> = {
  connecting: "status.connecting",
  offline: "status.offline",
  "waiting-game": "status.waitingGame",
  monitor: "status.monitor",
  live: "status.live",
  demo: "status.demo",
  "demo-auto": "status.demoAuto",
};

/** Connection / game / demo indicator with the measured frame rate. Click opens the settings. */
export function StatusPill({ onClick }: { onClick: () => void }) {
  const { t } = useT();
  const phase = useConnectionPhase();
  const rxHz = useTelemetry((s) => s.rxHz);
  const attempt = useTelemetry((s) => s.reconnectAttempt);
  // The home server never has a game: « connected » is all there is to say.
  const onHub = useHub((s) => s.role === "hub");
  const label = onHub && phase === "waiting-game" ? "status.hub" : LABEL[phase];
  // In the phone app a server is optional (Races works without one): with none chosen there is
  // nothing unreachable to report, so no red light.
  const noServer = useApp((a) => a.status === "none");
  if (APP_BUILD && noServer) {
    return (
      <button
        type="button"
        className="status is-idle"
        onClick={onClick}
        title={t("topbar.settings")}
      >
        <span className="status-dot" />
        <span className="status-text">{t("status.noServer")}</span>
      </button>
    );
  }
  const showHz =
    phase === "live" || phase === "demo" || phase === "demo-auto" || phase === "monitor";
  return (
    <button
      type="button"
      className={`status is-${phase}`}
      onClick={onClick}
      title={t("topbar.settings")}
    >
      <span className="status-dot" />
      <span className="status-text">
        {t(label)}
        {phase === "offline" && attempt > 0 ? ` (${attempt})` : ""}
      </span>
      {showHz && (
        <span className="status-hz num">
          {fmtNum(rxHz, 0)} {t("topbar.hz")}
        </span>
      )}
    </button>
  );
}
