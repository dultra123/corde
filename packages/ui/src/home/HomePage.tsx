import { type ReactNode, useEffect, useMemo, useState } from "react";
import { IconEngineer } from "../dashboard/icons";
import { type TranslationKey, useT } from "../i18n";
import { isDesktop, isLocalPage, restartApp } from "../lib/desktop";
import { fmtBytes, fmtDuration, fmtLapTime, fmtRelativeDay } from "../lib/format";
import { lapNo } from "../lib/laps";
import { routeHash } from "../nav/routes";
import { connection } from "../net/connection";
import { fetchProfile, fetchResults, type Profile, type ResultRow } from "../net/profile-api";
import { ApiError, fetchSession, fetchSessions, fetchStorage } from "../net/sessions-api";
import { fetchNetwork, saveSettings } from "../net/system-api";
import { classLabel } from "../profile/lib";
import { LapTimesChart } from "../sessions/LapTimesChart";
import { lapStats, sessionDurationS } from "../sessions/lib/stats";
import {
  errorMessage,
  SessionTypeTag,
  sessionTypeKey,
  useAsync,
  weatherSummary,
} from "../sessions/shared";
import { useRecording } from "../store/recording";
import { useSettings } from "../store/settings";
import { useConnectionPhase, useTelemetry } from "../store/telemetry";
import { GuideDialog } from "./GuideDialog";
import {
  bareAddress,
  type HomePhase,
  homeLamps,
  homePhase,
  type Lamp,
  lastRaces,
  matchResult,
  ordinalSuffix,
  phoneState,
} from "./lib";
import { Qr } from "./Qr";

/** Bumps once a minute while the page is visible: results are imported in the background. */
function useMinuteTick(): number {
  const [n, setN] = useState(0);
  useEffect(() => {
    const id = setInterval(() => {
      if (document.visibilityState !== "hidden") setN((x) => x + 1);
    }, 60_000);
    return () => clearInterval(id);
  }, []);
  return n;
}

/** `12` + a raised `e` / `er` (`th`, `st`… in English). */
function Ordinal({ n }: { n: number }) {
  const { lang } = useT();
  return (
    <>
      {n}
      <sup>{ordinalSuffix(n, lang)}</sup>
    </>
  );
}

/* ------------------------------------------------------------------ status */

const LAMP_KEYS = ["server", "game", "session", "track", "recording"] as const;

const HEADLINE: Record<HomePhase, TranslationKey> = {
  connecting: "home.state.connecting",
  offline: "home.state.offline",
  waiting: "home.state.waiting",
  menu: "home.state.game",
  garage: "home.state.game",
  track: "home.state.track",
  demo: "home.state.demo",
};

