import { useEffect, useId, useRef, useState } from "react";
import { IconChevron } from "../dashboard/icons";
import {
  cellsRoughlySquare,
  ratioAllowed,
  shapePath,
  swapped,
  withPreset,
  withRadius,
  withShape,
  withSize,
} from "../dashdef/format";
import {
  type DashDefinition,
  type DashFormat,
  MAX_FORMAT_SIDE,
  MIN_FORMAT_SIDE,
  maxShapeRadius,
  PRESET_IDS,
  PRESETS,
  type PresetId,
  presetFormat,
  presetOf,
  radiusOf,
  SHAPES,
  type ShapeId,
  shapeOf,
} from "../dashdef/model";
import { adaptGrids, setFormat } from "../dashdef/ops";
import { type TranslationKey, useT } from "../i18n";
import { useEditor } from "./editor-store";
import { DraftNumber, Field } from "./inputs";

/**
 * The format of a dashboard (ADR 0018): its size and the shape of its page. One control for
 * the editor's bar (« Format : Rond 480 × 480 », a panel under it) and the same fields when a
 * dashboard is created in the library.
 */

type Translate = (key: TranslationKey, params?: Record<string, string | number>) => string;

const PRESET_NAME: Record<PresetId | "custom", TranslationKey> = {
  "phone-landscape": "studio.preset.phone-landscape",
  "phone-portrait": "studio.preset.phone-portrait",
  tablet: "studio.preset.tablet",
  "screen-16-9": "studio.preset.screen-16-9",
  "wheel-strip": "studio.preset.wheel-strip",
  "wheel-round": "studio.preset.wheel-round",
  square: "studio.preset.square",
  "screen-5-3": "studio.preset.screen-5-3",
  "screen-3-2": "studio.preset.screen-3-2",
  fill: "studio.preset.fill",
  custom: "studio.preset.custom",
};

const SHAPE_NAME: Record<ShapeId, TranslationKey> = {
  rect: "studio.shape.rect",
  rounded: "studio.shape.rounded",
  round: "studio.shape.round",
};

/** What the choice of a shape is drawn with (a glyph: never a format of a dashboard). */
const SHAPE_SAMPLE: Record<ShapeId, DashFormat> = {
  rect: { preset: "custom", width: 40, height: 26 },
  rounded: { preset: "custom", width: 40, height: 26, shape: "rounded", radius: 8 },
  round: { preset: "custom", width: 26, height: 26, shape: "round" },
};

/** « 852 × 393 » */
export function sizeWords(f: Pick<DashFormat, "width" | "height">): string {
  return `${f.width} × ${f.height}`;
}

/** A format in words: « 852 × 393 », « Rond 480 × 480 », « Coins arrondis 800 × 480 ». */
export function formatWords(f: DashFormat, t: Translate): string {
  if (f.preset === "fill") return t("studio.format.words.fill");
  const size = sizeWords(f);
  const shape = shapeOf(f);
  if (shape === "round")
    return t(f.width === f.height ? "studio.format.words.round" : "studio.format.words.oval", {
      size,
    });
  if (shape === "rounded") return t("studio.format.words.rounded", { size });
  return size;
}

/** A small drawing of a format: its proportions and its shape (`fill`: a dashed frame). */
export function FormatGlyph({ format, free = false }: { format: DashFormat; free?: boolean }) {
  const W = 40;
  const H = 26;
  if (format.preset === "fill" || free) {
    return (
      <svg className="fmt-glyph" viewBox={`0 0 ${W} ${H}`} aria-hidden="true">
        <rect className="is-free" x="2" y="2" width={W - 4} height={H - 4} rx="2" />
        {free ? (
          <path d="M16 13h8M20 9v8" />
        ) : (
          <path d="M9 9l-3 4 3 4M31 9l3 4-3 4M17 6l3-2.5L23 6M17 20l3 2.5 3-2.5" />
        )}
      </svg>
    );
  }
  const k = Math.min((W - 4) / format.width, (H - 4) / format.height);
  const w = format.width * k;
  const h = format.height * k;
  return (
    <svg className="fmt-glyph" viewBox={`0 0 ${W} ${H}`} aria-hidden="true">
      <path
        className="is-shape"
        d={shapePath(format)}
        transform={`translate(${((W - w) / 2).toFixed(2)} ${((H - h) / 2).toFixed(2)}) scale(${k.toFixed(5)})`}
      />
    </svg>
  );
}

