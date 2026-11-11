import type {
  AnalysisReport,
  DetectedSymptom,
  FeedbackInterpretation,
  PlanIteration,
  SetupChange,
} from "@corde/protocol";
import { useEffect, useState } from "react";
import { type TranslationKey, useT } from "../i18n";
import { fmtDateTime } from "../lib/format";
import {
  changeAmount,
  fmtPct,
  interpretationQuestions,
  iterationToText,
  resolvedText,
  symptomChips,
  symptomLabel,
} from "./lib";
import { DroppedList, ResolvedRows } from "./SetupSection";

export type IterationMeta = Pick<AnalysisReport, "track_name" | "vehicle" | "session_id">;

/** One detected symptom: label, attribute chips, the driver's words. */
export function SymptomRow({ s, compact }: { s: DetectedSymptom; compact?: boolean }) {
  const { t, lang } = useT();
  const chips = symptomChips(s, lang);
  return (
    <li className={`symptom${s.negated ? " is-negated" : ""}${compact ? " is-compact" : ""}`}>
      <span className="symptom-label">
        {s.negated ? <s>{symptomLabel(s, lang)}</s> : symptomLabel(s, lang)}
      </span>
      <span className="echips is-small symptom-chips">
        {s.negated && <span className="echip is-negated">{t("eng.interp.negated")}</span>}
        {chips.map((c) => (
          <span key={c.kind} className={`echip is-${c.kind}`}>
            {c.text}
          </span>
        ))}
        {!compact && (
          <span
            className="echip is-ghost num"
            title={t("eng.findings.confidence", { pct: Math.round(s.confidence * 100) })}
          >
            {fmtPct(s.confidence)}
          </span>
        )}
      </span>
      {!compact && s.matched_text && (
        <span className="symptom-matched faint">
          {t("eng.interp.matched", { text: s.matched_text })}
        </span>
      )}
    </li>
  );
}

/** Symptoms, questions (with a "clarify" button) and what was not understood. */
export function InterpretationView({
  interpretation,
  onClarify,
}: {
  interpretation: FeedbackInterpretation;
  onClarify?: (question: string) => void;
}) {
  const { t, lang } = useT();
  const questions = interpretationQuestions(interpretation, lang);
  return (
    <div className="interp">
      <div className="interp-head">
        <h4>{t("eng.interp.title")}</h4>
        <span className="faint num">
          {t("eng.interp.confidence", { pct: fmtPct(interpretation.confidence) })} ·{" "}
          {t(`eng.interp.by.${interpretation.extractor}` as TranslationKey)}
        </span>
      </div>
      {interpretation.symptoms.length === 0 ? (
        <p className="dim interp-none">{t("eng.interp.none")}</p>
      ) : (
        <ul className="symptoms">
          {interpretation.symptoms.map((s, i) => (
            <SymptomRow key={`${s.symptom_id}-${i}`} s={s} />
          ))}
        </ul>
      )}
      {questions.length > 0 && (
        <div className="interp-questions">
          <span className="interp-k">{t("eng.interp.questions")}</span>
          <ul>
            {questions.map((q) => (
              <li key={q} className="interp-q">
                <span>{q}</span>
                {onClarify && (
                  <button type="button" className="btn is-small" onClick={() => onClarify(q)}>
                    {t("eng.interp.clarify")}
                  </button>
                )}
              </li>
            ))}
          </ul>
        </div>
      )}
      {interpretation.unmatched_fragments.length > 0 && (
        <p className="faint interp-unmatched">
          {t("eng.interp.unmatched", { list: interpretation.unmatched_fragments.join(" · ") })}
        </p>
      )}
      {interpretation.notes_fr.length > 0 && (
        <ul className="interp-notes dim">
          {interpretation.notes_fr.map((n) => (
            <li key={n}>{n}</li>
          ))}
        </ul>
      )}
    </div>
  );
}

function DeltaRow({ c, n }: { c: SetupChange; n: number }) {
  const { t, lang } = useT();
  const { arrow, amount, note } = changeAmount(c, lang);
  const why = lang === "fr" ? c.rationale_fr : c.rationale_en;
  return (
    <li className={`delta is-${c.direction}`}>
      <span className="delta-n num">{n}</span>
      <div className="delta-main">
        <div className="delta-row">
          <span className="delta-label">{c.label_fr}</span>
          <span className="delta-amount num">
            <span className="change-arrow" aria-hidden="true">
              {arrow}
            </span>
            {amount}
          </span>
        </div>
        <div className="change-where faint num">
          {c.section ? `${c.section} · ` : ""}
          {c.parameter_key}
          {note && <span className="change-note"> · {note}</span>}
        </div>
        <ResolvedRows c={c} />
        {why && (
          <p className="change-why dim">
            <span className="change-why-k">{t("eng.plans.why")}</span> {why}
          </p>
        )}
      </div>
    </li>
  );
}

