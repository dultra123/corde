import type { Feedback, PlanIteration, SetupPhilosophy } from "@corde/protocol";
import { type FormEvent, useRef, useState } from "react";
import { type TranslationKey, useT } from "../i18n";
import { fmtDateTime } from "../lib/format";
import {
  type FeedbackResponse,
  fetchFeedback,
  fetchIterations,
  postFeedback,
} from "../net/engineer-api";
import { errorMessage, useAsync } from "../sessions/shared";
import {
  InterpretationView,
  type IterationMeta,
  IterationsHistory,
  IterationView,
  SymptomRow,
} from "./IterationSection";
import { iterationOfFeedback, PHILOSOPHIES, sortIterations } from "./lib";

/** Must match `FEEDBACK_MAX_CHARS` in corde-server. */
export const FEEDBACK_MAX = 2000;

type PlanChoice = SetupPhilosophy | "none";

/** Feedback items shown before the list folds the older ones away. */
const VISIBLE_FEEDBACK = 3;

export function FeedbackSection({
  sessionId,
  meta,
  base = null,
  onIteration,
}: {
  sessionId: string;
  meta: IterationMeta;
  /** Setup the iterations are resolved against (null: the one the server proposes). */
  base?: string | null;
  /** Called after a feedback produced a new iteration. */
  onIteration?: () => void;
}) {
  const { t, lang } = useT();
  const list = useAsync((signal) => fetchFeedback(sessionId, signal), [sessionId]);
  const history = useAsync(
    (signal) => fetchIterations(sessionId, undefined, signal, base),
    [sessionId, base],
  );
  const [text, setText] = useState("");
  const [plan, setPlan] = useState<PlanChoice>("none");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [added, setAdded] = useState<FeedbackResponse[]>([]);
  const [latest, setLatest] = useState<FeedbackResponse | null>(null);
  const [showAll, setShowAll] = useState(false);
  const textarea = useRef<HTMLTextAreaElement>(null);

  const trimmed = text.trim();
  const tooLong = [...trimmed].length > FEEDBACK_MAX;
  const canSend = trimmed.length > 0 && !tooLong && !busy;

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    if (!canSend) return;
    setBusy(true);
    setError(null);
    try {
      const saved = await postFeedback(sessionId, {
        text: trimmed,
        ...(plan === "none" ? {} : { plan }),
      });
      setAdded((cur) => [...cur, saved]);
      setLatest(saved);
      setText("");
      list.reload();
      history.reload();
      onIteration?.();
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  /** "Préciser": the question goes into the text box, the driver completes it. */
  const clarify = (question: string) => {
    setText((cur) => `${cur.trim() ? `${cur.trim()}\n` : ""}${question} `);
    const el = textarea.current;
    if (el) {
      el.focus();
      requestAnimationFrame(() => el.setSelectionRange(el.value.length, el.value.length));
      el.scrollIntoView({ block: "center", behavior: "smooth" });
    }
  };

  // Server lists first; anything just sent that the reload has not returned yet is appended.
  const baseRows = list.data ?? [];
  const ids = new Set(baseRows.map((f) => f.id));
  const rows: Feedback[] = [
    ...baseRows,
    ...added.map((a) => a.feedback).filter((f) => !ids.has(f.id)),
  ];
  const baseIts = history.data ?? [];
  const itIds = new Set(baseIts.map((it) => it.id));
  const iterations: PlanIteration[] = sortIterations([
    ...baseIts,
    ...added.map((a) => a.iteration).filter((it) => !itIds.has(it.id)),
  ]);
  // Newest first; older ones folded unless asked.
  const newestFirst = [...rows].reverse();
  const visible = showAll ? newestFirst : newestFirst.slice(0, VISIBLE_FEEDBACK);
  const hidden = newestFirst.length - visible.length;

  return (
    <>
      <section className="card eng-card is-wide feedback">
        <div className="card-head">
          <h2>{t("eng.feedback.title")}</h2>
        </div>
        <p className="eng-card-hint dim">
          {t("eng.feedback.hint")} <span className="faint">{t("eng.feedback.later")}</span>
        </p>
        <form className="feedback-form" onSubmit={submit}>
          <textarea
            ref={textarea}
            className="input feedback-text"
            value={text}
            onChange={(e) => setText(e.target.value)}
            placeholder={t("eng.feedback.placeholder")}
            rows={4}
            maxLength={FEEDBACK_MAX + 200}
            aria-label={t("eng.feedback.title")}
          />
          <div className="feedback-row">
            <label className="feedback-plan">
              <span className="dim">{t("eng.feedback.plan")}</span>
              <select
                className="select"
                value={plan}
                onChange={(e) => setPlan(e.target.value as PlanChoice)}
              >
                <option value="none">{t("eng.feedback.plan.none")}</option>
                {PHILOSOPHIES.map((p) => (
                  <option key={p} value={p}>
                    {t(`eng.plans.${p}` as TranslationKey)}
                  </option>
                ))}
              </select>
            </label>
            <span className={`feedback-count num${tooLong ? " is-over" : ""}`}>
              {[...trimmed].length} / {FEEDBACK_MAX}
            </span>
            <button type="submit" className="btn is-active is-cta" disabled={!canSend}>
              {busy ? <span className="spinner is-small" /> : null}
              {busy ? t("eng.feedback.sending") : t("eng.feedback.send")}
            </button>
          </div>
          {error && (
            <p className="form-error" role="alert">
              {t("eng.feedback.error", { msg: error })}
            </p>
          )}
        </form>

        {latest && (
          <div className="feedback-result">
            <InterpretationView interpretation={latest.interpretation} onClarify={clarify} />
            <IterationView it={latest.iteration} meta={meta} latest />
          </div>
        )}

        <h3 className="feedback-saved-title dim">{t("eng.feedback.saved")}</h3>
        {list.error && !list.data ? (
          <p className="form-error">{t("eng.feedback.listError", { msg: list.error })}</p>
        ) : rows.length === 0 ? (
          <p className="faint">{list.loading ? t("common.loading") : t("eng.feedback.none")}</p>
        ) : (
          <>
            <ul className="feedback-list">
              {visible.map((f) => {
                const it = iterationOfFeedback(iterations, f.id);
                return (
                  <li key={f.id} className="feedback-item">
                    <div className="feedback-meta">
                      <span className="num dim">{fmtDateTime(f.created_ms, lang)}</span>
                      {f.plan && (
                        <span className={`echip is-plan is-${f.plan}`}>
                          {t(`eng.plans.${f.plan}` as TranslationKey)}
                        </span>
                      )}
                      {it && (
                        <span className="echip is-ghost num">
                          {t("eng.iter.title", { n: it.iteration })}
                          {" · "}
                          {it.delta.length === 0
                            ? t("eng.iter.noChange")
                            : it.delta.length === 1
                              ? t("eng.iter.oneChange")
                              : t("eng.iter.nChanges", { n: it.delta.length })}
                        </span>
                      )}
                    </div>
                    <p className="feedback-body">{f.text}</p>
                    {f.interpretation && f.interpretation.symptoms.length > 0 && (
                      <ul className="symptoms is-compact">
                        {f.interpretation.symptoms.map((s, i) => (
                          <SymptomRow key={`${s.symptom_id}-${i}`} s={s} compact />
                        ))}
                      </ul>
                    )}
                  </li>
                );
              })}
            </ul>
            {hidden > 0 && (
              <button
                type="button"
                className="btn is-small feedback-more"
                onClick={() => setShowAll(true)}
              >
                {t("eng.feedback.hidden", { n: hidden })}
              </button>
            )}
          </>
        )}
      </section>

      <IterationsHistory
        iterations={iterations}
        meta={meta}
        loading={history.loading}
        error={history.error}
      />
    </>
  );
}