interface FieldsProps {
  format: DashFormat;
  onChange: (next: DashFormat) => void;
  /** A size or the radius is being typed, then let go: the editor makes one step of it. */
  onEditStart?: () => void;
  onEditEnd?: () => void;
}

/** The fields of a format: the presets, the size, the shape and its radius. */
export function FormatFields({ format, onChange, onEditStart, onEditEnd }: FieldsProps) {
  const { t } = useT();
  const uid = useId();
  const ids = {
    w: `${uid}w`,
    h: `${uid}h`,
    r: `${uid}r`,
    rs: `${uid}rs`,
    hint: `${uid}hint`,
    shape: `${uid}shape`,
  };
  const preset = presetOf(format);
  const fill = preset === "fill";
  const shape = shapeOf(format);
  const maxR = maxShapeRadius(format.width, format.height);
  const [refused, setRefused] = useState(false);
  // The format when a size field was taken: what is typed is applied to it, not to the sizes
  // typed on the way (« 1000 » goes through « 100 », which would shrink the corners for good).
  const base = useRef<DashFormat | null>(null);
  const take = () => {
    base.current = format;
    onEditStart?.();
  };
  const leave = () => {
    base.current = null;
    setRefused(false);
    onEditEnd?.();
  };

  // A size is accepted when both sides are within their bounds and the proportions 8:1 at most.
  const size = (w: number, h: number): boolean => {
    const ok = ratioAllowed(w, h);
    setRefused(!ok);
    if (ok) onChange(withSize(base.current ?? format, w, h));
    return ok;
  };

  return (
    <div className="fmt">
      <fieldset className="fmt-group">
        <legend className="ins-sub">{t("studio.format.presets")}</legend>
        <div className="fmt-presets">
          {[...PRESET_IDS, "custom" as const].map((p) => {
            const on = p === preset;
            const sample = p === "custom" ? format : presetFormat(p);
            const s = p === "custom" ? null : PRESETS[p].shape;
            return (
              <button
                key={p}
                type="button"
                className={`fmt-preset${on ? " is-on" : ""}`}
                aria-pressed={on}
                data-preset={p}
                onClick={() => {
                  setRefused(false);
                  onChange(withPreset(format, p));
                }}
              >
                <FormatGlyph format={sample} free={p === "custom"} />
                <span className="fmt-preset-name">{t(PRESET_NAME[p])}</span>
                <small className="num dim">
                  {p === "fill"
                    ? t("studio.format.anySize")
                    : p === "custom"
                      ? t("studio.format.free")
                      : `${sizeWords(sample)}${s ? ` · ${t(SHAPE_NAME[s]).toLowerCase()}` : ""}`}
                </small>
              </button>
            );
          })}
        </div>
      </fieldset>

      <fieldset className="fmt-group" disabled={fill}>
        <legend className="ins-sub">{t("studio.format.size")}</legend>
        <div className="fmt-size">
          <Field label={t("studio.width")} id={ids.w}>
            <DraftNumber
              id={ids.w}
              value={format.width}
              min={MIN_FORMAT_SIDE}
              max={MAX_FORMAT_SIDE}
              describedBy={ids.hint}
              onCommit={(v) => size(v, format.height)}
              onFocus={take}
              onBlur={leave}
            />
          </Field>
          <button
            type="button"
            className="btn is-icon fmt-swap"
            aria-label={t("studio.format.swap")}
            title={t("studio.format.swap")}
            disabled={format.width === format.height}
            onClick={() => onChange(swapped(format))}
          >
            <svg viewBox="0 0 24 24" aria-hidden="true">
              <path
                d="M7 8h11l-3-3M17 16H6l3 3"
                fill="none"
                stroke="currentColor"
                strokeWidth="2"
                strokeLinecap="round"
                strokeLinejoin="round"
              />
            </svg>
          </button>
          <Field label={t("studio.height")} id={ids.h}>
            <DraftNumber
              id={ids.h}
              value={format.height}
              min={MIN_FORMAT_SIDE}
              max={MAX_FORMAT_SIDE}
              describedBy={ids.hint}
              onCommit={(v) => size(format.width, v)}
              onFocus={take}
              onBlur={leave}
            />
          </Field>
        </div>
        <p id={ids.hint} className={`ins-hint fmt-hint${refused ? " is-bad" : " dim"}`}>
          {fill ? t("studio.format.fillHint") : t("studio.format.sizeHint")}
        </p>
      </fieldset>

      <fieldset className="fmt-group" disabled={fill}>
        <legend className="ins-sub">{t("studio.format.shape")}</legend>
        <div className="fmt-shapes">
          {SHAPES.map((s) => {
            const on = s === shape;
            return (
              <label key={s} className={`fmt-shape${on ? " is-on" : ""}`}>
                <input
                  type="radio"
                  name={ids.shape}
                  value={s}
                  checked={on}
                  onChange={() => onChange(withShape(format, s))}
                />
                <FormatGlyph format={SHAPE_SAMPLE[s]} />
                <span>{t(SHAPE_NAME[s])}</span>
              </label>
            );
          })}
        </div>
        {shape === "rounded" && (
          <div className="fmt-radius">
            <Field label={t("studio.format.radius")} id={ids.r}>
              <div className="ins-range">
                <input
                  id={ids.rs}
                  type="range"
                  aria-label={t("studio.format.radius")}
                  min={1}
                  max={maxR}
                  step={1}
                  value={radiusOf(format)}
                  onFocus={onEditStart}
                  onBlur={onEditEnd}
                  onChange={(e) => onChange(withRadius(format, Number(e.target.value)))}
                />
                <DraftNumber
                  id={ids.r}
                  value={radiusOf(format)}
                  min={1}
                  max={maxR}
                  onCommit={(v) => {
                    onChange(withRadius(format, v));
                    return true;
                  }}
                  onFocus={onEditStart}
                  onBlur={onEditEnd}
                />
              </div>
            </Field>
          </div>
        )}
        {!fill && shape !== "rect" && (
          <p className="dim ins-hint fmt-hint">{t("studio.format.shapeHint")}</p>
        )}
      </fieldset>
    </div>
  );
}

