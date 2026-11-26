import { useEffect, useRef } from "react";
import { downloadBytes } from "../dashdef/archive";
import { getElementSpec } from "../dashdef/catalog";
import { getField, label } from "../dashdef/fields";
import type { DashDefinition } from "../dashdef/model";
import { MZDASH_SUFFIX, type MzdashNote, type MzdashResult, toMzdash } from "../dashdef/mzdash";
import { type TranslationKey, useT } from "../i18n";
import type { Lang } from "../store/settings";

/**
 * « Exporter pour le volant MOZA » (ADR 0021): the dashboard written as a file for the screen
 * of the wheel, handed to the browser as a download, and what the driver must know about it —
 * what was left out and why, where the file goes, and that the wheel is fed by the software
 * that drives it, not by Corde. Nothing is sent anywhere and nothing is written on the PC
 * besides that download.
 */

/** A random GUID. `crypto.randomUUID` is absent on a page served over plain HTTP. */
function guid(): string {
  const b = new Uint8Array(16);
  crypto.getRandomValues(b);
  b[6] = ((b[6] ?? 0) & 0x0f) | 0x40;
  b[8] = ((b[8] ?? 0) & 0x3f) | 0x80;
  const h = [...b].map((x) => x.toString(16).padStart(2, "0")).join("");
  return `${h.slice(0, 8)}-${h.slice(8, 12)}-${h.slice(12, 16)}-${h.slice(16, 20)}-${h.slice(20)}`;
}

/** Writes the file and hands it to the browser. */
export function exportForMoza(def: DashDefinition, lang: Lang): MzdashResult {
  const r = toMzdash(def, { lang, now: Date.now() / 1000, guid: guid() });
  downloadBytes(`${r.name}${MZDASH_SUFFIX}`, new TextEncoder().encode(r.json), "application/json");
  return r;
}

const REASON: Record<MzdashNote["kind"], TranslationKey> = {
  element: "studio.moza.note.element",
  field: "studio.moza.note.field",
  condition: "studio.moza.note.condition",
  media: "studio.moza.note.media",
  partial: "studio.moza.note.partial",
  overflow: "studio.moza.note.overflow",
};

type Translate = (key: TranslationKey, params?: Record<string, string | number>) => string;

/** One note as a line: the page, the element, and why. */
export function noteLine(n: MzdashNote, t: Translate, lang: Lang): string {
  const spec = getElementSpec(n.element);
  const element =
    n.element === "background"
      ? t("studio.moza.background")
      : spec
        ? label(spec.name, lang)
        : n.element;
  const field = getField(n.what.replace(/^clock:/, ""));
  const what = n.what.startsWith("clock:")
    ? t("studio.moza.note.clock")
    : field
      ? label(field.label, lang)
      : n.what;
  const reason =
    n.kind === "partial" && n.what === "arc"
      ? t("studio.moza.note.arc")
      : t(REASON[n.kind], { what });
  // French puts a space before the colon, English does not.
  return `${n.page} · ${element}${lang === "en" ? ":" : " :"} ${reason}`;
}

/** The notes as lines, the same line said once with its count. */
export function noteLines(notes: readonly MzdashNote[], t: Translate, lang: Lang): string[] {
  const count = new Map<string, number>();
  for (const n of notes) {
    const line = noteLine(n, t, lang);
    count.set(line, (count.get(line) ?? 0) + 1);
  }
  return [...count].map(([line, n]) => (n > 1 ? `${line} (× ${n})` : line));
}

export function MozaExportDialog({
  result,
  onClose,
}: {
  result: MzdashResult | null;
  onClose: () => void;
}) {
  const { t, lang } = useT();
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    const d = ref.current;
    if (!d) return;
    if (result && !d.open) d.showModal();
    else if (!result && d.open) d.close();
  }, [result]);
  const file = result ? `${result.name}${MZDASH_SUFFIX}` : "";
  const lines = result ? noteLines(result.notes, t, lang) : [];
  return (
    <dialog
      className="sheet studio-moza"
      ref={ref}
      onClose={onClose}
      aria-label={t("studio.moza.title")}
    >
      {result && (
        <>
          <div className="sheet-head">{t("studio.moza.title")}</div>
          <div className="sheet-body">
            <p>{t("studio.moza.done", { file, written: result.written, total: result.total })}</p>
            {lines.length > 0 && (
              <>
                <h3 className="ins-sub">{t("studio.moza.left", { n: lines.length })}</h3>
                <ul className="studio-moza-notes">
                  {lines.map((l) => (
                    <li key={l}>{l}</li>
                  ))}
                </ul>
              </>
            )}
            <h3 className="ins-sub">{t("studio.moza.howTitle")}</h3>
            <ol className="studio-moza-how">
              <li>{t("studio.moza.how1")}</li>
              <li>{t("studio.moza.how2", { name: result.name })}</li>
              <li>{t("studio.moza.how3")}</li>
            </ol>
            <p className="dim">{t("studio.moza.data")}</p>
            <p className="dim">{t("studio.moza.experimental")}</p>
          </div>
          <div className="sheet-foot">
            <button type="button" className="btn is-primary" onClick={onClose}>
              {t("common.close")}
            </button>
          </div>
        </>
      )}
    </dialog>
  );
}
