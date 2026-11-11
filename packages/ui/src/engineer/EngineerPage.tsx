import type { AnalysisReport } from "@corde/protocol";
import { useCallback, useEffect, useRef, useState } from "react";
import { IconRefresh } from "../dashboard/icons";
import { type TranslationKey, useT } from "../i18n";
import { fmtDateTime } from "../lib/format";
import {
  fetchAnalysis,
  fetchIterations,
  fetchSetups,
  recomputeAnalysis,
} from "../net/engineer-api";
import { ApiError, fetchSession, type SessionDetail } from "../net/sessions-api";
import { errorMessage, Notice, PageHeader, SessionTypeTag, useAsync } from "../sessions/shared";
import { FeedbackSection } from "./FeedbackSection";
import { missingChannels, qualityLevel } from "./lib";
import { PlansSection } from "./PlansSection";
import {
  BalanceSection,
  BrakesSection,
  FindingsSection,
  LapsSection,
  StyleSection,
  SuspensionSection,
  TyresSection,
} from "./Sections";
import { SetupSection } from "./SetupSection";

type Phase =
  | { kind: "loading" }
  | { kind: "ready"; report: AnalysisReport; recomputing: boolean }
  | { kind: "notEnough"; message: string }
  | { kind: "notFound" }
  | { kind: "error"; message: string };

/**
 * Loads the engineer's report, telling apart "not enough clean laps" (422, the server's French
 * sentence) from a missing session (404) and a network / server failure. `recompute` keeps the
 * current report on screen while the new one is calculated.
 */
function useAnalysis(
  id: string,
  base: string | null,
): { phase: Phase; reload: () => void; recompute: () => void } {
  const [phase, setPhase] = useState<Phase>({ kind: "loading" });
  const [tick, setTick] = useState(0);
  const gen = useRef(0);

  // biome-ignore lint/correctness/useExhaustiveDependencies: `tick` forces a reload after an error
  useEffect(() => {
    const ctrl = new AbortController();
    const my = ++gen.current;
    setPhase((p) => (p.kind === "ready" ? { ...p, recomputing: true } : { kind: "loading" }));
    fetchAnalysis(id, ctrl.signal, base)
      .then((report) => {
        if (my === gen.current) setPhase({ kind: "ready", report, recomputing: false });
      })
      .catch((e: unknown) => {
        if (ctrl.signal.aborted || my !== gen.current) return;
        setPhase(phaseOfError(e));
      });
    return () => ctrl.abort();
  }, [id, tick, base]);

  const reload = useCallback(() => setTick((n) => n + 1), []);
  const recompute = useCallback(() => {
    const my = ++gen.current;
    setPhase((p) => (p.kind === "ready" ? { ...p, recomputing: true } : { kind: "loading" }));
    recomputeAnalysis(id, undefined, base)
      .then((report) => {
        if (my === gen.current) setPhase({ kind: "ready", report, recomputing: false });
      })
      .catch((e: unknown) => {
        if (my === gen.current) setPhase(phaseOfError(e));
      });
  }, [id, base]);

  return { phase, reload, recompute };
}

function phaseOfError(e: unknown): Phase {
  if (e instanceof ApiError) {
    if (e.status === 422) return { kind: "notEnough", message: e.message };
    if (e.status === 404) return { kind: "notFound" };
  }
  return { kind: "error", message: errorMessage(e) };
}

function QualityPill({ report }: { report: AnalysisReport }) {
  const { t } = useT();
  const q = report.data_quality;
  const level = qualityLevel(q);
  const missing = missingChannels(q).map((m) => t(`eng.quality.ch.${m}` as TranslationKey));
  const title = [
    `${q.sample_rate_hz.toFixed(0)} Hz`,
    missing.length ? t("eng.quality.missing", { list: missing.join(", ") }) : null,
    ...q.notes,
  ]
    .filter(Boolean)
    .join("\n");
  return (
    <span className={`qpill is-${level}`} title={title}>
      <span className="qpill-dot" />
      {t(`eng.quality.${level}`)}
      <span className="qpill-rate num">{q.sample_rate_hz.toFixed(0)} Hz</span>
    </span>
  );
}

