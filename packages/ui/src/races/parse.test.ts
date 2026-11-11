import { describe, expect, it } from "vitest";
import lmuTools from "./fixtures/lmutools-assumed.html?raw";
import raceControl from "./fixtures/racecontrol-2026-11-11.html?raw";
import stamps from "./fixtures/racecontrol-stamps.html?raw";
import {
  classTags,
  combine,
  fmtMin,
  frDuration,
  htmlLines,
  parseLmuTools,
  parseRaceControl,
  RaceParseError,
  sameRace,
  trackKey,
  viewRaces,
} from "./parse";

/** When racecontrol.gg was read: Wednesday 11 Nov 2026, 20:10 UTC. */
const NOW = Date.UTC(2026, 10, 11, 20, 10);
const at = (h: number, m: number, day = 11) => Date.UTC(2026, 10, day, h, m);

describe("racecontrol.gg (sample after the real page of 11 Nov 2026)", () => {
  const rc = parseRaceControl(raceControl, NOW);

  it("reads the four daily races of the Le Mans Ultimate section only", () => {
    expect(rc.races.map((r) => [r.level, r.name, r.track, r.duration])).toEqual([
      ["advanced", "One Stint Sprint", "Monza", "55 min"],
      ["beginner", "LMP3 Fixed", "Monza", "32 min"],
      ["intermediate", "Prototype Fixed", "Circuit De Barcelona Catalunya", "44 min"],
      ["advanced", "ELMS Super 60", "Le Mans", "1 h 13"],
    ]);
    expect(rc.races.some((r) => r.name === "Mazda MX-5 Cup")).toBe(false);
    expect(rc.races.every((r) => r.cat === "daily" && r.src === "rc")).toBe(true);
  });

  it("guesses the site's time zone from the first start (no zone on the page)", () => {
    expect(rc.method).toBe("auto");
    // 4:25pm on the page is the first start after 20:10 UTC → the page is in UTC−4.
    expect(rc.offset).toBe(-4);
    const sprint = rc.races[0];
    expect(sprint?.starts[0]).toBe(at(20, 25));
    expect(sprint?.starts.at(-1)).toBe(at(3, 55, 12));
    expect(sprint?.starts).toHaveLength(6);
    expect(rc.races[1]?.starts).toHaveLength(10);
    expect(rc.races[2]?.starts).toHaveLength(8);
    expect(rc.races[3]?.starts[0]).toBe(at(20, 55));
  });

  it("keeps the pictures of the cards, not logos nor avatars", () => {
    expect(rc.races[0]?.image).toBe(
      "https://img.example/rc/daily/one-stint-sprint-monza.webp?w=800",
    );
    expect(rc.races.every((r) => r.image && !/logo|avatar/.test(r.image))).toBe(true);
  });

  it("reads absolute times when the page gives them", () => {
    const s = parseRaceControl(stamps, NOW);
    expect(s.method).toBe("horodatage");
    expect(s.offset).toBeNull();
    expect(s.races[0]?.starts).toEqual([at(20, 30), at(21, 15)]);
  });

  it("throws on a page without races", () => {
    expect(() => parseRaceControl("<html><body>Maintenance</body></html>", NOW)).toThrow(
      RaceParseError,
    );
  });
});

describe("lmu.tools (sample page, shape assumed from the owner's script)", () => {
  const lt = parseLmuTools(lmuTools, NOW);

  it("finds the site's time zone from the « in … » countdowns", () => {
    expect(lt.offset).toBe(1);
  });

  it("reads the weekly races with their format", () => {
    expect(lt.weekly.map((r) => r.name)).toEqual(["GT3 Sprint Weekly", "Hypercar Weekly"]);
    const gt3 = lt.weekly[0];
    expect(gt3?.id).toBe("weekly-gt3-sprint-weekly");
    expect(gt3?.track).toBe("Algarve International Circuit");
    expect(gt3?.classes).toEqual(["LMGT3"]);
    expect(gt3?.fmt).toEqual({ p: 10, q: 10, r: 40 });
    expect(gt3?.duration).toBe("40 min");
    expect(gt3?.setup).toBe("fixed");
    expect(gt3?.srMult).toBe(1.5);
    // Friday 19:00 in UTC+1.
    expect(gt3?.starts).toEqual([at(18, 0, 13), at(18, 0, 14), at(14, 0, 15)]);
    // A start without countdown: next occurrence after the previous one.
    expect(lt.weekly[1]?.starts).toEqual([at(19, 0, 14), at(19, 0, 15)]);
    expect(lt.weekly[1]?.duration).toBe("1 h 30");
  });

  it("gives the daily races a level from their multiplier", () => {
    expect(lt.daily.map((r) => [r.id, r.level])).toEqual([
      ["one-stint-sprint-advanced", "advanced"],
      ["lmp3-fixed-beginner", "beginner"],
    ]);
    expect(lt.daily[0]?.starts).toEqual([at(20, 25), at(21, 55)]);
  });

  it("reads the championships", () => {
    const c = lt.champ[0];
    expect(c?.name).toBe("RC Pro LMGT3 Championship");
    expect(c?.setup).toBe("open");
    expect(c?.starts).toEqual([at(19, 0, 12), at(13, 0, 14)]);
  });

  it("reads the special events from their poll link", () => {
    expect(lt.special.map((r) => [r.name, r.track, r.hours, r.duration, r.classes])).toEqual([
      [
        "Sebring 1000 Miles",
        "Sebring International Raceway",
        8,
        "8 h",
        ["HYPERCAR", "LMP2", "LMGT3"],
      ],
      ["6h Circuit à annoncer", "Circuit à annoncer", 6, "6 h", ["LMGT3"]],
    ]);
    const s = lt.special[0];
    expect(s?.from).toBe(new Date(2026, 10, 21, 0, 0, 0).getTime());
    expect(s?.remindAt).toBe(new Date(2026, 10, 21, 10, 0, 0).getTime());
    expect(s?.to).toBe(new Date(2026, 10, 22, 23, 0, 0).getTime() + 59 * 60e3);
  });

  it("throws on a page without races", () => {
    expect(() => parseLmuTools("<main><h2>Schedule</h2></main>", NOW)).toThrow(RaceParseError);
  });
});

