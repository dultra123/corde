import type {
  BalanceAnalysis,
  BalanceCell,
  BrakeAnalysis,
  CornerPhase,
  CornerSpeed,
  DamperHist,
  DrivingStyle,
  Finding,
  LapStats,
  Severity,
  SuspensionAnalysis,
  TyreAnalysis,
  TyreWheelAnalysis,
} from "@corde/protocol";
import { type ReactNode, useMemo, useState } from "react";
import { type TranslationKey, useT } from "../i18n";
import { BRAKE_SCALE, tempColor, tyreScale } from "../lib/color";
import { fmtLapTime, fmtNum } from "../lib/format";
import { lapNo } from "../lib/laps";
import { WHEEL_LABELS } from "../sessions/channels";
import {
  BALANCE_SIGMAS_FULL_SCALE,
  BRAKE_WINDOW_COLOR,
  balanceColor,
  balanceInk,
  balanceWord,
  CAMBER_KEYS,
  fmtEvidence,
  fmtIndex,
  fmtSigmas,
  fmtSigned,
  isRelativeBalance,
  SEVERITY_RANK,
  STYLE_KEYS,
  sortFindings,
  styleValue,
} from "./lib";

const WHEEL_IDS = ["fl", "fr", "rl", "rr"] as const;
const PHASES: CornerPhase[] = ["entry", "mid", "exit"];
const SPEEDS: CornerSpeed[] = ["slow", "medium", "fast"];

/* ------------------------------------------------------------------ chrome */

function Card({
  title,
  hint,
  actions,
  className,
  children,
}: {
  title: string;
  hint?: string;
  actions?: ReactNode;
  className?: string;
  children: ReactNode;
}) {
  return (
    <section className={`card eng-card${className ? ` ${className}` : ""}`}>
      <div className="card-head">
        <h2>{title}</h2>
        {actions && <div className="card-actions">{actions}</div>}
      </div>
      {hint && <p className="eng-card-hint faint">{hint}</p>}
      {children}
    </section>
  );
}

/** Label / value pair used by the small figures under the main visual of a card. */
function Fig({
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
    <div className="eng-fig">
      <div className="eng-fig-label dim">{label}</div>
      <div className="eng-fig-value num" style={{ color }}>
        {value}
        {unit && <span className="eng-fig-unit"> {unit}</span>}
      </div>
      {hint && <div className="eng-fig-hint faint">{hint}</div>}
    </div>
  );
}

/* ------------------------------------------------------------------ driving style */

export function StyleSection({ style }: { style: DrivingStyle }) {
  const { t } = useT();
  const [open, setOpen] = useState<string | null>(null);
  return (
    <Card title={t("eng.style.title")}>
      <ul className="stylebars">
        {STYLE_KEYS.map((key) => {
          const v = styleValue(style, key);
          const hint = t(`eng.style.${key}.hint` as TranslationKey);
          const isOpen = open === key;
          return (
            <li key={key} className="stylebar">
              <button
                type="button"
                className="stylebar-row"
                title={hint}
                aria-expanded={isOpen}
                onClick={() => setOpen(isOpen ? null : key)}
              >
                <span className="stylebar-label">{t(`eng.style.${key}` as TranslationKey)}</span>
                <span className="stylebar-track" aria-hidden="true">
                  <span className="stylebar-fill" style={{ width: `${v}%` }} />
                </span>
                <span className="stylebar-value num">{v.toFixed(0)}</span>
              </button>
              {isOpen && <p className="stylebar-hint dim">{hint}</p>}
            </li>
          );
        })}
      </ul>
      <div className="eng-figs is-3">
        <Fig label={t("eng.style.lockups")} value={fmtNum(style.lockup_rate, 1)} />
        <Fig label={t("eng.style.wheelspin")} value={fmtNum(style.wheelspin_rate, 1)} />
        <Fig
          label={t("eng.style.coasting")}
          value={fmtNum(style.coasting_pct, 1)}
          unit="%"
          hint={t("eng.style.coastingHint")}
        />
      </div>
      {style.labels_fr.length > 0 && (
        <div className="echips">
          {style.labels_fr.map((l) => (
            <span key={l} className="echip is-accent">
              {l}
            </span>
          ))}
        </div>
      )}
    </Card>
  );
}

