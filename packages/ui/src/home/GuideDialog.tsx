import { useEffect, useRef } from "react";
import { useT } from "../i18n";

const STEPS = [1, 2, 3, 4, 5] as const;

/**
 * The quick-start guide, inside the app: in the desktop window a link to a web page would
 * replace the whole interface, with no way back.
 */
export function GuideDialog({ open, onClose }: { open: boolean; onClose: () => void }) {
  const { t } = useT();
  const ref = useRef<HTMLDialogElement>(null);

  useEffect(() => {
    const d = ref.current;
    if (!d) return;
    if (open && !d.open) d.showModal();
    else if (!open && d.open) d.close();
  }, [open]);

  return (
    <dialog className="sheet guide" ref={ref} onClose={onClose} aria-label={t("guide.title")}>
      <div className="sheet-head">{t("guide.title")}</div>
      <div className="sheet-body">
        <p className="guide-intro dim">{t("guide.intro")}</p>
        <ol className="guide-steps">
          {STEPS.map((n) => (
            <li key={n} className="guide-step">
              <b className="guide-step-title">{t(`guide.step${n}.title`)}</b>
              <span className="dim">{t(`guide.step${n}.body`)}</span>
            </li>
          ))}
        </ol>
        <div className="guide-trouble">
          <b className="guide-trouble-title">{t("guide.trouble.title")}</b>
          <p className="dim">{t("guide.trouble.body")}</p>
        </div>
        <p className="guide-options faint">{t("guide.options")}</p>
      </div>
      <div className="sheet-foot">
        <button type="button" className="btn is-active" onClick={onClose}>
          {t("common.close")}
        </button>
      </div>
    </dialog>
  );
}
