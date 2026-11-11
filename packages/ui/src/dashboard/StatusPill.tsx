import { useT } from "../i18n";
import { fmtNum } from "../lib/format";
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
        {t(LABEL[phase])}
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