/* ------------------------------------------------------------------ laps */

export function LapsSection({ stats }: { stats: LapStats }) {
  const { t } = useT();
  const degr = stats.degradation_s_per_lap;
  return (
    <Card title={t("eng.laps.title")}>
      <div className="eng-figs is-3 is-big">
        <Fig label={t("eng.laps.best")} value={fmtLapTime(stats.best_s)} color="var(--best)" />
        <Fig label={t("eng.laps.mean")} value={fmtLapTime(stats.mean_valid_s)} />
        <Fig
          label={t("eng.laps.consistency")}
          value={fmtNum(stats.consistency_score, 0)}
          unit="/ 100"
          hint={
            stats.std_valid_s === null
              ? undefined
              : t("eng.laps.consistencyHint", { sd: stats.std_valid_s.toFixed(3) })
          }
        />
        <Fig
          label={t("eng.laps.degradation")}
          value={degr === null ? "—" : fmtSigned(degr, 3)}
          unit={degr === null ? undefined : "s"}
          hint={t("eng.laps.degradationHint")}
          color={degr !== null && degr > 0.08 ? "var(--warm)" : undefined}
        />
        <Fig
          label={t("eng.laps.fuel")}
          value={fmtNum(stats.fuel_per_lap_l, 2)}
          unit={stats.fuel_per_lap_l === null ? undefined : "L"}
        />
        <Fig label={t("eng.laps.topSpeed")} value={fmtNum(stats.top_speed_kmh, 0)} unit="km/h" />
      </div>
    </Card>
  );
}

/* ------------------------------------------------------------------ tyres */

function TyreTile({ w, label }: { w: TyreWheelAnalysis; label: string }) {
  const { t } = useT();
  const scale = tyreScale(w.optimal_c);
  const zones: [TranslationKey, number][] = [
    ["eng.tyres.inner", w.inner_c],
    ["eng.tyres.middle", w.middle_c],
    ["eng.tyres.outer", w.outer_c],
  ];
  const camber = t(CAMBER_KEYS[w.camber_hint] as TranslationKey);
  return (
    <div className="etyre">
      <div className="etyre-head">
        <span className="etyre-name">{label}</span>
        <span className={`etyre-camber is-${w.camber_hint}`}>{camber}</span>
      </div>
      <div className="etyre-zones" title={`I−O ${fmtSigned(w.imo_spread_c, 1, " °C")}`}>
        {zones.map(([k, v]) => (
          <div key={k} className="etyre-zone" style={{ background: tempColor(v, scale) }}>
            <span className="etyre-zone-key">{t(k)}</span>
            <span className="etyre-zone-val num">{fmtNum(v, 0)}°</span>
          </div>
        ))}
      </div>
      <dl className="etyre-facts">
        <div>
          <dt>{t("eng.tyres.carcass")}</dt>
          <dd className="num">
            {fmtNum(w.avg_carcass_c, 0)}°
            {w.delta_to_optimal_c !== null && (
              <small className="faint">
                {" "}
                {t("eng.tyres.toOptimal", { d: fmtSigned(w.delta_to_optimal_c, 0) })}
              </small>
            )}
          </dd>
        </div>
        <div>
          <dt>{t("eng.tyres.pressure")}</dt>
          <dd className="num">
            {fmtNum(w.avg_pressure_kpa, 0)} kPa
            <small className="faint">
              {" "}
              {t("eng.tyres.drift", { d: fmtSigned(w.pressure_drift_kpa, 1) })}
            </small>
          </dd>
        </div>
        <div>
          <dt>{t("eng.tyres.wear")}</dt>
          <dd className="num">
            {fmtNum(w.wear_per_lap_pct, 2)} %
            <small className="faint"> {t("eng.tyres.perLap")}</small>
          </dd>
        </div>
      </dl>
    </div>
  );
}

export function TyresSection({ tyres }: { tyres: TyreAnalysis }) {
  const { t, lang } = useT();
  return (
    <Card title={t("eng.tyres.title")} hint={t("eng.tyres.hint")}>
      <div className="car-grid">
        {tyres.per_wheel.map((w, i) => (
          <TyreTile key={WHEEL_IDS[i]} w={w} label={WHEEL_LABELS[lang][i] ?? ""} />
        ))}
      </div>
    </Card>
  );
}

