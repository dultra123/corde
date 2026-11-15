import { useRef, useState } from "react";
import {
  ELEMENT_GROUPS,
  type ElementGroup,
  getElementSpec,
  initialProps,
  listElementSpecs,
  type PropSpec,
  propsWithDefaults,
  SCALE_PROP,
} from "../dashdef/catalog";
import { isKeyLabel } from "../dashdef/elements";
import { FIELD_GROUPS, FIELDS, type FieldGroup } from "../dashdef/fields";
import {
  ACCEPT_IMAGES,
  ACCEPT_MEDIA,
  ACCEPT_VIDEOS,
  kindOfId,
  type MediaKind,
  prepareMedia,
  uploadMedia,
} from "../dashdef/media";
import {
  CONDITIONS,
  type ConditionKind,
  type DashCondition,
  type DashDefinition,
  type DashElement,
  type DashPageDef,
  type DashThreshold,
  FITS,
  FONTS,
  type FontId,
  MAX_BORDER_WIDTH,
  MAX_COLS,
  MAX_RADIUS,
  MAX_ROWS,
  MAX_THRESHOLDS,
  MIN_COLS,
  MIN_ROWS,
  OPS,
  type Op,
  THEME_KEYS,
  validAssetId,
  validColor,
} from "../dashdef/model";
import {
  addElement,
  duplicateElement,
  removeElement,
  removeElements,
  reorderElement,
  setAuthor,
  setBackground,
  setFont,
  setGrid,
  setProps,
  setShow,
  setStyle,
  setThemeColor,
  setThresholds,
  updateElement,
} from "../dashdef/ops";
import { fontOf } from "../dashdef/Renderer";
import { MediaView } from "../dashdef/visuals";
import { type TranslationKey, useT } from "../i18n";
import { DRAG_TYPE } from "./Canvas";
import { useEditor } from "./editor-store";
import { formatWords } from "./Format";
import { Field, fieldId, NumberInput } from "./inputs";
import { nameOf, textOf } from "./messages";

/** A slider with its value. */
function RangeInput({
  id,
  value,
  min,
  max,
  onChange,
}: {
  id: string;
  value: number;
  min: number;
  max: number;
  onChange: (v: number) => void;
}) {
  return (
    <div className="ins-range">
      <input
        id={id}
        type="range"
        min={min}
        max={max}
        step={1}
        value={value}
        onChange={(e) => onChange(Number(e.target.value))}
      />
      <output htmlFor={id} className="num">
        {value}
      </output>
    </div>
  );
}

/** A colour: « automatique » / « aucun » (the element's own) or a colour picked. */
function ColorInput({
  id,
  value,
  auto,
  autoLabel,
  initial = "#ffb454",
  onChange,
}: {
  id: string;
  value: unknown;
  auto: boolean;
  autoLabel?: string;
  initial?: string;
  onChange: (v: string) => void;
}) {
  const { t } = useT();
  const fixed = validColor(value);
  return (
    <div className="ins-color">
      {auto && (
        <select
          id={id}
          className="select"
          value={fixed ? "fixed" : "auto"}
          onChange={(e) => onChange(e.target.value === "auto" ? "auto" : initial)}
        >
          <option value="auto">{autoLabel ?? t("studio.color.auto")}</option>
          <option value="fixed">{t("studio.color.fixed")}</option>
        </select>
      )}
      {(fixed || !auto) && (
        <input
          id={auto ? undefined : id}
          type="color"
          className="ins-swatch"
          aria-label={t("studio.color.pick")}
          value={fixed ? (value as string).toLowerCase() : "#ffffff"}
          onChange={(e) => onChange(e.target.value)}
        />
      )}
    </div>
  );
}