/** What the game says about the loaded session — rounded, so the card does not re-render at 20 Hz. */
function SessionFacts({ withTrack }: { withTrack: boolean }) {
  const { t, nOf } = useT();
  const track = useTelemetry((s) => s.frame?.session.track_name ?? "");
  const type = useTelemetry((s) => s.frame?.session.session_type ?? null);
  const car = useTelemetry((s) => {
    const p = s.frame?.player;
    if (p) return p.vehicle_model || p.vehicle_name;
    return s.frame?.standings.find((x) => x.is_player)?.vehicle_name ?? "";
  });
  const carClass = useTelemetry(
    (s) =>
      s.frame?.player?.class_name ?? s.frame?.standings.find((x) => x.is_player)?.class_name ?? "",
  );
  const cars = useTelemetry((s) => s.frame?.session.num_vehicles ?? 0);
  const minutesLeft = useTelemetry((s) => {
    const ses = s.frame?.session;
    if (!ses || !(ses.end_s > 0) || !(ses.time_remaining_s > 0)) return null;
    return Math.ceil(ses.time_remaining_s / 60);
  });
  const air = useTelemetry((s) => Math.round(s.frame?.session.weather.ambient_c ?? Number.NaN));
  const trackC = useTelemetry((s) => Math.round(s.frame?.session.weather.track_c ?? Number.NaN));
  const rain = useTelemetry((s) => Math.round((s.frame?.session.weather.rain ?? 0) * 20) / 20);
  const damp = useTelemetry((s) => (s.frame?.session.weather.wetness_avg ?? 0) > 0.1);

  return (
    <dl className="meta home-facts">
      {withTrack && track && (
        <div>
          <dt>{t("session.track")}</dt>
          <dd>{track}</dd>
        </div>
      )}
      {withTrack && type && (
        <div>
          <dt>{t("home.lamp.session")}</dt>
          <dd>{t(sessionTypeKey(type))}</dd>
        </div>
      )}
      {car && (
        <div>
          <dt>{t("home.fact.car")}</dt>
          <dd>
            {car}
            {carClass && <span className="dim"> · {carClass}</span>}
          </dd>
        </div>
      )}
      {minutesLeft !== null && (
        <div>
          <dt>{t("session.remaining")}</dt>
          <dd className="num">{fmtDuration(minutesLeft * 60)}</dd>
        </div>
      )}
      {cars > 0 && (
        <div>
          <dt>{t("home.fact.field")}</dt>
          <dd className="num">{nOf(cars, "word.car", "word.cars")}</dd>
        </div>
      )}
      {Number.isFinite(air) && Number.isFinite(trackC) && (
        <div>
          <dt>{t("home.fact.weather")}</dt>
          <dd>
            {weatherSummary(
              { rain, wetness_avg: damp ? 1 : 0, ambient_c: air, track_c: trackC },
              t,
            )}
          </dd>
        </div>
      )}
    </dl>
  );
}

/** One line about the recorder: what it writes right now, or when it will start. */
function RecordingLine() {
  const { t, lang } = useT();
  const status = useRecording((s) => s.status);
  const error = useRecording((s) => s.error);
  const liveTrack = useTelemetry((s) => s.frame?.session.track_name ?? "");
  if (!status) {
    if (!error) return null;
    return (
      <div className="home-rec is-error">
        <span className="home-rec-dot" aria-hidden="true" />
        <span>{t("rec.unavailable", { msg: error })}</span>
      </div>
    );
  }
  let state: "live" | "idle" | "armed" | "off";
  if (status.recording) state = status.idle_reason ? "idle" : "live";
  else state = status.armed ? "armed" : "off";

  const details: string[] = [];
  if (status.recording) {
    if (liveTrack) details.push(liveTrack);
    details.push(t("rec.lap", { n: lapNo(status.laps) }));
    details.push(fmtBytes(status.bytes, lang));
    if (status.idle_reason) details.push(status.idle_reason);
  }
  return (
    <div className={`home-rec is-${state}`}>
      <span className="home-rec-dot" aria-hidden="true" />
      <span className="home-rec-text">
        <b className="home-rec-state">{t(`rec.${state}`)}</b>
        {details.length > 0 && <span className="dim num"> — {details.join(" · ")}</span>}
        {state === "off" && (
          <span className="dim"> — {status.auto ? t("rec.offAuto") : t("rec.offManual")}</span>
        )}
      </span>
      {status.recording && status.session_id && (
        <a className="link" href={routeHash({ page: "session", id: status.session_id })}>
          {t("home.rec.open")}
        </a>
      )}
    </div>
  );
}

function Restart({ fallback }: { fallback: ReactNode }) {
  const { t } = useT();
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  if (!isDesktop()) return <>{fallback}</>;
  const run = async () => {
    setBusy(true);
    setError(null);
    try {
      await restartApp();
    } catch (e) {
      setError(errorMessage(e));
      setBusy(false);
    }
  };
  return (
    <>
      <button
        type="button"
        className="home-btn is-primary"
        onClick={() => void run()}
        disabled={busy}
      >
        {t("home.phone.restartButton")}
      </button>
      {error && (
        <span className="form-error" role="alert">
          {t("settings.appError", { msg: error })}
        </span>
      )}
    </>
  );
}

