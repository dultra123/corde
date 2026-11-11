import type {
  PlanIteration,
  SetupChange,
  SetupFileInfo,
  SetupFilesCreated,
  SetupLibrary,
} from "@corde/protocol";
import { useEffect, useMemo, useState } from "react";
import { type TranslationKey, useT } from "../i18n";
import { fmtDateTime } from "../lib/format";
import { createSetupFiles } from "../net/engineer-api";
import { ApiError } from "../net/sessions-api";
import { errorMessage } from "../sessions/shared";
import {
  type FilePlanChoice,
  filePlanChoices,
  resolvedLines,
  resolvedNotes,
  statusTone,
} from "./lib";

/** `before → after` of a resolved change, one line per group of wheels, with its status. */
export function ResolvedRows({ c }: { c: SetupChange }) {
  const { t, lang } = useT();
  const r = c.resolved;
  if (!r) return null;
  const lines = resolvedLines(c, lang);
  const notes = resolvedNotes(c, lang);
  const tone = statusTone(r.status);
  return (
    <div className={`resolved is-${tone}`}>
      {lines.map((l, i) => (
        <div key={`${l.wheels ?? ""}-${i}`} className="resolved-line num">
          {l.wheels && <span className="resolved-wheels">{l.wheels}</span>}
          <span className="resolved-before">{l.before}</span>
          <span className="resolved-arrow" aria-hidden="true">
            →
          </span>
          <span className={`resolved-after${l.moved ? " is-moved" : ""}`}>{l.after}</span>
          {l.delta && <span className="resolved-delta faint">{l.delta}</span>}
        </div>
      ))}
      {(r.status !== "applied" || notes.length > 0) && (
        <div className="resolved-meta">
          {r.status !== "applied" && (
            <span className={`echip is-status is-${tone}`}>
              {t(`eng.setup.status.${r.status}` as TranslationKey)}
            </span>
          )}
          {notes.length > 0 && (
            <details className="resolved-notes">
              <summary className="faint">{t("eng.plans.notes")}</summary>
              <ul className="dim">
                {notes.map((n) => (
                  <li key={n}>{n}</li>
                ))}
              </ul>
            </details>
          )}
        </div>
      )}
    </div>
  );
}

/** Changes left out by the resolution (not adjustable…), folded, each with its reason. */
export function DroppedList({ dropped }: { dropped: readonly SetupChange[] }) {
  const { t, lang } = useT();
  if (dropped.length === 0) return null;
  return (
    <details className="dropped">
      <summary className="dim">
        {t("eng.plans.dropped", { n: dropped.length })}{" "}
        <span className="faint">— {t("eng.plans.droppedHint")}</span>
      </summary>
      <ul className="dropped-list">
        {dropped.map((d, i) => {
          const why = resolvedNotes(d, lang).at(-1);
          const status = d.resolved?.status;
          return (
            <li key={`${d.parameter_id}-${i}`} className="dropped-item">
              <span className="dropped-label">{d.label_fr}</span>
              {status && (
                <span className={`echip is-status is-${statusTone(status)}`}>
                  {t(`eng.setup.status.${status}` as TranslationKey)}
                </span>
              )}
              {why && <span className="dropped-why faint">{why}</span>}
            </li>
          );
        })}
      </ul>
    </details>
  );
}

function fileOption(f: SetupFileInfo, lang: "fr" | "en"): string {
  const date = fmtDateTime(f.mtime_ms, lang);
  return `${f.folder} / ${f.file} — ${date}`;
}

function choiceLabel(
  c: FilePlanChoice,
  t: (k: TranslationKey, p?: Record<string, string | number>) => string,
): string {
  const plan = t(`eng.plans.${c.philosophy}` as TranslationKey);
  return c.iteration === null ? plan : t("eng.setup.files.iteration", { n: c.iteration, plan });
}

/**
 * "Ton réglage": the base the plans are computed on (proposed by the server, the driver can pick
 * another one of the same car), and the button that writes the chosen plans as `.svm` files.
 */
