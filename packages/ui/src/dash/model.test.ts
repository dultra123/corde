import type { LapRecord } from "@corde/protocol";
import { describe, expect, it } from "vitest";
import {
  chooseView,
  classCount,
  DASH_PAGES,
  dashPresentation,
  deltaView,
  energyAutonomy,
  flagsOf,
  flagTone,
  frontBias,
  fuelAutonomy,
  fuelToFinish,
  gapView,
  hasHybrid,
  isPhone,
  LED_COLORS,
  LED_COUNT,
  lapLimit,
  lapsLeft,
  lapsLeftTimed,
  pageIndex,
  playerDelta,
  revState,
  setting,
  stepPage,
} from "./model";

function lap(n: number, over: Partial<LapRecord> = {}): LapRecord {
  return {
    lap_number: n,
    lap_time_s: 100,
    s1: null,
    s2: null,
    s3: null,
    valid: true,
    in_lap: false,
    out_lap: false,
    fuel_used_l: 3,
    virtual_energy_used: 0.05,
    top_speed_kmh: 300,
    front_compound: "",
    rear_compound: "",
    avg_tyre_temp_c: [0, 0, 0, 0],
    avg_tyre_pressure_kpa: [0, 0, 0, 0],
    max_brake_temp_c: [0, 0, 0, 0],
    wear_remaining_end: [1, 1, 1, 1],
    track_c: 30,
    ambient_c: 20,
    rain: 0,
    session_elapsed_end_s: 0,
    timed_by_game: true,
    note: null,
    ...over,
  };
}

describe("pages", () => {
  it("has the four pages in order", () => {
    expect(DASH_PAGES).toEqual(["course", "tyres", "energy", "timing"]);
  });
  it("reads a page name, the first page when unknown", () => {
    expect(pageIndex("energy")).toBe(2);
    expect(pageIndex("nope")).toBe(0);
    expect(pageIndex(null)).toBe(0);
  });
  it("steps without wrapping around", () => {
    expect(stepPage(0, -1)).toBe(0);
    expect(stepPage(0, 1)).toBe(1);
    expect(stepPage(3, 1)).toBe(3);
    expect(stepPage(9, 0)).toBe(3);
    expect(stepPage(Number.NaN, 0)).toBe(0);
  });
});

describe("which view the Direct page shows", () => {
  const pc = { app: false, coarse: false, width: 1920, height: 1080, forced: false };
  const phone = { app: false, coarse: true, width: 852, height: 393, forced: false };
  const tablet = { app: false, coarse: true, width: 1180, height: 820, forced: false };

  it("auto: the dashboard on a phone, the widgets on a PC and a tablet", () => {
    expect(chooseView("auto", phone)).toBe("dash");
    expect(chooseView("auto", { ...phone, width: 393, height: 852 })).toBe("dash");
    expect(chooseView("auto", pc)).toBe("widgets");
    expect(chooseView("auto", tablet)).toBe("widgets");
    // a small window on a PC (mouse) stays on the widgets
    expect(chooseView("auto", { ...pc, width: 400, height: 700 })).toBe("widgets");
  });
  it("auto: always the dashboard in the phone application", () => {
    expect(chooseView("auto", { ...tablet, app: true })).toBe("dash");
  });
  it("follows the setting", () => {
    expect(chooseView("dash", pc)).toBe("dash");
    expect(chooseView("widgets", phone)).toBe("widgets");
    expect(chooseView("widgets", { ...phone, app: true })).toBe("widgets");
  });
  it("#/dash is always the dashboard", () => {
    expect(chooseView("widgets", { ...pc, forced: true })).toBe("dash");
  });
  it("the dashboard: full screen on a phone, inside the page on a PC or a tablet", () => {
    expect(dashPresentation(phone, false)).toBe("full");
    expect(dashPresentation({ ...tablet, app: true }, false)).toBe("full");
    expect(dashPresentation({ ...pc, forced: true }, false)).toBe("full");
    expect(dashPresentation(pc, false)).toBe("page");
    expect(dashPresentation(tablet, false)).toBe("page");
    // full screen on request
    expect(dashPresentation(pc, true)).toBe("full");
  });
  it("a phone is a touch screen with a short side of 500 px at most", () => {
    expect(isPhone({ coarse: true, width: 932, height: 430 })).toBe(true);
    expect(isPhone({ coarse: true, width: 500, height: 900 })).toBe(true);
    expect(isPhone({ coarse: true, width: 768, height: 1024 })).toBe(false);
    expect(isPhone({ coarse: false, width: 393, height: 852 })).toBe(false);
  });
});

