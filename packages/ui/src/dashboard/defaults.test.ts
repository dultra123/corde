import { describe, expect, it } from "vitest";
import "../widgets";
import { PROFILE_GAMES } from "../lib/games";
import { getWidget } from "../widgets/registry";
import {
  DESKTOP_COLUMNS,
  defaultDesktopLayout,
  defaultMobileLayout,
  gameLayoutId,
  gameLayouts,
  MOBILE_COLUMNS,
  shareColumns,
} from "./defaults";

describe("shares of a row turned into columns", () => {
  it("always fill the row, in whole columns", () => {
    expect(shareColumns([3, 4, 3, 2], 12)).toEqual([3, 4, 3, 2]);
    expect(shareColumns([3, 4, 2], 12)).toEqual([4, 5, 3]);
    expect(shareColumns([4], 12)).toEqual([12]);
    expect(shareColumns([], 12)).toEqual([]);
    for (const shares of [[3, 4], [1, 1, 1], [3, 4, 3], [2, 3], [1, 5, 1, 1, 1], [8]]) {
      const out = shareColumns(shares, 12);
      expect(out.reduce((a, b) => a + b, 0)).toBe(12);
      for (const w of out) expect(w).toBeGreaterThanOrEqual(1);
    }
  });
});

describe("the layouts proposed for a game", () => {
  const kinds = (game: (typeof PROFILE_GAMES)[number], device: "desktop" | "mobile") =>
    gameLayouts(game, "x")
      .find((l) => l.device === device)
      ?.widgets.map((w) => w.kind) ?? [];

  it("Le Mans Ultimate's are « Course » as it is", () => {
    expect(kinds("lmu", "desktop").sort()).toEqual(
      defaultDesktopLayout()
        .widgets.map((w) => w.kind)
        .sort(),
    );
    expect(kinds("lmu", "mobile").sort()).toEqual(
      defaultMobileLayout()
        .widgets.map((w) => w.kind)
        .sort(),
    );
    const [desktop] = gameLayouts("lmu", "x");
    const course = defaultDesktopLayout();
    for (const w of desktop?.widgets ?? []) {
      const ref = course.widgets.find((c) => c.kind === w.kind);
      expect({ x: w.x, y: w.y, w: w.w, h: w.h }).toEqual({
        x: ref?.x,
        y: ref?.y,
        w: ref?.w,
        h: ref?.h,
      });
    }
  });

  it("leave out what the game does not publish", () => {
    expect(kinds("iracing", "desktop")).not.toContain("tyres");
    expect(kinds("iracing", "desktop")).not.toContain("brakes");
    expect(kinds("iracing", "mobile")).not.toContain("tyres");
    expect(kinds("iracing", "desktop")).toContain("standings");
    expect(kinds("acc", "desktop")).not.toContain("standings");
    expect(kinds("acc", "desktop")).toContain("tyres");
    // rFactor 2: everything but the levels of the driver aids.
    expect(kinds("rfactor2", "desktop")).not.toContain("electronics");
    expect(kinds("rfactor2", "desktop")).toEqual(
      expect.arrayContaining(["tyres", "brakes", "standings", "gaps", "map", "fuel"]),
    );
    expect(kinds("ac", "desktop")).toEqual(
      expect.not.arrayContaining(["standings", "gaps", "electronics"]),
    );
    expect(kinds("ac", "desktop")).toEqual(expect.arrayContaining(["tyres", "brakes", "map"]));
    expect(kinds("ac_rally", "desktop")).toEqual(
      expect.not.arrayContaining(["standings", "gaps", "electronics"]),
    );
    expect(kinds("ac_rally", "desktop")).toEqual(
      expect.arrayContaining(["laps", "inputs", "tyres", "brakes", "map", "fuel"]),
    );
    expect(kinds("ac_evo", "desktop")).not.toContain("standings");
    expect(kinds("ac_evo", "desktop")).toEqual(
      expect.arrayContaining(["tyres", "brakes", "gaps", "electronics"]),
    );
  });

  it("are full rows of known widgets, none on another, on every game", () => {
    for (const game of PROFILE_GAMES) {
      const layouts = gameLayouts(game, "Nom");
      expect(layouts.map((l) => l.id)).toEqual([
        gameLayoutId(game, "desktop"),
        gameLayoutId(game, "mobile"),
      ]);
      for (const layout of layouts) {
        const columns = layout.device === "mobile" ? MOBILE_COLUMNS : DESKTOP_COLUMNS;
        expect(layout.columns).toBe(columns);
        expect(layout.name).toBe("Nom");
        expect(layout.widgets.length).toBeGreaterThan(3);
        expect(new Set(layout.widgets.map((w) => w.id)).size).toBe(layout.widgets.length);
        const rows = new Map<number, number>();
        for (const w of layout.widgets) {
          const def = getWidget(w.kind);
          expect(def, w.kind).toBeDefined();
          expect(w.w).toBeGreaterThanOrEqual(Math.min(def?.minW ?? 1, columns));
          expect(w.x + w.w).toBeLessThanOrEqual(columns);
          rows.set(w.y, (rows.get(w.y) ?? 0) + w.w);
        }
        // every row is filled from edge to edge: no hole where a widget was taken out
        for (const [, width] of rows) expect(width).toBe(columns);
      }
    }
  });
});
