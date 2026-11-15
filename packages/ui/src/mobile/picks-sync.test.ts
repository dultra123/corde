import { describe, expect, it } from "vitest";
import type { RacePick } from "../net/races-api";
import { emptyPickSet, mergePickSets, type PickSet } from "../races/picks-merge";
import { FakePrefs } from "./doubles";
import { KEYS } from "./kv";
import { PicksSync, type PicksSyncView } from "./picks-sync";
import { pickSetFromWire, RaceBook, validPickId } from "./race-book";
import { ServerBook } from "./servers";

const NOW = 1_762_891_800_000; // 2026-11-11 20:10 UTC
const H = 3_600_000;
const TOKEN = "ab".repeat(32);

function pick(race: string, start: number, extra: Partial<RacePick> = {}): RacePick {
  return {
    id: `${race}@${start}`,
    race_id: race,
    cat: "daily",
    src: "rc",
    name: "One Stint Sprint",
    level: "advanced",
    track: "Monza",
    duration: "55 min",
    image: null,
    start_ms: start,
    eve_ms: null,
    ...extra,
  };
}

const json = (status: number, body: unknown) =>
  new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json" },
  });

/** A Corde server as the phone sees it: it merges what it receives and answers its copy. */
class FakeServer {
  set: PickSet = emptyPickSet();
  /** `ok`, or what the server answers instead. */
  mode: "ok" | "off" | "old" | "revoked" = "ok";
  calls = 0;
  lastAuth: string | undefined;
  constructor(
    readonly now: () => number,
    readonly needsToken = false,
  ) {}

  answer(init: RequestInit): Response {
    this.calls += 1;
    this.lastAuth = (init.headers as Record<string, string> | undefined)?.Authorization;
    if (this.mode === "off") throw new TypeError("Load failed");
    if (this.mode === "old") return json(405, { error: "method not allowed" });
    if (this.mode === "revoked" || (this.needsToken && this.lastAuth !== `Bearer ${TOKEN}`))
      return json(401, { code: "unpaired" });
    this.set = mergePickSets(this.set, pickSetFromWire(JSON.parse(String(init.body))), this.now());
    return json(200, this.set);
  }

  /** As if picked on that server's own page. */
  put(p: RacePick, at: number): void {
    this.set = mergePickSets(
      this.set,
      { picks: [{ pick: p, changed_ms: at }], removed: [] },
      this.now(),
    );
  }

  ids(): string[] {
    return this.set.picks.map((s) => s.pick.id);
  }
}

async function setup() {
  let now = NOW;
  const clock = () => now;
  const prefs = new FakePrefs();
  const pc = new FakeServer(clock);
  const nas = new FakeServer(clock, true);
  const logs: string[] = [];
  /** While set, the PC only answers once it is resolved. */
  let pcGate: Promise<void> | null = null;
  const servers = new ServerBook({
    prefs,
    fetch: async (url, init = {}) => {
      if (url === "http://192.168.1.20:8471/api/races/picks/sync") {
        if (pcGate) await pcGate;
        return pc.answer(init);
      }
      if (url === "http://100.100.1.2:8471/api/races/picks/sync") return nas.answer(init);
      if (url === "http://100.100.1.2:8471/api/hub/pair")
        return json(201, { device: { id: "d1", name: "iPhone", admin: true }, token: TOKEN });
      throw new TypeError("Load failed");
    },
    log: (l) => logs.push(l),
    now: clock,
  });
  const pcEntry = await servers.add("192.168.1.20", "PC");
  const nasEntry = await servers.add("100.100.1.2", "NAS");
  if (!pcEntry.ok || !nasEntry.ok) throw new Error("servers");
  await servers.pair(nasEntry.entry.id, "12345678", "iPhone");
  const book = new RaceBook(prefs, clock);
  let merged = 0;
  const views: PicksSyncView[] = [];
  const sync = new PicksSync({
    servers,
    book,
    onMerged: () => {
      merged += 1;
    },
    onView: (v) => views.push(v),
    log: (l) => logs.push(l),
    now: clock,
  });
  return {
    pc,
    nas,
    book,
    sync,
    servers,
    prefs,
    logs,
    views,
    merged: () => merged,
    /** Holds the PC's next answers; call the result to let them go. */
    holdPc: (): (() => void) => {
      let open = () => {};
      pcGate = new Promise<void>((ok) => {
        open = ok;
      });
      return () => {
        pcGate = null;
        open();
      };
    },
    tick: (ms: number) => {
      now += ms;
    },
    outcomes: (v: PicksSyncView) => v.servers.map((s) => `${s.name}:${s.outcome}`),
  };
}

