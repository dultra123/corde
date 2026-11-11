import { useCallback, useMemo, useState } from "react";
import { IconEngineer } from "../dashboard/icons";
import { useT } from "../i18n";
import { BRAKE_SCALE, COLORS, tempColor, tyreScale, wearColor } from "../lib/color";
import {
  fmtBytes,
  fmtDateTime,
  fmtDelta,
  fmtDuration,
  fmtLapTime,
  fmtNum,
  fmtPct,
} from "../lib/format";
import { lapNo, lapStatus, lapStatusHintKey, lapStatusKey } from "../lib/laps";
import { routeHash } from "../nav/routes";
import { fetchSession, type LapRow } from "../net/sessions-api";
import { useRecording } from "../store/recording";
import { LapTimesChart } from "./LapTimesChart";
import { lapStats, lapTones, sessionDurationS, type Tone } from "./lib/stats";
import { MiniBars, Notice, PageHeader, SessionTypeTag, useAsync, weatherSummary } from "./shared";

const TONE_COLOR: Record<Tone, string | undefined> = {
  best: COLORS.best,
  pb: COLORS.faster,
  normal: undefined,
  muted: COLORS.faint,
};

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

function LapFlags({ lap }: { lap: LapRow }) {
  const { t } = useT();
  const flags: { key: string; cls: string }[] = [];
  if (!lap.valid) flags.push({ key: t("session.lap.invalid"), cls: "is-bad" });
  if (lap.out_lap) flags.push({ key: t("session.lap.outLap"), cls: "" });
  if (lap.in_lap) flags.push({ key: t("session.lap.inLap"), cls: "" });
  if (!flags.length) return null;
  return (
    <span className="lapflags">
      {flags.map((f) => (
        <span key={f.key} className={`pill ${f.cls}`}>
          {f.key}
        </span>
      ))}
    </span>
  );
}