function FieldSelect({
  id,
  value,
  fraction,
  onChange,
}: {
  id: string;
  value: unknown;
  fraction?: boolean;
  onChange: (v: string) => void;
}) {
  const { t, lang } = useT();
  const groups = Object.keys(FIELD_GROUPS) as FieldGroup[];
  return (
    <select
      id={id}
      className="select"
      value={typeof value === "string" ? value : ""}
      onChange={(e) => onChange(e.target.value)}
    >
      {typeof value === "string" && !FIELDS.some((f) => f.id === value) && (
        <option value={value}>{t("studio.unknownField")}</option>
      )}
      {groups.map((g) => {
        const fs = FIELDS.filter((f) => f.group === g && (!fraction || f.fraction));
        if (fs.length === 0) return null;
        return (
          <optgroup key={g} label={textOf(FIELD_GROUPS[g], lang, t)}>
            {fs.map((f) => (
              <option key={f.id} value={f.id}>
                {textOf(f.label, lang, t)}
              </option>
            ))}
          </optgroup>
        );
      })}
    </select>
  );
}

// ---------------------------------------------------------------- media

const MEDIA_ERR: Record<string, TranslationKey> = {
  type: "studio.media.err.type",
  "too-large": "studio.media.err.too-large",
  unreadable: "studio.media.err.unreadable",
  full: "studio.media.err.full",
  "pc-only": "studio.media.err.pc-only",
  offline: "studio.media.err.offline",
  refused: "studio.media.err.refused",
};

/**
 * Picks a picture or a video on the PC: read here, prepared (a picture encoded again without
 * its metadata, the metadata of an MP4 emptied), sent to the PC; the setting keeps its id.
 */
function MediaPicker({
  id,
  value,
  media,
  onChange,
}: {
  id: string;
  value: unknown;
  /** What may be picked: one kind, or both (a background). */
  media: MediaKind | "any";
  onChange: (assetId: string | null) => void;
}) {
  const { t } = useT();
  const input = useRef<HTMLInputElement>(null);
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<{ key: TranslationKey; bad: boolean } | null>(null);
  const has = validAssetId(value);
  const pick = async (file: File | undefined) => {
    if (!file) return;
    // Far too large to be a dashboard's: refused before it is read.
    if (file.size > 64 * 1024 * 1024) {
      setMsg({ key: "studio.media.err.too-large", bad: true });
      return;
    }
    setBusy(true);
    setMsg({ key: "studio.media.busy", bad: false });
    try {
      const prepared = await prepareMedia(new Uint8Array(await file.arrayBuffer()));
      if (!prepared.ok) {
        setMsg({ key: MEDIA_ERR[prepared.reason] ?? "studio.media.err.refused", bad: true });
        return;
      }
      if (media !== "any" && prepared.kind !== media) {
        setMsg({
          key: media === "image" ? "studio.media.err.wrongImage" : "studio.media.err.wrongVideo",
          bad: true,
        });
        return;
      }
      const sent = await uploadMedia(prepared.bytes);
      if (!sent.ok) {
        setMsg({ key: MEDIA_ERR[sent.reason] ?? "studio.media.err.refused", bad: true });
        return;
      }
      onChange(sent.id);
      setMsg({ key: "studio.media.ok", bad: false });
    } finally {
      setBusy(false);
    }
  };
  const accept =
    media === "image" ? ACCEPT_IMAGES : media === "video" ? ACCEPT_VIDEOS : ACCEPT_MEDIA;
  return (
    <div className="ins-media-wrap">
      <div className="ins-media">
        <div className="ins-media-thumb" aria-hidden="true">
          {has && <MediaView id={value} fit="cover" active={false} />}
        </div>
        <div className="ins-media-actions">
          <button
            id={id}
            type="button"
            className="btn"
            disabled={busy}
            onClick={() => input.current?.click()}
          >
            {has ? t("studio.media.replace") : t("studio.media.pick")}
          </button>
          {has && (
            <button
              type="button"
              className="btn"
              disabled={busy}
              onClick={() => {
                onChange(null);
                setMsg(null);
              }}
            >
              {t("studio.media.remove")}
            </button>
          )}
          <input
            ref={input}
            type="file"
            accept={accept}
            hidden
            onChange={(e) => {
              void pick(e.target.files?.[0]);
              e.target.value = "";
            }}
          />
        </div>
      </div>
      <p className={`ins-media-msg${msg?.bad ? " is-bad" : " dim"}`} role="status">
        {msg ? t(msg.key) : has ? "" : t("studio.media.none")}
      </p>
      {media !== "any" && (
        <p className="dim ins-media-msg">
          {t(media === "image" ? "studio.media.hintImage" : "studio.media.hintVideo")}
        </p>
      )}
    </div>
  );
}

