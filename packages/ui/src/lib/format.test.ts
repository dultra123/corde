import { describe, expect, it } from "vitest";
import { fmtRelativeDay } from "./format";

// Local-time constructors: the expectations hold in any time zone.
const at = (y: number, m: number, d: number, h = 0, min = 0) =>
  new Date(y, m - 1, d, h, min).getTime();
const NOW = at(2026, 11, 10, 9, 30); // Tuesday 10 Nov 2026, 09:30

describe("fmtRelativeDay", () => {
  it("says today and yesterday by calendar day, not by elapsed hours", () => {
    expect(fmtRelativeDay(at(2026, 11, 10, 0, 5), "fr", NOW)).toBe("aujourd'hui 00:05");
    expect(fmtRelativeDay(at(2026, 11, 9, 21, 4), "fr", NOW)).toBe("hier 21:04");
    // 23:59 the day before is "yesterday" even though it is less than 24 h ago
    expect(fmtRelativeDay(at(2026, 11, 9, 23, 59), "fr", NOW)).toBe("hier 23:59");
    expect(fmtRelativeDay(at(2026, 11, 9, 21, 4), "en", NOW)).toBe("yesterday 21:04");
    expect(fmtRelativeDay(at(2026, 11, 10, 8, 0), "en", NOW)).toBe("today 08:00");
  });

  it("uses the weekday for the last six days", () => {
    expect(fmtRelativeDay(at(2026, 11, 8, 14, 0), "fr", NOW)).toBe("dimanche 14:00");
    expect(fmtRelativeDay(at(2026, 11, 4, 7, 39), "fr", NOW)).toBe("mercredi 07:39");
    expect(fmtRelativeDay(at(2026, 11, 4, 7, 39), "en", NOW)).toBe("Wednesday 07:39");
  });

  it("falls back to the date beyond a week, with the year only when it differs", () => {
    expect(fmtRelativeDay(at(2026, 11, 3, 21, 5), "fr", NOW)).toBe("3 nov. 21:05");
    expect(fmtRelativeDay(at(2026, 11, 1, 21, 5), "fr", NOW)).toBe("1 nov. 21:05");
    expect(fmtRelativeDay(at(2025, 12, 24, 18, 0), "fr", NOW)).toBe("24 déc. 2025");
    expect(fmtRelativeDay(at(2026, 11, 1, 21, 5), "en", NOW)).toBe("1 Nov 21:05");
  });

  it("can leave the clock out", () => {
    expect(fmtRelativeDay(at(2026, 11, 9, 21, 4), "fr", NOW, { time: false })).toBe("hier");
    expect(fmtRelativeDay(at(2026, 11, 10, 1, 0), "fr", NOW, { time: false })).toBe("aujourd'hui");
    expect(fmtRelativeDay(at(2026, 11, 6, 21, 4), "fr", NOW, { time: false })).toBe("vendredi");
    expect(fmtRelativeDay(at(2026, 11, 1, 21, 4), "fr", NOW, { time: false })).toBe("1 nov.");
  });

  it("never pretends about a missing or future date", () => {
    expect(fmtRelativeDay(null)).toBe("—");
    expect(fmtRelativeDay(Number.NaN)).toBe("—");
    // clock skew: tomorrow is shown as a plain date
    expect(fmtRelativeDay(at(2026, 11, 11, 10, 0), "fr", NOW)).toBe("11 nov. 10:00");
  });
});
