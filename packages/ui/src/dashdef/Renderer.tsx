import type { CSSProperties, ReactNode } from "react";
import { useRef } from "react";
import type { PageProps } from "../dash/parts";
import type { Lang } from "../store/settings";
import { useElementSize } from "../widgets/shared/hooks";
import { numProp, propsWithDefaults } from "./catalog";
import {
  contextOf,
  ElementBoundary,
  elementComponent,
  pageComponent,
  UnknownElement,
} from "./elements";
import { aspectOf, type DashDefinition, type DashElement, type DashPageDef } from "./model";

/**
 * Draws one page of a definition at the scale of its box, keeping the proportions of its format
 * (`fill`: the whole box). The unit `--u` of the blocks comes from the size of the drawing, so
 * that a dashboard looks the same, only bigger or smaller, on any screen.
 */

/** The element type of a page that is one whole page of Corde over the whole grid, if it is. */
export function wholePageType(def: DashDefinition, page: DashPageDef): string | null {
  if (def.format.preset !== "fill" || page.elements.length !== 1) return null;
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

export function themeStyle(def: DashDefinition): CSSProperties {
  const t = def.theme;
  return {
    "--d-bg": t.background,
    "--d-panel": t.panel,
    "--d-line": t.line,
    "--d-text": t.text,
    "--text-dim": t.dim,
    "--accent": t.accent,
  } as CSSProperties;
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
  let body: ReactNode;
  if (Page) body = <Page {...data} />;
  else if (Comp) body = <Comp type={el.type} props={props} data={data} lang={lang} />;
  else body = <UnknownElement type={el.type} lang={lang} />;
  return (
    <div
      className={`del-in${Page ? " is-page" : ""}`}
      data-ctx={contextOf(el.type)}
      style={{ "--u": `${(u * scale).toFixed(3)}px` } as CSSProperties}
    >
      <ElementBoundary type={el.type} lang={lang}>
        {body}
      </ElementBoundary>
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
}

export function Stage({ def, page, data, lang, overlay, grid = false }: StageProps) {
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
          className={`dstage${grid ? " has-grid" : ""}`}
          style={
            {
              ...themeStyle(def),
              width: fit.width,
              height: fit.height,
              "--u": `${fit.u}px`,
              "--gap": `${gap / 2}px`,
              "--cols": page.cols,
              "--rows": page.rows,
              visibility: measured ? undefined : "hidden",
            } as CSSProperties
          }
        >
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
