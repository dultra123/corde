import { type Route, routeHash } from "../nav/routes";

/**
 * `corde://<page>` opens the application on a page (the CI's smoke test uses it to take a
 * picture of the Courses page in the simulator: `xcrun simctl openurl booted corde://races`).
 * A closed list of pages without parameters: a link (from a web page, another application)
 * can only choose a page to show, never act.
 */
const PAGES: Record<string, Route> = {
  home: { page: "home" },
  races: { page: "races" },
  servers: { page: "servers" },
  live: { page: "dashboard" },
  sessions: { page: "sessions" },
  profile: { page: "profile" },
};

/** The hash to open for `corde://races` (or `corde:races`, `corde:///races/`); `null` otherwise. */
export function hashOfAppLink(url: string): string | null {
  const m = /^corde:(?:\/\/)?\/?([a-z]+)\/?$/i.exec(url.trim());
  const page = m?.[1]?.toLowerCase();
  const route = page ? PAGES[page] : undefined;
  return route ? routeHash(route) : null;
}