function PropInput({
  spec,
  value,
  onChange,
}: {
  spec: PropSpec;
  value: unknown;
  onChange: (v: unknown) => void;
}) {
  const { t, lang } = useT();
  const id = fieldId(spec.key);
  const lab = textOf(spec.label, lang, t);
  const k = spec.type;
  switch (k.kind) {
    case "field":
      return (
        <Field label={lab} id={id}>
          <FieldSelect id={id} value={value} fraction={k.fraction} onChange={onChange} />
        </Field>
      );
    case "text":
      return (
        <Field label={lab} id={id}>
          <input
            id={id}
            className="input"
            maxLength={k.max}
            value={typeof value === "string" ? value : ""}
            onChange={(e) => onChange(e.target.value.replace(/[<>]/g, ""))}
          />
        </Field>
      );
    case "number":
      return (
        <Field label={lab} id={id}>
          <NumberInput
            id={id}
            value={typeof value === "number" ? value : Number(spec.default)}
            min={k.min}
            max={k.max}
            step={k.step}
            onChange={onChange}
          />
        </Field>
      );
    case "bool":
      return (
        <label className="ins-check">
          <input
            type="checkbox"
            checked={value === true}
            onChange={(e) => onChange(e.target.checked)}
          />
          {lab}
        </label>
      );
    case "color": {
      // A colour with a default of its own (`#…`) is always a colour; `auto` lets it choose.
      const auto = spec.default === "auto";
      return (
        <Field label={lab} id={id}>
          <ColorInput id={id} value={value} auto={auto} onChange={onChange} />
        </Field>
      );
    }
    case "choice":
      return (
        <Field label={lab} id={id}>
          <select
            id={id}
            className="select"
            value={typeof value === "string" ? value : String(spec.default)}
            onChange={(e) => onChange(e.target.value)}
          >
            {k.options.map((o) => (
              <option key={o.value} value={o.value}>
                {textOf(o.label, lang, t, o.label.fr === o.label.en && o.label.fr.includes("."))}
              </option>
            ))}
          </select>
        </Field>
      );
    case "media":
      return (
        <Field label={lab} id={id}>
          <MediaPicker id={id} value={value} media={k.media} onChange={(a) => onChange(a ?? "")} />
        </Field>
      );
  }
}

// ---------------------------------------------------------------- style, conditions, rules

