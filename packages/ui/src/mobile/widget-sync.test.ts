import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import type { RacePick, RaceSources } from "../net/races-api";
import { FakeNative, FakePrefs } from "./doubles";
import { defaultRaceSettings } from "./race-book";
import { IMAGE_RETRY_MS, KEEP_SCHEDULE_MS, WIDGET_KEYS, WidgetSync } from "./widget-sync";
import type { WidgetPayload } from "./widgets";

const here = dirname(fileURLToPath(import.meta.url));
const RC_HTML = readFileSync(join(here, "../races/fixtures/racecontrol-2026-11-11.html"), "utf8");
// The fixture was saved on 11 Nov. 2026 in the evening (see races/parse.test.ts).
const READ_AT = Date.UTC(2026, 10, 11, 19, 0);
const HOUR = 3600e3;

function sources(html: string | null, at = READ_AT): RaceSources {
  return {
    enabled: true,
    sources: [
      {
        id: "racecontrol",
        url: "https://racecontrol.gg/",
        state: html ? "ok" : "error",
        fetched_at_ms: html ? at : null,
        attempted_at_ms: at,
        error: html ? null : "site unreachable",
        html,
      },
      {
        id: "lmutools",
        url: "https://lmu.tools/schedule",
        state: "error",
        fetched_at_ms: null,
        attempted_at_ms: at,
        error: "site unreachable",
        html: null,
      },
    ],
  };
}

function setup(over: { enabled?: boolean; src?: RaceSources | null; now?: number } = {}) {
  const prefs = new FakePrefs();
  const native = new FakeNative();
  let picks: RacePick[] = [];
  let src: RaceSources | null = over.src === undefined ? sources(RC_HTML) : over.src;
  let now = over.now ?? READ_AT;
  const sync = new WidgetSync({
    prefs,
    native,
    picks: async () => picks,
    settings: async () => defaultRaceSettings(),
    enabled: async () => over.enabled ?? true,
    sources: () => src,
    lang: () => "fr",
    now: () => now,
  });
  return {
    prefs,
    native,
    sync,
    setPicks: (p: RacePick[]) => {
      picks = p;
    },
    setSources: (s: RaceSources | null) => {
      src = s;
    },
    setNow: (t: number) => {
      now = t;
    },
  };
}

const last = (n: FakeNative) => JSON.parse(n.written.at(-1) as string) as WidgetPayload;

