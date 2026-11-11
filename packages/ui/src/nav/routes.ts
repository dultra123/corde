import { useSyncExternalStore } from "react";

/**
 * Hash routes (no router library):
 *
 *   (no hash)                            home — the default page when the app opens
 *   #/home                               home (status, last session, last races, phone access)
 *   #/                                   live dashboard
 *   #/sessions                           recorded sessions
 *   #/sessions/<id>                      one session (laps)
 *   #/sessions/<id>/laps/<n>             one lap (charts)
 *   #/sessions/<id>/compare?a=3&b=5      two laps side by side
 *   #/sessions/<id>/engineer             the race engineer's report (analysis + setup plans + feedback)
 *   #/races                              online races: schedule, my races, reminders
 *   #/obs                                the widgets in OBS: addresses, sizes, options
 *   #/overlay/…                          a widget or a layout alone, for OBS (`overlay/model.ts`;
 *                                        not a page of this router: the app boots bare)
 *   #/profile                            driver profile (imported results)
 *   #/profile?track=<course>             …filtered on one track
 *   #/results/<id>                       one imported result (classification, player laps)
 *
 * The query string of the page (`?server=…`) is left untouched: only the hash changes, so a
 * phone that reached the UI with `?server=192.168.1.20:8471` keeps it while browsing.
 */
export type Route =
  | { page: "home" }
  | { page: "dashboard" }
  | { page: "sessions" }
  | { page: "session"; id: string }
  | { page: "lap"; id: string; lap: number }
  | { page: "compare"; id: string; a: number; b: number }
  | { page: "engineer"; id: string }
  | { page: "races" }
  | { page: "obs" }
  | { page: "profile"; track?: string }
  | { page: "result"; id: number };

export type Section = "home" | "dashboard" | "sessions" | "races" | "profile";

const SESSION_ID = /^[A-Za-z0-9][A-Za-z0-9._-]*$/;

function lapNumber(raw: string | null | undefined): number | null {
  if (raw === null || raw === undefined || raw === "") return null;
  if (!/^-?\d+$/.test(raw)) return null;
  const n = Number(raw);
  return Number.isSafeInteger(n) ? n : null;
}

/** `true` when the URL names no page at all (`""` or `"#"`): the app was just opened. */
export function isEmptyHash(hash: string): boolean {
  return hash === "" || hash === "#";
}

/**
 * Parses `location.hash` (with or without the leading `#`). No hash at all → home, `#/` → the
 * live dashboard, anything unknown → home.
 */
export function parseRoute(hash: string): Route {
  if (isEmptyHash(hash)) return { page: "home" };
  let h = hash.startsWith("#") ? hash.slice(1) : hash;
  if (!h.startsWith("/")) h = `/${h}`;
  const qIdx = h.indexOf("?");
  const path = qIdx >= 0 ? h.slice(0, qIdx) : h;
  const query = new URLSearchParams(qIdx >= 0 ? h.slice(qIdx + 1) : "");
  const parts = path
    .split("/")
    .filter((p) => p.length > 0)
    .map((p) => {
      try {
        return decodeURIComponent(p);
      } catch {
        return p;
      }
    });

  if (parts.length === 0) return { page: "dashboard" };
  if (parts[0] === "home") return { page: "home" };
  if (parts[0] === "races") return { page: "races" };
  if (parts[0] === "obs") return { page: "obs" };
  if (parts[0] === "profile") {
    const track = query.get("track")?.trim();
    return track ? { page: "profile", track } : { page: "profile" };
  }
  if (parts[0] === "results") {
    const id = lapNumber(parts[1]);
    return id !== null && id > 0 && parts.length === 2
      ? { page: "result", id }
      : { page: "profile" };
  }
  if (parts[0] !== "sessions") return { page: "home" };
  if (parts.length === 1) return { page: "sessions" };
  const id = parts[1] ?? "";
  if (!SESSION_ID.test(id)) return { page: "sessions" };
  if (parts.length === 2) return { page: "session", id };
  if (parts[2] === "laps" && parts.length === 4) {
    const lap = lapNumber(parts[3]);
    return lap === null ? { page: "session", id } : { page: "lap", id, lap };
  }
  if (parts[2] === "engineer" && parts.length === 3) return { page: "engineer", id };
  if (parts[2] === "compare" && parts.length === 3) {
    const a = lapNumber(query.get("a"));
    const b = lapNumber(query.get("b"));
    if (a !== null && b !== null && a !== b) return { page: "compare", id, a, b };
    if (a !== null) return { page: "lap", id, lap: a };
    return { page: "session", id };
  }
  return { page: "session", id };
}

/** Builds the `#/…` hash of a route (usable as an `href`). */
export function routeHash(route: Route): string {
  switch (route.page) {
    case "home":
      return "#/home";
    case "dashboard":
      return "#/";
    case "sessions":
      return "#/sessions";
    case "session":
      return `#/sessions/${encodeURIComponent(route.id)}`;
    case "lap":
      return `#/sessions/${encodeURIComponent(route.id)}/laps/${route.lap}`;
    case "compare":
      return `#/sessions/${encodeURIComponent(route.id)}/compare?a=${route.a}&b=${route.b}`;
    case "engineer":
      return `#/sessions/${encodeURIComponent(route.id)}/engineer`;
    case "races":
      return "#/races";
    case "obs":
      return "#/obs";
    case "profile":
      return route.track ? `#/profile?track=${encodeURIComponent(route.track)}` : "#/profile";
    case "result":
      return `#/results/${route.id}`;
  }
}

export function navigate(route: Route): void {
  const hash = routeHash(route);
  if (window.location.hash === hash) return;
  window.location.hash = hash;
}

/** `true` when both routes show the same page (same hash). */
export function sameRoute(a: Route, b: Route): boolean {
  return routeHash(a) === routeHash(b);
}

function subscribe(cb: () => void): () => void {
  window.addEventListener("hashchange", cb);
  return () => window.removeEventListener("hashchange", cb);
}

const getHash = () => (typeof window === "undefined" ? "" : window.location.hash);

/** The current route; re-renders on `hashchange`. */
export function useRoute(): Route {
  const hash = useSyncExternalStore(subscribe, getHash, () => "");
  return parseRoute(hash);
}

/** Which top-level tab a route belongs to. */
export function sectionOf(route: Route): Section {
  if (route.page === "home") return "home";
  if (route.page === "dashboard" || route.page === "obs") return "dashboard";
  if (route.page === "races") return "races";
  if (route.page === "profile" || route.page === "result") return "profile";
  return "sessions";
}