function StyleSection({ page, el }: { page: DashPageDef; el: DashElement }) {
  const { t } = useT();
  const { apply } = useEditor.getState();
  const s = el.style ?? {};
  const set = (patch: Parameters<typeof setStyle>[3]) =>
    apply((d) => setStyle(d, page.id, el.id, patch));
  const ids = {
    bg: fieldId("bg"),
    op: fieldId("op"),
    rad: fieldId("rad"),
    bd: fieldId("bd"),
    bw: fieldId("bw"),
  };
  return (
    <section className="ins-section" aria-label={t("studio.style.title")}>
      <h3 className="ins-sub">{t("studio.style.title")}</h3>
      <Field label={t("studio.style.bg")} id={ids.bg}>
        <ColorInput
          id={ids.bg}
          value={s.bg}
          auto
          autoLabel={t("studio.style.none")}
          initial="#1a2230"
          onChange={(v) => set({ bg: validColor(v) ? v : undefined })}
        />
      </Field>
      <Field label={t("studio.style.opacity")} id={ids.op}>
        <RangeInput
          id={ids.op}
          min={0}
          max={100}
          value={Math.round((s.opacity ?? 1) * 100)}
          onChange={(v) => set({ opacity: v >= 100 ? undefined : v / 100 })}
        />
      </Field>
      <Field label={t("studio.style.radius")} id={ids.rad}>
        <RangeInput
          id={ids.rad}
          min={0}
          max={MAX_RADIUS}
          value={s.radius ?? 0}
          onChange={(v) => set({ radius: v > 0 ? v : undefined })}
        />
      </Field>
      <div className="ins-grid">
        <Field label={t("studio.style.border")} id={ids.bd}>
          <ColorInput
            id={ids.bd}
            value={s.border}
            auto
            autoLabel={t("studio.style.none")}
            initial="#ffb454"
            onChange={(v) =>
              set(
                validColor(v)
                  ? { border: v, border_width: s.border_width || 2 }
                  : { border: undefined, border_width: undefined },
              )
            }
          />
        </Field>
        <Field label={t("studio.style.borderWidth")} id={ids.bw}>
          <NumberInput
            id={ids.bw}
            min={0}
            max={MAX_BORDER_WIDTH}
            value={s.border_width ?? 0}
            onChange={(v) => set({ border_width: v > 0 ? v : undefined })}
          />
        </Field>
      </div>
    </section>
  );
}

const SHOW_KEY: Record<ConditionKind | "always", TranslationKey> = {
  always: "studio.show.always",
  limiter: "studio.show.limiter",
  pits: "studio.show.pits",
  yellow: "studio.show.yellow",
  blue: "studio.show.blue",
  field: "studio.show.field",
};

const OP_KEY: Record<Op, TranslationKey> = {
  lt: "studio.op.lt",
  le: "studio.op.le",
  ge: "studio.op.ge",
  gt: "studio.op.gt",
};

const OP_SIGN: Record<Op, string> = { lt: "<", le: "≤", ge: "≥", gt: ">" };

/** A comparison of the closed list; `compact`: its sign only (the words in its title). */
function OpSelect({
  value,
  onChange,
  label,
  compact = false,
}: {
  value: string;
  onChange: (v: Op) => void;
  label: string;
  compact?: boolean;
}) {
  const { t } = useT();
  return (
    <select
      className="select"
      aria-label={label}
      value={value}
      onChange={(e) => onChange(e.target.value as Op)}
    >
      {OPS.map((o) => (
        <option key={o} value={o} title={t(OP_KEY[o])}>
          {compact ? OP_SIGN[o] : `${OP_SIGN[o]}  ${t(OP_KEY[o])}`}
        </option>
      ))}
    </select>
  );
}

function ShowSection({ page, el }: { page: DashPageDef; el: DashElement }) {
  const { t } = useT();
  const { apply } = useEditor.getState();
  const c = el.show;
  const set = (next: DashCondition | null) => apply((d) => setShow(d, page.id, el.id, next));
  const ids = { when: fieldId("when"), field: fieldId("cf"), value: fieldId("cv") };
  return (
    <section className="ins-section" aria-label={t("studio.show.title")}>
      <h3 className="ins-sub">{t("studio.show.title")}</h3>
      <select
        id={ids.when}
        className="select"
        aria-label={t("studio.show.title")}
        value={c?.when ?? "always"}
        onChange={(e) => {
          const w = e.target.value as ConditionKind | "always";
          if (w === "always") set(null);
          else if (w === "field") set({ when: "field", field: "fuel_laps", op: "lt", value: 2 });
          else set({ when: w });
        }}
      >
        {(["always", ...CONDITIONS] as const).map((k) => (
          <option key={k} value={k}>
            {t(SHOW_KEY[k])}
          </option>
        ))}
      </select>
      {c?.when === "field" && (
        <>
          <FieldSelect id={ids.field} value={c.field} onChange={(f) => set({ ...c, field: f })} />
          <div className="ins-stack">
            <OpSelect
              label={t("studio.show.field")}
              value={c.op ?? "lt"}
              onChange={(op) => set({ ...c, op })}
            />
            <NumberInput
              id={ids.value}
              min={-1_000_000}
              max={1_000_000}
              step={0.1}
              value={c.value ?? 0}
              onChange={(v) => set({ ...c, value: v })}
            />
          </div>
        </>
      )}
      {c && <p className="dim ins-hint">{t("studio.show.hint")}</p>}
    </section>
  );
}

