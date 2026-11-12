import { describe, expect, it, vi } from "vitest";
import { FakeHttp, page } from "./doubles";
import {
  MAX_BYTES,
  needsFetch,
  PHONE_SOURCES,
  PhoneSchedule,
  resolveLocation,
  sameSite,
} from "./schedule";

const UA = "Corde/0.8.0 (+https://github.com/dultra123/corde)";
const MIN = 60_000;

function setup(handler = (url: string) => page(`<html>${url}</html>`)) {
  let now = 1_000_000_000_000;
  const http = new FakeHttp((o) => handler(o.url));
  const cleared: string[] = [];
  const schedule = new PhoneSchedule({
    http,
    cookies: { clearCookies: async ({ url }) => void cleared.push(url ?? "") },
    userAgent: UA,
    now: () => now,
    timeoutMs: 50,
  });
  return { http, schedule, cleared, advance: (ms: number) => (now += ms), time: () => now };
}

describe("the schedule read by the phone", () => {
  it("reads only the two pages of the closed list, with the server's headers", async () => {
    const { http, schedule, cleared } = setup();
    const snap = await schedule.snapshot(true);
    expect(http.calls.map((c) => c.url).sort()).toEqual(
      ["https://lmu.tools/schedule", "https://racecontrol.gg/"].sort(),
    );
    for (const c of http.calls) {
      expect(c.headers).toEqual({
        "User-Agent": UA,
        Accept: "text/html",
        "Accept-Language": "en-GB,en;q=0.8",
      });
      expect(c.disableRedirects).toBe(true);
      expect(c.responseType).toBe("text");
      expect(c.method).toBeUndefined();
      expect(c.data).toBeUndefined();
    }
    expect(snap.enabled).toBe(true);
    expect(snap.sources.map((s) => [s.id, s.state])).toEqual([
      ["racecontrol", "ok"],
      ["lmutools", "ok"],
    ]);
    expect(snap.sources[0]?.html).toBe("<html>https://racecontrol.gg/</html>");
    // No cookie kept from either site.
    expect(cleared.sort()).toEqual(PHONE_SOURCES.map((s) => s.url).sort());
  });

  it("reads nothing while the schedule is off", async () => {
    const { http, schedule } = setup();
    const snap = await schedule.snapshot(false);
    expect(http.calls).toHaveLength(0);
    expect(snap.enabled).toBe(false);
    expect(snap.sources.every((s) => s.state === "disabled" && s.html === null)).toBe(true);
  });

  it("keeps each page 10 min / 60 min, and a forced reload waits a minute", async () => {
    const { http, schedule, advance } = setup();
    await schedule.snapshot(true);
    expect(http.calls).toHaveLength(2);
    advance(9 * MIN);
    await schedule.snapshot(true);
    expect(http.calls).toHaveLength(2);
    advance(MIN);
    await schedule.snapshot(true);
    expect(http.calls.map((c) => c.url).slice(2)).toEqual(["https://racecontrol.gg/"]);
    // « Actualiser » right after: racecontrol.gg was read less than a minute ago, lmu.tools
    // 10 min ago — only lmu.tools is read again.
    await schedule.snapshot(true, true);
    expect(http.calls.map((c) => c.url).slice(3)).toEqual(["https://lmu.tools/schedule"]);
    await schedule.snapshot(true, true);
    expect(http.calls).toHaveLength(4);
    advance(MIN);
    await schedule.snapshot(true, true);
    expect(http.calls).toHaveLength(6);
    advance(50 * MIN);
    await schedule.snapshot(true);
    // lmu.tools: 60 min after its last reading only.
    expect(http.calls.slice(6).map((c) => c.url)).toEqual(["https://racecontrol.gg/"]);
    advance(10 * MIN);
    await schedule.snapshot(true);
    expect(
      http.calls
        .slice(7)
        .map((c) => c.url)
        .sort(),
    ).toEqual(["https://lmu.tools/schedule", "https://racecontrol.gg/"].sort());
  });

  it("keeps the last good copy after a failure and waits a minute to try again", async () => {
    let fail = false;
    const { http, schedule, advance } = setup((url) => {
      if (fail) throw new Error("offline");
      return page(`<p>${url}</p>`);
    });
    await schedule.snapshot(true);
    fail = true;
    advance(10 * MIN);
    const snap = await schedule.snapshot(true);
    const rc = snap.sources[0];
    expect(rc?.state).toBe("error");
    expect(rc?.error).toBeTruthy();
    expect(rc?.html).toBe("<p>https://racecontrol.gg/</p>");
    const calls = http.calls.length;
    advance(30_000);
    await schedule.snapshot(true);
    expect(http.calls.length).toBe(calls);
    advance(30_000);
    fail = false;
    const again = await schedule.snapshot(true);
    expect(again.sources[0]?.state).toBe("ok");
    expect(http.calls.length).toBe(calls + 1);
  });

  it("shares a reading on its way", async () => {
    const { http, schedule } = setup();
    await Promise.all([schedule.snapshot(true), schedule.snapshot(true), schedule.snapshot(true)]);
    expect(http.calls).toHaveLength(2);
  });

  it("gives up after 20 s (here: 50 ms)", async () => {
    vi.useFakeTimers();
    try {
      const { schedule } = setup(() => new Promise(() => {}) as never);
      const p = schedule.snapshot(true);
      await vi.advanceTimersByTimeAsync(60);
      const snap = await p;
      expect(snap.sources.every((s) => s.state === "error")).toBe(true);
      expect(snap.sources[0]?.error).toMatch(/20 s/);
    } finally {
      vi.useRealTimers();
    }
  });

  it("refuses pages too large, other statuses and non-text answers", async () => {
    const big = "x".repeat(MAX_BYTES + 1);
    const answers: Record<string, ReturnType<typeof page>> = {
      "https://racecontrol.gg/": page(big),
      "https://lmu.tools/schedule": page({ json: true }),
    };
    const { schedule } = setup((url) => answers[url] ?? page("", 500));
    const snap = await schedule.snapshot(true);
    expect(snap.sources.map((s) => s.state)).toEqual(["error", "error"]);
    expect(snap.sources[0]?.error).toMatch(/3 Mi/);

    const { schedule: s2 } = setup(() => page("", 503));
    expect((await s2.snapshot(true)).sources[0]?.error).toBe("HTTP 503");
  });

  it("follows at most three redirections, https and on the same site only", async () => {
    const hops: Record<string, ReturnType<typeof page>> = {
      "https://racecontrol.gg/": page("", 301, { Location: "https://www.racecontrol.gg/" }),
      "https://www.racecontrol.gg/": page("", 302, { location: "/en" }),
      "https://www.racecontrol.gg/en": page("<ok/>"),
      "https://lmu.tools/schedule": page("", 302, { location: "https://evil.example/x" }),
    };
    const { schedule, http } = setup((url) => hops[url] ?? page("", 404));
    const snap = await schedule.snapshot(true);
    expect(snap.sources[0]?.html).toBe("<ok/>");
    expect(snap.sources[1]?.state).toBe("error");
    expect(http.calls.map((c) => c.url)).not.toContain("https://evil.example/x");

    let n = 0;
    const { schedule: loop } = setup(() => {
      n += 1;
      return page("", 302, { location: `/again${n}` });
    });
    const s = await loop.snapshot(true);
    expect(s.sources[0]?.error).toMatch(/redirections/);
    expect(n).toBeLessThanOrEqual(8);
  });

  it("has the server's rules for caches and sites", () => {
    expect(needsFetch(null, false, 10 * MIN, false)).toBe(true);
    expect(needsFetch(59_999, false, 10 * MIN, false)).toBe(false);
    expect(needsFetch(MIN, false, 10 * MIN, false)).toBe(true);
    expect(needsFetch(5 * MIN, true, 10 * MIN, false)).toBe(false);
    expect(needsFetch(5 * MIN, true, 10 * MIN, true)).toBe(true);
    expect(needsFetch(30_000, true, 10 * MIN, true)).toBe(false);
    expect(sameSite("https://www.racecontrol.gg/x", "racecontrol.gg")).toBe(true);
    expect(sameSite("https://racecontrol.gg:443/", "racecontrol.gg")).toBe(true);
    expect(sameSite("http://racecontrol.gg/", "racecontrol.gg")).toBe(false);
    expect(sameSite("https://racecontrol.gg:8443/", "racecontrol.gg")).toBe(false);
    expect(sameSite("https://racecontrol.gg.evil.example/", "racecontrol.gg")).toBe(false);
    expect(sameSite("https://racecontrol.gg@evil.example/", "racecontrol.gg")).toBe(false);
    expect(resolveLocation("https://lmu.tools/a", "/b")).toBe("https://lmu.tools/b");
    expect(resolveLocation("https://lmu.tools/a", "//lmu.tools/c")).toBe("https://lmu.tools/c");
    expect(resolveLocation("https://lmu.tools/a", "b")).toBeNull();
  });
});
