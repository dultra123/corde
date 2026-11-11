import type { DashboardLayout } from "@corde/protocol";
import { GridStack, type GridStackNode } from "gridstack";
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { useDashboard } from "../store/dashboard";
import { GridItem } from "./GridItem";

interface Props {
  layout: DashboardLayout;
  editing: boolean;
  cellHeight: number;
}

/**
 * gridstack owns positions and drag/resize; React owns the DOM of each item. Items register
 * themselves with `makeWidget` on mount; the grid writes positions back to the store on change.
 * The component is keyed by layout id from the parent, so a layout switch is a clean remount.
 */
export function GridCanvas({ layout, editing, cellHeight }: Props) {
  const ref = useRef<HTMLDivElement>(null);
  const [grid, setGrid] = useState<GridStack | null>(null);
  const applyPositions = useDashboard((s) => s.applyPositions);
  const layoutId = layout.id;
  const columns = layout.columns;
  // initial cell height only; later changes go through `grid.cellHeight()` below
  const cellRef = useRef(cellHeight);
  cellRef.current = cellHeight;

  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const g = GridStack.init(
      {
        column: columns,
        cellHeight: cellRef.current,
        margin: 3,
        mode: "top",
        animate: true,
        staticGrid: true,
        minRow: 1,
        alwaysShowResizeHandle: true,
        draggable: { handle: ".widget-head", appendTo: "body" },
        resizable: { handles: "se,sw" },
      },
      el,
    );
    if (!g) return;
    g.on("change", (_ev, nodes: GridStackNode[]) => {
      applyPositions(
        layoutId,
        nodes
          .filter((n) => typeof n.id === "string")
          .map((n) => ({ id: n.id as string, x: n.x ?? 0, y: n.y ?? 0, w: n.w ?? 1, h: n.h ?? 1 })),
      );
    });
    setGrid(g);
    return () => {
      g.offAll();
      g.destroy(false);
      setGrid(null);
    };
  }, [layoutId, columns, applyPositions]);

  useEffect(() => {
    grid?.setStatic(!editing);
  }, [grid, editing]);

  useEffect(() => {
    grid?.cellHeight(cellHeight);
  }, [grid, cellHeight]);

  return (
    <div className={`grid-stack${editing ? " is-editing" : ""}`} ref={ref}>
      {grid &&
        layout.widgets.map((w) => (
          <GridItem key={w.id} grid={grid} placement={w} editing={editing} />
        ))}
    </div>
  );
}
