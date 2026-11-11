import type { AnalysisReport, SetupChange, SetupPhilosophy } from "@corde/protocol";
import { useEffect, useState } from "react";
import { IconDownload } from "../dashboard/icons";
import { type TranslationKey, useT } from "../i18n";
import { changeAmount, PHILOSOPHIES, planExport, planToText, priorityDots } from "./lib";
import { DroppedList, ResolvedRows } from "./SetupSection";

function ChangeRow({ c, n }: { c: SetupChange; n: number }) {
  const { t, lang } = useT();
  const { arrow, amount, note } = changeAmount(c, lang);
  const dots = priorityDots(c.priority);
  const why = lang === "fr" ? c.rationale_fr : c.rationale_en;
  return (
    <li className={`change is-${c.direction}`}>
      <span className="change-n num">{n}</span>
      <div className="change-main">
        <div className="change-row">
          <span className="change-label">{c.label_fr}</span>
          <span className="change-amount num">
            <span className="change-arrow" aria-hidden="true">
              {arrow}
            </span>
            {amount}
          </span>
          <span
            className="change-prio"
            role="img"
            title={`${t("eng.plans.priority", { n: c.priority })}${c.priority === 1 ? ` — ${t("eng.plans.priorityFirst")}` : ""}`}
            aria-label={t("eng.plans.priority", { n: c.priority })}
          >
            {[1, 2, 3, 4, 5].map((i) => (
              <span key={i} className={`change-dot${i <= dots ? " is-on" : ""}`} />
            ))}
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
        {(c.side_effects_fr.length > 0 || c.conflicts_with.length > 0) && (
          <details className="change-more">
            <summary>
              {c.side_effects_fr.length > 0 && t("eng.plans.sideEffects")}
              {c.side_effects_fr.length > 0 && c.conflicts_with.length > 0 && " · "}
              {c.conflicts_with.length > 0 && (
                <span className="is-warn">
                  {t("eng.plans.conflicts", { rules: c.conflicts_with.join(", ") })}
                </span>
              )}
            </summary>
            {c.side_effects_fr.length > 0 && (
              <ul className="change-effects dim">
                {c.side_effects_fr.map((e) => (
                  <li key={e}>{e}</li>
                ))}
              </ul>
            )}
            <div className="faint change-rules num">{c.rule_ids.join(" · ")}</div>
          </details>
        )}
      </div>
    </li>
  );
}

export function PlansSection({ report }: { report: AnalysisReport }) {
  const { t, lang } = useT();
  const [which, setWhich] = useState<SetupPhilosophy>("aggressive");
  const [copied, setCopied] = useState(false);
  const plan = report.plans[which];

  useEffect(() => {
    if (!copied) return;
    const id = setTimeout(() => setCopied(false), 1800);
    return () => clearTimeout(id);
  }, [copied]);

  const copy = async () => {
    const text = planToText(plan, report, lang);
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
    } catch {
      // Clipboard refused (http on a phone, no focus): fall back to a selectable prompt.
      window.prompt(t("eng.plans.copy"), text);
    }
  };

  const exportJson = () => {
    const blob = new Blob([planExport(plan, report)], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `corde-setup-${which}-${report.session_id}.json`;
    document.body.appendChild(a);
    a.click();
    a.remove();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
  };

  return (
    <section className="card eng-card is-wide plans">
      <div className="card-head">
        <h2>{t("eng.plans.title")}</h2>
        <div className="card-actions">
          <button type="button" className="btn" onClick={copy}>
            {copied ? t("eng.plans.copied") : t("eng.plans.copy")}
          </button>
          <button type="button" className="btn" onClick={exportJson}>
            <IconDownload />
            {t("eng.plans.export")}
          </button>
        </div>
      </div>

      <div className="plans-tabs" role="tablist">
        {PHILOSOPHIES.map((p) => (
          <button
            key={p}
            type="button"
            role="tab"
            aria-selected={which === p}
            className={`plans-tab is-${p}${which === p ? " is-on" : ""}`}
            onClick={() => setWhich(p)}
          >
            <span className="plans-tab-name">{t(`eng.plans.${p}` as TranslationKey)}</span>
            <span className="plans-tab-count num">{report.plans[p].changes.length}</span>
          </button>
        ))}
      </div>

      <div className={`plan is-${which}`}>
        <p className="plan-intent">{plan.intent_fr}</p>
        {plan.warnings_fr.length > 0 && (
          <details className="plan-warnings">
            <summary>
              {t("eng.plans.warnings")} <span className="num">({plan.warnings_fr.length})</span>
            </summary>
            <ul className="dim">
              {plan.warnings_fr.map((w) => (
                <li key={w}>{w}</li>
              ))}
            </ul>
          </details>
        )}
        {plan.changes.length === 0 ? (
          <p className="dim">{t("eng.plans.empty")}</p>
        ) : (
          <ol className="changes">
            {plan.changes.map((c, i) => (
              <ChangeRow key={`${c.parameter_id}-${i}`} c={c} n={i + 1} />
            ))}
          </ol>
        )}
        <DroppedList dropped={plan.dropped ?? []} />
      </div>

      <p className="plans-foot dim">
        {report.plans.current_setup_known && report.plans.base ? (
          <span>{t("eng.plans.resolvedOn", { file: report.plans.base.file })} </span>
        ) : (
          <span>{t("eng.plans.noSetup")} </span>
        )}
        <span>{t("eng.plans.disclaimer")}</span>
      </p>
    </section>
  );
}
