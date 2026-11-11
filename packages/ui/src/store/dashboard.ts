import type { DashboardLayout, DeviceKind, WidgetPlacement } from "@corde/protocol";
import { create } from "zustand";
import { columnsFor, defaultLayouts, defaultOverlayLayout, newId } from "../dashboard/defaults";
import { isLayout } from "../net/layouts-api";
import { freeSpot, OVERLAY_ROWS, overlayCells, recommendedWidgetSize } from "../overlay/model";
import { getWidget } from "../widgets/registry";

export type Device = Extract<DeviceKind, "desktop" | "mobile">;
/** Which layout list a layout belongs to: the two screens, and the overlays (OBS). */
export type Slot = DeviceKind;
/**
 * What the dashboard shows on a desktop: the screen layout, or an overlay layout being edited
 * (the phone always shows its own layout).
 */
export type View = "screen" | "overlay";
export type SyncState = "local" | "syncing" | "synced" | "error";

export interface GridPosition {
  id: string;
  x: number;
  y: number;
  w: number;
  h: number;
}

interface Persisted {
  layouts: DashboardLayout[];
  current: Record<Slot, string>;
}

const KEY = "corde.layouts.v1";
/** legacy name (Muret, before 0.3.0): read when the new key is absent. */
const LEGACY_KEY = "muret.layouts.v1";

export interface DashboardState {
  layouts: DashboardLayout[];
  current: Record<Slot, string>;
  device: Device;
  view: View;
  editing: boolean;
  paletteOpen: boolean;
  sync: SyncState;
  /** Bumped on every local layout mutation (what the server sync listens to). */
  revision: number;

  setDevice(device: Device): void;
  setView(view: View): void;
  setEditing(on: boolean): void;
  setPaletteOpen(on: boolean): void;
  setSync(state: SyncState): void;

  selectLayout(id: string): void;
  createLayout(name: string, device?: Slot): string;
  renameLayout(id: string, name: string): void;
  duplicateLayout(id: string): string;
  deleteLayout(id: string): void;

  /** Positions coming back from the grid after a drag / resize. */
  applyPositions(layoutId: string, positions: GridPosition[]): void;
  addWidget(kind: string): void;
  removeWidget(widgetId: string): void;
  setWidgetConfig(widgetId: string, patch: Record<string, unknown>): void;

  /** Replaces the whole list (after a merge with the server). Keeps the current selection valid. */
  replaceLayouts(layouts: DashboardLayout[], fromServer: boolean): void;
}

function slotOf(layout: DashboardLayout): Slot {
  return layout.device === "mobile" || layout.device === "overlay" ? layout.device : "desktop";
}

/**
 * Overlay layouts came with Corde 0.5: a list saved by an older version has none, so the default
 * one is added (it reaches the server with the next sync, like any local layout).
 */
export function withOverlay(layouts: DashboardLayout[]): DashboardLayout[] {
  return layouts.some((l) => l.device === "overlay")
    ? layouts
    : [...layouts, defaultOverlayLayout()];
}

function load(): Persisted {
  const fallback = (): Persisted => {
    const layouts = withOverlay(defaultLayouts());
    return {
      layouts,
      current: {
        desktop: pickCurrent(layouts, undefined, "desktop"),
        mobile: pickCurrent(layouts, undefined, "mobile"),
        overlay: pickCurrent(layouts, undefined, "overlay"),
      },
    };
  };
  try {
    const raw = localStorage.getItem(KEY) ?? localStorage.getItem(LEGACY_KEY);
    if (!raw) return fallback();
    const parsed = JSON.parse(raw) as Partial<Persisted>;
    const stored = Array.isArray(parsed.layouts) ? parsed.layouts.filter(isLayout) : [];
    if (stored.length === 0) return fallback();
    const layouts = withOverlay(stored);
    const cur = parsed.current ?? ({} as Partial<Record<Slot, string>>);
    return {
      layouts,
      current: {
        desktop: pickCurrent(layouts, cur.desktop, "desktop"),
        mobile: pickCurrent(layouts, cur.mobile, "mobile"),
        overlay: pickCurrent(layouts, cur.overlay, "overlay"),
      },
    };
  } catch {
    return fallback();
  }
}

