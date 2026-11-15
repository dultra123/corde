import { type KeyboardEvent, type PointerEvent, useRef, useState } from "react";
import type { PageProps } from "../dash/parts";
import { getElementSpec, initialProps } from "../dashdef/catalog";
import type { DashDefinition, DashPageDef } from "../dashdef/model";
import {
  addElement,
  alignGuides,
  duplicateElement,
  elementsIn,
  moveElements,
  type Rect,
  removeElements,
  resizeElement,
  updateElement,
} from "../dashdef/ops";
import { boxStyle, type Fit, Stage } from "../dashdef/Renderer";
import { useT } from "../i18n";
import { useEditor } from "./editor-store";
import { nameOf } from "./messages";

/** The MIME type of an element dragged from the palette. */
export const DRAG_TYPE = "application/x-corde-element";

type Gesture =
  | {
      mode: "move";
      ids: string[];
      base: DashDefinition;
      px: number;
      py: number;
      cellW: number;
      cellH: number;
    }
  | {
      mode: "resize";
      id: string;
      base: DashDefinition;
      px: number;
      py: number;
      w: number;
      h: number;
      cellW: number;
      cellH: number;
    }
  | {
      mode: "frame";
      /** Where the frame started, in cells (fractions kept). */
      x0: number;
      y0: number;
      add: boolean;
      before: string[];
      cellW: number;
      cellH: number;
      left: number;
      top: number;
    };

interface Guides {
  x: number[];
  y: number[];
}

/** The rectangle of whole cells two corners (in cells) span. */
function cellRect(x0: number, y0: number, x1: number, y1: number): Rect {
  const x = Math.floor(Math.min(x0, x1));
  const y = Math.floor(Math.min(y0, y1));
  return {
    x,
    y,
    w: Math.max(1, Math.ceil(Math.max(x0, x1)) - x),
    h: Math.max(1, Math.ceil(Math.max(y0, y1)) - y),
  };
}