function RulesSection({ page, el }: { page: DashPageDef; el: DashElement }) {
  const { t } = useT();
  const { apply } = useEditor.getState();
  const rules = el.thresholds ?? [];
  const set = (next: DashThreshold[]) => apply((d) => setThresholds(d, page.id, el.id, next));
  return (
    <section className="ins-section" aria-label={t("studio.rules.title")}>
      <h3 className="ins-sub">{t("studio.rules.title")}</h3>
      {rules.map((r, i) => (
        <div key={i} className="ins-rule">
          <OpSelect
            compact
            label={t("studio.rules.if")}
            value={r.op}
            onChange={(op) => set(rules.map((x, j) => (j === i ? { ...x, op } : x)))}
          />
          <NumberInput
            id={fieldId(`rv${i}`)}
            min={-1_000_000}
            max={1_000_000}
            step={0.1}
            value={r.value}
            onChange={(v) => set(rules.map((x, j) => (j === i ? { ...x, value: v } : x)))}
          />
          <input
            type="color"
            className="ins-swatch"
            aria-label={t("studio.color.pick")}
            value={r.color.toLowerCase()}
            onChange={(e) =>
              set(rules.map((x, j) => (j === i ? { ...x, color: e.target.value } : x)))
            }
          />
          <button
            type="button"
            className="btn is-icon"
            aria-label={t("studio.rules.remove")}
            title={t("studio.rules.remove")}
            onClick={() => set(rules.filter((_, j) => j !== i))}
          >
            ✕
          </button>
        </div>
      ))}
      {rules.length < MAX_THRESHOLDS && (
        <button
          type="button"
          className="btn"
          onClick={() =>
            set([
              ...rules,
              rules.length === 0
                ? { op: "lt", value: 2, color: "#ff5a5a" }
                : { op: "lt", value: (rules[rules.length - 1]?.value ?? 0) * 2, color: "#ffb454" },
            ])
          }
        >
          + {t("studio.rules.add")}
        </button>
      )}
      <p className="dim ins-hint">
        {t("studio.rules.hint")} {el.type === "bar" ? t("studio.rules.hintBar") : ""}
      </p>
    </section>
  );
}