/* ------------------------------------------------------------------ brakes */

export function BrakesSection({ brakes }: { brakes: BrakeAnalysis }) {
  const { t, lang } = useT();
  const front = brakes.lockup_front_vs_rear;
  return (
    <Card title={t("eng.brakes.title")}>
      <div className="car-grid is-brakes">
        {brakes.per_wheel.map((b, i) => (
          <div key={WHEEL_IDS[i]} className="ebrake">
            <div className="ebrake-head">
              <span className="etyre-name">{WHEEL_LABELS[lang][i]}</span>
              <span
                className="ebrake-window"
                style={{ color: BRAKE_WINDOW_COLOR[b.window] }}
                title={t(`eng.brakes.window.${b.window}` as TranslationKey)}
              >
                <span className="ebrake-window-dot" />
                {t(`eng.brakes.window.${b.window}` as TranslationKey)}
              </span>
            </div>
            <div className="ebrake-temps">
              <span className="ebrake-temp">
                <span className="dim">{t("eng.brakes.avg")}</span>
                <b className="num" style={{ color: tempColor(b.avg_c, BRAKE_SCALE) }}>
                  {fmtNum(b.avg_c, 0)}°
                </b>
              </span>
              {b.typical_peak_c > 0 && (
                <span className="ebrake-temp" title={t("eng.brakes.typicalHint")}>
                  <span className="dim">{t("eng.brakes.typical")}</span>
                  <b className="num" style={{ color: tempColor(b.typical_peak_c, BRAKE_SCALE) }}>
                    {fmtNum(b.typical_peak_c, 0)}°
                  </b>
                </span>
              )}
              <span className="ebrake-temp">
                <span className="dim">{t("eng.brakes.max")}</span>
                <b className="num" style={{ color: tempColor(b.max_c, BRAKE_SCALE) }}>
                  {fmtNum(b.max_c, 0)}°
                </b>
              </span>
            </div>
            <div className="ebrake-bar" aria-hidden="true">
              <span
                className="ebrake-bar-fill"
                style={{
                  width: `${Math.min(100, Math.max(2, (b.max_c / 900) * 100))}%`,
                  background: tempColor(b.max_c, BRAKE_SCALE),
                }}
              />
              <span
                className="ebrake-bar-avg"
                style={{ left: `${Math.min(100, Math.max(0, (b.avg_c / 900) * 100))}%` }}
              />
            </div>
          </div>
        ))}
      </div>
      <div className="eng-figs is-2 is-foot">
        <Fig
          label={t("eng.brakes.bias")}
          value={
            brakes.bias_rear_avg === null
              ? "—"
              : t("eng.brakes.biasValue", { front: ((1 - brakes.bias_rear_avg) * 100).toFixed(1) })
          }
        />
        <Fig
          label={t("eng.brakes.lockups")}
          value={
            front === null
              ? t("eng.brakes.noLockups")
              : t("eng.brakes.lockupsValue", {
                  front: (front * 100).toFixed(0),
                  rear: ((1 - front) * 100).toFixed(0),
                })
          }
        />
      </div>
    </Card>
  );
}

/* ------------------------------------------------------------------ balance */