/**
 * After a change of format the cells of a page may be far from square (a 24 × 12 grid on a
 * square page). Nothing is changed silently: the note says it, and offers the finer grid
 * (`adaptGrids`), where no element moves.
 */
export function GridAdvice({ def }: { def: DashDefinition }) {
  const { t, nOf } = useT();
  const long = def.pages.filter((p) => !cellsRoughlySquare(def.format, p));
  if (long.length === 0) return null;
  const first = long[0] as DashDefinition["pages"][number];
  const adapted = adaptGrids(def);
  const can = adapted !== def;
  const next = adapted.pages.find((p) => p.id === first.id) ?? first;
  return (
    <div className="fmt-advice">
      <p>
        {t("studio.format.gridNote", {
          pages: nOf(long.length, "studio.page", "studio.pages"),
          page: first.name,
          grid: `${first.cols} × ${first.rows}`,
        })}
      </p>
      {can ? (
        <>
          <button
            type="button"
            className="btn"
            onClick={() => useEditor.getState().apply((d) => adaptGrids(d))}
          >
            {t("studio.format.gridAdapt", { grid: `${next.cols} × ${next.rows}` })}
          </button>
          <p className="dim ins-hint">{t("studio.format.gridAdaptHint")}</p>
        </>
      ) : (
        <p className="dim ins-hint">{t("studio.format.gridManual")}</p>
      )}
    </div>
  );
}

