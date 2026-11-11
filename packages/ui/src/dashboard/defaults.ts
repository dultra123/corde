import type { DashboardLayout, DeviceKind, WidgetPlacement } from "@corde/protocol";

export const DESKTOP_COLUMNS = 12;
export const MOBILE_COLUMNS = 4;
/** Below this viewport width the mobile layout (4 columns) is used. */
export const MOBILE_MAX_WIDTH = 700;

export const columnsFor = (device: DeviceKind): number =>
  device === "mobile" ? MOBILE_COLUMNS : DESKTOP_COLUMNS;

let counter = 0;
export function newId(prefix: string): string {
  counter += 1;
  const rnd = Math.random().toString(36).slice(2, 7);
  return `${prefix}-${Date.now().toString(36)}${counter.toString(36)}${rnd}`;
}

type Placement = Omit<WidgetPlacement, "id" | "config"> & { config?: Record<string, unknown> };

function place(items: Placement[]): WidgetPlacement[] {
  return items.map((p) => ({ ...p, id: newId(p.kind), config: p.config ?? {} }));
}

/** 12 columns × 16 rows: fills a 16:9 screen without scrolling. */
export function defaultDesktopLayout(): DashboardLayout {
  return {
    id: "default-desktop",
    name: "Course",
    device: "desktop",
    columns: DESKTOP_COLUMNS,
    updated_ms: 0,
    widgets: place([
      { kind: "laps", x: 0, y: 0, w: 3, h: 6 },
      { kind: "inputs", x: 3, y: 0, w: 4, h: 6 },
      { kind: "tyres", x: 7, y: 0, w: 3, h: 6 },
      { kind: "gaps", x: 10, y: 0, w: 2, h: 6 },
      { kind: "map", x: 0, y: 6, w: 3, h: 6 },
      { kind: "standings", x: 3, y: 6, w: 4, h: 6, config: { mode: "relative" } },
      { kind: "brakes", x: 7, y: 6, w: 2, h: 6 },
      { kind: "fuel", x: 9, y: 6, w: 3, h: 6 },
      { kind: "session", x: 0, y: 12, w: 4, h: 4 },
      { kind: "electronics", x: 4, y: 12, w: 8, h: 4 },
    ]),
  };
}

/** 4 columns, vertical scroll: the essentials first. */
export function defaultMobileLayout(): DashboardLayout {
  return {
    id: "default-mobile",
    name: "Téléphone",
    device: "mobile",
    columns: MOBILE_COLUMNS,
    updated_ms: 0,
    widgets: place([
      { kind: "laps", x: 0, y: 0, w: 4, h: 5 },
      { kind: "gaps", x: 0, y: 5, w: 4, h: 3 },
      { kind: "tyres", x: 0, y: 8, w: 2, h: 5 },
      { kind: "brakes", x: 2, y: 8, w: 2, h: 5 },
      { kind: "inputs", x: 0, y: 13, w: 4, h: 5 },
      { kind: "fuel", x: 0, y: 18, w: 4, h: 4 },
      { kind: "standings", x: 0, y: 22, w: 4, h: 5, config: { mode: "relative" } },
      { kind: "map", x: 0, y: 27, w: 4, h: 5 },
      { kind: "session", x: 0, y: 32, w: 4, h: 3 },
      { kind: "electronics", x: 0, y: 35, w: 4, h: 4 },
    ]),
  };
}

export function defaultLayouts(): DashboardLayout[] {
  return [defaultDesktopLayout(), defaultMobileLayout()];
}