/** One iteration: the delta first, agreements / disagreements, notes, the cumulative plan. */
export function IterationView({
  it,
  meta,
  latest,
}: {
  it: PlanIteration;
  meta: IterationMeta;
  latest?: boolean;
}) {
  const { t, lang } = useT();
  const [copied, setCopied] = useState(false);
  useEffect(() => {
    if (!copied) return;
    const id = setTimeout(() => setCopied(false), 1800);
    return () => clearTimeout(id);
  }, [copied]);

  const copy = async () => {
    const text = iterationToText(it, meta, lang);
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
    } catch {
      window.prompt(t("eng.iter.copy"), text);
    }
  };

  return (
    <div className={`iteration is-${it.base_plan}${latest ? " is-latest" : ""}`}>
      <div className="iteration-head">
        <h4>
          {t("eng.iter.title", { n: it.iteration })}
          <span className={`echip is-plan is-${it.base_plan}`}>
            {t(`eng.plans.${it.base_plan}` as TranslationKey)}
          </span>
          {latest && <span className="echip is-accent">{t("eng.iter.latest")}</span>}
        </h4>
        <div className="iteration-actions">
          <span className="faint num">{fmtDateTime(it.created_ms, lang)}</span>
          <button type="button" className="btn is-small" onClick={copy}>
            {copied ? t("eng.iter.copied") : t("eng.iter.copy")}
          </button>
        </div>
      </div>

      <div className="iteration-delta">
        <span className="interp-k">{t("eng.iter.delta")}</span>
        {it.delta.length === 0 ? (
          <p className="dim">{t("eng.iter.deltaEmpty")}</p>
        ) : (
          <ol className="deltas">
            {it.delta.map((c, i) => (
              <DeltaRow key={`${c.parameter_id}-${i}`} c={c} n={i + 1} />
            ))}
          </ol>
        )}
      </div>

      {(it.agreements_fr.length > 0 || it.disagreements_fr.length > 0) && (
        <div className="iteration-verdicts">
          {it.agreements_fr.length > 0 && (
            <div className="verdict is-agree">
              <span className="interp-k">{t("eng.iter.agreements")}</span>
              <ul>
                {it.agreements_fr.map((a) => (
                  <li key={a}>{a}</li>
                ))}
              </ul>
            </div>
          )}
          {it.disagreements_fr.length > 0 && (
            <div className="verdict is-disagree">
              <span className="interp-k">{t("eng.iter.disagreements")}</span>
              <ul>
                {it.disagreements_fr.map((d) => (
                  <li key={d}>{d}</li>
                ))}
              </ul>
            </div>
          )}
        </div>
      )}

      {it.notes_fr.length > 0 && (
        <ul className="iteration-notes dim">
          {it.notes_fr.map((n) => (
            <li key={n}>{n}</li>
          ))}
        </ul>
      )}

      <details className="iteration-cumulative">
        <summary className="iteration-cumulative-summary">
          {t("eng.iter.cumulative", { n: it.changes.length })}
          {it.data_findings_used.length > 0 && (
            <span className="faint">
              {" "}
              · {t("eng.iter.dataUsed", { n: it.data_findings_used.length })}
            </span>
          )}
        </summary>
        <ol className="cumulative">
          {it.changes.map((c, i) => {
            const { arrow, amount } = changeAmount(c, lang);
            const isNew = it.delta.some((d) => d.parameter_id === c.parameter_id);
            const real = resolvedText(c, lang);
            return (
              <li
                key={`${c.parameter_id}-${i}`}
                className={`cumul is-${c.direction}${isNew ? " is-new" : ""}`}
              >
                <span className="cumul-label">{c.label_fr}</span>
                <span className="cumul-amount num">
                  <span className="change-arrow" aria-hidden="true">
                    {arrow}
                  </span>
                  {amount}
                </span>
                <span className="faint num cumul-key">{real ?? c.parameter_key}</span>
              </li>
            );
          })}
        </ol>
      </details>
      <DroppedList dropped={it.dropped ?? []} />
    </div>
  );
}

/** Every iteration of the session, newest first; the newest one is open. */
export function IterationsHistory({
  iterations,
  meta,
  loading,
  error,
}: {
  iterations: readonly PlanIteration[];
  meta: IterationMeta;
  loading: boolean;
  error: string | null;
}) {
  const { t, lang } = useT();
  return (
    <section className="card eng-card is-wide iterations">
      <div className="card-head">
        <h2>{t("eng.iter.history")}</h2>
        <span className="faint num">{iterations.length}</span>
      </div>
      {error && iterations.length === 0 ? (
        <p className="form-error">{t("eng.iter.historyError", { msg: error })}</p>
      ) : iterations.length === 0 ? (
        <p className="faint">{loading ? t("common.loading") : t("eng.iter.historyNone")}</p>
      ) : (
        <div className="iterations-list">
          {iterations.map((it, i) =>
            i === 0 ? (
              <IterationView key={it.id} it={it} meta={meta} latest />
            ) : (
              <details key={it.id} className="iteration-folded">
                <summary className="iteration-folded-summary">
                  <span className="num">{t("eng.iter.title", { n: it.iteration })}</span>
                  <span className={`echip is-plan is-${it.base_plan}`}>
                    {t(`eng.plans.${it.base_plan}` as TranslationKey)}
                  </span>
                  <span className="faint num">{fmtDateTime(it.created_ms, lang)}</span>
                  <span className="faint">
                    {it.delta.length > 0
                      ? it.delta
                          .map((d) => `${d.label_fr} ${changeAmount(d, lang).arrow}`)
                          .join(" · ")
                      : t("eng.iter.deltaEmpty")}
                  </span>
                </summary>
                <IterationView it={it} meta={meta} />
              </details>
            ),
          )}
        </div>
      )}
    </section>
  );
}