function Header({
  id,
  session,
  report,
  recomputing,
  onRecompute,
}: {
  id: string;
  session: SessionDetail | null;
  report: AnalysisReport | null;
  recomputing: boolean;
  onRecompute?: () => void;
}) {
  const { t, lang } = useT();
  const track = report?.track_name ?? session?.track_name ?? id;
  const vehicle = report?.vehicle ?? session?.vehicle_model ?? session?.vehicle_name ?? "";
  const q = report?.data_quality;
  const missing = q ? missingChannels(q) : [];
  return (
    <PageHeader
      back={{ page: "session", id }}
      backLabel={t("eng.backToSession")}
      title={
        <>
          <span className="eng-kicker">{t("eng.title")}</span>
          {track}
        </>
      }
      subtitle={
        <>
          {session && <SessionTypeTag type={session.session_type} />}
          {vehicle && <span className="is-strong"> {vehicle}</span>}
          {session?.class_name && <span> · {session.class_name}</span>}
          {session && <span> · {fmtDateTime(session.started_wall_ms, lang)}</span>}
        </>
      }
      actions={
        onRecompute && (
          <button
            type="button"
            className="btn"
            onClick={onRecompute}
            disabled={recomputing}
            title={t("eng.recompute")}
          >
            {recomputing ? <span className="spinner is-small" /> : <IconRefresh />}
            {recomputing ? t("eng.recomputing") : t("eng.recompute")}
          </button>
        )
      }
      meta={
        report &&
        q && (
          <div className="eng-meta">
            <div className="eng-meta-row">
              <span className="num">
                {t("eng.lapsAnalysed", { n: q.valid_laps, total: q.total_laps })}
              </span>
              <QualityPill report={report} />
              <span className="faint">
                {t("eng.generated", { when: fmtDateTime(report.generated_ms, lang) })}
              </span>
            </div>
            {(q.valid_laps < 3 || missing.length > 0 || q.notes.length > 0) && (
              <ul className="eng-notes dim">
                {q.valid_laps < 3 && <li>{t("eng.quality.fewLaps")}</li>}
                {missing.length > 0 && (
                  <li>
                    {t("eng.quality.missing", {
                      list: missing
                        .map((m) => t(`eng.quality.ch.${m}` as TranslationKey))
                        .join(", "),
                    })}
                  </li>
                )}
                {q.notes.map((n) => (
                  <li key={n}>{n}</li>
                ))}
              </ul>
            )}
          </div>
        )
      }
    />
  );
}

export function EngineerPage({ id }: { id: string }) {
  const { t, lang } = useT();
  // The setup the plans are computed on: chosen by the driver, else the one the server proposes.
  const [chosenBase, setChosenBase] = useState<string | null>(null);
  const [itTick, setItTick] = useState(0);
  const { phase, reload, recompute } = useAnalysis(id, chosenBase);
  const session = useAsync((signal) => fetchSession(id, signal), [id]);
  const setups = useAsync((signal) => fetchSetups(id, signal), [id]);
  const iterations = useAsync(
    (signal) => fetchIterations(id, undefined, signal, chosenBase),
    [id, chosenBase, itTick],
  );
  const detail = session.data;
  const base = chosenBase ?? setups.data?.proposed?.path ?? null;

  if (phase.kind === "loading") {
    return (
      <div className="eng-page">
        <Header id={id} session={detail} report={null} recomputing={false} />
        <Notice kind="loading" title={t("eng.loading.title")}>
          {t("eng.loading.hint")}
        </Notice>
      </div>
    );
  }
  if (phase.kind === "notEnough") {
    return (
      <div className="eng-page">
        <Header id={id} session={detail} report={null} recomputing={false} />
        <Notice
          kind="empty"
          title={t("eng.notEnough.title")}
          action={
            <button type="button" className="btn" onClick={reload}>
              <IconRefresh />
              {t("eng.recompute")}
            </button>
          }
        >
          <p className="eng-server-msg">{phase.message}</p>
          <p>{t("eng.notEnough.hint")}</p>
        </Notice>
      </div>
    );
  }
  if (phase.kind === "notFound") {
    return (
      <div className="eng-page">
        <Header id={id} session={detail} report={null} recomputing={false} />
        <Notice kind="error" title={t("eng.notFound")} />
      </div>
    );
  }
  if (phase.kind === "error") {
    return (
      <div className="eng-page">
        <Header id={id} session={detail} report={null} recomputing={false} />
        <Notice
          kind="error"
          title={t("eng.error.title")}
          action={
            <button type="button" className="btn" onClick={reload}>
              {t("common.retry")}
            </button>
          }
        >
          {phase.message}
        </Notice>
      </div>
    );
  }

  const { report, recomputing } = phase;
  const summary = lang === "fr" ? report.summary_fr : report.summary_en;
  return (
    <div className={`eng-page${recomputing ? " is-recomputing" : ""}`}>
      <Header
        id={id}
        session={detail}
        report={report}
        recomputing={recomputing}
        onRecompute={recompute}
      />

      <section className="card eng-summary">
        <div className="card-head">
          <h2>{t("eng.summary.title")}</h2>
        </div>
        <p className="eng-summary-text">{summary}</p>
      </section>

      <div className="eng-grid">
        <StyleSection style={report.driving_style} />
        <LapsSection stats={report.lap_stats} />
        <TyresSection tyres={report.tyres} />
        <BrakesSection brakes={report.brakes} />
        <BalanceSection balance={report.balance} />
        <SuspensionSection suspension={report.suspension} />
      </div>

      <FindingsSection findings={report.findings} />
      <SetupSection
        sessionId={id}
        library={setups.data}
        loading={setups.loading}
        error={setups.error}
        base={base}
        onBase={setChosenBase}
        iterations={iterations.data ?? []}
      />
      <PlansSection report={report} />
      <FeedbackSection
        sessionId={id}
        base={chosenBase}
        onIteration={() => setItTick((n) => n + 1)}
        meta={{
          track_name: report.track_name,
          vehicle: report.vehicle,
          session_id: report.session_id,
        }}
      />
    </div>
  );
}