describe("WidgetSync", () => {
  it("writes the block once, then only when it changes", async () => {
    const { native, sync, setPicks } = setup();
    await sync.update();
    await sync.update();
    expect(native.written).toHaveLength(1);
    const p = last(native);
    expect(p.v).toBe(1);
    expect(p.sched).toBe("on");
    expect(p.races.length).toBeGreaterThan(0);
    expect(native.written[0]?.length ?? 0).toBeLessThan(48 * 1024);

    const first = p.races[0];
    const start = first?.s[0] as number;
    setPicks([
      {
        id: `${first?.id}@${start}`,
        race_id: first?.id as string,
        cat: "daily",
        src: "rc",
        name: first?.name as string,
        level: first?.lvl ?? null,
        track: first?.track as string,
        duration: first?.dur ?? "",
        image: null,
        start_ms: start,
        eve_ms: null,
      },
    ]);
    await sync.update();
    expect(native.written).toHaveLength(2);
    expect(last(native).mine).toHaveLength(1);
    expect(sync.lastWrite).toMatchObject({ ok: true, status: 0 });
  });

  it("keeps the last schedule for the widgets when the pages cannot be read (3 days)", async () => {
    const { native, sync, setSources, setNow, prefs } = setup();
    await sync.update();
    expect(prefs.data.has(WIDGET_KEYS.schedule)).toBe(true);
    setSources(sources(null, READ_AT + HOUR));
    setNow(READ_AT + HOUR);
    await sync.update();
    expect(last(native).sched).toBe("on");
    expect(last(native).races.length).toBeGreaterThan(0);
    setNow(READ_AT + KEEP_SCHEDULE_MS + HOUR);
    await sync.update();
    expect(last(native)).toMatchObject({ sched: "none", races: [] });
  });

  it("schedule off: nothing of it is written", async () => {
    const { native, sync } = setup({ enabled: false });
    await sync.update();
    expect(last(native)).toMatchObject({ sched: "off", races: [] });
  });

  it("a refusal of the store is reported and written again next time", async () => {
    const { native, sync } = setup();
    native.status = -34018;
    await sync.update();
    expect(sync.lastWrite).toMatchObject({ ok: false, status: -34018 });
    native.status = 0;
    await sync.update();
    expect(native.written).toHaveLength(1);
  });

  it("pictures: sent after the block, only when the list changed, retried after a failure", async () => {
    const { native, sync, setNow, setPicks } = setup();
    await sync.update();
    expect(native.written).toHaveLength(1);
    expect(native.imageCalls).toHaveLength(1);
    const items = native.imageCalls[0] ?? [];
    expect(items.length).toBeGreaterThan(0);
    expect(items.length).toBeLessThanOrEqual(8);
    const block = last(native);
    const keys = new Set(
      [...block.races, ...block.mine].map((r) => r.img).filter((k): k is string => !!k),
    );
    for (const i of items) {
      expect(i.url).toMatch(/^https:\/\//);
      expect(keys.has(i.key)).toBe(true);
    }
    expect(sync.lastImages).toMatchObject({ written: items.length, failed: 0 });

    // Same list: not sent again (even when the block changes for something else).
    await sync.update();
    await sync.update(true);
    expect(native.imageCalls).toHaveLength(1);
    setPicks([]);
    await sync.update();
    expect(native.imageCalls).toHaveLength(1);

    // A failed download: tried again 15 minutes later, not before.
    const first = items[0];
    if (!first) throw new Error("no picture");
    native.images.clear();
    native.failing.add(first.url);
    await sync.update(true);
    expect(native.imageCalls).toHaveLength(1);
    const s2 = sync as unknown as { lastImagesKey: string | null };
    s2.lastImagesKey = null;
    await sync.update(true);
    expect(native.imageCalls).toHaveLength(2);
    expect(sync.lastImages?.failed).toBe(1);
    await sync.update(true);
    expect(native.imageCalls).toHaveLength(2);
    native.failing.clear();
    setNow(READ_AT + IMAGE_RETRY_MS + 1);
    await sync.update(true);
    expect(native.imageCalls.length).toBeGreaterThanOrEqual(3);
    expect(sync.lastImages?.failed).toBe(0);
    const d = await sync.diagnostics();
    expect(d.images).toBe(native.images.size);
    expect(d.imageBytes).toBe(native.images.size * native.imageSize);
  });

  it("pictures: none while the schedule is off (the kept ones are deleted)", async () => {
    const { native, sync } = setup({ enabled: false });
    native.images.set("iold", 1000);
    await sync.update();
    expect(native.imageCalls).toEqual([[]]);
    expect(native.images.size).toBe(0);
  });

  it("pictures: a refused block sends no list", async () => {
    const { native, sync } = setup();
    native.status = -34018;
    await sync.update();
    expect(native.imageCalls).toHaveLength(0);
  });

  it("Live Activity: setting on by default, off ends it", async () => {
    const { native, sync, setPicks } = setup();
    const views: unknown[] = [];
    const s2 = sync as unknown as { opts: { onActivity?: (v: unknown) => void } };
    s2.opts.onActivity = (v) => views.push(v);
    const p: RacePick = {
      id: `x@${READ_AT + HOUR}`,
      race_id: "x",
      cat: "daily",
      src: "rc",
      name: "Sprint",
      level: "beginner",
      track: "Spa",
      duration: "20 min",
      image: null,
      start_ms: READ_AT + HOUR,
      eve_ms: null,
    };
    setPicks([p]);
    expect(await sync.liveActivityEnabled()).toBe(true);
    await sync.activity({ kind: "foreground" });
    expect(native.started).toHaveLength(1);
    expect(native.started[0]).toMatchObject({ pickId: p.id, startMs: p.start_ms, lang: "fr" });
    await sync.activity({ kind: "foreground" });
    expect(native.started).toHaveLength(1);
    await sync.setLiveActivityEnabled(false);
    expect(native.running).toHaveLength(0);
    expect(views.at(-1)).toMatchObject({ enabled: false, pickId: null });
  });
});