export function SessionPage({ id }: { id: string }) {
  const { t, lang } = useT();
  const revision = useRecording((s) => s.revision);
  const recLaps = useRecording((s) => s.status?.laps ?? -1);
  const recId = useRecording((s) => s.status?.session_id ?? null);
  const live = recId === id;
  const detail = useAsync((signal) => fetchSession(id, signal), [id, revision, live ? recLaps : 0]);
  const [selected, setSelected] = useState<number[]>([]);

  const toggle = useCallback((n: number) => {
    setSelected((cur) => {
      if (cur.includes(n)) return cur.filter((x) => x !== n);
      const next = [...cur, n];
      return next.length > 2 ? next.slice(-2) : next;
    });
  }, []);

  const session = detail.data;
  const laps = useMemo(
    () => [...(session?.laps ?? [])].sort((a, b) => a.lap_number - b.lap_number),
    [session],
  );
  const stats = useMemo(() => lapStats(laps), [laps]);
  const tones = useMemo(() => lapTones(laps), [laps]);
  const tyreRange = useMemo(() => {
    const all = laps.flatMap((l) => l.avg_tyre_temp_c).filter((v) => Number.isFinite(v));
    if (!all.length) return { min: 40, max: 120 };
    return { min: Math.min(...all) - 8, max: Math.max(...all) + 8 };
  }, [laps]);
  const brakeRange = useMemo(() => {
    const all = laps.flatMap((l) => l.max_brake_temp_c).filter((v) => Number.isFinite(v));
    if (!all.length) return { min: 100, max: 800 };
    return { min: Math.min(...all) - 50, max: Math.max(...all) + 50 };
  }, [laps]);
  const tyreColor = useMemo(() => {
    const scale = tyreScale(null);
    return (v: number) => tempColor(v, scale);
  }, []);
  const brakeColor = (v: number) => tempColor(v, BRAKE_SCALE);
  const fileBytes = session ? session.files.reduce((a, f) => a + f.bytes, 0) : 0;

  if (detail.error && !session) {
    const notFound = /404/.test(detail.error);
    return (
      <div className="session-page">
        <PageHeader back={{ page: "sessions" }} backLabel={t("nav.sessions")} title={id} />
        <Notice
          kind="error"
          title={notFound ? t("session.notFound") : t("common.serverError", { msg: detail.error })}
          action={
            !notFound && (
              <button type="button" className="btn" onClick={detail.reload}>
                {t("common.retry")}
              </button>
            )
          }
        />
      </div>
    );
  }
  if (!session) {
    return (
      <div className="session-page">
        <PageHeader back={{ page: "sessions" }} backLabel={t("nav.sessions")} title={id} />
        <Notice kind="loading">{t("common.loading")}</Notice>
      </div>
    );
  }

  const [a, b] = selected;
  const action =
    selected.length === 2 && a !== undefined && b !== undefined ? (
      <a
        className="btn is-active is-cta"
        href={routeHash({ page: "compare", id, a: Math.min(a, b), b: Math.max(a, b) })}
      >
        {t("session.compare", { a: lapNo(Math.min(a, b)), b: lapNo(Math.max(a, b)) })}
      </a>
    ) : selected.length === 1 && a !== undefined ? (
      <a className="btn is-active is-cta" href={routeHash({ page: "lap", id, lap: a })}>
        {t("session.analyse", { n: lapNo(a) })}
      </a>
    ) : (
      <span className="dim selhint">{t("session.laps.selectHint")}</span>
    );

  return (
    <div className="session-page">
      <PageHeader
        back={{ page: "sessions" }}
        backLabel={t("nav.sessions")}
        title={
          <>
            {session.track_name}
            {live && <span className="livetag">{t("sessions.inProgress")}</span>}
          </>
        }
        subtitle={
          <>
            <SessionTypeTag type={session.session_type} />
            <span className="is-strong"> {session.vehicle_model || session.vehicle_name}</span>
            {session.class_name && <span> · {session.class_name}</span>}
            {session.player_name && <span> · {session.player_name}</span>}
          </>
        }
        actions={
          <a
            className="btn is-active is-cta is-engineer"
            href={routeHash({ page: "engineer", id })}
            title={t("eng.openHint")}
          >
            <IconEngineer />
            {t("eng.open")}
          </a>
        }
        meta={
          <dl className="meta">
            <div>
              <dt>{t("session.started")}</dt>
              <dd className="num">{fmtDateTime(session.started_wall_ms, lang)}</dd>
            </div>
            <div>
              <dt>{t("session.duration")}</dt>
              <dd className="num">
                {fmtDuration(sessionDurationS(session.started_wall_ms, session.ended_wall_ms))}
              </dd>
            </div>
            <div>
              <dt>{t("sessions.col.weather")}</dt>
              <dd>{weatherSummary(session.weather, t)}</dd>
            </div>
            <div>
              <dt>{t("session.rate")}</dt>
              <dd className="num">{fmtNum(session.record_rate_hz, 0)} Hz</dd>
            </div>
            <div>
              <dt>{t("session.size")}</dt>
              <dd className="num">{fmtBytes(fileBytes, lang)}</dd>
            </div>
          </dl>
        }
      />

      <section className="kpis">
        <Kpi
          label={t("session.kpi.best")}
          value={fmtLapTime(stats.best?.lap_time_s ?? null)}
          hint={stats.best ? `${t("session.col.lap")} ${lapNo(stats.best.lap_number)}` : undefined}
          color={stats.best ? COLORS.best : undefined}
        />
        <Kpi
          label={t("session.kpi.mean")}
          value={fmtLapTime(stats.mean)}
          hint={t("session.kpi.meanHint")}
        />
        <Kpi
          label={t("session.kpi.consistency")}
          value={stats.stddev === null ? "—" : `± ${stats.stddev.toFixed(3)}`}
          unit={stats.stddev === null ? undefined : "s"}
          hint={t("session.kpi.consistencyHint")}
        />
        <Kpi
          label={t("session.kpi.fuel")}
          value={stats.fuelPerLap === null ? "—" : stats.fuelPerLap.toFixed(2)}
          unit={stats.fuelPerLap === null ? undefined : "L"}
          hint={t("session.kpi.fuelHint")}
        />
        <Kpi
          label={t("session.kpi.topSpeed")}
          value={fmtNum(stats.topSpeed, 0)}
          unit={stats.topSpeed === null ? undefined : "km/h"}
        />
        <Kpi
          label={t("session.kpi.validLaps")}
          value={String(stats.counted)}
          hint={t("session.kpi.validLapsHint", { n: stats.total })}
        />
      </section>

      {laps.length === 0 ? (
        <Notice kind="empty">{t("session.laps.none")}</Notice>
      ) : (
        <>
          <section className="card">
            <div className="card-head">
              <h2>{t("session.chart.title")}</h2>
            </div>
            <LapTimesChart
              laps={laps}
              bestLapS={stats.best?.lap_time_s ?? null}
              selected={selected}
              onToggle={toggle}
            />
          </section>

          <section className="card">
            <div className="card-head">
              <h2>{t("session.laps.title")}</h2>
              <div className="card-actions">{action}</div>
            </div>
            <p className="laps-legend dim">
              {t("session.laps.legend")}
              {laps.some((l) => l.time_source === "results") && (
                <> {t("session.laps.fromResults")}</>
              )}
            </p>
            <div className="tbl-wrap">
              <table className="tbl laps-tbl">
                <thead>
                  <tr>
                    <th className="is-check" />
                    <th className="is-num">{t("session.col.lap")}</th>
                    <th className="is-num">{t("session.col.time")}</th>
                    <th>{t("session.col.status")}</th>
                    <th className="is-num">S1</th>
                    <th className="is-num">S2</th>
                    <th className="is-num">S3</th>
                    <th className="is-num">{t("session.col.fuel")}</th>
                    <th className="is-num">{t("session.col.topSpeed")}</th>
                    <th>{t("session.col.tyres")}</th>
                    <th>{t("session.col.brakes")}</th>
                    <th>{t("session.col.wear")}</th>
                    <th />
                  </tr>
                </thead>
                <tbody>
                  {laps.map((lap) => {
                    const tone = tones.get(lap.lap_number);
                    const isSel = selected.includes(lap.lap_number);
                    const delta =
                      stats.best && tone?.time !== "muted"
                        ? lap.lap_time_s - stats.best.lap_time_s
                        : null;
                    const hottestTyre = Math.max(...lap.avg_tyre_temp_c);
                    const hottestBrake = Math.max(...lap.max_brake_temp_c);
                    const minWear = Math.min(...lap.wear_remaining_end);
                    const status = lapStatus(lap);
                    return (
                      <tr
                        key={lap.lap_number}
                        className={`is-link${isSel ? " is-selected" : ""}${tone?.time === "muted" ? " is-muted" : ""}`}
                        onClick={() => toggle(lap.lap_number)}
                      >
                        <td className="is-check">
                          <input
                            type="checkbox"
                            checked={isSel}
                            onChange={() => toggle(lap.lap_number)}
                            onClick={(e) => e.stopPropagation()}
                            aria-label={`${t("session.col.lap")} ${lapNo(lap.lap_number)}`}
                          />
                        </td>
                        <td className="num is-num is-strong">{lapNo(lap.lap_number)}</td>
                        <td
                          className="num is-num is-time"
                          style={{ color: TONE_COLOR[tone?.time ?? "normal"] }}
                        >
                          {fmtLapTime(lap.lap_time_s)}
                          {delta !== null && delta > 0.0005 && (
                            <small className="faint"> {fmtDelta(delta, 3)}</small>
                          )}
                        </td>
                        <td
                          className={`lap-status is-${status}`}
                          title={t(lapStatusHintKey(status))}
                        >
                          {t(lapStatusKey(status))}
                        </td>
                        <td
                          className="num is-num"
                          style={{ color: TONE_COLOR[tone?.s1 ?? "normal"] }}
                        >
                          {fmtLapTime(lap.s1, { ms: 3 })}
                        </td>
                        <td
                          className="num is-num"
                          style={{ color: TONE_COLOR[tone?.s2 ?? "normal"] }}
                        >
                          {fmtLapTime(lap.s2, { ms: 3 })}
                        </td>
                        <td
                          className="num is-num"
                          style={{ color: TONE_COLOR[tone?.s3 ?? "normal"] }}
                        >
                          {fmtLapTime(lap.s3, { ms: 3 })}
                        </td>
                        <td className="num is-num">
                          {lap.fuel_used_l > 0 ? `${lap.fuel_used_l.toFixed(2)} L` : "—"}
                        </td>
                        <td className="num is-num">{fmtNum(lap.top_speed_kmh, 0)}</td>
                        <td className="is-bars">
                          <MiniBars
                            values={lap.avg_tyre_temp_c}
                            min={tyreRange.min}
                            max={tyreRange.max}
                            color={tyreColor}
                            title={lap.avg_tyre_temp_c.map((v) => `${v.toFixed(0)}°`).join(" · ")}
                          />
                          <span className="num bars-val">{fmtNum(hottestTyre, 0)}°</span>
                        </td>
                        <td className="is-bars">
                          <MiniBars
                            values={lap.max_brake_temp_c}
                            min={brakeRange.min}
                            max={brakeRange.max}
                            color={brakeColor}
                            title={lap.max_brake_temp_c.map((v) => `${v.toFixed(0)}°`).join(" · ")}
                          />
                          <span className="num bars-val">{fmtNum(hottestBrake, 0)}°</span>
                        </td>
                        <td className="is-bars">
                          <MiniBars
                            values={lap.wear_remaining_end}
                            min={0}
                            max={1}
                            color={wearColor}
                            title={lap.wear_remaining_end.map((v) => fmtPct(v, 0)).join(" · ")}
                          />
                          <span className="num bars-val">{fmtPct(minWear, 0)}</span>
                        </td>
                        <td>
                          <LapFlags lap={lap} />
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
          </section>
        </>
      )}
    </div>
  );
}
