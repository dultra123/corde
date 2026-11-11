import { describe, expect, it } from "vitest";
import { lapNo, lapStatus, lapStatusHintKey, lapStatusKey } from "./laps";

const base = { valid: true, in_lap: false, out_lap: false, timed_by_game: true, note: null };

describe("lap numbering", () => {
  it("shows the game's lap number (records are 0-based)", () => {
    expect(lapNo(0)).toBe(1);
    expect(lapNo(20)).toBe(21);
  });
});

describe("lapStatus", () => {
  it("a timed flying lap counts", () => {
    expect(lapStatus(base)).toBe("counted");
  });
  it("uses the note when there is one", () => {
    expect(lapStatus({ ...base, note: "start_lap" })).toBe("start_lap");
    expect(lapStatus({ ...base, valid: false, timed_by_game: false, note: "aborted" })).toBe(
      "aborted",
    );
    expect(lapStatus({ ...base, valid: false, timed_by_game: false, note: "not_timed" })).toBe(
      "not_timed",
    );
  });
  it("falls back on the flags for laps recorded before notes existed", () => {
    expect(lapStatus({ valid: false, in_lap: false, out_lap: true })).toBe("out_lap");
    expect(lapStatus({ valid: false, in_lap: true, out_lap: false })).toBe("in_lap");
    expect(lapStatus({ valid: false, in_lap: false, out_lap: false })).toBe("invalid");
    expect(lapStatus({ valid: false, in_lap: false, out_lap: false, timed_by_game: false })).toBe(
      "not_timed",
    );
  });
  it("maps to an i18n key", () => {
    expect(lapStatusKey("not_timed")).toBe("lapStatus.not_timed");
    expect(lapStatusHintKey("start_lap")).toBe("lapStatus.hint.start_lap");
  });
});
