import { type KeyboardEvent, type PointerEvent, useRef } from "react";
import type { PageProps } from "../dash/parts";
import { getElementSpec } from "../dashdef/catalog";
import type { DashDefinition, DashPageDef } from "../dashdef/model";
import {
  addElement,
  duplicateElement,
  moveElement,
  removeElement,
  resizeElement,
  updateElement,
} from "../dashdef/ops";
import { boxStyle, type Fit, Stage } from "../dashdef/Renderer";
import { useT } from "../i18n";
import { useEditor } from "./editor-store";
import { nameOf } from "./messages";

/** The MIME type of an element dragged from the palette. */
export const DRAG_TYPE = "application/x-corde-element";

interface Gesture {
  mode: "move" | "resize";
  id: string;
  px: number;
  py: number;
  x: number;
  y: number;
  w: number;
  h: number;
  cellW: number;
  cellH: number;
}

/**
 * The page being edited, at the scale of the format, with a box over each element: click to
 * choose, drag to move, the corner to resize (all on whole cells), the keys of the keyboard for
 * the same (see `onKey`). The elements under the boxes are drawn live.
 */
export function Canvas({
  def,
  page,
  data,
}: {
  def: DashDefinition;
  page: DashPageDef;
  data: PageProps;
}) {
  const { t, lang } = useT();
  const selected = useEditor((s) => s.selected);
  const { apply, begin, preview, end, select } = useEditor.getState();
  const gesture = useRef<Gesture | null>(null);
  const layer = useRef<HTMLDivElement | null>(null);

  const start = (e: PointerEvent, id: string, mode: Gesture["mode"], fit: Fit) => {
    const el = page.elements.find((x) => x.id === id);
    if (!el || e.button !== 0) return;
    e.stopPropagation();
    e.preventDefault();
    select(id);
    (e.currentTarget as HTMLElement).setPointerCapture?.(e.pointerId);
    begin();
    gesture.current = {
      mode,
      id,
      px: e.clientX,
      py: e.clientY,
      x: el.x,
      y: el.y,
      w: el.w,
      h: el.h,
      cellW: fit.width / page.cols,
      cellH: fit.height / page.rows,
    };
  };

  const move = (e: PointerEvent) => {
    const g = gesture.current;
    if (!g) return;
    const dx = Math.round((e.clientX - g.px) / g.cellW);
    const dy = Math.round((e.clientY - g.py) / g.cellH);
    preview((d) =>
      updateElement(
        d,
        page.id,
        g.id,
        g.mode === "move" ? { x: g.x + dx, y: g.y + dy } : { w: g.w + dx, h: g.h + dy },
      ),
    );
  };

  const stop = () => {
    if (!gesture.current) return;
    gesture.current = null;
    end();
  };

  const onKey = (e: KeyboardEvent) => {
    const id = useEditor.getState().selected;
    if (!id || (e.target as HTMLElement).closest("input, select, textarea")) return;
    const arrows: Record<string, [number, number]> = {
      ArrowLeft: [-1, 0],
      ArrowRight: [1, 0],
      ArrowUp: [0, -1],
      ArrowDown: [0, 1],
    };
    const a = arrows[e.key];
    if (a) {
      e.preventDefault();
      apply((d) =>
        e.shiftKey
          ? resizeElement(d, page.id, id, a[0], a[1])
          : moveElement(d, page.id, id, a[0], a[1]),
      );
    } else if (e.key === "Delete" || e.key === "Backspace") {
      e.preventDefault();
      apply((d) => removeElement(d, page.id, id));
      select(null);
    } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "d") {
      e.preventDefault();
      let nid: string | null = null;
      apply((d) => {
        const r = duplicateElement(d, page.id, id);
        nid = r.id;
        return r.def;
      });
      if (nid) select(nid);
    } else if (e.key === "Escape") {
      select(null);
    }
  };

  const onDrop = (e: React.DragEvent, fit: Fit) => {
    const type = e.dataTransfer.getData(DRAG_TYPE);
    if (!type || !layer.current) return;
    e.preventDefault();
    const box = layer.current.getBoundingClientRect();
    const spec = getElementSpec(type);
    const w = spec?.size.w ?? 4;
    const h = spec?.size.h ?? 3;
    const x = Math.round((e.clientX - box.left) / (fit.width / page.cols) - w / 2);
    const y = Math.round((e.clientY - box.top) / (fit.height / page.rows) - h / 2);
    let nid: string | null = null;
    apply((d) => {
      const r = addElement(d, page.id, type, { x: Math.max(0, x), y: Math.max(0, y) });
      nid = r.id;
      return r.def;
    });
    if (nid) select(nid);
  };

  return (
    <div
      className="studio-canvas dash is-canvas"
      role="application"
      aria-label={t("studio.canvasLabel")}
      onKeyDown={onKey}
    >
      <Stage
        def={def}
        page={page}
        data={data}
        lang={lang}
        grid
        overlay={(fit) => (
          <div
            ref={layer}
            className="sel-layer"
            onPointerDown={(e) => {
              if (e.target === e.currentTarget) select(null);
            }}
            onPointerMove={move}
            onPointerUp={stop}
            onPointerCancel={stop}
            onDragOver={(e) => {
              if (e.dataTransfer.types.includes(DRAG_TYPE)) e.preventDefault();
            }}
            onDrop={(e) => onDrop(e, fit)}
          >
            {page.elements.map((el) => {
              const name = nameOf(getElementSpec(el.type), lang, t);
              const on = el.id === selected;
              return (
                <button
                  key={el.id}
                  type="button"
                  className={`sel-box${on ? " is-on" : ""}`}
                  style={boxStyle(el, page)}
                  aria-pressed={on}
                  aria-label={`${name} — ${el.x + 1}, ${el.y + 1} · ${el.w} × ${el.h}`}
                  title={name}
                  onFocus={() => select(el.id)}
                  onPointerDown={(e) => start(e, el.id, "move", fit)}
                >
                  {on && (
                    <>
                      <span className="sel-name">{name}</span>
                      <span className="sel-size num">
                        {el.w} × {el.h}
                      </span>
                      <span
                        className="sel-handle"
                        aria-hidden="true"
                        onPointerDown={(e) => start(e, el.id, "resize", fit)}
                      />
                    </>
                  )}
                </button>
              );
            })}
          </div>
        )}
      />
    </div>
  );
}
