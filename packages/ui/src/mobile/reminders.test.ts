import { describe, expect, it } from "vitest";
import type { RacePick } from "../net/races-api";
import { FakeNotifications } from "./doubles";
import {
  cancelAllReminders,
  ensurePermission,
  isReminderExtra,
  MAX_PENDING,
  notificationId,
  planReminders,
  SOUND,
  syncReminders,
} from "./reminders";

const START = 1_762_892_700_000; // 2026-11-11 20:25 UTC
const MIN = 60_000;

function pick(
  race: string,
  start: number,
  cat: RacePick["cat"] = "daily",
  eve: number | null = null,
) {
  return {
    id: `${race}@${start}`,
    race_id: race,
    cat,
    src: "rc",
    name: "LMP3 Fixed",
    level: cat === "daily" ? "beginner" : null,
    track: "Monza",
    duration: "32 min",
    image: null,
    start_ms: start,
    eve_ms: eve,
  } satisfies RacePick;
}

describe("reminders as local notifications", () => {
  it("plans one reminder per delay, future ones only", () => {
    const plan = planReminders([pick("race", START)], [15, 5, 0], START - 10 * MIN);
    expect(plan.map((p) => p.key)).toEqual([`race@${START}#5`, `race@${START}#0`]);
    expect(plan.map((p) => p.at)).toEqual([START - 5 * MIN, START]);
    expect(plan[0]?.reminder).toMatchObject({ kind: "lead", lead_min: 5, due_ms: START - 5 * MIN });
    // Due in less than a second: already too late (iOS would ring it at once).
    expect(planReminders([pick("race", START)], [0], START - 500)).toEqual([]);
  });

  it("rings special events the evening before and the day itself", () => {
    const day = START; // 10:00 of the event, as the page picks it
    const eve = START - 16 * 60 * MIN;
    const plan = planReminders([pick("le-mans", day, "special", eve)], [15, 5], eve - MIN);
    expect(plan.map((p) => [p.key, p.reminder.kind, p.at])).toEqual([
      [`le-mans@${day}#eve`, "eve", eve],
      [`le-mans@${day}#day`, "day", day],
    ]);
    // After the evening: only the day.
    expect(planReminders([pick("le-mans", day, "special", eve)], [15], eve + MIN)).toHaveLength(1);
  });

  it("keeps the 64 nearest (iOS limit), with stable unique identifiers", () => {
    const picks = Array.from({ length: 30 }, (_, i) => pick(`r${i}`, START + i * 60 * MIN));
    const plan = planReminders(picks, [60, 30, 15], START - 2 * 60 * MIN);
    expect(plan).toHaveLength(MAX_PENDING);
    const ats = plan.map((p) => p.at);
    expect(ats).toEqual([...ats].sort((a, b) => a - b));
    // the furthest ones wait
    expect(Math.max(...ats)).toBeLessThan(START + 29 * 60 * MIN - 15 * MIN);
    const ids = plan.map((p) => p.id);
    expect(new Set(ids).size).toBe(ids.length);
    expect(ids.every((id) => Number.isInteger(id) && id > 0 && id <= 0x7fffffff)).toBe(true);
    // Same picks, same identifiers.
    expect(planReminders(picks, [60, 30, 15], START - 2 * 60 * MIN).map((p) => p.id)).toEqual(ids);
    expect(notificationId(`r0@${START}#15`)).toBe(notificationId(`r0@${START}#15`));
  });

  it("schedules what is missing, keeps what is pending, cancels what is gone: no duplicate", async () => {
    const n = new FakeNotifications();
    n.permission = "granted";
    const now = START - 60 * MIN;
    const a = pick("a", START);
    const b = pick("b", START + 30 * MIN);
    let r = await syncReminders(n, planReminders([a, b], [15, 5], now), "fr");
    expect(r).toMatchObject({ blocked: false, scheduled: 4, cancelled: 0, kept: 0 });
    const first = n.pending[0];
    expect(first?.sound).toBe(SOUND);
    expect(first?.schedule?.at).toBeInstanceOf(Date);
    expect(first?.title).toBe("Départ dans 15 min");
    expect(first?.body).toMatch(/^LMP3 Fixed — Monza, \d\d:\d\d$/);
    expect(isReminderExtra(first?.extra)).toBe(true);

    // Same plan again: nothing moves.
    r = await syncReminders(n, planReminders([a, b], [15, 5], now), "fr");
    expect(r).toMatchObject({ scheduled: 0, cancelled: 0, kept: 4 });
    expect(n.pending).toHaveLength(4);

    // A pick removed, a delay changed.
    r = await syncReminders(n, planReminders([a], [10], now), "fr");
    expect(r).toMatchObject({ scheduled: 1, cancelled: 4, kept: 0 });
    expect(n.pending.map((p) => p.id)).toEqual([notificationId(`a@${START}#10`)]);

    // Language changed: scheduled again with the new text, still one.
    await syncReminders(n, planReminders([a], [10], now), "en");
    expect(n.pending).toHaveLength(1);
    expect(n.pending[0]?.title).toBe("Start in 10 min");

    // Other notifications of the application are not touched.
    n.pending.push({ id: 7, title: "x", body: "y", extra: { other: true } });
    expect(await cancelAllReminders(n)).toBe(1);
    expect(n.pending.map((p) => p.id)).toEqual([7]);
  });

  it("never asks for the permission itself, and schedules nothing without it", async () => {
    const n = new FakeNotifications();
    const r = await syncReminders(
      n,
      planReminders([pick("a", START)], [5], START - 60 * MIN),
      "fr",
    );
    expect(r.blocked).toBe(true);
    expect(n.requests).toBe(0);
    expect(n.scheduled).toHaveLength(0);
    n.permission = "denied";
    expect((await syncReminders(n, [], "fr")).blocked).toBe(true);
  });

  it("asks once when the system has not asked yet", async () => {
    const n = new FakeNotifications();
    expect(await ensurePermission(n)).toBe("granted");
    expect(n.requests).toBe(1);
    expect(await ensurePermission(n)).toBe("granted");
    expect(n.requests).toBe(1);
    const d = new FakeNotifications();
    d.permission = "denied";
    expect(await ensurePermission(d)).toBe("denied");
    expect(d.requests).toBe(0);
  });
});
