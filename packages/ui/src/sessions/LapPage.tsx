import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type uPlot from "uplot";
import { IconDownload, IconPlus, IconZoomOut } from "../dashboard/icons";
import { useT } from "../i18n";
import { fmtLapTime } from "../lib/format";
import { lapNo } from "../lib/laps";
import { navigate } from "../nav/routes";
import { fetchLapSamples, fetchSession, type LapRow, type LapSamples } from "../net/sessions-api";
import { ChannelPicker } from "./ChannelPicker";
import { ChartPanel, type XUnit } from "./ChartPanel";
import { displayFactor, isStepChannel } from "./channels";
import { buildCsv, csvFileName, downloadText } from "./lib/csv";
import { deltaTime, lapClock, monotonicIndices, pick, resample } from "./lib/delta";
import { buildPanel, DEFAULT_PANELS, type LapView, type PanelDef, panelChannels } from "./panels";
import { errorMessage, Notice, PageHeader, useAsync } from "./shared";

const COARSE_POINTS = 2500;
const FINE_POINTS = 6000;
const SYNC_KEY = "corde-lap";

interface LapData {
  lap: number;
  totalRows: number;
  points: number;
  level: number;
  t: number[];
  dist: number[];
  channels: Map<string, (number | null)[]>;
  units: Map<string, string>;
}

function toLapData(s: LapSamples, level: number): LapData {
  const channels = new Map<string, (number | null)[]>();
  const units = new Map<string, string>();
  for (const c of s.channels) {
    channels.set(c.name, c.values);
    units.set(c.name, c.unit);
  }
  return {
    lap: s.lap_number,
    totalRows: s.total_rows,
    points: s.points,
    level,
    t: s.t_s,
    dist: s.lap_dist_m,
    channels,
    units,
  };
}

/** Loads (and re-loads when channels or resolution change) the samples of one or two laps. */
function useLapSamples(sessionId: string, laps: number[], channels: string[], points: number) {
  const [data, setData] = useState<Map<number, LapData>>(() => new Map());
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const dataRef = useRef(data);
  dataRef.current = data;
  const sessionRef = useRef(sessionId);
  const lapsKey = laps.join(",");
  const chKey = [...channels].sort().join(",");
  const [tick, setTick] = useState(0);

  // biome-ignore lint/correctness/useExhaustiveDependencies: keys stand for the arrays
  useEffect(() => {
    if (sessionRef.current !== sessionId) {
      // another session: forget everything loaded so far
      sessionRef.current = sessionId;
      dataRef.current = new Map();
      setData(new Map());
    }
    const need = laps.filter((n) => {
      const d = dataRef.current.get(n);
      return !d || d.level !== points || channels.some((c) => !d.channels.has(c));
    });
    if (need.length === 0 || channels.length === 0) {
      setLoading(false);
      return;
    }
    const ctrl = new AbortController();
    setLoading(true);
    Promise.all(need.map((n) => fetchLapSamples(sessionId, n, channels, points, ctrl.signal)))
      .then((results) => {
        if (ctrl.signal.aborted) return;
        setData((prev) => {
          const next = new Map(prev);
          for (const r of results) next.set(r.lap_number, toLapData(r, points));
          return next;
        });
        setError(null);
      })
      .catch((e: unknown) => {
        if (!ctrl.signal.aborted) setError(errorMessage(e));
      })
      .finally(() => {
        if (!ctrl.signal.aborted) setLoading(false);
      });
    return () => ctrl.abort();
  }, [sessionId, lapsKey, chKey, points, tick]);

  const retry = useCallback(() => setTick((n) => n + 1), []);
  return { data, loading, error, retry };
}

function cachedGetter(compute: (channel: string) => (number | null)[] | undefined) {
  const cache = new Map<string, (number | null)[] | undefined>();
  return (channel: string) => {
    if (!cache.has(channel)) cache.set(channel, compute(channel));
    return cache.get(channel);
  };
}

function scaled(values: (number | null)[], channel: string): (number | null)[] {
  const f = displayFactor(channel);
  return f === 1 ? values : values.map((v) => (v === null ? null : v * f));
}

