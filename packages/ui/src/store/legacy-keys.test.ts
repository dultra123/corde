import { beforeEach, describe, expect, it, vi } from "vitest";

// legacy name (Muret, before 0.3.0): keys written by the application's former name.
const LEGACY_SETTINGS = "muret.settings.v1";
const LEGACY_LAYOUTS = "muret.layouts.v1";

const layout = {
  id: "l1",
  name: "Ancienne",
  device: "desktop",
  columns: 12,
  widgets: [],
  updated_ms: 1,
};

describe("storage keys of the former name", () => {
  beforeEach(() => {
    localStorage.clear();
    vi.resetModules();
  });

  it("settings fall back to the former key, then save under the new one", async () => {
    localStorage.setItem(LEGACY_SETTINGS, JSON.stringify({ lang: "en", rateHz: 30 }));
    const { useSettings } = await import("./settings");
    expect(useSettings.getState().lang).toBe("en");
    expect(useSettings.getState().rateHz).toBe(30);
    useSettings.getState().update({ rateHz: 10 });
    expect(JSON.parse(localStorage.getItem("corde.settings.v1") ?? "{}").rateHz).toBe(10);
    expect(localStorage.getItem(LEGACY_SETTINGS)).not.toBeNull();
  });

  it("the new settings key wins over the former one", async () => {
    localStorage.setItem(LEGACY_SETTINGS, JSON.stringify({ lang: "en" }));
    localStorage.setItem("corde.settings.v1", JSON.stringify({ lang: "fr" }));
    const { useSettings } = await import("./settings");
    expect(useSettings.getState().lang).toBe("fr");
  });

  it("layouts fall back to the former key", async () => {
    localStorage.setItem(
      LEGACY_LAYOUTS,
      JSON.stringify({ layouts: [layout], current: { desktop: "l1", mobile: "l1" } }),
    );
    const { useDashboard } = await import("./dashboard");
    expect(useDashboard.getState().layouts.map((l) => l.name)).toEqual(["Ancienne"]);
  });
});
