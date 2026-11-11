import { beforeEach, describe, expect, it, vi } from "vitest";

describe("overlay layouts in the dashboard store", () => {
  beforeEach(() => {
    localStorage.clear();
    vi.resetModules();
  });

  async function store() {
    await import("../widgets");
    const mod = await import("./dashboard");
    mod.useDashboard.getState().setDevice("desktop");
    return mod;
  }

  it("starts with the default overlay layout, the screen layout shown", async () => {
    const { useDashboard, selectCurrentLayout } = await store();
    const s = useDashboard.getState();
    const overlay = s.layouts.find((l) => l.device === "overlay");
    expect(overlay?.id).toBe("default-overlay");
    expect(overlay?.columns).toBe(48);
    expect(s.current.overlay).toBe("default-overlay");
    expect(selectCurrentLayout(s)?.device).toBe("desktop");
  });

  it("choosing an overlay layout shows it, choosing a screen layout goes back", async () => {
    const { useDashboard, selectCurrentLayout } = await store();
    useDashboard.getState().selectLayout("default-overlay");
    expect(useDashboard.getState().view).toBe("overlay");
    expect(selectCurrentLayout(useDashboard.getState())?.id).toBe("default-overlay");
    useDashboard.getState().selectLayout("default-desktop");
    expect(useDashboard.getState().view).toBe("screen");
    expect(selectCurrentLayout(useDashboard.getState())?.id).toBe("default-desktop");
    // the desktop choice was kept while the overlay was shown
    expect(useDashboard.getState().current.desktop).toBe("default-desktop");
  });

  it("a new overlay layout is a 48-column grid, widgets placed on the canvas", async () => {
    const { useDashboard, selectCurrentLayout } = await store();
    const id = useDashboard.getState().createLayout("Stream", "overlay");
    expect(useDashboard.getState().view).toBe("overlay");
    for (let i = 0; i < 3; i += 1) useDashboard.getState().addWidget("inputs");
    const l = selectCurrentLayout(useDashboard.getState());
    expect(l?.id).toBe(id);
    expect(l?.columns).toBe(48);
    const ws = l?.widgets ?? [];
    expect(ws).toHaveLength(3);
    for (const w of ws) {
      expect(w.y + w.h).toBeLessThanOrEqual(27);
      expect(w.x + w.w).toBeLessThanOrEqual(48);
    }
    // no two widgets on top of each other
    const [a, b] = ws;
    expect(a && b && (a.x !== b.x || a.y !== b.y)).toBe(true);
  });

  it("the phone never shows an overlay layout", async () => {
    const { useDashboard, selectCurrentLayout } = await store();
    useDashboard.getState().selectLayout("default-overlay");
    useDashboard.getState().setDevice("mobile");
    expect(selectCurrentLayout(useDashboard.getState())?.device).toBe("mobile");
  });

  it("deleting the last overlay layout brings the default one back", async () => {
    const { useDashboard } = await store();
    useDashboard.getState().deleteLayout("default-overlay");
    const overlays = useDashboard.getState().layouts.filter((l) => l.device === "overlay");
    expect(overlays).toHaveLength(1);
    expect(useDashboard.getState().current.overlay).toBe(overlays[0]?.id);
  });
});