function pickCurrent(layouts: DashboardLayout[], wanted: string | undefined, slot: Slot): string {
  if (wanted && layouts.some((l) => l.id === wanted && slotOf(l) === slot)) return wanted;
  const sameSlot = layouts.find((l) => slotOf(l) === slot);
  if (sameSlot) return sameSlot.id;
  // a screen without its own layout shows any other screen layout, never an overlay
  return slot === "overlay" ? "" : (layouts.find((l) => slotOf(l) !== "overlay")?.id ?? "");
}

/** The list the dashboard edits right now. */
export function activeSlot(s: Pick<DashboardState, "device" | "view">): Slot {
  return s.device === "desktop" && s.view === "overlay" ? "overlay" : s.device;
}

function save(state: Pick<DashboardState, "layouts" | "current">): void {
  try {
    const data: Persisted = { layouts: state.layouts, current: state.current };
    localStorage.setItem(KEY, JSON.stringify(data));
  } catch {
    // private mode / quota: layouts live for the session only
  }
}

function detectDevice(): Device {
  if (typeof window === "undefined") return "desktop";
  return window.innerWidth < 700 ? "mobile" : "desktop";
}

/** First free row below everything, for appended widgets. */
function bottomOf(widgets: WidgetPlacement[]): number {
  return widgets.reduce((m, w) => Math.max(m, w.y + w.h), 0);
}