describe("rev lights", () => {
  it("are five green, five yellow, five red", () => {
    expect(LED_COLORS).toHaveLength(LED_COUNT);
    expect(LED_COLORS.slice(0, 5).every((c) => c === "g")).toBe(true);
    expect(LED_COLORS.slice(5, 10).every((c) => c === "y")).toBe(true);
    expect(LED_COLORS.slice(10).every((c) => c === "r")).toBe(true);
  });
  it("light up from 55 % to 96 % of the maximum", () => {
    expect(revState(5000, 10000).lit).toBe(0);
    expect(revState(5500, 10000).lit).toBe(0);
    expect(revState(5600, 10000).lit).toBe(1);
    expect(revState(7550, 10000).lit).toBe(8);
    expect(revState(9500, 10000).lit).toBe(15);
    expect(revState(9600, 10000).lit).toBe(15);
    expect(revState(9600, 10000).shift).toBe(false);
  });
  it("flash from the shift point (97 %)", () => {
    expect(revState(9700, 10000)).toEqual({ lit: 15, shift: true, limiter: false });
    expect(revState(10500, 10000).shift).toBe(true);
  });
  it("alternate with the pit limiter, whatever the revs", () => {
    expect(revState(9900, 10000, true)).toEqual({ lit: 0, shift: false, limiter: true });
  });
  it("stay off without a maximum", () => {
    expect(revState(5000, 0)).toEqual({ lit: 0, shift: false, limiter: false });
    expect(revState(Number.NaN, 8000).lit).toBe(0);
  });
});

describe("flags", () => {
  const session = {
    phase: "green" as const,
    yellow_flag: "none" as const,
    sector_flags: [0, 0, 0] as [number, number, number],
  };
  const player = (sector: number, finish: "none" | "finished" = "none") => ({
    finish_status: finish,
    lap: { current_sector: sector },
  });

  it("none on a green track", () => {
    expect(flagsOf(session, player(1))).toEqual({ yellow: false, chequered: false, blue: false });
    expect(flagsOf(null, null)).toEqual({ yellow: false, chequered: false, blue: false });
  });
  it("yellow for a full course yellow", () => {
    expect(flagsOf({ ...session, phase: "full_course_yellow" }, player(1)).yellow).toBe(true);
    expect(flagsOf({ ...session, yellow_flag: "pits_closed" }, player(1)).yellow).toBe(true);
    expect(flagsOf({ ...session, yellow_flag: "invalid" }, player(1)).yellow).toBe(false);
  });
  it("yellow only in the driver's own sector", () => {
    const s = { ...session, sector_flags: [0, 1, 0] as [number, number, number] };
    expect(flagsOf(s, player(2)).yellow).toBe(true);
    expect(flagsOf(s, player(1)).yellow).toBe(false);
    expect(flagsOf(s, null).yellow).toBe(false);
  });
  it("chequered once the driver finished, blue from the standing", () => {
    expect(flagsOf(session, player(1, "finished")).chequered).toBe(true);
    const standings = [
      { is_player: false, flag_blue: true },
      { is_player: true, flag_blue: true },
    ];
    expect(flagsOf(session, player(1), standings).blue).toBe(true);
    expect(flagsOf(session, player(1), [standings[0] as (typeof standings)[0]]).blue).toBe(false);
  });
  it("the frame takes the most important one", () => {
    expect(flagTone({ yellow: true, chequered: true, blue: true })).toBe("chequered");
    expect(flagTone({ yellow: true, chequered: false, blue: true })).toBe("yellow");
    expect(flagTone({ yellow: false, chequered: false, blue: true })).toBe("blue");
    expect(flagTone({ yellow: false, chequered: false, blue: false })).toBeNull();
  });
});

