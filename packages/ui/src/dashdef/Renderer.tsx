import type { CSSProperties, ReactNode } from "react";
import { useRef } from "react";
import { setData } from "../dash/live";
import type { PageProps } from "../dash/parts";
import type { Lang } from "../store/settings";
import { useElementSize } from "../widgets/shared/hooks";
import { numProp, propsWithDefaults } from "./catalog";
import { readCtx, useElementDraw } from "./draw";
import {
  contextOf,
  ElementBoundary,
  elementComponent,
  pageComponent,
  UnknownElement,
} from "./elements";
import {
  aspectOf,
  type DashDefinition,
  type DashElement,
  type DashPageDef,
  type DashStyle,
  FITS,
  type FontId,
  type Fit as MediaFit,
  validColor,
} from "./model";
import { conditionHolds } from "./rules";
import { MediaView } from "./visuals";

/**
 * Draws one page of a definition at the scale of its box, keeping the proportions of its format
 * (`fill`: the whole box). The unit `--u` of the blocks comes from the size of the drawing, so
 * that a dashboard looks the same, only bigger or smaller, on any screen.
 */

/** The element type of a page that is one whole page of Corde over the whole grid, if it is. */
export function wholePageType(def: DashDefinition, page: DashPageDef): string | null {
  if (def.format.preset !== "fill" || page.elements.length !== 1 || page.background) return null;
  const el = page.elements[0] as DashElement;
  if (el.x !== 0 || el.y !== 0 || el.w !== page.cols || el.h !== page.rows) return null;
  return pageComponent(el.type) ? el.type : null;
}

export interface Fit {
  width: number;
  height: number;
  /** The unit of the blocks, px. */
  u: number;
}

/** The drawing size in a box, and its unit (the rule of `styles/dash.css`). */
export function fitStage(boxW: number, boxH: number, aspect: number | null): Fit {
  let width = Math.max(0, boxW);
  let height = Math.max(0, boxH);
  if (aspect !== null && width > 0 && height > 0) {
    if (width / height > aspect) width = height * aspect;
    else height = width / aspect;
  }
  const u =
    width >= height
      ? Math.min(height / 100, width * 0.0046)
      : Math.min(height * 0.005, width * 0.0092);
  return { width, height, u: Math.max(0, u) };
}

/** Where an element sits, in percent of the drawing. */
export function boxStyle(el: Pick<DashElement, "x" | "y" | "w" | "h">, page: DashPageDef) {
  return {
    left: `${(el.x / page.cols) * 100}%`,
    top: `${(el.y / page.rows) * 100}%`,
    width: `${(el.w / page.cols) * 100}%`,
    height: `${(el.h / page.rows) * 100}%`,
  };
}

/** The fonts of the numbers (ADR 0016): the two of Corde and two shipped with it. */
export const FONT_STACKS: Record<FontId, string> = {
  condensed: '"Barlow Semi Condensed", "Barlow", system-ui, sans-serif',
  barlow: '"Barlow", system-ui, sans-serif',
  digital: '"Share Tech Mono", ui-monospace, monospace',
  wide: '"Orbitron", "Barlow", system-ui, sans-serif',
};

export function fontOf(def: Pick<DashDefinition, "theme">): FontId {
  const f = def.theme.font;
  return f && f in FONT_STACKS ? (f as FontId) : "condensed";
}

export function themeStyle(def: DashDefinition): CSSProperties {
  const t = def.theme;
  const font = fontOf(def);
  return {
    "--d-bg": t.background,
    "--d-panel": t.panel,
    "--d-line": t.line,
    "--d-text": t.text,
    "--text-dim": t.dim,
    "--accent": t.accent,
    ...(font !== "condensed" ? { "--font-num": FONT_STACKS[font] } : {}),
  } as CSSProperties;
}

/**
 * The look of an element's box (ADR 0016). Sizes follow the unit of the drawing (`--su`), not
 * the text size of the element.
 */
export function boxLook(s: DashStyle | undefined): CSSProperties {
  if (!s) return {};
  const out: CSSProperties = {};
  if (validColor(s.bg)) out.backgroundColor = s.bg;
  if (typeof s.opacity === "number" && s.opacity >= 0 && s.opacity < 1) out.opacity = s.opacity;
  if (typeof s.radius === "number" && s.radius > 0) {
    out.borderRadius = `calc(var(--su) * ${(s.radius * 0.25).toFixed(2)})`;
    out.overflow = "hidden";
  }
  const w = typeof s.border_width === "number" ? s.border_width : 0;
  if (w > 0 && validColor(s.border)) {
    out.borderStyle = "solid";
    out.borderColor = s.border;
    (out as Record<string, string>)["--bw"] = `calc(var(--su) * ${(w * 0.25).toFixed(2)})`;
    out.borderWidth = "var(--bw)";
  }
  return out;
}