function StatusCard({ onSettings }: { onSettings: () => void }) {
  const { t } = useT();
  const connPhase = useConnectionPhase();
  const track = useTelemetry((s) => s.frame?.session.track_name.trim() ?? "");
  const type = useTelemetry((s) => s.frame?.session.session_type ?? null);
  const rec = useRecording((s) => s.status);
  const autoDashboard = useSettings((s) => s.autoDashboard);
  const phase = homePhase(connPhase, track !== "");
  const lamps = homeLamps(phase, rec);
  const linked = phase !== "connecting" && phase !== "offline" && phase !== "demo";

  let detail: string | null = null;
  if (phase === "garage") detail = t("home.state.garage");
  else if (phase === "track")
    detail = type ? `${track}, ${t(sessionTypeKey(type))}` : track || null;
  else if (phase === "demo") detail = t("home.state.demoDetail");

  let hint: string | null = null;
  if (phase === "offline") hint = t(isDesktop() ? "home.hint.offlineDesktop" : "home.hint.offline");
  else if (phase === "waiting")
    hint = t(autoDashboard ? "home.hint.waiting" : "home.hint.waitingManual");
  else if (phase === "menu") hint = t("home.hint.menu");
  else if (phase === "demo") hint = t("home.hint.demo");

  const dashboard = routeHash({ page: "dashboard" });
  let actions: ReactNode = null;
  if (phase === "track" || phase === "demo") {
    actions = (
      <>
        <a className="home-btn is-primary" href={dashboard}>
          {t("home.openDashboard")}
        </a>
        {phase === "demo" && (
          <button type="button" className="home-btn" onClick={() => connection.setDemo(false)}>
            {t("home.quitDemo")}
          </button>
        )}
      </>
    );
  } else if (phase === "garage" || phase === "menu" || phase === "waiting") {
    actions = (
      <a className="home-btn" href={dashboard}>
        {t("home.openDashboard")}
      </a>
    );
  } else if (phase === "offline") {
    actions = (
      <>
        <Restart
          fallback={
            <button type="button" className="home-btn" onClick={onSettings}>
              {t("topbar.settings")}
            </button>
          }
        />
        <a className="home-btn" href={dashboard}>
          {t("home.openDemo")}
        </a>
      </>
    );
  }

  return (
    <section className={`home-status is-${phase}`} aria-live="polite">
      {phase !== "demo" && (
        <ol className="chain" aria-label={t("home.lamps")}>
          {LAMP_KEYS.map((key) => {
            const lamp: Lamp = lamps[key];
            return (
              <li key={key} className={`chain-step is-${lamp}`}>
                <i className="chain-light" aria-hidden="true" />
                <span>{t(`home.lamp.${key}`)}</span>
                <span className="sr-only"> : {t(`home.lamp.${lamp}`)}</span>
              </li>
            );
          })}
        </ol>
      )}
      <div className="home-status-main">
        <div className="home-status-text">
          <p className="home-headline">
            {t(HEADLINE[phase])}
            {detail && (
              <span className="home-headline-detail">
                <span className="home-headline-dash" aria-hidden="true">
                  {" — "}
                </span>
                <span className="sr-only">, </span>
                {detail}
              </span>
            )}
          </p>
          {hint && <p className="home-hint dim">{hint}</p>}
        </div>
        {actions && <div className="home-status-actions">{actions}</div>}
      </div>
      {(phase === "garage" || phase === "track") && <SessionFacts withTrack={phase === "garage"} />}
      {linked && <RecordingLine />}
    </section>
  );
}

/* ------------------------------------------------------------------ last session */

function Stat({
  label,
  hint,
  children,
  tone,
}: {
  label: string;
  hint?: string;
  children: ReactNode;
  tone?: "best";
}) {
  return (
    <div className="home-stat">
      <dt className="home-stat-label">{label}</dt>
      <dd className={`home-stat-value num${tone ? ` is-${tone}` : ""}`}>{children}</dd>
      {hint && <dd className="home-stat-hint faint">{hint}</dd>}
    </div>
  );
}