export const useDashboard = create<DashboardState>((set, get) => {
  const initial = load();

  const touch = (id: string, fn: (l: DashboardLayout) => DashboardLayout) => {
    set((s) => ({
      layouts: s.layouts.map((l) => (l.id === id ? { ...fn(l), updated_ms: Date.now() } : l)),
      revision: s.revision + 1,
      sync: s.sync === "synced" ? "local" : s.sync,
    }));
    save(get());
  };

  return {
    layouts: initial.layouts,
    current: initial.current,
    device: detectDevice(),
    view: "screen",
    editing: false,
    paletteOpen: false,
    sync: "local",
    revision: 0,

    setDevice(device) {
      if (get().device !== device) set({ device, editing: false, paletteOpen: false });
    },
    setView(view) {
      if (get().view !== view) set({ view, paletteOpen: false });
    },
    setEditing(on) {
      set({ editing: on, paletteOpen: on ? get().paletteOpen : false });
    },
    setPaletteOpen(on) {
      set({ paletteOpen: on });
    },
    setSync(sync) {
      set({ sync });
    },

    selectLayout(id) {
      const l = get().layouts.find((x) => x.id === id);
      if (!l) return;
      const slot = slotOf(l);
      set((s) => ({
        current: { ...s.current, [slot]: id },
        view: slot === "overlay" ? "overlay" : slot === "desktop" ? "screen" : s.view,
      }));
      save(get());
    },

    createLayout(name, device = activeSlot(get())) {
      const id = newId("layout");
      const layout: DashboardLayout = {
        id,
        name: name.trim() || "Sans nom",
        device,
        columns: columnsFor(device),
        widgets: [],
        updated_ms: Date.now(),
      };
      set((s) => ({
        layouts: [...s.layouts, layout],
        current: { ...s.current, [device]: id },
        view: device === "overlay" ? "overlay" : device === "desktop" ? "screen" : s.view,
        revision: s.revision + 1,
      }));
      save(get());
      return id;
    },

    renameLayout(id, name) {
      const clean = name.trim();
      if (!clean) return;
      touch(id, (l) => ({ ...l, name: clean }));
    },

    duplicateLayout(id) {
      const src = get().layouts.find((l) => l.id === id);
      if (!src) return id;
      const copy: DashboardLayout = {
        ...src,
        id: newId("layout"),
        name: `${src.name} (copie)`,
        widgets: src.widgets.map((w) => ({ ...w, id: newId(w.kind), config: { ...w.config } })),
        updated_ms: Date.now(),
      };
      const slot = slotOf(copy);
      set((s) => ({
        layouts: [...s.layouts, copy],
        current: { ...s.current, [slot]: copy.id },
        revision: s.revision + 1,
      }));
      save(get());
      return copy.id;
    },

    deleteLayout(id) {
      const s = get();
      const victim = s.layouts.find((l) => l.id === id);
      if (!victim) return;
      let layouts = s.layouts.filter((l) => l.id !== id);
      const slot = slotOf(victim);
      if (!layouts.some((l) => slotOf(l) === slot)) {
        // never leave a device (or the overlays) without a layout: recreate the default one
        const fresh = [...defaultLayouts(), defaultOverlayLayout()].find((l) => slotOf(l) === slot);
        if (fresh)
          layouts = [...layouts, { ...fresh, id: newId("layout"), updated_ms: Date.now() }];
      }
      set({
        layouts,
        current: { ...s.current, [slot]: pickCurrent(layouts, undefined, slot) },
        revision: s.revision + 1,
      });
      save(get());
    },

    applyPositions(layoutId, positions) {
      const byId = new Map(positions.map((p) => [p.id, p]));
      const cur = get().layouts.find((l) => l.id === layoutId);
      if (!cur) return;
      const changed = cur.widgets.some((w) => {
        const p = byId.get(w.id);
        return p && (p.x !== w.x || p.y !== w.y || p.w !== w.w || p.h !== w.h);
      });
      if (!changed) return;
      touch(layoutId, (l) => ({
        ...l,
        widgets: l.widgets.map((w) => {
          const p = byId.get(w.id);
          return p ? { ...w, x: p.x, y: p.y, w: p.w, h: p.h } : w;
        }),
      }));
    },

    addWidget(kind) {
      const def = getWidget(kind);
      const s = get();
      const layoutId = s.current[activeSlot(s)];
      const layout = s.layouts.find((l) => l.id === layoutId);
      if (!def || !layout) return;
      const size =
        layout.device === "overlay"
          ? overlayCells(recommendedWidgetSize(def), { w: def.minW, h: def.minH })
          : s.device === "mobile"
            ? (def.mobileSize ?? def.defaultSize)
            : def.defaultSize;
      const w = Math.min(size.w, layout.columns);
      // an overlay canvas has a bottom: find a free place on it rather than below everything
      const spot =
        layout.device === "overlay"
          ? (freeSpot(layout.widgets, w, size.h, layout.columns, OVERLAY_ROWS) ?? { x: 0, y: 0 })
          : { x: 0, y: bottomOf(layout.widgets) };
      const placement: WidgetPlacement = {
        id: newId(kind),
        kind,
        x: spot.x,
        y: spot.y,
        w,
        h: size.h,
        config: { ...(def.defaultConfig ?? {}) },
      };
      touch(layoutId, (l) => ({ ...l, widgets: [...l.widgets, placement] }));
    },

    removeWidget(widgetId) {
      const s = get();
      const layoutId = s.current[activeSlot(s)];
      touch(layoutId, (l) => ({ ...l, widgets: l.widgets.filter((w) => w.id !== widgetId) }));
    },

    setWidgetConfig(widgetId, patch) {
      const s = get();
      const layoutId = s.current[activeSlot(s)];
      touch(layoutId, (l) => ({
        ...l,
        widgets: l.widgets.map((w) =>
          w.id === widgetId ? { ...w, config: { ...w.config, ...patch } } : w,
        ),
      }));
    },

    replaceLayouts(layouts, fromServer) {
      if (layouts.length === 0) return;
      const s = get();
      set({
        layouts,
        current: {
          desktop: pickCurrent(layouts, s.current.desktop, "desktop"),
          mobile: pickCurrent(layouts, s.current.mobile, "mobile"),
          overlay: pickCurrent(layouts, s.current.overlay, "overlay"),
        },
        sync: fromServer ? "synced" : s.sync,
      });
      save(get());
    },
  };
});

/** The layout shown on the current device. */
export function selectCurrentLayout(s: DashboardState): DashboardLayout | null {
  return s.layouts.find((l) => l.id === s.current[activeSlot(s)]) ?? null;
}

export const useCurrentLayout = () => useDashboard(selectCurrentLayout);
