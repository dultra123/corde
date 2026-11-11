import { useMemo, useState } from "react";
import { IconRefresh } from "../dashboard/icons";
import { useT } from "../i18n";
import { COLORS } from "../lib/color";
import { fmtDate, fmtDateTime, fmtLapTime, fmtNum } from "../lib/format";
import { navigate, routeHash } from "../nav/routes";
import {
  fetchProfile,
  fetchResults,
  importResults,
  type Profile,
  type ResultRow,
  type TrackSummary,
} from "../net/profile-api";
import { errorMessage, Notice, PageHeader, SessionTypeTag, useAsync } from "../sessions/shared";
import { BestLapChart } from "./BestLapChart";
import { bestLapSeries, classLabel, fmtPos } from "./lib";

const TRACK_LIMIT = 1000;

function Kpi({
  label,
  value,
  unit,
  hint,
  color,
}: {
  label: string;
  value: string;
  unit?: string;
  hint?: string;
  color?: string;
}) {
  return (
    <div className="kpi">
      <div className="kpi-label dim">{label}</div>
      <div className="kpi-value num" style={{ color }}>
        {value}
        {unit && <span className="kpi-unit"> {unit}</span>}
      </div>
      {hint && <div className="kpi-hint faint">{hint}</div>}
    </div>
  );
}

/** One row of the results tables (recent / per track). */
function ResultLine({ r, showTrack }: { r: ResultRow; showTrack: boolean }) {
  const { t, lang } = useT();
  const href = routeHash({ page: "result", id: r.id });
  const race = r.session_kind === "race";
  return (
    <tr
      className="is-link"
      onClick={(e) => {
        if ((e.target as HTMLElement).closest("a")) return;
        navigate({ page: "result", id: r.id });
      }}
    >
      <td className="num">
        <a href={href} className="cell-link">
          {fmtDateTime(r.started_wall_ms, lang)}
        </a>
      </td>
      {showTrack && <td className="is-strong">{r.track_course}</td>}
      <td>
        <SessionTypeTag type={r.session_kind} />
      </td>
      <td>
        <span className="is-strong">{r.player_vehicle || "—"}</span>
        {r.player_class && <span className="dim"> · {classLabel(r.player_class)}</span>}
      </td>
      <td className="num is-num">
        <span className={race && (r.player_class_position ?? 99) <= 3 ? "is-podium" : undefined}>
          {fmtPos(r.player_class_position, r.num_class_drivers)}
        </span>
        <small className="faint">
          {" "}
          {t("profile.col.overallShort", { pos: fmtPos(r.player_position, r.num_drivers) })}
        </small>
      </td>
      <td className="num is-num is-optional">{r.player_laps}</td>
      <td className="num is-num is-best">{fmtLapTime(r.player_best_lap_s)}</td>
      <td className="is-optional dim">
        {r.player_finish === "dnf"
          ? t("profile.finish.dnf")
          : r.player_finish === "dq"
            ? t("profile.finish.dq")
            : race
              ? t("profile.finish.finished")
              : ""}
      </td>
    </tr>
  );
}

function ResultsTable({ rows, showTrack }: { rows: readonly ResultRow[]; showTrack: boolean }) {
  const { t } = useT();
  return (
    <div className="tbl-wrap">
      <table className="tbl results-tbl">
        <thead>
          <tr>
            <th>{t("sessions.col.date")}</th>
            {showTrack && <th>{t("sessions.col.track")}</th>}
            <th>{t("sessions.col.type")}</th>
            <th>{t("sessions.col.car")}</th>
            <th className="is-num">{t("profile.col.classPos")}</th>
            <th className="is-num is-optional">{t("sessions.col.laps")}</th>
            <th className="is-num">{t("sessions.col.best")}</th>
            <th className="is-optional">{t("profile.col.status")}</th>
          </tr>
        </thead>
        <tbody>
          {rows.map((r) => (
            <ResultLine key={r.id} r={r} showTrack={showTrack} />
          ))}
        </tbody>
      </table>
    </div>
  );
}