/** `P3 · 3ᵉ GT3 sur 14` — where the game classified the driver in that session. */
function ResultBadge({ result }: { result: ResultRow }) {
  const { t } = useT();
  const cls = result.player_class_position;
  const overall = result.player_position;
  const out =
    result.player_finish === "dnf"
      ? t("home.last.dnf")
      : result.player_finish === "dq"
        ? t("home.last.dq")
        : null;
  if (!out && !overall && !cls) return null;
  const podium = !out && result.session_kind === "race" && cls !== null && cls <= 3;
  return (
    <a
      className={`home-result${podium ? " is-podium" : ""}${out ? " is-out" : ""}`}
      href={routeHash({ page: "result", id: result.id })}
      title={t("home.last.resultHint")}
    >
      <b className="num home-result-pos">{out ?? `P${overall ?? cls}`}</b>
      {cls !== null && cls > 0 && (
        <span>
          <Ordinal n={cls} />{" "}
          {t("home.last.classOf", {
            cls: classLabel(result.player_class),
            total: result.num_class_drivers,
          })}
        </span>
      )}
    </a>
  );
}

function EmptySession() {
  const { t } = useT();
  const steps = [1, 2, 3] as const;
  return (
    <div className="home-empty">
      <p className="home-empty-lead">{t("home.last.empty.hint")}</p>
      <ol className="home-steps">
        {steps.map((n) => (
          <li key={n} className="home-step">
            <b className="home-step-title">{t(`home.last.empty.step${n}`)}</b>
            <span className="dim home-step-hint">{t(`home.last.empty.step${n}Hint`)}</span>
          </li>
        ))}
      </ol>
    </div>
  );
}