export function SetupSection({
  sessionId,
  library,
  loading,
  error,
  base,
  onBase,
  iterations,
}: {
  sessionId: string;
  library: SetupLibrary | null;
  loading: boolean;
  error: string | null;
  /** The base in use (`Folder/File.svm`): the chosen one, else the proposed one. */
  base: string | null;
  onBase: (path: string) => void;
  iterations: readonly PlanIteration[];
}) {
  const { t, lang } = useT();
  const choices = useMemo(() => filePlanChoices(iterations), [iterations]);
  const [checked, setChecked] = useState<Set<string>>(() => new Set(["safe"]));
  const [busy, setBusy] = useState(false);
  const [created, setCreated] = useState<SetupFilesCreated | null>(null);
  const [failure, setFailure] = useState<string | null>(null);

  // A new base: the previous result no longer applies.
  // biome-ignore lint/correctness/useExhaustiveDependencies: reset on base change only
  useEffect(() => {
    setCreated(null);
    setFailure(null);
  }, [base]);

  const head = (
    <div className="card-head">
      <h2>{t("eng.setup.title")}</h2>
    </div>
  );

  if (!library) {
    return (
      <section className="card eng-card is-wide setup">
        {head}
        {error ? (
          <p className="form-error">{t("eng.setup.error", { msg: error })}</p>
        ) : (
          <p className="faint">{loading ? t("eng.setup.loading") : ""}</p>
        )}
      </section>
    );
  }

  if (!library.user_data_known) {
    return (
      <section className="card eng-card is-wide setup">
        {head}
        <div className="setup-empty">
          <p className="setup-empty-line setup-empty-title">{t("eng.setup.noGame.title")}</p>
          <p className="setup-empty-line dim">{t("eng.setup.noGame.hint")}</p>
        </div>
      </section>
    );
  }

  if (library.candidates.length === 0 || !base) {
    return (
      <section className="card eng-card is-wide setup">
        {head}
        <div className="setup-empty">
          <p className="setup-empty-line setup-empty-title">
            {t("eng.setup.none.title", {
              car: library.vehicle || (lang === "fr" ? "voiture" : "car"),
            })}
          </p>
          <p className="setup-empty-line dim">{t("eng.setup.none.hint")}</p>
          {library.settings_dir && (
            <p className="setup-empty-line faint num setup-dir">
              {t("eng.setup.none.folder", {
                dir: library.settings_dir,
                n: library.total_files,
              })}
            </p>
          )}
        </div>
      </section>
    );
  }

  const current = library.candidates.find((f) => f.path === base) ?? library.proposed ?? null;
  const isProposed = !!library.proposed && current?.path === library.proposed.path;

  const toggle = (ref: string) =>
    setChecked((cur) => {
      const next = new Set(cur);
      if (next.has(ref)) next.delete(ref);
      else next.add(ref);
      return next;
    });

  const selected = choices.filter((c) => checked.has(c.ref)).map((c) => c.ref);

  const create = async () => {
    if (!current || selected.length === 0) return;
    setBusy(true);
    setFailure(null);
    setCreated(null);
    try {
      const res = await createSetupFiles(sessionId, { base: current.path, plans: selected });
      setCreated(res);
    } catch (e) {
      setFailure(
        e instanceof ApiError && e.status === 403
          ? t("eng.setup.files.forbidden")
          : t("eng.setup.files.error", { msg: errorMessage(e) }),
      );
    } finally {
      setBusy(false);
    }
  };

  return (
    <section className="card eng-card is-wide setup">
      {head}
      <p className="eng-card-hint dim">{t("eng.setup.hint")}</p>

      <div className="setup-grid">
        <div className="setup-base">
          <span className="interp-k">{t("eng.setup.base")}</span>
          {current && (
            <>
              <div className="setup-file">
                <span className="setup-file-name num">{current.file}</span>
                <span className="echip is-ghost">{current.folder}</span>
                {isProposed && <span className="echip is-accent">{t("eng.setup.proposed")}</span>}
              </div>
              <div className="faint num setup-file-meta">
                {t("eng.setup.saved", { when: fmtDateTime(current.mtime_ms, lang) })} ·{" "}
                {current.vehicle}
              </div>
              <p className={`setup-notes${current.notes ? "" : " faint"}`}>
                {current.notes ? `« ${current.notes} »` : t("eng.setup.noNotes")}
              </p>
              {isProposed && library.proposed_reason_fr && lang === "fr" && (
                <p className="faint setup-reason">{library.proposed_reason_fr}</p>
              )}
            </>
          )}
          {library.candidates.length > 1 && (
            <label className="setup-pick">
              <span className="dim">{t("eng.setup.change")}</span>
              <select
                className="select"
                value={current?.path ?? ""}
                onChange={(e) => onBase(e.target.value)}
              >
                {library.candidates.map((f) => (
                  <option key={f.path} value={f.path}>
                    {fileOption(f, lang)}
                    {library.track_folder && f.folder !== library.track_folder
                      ? ` (${t("eng.setup.otherTrack")})`
                      : ""}
                  </option>
                ))}
              </select>
            </label>
          )}
          <div className="setup-flags">
            <span className={`echip setup-flag ${library.bounds_known ? "is-ok" : "is-ghost"}`}>
              {library.bounds_known ? t("eng.setup.bounds.known") : t("eng.setup.bounds.unknown")}
            </span>
            {library.unreadable.length > 0 && (
              <span className="echip setup-flag is-ghost" title={library.unreadable.join("\n")}>
                {t("eng.setup.unreadable", { n: library.unreadable.length })}
              </span>
            )}
          </div>
        </div>

        <div className="setup-files">
          <span className="interp-k">{t("eng.setup.files.title")}</span>
          <p className="dim setup-files-hint">
            {t("eng.setup.files.hint", { folder: current?.folder ?? "" })}
          </p>
          <div className="setup-choices">
            {choices.map((c) => (
              <label key={c.ref} className={`setup-choice is-${c.philosophy}`}>
                <input
                  type="checkbox"
                  checked={checked.has(c.ref)}
                  onChange={() => toggle(c.ref)}
                />
                <span>{choiceLabel(c, t)}</span>
              </label>
            ))}
          </div>
          <button
            type="button"
            className="btn is-active is-cta setup-create"
            disabled={busy || selected.length === 0}
            onClick={create}
          >
            {busy ? <span className="spinner is-small" /> : null}
            {busy ? t("eng.setup.files.creating") : t("eng.setup.files.create")}
          </button>
          {failure && (
            <p className="form-error" role="alert">
              {failure}
            </p>
          )}
          {created && (
            <div className="setup-done" role="status">
              <span className="interp-k setup-done-k">{t("eng.setup.files.done")}</span>
              <ul>
                {created.files.map((f) => (
                  <li key={f.path}>
                    <span className="num setup-done-file">{f.file}</span>
                    <span className="faint">
                      {" "}
                      · {f.plan_label_fr} · {t("eng.setup.files.lines", { n: f.changed_lines })}
                    </span>
                  </li>
                ))}
              </ul>
              <p className="setup-howto">
                {t("eng.setup.files.howto", { file: created.files[0]?.file ?? "Corde_…" })}
              </p>
            </div>
          )}
        </div>
      </div>
    </section>
  );
}