interface Props {
  id: string;
  a: number;
  b: number | null;
}

export function LapPage({ id, a, b }: Props) {
  const { t, lang } = useT();
  const session = useAsync((signal) => fetchSession(id, signal), [id]);
  const [axis, setAxis] = useState<"dist" | "time">("dist");
  const [panels, setPanels] = useState<PanelDef[]>(DEFAULT_PANELS);
  const [fine, setFine] = useState(false);
  const [pickerOpen, setPickerOpen] = useState(false);
  const plotsRef = useRef(new Set<uPlot>());
  const customSeq = useRef(0);

  const lapNumbers = useMemo(() => (b === null ? [a] : [a, b]), [a, b]);
  const channels = useMemo(() => [...new Set(panels.flatMap(panelChannels))], [panels]);
  const points = fine ? FINE_POINTS : COARSE_POINTS;
  const samples = useLapSamples(id, lapNumbers, channels, points);

  // reset the resolution when the laps change
  // biome-ignore lint/correctness/useExhaustiveDependencies: intentional reset on lap change
  useEffect(() => setFine(false), [a, b]);

  const lapsById = useMemo(() => {
    const m = new Map<number, LapRow>();
    for (const l of session.data?.laps ?? []) m.set(l.lap_number, l);
    return m;
  }, [session.data]);

  /* ---------------------------------------------------------------- derived */

  const prepared = useMemo(() => {
    const A = samples.data.get(a);
    if (!A) return null;
    const clockA = lapClock(A.dist, A.t);
    const axisA = axis === "dist" ? A.dist : clockA;
    const monoA = monotonicIndices(axisA);
    const x = pick(axisA, monoA);
    const views: LapView[] = [
      {
        label: "A",
        lapNumber: a,
        get: cachedGetter((ch) => {
          const raw = A.channels.get(ch);
          return raw ? scaled(pick(raw, monoA), ch) : undefined;
        }),
      },
    ];
    let delta: (number | null)[] | null = null;
    let totalRows = A.totalRows;
    if (b !== null) {
      const B = samples.data.get(b);
      if (B) {
        totalRows = Math.max(totalRows, B.totalRows);
        const clockB = lapClock(B.dist, B.t);
        const axisB = axis === "dist" ? B.dist : clockB;
        const monoB = monotonicIndices(axisB);
        const xB = pick(axisB, monoB);
        views.push({
          label: "B",
          lapNumber: b,
          get: cachedGetter((ch) => {
            const raw = B.channels.get(ch);
            if (!raw) return undefined;
            const mode = isStepChannel(ch) ? "step" : "linear";
            return scaled(resample(xB, pick(raw, monoB), x, mode), ch);
          }),
        });
        const monoBd = monotonicIndices(B.dist);
        delta = deltaTime(
          { dist: pick(A.dist, monoA), t: pick(clockA, monoA) },
          { dist: pick(B.dist, monoBd), t: pick(clockB, monoBd) },
        );
      }
    }
    return { x, views, delta, totalRows, points: A.points };
  }, [samples.data, a, b, axis]);

  const built = useMemo(() => {
    if (!prepared) return [];
    const defs: PanelDef[] =
      b !== null && prepared.delta ? [{ id: "delta", kind: "delta" }, ...panels] : panels;
    return defs
      .map((def) => ({ def, panel: buildPanel(def, prepared.views, lang, t, prepared.delta) }))
      .filter((p) => p.panel !== null && p.panel.series.length > 0);
  }, [prepared, panels, b, lang, t]);

  /* ---------------------------------------------------------------- actions */

  const register = useCallback((plot: uPlot, on: boolean) => {
    if (on) plotsRef.current.add(plot);
    else plotsRef.current.delete(plot);
  }, []);

  const onZoom = useCallback(
    (range: [number, number] | null) => {
      if (range && !fine && prepared && prepared.totalRows > prepared.points * 1.3) setFine(true);
    },
    [fine, prepared],
  );

  const resetZoom = () => {
    if (!prepared || prepared.x.length < 2) return;
    const min = prepared.x[0] as number;
    const max = prepared.x[prepared.x.length - 1] as number;
    for (const u of plotsRef.current) u.setScale("x", { min, max });
  };

  const addCustom = (chans: string[]) => {
    customSeq.current += 1;
    setPanels((cur) => [
      ...cur,
      { id: `custom-${customSeq.current}`, kind: "custom", channels: chans },
    ]);
  };

  const exportCsv = (lap: number) => {
    const d = samples.data.get(lap);
    if (!d) return;
    const columns = [
      { name: "lap_dist_m", unit: "m", values: d.dist },
      { name: "lap_time_s", unit: "s", values: lapClock(d.dist, d.t) },
      { name: "session_time_s", unit: "s", values: d.t },
      ...[...d.channels.entries()].map(([name, values]) => ({
        name,
        unit: d.units.get(name) ?? "",
        values,
      })),
    ];
    downloadText(csvFileName(id, lap, lang), buildCsv(columns));
  };

  const setLap = (which: "a" | "b", value: string) => {
    const n = value === "" ? null : Number(value);
    if (which === "a") {
      if (n === null) return;
      if (b !== null && n === b) navigate({ page: "lap", id, lap: n });
      else if (b !== null) navigate({ page: "compare", id, a: n, b });
      else navigate({ page: "lap", id, lap: n });
    } else if (n === null || n === a) navigate({ page: "lap", id, lap: a });
    else navigate({ page: "compare", id, a, b: n });
  };

  /* ---------------------------------------------------------------- render */

  const allLaps = useMemo(
    () => [...(session.data?.laps ?? [])].sort((x, y) => x.lap_number - y.lap_number),
    [session.data],
  );
  const lapA = lapsById.get(a);
  const lapB = b === null ? undefined : lapsById.get(b);
  const title =
    b === null
      ? t("lap.title", { n: lapNo(a) })
      : t("lap.compareTitle", { a: lapNo(a), b: lapNo(b) });
  const xUnit: XUnit = axis === "dist" ? "m" : "s";

  const lapOption = (l: LapRow) => (
    <option key={l.lap_number} value={l.lap_number}>
      {t("session.col.lap")} {lapNo(l.lap_number)} · {fmtLapTime(l.lap_time_s)}
      {l.valid ? "" : ` (${t("session.lap.invalid").toLowerCase()})`}
    </option>
  );

  const status =
    samples.loading && prepared ? (
      <span className="lap-status dim">
        <span className="spinner is-small" /> {t("lap.loadingFine")}
      </span>
    ) : null;

  const toolbar = (
    <div className="lap-toolbar">
      <div className="lap-picks">
        <label className="lap-pick">
          <span className="lap-pick-tag is-a">{t("lap.lapA")}</span>
          <select className="select" value={a} onChange={(e) => setLap("a", e.target.value)}>
            {allLaps.length === 0 && (
              <option value={a}>
                {t("session.col.lap")} {lapNo(a)}
              </option>
            )}
            {allLaps.map(lapOption)}
          </select>
        </label>
        <label className="lap-pick">
          <span className="lap-pick-tag is-b">{t("lap.lapB")}</span>
          <select
            className="select"
            value={b ?? ""}
            onChange={(e) => setLap("b", e.target.value)}
            disabled={allLaps.length < 2}
          >
            <option value="">{allLaps.length < 2 ? t("lap.noCompare") : t("lap.pickOther")}</option>
            {allLaps.filter((l) => l.lap_number !== a).map(lapOption)}
          </select>
        </label>
      </div>
      <div className="lap-tools">
        <fieldset className="seg" aria-label={t("lap.axis")}>
          <button
            type="button"
            aria-pressed={axis === "dist"}
            className={`seg-btn${axis === "dist" ? " is-on" : ""}`}
            onClick={() => setAxis("dist")}
          >
            {t("lap.xDistance")}
          </button>
          <button
            type="button"
            aria-pressed={axis === "time"}
            className={`seg-btn${axis === "time" ? " is-on" : ""}`}
            onClick={() => setAxis("time")}
          >
            {t("lap.xTime")}
          </button>
        </fieldset>
        <button type="button" className="btn" onClick={resetZoom} title={t("lap.resetZoom")}>
          <IconZoomOut />
          <span className="btn-label">{t("lap.resetZoom")}</span>
        </button>
        <button type="button" className="btn" onClick={() => setPickerOpen(true)}>
          <IconPlus />
          <span className="btn-label">{t("lap.addPanel")}</span>
        </button>
        {lapNumbers.map((n) => (
          <button
            key={n}
            type="button"
            className="btn"
            onClick={() => exportCsv(n)}
            disabled={!samples.data.get(n)}
            title={t("lap.exportCsv")}
          >
            <IconDownload />
            <span className="btn-label">
              {t("lap.exportCsv")}
              {b !== null ? ` ${n === a ? "A" : "B"}` : ""}
            </span>
          </button>
        ))}
        {status}
      </div>
    </div>
  );

  return (
    <div className="lap-page">
      <PageHeader
        back={{ page: "session", id }}
        backLabel={session.data?.track_name ?? t("common.back")}
        title={title}
        subtitle={
          <span className="lap-subtitle">
            {lapA && (
              <span className="lap-chip">
                <b className="lap-pick-tag is-a">A</b> {t("session.col.lap")} {lapNo(a)} ·{" "}
                <span className="num">{fmtLapTime(lapA.lap_time_s)}</span>
              </span>
            )}
            {lapB && b !== null && (
              <span className="lap-chip">
                <b className="lap-pick-tag is-b">B</b> {t("session.col.lap")} {lapNo(b)} ·{" "}
                <span className="num">{fmtLapTime(lapB.lap_time_s)}</span>
              </span>
            )}
            {session.data && (
              <span className="dim">
                {session.data.track_name} ·{" "}
                {session.data.vehicle_model || session.data.vehicle_name}
              </span>
            )}
          </span>
        }
      />
      {toolbar}

      {samples.error && !prepared ? (
        <Notice
          kind="error"
          title={t("lap.error", { msg: samples.error })}
          action={
            <button type="button" className="btn" onClick={samples.retry}>
              {t("common.retry")}
            </button>
          }
        />
      ) : !prepared ? (
        <Notice kind="loading">{t("lap.loading")}</Notice>
      ) : (
        <div className="panels">
          {samples.error && <Notice kind="error" title={t("lap.error", { msg: samples.error })} />}
          {built.map(({ def, panel }, i) =>
            panel ? (
              <ChartPanel
                key={def.id}
                title={panel.title}
                subtitle={panel.subtitle}
                x={prepared.x}
                xUnit={xUnit}
                series={panel.series}
                scales={panel.scales}
                height={panel.height}
                syncKey={SYNC_KEY}
                showXAxis={i === built.length - 1}
                onZoom={onZoom}
                register={register}
                onRemove={
                  def.kind === "delta"
                    ? undefined
                    : () => setPanels((cur) => cur.filter((p) => p.id !== def.id))
                }
                controls={
                  def.kind === "tyreTemp" ? (
                    <select
                      className="select cp-select"
                      value={def.zones ? "zones" : "centre"}
                      aria-label={t("lap.tyreMode")}
                      onChange={(e) =>
                        setPanels((cur) =>
                          cur.map((p) =>
                            p.id === def.id ? { ...p, zones: e.target.value === "zones" } : p,
                          ),
                        )
                      }
                    >
                      <option value="centre">{t("lap.tyreCentre")}</option>
                      <option value="zones">{t("lap.tyreZones")}</option>
                    </select>
                  ) : undefined
                }
              />
            ) : null,
          )}
          <div className="lap-foot faint">
            {t("lap.points", { n: prepared.x.length })}
            {" · "}
            {t("lap.zoomHint")}
          </div>
        </div>
      )}

      <ChannelPicker open={pickerOpen} onClose={() => setPickerOpen(false)} onAdd={addCustom} />
    </div>
  );
}