function LastSessionCard({ minute }: { minute: number }) {
  const { t, lang } = useT();
  const revision = useRecording((s) => s.revision);
  const recId = useRecording((s) => s.status?.session_id ?? null);
  const recLaps = useRecording((s) => s.status?.laps ?? 0);
  const list = useAsync((signal) => fetchSessions({ limit: 1 }, signal), [revision, recId]);
  const row = list.data?.[0] ?? null;
  const id = row?.id ?? null;
  const live = id !== null && id === recId;
  const detail = useAsync(
    (signal) => (id ? fetchSession(id, signal) : Promise.resolve(null)),
    [id, revision, live ? recLaps : 0],
  );
  // The game writes its result file when the session ends; the server imports it within a minute.
  const results = useAsync(
    (signal) => (id ? fetchResults({ limit: 5 }, signal) : Promise.resolve([] as ResultRow[])),
    [id, revision, minute],
  );
  // re-render while the session is being recorded, so that its duration keeps counting
  useRecording((s) => (live ? s.status?.rows : 0));

  const session = detail.data && detail.data.id === id ? detail.data : null;
  const laps = useMemo(
    () => [...(session?.laps ?? [])].sort((a, b) => a.lap_number - b.lap_number),
    [session],
  );
  const stats = useMemo(() => lapStats(laps), [laps]);
  const result = useMemo(
    () => (row && results.data ? matchResult(row, results.data) : null),
    [row, results.data],
  );

  const head = (aside?: ReactNode) => (
    <div className="home-card-head">
      <h2 className="home-card-title">{t("home.last.title")}</h2>
      {aside}
    </div>
  );

  if (!list.data) {
    return (
      <section className="card home-card home-last">
        {head()}
        {list.error ? (
          <p className="form-error" role="alert">
            {t("common.serverError", { msg: list.error })}{" "}
            <button type="button" className="link" onClick={list.reload}>
              {t("common.retry")}
            </button>
          </p>
        ) : (
          <p className="dim">{t("common.loading")}</p>
        )}
      </section>
    );
  }
  if (!row) {
    return (
      <section className="card home-card home-last is-empty">
        {head()}
        <EmptySession />
      </section>
    );
  }

  const duration = sessionDurationS(row.started_wall_ms, row.ended_wall_ms);
  const best = stats.best?.lap_time_s ?? row.best_lap_s;
  return (
    <section className="card home-card home-last">
      {head(
        <span className="home-when dim num">
          {fmtRelativeDay(row.started_wall_ms, lang)} · {fmtDuration(duration)}
        </span>,
      )}
      <div className="home-last-id">
        <div className="home-last-what">
          <p className="home-last-track">
            {row.track_name}
            <SessionTypeTag type={row.session_type} />
            {live && <span className="livetag">{t("sessions.inProgress")}</span>}
          </p>
          <p className="home-last-car dim">
            <span className="is-strong">{row.vehicle_model || row.vehicle_name || "—"}</span>
            {row.class_name && <span> · {row.class_name}</span>}
          </p>
        </div>
        {result && <ResultBadge result={result} />}
      </div>
      <dl className="home-stats">
        <Stat
          label={t("session.kpi.best")}
          tone={best !== null ? "best" : undefined}
          hint={stats.best ? `${t("session.col.lap")} ${lapNo(stats.best.lap_number)}` : undefined}
        >
          {fmtLapTime(best)}
        </Stat>
        <Stat
          label={t("home.last.laps")}
          hint={t("session.kpi.validLapsHint", { n: session ? stats.total : row.lap_count })}
        >
          {session ? stats.counted : "—"}
        </Stat>
        <Stat label={t("session.kpi.consistency")} hint={t("home.last.consistencyHint")}>
          {stats.stddev === null ? (
            "—"
          ) : (
            <>
              ± {stats.stddev.toFixed(3)}
              <small className="home-stat-unit"> s</small>
            </>
          )}
        </Stat>
      </dl>
      {laps.length > 0 ? (
        <LapTimesChart laps={laps} bestLapS={stats.best?.lap_time_s ?? null} height={168} compact />
      ) : (
        <p className="home-last-nolaps dim">
          {session ? t("session.laps.none") : t("common.loading")}
        </p>
      )}
      <div className="home-actions">
        <a className="home-btn is-primary" href={routeHash({ page: "session", id: row.id })}>
          {t("home.last.view")}
        </a>
        <a
          className="home-btn"
          href={routeHash({ page: "engineer", id: row.id })}
          title={t("eng.openHint")}
        >
          <IconEngineer />
          {t("eng.open")}
        </a>
      </div>
    </section>
  );
}

/* ------------------------------------------------------------------ last races */

async function loadRaces(signal: AbortSignal): Promise<{ races: ResultRow[]; profile: Profile }> {
  const profile = await fetchProfile(signal);
  let races = lastRaces(profile.recent);
  // `recent` holds the last ten results of every kind: with practice and qualifying around each
  // race it may not reach five races.
  if (races.length < Math.min(5, profile.totals.races)) {
    races = lastRaces(await fetchResults({ kind: "race", limit: 5 }, signal));
  }
  return { races, profile };
}

function RaceLine({ r }: { r: ResultRow }) {
  const { t, lang } = useT();
  const pos = r.player_class_position;
  const out = r.player_finish === "dnf" || r.player_finish === "dq";
  return (
    <li className="home-race">
      <a className="home-race-link" href={routeHash({ page: "result", id: r.id })}>
        <span
          className={`home-race-pos num${out ? " is-out" : pos !== null && pos <= 3 ? " is-podium" : ""}`}
        >
          {r.player_finish === "dnf" ? (
            t("home.races.dnf")
          ) : r.player_finish === "dq" ? (
            t("home.races.dq")
          ) : pos !== null && pos > 0 ? (
            <>
              <Ordinal n={pos} />
              <small className="home-race-of"> / {r.num_class_drivers}</small>
            </>
          ) : (
            "—"
          )}
        </span>
        <span className="home-race-main">
          <span className="home-race-track">{r.track_course || r.track_venue}</span>
          <span className="home-race-car dim">
            {r.player_vehicle || "—"}
            {r.player_class && ` · ${classLabel(r.player_class)}`}
          </span>
        </span>
        <span className="home-race-side">
          <span className="home-race-best num">{fmtLapTime(r.player_best_lap_s)}</span>
          <span className="home-race-date faint num">
            {fmtRelativeDay(r.started_wall_ms, lang, Date.now(), { time: false })}
          </span>
        </span>
      </a>
    </li>
  );
}