function ImportFooter({
  profile,
  onImported,
}: {
  profile: Profile | null;
  onImported: () => void;
}) {
  const { t, nOf, lang } = useT();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const imp = profile?.import ?? null;

  const run = async () => {
    setBusy(true);
    setError(null);
    try {
      await importResults();
      onImported();
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <footer className="page-foot import-foot dim">
      <div className="import-foot-main">
        <span className="is-strong">{t("profile.import.title")}</span>
        {imp?.results_dir ? (
          <span className="faint page-foot-path" title={imp.results_dir}>
            {imp.results_dir}
          </span>
        ) : (
          <span className="faint">{t("profile.import.noDir")}</span>
        )}
        {imp && (
          <span className="num">
            {imp.last_scan_ms
              ? t("profile.import.lastScan", {
                  when: fmtDateTime(imp.last_scan_ms, lang),
                  files: nOf(imp.files_seen, "word.file", "word.files"),
                  imported: nOf(imp.total_imported, "word.imported", "word.importeds"),
                })
              : t("profile.import.never")}
          </span>
        )}
        {imp?.player_name && (
          <span className="faint">{t("profile.import.player", { name: imp.player_name })}</span>
        )}
      </div>
      <div className="import-foot-actions">
        <button
          type="button"
          className="btn"
          onClick={() => void run()}
          disabled={busy || !imp || (!imp.enabled && !imp.results_dir)}
          title={t("profile.import.rescanHint")}
        >
          {busy ? <span className="spinner is-small" /> : <IconRefresh />}
          <span>{t("profile.import.rescan")}</span>
        </button>
      </div>
      {(error || imp?.error) && (
        <p className="form-error import-foot-error" role="alert">
          {error ?? imp?.error}
        </p>
      )}
      {imp && imp.skipped.length > 0 && (
        <details className="import-skipped">
          <summary>
            {t("profile.import.skipped", {
              files: nOf(imp.files_skipped, "word.file", "word.files"),
              word: t(imp.files_skipped >= 2 ? "word.skippeds" : "word.skipped"),
            })}
          </summary>
          <ul>
            {imp.skipped.map((s) => (
              <li key={s.file_name}>
                <span className="num">{s.file_name}</span> — {s.reason}
              </li>
            ))}
          </ul>
        </details>
      )}
    </footer>
  );
}

export function ProfilePage({ track }: { track?: string }) {
  const { t, nOf, lang } = useT();
  const profile = useAsync((signal) => fetchProfile(signal), []);
  const p = profile.data;
  const tracks = p?.tracks ?? [];

  // Track of the progression chart: the filtered one, else the last visited.
  const chartTrack = track ?? tracks[0]?.track_course ?? null;
  const trackResults = useAsync(
    (signal) =>
      chartTrack
        ? fetchResults({ track: chartTrack, limit: TRACK_LIMIT }, signal)
        : Promise.resolve([] as ResultRow[]),
    [chartTrack, p?.import.last_scan_ms ?? 0],
  );
  const onTrack = useMemo(
    () => (trackResults.data ?? []).filter((r) => r.track_course === chartTrack),
    [trackResults.data, chartTrack],
  );
  const points = useMemo(
    () => (chartTrack ? bestLapSeries(onTrack, chartTrack) : []),
    [onTrack, chartTrack],
  );
  const filtered = track ? (tracks.find((x) => x.track_course === track) ?? null) : null;

  const setTrack = (course: string | null) =>
    navigate(course ? { page: "profile", track: course } : { page: "profile" });

  if (profile.error && !p) {
    return (
      <div className="profile-page">
        <PageHeader title={t("profile.title")} />
        <Notice
          kind="error"
          title={t("common.serverError", { msg: profile.error })}
          action={
            <button type="button" className="btn" onClick={profile.reload}>
              {t("common.retry")}
            </button>
          }
        />
      </div>
    );
  }
  if (!p) {
    return (
      <div className="profile-page">
        <PageHeader title={t("profile.title")} />
        <Notice kind="loading">{t("common.loading")}</Notice>
      </div>
    );
  }

  const tot = p.totals;
  const empty = tot.sessions === 0;
  const imp = p.import;

  return (
    <div className="profile-page">
      <PageHeader
        title={p.player_name ?? t("profile.title")}
        subtitle={
          p.player_name ? (
            <>
              <span>{t("profile.subtitle")}</span>
              <span> · </span>
              <span>
                {tot.sessions === 1
                  ? t("profile.countOne")
                  : t("profile.count", { n: tot.sessions })}
              </span>
            </>
          ) : (
            t("profile.subtitle")
          )
        }
      />

      {empty ? (
        <Notice
          kind={imp.error ? "error" : "empty"}
          title={t("profile.empty.title")}
          action={
            imp.error ? (
              <code className="import-hint">
                corde-server --lmu-results-dir "C:\Program Files (x86)\Steam\steamapps\common\Le
                Mans Ultimate\UserData\Log\Results"
              </code>
            ) : undefined
          }
        >
          {imp.error
            ? imp.error
            : imp.files_seen === 0
              ? t("profile.empty.noFiles", { dir: imp.results_dir ?? "—" })
              : imp.files_skipped > 0
                ? t("profile.empty.skipped", {
                    files: nOf(imp.files_skipped, "word.file", "word.files"),
                  })
                : t("profile.empty.hint")}
        </Notice>
      ) : (
        <>
          <section className="kpis">
            <Kpi
              label={t("profile.kpi.sessions")}
              value={String(tot.sessions)}
              hint={t("profile.kpi.sessionsHint", {
                races: nOf(tot.races, "word.race", "word.races"),
                quals: nOf(tot.qualifyings, "word.qual", "word.quals"),
                practices: nOf(tot.practices, "word.practice", "word.practices"),
              })}
            />
            <Kpi
              label={t("profile.kpi.laps")}
              value={String(tot.laps)}
              hint={t("profile.kpi.lapsHint", {
                tracks: nOf(tracks.length, "word.track", "word.tracks"),
              })}
            />
            <Kpi label={t("profile.kpi.km")} value={fmtNum(tot.km, 0)} unit="km" />
            <Kpi
              label={t("profile.kpi.classWins")}
              value={String(tot.class_wins)}
              hint={t("profile.kpi.podiumsHint", {
                podiums: nOf(tot.class_podiums, "word.podium", "word.podiums"),
              })}
              color={tot.class_wins > 0 ? COLORS.accent : undefined}
            />
            <Kpi
              label={t("profile.kpi.wins")}
              value={String(tot.wins)}
              hint={t("profile.kpi.podiumsHint", {
                podiums: nOf(tot.podiums, "word.podium", "word.podiums"),
              })}
              color={tot.wins > 0 ? COLORS.accent : undefined}
            />
            <Kpi
              label={t("profile.kpi.dnfs")}
              value={String(tot.dnfs)}
              hint={t("profile.kpi.dnfsHint", { races: nOf(tot.races, "word.race", "word.races") })}
              color={tot.dnfs > 0 ? COLORS.slower : undefined}
            />
          </section>

          <section className="card">
            <div className="card-head">
              <h2>{t("profile.tracks.title")}</h2>
              <span className="dim selhint">{t("profile.tracks.hint")}</span>
            </div>
            <div className="tbl-wrap">
              <table className="tbl tracks-tbl">
                <thead>
                  <tr>
                    <th>{t("sessions.col.track")}</th>
                    <th className="is-num">{t("sessions.col.best")}</th>
                    <th className="is-optional">{t("profile.col.bestWith")}</th>
                    <th className="is-num">{t("profile.col.sessions")}</th>
                    <th className="is-num">{t("profile.col.bestClassPos")}</th>
                    <th className="is-num is-optional">{t("profile.col.lastVisit")}</th>
                  </tr>
                </thead>
                <tbody>
                  {tracks.map((tr: TrackSummary) => {
                    const on = tr.track_course === track;
                    return (
                      <tr
                        key={`${tr.track_venue}/${tr.track_course}`}
                        className={`is-link${on ? " is-selected" : ""}`}
                        onClick={() => setTrack(on ? null : tr.track_course)}
                      >
                        <td>
                          <span className="is-strong">{tr.track_course}</span>
                          {tr.track_venue !== tr.track_course && (
                            <span className="dim track-venue"> {tr.track_venue}</span>
                          )}
                        </td>
                        <td className="num is-num is-best">{fmtLapTime(tr.best_lap_s)}</td>
                        <td className="is-optional">
                          {tr.best_lap_s !== null ? (
                            <>
                              <span>{tr.best_lap_vehicle}</span>
                              <span className="dim num"> · {fmtDate(tr.best_lap_date, lang)}</span>
                            </>
                          ) : (
                            <span className="faint">—</span>
                          )}
                        </td>
                        <td className="num is-num">
                          {tr.sessions}
                          {tr.races > 0 && (
                            <small className="faint">
                              {" "}
                              {t("profile.col.racesShort", {
                                races: nOf(tr.races, "word.race", "word.races"),
                              })}
                            </small>
                          )}
                        </td>
                        <td className="num is-num">
                          {tr.best_class_position !== null ? (
                            <>
                              <span
                                className={tr.best_class_position <= 3 ? "is-podium" : undefined}
                              >
                                {tr.best_class_position}
                              </span>
                              {tr.avg_class_position !== null && tr.races > 1 && (
                                <small className="faint">
                                  {" "}
                                  {t("profile.col.avgShort", {
                                    n: fmtNum(tr.avg_class_position, 1),
                                  })}
                                </small>
                              )}
                            </>
                          ) : (
                            <span className="faint">—</span>
                          )}
                        </td>
                        <td className="num is-num dim is-optional">
                          {fmtDate(tr.last_date, lang)}
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
          </section>

          <div className="profile-grid">
            <section className="card">
              <div className="card-head">
                <h2>{t("profile.chart.title")}</h2>
                <div className="card-actions">
                  <select
                    className="select chart-select"
                    value={chartTrack ?? ""}
                    onChange={(e) => setTrack(e.target.value || null)}
                    aria-label={t("sessions.filter.track")}
                  >
                    {tracks.map((tr) => (
                      <option key={tr.track_course} value={tr.track_course}>
                        {tr.track_course}
                      </option>
                    ))}
                  </select>
                </div>
              </div>
              {trackResults.error ? (
                <Notice kind="error">{t("common.serverError", { msg: trackResults.error })}</Notice>
              ) : (
                <BestLapChart points={points} />
              )}
            </section>

            <section className="card">
              <div className="card-head">
                <h2>{t("profile.vehicles.title")}</h2>
              </div>
              <div className="tbl-wrap">
                <table className="tbl vehicles-tbl">
                  <thead>
                    <tr>
                      <th>{t("sessions.col.car")}</th>
                      <th className="is-num">{t("profile.col.sessions")}</th>
                      <th className="is-num">{t("sessions.col.laps")}</th>
                      <th className="is-num">{t("sessions.col.best")}</th>
                      <th className="is-optional">{t("profile.col.bestAt")}</th>
                    </tr>
                  </thead>
                  <tbody>
                    {p.vehicles.map((v) => (
                      <tr key={`${v.vehicle_name}/${v.car_class}`}>
                        <td>
                          <span className="is-strong">{v.vehicle_name}</span>
                          <span className="dim"> · {classLabel(v.car_class)}</span>
                        </td>
                        <td className="num is-num">{v.sessions}</td>
                        <td className="num is-num">{v.laps}</td>
                        <td className="num is-num is-best">{fmtLapTime(v.best_lap_s)}</td>
                        <td className="is-optional dim">{v.best_track ?? "—"}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </section>
          </div>

          <section className="card">
            <div className="card-head">
              <h2>
                {filtered
                  ? t("profile.results.onTrack", { track: filtered.track_course })
                  : t("profile.results.recent")}
              </h2>
              <div className="card-actions">
                {filtered ? (
                  <button type="button" className="btn" onClick={() => setTrack(null)}>
                    {t("profile.results.all")}
                  </button>
                ) : (
                  <span className="dim selhint">{t("profile.results.recentHint")}</span>
                )}
              </div>
            </div>
            {filtered ? (
              trackResults.loading && onTrack.length === 0 ? (
                <Notice kind="loading">{t("common.loading")}</Notice>
              ) : (
                <ResultsTable rows={onTrack} showTrack={false} />
              )
            ) : (
              <ResultsTable rows={p.recent} showTrack />
            )}
          </section>
        </>
      )}

      <ImportFooter
        profile={p}
        onImported={() => {
          profile.reload();
          trackResults.reload();
        }}
      />
    </div>
  );
}