describe("« Mes courses » shared with the servers", () => {
  it("brings to the phone a race picked on the PC, through whichever server answers", async () => {
    const s = await setup();
    const a = pick("one-stint-sprint-advanced", NOW + 2 * H);
    // Picked on the PC; the PC passed it on to the home server by itself.
    s.pc.put(a, NOW - 1000);
    s.nas.put(a, NOW - 1000);
    // Away from home: the PC's address does not answer, the home server does (VPN).
    s.pc.mode = "off";
    const view = await s.sync.sync();
    expect(s.outcomes(view)).toEqual(["PC:unreachable", "NAS:ok"]);
    expect(await s.book.list()).toEqual([a]);
    expect(s.merged()).toBe(1);
    expect(s.nas.lastAuth).toBe(`Bearer ${TOKEN}`);
    // Nothing new: the next exchange changes nothing on the phone.
    await s.sync.sync();
    expect(s.merged()).toBe(1);
  });

  it("sends the phone's picks and removals to every server, and passes on what one taught it", async () => {
    const s = await setup();
    const mine = pick("weekly-gt3-sprint", NOW + 3 * H);
    await s.book.put(mine);
    // Only the PC knows this one (it is not paired with the home server).
    const theirs = pick("one-stint-sprint-beginner", NOW + 4 * H);
    s.pc.put(theirs, NOW - 500);
    const view = await s.sync.sync();
    expect(s.outcomes(view)).toEqual(["PC:ok", "NAS:ok"]);
    expect((await s.book.list()).map((p) => p.id)).toEqual([mine.id, theirs.id]);
    // The phone's pick is on both; the PC's pick reached the home server through the phone.
    expect(s.pc.ids()).toEqual([mine.id, theirs.id]);
    expect(s.nas.ids()).toEqual([mine.id, theirs.id]);

    // Removed on the phone: both servers forget it, and do not send it back.
    s.tick(1000);
    await s.book.remove(theirs.id);
    await s.sync.sync();
    expect(s.pc.ids()).toEqual([mine.id]);
    expect(s.nas.ids()).toEqual([mine.id]);
    expect((await s.book.list()).map((p) => p.id)).toEqual([mine.id]);
    expect(s.nas.set.removed.map((r) => r.id)).toEqual([theirs.id]);
  });

  it("leaves out a server that is off, too old or not paired, and keeps the picks", async () => {
    const s = await setup();
    const mine = pick("weekly-gt3-sprint", NOW + 3 * H);
    await s.book.put(mine);
    s.pc.mode = "old";
    s.nas.mode = "revoked";
    let view = await s.sync.sync();
    expect(s.outcomes(view)).toEqual(["PC:old", "NAS:unpaired"]);
    expect(await s.book.list()).toEqual([mine]);
    expect(s.merged()).toBe(0);
    s.pc.mode = "off";
    s.nas.mode = "off";
    view = await s.sync.sync();
    expect(s.outcomes(view)).toEqual(["PC:unreachable", "NAS:unreachable"]);
    expect(s.views.at(-1)).toEqual(view);
    // Back: everything is exchanged, nothing was lost meanwhile.
    s.pc.mode = "ok";
    s.nas.mode = "ok";
    await s.sync.sync();
    expect(s.pc.ids()).toEqual([mine.id]);
    expect(s.nas.ids()).toEqual([mine.id]);
  });

  it("asks a home server only with the phone's token, and never writes it down", async () => {
    const s = await setup();
    await s.book.put(pick("weekly-gt3-sprint", NOW + 3 * H));
    await s.sync.sync();
    // The PC needs no pairing: nothing is sent to it but the picks.
    expect(s.pc.lastAuth).toBeUndefined();
    expect(s.nas.lastAuth).toBe(`Bearer ${TOKEN}`);
    // Not paired any more: the home server is not even asked.
    const nasId = s.servers.list().find((e) => e.name === "NAS")?.id ?? "";
    await s.prefs.remove({ key: KEYS.token(nasId) });
    await s.servers.load();
    const before = s.nas.calls;
    const view = await s.sync.sync();
    expect(s.outcomes(view)).toEqual(["PC:ok", "NAS:unpaired"]);
    expect(s.nas.calls).toBe(before);
    for (const line of s.logs) expect(line).not.toContain(TOKEN);
    expect(s.logs.filter((l) => l.startsWith("picks:"))).toEqual([
      "picks: shared with 2 of 2 server(s)",
      "picks: shared with 1 of 2 server(s)",
    ]);
  });

  it("runs one exchange at a time, and once more for a pick made meanwhile", async () => {
    const s = await setup();
    const release = s.holdPc();
    const first = s.sync.sync();
    // Picked while the PC has not answered yet: asked for again, not started twice.
    const late = pick("weekly-gt3-sprint", NOW + 3 * H);
    await s.book.put(late);
    const second = s.sync.sync();
    expect(second).toBe(first);
    expect(s.pc.calls).toBe(0);
    release();
    await first;
    expect(s.pc.ids()).toEqual([late.id]);
    expect(s.pc.calls).toBe(2);
    expect(await s.book.list()).toEqual([late]);
  });

  it("does nothing without a server", async () => {
    const prefs = new FakePrefs();
    const servers = new ServerBook({
      prefs,
      fetch: async () => {
        throw new Error("no request expected");
      },
    });
    await servers.load();
    const book = new RaceBook(prefs, () => NOW);
    await book.put(pick("weekly-gt3-sprint", NOW + 3 * H));
    const sync = new PicksSync({ servers, book, onMerged: () => {}, now: () => NOW });
    expect(await sync.sync()).toEqual({ at: NOW, servers: [] });
  });
});