function RacesCard({ minute }: { minute: number }) {
  const { t } = useT();
  const revision = useRecording((s) => s.revision);
  const data = useAsync(loadRaces, [revision, minute]);
  const races = data.data?.races ?? null;
  const imp = data.data?.profile.import ?? null;
  return (
    <section className="card home-card home-races-card">
      <div className="home-card-head">
        <h2 className="home-card-title">{t("home.races.title")}</h2>
        <a className="link" href={routeHash({ page: "profile" })}>
          {t("home.races.all")}
        </a>
      </div>
      {races === null ? (
        data.error ? (
          <p className="form-error" role="alert">
            {t("common.serverError", { msg: data.error })}
          </p>
        ) : (
          <p className="dim">{t("common.loading")}</p>
        )
      ) : races.length === 0 ? (
        <p className="home-card-empty dim">
          {imp?.enabled && !imp.results_dir ? t("home.races.noFolder") : t("home.races.empty")}
        </p>
      ) : (
        <ul className="home-races">
          {races.map((r) => (
            <RaceLine key={r.id} r={r} />
          ))}
        </ul>
      )}
    </section>
  );
}

/* ------------------------------------------------------------------ phone */

function PhoneCard() {
  const { t } = useT();
  const net = useAsync((signal) => fetchNetwork(signal), []);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [chosen, setChosen] = useState(0);
  // Only the PC that runs Corde may change the setting (the server refuses anyone else).
  const canChange = isDesktop() || isLocalPage();

  const set = async (on: boolean) => {
    setBusy(true);
    setError(null);
    try {
      await saveSettings({ lan_access: on });
      net.reload();
    } catch (e) {
      setError(
        e instanceof ApiError && e.status === 403
          ? t("home.phone.remote")
          : t("home.phone.error", { msg: errorMessage(e) }),
      );
    } finally {
      setBusy(false);
    }
  };

  const state = net.data ? phoneState(net.data) : null;
  const restart = (
    <div className="home-restart">
      <Restart
        fallback={<span className="home-restart-how">{t("home.phone.restartManual")}</span>}
      />
    </div>
  );

  let body: ReactNode;
  if (!state) {
    body = net.error ? (
      <p className="form-error" role="alert">
        {t("common.serverError", { msg: net.error })}
      </p>
    ) : (
      <p className="dim">{t("common.loading")}</p>
    );
  } else if (state.kind === "on") {
    const url = state.urls[Math.min(chosen, state.urls.length - 1)] ?? "";
    const others = state.urls.filter((u) => u !== url);
    body = (
      <div className="home-phone-on">
        <Qr value={url} label={t("home.phone.qrLabel", { url })} />
        <div className="home-phone-text">
          <p className="home-url num">
            <span className="home-url-scheme">http://</span>
            {bareAddress(url)}
          </p>
          <p className="dim">{t("home.phone.on")}</p>
          {others.length > 0 && (
            <p className="home-phone-others dim">
              {t("home.phone.other")}{" "}
              {others.map((u) => (
                <button
                  key={u}
                  type="button"
                  className="link num"
                  onClick={() => setChosen(state.urls.indexOf(u))}
                >
                  {bareAddress(u)}
                </button>
              ))}
            </p>
          )}
          {state.stopping ? (
            <div className="home-notice">
              <p className="home-notice-title">{t("home.phone.restartTitle")}</p>
              <p>{t("home.phone.restartOff")}</p>
              {restart}
              {canChange && (
                <button
                  type="button"
                  className="link"
                  onClick={() => void set(true)}
                  disabled={busy}
                >
                  {t("home.phone.keep")}
                </button>
              )}
            </div>
          ) : (
            canChange && (
              <button
                type="button"
                className="link"
                onClick={() => void set(false)}
                disabled={busy}
              >
                {t("home.phone.disable")}
              </button>
            )
          )}
        </div>
      </div>
    );
  } else if (state.kind === "no-address") {
    body = (
      <>
        <p className="dim home-phone-lead">{t("home.phone.noAddress")}</p>
        {canChange && !state.stopping && (
          <button type="button" className="link" onClick={() => void set(false)} disabled={busy}>
            {t("home.phone.disable")}
          </button>
        )}
      </>
    );
  } else if (state.kind === "restart") {
    body = (
      <div className="home-notice">
        <p className="home-notice-title">{t("home.phone.restartTitle")}</p>
        <p>{t("home.phone.restartOn")}</p>
        <p className="dim">{t("home.phone.firewall")}</p>
        {restart}
        {canChange && (
          <button type="button" className="link" onClick={() => void set(false)} disabled={busy}>
            {t("home.phone.cancel")}
          </button>
        )}
      </div>
    );
  } else {
    body = (
      <>
        <p className="dim home-phone-lead">{t("home.phone.off")}</p>
        {canChange ? (
          <button
            type="button"
            className="home-btn home-phone-enable"
            onClick={() => void set(true)}
            disabled={busy}
          >
            {t("home.phone.enable")}
          </button>
        ) : (
          <p className="faint">{t("home.phone.remote")}</p>
        )}
      </>
    );
  }

  return (
    <section className="card home-card home-phone">
      <div className="home-card-head">
        <h2 className="home-card-title">{t("home.phone.title")}</h2>
      </div>
      <div className="home-phone-body">
        {body}
        {error && (
          <p className="form-error" role="alert">
            {error}
          </p>
        )}
      </div>
    </section>
  );
}