/**
 * The page being edited, at the scale of the format, with a box over each element: click to
 * choose (Maj + clic: several), a frame drawn on the page to choose all it touches, drag to move
 * (the whole selection together, with guides where edges and centres line up), the corner to
 * resize (all on whole cells), the keys of the keyboard for the same (see `onKey`). The elements
 * under the boxes are drawn live.
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
  const selection = useEditor((s) => s.selection);
  const { apply, begin, preview, end, select, toggle, selectMany } = useEditor.getState();
  const gesture = useRef<Gesture | null>(null);
  const layer = useRef<HTMLDivElement | null>(null);
  const [guides, setGuides] = useState<Guides | null>(null);
  const [frame, setFrame] = useState<Rect | null>(null);

  const start = (e: PointerEvent, id: string, mode: "move" | "resize", fit: Fit) => {
    const el = page.elements.find((x) => x.id === id);
    if (!el || e.button !== 0) return;
    e.stopPropagation();
    e.preventDefault();
    if (mode === "move" && e.shiftKey) {
      toggle(id);
      return;
    }
    const cur = useEditor.getState().selection;
    const ids = mode === "move" && cur.includes(id) ? cur : [id];
    if (!(mode === "move" && cur.includes(id))) select(id);
    (e.currentTarget as HTMLElement).setPointerCapture?.(e.pointerId);
    begin();
    const base = useEditor.getState().h?.present ?? def;
    const cellW = fit.width / page.cols;
    const cellH = fit.height / page.rows;
    gesture.current =
      mode === "move"
        ? { mode, ids, base, px: e.clientX, py: e.clientY, cellW, cellH }
        : { mode, id, base, px: e.clientX, py: e.clientY, w: el.w, h: el.h, cellW, cellH };
  };

  const startFrame = (e: PointerEvent, fit: Fit) => {
    if (e.button !== 0 || e.target !== e.currentTarget || !layer.current) return;
    const box = layer.current.getBoundingClientRect();
    const cellW = fit.width / page.cols;
    const cellH = fit.height / page.rows;
    const add = e.shiftKey;
    if (!add) select(null);
    layer.current.setPointerCapture?.(e.pointerId);
    gesture.current = {
      mode: "frame",
      x0: (e.clientX - box.left) / cellW,
      y0: (e.clientY - box.top) / cellH,
      add,
      before: add ? useEditor.getState().selection : [],
      cellW,
      cellH,
      left: box.left,
      top: box.top,
    };
  };

  const move = (e: PointerEvent) => {
    const g = gesture.current;
    if (!g) return;
    if (g.mode === "frame") {
      const x1 = (e.clientX - g.left) / g.cellW;
      const y1 = (e.clientY - g.top) / g.cellH;
      // A click without a drag only clears the selection.
      if (Math.abs(x1 - g.x0) < 0.2 && Math.abs(y1 - g.y0) < 0.2) return;
      const r = cellRect(
        Math.max(0, Math.min(page.cols, g.x0)),
        Math.max(0, Math.min(page.rows, g.y0)),
        Math.max(0, Math.min(page.cols, x1)),
        Math.max(0, Math.min(page.rows, y1)),
      );
      setFrame(r);
      selectMany([...g.before, ...elementsIn(page, r)]);
      return;
    }
    const dx = Math.round((e.clientX - g.px) / g.cellW);
    const dy = Math.round((e.clientY - g.py) / g.cellH);
    if (g.mode === "move") {
      const next = moveElements(g.base, page.id, g.ids, dx, dy);
      preview(() => next);
      const moved = next.pages
        .find((p) => p.id === page.id)
        ?.elements.filter((x) => g.ids.includes(x.id));
      if (moved && moved.length > 0) {
        const x = Math.min(...moved.map((m) => m.x));
        const y = Math.min(...moved.map((m) => m.y));
        const box = {
          x,
          y,
          w: Math.max(...moved.map((m) => m.x + m.w)) - x,
          h: Math.max(...moved.map((m) => m.y + m.h)) - y,
        };
        const found = alignGuides(page, box, new Set(g.ids));
        setGuides(found.x.length || found.y.length ? found : null);
      }
    } else {
      preview(() => updateElement(g.base, page.id, g.id, { w: g.w + dx, h: g.h + dy }));
    }
  };

  const stop = () => {
    const g = gesture.current;
    if (!g) return;
    gesture.current = null;
    setGuides(null);
    setFrame(null);
    if (g.mode !== "frame") end();
  };

  const onKey = (e: KeyboardEvent) => {
    const ids = useEditor.getState().selection;
    if (ids.length === 0 || (e.target as HTMLElement).closest("input, select, textarea")) return;
    const arrows: Record<string, [number, number]> = {
      ArrowLeft: [-1, 0],
      ArrowRight: [1, 0],
      ArrowUp: [0, -1],
      ArrowDown: [0, 1],
    };
    const a = arrows[e.key];
    if (a) {
      e.preventDefault();
      const one = ids.length === 1 ? (ids[0] as string) : null;
      apply((d) =>
        e.shiftKey && one
          ? resizeElement(d, page.id, one, a[0], a[1])
          : moveElements(d, page.id, ids, a[0], a[1]),
      );
    } else if (e.key === "Delete" || e.key === "Backspace") {
      e.preventDefault();
      apply((d) => removeElements(d, page.id, ids));
      select(null);
    } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "d" && ids.length === 1) {
      e.preventDefault();
      let nid: string | null = null;
      apply((d) => {
        const r = duplicateElement(d, page.id, ids[0] as string);
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
      const r = addElement(
        d,
        page.id,
        type,
        { x: Math.max(0, x), y: Math.max(0, y) },
        initialProps(type, lang),
      );
      nid = r.id;
      return r.def;
    });
    if (nid) select(nid);
  };

  const pct = (v: number, of: number) => `${(v / of) * 100}%`;

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
        editing
        overlay={(fit) => (
          <div
            ref={layer}
            className="sel-layer"
            onPointerDown={(e) => startFrame(e, fit)}
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
              const on = selection.includes(el.id);
              const single = on && selection.length === 1;
              return (
                <button
                  key={el.id}
                  type="button"
                  className={`sel-box${on ? " is-on" : ""}${el.show ? " has-condition" : ""}`}
                  style={boxStyle(el, page)}
                  aria-pressed={on}
                  aria-label={`${name} — ${el.x + 1}, ${el.y + 1} · ${el.w} × ${el.h}`}
                  title={name}
                  onFocus={() => {
                    if (!useEditor.getState().selection.includes(el.id)) select(el.id);
                  }}
                  onPointerDown={(e) => start(e, el.id, "move", fit)}
                >
                  {single && (
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
            {frame && (
              <div
                className="sel-frame"
                aria-hidden="true"
                style={{
                  left: pct(frame.x, page.cols),
                  top: pct(frame.y, page.rows),
                  width: pct(frame.w, page.cols),
                  height: pct(frame.h, page.rows),
                }}
              />
            )}
            {guides?.x.map((x) => (
              <i
                key={`gx${x}`}
                className="sel-guide is-v"
                aria-hidden="true"
                style={{ left: pct(x, page.cols) }}
              />
            ))}
            {guides?.y.map((y) => (
              <i
                key={`gy${y}`}
                className="sel-guide is-h"
                aria-hidden="true"
                style={{ top: pct(y, page.rows) }}
              />
            ))}
          </div>
        )}
      />
    </div>
  );
}
