import { describe, expect, it } from "vitest";
import { buildCsv, csvFileName } from "./csv";

describe("buildCsv", () => {
  it("writes a header with units and one row per sample", () => {
    const text = buildCsv([
      { name: "lap_dist_m", unit: "m", values: [0, 12.5, 25] },
      { name: "speed_kmh", unit: "km/h", values: [100.123456789, null, 120] },
      { name: "gear", unit: "", values: [3, 4] },
    ]);
    expect(text).toBe("lap_dist_m [m],speed_kmh [km/h],gear\n0,100.1235,3\n12.5,,4\n25,120,\n");
  });
  it("handles an empty export", () => {
    expect(buildCsv([])).toBe("");
  });
  it("builds a safe file name", () => {
    expect(csvFileName("20261104-063959-paul-ricard-1a-race", 4, "fr")).toBe(
      "corde-20261104-063959-paul-ricard-1a-race-tour-4.csv",
    );
    expect(csvFileName("a b/c", 1, "en")).toBe("corde-a-b-c-lap-1.csv");
  });
});
