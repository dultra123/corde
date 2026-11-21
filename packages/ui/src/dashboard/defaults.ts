import type { DashboardLayout, DeviceKind, GameId, WidgetPlacement } from "@corde/protocol";
import { type GameTraits, gameTraits } from "../lib/games";
import { OVERLAY_COLUMNS } from "../overlay/model";

export const DESKTOP_COLUMNS = 12;
export const MOBILE_COLUMNS = 4;
/** Below this viewport width the mobile layout (4 columns) is used. */
export const MOBILE_MAX_WIDTH = 700;

export const columnsFor = (device: DeviceKind): number =>
  device === "mobile" ? MOBILE_COLUMNS : device === "overlay" ? OVERLAY_COLUMNS : DESKTOP_COLUMNS;

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

/** Id of the overlay layout every installation starts with (the OBS page offers it first). */
export const DEFAULT_OVERLAY_ID = "default-overlay";

/**
 * 48 × 27 cells of 40 px on a 1920 × 1080 canvas, transparent around the widgets: the map in a
 * corner, the gaps across, the relative standings on the left, the pedals at the bottom.
 */
export function defaultOverlayLayout(): DashboardLayout {
  return {
    id: DEFAULT_OVERLAY_ID,
    name: "OBS",
    device: "overlay",
    columns: OVERLAY_COLUMNS,
    updated_ms: 0,
    widgets: place([
      { kind: "map", x: 0, y: 0, w: 9, h: 9 },
      { kind: "gaps", x: 40, y: 0, w: 8, h: 9 },
      { kind: "standings", x: 0, y: 17, w: 16, h: 10, config: { mode: "relative" } },
      { kind: "inputs", x: 17, y: 19, w: 15, h: 8, config: { window: 10 } },
      { kind: "tyres", x: 36, y: 17, w: 12, h: 10 },
    ]),
  };
}

export function defaultLayouts(): DashboardLayout[] {
  return [defaultDesktopLayout(), defaultMobileLayout()];
}

// ---------------------------------------------------------------- per game

/** What a widget needs from the game to be worth showing (`null`: always shown). */
const NEEDS: Record<string, keyof GameTraits | null> = {
  laps: null,
  inputs: null,
  tyres: "tyres",
  gaps: "gaps",
  map: null,
  standings: "standings",
  brakes: "brakes",
  fuel: null,
  session: null,
  electronics: "electronics",
};

function shownFor(traits: GameTraits, kind: string): boolean {
  const need = NEEDS[kind];
  return need === null || need === undefined || traits[need];
}

/**
 * The rows of « Course », each widget with the share of the width it takes. A widget the game
 * has nothing for is taken out and the others share its place.
 */
const DESKTOP_ROWS: { h: number; items: [kind: string, share: number][] }[] = [
  {
    h: 6,
    items: [
      ["laps", 3],
      ["inputs", 4],
      ["tyres", 3],
      ["gaps", 2],
    ],
  },
  {
    h: 6,
    items: [
      ["map", 3],
      ["standings", 4],
      ["brakes", 2],
      ["fuel", 3],
    ],
  },
  {
    h: 4,
    items: [
      ["session", 4],
      ["electronics", 8],
    ],
  },
];

/** Shares of the width turned into whole columns that add up to `columns`. */
export function shareColumns(shares: number[], columns: number): number[] {
  const total = shares.reduce((a, b) => a + b, 0);
  if (shares.length === 0 || total <= 0) return [];
  const exact = shares.map((s) => (s / total) * columns);
  const out = exact.map((v) => Math.max(1, Math.floor(v)));
  // what is left goes to the largest remainders, first come first served
  const order = exact.map((v, i) => [v - Math.floor(v), i] as const).sort((a, b) => b[0] - a[0]);
  let left = columns - out.reduce((a, b) => a + b, 0);
  for (let k = 0; left > 0 && order.length > 0; k = (k + 1) % order.length, left -= 1) {
    const at = order[k]?.[1] ?? 0;
    out[at] = (out[at] ?? 0) + 1;
  }
  return out;
}

const MOBILE_ORDER: [kind: string, h: number][] = [
  ["laps", 5],
  ["gaps", 3],
  ["tyres", 5],
  ["brakes", 5],
  ["inputs", 5],
  ["fuel", 4],
  ["standings", 5],
  ["map", 5],
  ["session", 3],
  ["electronics", 4],
];

export const gameLayoutId = (game: GameId, device: "desktop" | "mobile"): string =>
  `game-${game}-${device}`;

/**
 * The layouts Corde proposes for a game, PC and phone: « Course » without what the game does
 * not publish. Their ids are the same on every device, so that they are created once.
 */
export function gameLayouts(game: GameId, name: string): DashboardLayout[] {
  const traits = gameTraits(game);
  const desktop: Placement[] = [];
  let y = 0;
  for (const row of DESKTOP_ROWS) {
    const items = row.items.filter(([kind]) => shownFor(traits, kind));
    const widths = shareColumns(
      items.map(([, share]) => share),
      DESKTOP_COLUMNS,
    );
    let x = 0;
    items.forEach(([kind], i) => {
      const w = widths[i] ?? 1;
      desktop.push({
        kind,
        x,
        y,
        w,
        h: row.h,
        ...(kind === "standings" ? { config: { mode: "relative" } } : {}),
      });
      x += w;
    });
    if (items.length > 0) y += row.h;
  }
  const mobile: Placement[] = [];
  let my = 0;
  for (const [kind, h] of MOBILE_ORDER) {
    if (!shownFor(traits, kind)) continue;
    mobile.push({
      kind,
      x: 0,
      y: my,
      w: MOBILE_COLUMNS,
      h,
      ...(kind === "standings" ? { config: { mode: "relative" } } : {}),
    });
    my += h;
  }
  return [
    {
      id: gameLayoutId(game, "desktop"),
      name,
      device: "desktop",
      columns: DESKTOP_COLUMNS,
      updated_ms: 0,
      widgets: place(desktop),
    },
    {
      id: gameLayoutId(game, "mobile"),
      name,
      device: "mobile",
      columns: MOBILE_COLUMNS,
      updated_ms: 0,
      widgets: place(mobile),
    },
  ];
}