function ElementInspector({ page, el }: { page: DashPageDef; el: DashElement }) {
  const { t, lang } = useT();
  const { apply, select } = useEditor.getState();
  const spec = getElementSpec(el.type);
  const props = propsWithDefaults(el.type, el.props);
  const set = (key: string, v: unknown) => apply((d) => setProps(d, page.id, el.id, { [key]: v }));
  const rect = (patch: Partial<Pick<DashElement, "x" | "y" | "w" | "h">>) =>
    apply((d) => updateElement(d, page.id, el.id, patch));
  const ids = { x: fieldId("x"), y: fieldId("y"), w: fieldId("w"), h: fieldId("h") };
  return (
    <div className="ins">
      <h2 className="ins-title">{nameOf(spec, lang, t)}</h2>
      {!spec && <p className="dim">{t("studio.unknownHint", { type: el.type })}</p>}
      <div className="ins-grid">
        <Field label={t("studio.x")} id={ids.x}>
          <NumberInput
            id={ids.x}
            value={el.x + 1}
            min={1}
            max={page.cols}
            onChange={(v) => rect({ x: v - 1 })}
          />
        </Field>
        <Field label={t("studio.y")} id={ids.y}>
          <NumberInput
            id={ids.y}
            value={el.y + 1}
            min={1}
            max={page.rows}
            onChange={(v) => rect({ y: v - 1 })}
          />
        </Field>
        <Field label={t("studio.w")} id={ids.w}>
          <NumberInput
            id={ids.w}
            value={el.w}
            min={spec?.min.w ?? 1}
            max={page.cols}
            onChange={(v) => rect({ w: v })}
          />
        </Field>
        <Field label={t("studio.h")} id={ids.h}>
          <NumberInput
            id={ids.h}
            value={el.h}
            min={spec?.min.h ?? 1}
            max={page.rows}
            onChange={(v) => rect({ h: v })}
          />
        </Field>
      </div>
      {spec?.props.map((p) => (
        <PropInput key={p.key} spec={p} value={props[p.key]} onChange={(v) => set(p.key, v)} />
      ))}
      {spec && (
        <PropInput spec={SCALE_PROP} value={props.scale} onChange={(v) => set("scale", v)} />
      )}
      {spec?.thresholds && <RulesSection page={page} el={el} />}
      <ShowSection page={page} el={el} />
      <StyleSection page={page} el={el} />
      <div className="ins-actions">
        <button
          type="button"
          className="btn"
          onClick={() => {
            let nid: string | null = null;
            apply((d) => {
              const r = duplicateElement(d, page.id, el.id);
              nid = r.id;
              return r.def;
            });
            if (nid) select(nid);
          }}
        >
          {t("studio.duplicate")}
        </button>
        <button
          type="button"
          className="btn"
          onClick={() => apply((d) => reorderElement(d, page.id, el.id, "front"))}
        >
          {t("studio.front")}
        </button>
        <button
          type="button"
          className="btn"
          onClick={() => apply((d) => reorderElement(d, page.id, el.id, "back"))}
        >
          {t("studio.back")}
        </button>
        <button
          type="button"
          className="btn is-danger"
          onClick={() => {
            apply((d) => removeElement(d, page.id, el.id));
            select(null);
          }}
        >
          {t("studio.remove")}
        </button>
      </div>
      <p className="dim ins-hint">{t("studio.keysHint")}</p>
    </div>
  );
}

function MultiInspector({ page, ids }: { page: DashPageDef; ids: string[] }) {
  const { t } = useT();
  const { apply, select } = useEditor.getState();
  return (
    <div className="ins">
      <h2 className="ins-title">{t("studio.multi.title", { n: ids.length })}</h2>
      <p className="dim ins-hint">{t("studio.multi.hint")}</p>
      <div className="ins-actions">
        <button
          type="button"
          className="btn is-danger"
          onClick={() => {
            apply((d) => removeElements(d, page.id, ids));
            select(null);
          }}
        >
          {t("studio.multi.remove")}
        </button>
      </div>
    </div>
  );
}

const THEME_KEY: Record<(typeof THEME_KEYS)[number], TranslationKey> = {
  background: "studio.theme.background",
  panel: "studio.theme.panel",
  line: "studio.theme.line",
  text: "studio.theme.text",
  dim: "studio.theme.dim",
  accent: "studio.theme.accent",
};

const FONT_KEY: Record<FontId, TranslationKey> = {
  condensed: "studio.font.condensed",
  barlow: "studio.font.barlow",
  digital: "studio.font.digital",
  wide: "studio.font.wide",
};

const FIT_KEY = {
  contain: { fr: "Contenir (toute l'image)", en: "Contain (the whole picture)" },
  cover: { fr: "Couvrir (remplit, rogne)", en: "Cover (fills, crops)" },
  fill: { fr: "Étirer", en: "Stretch" },
} as const;