/* ------------------------------------------------------------------ foot */

function HomeFoot({ online, onGuide }: { online: boolean; onGuide: () => void }) {
  const { t, nOf, lang } = useT();
  const version = useTelemetry((s) => s.hello?.serverVersion ?? null);
  const revision = useRecording((s) => s.revision);
  const storage = useAsync(
    (signal) => (online ? fetchStorage(signal) : Promise.resolve(null)),
    [online, revision],
  );
  const st = online ? storage.data : null;
  return (
    <footer className="page-foot home-foot dim">
      <span className="is-strong">
        {version ? t("home.foot.version", { v: version }) : t("app.name")}
      </span>
      {st && (
        <>
          <span className="home-foot-data">
            <span>{t("home.foot.data")}</span>{" "}
            <span className="faint page-foot-path" title={st.path}>
              {st.path}
            </span>
          </span>
          <span className="num">
            {nOf(st.sessions, "word.session", "word.sessions")} · {fmtBytes(st.total_bytes, lang)}
          </span>
        </>
      )}
      <button type="button" className="link home-foot-guide" onClick={onGuide}>
        {t("home.foot.guide")}
      </button>
    </footer>
  );
}

/* ------------------------------------------------------------------ page */

/**
 * The home page (`#/home`, shown when the app opens): where things stand, the last recorded
 * session, the last races, and how to get the dashboard on a phone.
 */
export function HomePage({ onSettings }: { onSettings: () => void }) {
  const { t } = useT();
  // REST is only worth trying while a server answers on the socket.
  const online = useTelemetry((s) => s.socket === "open");
  const minute = useMinuteTick();
  const [guide, setGuide] = useState(false);

  return (
    <div className="home-page">
      <h1 className="sr-only">{t("home.title")}</h1>
      <StatusCard onSettings={onSettings} />
      {online && (
        <div className="home-grid">
          <LastSessionCard minute={minute} />
          <RacesCard minute={minute} />
          <PhoneCard />
        </div>
      )}
      <HomeFoot online={online} onGuide={() => setGuide(true)} />
      <GuideDialog open={guide} onClose={() => setGuide(false)} />
    </div>
  );
}