function BalanceCellView({ cell, relative }: { cell: BalanceCell; relative: boolean }) {
  const { t } = useT();
  const empty = cell.samples <= 0;
  const word = balanceWord(cell.understeer_index, cell.samples);
  const wordKey = (
    relative ? `eng.balance.rel.${word ?? "neutral"}` : `eng.balance.${word ?? "neutral"}`
  ) as TranslationKey;
  const parts = [
    t(wordKey),
    ...(relative
      ? [
          fmtSigmas(cell.understeer_index),
          t("eng.balance.rawIndex", { v: fmtIndex(cell.raw_index) }),
        ]
      : []),
    t("eng.balance.samples", { n: cell.samples }),
    t("eng.balance.confidence", { pct: (cell.confidence * 100).toFixed(0) }),
  ];
  return (
    <td
      className={`bal-cell${empty ? " is-empty" : ""}`}
      style={{ background: balanceColor(cell.understeer_index, cell.confidence, cell.samples) }}
      title={empty ? t("eng.balance.empty") : parts.join(" · ")}
    >
      {empty ? (
        <span className="bal-empty faint">—</span>
      ) : (
        <>
          <span
            className="bal-index num"
            style={{ color: balanceInk(cell.understeer_index, cell.samples) }}
          >
            {fmtIndex(cell.understeer_index)}
          </span>
          <span className="bal-sub faint num">
            <span className="bal-sub-n">{t("eng.balance.samples", { n: cell.samples })}</span>
            <span className="bal-sub-sep"> · </span>
            <span className="bal-sub-c">
              {t("eng.balance.confidence", { pct: (cell.confidence * 100).toFixed(0) })}
            </span>
          </span>
        </>
      )}
    </td>
  );
}

