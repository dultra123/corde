import { describe, expect, it } from "vitest";
import { DESKTOP_COLUMNS, defaultLayouts, MOBILE_COLUMNS } from "../dashboard/defaults";
import { en } from "../i18n/en";
import { fr } from "../i18n/fr";
import { getWidget, listWidgets } from "./index";

const EXPECTED = [
  "tyres",
  "brakes",
  "inputs",
  "laps",
  "fuel",
  "map",
  "session",
  "standings",
  "gaps",
  "electronics",
];

describe("widget registry", () => {
  it("registers the ten v0 widgets with consistent sizes", () => {
    const ids = listWidgets().map((w) => w.id);
    for (const id of EXPECTED) expect(ids).toContain(id);
    for (const w of listWidgets()) {
      expect(w.minW).toBeGreaterThanOrEqual(1);
      expect(w.minH).toBeGreaterThanOrEqual(1);
      expect(w.defaultSize.w).toBeGreaterThanOrEqual(w.minW);
      expect(w.defaultSize.h).toBeGreaterThanOrEqual(w.minH);
      expect(w.defaultSize.w).toBeLessThanOrEqual(DESKTOP_COLUMNS);
      if (w.mobileSize) {
        expect(w.mobileSize.w).toBeLessThanOrEqual(MOBILE_COLUMNS);
        expect(w.mobileSize.w).toBeGreaterThanOrEqual(Math.min(w.minW, MOBILE_COLUMNS));
      }
      expect(typeof w.component).toBe("function");
      expect(fr[w.titleKey]).toBeTruthy();
      expect(en[w.titleKey]).toBeTruthy();
    }
    expect(getWidget("nope")).toBeUndefined();
  });

  it("default layouts only use registered kinds and fit their grid", () => {
    for (const layout of defaultLayouts()) {
      const ids = new Set<string>();
      for (const w of layout.widgets) {
        const def = getWidget(w.kind);
        expect(def, `unknown widget kind ${w.kind}`).toBeDefined();
        expect(ids.has(w.id)).toBe(false);
        ids.add(w.id);
        expect(w.x + w.w).toBeLessThanOrEqual(layout.columns);
        expect(w.w).toBeGreaterThanOrEqual(Math.min(def?.minW ?? 1, layout.columns));
        expect(w.h).toBeGreaterThanOrEqual(def?.minH ?? 1);
      }
    }
  });

  it("config fields have translated labels", () => {
    for (const w of listWidgets()) {
      for (const f of w.configFields ?? []) {
        expect(fr[f.labelKey]).toBeTruthy();
        for (const o of f.options) expect(en[o.labelKey]).toBeTruthy();
      }
    }
  });
});
