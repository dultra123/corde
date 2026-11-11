import { IconRecord, IconStop } from "../dashboard/icons";
import { useT } from "../i18n";
import { fmtBytes } from "../lib/format";
import { routeHash } from "../nav/routes";
import { useRecording } from "../store/recording";
import { useTelemetry } from "../store/telemetry";

interface Props {
  /** Track name of the session being recorded, when the caller knows it (sessions list). */
  trackOf?: (sessionId: string) => string | undefined;
}

/** The "● Enregistrement en cours — circuit, tour N, X Mo" strip with start / stop buttons. */
export function RecordingBanner({ trackOf }: Props) {
  const { t, lang } = useT();
  const status = useRecording((s) => s.status);
  const error = useRecording((s) => s.error);
  const busy = useRecording((s) => s.busy);
  const { start, stop } = useRecording.getState();
  const liveTrack = useTelemetry((s) => s.frame?.session.track_name ?? null);

  if (!status) {
    return (
      <div className={`recbar is-${error ? "error" : "unknown"}`}>
        <span className="recbar-dot" />
        <span className="recbar-text">
          {error ? t("rec.unavailable", { msg: error }) : t("common.loading")}
        </span>
      </div>
    );
  }

  const track = status.session_id
    ? (trackOf?.(status.session_id) ?? liveTrack ?? status.session_id)
    : null;

  let state: "live" | "idle" | "armed" | "off";
  if (status.recording) state = status.idle_reason ? "idle" : "live";
  else state = status.armed ? "armed" : "off";

  const details: string[] = [];
  if (state === "live" || state === "idle") {
    if (track) details.push(track);
    details.push(t("rec.lap", { n: status.laps + 1 }));
    details.push(fmtBytes(status.bytes, lang));
    if (state === "idle" && status.idle_reason) details.push(status.idle_reason);
  }

  return (
    <div className={`recbar is-${state}`}>
      <span className="recbar-dot" aria-hidden="true" />
      <span className="recbar-text">
        <b>{t(`rec.${state}`)}</b>
        {details.length > 0 && <span className="recbar-detail"> — {details.join(" · ")}</span>}
        {state === "off" && (
          <span className="recbar-detail dim">
            {" "}
            {status.auto ? t("rec.offAuto") : t("rec.offManual")}
          </span>
        )}
      </span>
      <div className="recbar-actions">
        {status.recording && status.session_id && (
          <a
            className="btn recbar-open"
            href={routeHash({ page: "session", id: status.session_id })}
          >
            {t("sessions.open")}
          </a>
        )}
        {status.recording || status.armed ? (
          <button
            type="button"
            className="btn is-danger"
            onClick={() => void stop()}
            disabled={busy}
          >
            <IconStop />
            <span>{t("rec.stop")}</span>
          </button>
        ) : (
          <button type="button" className="btn is-rec" onClick={() => void start()} disabled={busy}>
            <IconRecord />
            <span>{t("rec.start")}</span>
          </button>
        )}
      </div>
    </div>
  );
}