/**
 * A size or the radius being typed: its changes are one step of « Annuler », from the field
 * taken to the field left (`begin` / `preview` / `end` of the editor, as a drag on the page).
 */
function startTyping(typing: { current: boolean }) {
  if (typing.current) return;
  typing.current = true;
  useEditor.getState().begin();
}

function endTyping(typing: { current: boolean }) {
  if (!typing.current) return;
  typing.current = false;
  useEditor.getState().end();
}

/**
 * The « Format » control of the editor's bar: the format in words, and its panel. Each change
 * is a step of « Annuler » (a size being typed: one step for the whole entry).
 */
export function FormatControl({
  def,
  open,
  onOpenChange,
}: {
  def: DashDefinition;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const { t } = useT();
  const root = useRef<HTMLDivElement>(null);
  const button = useRef<HTMLButtonElement>(null);
  const panel = useRef<HTMLDivElement>(null);
  const panelId = useId();
  const close = useRef(onOpenChange);
  close.current = onOpenChange;
  const typing = useRef(false);

  // Only when the panel opens or closes: a render in between (every change of the format is
  // one) must neither move the keyboard nor end the entry being typed.
  useEffect(() => {
    if (!open) return;
    // Opened (from the bar or from the properties): the keyboard goes to the format in use.
    const first =
      panel.current?.querySelector<HTMLElement>(".fmt-preset.is-on") ??
      panel.current?.querySelector<HTMLElement>("button, input");
    first?.focus();
    // Heard first (capture): a press outside ends the entry being typed before anything else
    // acts on it — a drag started on the page by that same press is a step of its own.
    const onDown = (e: PointerEvent) => {
      if (!root.current || root.current.contains(e.target as Node)) return;
      endTyping(typing);
      close.current(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      e.stopPropagation();
      endTyping(typing);
      close.current(false);
      button.current?.focus();
    };
    document.addEventListener("pointerdown", onDown, true);
    document.addEventListener("keydown", onKey, true);
    return () => {
      document.removeEventListener("pointerdown", onDown, true);
      document.removeEventListener("keydown", onKey, true);
      // A field still being typed in when the panel goes: its step ends here.
      endTyping(typing);
    };
  }, [open]);

  const change = (f: DashFormat) => {
    const st = useEditor.getState();
    if (typing.current && st.gesture) st.preview((d) => setFormat(d, f));
    else st.apply((d) => setFormat(d, f));
  };

  return (
    <div className="layoutmenu fmt-control" ref={root}>
      <button
        ref={button}
        type="button"
        className={`btn fmt-btn${open ? " is-open" : ""}`}
        aria-haspopup="dialog"
        aria-expanded={open}
        aria-controls={open ? panelId : undefined}
        title={t("studio.format.title")}
        onClick={() => onOpenChange(!open)}
      >
        <FormatGlyph format={def.format} />
        <span className="fmt-btn-label">{t("studio.format")}</span>
        <span className="fmt-btn-words num">{formatWords(def.format, t)}</span>
        <IconChevron />
      </button>
      {open && (
        <div
          ref={panel}
          id={panelId}
          className="menu fmt-panel"
          role="dialog"
          aria-label={t("studio.format.title")}
        >
          <FormatFields
            format={def.format}
            onChange={change}
            onEditStart={() => startTyping(typing)}
            onEditEnd={() => endTyping(typing)}
          />
          <GridAdvice def={def} />
          <div className="fmt-foot">
            <p className="dim ins-hint">{t("studio.format.keepHint")}</p>
            <button
              type="button"
              className="btn"
              onClick={() => {
                endTyping(typing);
                onOpenChange(false);
                button.current?.focus();
              }}
            >
              {t("common.close")}
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
