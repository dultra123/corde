import type { SessionType } from "@corde/protocol";
import { useEffect, useMemo, useRef, useState } from "react";
import { IconEngineer, IconTrash } from "../dashboard/icons";
import { useT } from "../i18n";
import { fmtBytes, fmtDateTime, fmtDuration, fmtLapTime } from "../lib/format";
import { navigate, routeHash } from "../nav/routes";
import {
  deleteSession,
  fetchSessions,
  fetchStorage,
  type SessionRow,
  type StorageStats,
} from "../net/sessions-api";
import { useRecording } from "../store/recording";
import { sessionDurationS } from "./lib/stats";
import { RecordingBanner } from "./RecordingBanner";
import {
  errorMessage,
  Notice,
  PageHeader,
  SessionTypeTag,
  sessionTypeKey,
  useAsync,
  weatherSummary,
} from "./shared";

const TYPES: SessionType[] = ["race", "qualify", "practice", "warmup", "test_day"];
const LIST_LIMIT = 500;

function DeleteDialog({
  session,
  onClose,
  onDeleted,
}: {
  session: SessionRow | null;
  onClose: () => void;
  onDeleted: () => void;
}) {
  const { t, lang } = useT();
  const ref = useRef<HTMLDialogElement>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    const d = ref.current;
    if (!d) return;
    if (session && !d.open) {
      setError(null);
      d.showModal();
    } else if (!session && d.open) d.close();
  }, [session]);

  const confirm = async () => {
    if (!session) return;
    setBusy(true);
    try {
      await deleteSession(session.id);
      onDeleted();
      onClose();
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  const name = session
    ? `${session.track_name} · ${fmtDateTime(session.started_wall_ms, lang)}`
    : "";
  return (
    <dialog className="sheet" ref={ref} onClose={onClose} aria-label={t("sessions.delete.title")}>
      <div className="sheet-head">{t("sessions.delete.title")}</div>
      <div className="sheet-body">
        <p className="dim">{t("sessions.delete.hint", { name })}</p>
        {error && (
          <p className="form-error" role="alert">
            {error.includes("409") ? t("sessions.delete.busy") : error}
          </p>
        )}
      </div>
      <div className="sheet-foot">
        <button type="button" className="btn" onClick={onClose} disabled={busy}>
          {t("common.cancel")}
        </button>
        <button
          type="button"
          className="btn is-danger-solid"
          onClick={() => void confirm()}
          disabled={busy}
        >
          <IconTrash />
          <span>{t("sessions.delete.action")}</span>
        </button>
      </div>
    </dialog>
  );
}

export function SessionsPage() {
  const { t, lang } = useT();
  const revision = useRecording((s) => s.revision);
  const recLaps = useRecording((s) => s.status?.laps ?? -1);
  const recId = useRecording((s) => s.status?.session_id ?? null);
  const recBytes = useRecording((s) => s.status?.bytes ?? 0);
  const [track, setTrack] = useState("");
  const [type, setType] = useState<SessionType | "">("");
  const [victim, setVictim] = useState<SessionRow | null>(null);

  const sessions = useAsync(
    (signal) => fetchSessions({ limit: LIST_LIMIT }, signal),
    [revision, recLaps],
  );
  const storage = useAsync<StorageStats>((signal) => fetchStorage(signal), [revision, recLaps]);

  const all = sessions.data ?? [];
  const tracks = useMemo(
    () => [...new Set(all.map((s) => s.track_name))].sort((a, b) => a.localeCompare(b)),
    [all],
  );
  const rows = useMemo(
    () =>
      all.filter((s) => (!track || s.track_name === track) && (!type || s.session_type === type)),
    [all, track, type],
  );

  // Size per session is not in the list API: estimate from the global sample / byte ratio.
  const bytesPerSample = useMemo(() => {
    const samples = all.reduce((a, s) => a + s.sample_count, 0);
    const bytes = storage.data?.samples_bytes ?? 0;
    return samples > 0 && bytes > 0 ? bytes / samples : null;
  }, [all, storage.data]);

  const trackOf = (id: string) => all.find((s) => s.id === id)?.track_name;

  const list = (() => {
    if (sessions.error && all.length === 0) {
      return (
        <Notice
          kind="error"
          title={t("common.serverError", { msg: sessions.error })}
          action={
            <button type="button" className="btn" onClick={sessions.reload}>
              {t("common.retry")}
            </button>
          }
        />
      );
    }
    if (sessions.loading && all.length === 0) {
      return <Notice kind="loading">{t("common.loading")}</Notice>;
    }
    if (all.length === 0) {
      return (
        <Notice kind="empty" title={t("sessions.empty.title")}>
          {t("sessions.empty.hint")}
        </Notice>
      );
    }
    if (rows.length === 0) {
      return <Notice kind="empty">{t("sessions.emptyFiltered")}</Notice>;
    }
    return (
      <div className="tbl-wrap">
        <table className="tbl sessions-tbl">
          <thead>
            <tr>
              <th>{t("sessions.col.date")}</th>
              <th>{t("sessions.col.track")}</th>
              <th>{t("sessions.col.type")}</th>
              <th>{t("sessions.col.car")}</th>
              <th className="is-num">{t("sessions.col.laps")}</th>
              <th className="is-num">{t("sessions.col.best")}</th>
              <th className="is-optional">{t("sessions.col.weather")}</th>
              <th className="is-num is-optional">{t("sessions.col.duration")}</th>
              <th className="is-num is-optional" title={t("sessions.sizeApprox")}>
                {t("sessions.col.size")}
              </th>
              <th aria-label={t("common.delete")} />
            </tr>
          </thead>
          <tbody>
            {rows.map((s) => {
              const live = s.id === recId && s.ended_wall_ms === null;
              const href = routeHash({ page: "session", id: s.id });
              const size = live
                ? recBytes
                : bytesPerSample !== null
                  ? s.sample_count * bytesPerSample
                  : null;
              return (
                <tr
                  key={s.id}
                  className={`is-link${live ? " is-live" : ""}`}
                  onClick={(e) => {
                    if ((e.target as HTMLElement).closest("button, a")) return;
                    navigate({ page: "session", id: s.id });
                  }}
                >
                  <td className="num">
                    <a href={href} className="cell-link">
                      {fmtDateTime(s.started_wall_ms, lang)}
                    </a>
                    {live && <span className="livetag">{t("sessions.inProgress")}</span>}
                  </td>
                  <td className="is-strong">{s.track_name}</td>
                  <td>
                    <SessionTypeTag type={s.session_type} />
                  </td>
                  <td>
                    <span className="is-strong">{s.vehicle_model || s.vehicle_name || "—"}</span>
                    {s.class_name && <span className="dim"> · {s.class_name}</span>}
                  </td>
                  <td className="num is-num">{s.lap_count}</td>
                  <td className="num is-num is-best">{fmtLapTime(s.best_lap_s)}</td>
                  <td className="dim is-optional">{weatherSummary(s.weather, t)}</td>
                  <td className="num is-num dim is-optional">
                    {fmtDuration(sessionDurationS(s.started_wall_ms, s.ended_wall_ms) || null)}
                  </td>
                  <td className="num is-num dim is-optional">
                    {size === null ? "—" : `${live ? "" : "≈ "}${fmtBytes(size, lang)}`}
                  </td>
                  <td className="is-actions">
                    <a
                      className="btn is-icon is-engineer"
                      href={routeHash({ page: "engineer", id: s.id })}
                      aria-label={t("eng.open")}
                      title={t("eng.openHint")}
                    >
                      <IconEngineer />
                    </a>
                    <button
                      type="button"
                      className="btn is-icon is-danger"
                      aria-label={t("common.delete")}
                      title={live ? t("sessions.delete.busy") : t("common.delete")}
                      disabled={live}
                      onClick={() => setVictim(s)}
                    >
                      <IconTrash />
                    </button>
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
    );
  })();

  return (
    <div className="sessions-page">
      <RecordingBanner trackOf={trackOf} />
      <PageHeader
        title={t("sessions.title")}
        subtitle={
          all.length > 0
            ? all.length === 1
              ? t("sessions.countOne")
              : t("sessions.count", { n: all.length })
            : undefined
        }
        actions={
          all.length > 0 && (
            <div className="filters">
              <label className="filter">
                <span className="dim">{t("sessions.filter.track")}</span>
                <select className="select" value={track} onChange={(e) => setTrack(e.target.value)}>
                  <option value="">{t("sessions.filter.allTracks")}</option>
                  {tracks.map((name) => (
                    <option key={name} value={name}>
                      {name}
                    </option>
                  ))}
                </select>
              </label>
              <label className="filter">
                <span className="dim">{t("sessions.filter.type")}</span>
                <select
                  className="select"
                  value={type}
                  onChange={(e) => setType(e.target.value as SessionType | "")}
                >
                  <option value="">{t("sessions.filter.allTypes")}</option>
                  {TYPES.map((ty) => (
                    <option key={ty} value={ty}>
                      {t(sessionTypeKey(ty))}
                    </option>
                  ))}
                </select>
              </label>
            </div>
          )
        }
      />
      {list}
      <footer className="page-foot dim">
        {storage.data ? (
          <>
            <span>{t("sessions.storage")}</span>
            <span className="num">
              {t("sessions.storageDetail", {
                sessions: storage.data.sessions,
                files: storage.data.files,
                size: fmtBytes(storage.data.total_bytes, lang),
              })}
            </span>
            <span className="faint page-foot-path">{storage.data.path}</span>
          </>
        ) : storage.error ? (
          <span>{t("common.serverError", { msg: storage.error })}</span>
        ) : null}
      </footer>
      <DeleteDialog
        session={victim}
        onClose={() => setVictim(null)}
        onDeleted={() => {
          sessions.reload();
          storage.reload();
        }}
      />
    </div>
  );
}
