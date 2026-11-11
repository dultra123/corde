import { describe, expect, it } from "vitest";
import { isEmptyHash, parseRoute, type Route, routeHash, sameRoute, sectionOf } from "./routes";

describe("parseRoute", () => {
  it("opens on the home page and keeps #/ for the live dashboard", () => {
    expect(parseRoute("")).toEqual({ page: "home" });
    expect(parseRoute("#")).toEqual({ page: "home" });
    expect(parseRoute("#/home")).toEqual({ page: "home" });
    expect(parseRoute("#/home/")).toEqual({ page: "home" });
    expect(parseRoute("#/")).toEqual({ page: "dashboard" });
    expect(parseRoute("/")).toEqual({ page: "dashboard" });
    expect(parseRoute("#/nope/whatever")).toEqual({ page: "home" });
  });

  it("tells an app that was just opened from an explicit route", () => {
    expect(isEmptyHash("")).toBe(true);
    expect(isEmptyHash("#")).toBe(true);
    expect(isEmptyHash("#/")).toBe(false);
    expect(isEmptyHash("#/home")).toBe(false);
  });

  it("parses the sessions list and one session", () => {
    expect(parseRoute("#/sessions")).toEqual({ page: "sessions" });
    expect(parseRoute("#/sessions/")).toEqual({ page: "sessions" });
    expect(parseRoute("#/sessions/20261104-063959-paul-ricard-1a-race")).toEqual({
      page: "session",
      id: "20261104-063959-paul-ricard-1a-race",
    });
    expect(parseRoute("/sessions/abc")).toEqual({ page: "session", id: "abc" });
  });

  it("parses a lap and falls back to the session on a bad lap number", () => {
    expect(parseRoute("#/sessions/abc/laps/4")).toEqual({ page: "lap", id: "abc", lap: 4 });
    expect(parseRoute("#/sessions/abc/laps/x")).toEqual({ page: "session", id: "abc" });
    expect(parseRoute("#/sessions/abc/laps")).toEqual({ page: "session", id: "abc" });
  });

  it("parses a comparison and degrades gracefully", () => {
    expect(parseRoute("#/sessions/abc/compare?a=3&b=5")).toEqual({
      page: "compare",
      id: "abc",
      a: 3,
      b: 5,
    });
    expect(parseRoute("#/sessions/abc/compare?b=5&a=3")).toEqual({
      page: "compare",
      id: "abc",
      a: 3,
      b: 5,
    });
    // same lap twice → single lap
    expect(parseRoute("#/sessions/abc/compare?a=3&b=3")).toEqual({
      page: "lap",
      id: "abc",
      lap: 3,
    });
    expect(parseRoute("#/sessions/abc/compare?a=3")).toEqual({ page: "lap", id: "abc", lap: 3 });
    expect(parseRoute("#/sessions/abc/compare")).toEqual({ page: "session", id: "abc" });
  });

  it("parses the engineer page and falls back to the session on extra segments", () => {
    expect(parseRoute("#/sessions/abc/engineer")).toEqual({ page: "engineer", id: "abc" });
    expect(parseRoute("#/sessions/abc/engineer/")).toEqual({ page: "engineer", id: "abc" });
    expect(parseRoute("#/sessions/abc/engineer/x")).toEqual({ page: "session", id: "abc" });
    expect(parseRoute("#/sessions/engineer")).toEqual({ page: "session", id: "engineer" });
  });

  it("rejects ids that could escape the API path", () => {
    expect(parseRoute("#/sessions/..%2Fetc")).toEqual({ page: "sessions" });
    expect(parseRoute("#/sessions/%20")).toEqual({ page: "sessions" });
  });

  it("parses the online races page", () => {
    expect(parseRoute("#/races")).toEqual({ page: "races" });
    expect(parseRoute("#/races/")).toEqual({ page: "races" });
    expect(routeHash({ page: "races" })).toBe("#/races");
    expect(sectionOf({ page: "races" })).toBe("races");
  });

  it("parses the profile and its track filter", () => {
    expect(parseRoute("#/profile")).toEqual({ page: "profile" });
    expect(parseRoute("#/profile/")).toEqual({ page: "profile" });
    expect(parseRoute("#/profile?track=Paul%20Ricard%20-%201A")).toEqual({
      page: "profile",
      track: "Paul Ricard - 1A",
    });
    expect(parseRoute("#/profile?track=%20")).toEqual({ page: "profile" });
    expect(parseRoute("#/profile/whatever")).toEqual({ page: "profile" });
  });

  it("parses one result and falls back to the profile on a bad id", () => {
    expect(parseRoute("#/results/12")).toEqual({ page: "result", id: 12 });
    expect(parseRoute("#/results/0")).toEqual({ page: "profile" });
    expect(parseRoute("#/results/x")).toEqual({ page: "profile" });
    expect(parseRoute("#/results")).toEqual({ page: "profile" });
    expect(parseRoute("#/results/12/laps")).toEqual({ page: "profile" });
  });

  it("round-trips through routeHash", () => {
    const routes: Route[] = [
      { page: "home" },
      { page: "dashboard" },
      { page: "sessions" },
      { page: "session", id: "20261104-063959-paul-ricard-1a-race" },
      { page: "lap", id: "abc", lap: 12 },
      { page: "compare", id: "abc", a: 2, b: 9 },
      { page: "engineer", id: "20261104-063959-paul-ricard-1a-race" },
      { page: "profile" },
      { page: "profile", track: "Silverstone International Circuit" },
      { page: "result", id: 7 },
    ];
    for (const r of routes) expect(parseRoute(routeHash(r))).toEqual(r);
  });

  it("compares routes by the page they show", () => {
    expect(sameRoute({ page: "home" }, { page: "home" })).toBe(true);
    expect(sameRoute({ page: "home" }, { page: "dashboard" })).toBe(false);
    expect(sameRoute({ page: "session", id: "a" }, { page: "session", id: "a" })).toBe(true);
    expect(sameRoute({ page: "session", id: "a" }, { page: "session", id: "b" })).toBe(false);
    expect(sameRoute({ page: "session", id: "a" }, { page: "engineer", id: "a" })).toBe(false);
  });

  it("maps routes to the four top-level tabs", () => {
    expect(sectionOf({ page: "home" })).toBe("home");
    expect(sectionOf({ page: "dashboard" })).toBe("dashboard");
    expect(sectionOf({ page: "sessions" })).toBe("sessions");
    expect(sectionOf({ page: "lap", id: "a", lap: 1 })).toBe("sessions");
    expect(sectionOf({ page: "engineer", id: "a" })).toBe("sessions");
    expect(sectionOf({ page: "profile" })).toBe("profile");
    expect(sectionOf({ page: "result", id: 3 })).toBe("profile");
  });
});

describe("the OBS page", () => {
  it("is #/obs, under the dashboard tab", () => {
    expect(parseRoute("#/obs")).toEqual({ page: "obs" });
    expect(routeHash({ page: "obs" })).toBe("#/obs");
    expect(sectionOf({ page: "obs" })).toBe("dashboard");
  });
});
