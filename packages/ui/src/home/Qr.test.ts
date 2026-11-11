import { describe, expect, it } from "vitest";
import { qrMatrix, qrPath } from "./Qr";

const URL = "http://192.168.1.20:8471";

/** The 7×7 finder pattern every QR code has in three corners. */
function isFinder(m: boolean[][], top: number, left: number): boolean {
  for (let r = 0; r < 7; r += 1) {
    for (let c = 0; c < 7; c += 1) {
      const ring = r === 0 || r === 6 || c === 0 || c === 6;
      const core = r >= 2 && r <= 4 && c >= 2 && c <= 4;
      if (m[top + r]?.[left + c] !== (ring || core)) return false;
    }
  }
  return true;
}

describe("qrMatrix", () => {
  it("builds a square code with its three finder patterns", () => {
    const m = qrMatrix(URL);
    const n = m.length;
    // a LAN address fits in a version 2 code (25 modules): large modules, easy to scan
    expect(n).toBe(25);
    expect(m.every((row) => row.length === n)).toBe(true);
    expect(isFinder(m, 0, 0)).toBe(true);
    expect(isFinder(m, 0, n - 7)).toBe(true);
    expect(isFinder(m, n - 7, 0)).toBe(true);
    expect(isFinder(m, n - 7, n - 7)).toBe(false);
  });

  it("encodes the text (another address, another code)", () => {
    expect(qrMatrix(URL)).toEqual(qrMatrix(URL));
    expect(qrMatrix(URL)).not.toEqual(qrMatrix("http://192.168.1.21:8471"));
  });
});

describe("qrPath", () => {
  it("draws every dark module once, inside the quiet zone", () => {
    const m = qrMatrix(URL);
    const { size, d } = qrPath(URL);
    expect(size).toBe(m.length + 8);
    const runs = [...d.matchAll(/M(\d+) (\d+)h(\d+)v1h-(\d+)z/g)];
    expect(runs.length).toBeGreaterThan(50);
    expect(runs.map((r) => r[0]).join("")).toBe(d);
    let dark = 0;
    for (const [, x, y, w] of runs) {
      const col = Number(x) - 4;
      const row = Number(y) - 4;
      expect(col).toBeGreaterThanOrEqual(0);
      expect(col + Number(w)).toBeLessThanOrEqual(m.length);
      for (let c = col; c < col + Number(w); c += 1) expect(m[row]?.[c]).toBe(true);
      dark += Number(w);
    }
    expect(dark).toBe(m.flat().filter(Boolean).length);
  });
});
