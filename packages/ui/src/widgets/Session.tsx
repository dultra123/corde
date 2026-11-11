import type { GamePhase, SessionType, YellowFlagState } from "@corde/protocol";
import { useT } from "../i18n";
import { fmtClock, fmtNum, fmtTimeOfDay } from "../lib/format";
import { useSession } from "../store/telemetry";
import { registerWidget, type WidgetProps } from "./registry";
import { Empty } from "./shared/Empty";

const sessionKey = (s: SessionType) => `session.type.${s}` as const;
const phaseKey = (p: GamePhase) => `phase.${p}` as const;
const yellowKey = (y: YellowFlagState) => `yellow.${y}` as const;

function phaseTone(p: GamePhase, y: YellowFlagState): "green" | "yellow" | "red" | "none" {
  if (p === "full_course_yellow" || (y !== "none" && y !== "invalid")) return "yellow";
  if (p === "session_stopped" || p === "paused") return "red";
  if (p === "green") return "green";
  return "none";
}

function Stat({ label, value, unit }: { label: string; value: string; unit?: string }) {
  return (
    <div className="stat">
      <span className="stat-label dim">{label}</span>
      <span className="stat-value num">
        {value}
        {unit && <small>{unit}</small>}
      </span>
    </div>
  );
}

/** Track rubbering 0..4 as a row of dots. */
function Grip({ label, level }: { label: string; level: number | null }) {
  return (
    <div className="stat">
      <span className="stat-label dim">{label}</span>
      <span className="stat-value grip" role="img" aria-label={level === null ? "—" : `${level}/4`}>
        {level === null
          ? "—"
          : [0, 1, 2, 3].map((i) => <i key={i} className={i < Math.round(level) ? "is-on" : ""} />)}
      </span>
    </div>
  );
}

function SessionWidget(_props: WidgetProps) {
  const { t } = useT();
  const session = useSession();
  if (!session) return <Empty textKey="widget.noData" />;
  const w = session.weather;
  const tone = phaseTone(session.phase, session.yellow_flag);
  const yellowSectors = session.sector_flags
    .map((f, i) => (f > 0 ? i + 1 : 0))
    .filter((i) => i > 0);
  const timed = session.end_s > 0;
  const grip = w.track_grip_level;
  return (
    <div className="session">
      <div className="session-head">
        <div className="session-track">{session.track_name || "—"}</div>
        <div className="session-type dim">
          {t(sessionKey(session.session_type))}
          {session.is_multiplayer && session.server_name ? ` · ${session.server_name}` : ""}
        </div>
      </div>
      <div className="session-flags">
        <span className={`flag is-${tone}`}>{t(phaseKey(session.phase))}</span>
        {session.yellow_flag !== "none" && session.yellow_flag !== "invalid" && (
          <span className="flag is-yellow">{t(yellowKey(session.yellow_flag))}</span>
        )}
        {yellowSectors.map((s) => (
          <span key={s} className="flag is-yellow">
            S{s}
          </span>
        ))}
      </div>
      <div className="session-stats">
        <Stat
          label={timed ? t("session.remaining") : t("session.elapsed")}
          value={timed ? fmtClock(session.time_remaining_s) : fmtClock(session.elapsed_s)}
        />
        <Stat label={t("session.clock")} value={fmtTimeOfDay(w.time_of_day_s)} />
        <Stat label={t("session.air")} value={fmtNum(w.ambient_c, 0)} unit="°C" />
        <Stat label={t("session.trackTemp")} value={fmtNum(w.track_c, 0)} unit="°C" />
        <Stat label={t("session.rain")} value={fmtNum(w.rain * 100, 0)} unit="%" />
        <Stat label={t("session.wet")} value={fmtNum(w.wetness_avg * 100, 0)} unit="%" />
        <Grip label={t("session.grip")} level={grip} />
        <Stat
          label={t("session.cars")}
          value={String(session.num_vehicles)}
          unit={
            session.max_laps > 0 && session.max_laps < 10000
              ? ` · ${session.max_laps} ${t("standings.laps")}`
              : undefined
          }
        />
      </div>
    </div>
  );
}

export const sessionWidget = registerWidget({
  id: "session",
  titleKey: "widget.session",
  minW: 2,
  minH: 3,
  defaultSize: { w: 4, h: 4 },
  mobileSize: { w: 4, h: 3 },
  component: SessionWidget,
});
