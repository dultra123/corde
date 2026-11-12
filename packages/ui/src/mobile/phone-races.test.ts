import { describe, expect, it } from "vitest";
import type { RacePick } from "../net/races-api";
import { FakeHttp, FakeNotifications, FakePrefs, page } from "./doubles";
import { KEYS } from "./kv";
import { PhoneRaces } from "./phone-races";
import { RaceBook } from "./race-book";
import { PhoneSchedule } from "./schedule";

const NOW = 1_762_891_800_000;
const H = 3_600_000;

function pick(race: string, start: number): RacePick {
  return {
    id: `${race}@${start}`,
    race_id: race,
    cat: "daily",
    src: "rc",
    name: "GT3 Sprint",
    level: "intermediate",
    track: "Spa",
    duration: "30 min",
    image: null,
    start_ms: start,
    eve_ms: null,
  };
}

function setup() {
  const prefs = new FakePrefs();
  const notifications = new FakeNotifications();
  const http = new FakeHttp(() => page("<html></html>"));
  const permissions: string[] = [];
  const races = new PhoneRaces({
    prefs,
    schedule: new PhoneSchedule({ http, userAgent: "Corde/test", now: () => NOW }),
    book: new RaceBook(prefs, () => NOW),
    notifications,
    lang: () => "fr",
    now: () => NOW,
    onPermission: (p) => permissions.push(p),
  });
  return { prefs, notifications, http, races, permissions };
}

describe("the races of the iPhone application", () => {
  it("is off until the driver turns the schedule on, and reads nothing before", async () => {
    const { races, http, prefs } = setup();
    expect(await races.fetchEnabled()).toBe(false);
    expect((await races.fetchSources(false)).enabled).toBe(false);
    expect(http.calls).toHaveLength(0);
    expect(races.canSwitch()).toBe(true);
    await races.setEnabled(true);
    expect(prefs.data.get(KEYS.racesEnabled)).toBe("true");
    expect((await races.fetchSources(false)).sources.map((s) => s.state)).toEqual(["ok", "ok"]);
  });

  it("asks for the notifications at the first pick only, then plans the reminders", async () => {
    const { races, notifications, permissions } = setup();
    // Nothing at start-up: planning without picks never asks.
    await races.replan();
    expect(notifications.requests).toBe(0);

    await races.savePick(pick("a", NOW + 2 * H));
    await races.replan();
    expect(notifications.requests).toBe(1);
    expect(permissions).toEqual(["granted"]);
    // Default delays: 15 and 5 min.
    expect(notifications.pending.map((n) => n.title).sort()).toEqual([
      "Départ dans 15 min",
      "Départ dans 5 min",
    ]);

    await races.savePick(pick("b", NOW + 3 * H));
    await races.replan();
    expect(notifications.requests).toBe(1);
    expect(notifications.pending).toHaveLength(4);

    // Delays changed: planned again.
    await races.saveSettings({ leads: [30] });
    await races.replan();
    expect(notifications.pending.map((n) => n.title)).toEqual([
      "Départ dans 30 min",
      "Départ dans 30 min",
    ]);

    // Removed: cancelled, nothing left.
    await races.deletePick(`a@${NOW + 2 * H}`);
    await races.deletePick(`b@${NOW + 3 * H}`);
    await races.replan();
    expect(notifications.pending).toHaveLength(0);
  });

  it("schedules nothing when the driver refused, and says so", async () => {
    const { races, notifications, permissions } = setup();
    notifications.answer = "denied";
    await races.savePick(pick("a", NOW + 2 * H));
    const r = await races.replan();
    expect(permissions).toEqual(["denied"]);
    expect(r?.blocked).toBe(true);
    expect(notifications.pending).toHaveLength(0);
    expect(await races.permission()).toBe("denied");
  });

  it("answers 409 beyond 50 picks, like the server", async () => {
    const { races } = setup();
    for (let i = 0; i < 50; i += 1) await races.savePick(pick(`r${i}`, NOW + H + i * 60_000));
    await expect(races.savePick(pick("x", NOW + H))).rejects.toMatchObject({ status: 409 });
    expect(await races.fetchPicks()).toHaveLength(50);
  });
});
