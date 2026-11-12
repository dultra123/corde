import type { HttpResponse } from "@capacitor/core";
import { t } from "../i18n";
import type { RaceSources, SourceId, SourceSnapshot } from "../net/races-api";
import type { CookiesPort, HttpPort, LogPort } from "./ports";

/**
 * The two public pages of the online race schedule, read **by the phone itself** — the iPhone
 * application needs no PC for the « Courses » page. Same rules as the server
 * (`crates/corde-server/src/races/sources.rs`), with Capacitor's native HTTP (`CapacitorHttp`,
 * `URLSession` on iOS: no CORS, unlike a web page):
 *
 * - a **closed list** of two addresses ({@link PHONE_SOURCES}): no address ever comes from
 *   elsewhere;
 * - a cache of 10 min (racecontrol.gg) and 60 min (lmu.tools); a forced reload at most every
 *   60 s; after a failure, no new attempt for 60 s (the last good copy stays shown);
 * - a 20 s deadline per page (redirections included), a 3 MiB limit;
 * - redirections followed by this module, at most 3, only to `https://` on the same site
 *   (native redirections are turned off: `disableRedirects`);
 * - `User-Agent: Corde/<version> (+https://github.com/dultra123/corde)`, `Accept: text/html`,
 *   `Accept-Language: en-GB,en;q=0.8`; the cookies a site may set are cleared after each page;
 * - nothing is read while the driver has not turned the schedule on.
 *
 * The page is kept in memory only, as text, and given to the parser (`races/parse.ts`) —
 * never to the DOM — exactly like the server's answer, so the Courses page and the home
 * carousel work unchanged on it.
 */
export interface PhoneSourceDef {
  id: SourceId;
  url: string;
  /** The site; redirections may only stay on it (or its `www.` form). */
  host: string;
  ttlMs: number;
}

export const PHONE_SOURCES: readonly PhoneSourceDef[] = [
  { id: "racecontrol", url: "https://racecontrol.gg/", host: "racecontrol.gg", ttlMs: 10 * 60_000 },
  { id: "lmutools", url: "https://lmu.tools/schedule", host: "lmu.tools", ttlMs: 60 * 60_000 },
];

/** A forced reload, or a new attempt after a failure, waits at least this long. */
export const MIN_RETRY_MS = 60_000;
/** Deadline of one page (redirections included). */
export const TIMEOUT_MS = 20_000;
/** Largest page accepted (bytes of UTF-8). */
export const MAX_BYTES = 3 * 1024 * 1024;
/** Redirections followed (same site only). */
export const MAX_REDIRECTS = 3;

/**
 * `true` when a source must be read now (port of the server's `needs_fetch`). `ageMs` = time
 * since the last attempt (`null`: never tried).
 */
export function needsFetch(
  ageMs: number | null,
  lastOk: boolean,
  ttlMs: number,
  force: boolean,
): boolean {
  if (ageMs === null) return true;
  if (!lastOk) return ageMs >= MIN_RETRY_MS;
  return ageMs >= ttlMs || (force && ageMs >= MIN_RETRY_MS);
}

