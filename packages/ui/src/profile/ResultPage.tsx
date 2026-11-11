import { useMemo, useState } from "react";
import { useT } from "../i18n";
import { COLORS, classColor } from "../lib/color";
import {
  fmtClock,
  fmtDateTime,
  fmtDelta,
  fmtDuration,
  fmtLapTime,
  fmtNum,
  fmtPct,
} from "../lib/format";
import { routeHash } from "../nav/routes";
import { type FileDriver, fetchResult, type ResultDetail } from "../net/profile-api";
import { Notice, PageHeader, SessionTypeTag, useAsync } from "../sessions/shared";
import {
  classKeyOf,
  classLabel,
  fmtPos,
  type Gap,
  gained,
  type LapTone,
  lapTones,
  standings,
} from "./lib";

const TONE_COLOR: Record<LapTone, string | undefined> = {
  best: COLORS.best,
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

function ClassTag({ carClass }: { carClass: string }) {
  const color = classColor(classKeyOf(carClass));
  return (
    <span className="classtag" style={{ borderColor: color, color }}>
      {classLabel(carClass)}
    </span>
  );
}

function GapCell({ gap, race }: { gap: Gap; race: boolean }) {
  const { t } = useT();
  switch (gap.kind) {
    case "leader":
      return (
        <span className="is-strong">{race ? t("result.gap.winner") : t("result.gap.fastest")}</span>
      );
    case "time":
      return <span>{race ? `+${fmtLapTime(gap.s, { ms: 3 })}` : fmtDelta(gap.s)}</span>;
    case "laps":
      return (
        <span>{gap.n === 1 ? t("result.gap.oneLap") : t("result.gap.laps", { n: gap.n })}</span>
      );
    case "dnf":
      return (
        <span className="is-dnf">
          {t("profile.finish.dnf")}
          {gap.reason && gap.reason !== "DNF" && <small className="faint"> · {gap.reason}</small>}
        </span>
      );
    case "dq":
      return <span className="is-dnf">{t("profile.finish.dq")}</span>;
    default:
      return <span className="faint">—</span>;
  }
}

function Standings({ detail, me }: { detail: ResultDetail; me: FileDriver | null }) {
  const { t } = useT();
  const race = detail.session_kind === "race";
  const classes = useMemo(() => {
    const set = new Map<string, number>();
    for (const d of detail.file.drivers) set.set(d.car_class, (set.get(d.car_class) ?? 0) + 1);
    return [...set.entries()].sort((a, b) => b[1] - a[1]).map(([c]) => c);
  }, [detail.file.drivers]);
  const [cls, setCls] = useState<string | null>(() =>
    classes.length > 1 ? (me?.car_class ?? classes[0] ?? null) : null,
  );
  const rows = useMemo(
    () => standings(detail.file.drivers, cls, race),
    [detail.file.drivers, cls, race],
  );
  return (
    <section className="card">
      <div className="card-head">
        <h2>{t("result.standings.title")}</h2>
        {classes.length > 1 && (
          <div className="seg" role="tablist" aria-label={t("result.standings.class")}>
            <button
              type="button"
              className={`seg-btn${cls === null ? " is-on" : ""}`}
              onClick={() => setCls(null)}
            >
              {t("result.standings.all")}
            </button>
            {classes.map((c) => (
              <button
                key={c}
                type="button"
                className={`seg-btn${cls === c ? " is-on" : ""}`}
                onClick={() => setCls(c)}
              >
                {classLabel(c)}
              </button>
            ))}
          </div>
        )}
      </div>
      <div className="tbl-wrap">
        <table className="tbl standings-tbl">
          <thead>
            <tr>
              <th className="is-num">{t("standings.pos")}</th>
              {cls === null && classes.length > 1 && (
                <th className="is-num is-optional">{t("result.col.classPos")}</th>
              )}
              <th>{t("standings.driver")}</th>
              <th>{t("sessions.col.car")}</th>
              {cls === null && <th className="is-optional">{t("result.col.class")}</th>}
              <th className="is-num">{t("sessions.col.laps")}</th>
              <th className="is-num">{t("sessions.col.best")}</th>
              <th className="is-num">{race ? t("result.col.gap") : t("result.col.gapBest")}</th>
              {race && <th className="is-num is-optional">{t("result.col.grid")}</th>}
              {race && <th className="is-num is-optional">{t("result.col.pits")}</th>}
            </tr>
          </thead>
          <tbody>
            {rows.map(({ driver: d, classPos, gap }, i) => {
              const isMe = me !== null && d === me;
              const pos = cls === null ? (d.position ?? i + 1) : classPos;
              return (
                <tr
                  key={`${d.name}-${d.car_number ?? i}`}
                  className={isMe ? "is-selected is-me" : undefined}
                >
                  <td className="num is-num is-strong">{pos}</td>
                  {cls === null && classes.length > 1 && (
                    <td className="num is-num dim is-optional">{d.class_position ?? "—"}</td>
                  )}
                  <td>
                    <span className="is-strong">{d.name}</span>
                    {d.team && <span className="dim driver-team"> {d.team}</span>}
                  </td>
                  <td>
                    {d.car_type || d.vehicle_name}
                    {d.car_number && <span className="faint num"> #{d.car_number}</span>}
                  </td>
                  {cls === null && (
                    <td className="is-optional">
                      <ClassTag carClass={d.car_class} />
                    </td>
                  )}
                  <td className="num is-num">{d.laps_completed}</td>
                  <td className="num is-num">{fmtLapTime(d.best_lap_s)}</td>
                  <td className="num is-num">
                    <GapCell gap={gap} race={race} />
                  </td>
                  {race && (
                    <td className="num is-num dim is-optional">
                      {cls === null ? (d.grid_pos ?? "—") : (d.class_grid_pos ?? d.grid_pos ?? "—")}
                    </td>
                  )}
                  {race && <td className="num is-num dim is-optional">{d.pitstops}</td>}
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
    </section>
  );
}

function PlayerLaps({ detail }: { detail: ResultDetail }) {
  const { t } = useT();
  const laps = useMemo(
    () => [...detail.laps].sort((a, b) => a.lap_number - b.lap_number),
    [detail.laps],
  );
  const tones = useMemo(() => lapTones(laps), [laps]);
  const best = useMemo(() => {
    const timed = laps.filter((l) => l.lap_time_s !== null && !l.pit);
    return timed.length ? Math.min(...timed.map((l) => l.lap_time_s as number)) : null;
  }, [laps]);
  const hasFuel = laps.some((l) => l.fuel_fraction !== null);
  const hasWear = laps.some((l) => l.tyre_wear !== null);
  const hasSpeed = laps.some((l) => l.top_speed_kmh !== null);
  if (laps.length === 0) return <Notice kind="empty">{t("result.laps.none")}</Notice>;
  return (
    <section className="card">
      <div className="card-head">
        <h2>{t("result.laps.title")}</h2>
        <span className="dim selhint">{t("result.laps.hint")}</span>
      </div>
      <div className="tbl-wrap">
        <table className="tbl laps-tbl result-laps-tbl">
          <thead>
            <tr>
              <th className="is-num">{t("session.col.lap")}</th>
              <th className="is-num">{t("session.col.time")}</th>
              <th className="is-num">S1</th>
              <th className="is-num">S2</th>
              <th className="is-num">S3</th>
              <th className="is-num">{t("standings.pos")}</th>
              {hasSpeed && <th className="is-num is-optional">{t("session.col.topSpeed")}</th>}
              {hasFuel && <th className="is-num is-optional">{t("session.col.fuel")}</th>}
              <th className="is-optional">{t("result.col.compounds")}</th>
              {hasWear && <th className="is-optional">{t("session.col.wear")}</th>}
              <th />
            </tr>
          </thead>
          <tbody>
            {laps.map((l) => {
              const tone = tones.get(l);
              const delta =
                best !== null && l.lap_time_s !== null && tone?.time !== "muted"
                  ? l.lap_time_s - best
                  : null;
              return (
                <tr key={l.lap_number} className={tone?.time === "muted" ? "is-muted" : undefined}>
                  <td className="num is-num is-strong">{l.lap_number}</td>
                  <td
                    className="num is-num is-time"
                    style={{ color: TONE_COLOR[tone?.time ?? "normal"] }}
                  >
                    {fmtLapTime(l.lap_time_s)}
                    {delta !== null && delta > 0.0005 && (
                      <small className="faint"> {fmtDelta(delta, 3)}</small>
                    )}
                  </td>
                  <td className="num is-num" style={{ color: TONE_COLOR[tone?.s1 ?? "normal"] }}>
                    {fmtLapTime(l.s1, { ms: 3 })}
                  </td>
                  <td className="num is-num" style={{ color: TONE_COLOR[tone?.s2 ?? "normal"] }}>
                    {fmtLapTime(l.s2, { ms: 3 })}
                  </td>
                  <td className="num is-num" style={{ color: TONE_COLOR[tone?.s3 ?? "normal"] }}>
                    {fmtLapTime(l.s3, { ms: 3 })}
                  </td>
                  <td className="num is-num">
                    {l.position !== null && l.position > 0 && l.position < 100 ? l.position : "—"}
                  </td>
                  {hasSpeed && (
                    <td className="num is-num is-optional">{fmtNum(l.top_speed_kmh, 0)}</td>
                  )}
                  {hasFuel && (
                    <td className="num is-num is-optional">{fmtPct(l.fuel_fraction, 0)}</td>
                  )}
                  <td className="is-optional dim">
                    {l.front_compound
                      ? l.front_compound === l.rear_compound || !l.rear_compound
                        ? l.front_compound
                        : `${l.front_compound} / ${l.rear_compound}`
                      : "—"}
                  </td>
                  {hasWear && (
                    <td className="num is-optional dim">
                      {l.tyre_wear ? l.tyre_wear.map((w) => fmtPct(w, 0)).join(" · ") : "—"}
                    </td>
                  )}
                  <td>
                    {l.pit && <span className="pill">{t("result.lap.pit")}</span>}
                    {!l.pit && l.lap_time_s === null && (
                      <span className="pill is-faint">{t("result.lap.noTime")}</span>
                    )}
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
    </section>
  );
}

function Events({ detail }: { detail: ResultDetail }) {
  const { t } = useT();
  const player = detail.player_name.trim().toLowerCase();
  const all = detail.file.events;
  // A contact is usually reported by both cars: keep the player's line, drop the mirror.
  const events = useMemo(
    () =>
      all.filter((e) => {
        if (e.kind !== "contact" || e.driver.trim().toLowerCase() === player) return true;
        return !all.some(
          (o) =>
            o.kind === "contact" &&
            o.driver.trim().toLowerCase() === player &&
            o.other?.trim().toLowerCase() === e.driver.trim().toLowerCase() &&
            Math.abs(o.et_s - e.et_s) < 2,
        );
      }),
    [all, player],
  );
  if (!events.length) return null;
  return (
    <section className="card">
      <div className="card-head">
        <h2>{t("result.events.title")}</h2>
        <span className="dim selhint">{t("result.events.hint")}</span>
      </div>
      <ul className="events">
        {events.map((e, i) => (
          <li key={`${e.et_s}-${i}`} className={`event is-${e.kind}`}>
            <span className="num dim event-time">{fmtClock(e.et_s)}</span>
            <span className="num faint event-lap">
              {e.lap !== null ? `${t("session.col.lap")} ${e.lap}` : ""}
            </span>
            <span className="event-text">
              {e.kind === "contact"
                ? e.driver.trim().toLowerCase() !== player
                  ? `${t("result.events.contactWith", { who: e.driver })} (${t("result.events.reportedBy")})`
                  : e.other
                    ? t("result.events.contactWith", { who: e.other })
                    : t("result.events.contact", { what: e.text.replace(/^contact with /, "") })
                : e.kind === "penalty"
                  ? t("result.events.penalty", { what: e.text })
                  : e.text === "Warning"
                    ? t("result.events.warning")
                    : e.text.startsWith("Invalid Lap")
                      ? t("result.events.invalidLap")
                      : e.text === "No Further Action"
                        ? t("result.events.noAction")
                        : e.text}
            </span>
          </li>
        ))}
      </ul>
    </section>
  );
}

export function ResultPage({ id }: { id: number }) {
  const { t, nOf, lang } = useT();
  const detail = useAsync((signal) => fetchResult(id, signal), [id]);
  const d = detail.data;
  const me = useMemo(() => {
    if (!d) return null;
    const key = d.player_name.trim().toLowerCase();
    return d.file.drivers.find((x) => x.name.trim().toLowerCase() === key) ?? null;
  }, [d]);

  const back = { page: "profile" as const };
  if (detail.error && !d) {
    const notFound = /404/.test(detail.error);
    return (
      <div className="result-page">
        <PageHeader back={back} backLabel={t("nav.profile")} title={t("result.title", { id })} />
        <Notice
          kind="error"
          title={notFound ? t("result.notFound") : t("common.serverError", { msg: detail.error })}
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
  if (!d) {
    return (
      <div className="result-page">
        <PageHeader back={back} backLabel={t("nav.profile")} title={t("result.title", { id })} />
        <Notice kind="loading">{t("common.loading")}</Notice>
      </div>
    );
  }

  const race = d.session_kind === "race";
  const classWinner = standings(d.file.drivers, d.player_class, race)[0]?.driver ?? null;
  const bestLap = d.laps.filter((l) => l.lap_time_s !== null && !l.pit);
  const bestLapRow = bestLap.length
    ? bestLap.reduce((a, b) => ((a.lap_time_s as number) <= (b.lap_time_s as number) ? a : b))
    : null;
  const classGain = gained(d.player_class_grid_pos, d.player_class_position);
  const inc = me?.incidents ?? null;
  const lapsDown = classWinner && me ? classWinner.laps_completed - me.laps_completed : null;

  return (
    <div className="result-page">
      <PageHeader
        back={back}
        backLabel={t("nav.profile")}
        title={d.track_course}
        subtitle={
          <>
            <SessionTypeTag type={d.session_kind} />
            <span className="is-strong"> {d.player_vehicle}</span>
            {d.player_class && <span> · {classLabel(d.player_class)}</span>}
            {me?.car_number && <span className="num"> · #{me.car_number}</span>}
            {me?.team && <span> · {me.team}</span>}
          </>
        }
        meta={
          <dl className="meta">
            <div>
              <dt>{t("session.started")}</dt>
              <dd className="num">{fmtDateTime(d.started_wall_ms, lang)}</dd>
            </div>
            <div>
              <dt>{t("result.meta.format")}</dt>
              <dd className="num">
                {d.minutes !== null
                  ? fmtDuration(d.minutes * 60)
                  : d.laps_planned !== null
                    ? t("result.meta.lapsPlanned", {
                        laps: nOf(d.laps_planned, "word.lap", "word.laps"),
                      })
                    : "—"}
                {d.setting && <span className="dim"> · {d.setting}</span>}
              </dd>
            </div>
            <div>
              <dt>{t("result.meta.entries")}</dt>
              <dd className="num">
                {t("result.meta.entriesValue", {
                  cars: nOf(d.num_drivers, "word.car", "word.cars"),
                  c: d.num_class_drivers,
                  cls: classLabel(d.player_class),
                })}
              </dd>
            </div>
            {d.track_venue !== d.track_course && (
              <div>
                <dt>{t("session.track")}</dt>
                <dd>{d.track_venue}</dd>
              </div>
            )}
            <div>
              <dt>{t("result.meta.file")}</dt>
              <dd className="num result-file" title={d.file_name}>
                {d.file_name}
                {d.game_version && <span className="dim"> · LMU {d.game_version}</span>}
              </dd>
            </div>
            <div>
              <dt>{t("result.meta.recorded")}</dt>
              <dd>
                {d.linked_session_id ? (
                  <a
                    className="link"
                    href={routeHash({ page: "session", id: d.linked_session_id })}
                  >
                    {t("result.meta.openSession")}
                  </a>
                ) : (
                  <span className="faint">{t("result.meta.noSession")}</span>
                )}
              </dd>
            </div>
          </dl>
        }
      />

      <section className="kpis">
        <Kpi
          label={t("result.kpi.classPos")}
          value={fmtPos(d.player_class_position, d.num_class_drivers)}
          hint={
            race && classGain !== null
              ? t("result.kpi.fromGrid", {
                  grid: d.player_class_grid_pos ?? 0,
                  delta: classGain > 0 ? `+${classGain}` : String(classGain),
                })
              : classLabel(d.player_class)
          }
          color={race && (d.player_class_position ?? 99) <= 3 ? COLORS.accent : undefined}
        />
        <Kpi
          label={t("result.kpi.overallPos")}
          value={fmtPos(d.player_position, d.num_drivers)}
          hint={
            race && d.player_grid_pos !== null
              ? t("result.kpi.grid", { n: d.player_grid_pos })
              : undefined
          }
        />
        <Kpi
          label={t("session.kpi.best")}
          value={fmtLapTime(d.player_best_lap_s)}
          hint={bestLapRow ? `${t("session.col.lap")} ${bestLapRow.lap_number}` : undefined}
          color={d.player_best_lap_s !== null ? COLORS.best : undefined}
        />
        <Kpi
          label={t("sessions.col.laps")}
          value={String(d.player_laps)}
          hint={
            race && lapsDown !== null && lapsDown > 0
              ? t("result.kpi.lapsDown", { laps: nOf(lapsDown, "word.lap", "word.laps") })
              : classWinner && race
                ? t("result.kpi.sameLap")
                : undefined
          }
        />
        <Kpi
          label={t("result.kpi.incidents")}
          value={inc ? String(inc.contacts) : "—"}
          hint={
            inc
              ? t("result.kpi.incidentsHint", {
                  warnings: nOf(inc.track_limit_warnings, "word.warning", "word.warnings"),
                  invalid: nOf(inc.invalid_laps, "word.invalidLap", "word.invalidLaps"),
                })
              : undefined
          }
          color={inc && inc.contacts > 0 ? COLORS.warm : undefined}
        />
        <Kpi
          label={t("result.kpi.pits")}
          value={me ? String(me.pitstops) : "—"}
          hint={
            inc && inc.penalties > 0
              ? t("result.kpi.penalties", {
                  penalties: nOf(inc.penalties, "word.penalty", "word.penalties"),
                })
              : undefined
          }
          color={inc && inc.penalties > 0 ? COLORS.slower : undefined}
        />
      </section>

      <Standings detail={d} me={me} />
      <PlayerLaps detail={d} />
      <Events detail={d} />
    </div>
  );
}