function BackgroundSection({ page }: { page: DashPageDef }) {
  const { t, lang } = useT();
  const { apply } = useEditor.getState();
  const bg = page.background ?? {};
  const set = (patch: Parameters<typeof setBackground>[2]) =>
    apply((d) => setBackground(d, page.id, patch));
  const ids = {
    color: fieldId("bgc"),
    media: fieldId("bgm"),
    fit: fieldId("bgf"),
    op: fieldId("bgo"),
  };
  return (
    <section className="ins-section" aria-label={t("studio.bg.title")}>
      <h3 className="ins-sub">{t("studio.bg.title")}</h3>
      <Field label={t("studio.bg.color")} id={ids.color}>
        <ColorInput
          id={ids.color}
          value={bg.color}
          auto
          autoLabel={t("studio.bg.theme")}
          initial="#101820"
          onChange={(v) => set({ color: validColor(v) ? v : undefined })}
        />
      </Field>
      <Field label={t("studio.bg.media")} id={ids.media}>
        <MediaPicker
          id={ids.media}
          value={bg.asset}
          media="any"
          onChange={(a) => set({ asset: a ?? undefined })}
        />
      </Field>
      {validAssetId(bg.asset) && (
        <div className="ins-grid">
          <Field label={t("studio.bg.fit")} id={ids.fit}>
            <select
              id={ids.fit}
              className="select"
              value={bg.fit ?? "cover"}
              onChange={(e) =>
                set({ fit: e.target.value === "cover" ? undefined : e.target.value })
              }
            >
              {FITS.map((f) => (
                <option key={f} value={f}>
                  {FIT_KEY[f][lang]}
                </option>
              ))}
            </select>
          </Field>
          <Field label={t("studio.bg.opacity")} id={ids.op}>
            <NumberInput
              id={ids.op}
              min={0}
              max={100}
              value={Math.round((bg.opacity ?? 1) * 100)}
              onChange={(v) => set({ opacity: v >= 100 ? undefined : v / 100 })}
            />
          </Field>
        </div>
      )}
      {kindOfId(bg.asset) === "video" && (
        <p className="dim ins-hint">{t("studio.media.hintVideo")}</p>
      )}
    </section>
  );
}

function DashInspector({
  def,
  page,
  onFormat,
}: {
  def: DashDefinition;
  page: DashPageDef;
  onFormat: () => void;
}) {
  const { t } = useT();
  const { apply } = useEditor.getState();
  const ids = {
    author: fieldId("author"),
    format: fieldId("format"),
    cols: fieldId("cols"),
    rows: fieldId("rows"),
    font: fieldId("font"),
  };
  return (
    <div className="ins">
      <h2 className="ins-title">{t("studio.dashSettings")}</h2>
      <Field label={t("studio.author")} id={ids.author}>
        <input
          id={ids.author}
          className="input"
          maxLength={60}
          value={def.author ?? ""}
          placeholder={t("studio.authorHint")}
          onChange={(e) => apply((d) => setAuthor(d, e.target.value.replace(/[<>]/g, "")))}
        />
      </Field>
      {/* The format is set in the bar of the editor (« Format »): here, what it is and the way there. */}
      <Field label={t("studio.format")} id={ids.format}>
        <div className="ins-format">
          <span className="num">{formatWords(def.format, t)}</span>
          <button id={ids.format} type="button" className="btn" onClick={onFormat}>
            {t("studio.format.change")}
          </button>
        </div>
      </Field>
      <h3 className="ins-sub">{t("studio.grid")}</h3>
      <div className="ins-grid">
        <Field label={t("studio.cols")} id={ids.cols}>
          <NumberInput
            id={ids.cols}
            value={page.cols}
            min={MIN_COLS}
            max={MAX_COLS}
            onChange={(v) => apply((d) => setGrid(d, page.id, v, page.rows))}
          />
        </Field>
        <Field label={t("studio.rows")} id={ids.rows}>
          <NumberInput
            id={ids.rows}
            value={page.rows}
            min={MIN_ROWS}
            max={MAX_ROWS}
            onChange={(v) => apply((d) => setGrid(d, page.id, page.cols, v))}
          />
        </Field>
      </div>
      <BackgroundSection page={page} />
      <h3 className="ins-sub">{t("studio.colors")}</h3>
      <div className="ins-theme">
        {THEME_KEYS.map((k) => {
          const id = fieldId(k);
          return (
            <div key={k} className="ins-theme-row">
              <input
                id={id}
                type="color"
                className="ins-swatch"
                value={def.theme[k].toLowerCase()}
                onChange={(e) => apply((d) => setThemeColor(d, k, e.target.value))}
              />
              <label htmlFor={id}>{t(THEME_KEY[k])}</label>
            </div>
          );
        })}
      </div>
      <Field label={t("studio.font")} id={ids.font}>
        <select
          id={ids.font}
          className="select"
          value={fontOf(def)}
          onChange={(e) => apply((d) => setFont(d, e.target.value as FontId))}
        >
          {FONTS.map((f) => (
            <option key={f} value={f}>
              {t(FONT_KEY[f])}
            </option>
          ))}
        </select>
      </Field>
      <p className="dim ins-hint">{t("studio.noSelectionHint")}</p>
    </div>
  );
}