/** `true` when `url` is `https://` on `host` (or `www.<host>`, or `host` without `www.`). */
export function sameSite(url: string, host: string): boolean {
  if (!url.startsWith("https://")) return false;
  const authority = url.slice("https://".length).split(/[/?#]/, 1)[0] ?? "";
  if (authority.includes("@")) return false;
  let name = authority.toLowerCase();
  const colon = name.lastIndexOf(":");
  if (colon >= 0) {
    if (name.slice(colon + 1) !== "443") return false;
    name = name.slice(0, colon);
  }
  const bare = host.replace(/^www\./, "");
  return name === bare || name === `www.${bare}`;
}

/** A `Location` header resolved against the page that sent it; `null` for anything else. */
export function resolveLocation(base: string, location: string): string | null {
  const loc = location.trim();
  if (loc.startsWith("https://") || loc.startsWith("http://")) return loc;
  if (loc.startsWith("//")) return `https:${loc}`;
  if (loc.startsWith("/")) {
    if (!base.startsWith("https://")) return null;
    const authority = base.slice("https://".length).split("/", 1)[0];
    return authority ? `https://${authority}${loc}` : null;
  }
  return null;
}

function header(res: HttpResponse, name: string): string | null {
  const h = res.headers ?? {};
  for (const [k, v] of Object.entries(h)) {
    if (k.toLowerCase() === name) return typeof v === "string" ? v : String(v);
  }
  return null;
}

function utf8Length(text: string): number {
  return new TextEncoder().encode(text).length;
}

class FetchError extends Error {}

interface Slot {
  html: string | null;
  fetchedAt: number | null;
  attemptedAt: number | null;
  lastOk: boolean;
  error: string | null;
  inFlight: Promise<void> | null;
}

export interface PhoneScheduleOptions {
  http: HttpPort;
  /** Clears what a site may have set (`CapacitorCookies.clearCookies`); optional in tests. */
  cookies?: CookiesPort | null;
  userAgent: string;
  now?: () => number;
  log?: LogPort;
  /** Deadline of one page, ms (tests shorten it). */
  timeoutMs?: number;
}

export class PhoneSchedule {
  private readonly slots = new Map<SourceId, Slot>();
  private readonly now: () => number;
  private readonly timeoutMs: number;

  constructor(private readonly opts: PhoneScheduleOptions) {
    this.now = opts.now ?? Date.now;
    this.timeoutMs = opts.timeoutMs ?? TIMEOUT_MS;
    for (const s of PHONE_SOURCES) {
      this.slots.set(s.id, {
        html: null,
        fetchedAt: null,
        attemptedAt: null,
        lastOk: false,
        error: null,
        inFlight: null,
      });
    }
  }

  /**
   * Both pages, each read first when its cache is stale (side by side). With `enabled` false,
   * nothing is read and no page is returned — the same answer as the server's.
   */
  async snapshot(enabled: boolean, force = false): Promise<RaceSources> {
    if (!enabled) {
      return {
        enabled: false,
        sources: PHONE_SOURCES.map((d) => ({
          id: d.id,
          url: d.url,
          state: "disabled",
          fetched_at_ms: null,
          attempted_at_ms: null,
          error: null,
          html: null,
        })),
      };
    }
    const sources = await Promise.all(PHONE_SOURCES.map((d) => this.one(d, force)));
    return { enabled: true, sources };
  }

  /** The copy held for `id` (diagnosis), `null` when none. */
  heldPage(id: SourceId): string | null {
    return this.slots.get(id)?.html ?? null;
  }

  private async one(def: PhoneSourceDef, force: boolean): Promise<SourceSnapshot> {
    const s = this.slots.get(def.id) as Slot;
    // Two pages opened at once wait for one reading.
    if (s.inFlight) await s.inFlight;
    else {
      const age = s.attemptedAt === null ? null : Math.max(0, this.now() - s.attemptedAt);
      if (needsFetch(age, s.lastOk, def.ttlMs, force)) {
        s.inFlight = this.read(def, s).finally(() => {
          s.inFlight = null;
        });
        await s.inFlight;
      }
    }
    return {
      id: def.id,
      url: def.url,
      state: s.lastOk ? "ok" : "error",
      fetched_at_ms: s.fetchedAt,
      attempted_at_ms: s.attemptedAt,
      error: s.error,
      html: s.html,
    };
  }

  private async read(def: PhoneSourceDef, s: Slot): Promise<void> {
    const started = this.now();
    try {
      const html = await this.withDeadline(this.fetchPage(def));
      s.html = html;
      s.fetchedAt = started;
      s.lastOk = true;
      s.error = null;
      this.opts.log?.(`races: ${def.url} read (${utf8Length(html)} bytes)`);
    } catch (e) {
      s.lastOk = false;
      s.error = e instanceof FetchError ? e.message : t("mobile.schedule.err.network");
      this.opts.log?.(`races: ${def.url} unreadable — ${s.error}`);
    } finally {
      s.attemptedAt = started;
    }
  }

  private withDeadline<T>(p: Promise<T>): Promise<T> {
    return new Promise<T>((resolve, reject) => {
      const timer = setTimeout(
        () => reject(new FetchError(t("mobile.schedule.err.timeout"))),
        this.timeoutMs,
      );
      p.then(
        (v) => {
          clearTimeout(timer);
          resolve(v);
        },
        (e: unknown) => {
          clearTimeout(timer);
          reject(e);
        },
      );
    });
  }

  /** `GET` the page, following at most {@link MAX_REDIRECTS} redirections on the same site. */
  async fetchPage(def: PhoneSourceDef): Promise<string> {
    let url = def.url;
    for (let hop = 0; ; hop += 1) {
      let res: HttpResponse;
      try {
        res = await this.opts.http.get({
          url,
          headers: {
            "User-Agent": this.opts.userAgent,
            Accept: "text/html",
            "Accept-Language": "en-GB,en;q=0.8",
          },
          responseType: "text",
          connectTimeout: this.timeoutMs,
          readTimeout: this.timeoutMs,
          disableRedirects: true,
        });
      } catch {
        throw new FetchError(t("mobile.schedule.err.network"));
      } finally {
        // A site may set cookies (they would be sent next time): none is kept.
        void this.opts.cookies?.clearCookies({ url }).catch(() => {});
      }
      if (res.status >= 300 && res.status < 400) {
        const next = resolveLocation(url, header(res, "location") ?? "");
        if (hop >= MAX_REDIRECTS) throw new FetchError(t("mobile.schedule.err.redirects"));
        if (!next || !sameSite(next, def.host)) {
          throw new FetchError(t("mobile.schedule.err.redirectAway"));
        }
        url = next;
        continue;
      }
      if (res.status !== 200) throw new FetchError(`HTTP ${res.status}`);
      if (typeof res.data !== "string") throw new FetchError(t("mobile.schedule.err.notText"));
      if (res.data.length > MAX_BYTES || utf8Length(res.data) > MAX_BYTES) {
        throw new FetchError(t("mobile.schedule.err.tooBig"));
      }
      return res.data;
    }
  }
}