describe("delta", () => {
  it("is signed, three decimals, coloured", () => {
    expect(deltaView(-0.342)).toEqual({ text: "−0.342", tone: "faster", frac: -0.342 });
    expect(deltaView(0.5).tone).toBe("slower");
    expect(deltaView(0.5).text).toBe("+0.500");
    expect(deltaView(0).tone).toBe("even");
  });
  it("fills the bar at one second", () => {
    expect(deltaView(-3).frac).toBe(-1);
    expect(deltaView(2).frac).toBe(1);
  });
  it("shows « — » without a best lap", () => {
    expect(deltaView(null)).toEqual({ text: "—", tone: "none", frac: 0 });
    const p = { lap: { best_lap_s: null, delta_best_s: -1 } };
    expect(playerDelta(p as never)).toBeNull();
    expect(playerDelta({ lap: { best_lap_s: 100, delta_best_s: -1 } } as never)).toBe(-1);
  });
});

describe("gaps", () => {
  it("negative ahead, positive behind, like the widgets", () => {
    expect(gapView(1.84, "ahead")).toEqual({ text: "−1.8", tone: "plain" });
    expect(gapView(2.5, "behind")).toEqual({ text: "+2.5", tone: "plain" });
  });
  it("a close car: green ahead, red behind", () => {
    expect(gapView(0.6, "ahead").tone).toBe("close-ahead");
    expect(gapView(0.6, "behind").tone).toBe("close-behind");
  });
  it("nobody there: « — »", () => {
    expect(gapView(null, "ahead")).toEqual({ text: "—", tone: "none" });
  });
});

describe("session and energy", () => {
  it("a lap limit only when the session has one", () => {
    expect(lapLimit({ max_laps: 38 })).toBe(38);
    expect(lapLimit({ max_laps: 0 })).toBeNull();
    expect(lapLimit({ max_laps: 2147483647 })).toBeNull();
    expect(lapsLeft({ max_laps: 38 }, { lap: { laps_completed: 11 } } as never)).toBe(27);
    expect(lapsLeft({ max_laps: 0 }, { lap: { laps_completed: 11 } } as never)).toBeNull();
  });
  it("laps that fit in a timed session, from the recent pace", () => {
    const laps = [lap(1, { out_lap: true, lap_time_s: 200 }), lap(2), lap(3)];
    expect(lapsLeftTimed({ end_s: 3600, time_remaining_s: 950 }, laps)).toBe(10);
    expect(lapsLeftTimed({ end_s: 0, time_remaining_s: 950 }, laps)).toBeNull();
    expect(lapsLeftTimed({ end_s: 3600, time_remaining_s: 950 }, [])).toBeNull();
  });
  it("autonomy from the laps' use (shared with the Fuel widget)", () => {
    const laps = [lap(1), lap(2)];
    expect(fuelAutonomy({ fuel_l: 30 }, laps)).toEqual({ perLap: 3, laps: 10 });
    expect(fuelAutonomy({ fuel_l: 30 }, [])).toBeNull();
    const ve = energyAutonomy({ virtual_energy: 0.5 }, laps);
    expect(ve?.laps).toBeCloseTo(10);
    expect(energyAutonomy({ virtual_energy: null }, laps)).toBeNull();
  });
  it("fuel missing (positive) or spare (negative) at the flag", () => {
    expect(fuelToFinish(30, 3, 12)).toBe(6);
    expect(fuelToFinish(30, 3, 5)).toBe(-15);
    expect(fuelToFinish(30, null, 5)).toBeNull();
    expect(fuelToFinish(30, 3, null)).toBeNull();
  });
  it("a hybrid only when the car reports one", () => {
    expect(hasHybrid({ battery_soc: null, motor_state: "unavailable" })).toBe(false);
    expect(hasHybrid({ battery_soc: 0.4, motor_state: "unavailable" })).toBe(true);
    expect(hasHybrid({ battery_soc: null, motor_state: "inactive" })).toBe(true);
  });
});

describe("the car's settings", () => {
  it("a setting the car does not have is left out", () => {
    expect(setting(4, 11)).toBe(4);
    expect(setting(0, 0)).toBeNull();
  });
  it("front brake bias in percent", () => {
    expect(frontBias({ brake_bias_rear: 0.455 })).toBeCloseTo(54.5);
    expect(frontBias({ brake_bias_rear: Number.NaN })).toBeNull();
    expect(frontBias({ brake_bias_rear: 2 })).toBeNull();
  });
  it("counts the classes on track", () => {
    expect(
      classCount([{ class_name: "Hyper" }, { class_name: "LMGT3" }, { class_name: "Hyper" }]),
    ).toBe(2);
    expect(classCount([])).toBe(0);
  });
});