describe("the copy of the picks answered by a server", () => {
  it("is checked entry by entry: what does not pass is left out", () => {
    const good = pick("one", NOW + H);
    const set = pickSetFromWire({
      picks: [
        { pick: good, changed_ms: 12 },
        { pick: pick("two", NOW + H), changed_ms: -1 },
        { pick: { id: "x@1" }, changed_ms: 1 },
        { pick: { ...pick("three", NOW + H), extra: 1 }, changed_ms: 1 },
        "text",
        null,
      ],
      removed: [
        { id: `four@${NOW + H}`, removed_ms: 3 },
        { id: "../x@1", removed_ms: 3 },
        { id: `five@${NOW + H}`, removed_ms: 5_000_000_000_000 },
        { id: 7, removed_ms: 1 },
      ],
      later: "ignored",
    });
    expect(set).toEqual({
      picks: [{ pick: good, changed_ms: 12 }],
      removed: [{ id: `four@${NOW + H}`, removed_ms: 3 }],
    });
    expect(pickSetFromWire({})).toEqual({ picks: [], removed: [] });
    for (const bad of [null, [], "text", 3]) expect(() => pickSetFromWire(bad)).toThrow();
    expect(validPickId("one-stint-sprint-advanced@1762892700000")).toBe(true);
    for (const bad of ["one", "one@x", "@1", "One@1", "one@123456789012345", 5])
      expect(validPickId(bad)).toBe(false);
  });
});