describe("the whole schedule", () => {
  const rc = parseRaceControl(raceControl, NOW);
  const lt = parseLmuTools(lmuTools, NOW);

  it("completes the daily races of racecontrol.gg with lmu.tools", () => {
    const all = combine(rc, lt, null);
    expect(all.method).toBe("auto");
    expect(all.extrasAt).toBe(NOW);
    const sprint = all.races.find((r) => r.id === "one-stint-sprint-advanced");
    expect(sprint?.classes).toEqual(["HYPERCAR", "LMP2"]);
    expect(sprint?.fmt).toEqual({ p: 10, q: 10, r: 55 });
    expect(sprint?.starts[0]).toBe(at(20, 25));
    const lmp3 = all.races.find((r) => r.id === "lmp3-fixed-beginner");
    expect(lmp3?.classes).toEqual(["LMP3"]);
    expect(all.races.map((r) => r.cat)).toEqual([
      "daily",
      "daily",
      "daily",
      "daily",
      "weekly",
      "weekly",
      "special",
      "special",
      "champ",
    ]);
  });

  it("falls back on lmu.tools' daily races without racecontrol.gg", () => {
    const all = combine(null, lt, null);
    expect(all.races.filter((r) => r.cat === "daily").map((r) => r.id)).toEqual([
      "one-stint-sprint-advanced",
      "lmp3-fixed-beginner",
    ]);
    expect(all.method).toBeNull();
  });

  it("view: hour correction on racecontrol.gg only, past starts removed", () => {
    const all = combine(rc, lt, null);
    const v = viewRaces(all.races, 1, at(21, 25) + 30_000);
    const sprint = v.find((r) => r.id === "one-stint-sprint-advanced");
    expect(sprint?.starts[0]).toBe(at(21, 25)); // 20:25 + 1 h, still within the minute
    const gt3 = v.find((r) => r.id === "weekly-gt3-sprint-weekly");
    expect(gt3?.starts[0]).toBe(at(18, 0, 13));
    const later = viewRaces(all.races, 0, at(4, 0, 12));
    expect(later.filter((r) => r.cat === "daily")).toHaveLength(0);
    expect(later.filter((r) => r.cat === "special")).toHaveLength(2);
  });
});

describe("helpers", () => {
  it("formats durations like the script", () => {
    expect(fmtMin(55)).toBe("55 min");
    expect(fmtMin(73)).toBe("1 h 13");
    expect(fmtMin(120)).toBe("2 h");
    expect(fmtMin(0)).toBe("");
    expect(frDuration("1h 13 minutes")).toBe("1 h 13");
    expect(frDuration("2 hours")).toBe("2 h");
  });

  it("matches the same race on both sites", () => {
    expect(trackKey("Autodromo Nazionale Monza")).toBe("monza");
    expect(trackKey("Circuit de la Sarthe (WEC)")).toBe("sarthe");
    expect(
      sameRace({ name: "LMP3 Fixed", track: "Monza" }, { name: "LMP3 Fixed", track: "Spa" }),
    ).toBe(false);
    expect(
      sameRace(
        { name: "The Winter One Stint Sprint", track: "Monza" },
        { name: "One Stint Sprint", track: "" },
      ),
    ).toBe(true);
    expect(classTags(["Hypercars & LMGT3", "LMDh, GTE", "lmp2"])).toEqual([
      "HYPERCAR",
      "LMGT3",
      "LMGTE",
      "LMP2",
    ]);
  });

  it("turns a page into text lines and never runs its scripts", () => {
    const lines = htmlLines(
      '<p>A&amp;B</p><script>alert("x")</script><a href="/x?a=1&amp;b=2">go</a>',
      "href",
    );
    expect(lines).toEqual(["A&B", "§HREF /x?a=1&b=2", "go"]);
  });
});
