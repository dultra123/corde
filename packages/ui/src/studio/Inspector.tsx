import type { ReactNode } from "react";
import {
  ELEMENT_GROUPS,
  type ElementGroup,
  getElementSpec,
  listElementSpecs,
  type PropSpec,
  propsWithDefaults,
  SCALE_PROP,
} from "../dashdef/catalog";
import { isKeyLabel } from "../dashdef/elements";
import { FIELD_GROUPS, FIELDS, type FieldGroup } from "../dashdef/fields";
import {
  type DashDefinition,
  type DashElement,
  type DashPageDef,
  MAX_COLS,
  MAX_FORMAT_SIDE,
  MAX_ROWS,
  MIN_COLS,
  MIN_FORMAT_SIDE,
  MIN_ROWS,
  PRESET_IDS,
  type PresetId,
  THEME_KEYS,
  validColor,
} from "../dashdef/model";
import {
  addElement,
  duplicateElement,
  removeElement,
  reorderElement,
  setAuthor,
  setGrid,
  setPreset,
  setProps,
  setThemeColor,
  updateElement,
} from "../dashdef/ops";
import { type TranslationKey, useT } from "../i18n";
import { DRAG_TYPE } from "./Canvas";
import { useEditor } from "./editor-store";
import { nameOf, textOf } from "./messages";

let uid = 0;
function fieldId(key: string): string {
  uid += 1;
  return `ins-${key}-${uid}`;
}

function Field({ label, children, id }: { label: string; children: ReactNode; id: string }) {
  return (
    <div className="ins-field">
      <label htmlFor={id}>{label}</label>
      {children}
    </div>
  );
}

function NumberInput({
  id,
  value,
  min,
  max,
  step = 1,
  onChange,
}: {
  id: string;
  value: number;
  min: number;
  max: number;
  step?: number;
  onChange: (v: number) => void;
}) {
  return (
    <input
      id={id}
      className="input num"
      type="number"
      value={value}
      min={min}
      max={max}
      step={step}
      onChange={(e) => {
        const v = Number(e.target.value);
        if (Number.isFinite(v)) onChange(Math.max(min, Math.min(max, v)));
      }}
    />
  );
}

/** A colour: « automatique » (the element's own) or a colour picked. */
function ColorInput({
  id,
  value,
  auto,
  onChange,
}: {
  id: string;
  value: unknown;
  auto: boolean;
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
          onChange={(e) => onChange(e.target.value === "auto" ? "auto" : "#ffb454")}
        >
          <option value="auto">{t("studio.color.auto")}</option>
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
    case "field": {
      const groups = Object.keys(FIELD_GROUPS) as FieldGroup[];
      return (
        <Field label={lab} id={id}>
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
              const fs = FIELDS.filter((f) => f.group === g && (!k.fraction || f.fraction));
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
        </Field>
      );
    }
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
    case "color":
      return (
        <Field label={lab} id={id}>
          <ColorInput id={id} value={value} auto onChange={onChange} />
        </Field>
      );
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
  }
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

const PRESET_KEY: Record<PresetId | "custom", TranslationKey> = {
  "phone-landscape": "studio.preset.phone-landscape",
  "phone-portrait": "studio.preset.phone-portrait",
  tablet: "studio.preset.tablet",
  "screen-16-9": "studio.preset.screen-16-9",
  "wheel-strip": "studio.preset.wheel-strip",
  fill: "studio.preset.fill",
  custom: "studio.preset.custom",
};

const THEME_KEY: Record<(typeof THEME_KEYS)[number], TranslationKey> = {
  background: "studio.theme.background",
  panel: "studio.theme.panel",
  line: "studio.theme.line",
  text: "studio.theme.text",
  dim: "studio.theme.dim",
  accent: "studio.theme.accent",
};

function DashInspector({ def, page }: { def: DashDefinition; page: DashPageDef }) {
  const { t } = useT();
  const { apply } = useEditor.getState();
  const ids = {
    author: fieldId("author"),
    preset: fieldId("preset"),
    fw: fieldId("fw"),
    fh: fieldId("fh"),
    cols: fieldId("cols"),
    rows: fieldId("rows"),
  };
  const preset = (PRESET_IDS as string[]).includes(def.format.preset)
    ? def.format.preset
    : "custom";
  const setSize = (width: number, height: number) =>
    apply((d) => {
      const r = width / height;
      if (r < 1 / 8 || r > 8) return d;
      return { ...d, format: { preset: "custom", width, height }, updated_ms: Date.now() };
    });
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
      <Field label={t("studio.format")} id={ids.preset}>
        <select
          id={ids.preset}
          className="select"
          value={preset}
          onChange={(e) => apply((d) => setPreset(d, e.target.value as PresetId | "custom"))}
        >
          {[...PRESET_IDS, "custom" as const].map((p) => (
            <option key={p} value={p}>
              {t(PRESET_KEY[p])}
            </option>
          ))}
        </select>
      </Field>
      {preset === "custom" && (
        <div className="ins-grid">
          <Field label={t("studio.width")} id={ids.fw}>
            <NumberInput
              id={ids.fw}
              value={def.format.width}
              min={MIN_FORMAT_SIDE}
              max={MAX_FORMAT_SIDE}
              onChange={(v) => setSize(v, def.format.height)}
            />
          </Field>
          <Field label={t("studio.height")} id={ids.fh}>
            <NumberInput
              id={ids.fh}
              value={def.format.height}
              min={MIN_FORMAT_SIDE}
              max={MAX_FORMAT_SIDE}
              onChange={(v) => setSize(def.format.width, v)}
            />
          </Field>
        </div>
      )}
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
      <p className="dim ins-hint">{t("studio.noSelectionHint")}</p>
    </div>
  );
}

export function Inspector({ def, page }: { def: DashDefinition; page: DashPageDef }) {
  const selected = useEditor((s) => s.selected);
  const el = page.elements.find((e) => e.id === selected);
  return (
    <aside className="studio-side is-right" aria-label="Inspector">
      {el ? (
        <ElementInspector key={el.id} page={page} el={el} />
      ) : (
        <DashInspector def={def} page={page} />
      )}
    </aside>
  );
}

const GROUP_ORDER: ElementGroup[] = ["generic", "corde", "pages", "widgets"];

/** The catalogue: a click adds the element at the first free place, a drag puts it where dropped. */
export function Palette({ page }: { page: DashPageDef }) {
  const { t, lang } = useT();
  const { apply, select } = useEditor.getState();
  const specs = listElementSpecs();
  return (
    <aside className="studio-side is-left" aria-label={t("studio.palette")}>
      <h2 className="ins-title">{t("studio.palette")}</h2>
      <p className="dim ins-hint">{t("studio.paletteHint")}</p>
      {GROUP_ORDER.map((g) => {
        const list = specs.filter((s) => s.group === g);
        if (list.length === 0) return null;
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
                        const r = addElement(d, page.id, s.type);
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
    </aside>
  );
}