export function Inspector({
  def,
  page,
  onFormat,
}: {
  def: DashDefinition;
  page: DashPageDef;
  /** Opens the « Format » panel of the editor's bar. */
  onFormat: () => void;
}) {
  const selection = useEditor((s) => s.selection);
  const ids = selection.filter((id) => page.elements.some((e) => e.id === id));
  const el = ids.length === 1 ? page.elements.find((e) => e.id === ids[0]) : undefined;
  return (
    <aside className="studio-side is-right" aria-label="Inspector">
      {el ? (
        <ElementInspector key={el.id} page={page} el={el} />
      ) : ids.length > 1 ? (
        <MultiInspector page={page} ids={ids} />
      ) : (
        <DashInspector def={def} page={page} onFormat={onFormat} />
      )}
    </aside>
  );
}

const GROUP_ORDER: ElementGroup[] = ["generic", "visual", "corde", "pages", "widgets"];

/** Lower case, without accents: « Vidéo » is found with « video ». */
function fold(s: string): string {
  return s.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();
}

/** The catalogue: a click adds the element at the first free place, a drag puts it where dropped. */
export function Palette({ page }: { page: DashPageDef }) {
  const { t, lang } = useT();
  const { apply, select } = useEditor.getState();
  const [query, setQuery] = useState("");
  const specs = listElementSpecs();
  const q = fold(query.trim());
  const found = (name: string) => !q || fold(name).includes(q);
  let shown = 0;
  return (
    <aside className="studio-side is-left" aria-label={t("studio.palette")}>
      <h2 className="ins-title">{t("studio.palette")}</h2>
      <input
        className="input pal-search"
        type="search"
        value={query}
        placeholder={t("studio.search")}
        aria-label={t("studio.search")}
        onChange={(e) => setQuery(e.target.value)}
      />
      <p className="dim ins-hint">{t("studio.paletteHint")}</p>
      {GROUP_ORDER.map((g) => {
        const list = specs.filter(
          (s) => s.group === g && found(textOf(s.name, lang, t, isKeyLabel(s))),
        );
        if (list.length === 0) return null;
        shown += list.length;
        return (
          <section key={g} className="pal-group">
            <h3 className="ins-sub">{textOf(ELEMENT_GROUPS[g], lang, t)}</h3>
            <ul className="pal-list">
              {list.map((s) => (
                <li key={s.type}>
                  <button
                    type="button"
                    className="chip pal-chip"
                    draggable
                    onDragStart={(e) => {
                      e.dataTransfer.setData(DRAG_TYPE, s.type);
                      e.dataTransfer.effectAllowed = "copy";
                    }}
                    onClick={() => {
                      let nid: string | null = null;
                      apply((d) => {
                        const r = addElement(
                          d,
                          page.id,
                          s.type,
                          undefined,
                          initialProps(s.type, lang),
                        );
                        nid = r.id;
                        return r.def;
                      });
                      if (nid) select(nid);
                    }}
                  >
                    <span>{textOf(s.name, lang, t, isKeyLabel(s))}</span>
                    <small className="num dim">
                      {s.size.w}×{s.size.h}
                    </small>
                  </button>
                </li>
              ))}
            </ul>
          </section>
        );
      })}
      {shown === 0 && <p className="dim ins-hint">{t("studio.searchNone")}</p>}
    </aside>
  );
}