export function ElementView({
  el,
  data,
  lang,
  u,
}: {
  el: DashElement;
  data: PageProps;
  lang: Lang;
  u: number;
}) {
  const props = propsWithDefaults(el.type, el.props);
  const scale = numProp(props, "scale", 1, 0.5, 3);
  const Page = pageComponent(el.type);
  const Comp = elementComponent(el.type);
  const box = useRef<HTMLDivElement | null>(null);
  // « Afficher quand… »: followed on every frame, and at once when the condition changes.
  const show = el.show;
  useElementDraw(() => {
    setData(box.current, "off", show && !conditionHolds(show, readCtx()) ? "1" : "0");
  });
  let body: ReactNode;
  if (Page) body = <Page {...data} />;
  else if (Comp)
    body = <Comp type={el.type} props={props} data={data} lang={lang} thresholds={el.thresholds} />;
  else body = <UnknownElement type={el.type} lang={lang} />;
  return (
    <div
      ref={box}
      className={`del-in${Page ? " is-page" : ""}${el.style ? " has-look" : ""}`}
      data-ctx={contextOf(el.type)}
      data-off="0"
      style={{ "--u": `${(u * scale).toFixed(3)}px`, ...boxLook(el.style) } as CSSProperties}
    >
      <ElementBoundary type={el.type} lang={lang}>
        {body}
      </ElementBoundary>
    </div>
  );
}

function fitOf(v: unknown): MediaFit {
  return typeof v === "string" && (FITS as readonly string[]).includes(v)
    ? (v as MediaFit)
    : "cover";
}

/** The background of a page: its colour, its picture or video. */
function PageBackground({ page, active }: { page: DashPageDef; active: boolean }) {
  const bg = page.background;
  if (!bg) return null;
  return (
    <div
      className="dstage-bg"
      aria-hidden="true"
      style={validColor(bg.color) ? { background: bg.color } : undefined}
    >
      {bg.asset && (
        <MediaView
          id={bg.asset}
          fit={fitOf(bg.fit)}
          opacity={typeof bg.opacity === "number" ? bg.opacity : 1}
          active={active}
        />
      )}
    </div>
  );
}

interface StageProps {
  def: DashDefinition;
  page: DashPageDef;
  data: PageProps;
  lang: Lang;
  /** Drawn over the elements, in the same box (the editor's handles). */
  overlay?: (fit: Fit) => ReactNode;
  /** Grid lines (the editor). */
  grid?: boolean;
  /** The editor: an element hidden by its condition stays there, faint. */
  editing?: boolean;
}

export function Stage({
  def,
  page,
  data,
  lang,
  overlay,
  grid = false,
  editing = false,
}: StageProps) {
  const wrap = useRef<HTMLDivElement>(null);
  const size = useElementSize(wrap);
  // Until the box is measured: drawn at the size of the format, hidden (no jump, no flash).
  const measured = size.width > 0 && size.height > 0;
  const fit = measured
    ? fitStage(size.width, size.height, aspectOf(def))
    : fitStage(def.format.width, def.format.height, aspectOf(def));
  const gap = fit.u * 1.8;
  return (
    <div className="dstage-wrap" ref={wrap}>
      {fit.width > 0 && (
        <div
          className={`dstage${grid ? " has-grid" : ""}${editing ? " is-editing" : ""}`}
          data-font={fontOf(def)}
          style={
            {
              ...themeStyle(def),
              width: fit.width,
              height: fit.height,
              "--u": `${fit.u}px`,
              "--su": `${fit.u}px`,
              "--gap": `${gap / 2}px`,
              "--cols": page.cols,
              "--rows": page.rows,
              visibility: measured ? undefined : "hidden",
            } as CSSProperties
          }
        >
          <PageBackground page={page} active={data.active} />
          {page.elements.map((el) => (
            <div key={el.id} className="del" style={boxStyle(el, page)}>
              <ElementView el={el} data={data} lang={lang} u={fit.u} />
            </div>
          ))}
          {overlay?.(fit)}
        </div>
      )}
    </div>
  );
}

/** One page of a dashboard: a whole page of Corde as in 1.1.0, or the drawing of its elements. */
export function DashPage({
  def,
  page,
  data,
  lang,
}: {
  def: DashDefinition;
  page: DashPageDef;
  data: PageProps;
  lang: Lang;
}) {
  const whole = wholePageType(def, page);
  const Page = whole ? pageComponent(whole) : undefined;
  if (Page) return <Page {...data} />;
  return <Stage def={def} page={page} data={data} lang={lang} />;
}