export function BalanceSection({ balance }: { balance: BalanceAnalysis }) {
  const { t } = useT();
  const corners = balance.corners;
  const relative = isRelativeBalance(balance);
  const ref = balance.reference;
  return (
    <Card
      title={t("eng.balance.title")}
      hint={t(relative ? "eng.balance.hintRelative" : "eng.balance.hint")}
    >
      {ref ? (
        <p className="bal-ref faint">
          {t("eng.balance.reference", {
            raw: fmtIndex(ref.raw_index),
            passes: ref.passes,
            trigger: fmtIndex(ref.trigger_index).replace("+", "±"),
            sigmas: (ref.trigger_index * BALANCE_SIGMAS_FULL_SCALE).toFixed(1),
          })}
        </p>
      ) : (
        <p className="bal-ref faint">{t("eng.balance.absoluteOld")}</p>
      )}
      <div className="tbl-wrap bal-wrap">
        <table className="bal">
          <thead>
            <tr>
              <th />
              {SPEEDS.map((s) => (
                <th key={s}>{t(`eng.balance.speed.${s}` as TranslationKey)}</th>
              ))}
            </tr>
          </thead>
          <tbody>
            {PHASES.map((p) => (
              <tr key={p}>
                <th scope="row" className="bal-rowhead">
                  {t(`eng.balance.phase.${p}` as TranslationKey)}
                </th>
                {SPEEDS.map((s) => (
                  <BalanceCellView key={s} cell={balance.per_phase[p][s]} relative={relative} />
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <div className="bal-legend faint num" aria-hidden="true">
        <span style={{ color: "var(--cold)" }}>
          −1 {t(relative ? "eng.balance.rel.oversteer" : "eng.balance.oversteer")}
        </span>
        <span className="bal-legend-bar" />
        <span style={{ color: "var(--hot)" }}>
          +1 {t(relative ? "eng.balance.rel.understeer" : "eng.balance.understeer")}
        </span>
      </div>
      {balance.overall_fr && <p className="eng-overall">{balance.overall_fr}</p>}
      <details className="eng-details">
        <summary>
          {corners.length === 1
            ? t("eng.corners.titleOne")
            : t("eng.corners.title", { n: corners.length })}
        </summary>
        {corners.length === 0 ? (
          <p className="faint">{t("eng.corners.none")}</p>
        ) : (
          <div className="tbl-wrap">
            <table className="tbl corners-tbl">
              <thead>
                <tr>
                  <th className="is-num">{t("eng.corners.col.n")}</th>
                  <th className="is-num">{t("eng.corners.col.dist")}</th>
                  <th className="is-num">{t("eng.corners.col.vmin")}</th>
                  <th>{t("eng.corners.col.dir")}</th>
                  <th className="is-num is-optional">{t("eng.corners.col.passes")}</th>
                  <th className="is-num">{t("eng.balance.phase.entry")}</th>
                  <th className="is-num">{t("eng.balance.phase.mid")}</th>
                  <th className="is-num">{t("eng.balance.phase.exit")}</th>
                </tr>
              </thead>
              <tbody>
                {corners.map((c) => (
                  <tr key={c.index}>
                    <td className="num is-num is-strong">{c.index}</td>
                    <td className="num is-num">{fmtNum(c.apex_dist_m, 0)} m</td>
                    <td className="num is-num">
                      {fmtNum(c.min_speed_kmh, 0)}
                      <small className="faint">
                        {" "}
                        {c.speed_class
                          ? t(`eng.balance.speed.${c.speed_class}` as TranslationKey)
                          : ""}
                      </small>
                    </td>
                    <td>
                      <span className="corner-dir">
                        {c.direction === "left" ? "↰" : c.direction === "right" ? "↱" : "·"}{" "}
                        <span className="dim">
                          {c.direction === "left" || c.direction === "right"
                            ? t(`eng.corners.${c.direction}`)
                            : "—"}
                        </span>
                      </span>
                    </td>
                    <td className="num is-num is-optional">{c.passes}</td>
                    {[c.balance_entry, c.balance_mid, c.balance_exit].map((v, i) => (
                      <td
                        key={i}
                        className="num is-num"
                        style={{ color: v === null ? "var(--text-faint)" : balanceInk(v) }}
                      >
                        {v === null ? "—" : fmtIndex(v)}
                      </td>
                    ))}
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </details>
    </Card>
  );
}

/* ------------------------------------------------------------------ suspension */

const HIST_KEYS: [keyof DamperHist, TranslationKey][] = [
  ["slow_bump_pct", "eng.susp.slowBump"],
  ["slow_rebound_pct", "eng.susp.slowRebound"],
  ["fast_bump_pct", "eng.susp.fastBump"],
  ["fast_rebound_pct", "eng.susp.fastRebound"],
];

export function SuspensionSection({ suspension }: { suspension: SuspensionAnalysis }) {
  const { t, lang } = useT();
  const s = suspension;
  const maxPct = Math.max(
    1,
    ...s.damper_velocity_hist.flatMap((h) => HIST_KEYS.map(([k]) => h[k])),
  );
  return (
    <Card title={t("eng.susp.title")}>
      <div className="eng-figs is-3">
        <Fig
          label={t("eng.susp.bottoming")}
          value={fmtNum(s.bottoming_events_per_lap, 1)}
          color={s.bottoming_events_per_lap > 0.5 ? "var(--warm)" : undefined}
        />
        <Fig
          label={t("eng.susp.minRh")}
          value={`${t("eng.susp.front")} ${fmtNum(s.min_ride_height_mm[0], 0)} · ${t("eng.susp.rear")} ${fmtNum(s.min_ride_height_mm[1], 0)}`}
          unit="mm"
        />
        <div className="eng-fig">
          <div className="eng-fig-label dim">{t("eng.susp.saturation")}</div>
          <div className="sat-bars" title={t("eng.susp.saturationHint")}>
            {s.travel_saturation_pct.map((v, i) => (
              <span key={WHEEL_IDS[i]} className="sat-bar">
                <span className="sat-bar-name faint">{WHEEL_LABELS[lang][i]}</span>
                <span className="sat-bar-track">
                  <span
                    className="sat-bar-fill"
                    style={{
                      width: `${Math.min(100, v * 5)}%`,
                      background: v > 10 ? "var(--hot)" : "var(--text-dim)",
                    }}
                  />
                </span>
                <span className="sat-bar-val num">{fmtNum(v, 1)} %</span>
              </span>
            ))}
          </div>
        </div>
      </div>
      <div className="dampers">
        <div className="dampers-head">
          <span className="eng-fig-label dim">{t("eng.susp.dampers")}</span>
          <span className="faint eng-card-hint">{t("eng.susp.dampersHint")}</span>
        </div>
        <div className="car-grid is-dampers">
          {s.damper_velocity_hist.map((h, i) => (
            <div key={WHEEL_IDS[i]} className="damper">
              <div className="damper-head">
                <span className="etyre-name">{WHEEL_LABELS[lang][i]}</span>
                <span
                  className="faint num"
                  style={{ color: Math.abs(h.skew_slow) >= 5 ? "var(--warm)" : undefined }}
                >
                  {t("eng.susp.skew", { v: fmtSigned(h.skew_slow, 0) })}
                </span>
              </div>
              {HIST_KEYS.map(([k, label]) => (
                <div key={k} className="damper-row">
                  <span className="damper-label dim">{t(label)}</span>
                  <span className="damper-track">
                    <span
                      className={`damper-fill${k.startsWith("fast") ? " is-fast" : ""}`}
                      style={{ width: `${(h[k] / maxPct) * 100}%` }}
                    />
                  </span>
                  <span className="damper-val num">{fmtNum(h[k], 0)} %</span>
                </div>
              ))}
            </div>
          ))}
        </div>
      </div>
    </Card>
  );
}

/* ------------------------------------------------------------------ findings */

const SEVERITY_GLYPH: Record<Severity, string> = { major: "!!", minor: "!", info: "i" };

function FindingCard({ f }: { f: Finding }) {
  const { t, lang } = useT();
  const fr = lang === "fr";
  const laps = f.lap_numbers.length
    ? t("eng.findings.laps", { list: f.lap_numbers.map(lapNo).join(", ") })
    : t("eng.findings.allLaps");
  return (
    <article className={`finding is-${f.severity}`}>
      <div className="finding-head">
        <span
          className="finding-badge"
          role="img"
          aria-label={t(`eng.findings.${f.severity}`)}
          title={t(`eng.findings.${f.severity}`)}
        >
          {SEVERITY_GLYPH[f.severity]}
        </span>
        <h3 className="finding-title">{fr ? f.title_fr : f.title_en}</h3>
      </div>
      <p className="finding-detail dim">{fr ? f.detail_fr : f.detail_en}</p>
      {f.evidence.length > 0 && (
        <dl className="finding-evidence">
          {f.evidence.map((e) => (
            <div key={e.label}>
              <dt className="faint">{e.label}</dt>
              <dd className="num">{fmtEvidence(e)}</dd>
            </div>
          ))}
        </dl>
      )}
      <div className="echips is-small">
        {f.phase && <span className="echip">{t(`eng.balance.phase.${f.phase}`)}</span>}
        {f.corner_speed && (
          <span className="echip">{t(`eng.balance.speed.${f.corner_speed}`)}</span>
        )}
        <span className="echip is-ghost num">
          {t("eng.findings.confidence", { pct: (f.confidence * 100).toFixed(0) })}
        </span>
        <span className="echip is-ghost">{laps}</span>
      </div>
    </article>
  );
}

export function FindingsSection({ findings }: { findings: Finding[] }) {
  const { t } = useT();
  const [filter, setFilter] = useState<Severity | "all">("all");
  const sorted = useMemo(() => sortFindings(findings), [findings]);
  const counts = useMemo(() => {
    const c: Record<Severity, number> = { major: 0, minor: 0, info: 0 };
    for (const f of findings) c[f.severity] += 1;
    return c;
  }, [findings]);
  const shown = filter === "all" ? sorted : sorted.filter((f) => f.severity === filter);
  const levels = (Object.keys(SEVERITY_RANK) as Severity[]).sort(
    (a, b) => SEVERITY_RANK[a] - SEVERITY_RANK[b],
  );
  return (
    <Card
      title={t("eng.findings.title")}
      className="is-wide"
      actions={
        findings.length > 0 && (
          <div className="seg">
            <button
              type="button"
              className={`seg-btn${filter === "all" ? " is-on" : ""}`}
              onClick={() => setFilter("all")}
            >
              {t("eng.findings.filter.all")}{" "}
              <span className="seg-count num">{findings.length}</span>
            </button>
            {levels.map((s) => (
              <button
                key={s}
                type="button"
                className={`seg-btn is-${s}${filter === s ? " is-on" : ""}`}
                onClick={() => setFilter(s)}
                disabled={counts[s] === 0}
              >
                {t(`eng.findings.${s}`)} <span className="seg-count num">{counts[s]}</span>
              </button>
            ))}
          </div>
        )
      }
    >
      {findings.length === 0 ? (
        <p className="dim">{t("eng.findings.empty")}</p>
      ) : shown.length === 0 ? (
        <p className="dim">{t("eng.findings.none")}</p>
      ) : (
        <div className="findings">
          {shown.map((f) => (
            <FindingCard key={f.id} f={f} />
          ))}
        </div>
      )}
    </Card>
  );
}
