import { type AddressError, parseServerAddress } from "./addresses";
import { KEYS, readJson, readText, writeJson } from "./kv";
import type { FetchPort, LogPort, PrefsPort } from "./ports";

/**
 * « Mes serveurs » — the Corde servers the iPhone application knows: the PC (live view at
 * home) and, later, the home server. Kept in the phone's storage; one of them is **active**
 * (the one the Direct, Sessions, Engineer and Profile pages read).
 *
 * The device token of a home server is kept apart from the list, under its own key, and only
 * this module reads it: it is never part of an entry, never shown, never written to a log or
 * to an error message (tested).
 */
export const MAX_SERVERS = 10;
const NAME_MAX = 40;
const PROBE_TIMEOUT_MS = 6_000;
const PAIR_TIMEOUT_MS = 15_000;

export interface ServerEntry {
  id: string;
  /** `http://192.168.1.20:8471` (no trailing slash). */
  base: string;
  /** `192.168.1.20:8471`. */
  label: string;
  /** Given by the driver, or the address. */
  name: string;
  added_ms: number;
  /** What the last successful test said; `null` before. */
  role: "desktop" | "hub" | null;
}

export type ProbeResult =
  /** A Corde answered (and, for a home server, accepted this phone). */
  | { kind: "ok"; role: "desktop" | "hub"; version: string }
  /** A home server that wants a pairing code (no token, or a revoked one). */
  | { kind: "unpaired" }
  /** The server refused this address or this application (403). */
  | { kind: "refused"; message: string }
  /** Something answered, but not a Corde server. */
  | { kind: "not_corde" }
  /** No answer: wrong address, server off, other network, local network not allowed… */
  | { kind: "unreachable" }
  /** Any other HTTP error. */
  | { kind: "error"; status: number; message: string };

export type PairResult =
  | { ok: true; deviceName: string; admin: boolean }
  | {
      ok: false;
      /** Server's code (`wrong_code`, `expired`, `rate_limited`…) or `unreachable` / `invalid`. */
      code: string;
      /** The server's French message, when it gave one. */
      message: string | null;
      retryAfterS: number | null;
    };

function randomId(): string {
  const b = new Uint8Array(8);
  crypto.getRandomValues(b);
  return Array.from(b, (x) => x.toString(16).padStart(2, "0")).join("");
}

const isEntry = (v: unknown): v is ServerEntry => {
  if (typeof v !== "object" || v === null) return false;
  const o = v as Record<string, unknown>;
  return (
    typeof o.id === "string" &&
    /^[0-9a-f]{8,32}$/.test(o.id) &&
    typeof o.base === "string" &&
    typeof o.name === "string" &&
    typeof o.added_ms === "number"
  );
};

function cleanName(name: string, fallback: string): string {
  // biome-ignore lint/suspicious/noControlCharactersInRegex: control characters are removed
  const n = name.replace(/[\u0000-\u001f\u007f]/g, "").trim();
  return ([...n].slice(0, NAME_MAX).join("") || fallback).trim();
}

/** A fetch with a deadline; network failures and timeouts become `null`. */
async function fetchWithin(
  fetchFn: FetchPort,
  url: string,
  init: RequestInit,
  ms: number,
): Promise<Response | null> {
  const ctrl = new AbortController();
  const timer = setTimeout(() => ctrl.abort(), ms);
  try {
    return await fetchFn(url, { ...init, signal: ctrl.signal, credentials: "omit" });
  } catch {
    return null;
  } finally {
    clearTimeout(timer);
  }
}

async function jsonOf(res: Response): Promise<Record<string, unknown> | null> {
  try {
    const v: unknown = await res.json();
    return typeof v === "object" && v !== null ? (v as Record<string, unknown>) : null;
  } catch {
    return null;
  }
}

export interface ServerBookOptions {
  prefs: PrefsPort;
  fetch: FetchPort;
  log?: LogPort;
  now?: () => number;
  newId?: () => string;
  /** The name sent when pairing with an empty one (`iPhone`, `Android`). */
  deviceName?: string;
}

export class ServerBook {
  private entries: ServerEntry[] = [];
  private activeId: string | null = null;
  /** Tokens read from storage, by server id (memory of this process only). */
  private readonly tokens = new Map<string, string>();
  private readonly prefs: PrefsPort;
  private readonly fetchFn: FetchPort;
  private readonly log: LogPort;
  private readonly now: () => number;
  private readonly newId: () => string;
  private readonly deviceName: string;

  constructor(opts: ServerBookOptions) {
    this.deviceName = opts.deviceName ?? "iPhone";
    this.prefs = opts.prefs;
    this.fetchFn = opts.fetch;
    this.log = opts.log ?? (() => {});
    this.now = opts.now ?? Date.now;
    this.newId = opts.newId ?? randomId;
  }

  async load(): Promise<void> {
    const raw = await readJson(this.prefs, KEYS.servers);
    this.entries = [];
    if (Array.isArray(raw)) {
      for (const e of raw) {
        if (!isEntry(e)) continue;
        const parsed = parseServerAddress(e.base);
        if (!parsed.ok || this.entries.some((x) => x.id === e.id)) continue;
        this.entries.push({
          id: e.id,
          base: parsed.address.base,
          label: parsed.address.label,
          name: cleanName(e.name, parsed.address.label),
          added_ms: e.added_ms,
          role: e.role === "desktop" || e.role === "hub" ? e.role : null,
        });
      }
    }
    this.entries = this.entries.slice(0, MAX_SERVERS);
    const active = await readText(this.prefs, KEYS.activeServer);
    this.activeId = this.entries.some((e) => e.id === active) ? active : null;
    this.tokens.clear();
    for (const e of this.entries) {
      const t = await readText(this.prefs, KEYS.token(e.id));
      if (t && /^[0-9a-f]{16,128}$/i.test(t)) this.tokens.set(e.id, t);
    }
  }

  list(): ServerEntry[] {
    return this.entries.map((e) => ({ ...e }));
  }

  get(id: string): ServerEntry | null {
    const e = this.entries.find((x) => x.id === id);
    return e ? { ...e } : null;
  }

  active(): ServerEntry | null {
    return this.activeId ? this.get(this.activeId) : null;
  }

  /** This phone holds a device token for that server (paired with a home server). */
  isPaired(id: string): boolean {
    return this.tokens.has(id);
  }

  /** The token of the active server, for `net/auth` only. */
  activeToken(): string | null {
    return this.activeId ? (this.tokens.get(this.activeId) ?? null) : null;
  }

  private async save(): Promise<void> {
    await writeJson(this.prefs, KEYS.servers, this.entries);
  }

  /** Adds a server; the first one becomes the active one. */
  async add(
    input: string,
    name = "",
  ): Promise<
    { ok: true; entry: ServerEntry } | { ok: false; error: AddressError | "duplicate" | "full" }
  > {
    const parsed = parseServerAddress(input);
    if (!parsed.ok) return { ok: false, error: parsed.error };
    const { base, label } = parsed.address;
    if (this.entries.some((e) => e.base === base)) return { ok: false, error: "duplicate" };
    if (this.entries.length >= MAX_SERVERS) return { ok: false, error: "full" };
    const entry: ServerEntry = {
      id: this.newId(),
      base,
      label,
      name: cleanName(name, label),
      added_ms: this.now(),
      role: null,
    };
    this.entries.push(entry);
    await this.save();
    if (!this.activeId) await this.setActive(entry.id);
    this.log(`servers: ${label} added`);
    return { ok: true, entry: { ...entry } };
  }

  async rename(id: string, name: string): Promise<void> {
    const e = this.entries.find((x) => x.id === id);
    if (!e) return;
    e.name = cleanName(name, e.label);
    await this.save();
  }

  /** Removes a server and its token. */
  async remove(id: string): Promise<void> {
    const e = this.entries.find((x) => x.id === id);
    if (!e) return;
    if (this.tokens.has(id)) await this.unpair(id);
    this.entries = this.entries.filter((x) => x.id !== id);
    await this.save();
    if (this.activeId === id) {
      await this.setActive(this.entries[0]?.id ?? null);
    }
    this.log(`servers: ${e.label} removed`);
  }

  async setActive(id: string | null): Promise<void> {
    this.activeId = id && this.entries.some((e) => e.id === id) ? id : null;
    if (this.activeId) await this.prefs.set({ key: KEYS.activeServer, value: this.activeId });
    else await this.prefs.remove({ key: KEYS.activeServer });
  }

  private async forgetToken(id: string): Promise<void> {
    this.tokens.delete(id);
    await this.prefs.remove({ key: KEYS.token(id) });
  }

  private headers(id: string): Record<string, string> {
    const t = this.tokens.get(id);
    return t
      ? { Accept: "application/json", Authorization: `Bearer ${t}` }
      : { Accept: "application/json" };
  }

  /**
   * Tests a server: `GET /api/health` (with the token when there is one). A home server that
   * refuses the token (revoked) makes this phone forget it.
   */
  async probe(id: string): Promise<ProbeResult> {
    const e = this.entries.find((x) => x.id === id);
    if (!e) return { kind: "unreachable" };
    const res = await fetchWithin(
      this.fetchFn,
      `${e.base}/api/health`,
      { headers: this.headers(id) },
      PROBE_TIMEOUT_MS,
    );
    let result: ProbeResult;
    if (!res) result = { kind: "unreachable" };
    else if (res.status === 401) {
      // Only a home server answers 401 (a PC has no pairing).
      if (this.tokens.has(id)) await this.forgetToken(id);
      if (e.role !== "hub") {
        e.role = "hub";
        await this.save();
      }
      result = { kind: "unpaired" };
    } else if (res.status === 403) {
      const body = await jsonOf(res);
      result = {
        kind: "refused",
        message: typeof body?.error === "string" ? body.error : "",
      };
    } else if (!res.ok) {
      const body = await jsonOf(res);
      result = {
        kind: "error",
        status: res.status,
        message: typeof body?.error === "string" ? body.error : "",
      };
    } else {
      const body = await jsonOf(res);
      if (body?.ok !== true || typeof body.version !== "string") {
        result = { kind: "not_corde" };
      } else {
        const role = body.role === "hub" ? "hub" : "desktop";
        result = { kind: "ok", role, version: body.version };
        if (e.role !== role) {
          e.role = role;
          await this.save();
        }
      }
    }
    this.log(`servers: test of ${e.label} → ${result.kind}`);
    return result;
  }

  /**
   * Pairs this phone with a home server, with a code read on the server
   * (`sudo corde-server pair`). The token received is stored, never returned.
   */
  async pair(id: string, code: string, deviceName: string): Promise<PairResult> {
    const e = this.entries.find((x) => x.id === id);
    if (!e) return { ok: false, code: "unknown_server", message: null, retryAfterS: null };
    const digits = code.replace(/\D/g, "");
    if (digits.length !== 8)
      return { ok: false, code: "invalid", message: null, retryAfterS: null };
    const res = await fetchWithin(
      this.fetchFn,
      `${e.base}/api/hub/pair`,
      {
        method: "POST",
        headers: { Accept: "application/json", "Content-Type": "application/json" },
        body: JSON.stringify({
          code: digits,
          name: cleanName(deviceName, this.deviceName),
          kind: "app",
        }),
      },
      PAIR_TIMEOUT_MS,
    );
    if (!res) return { ok: false, code: "unreachable", message: null, retryAfterS: null };
    const body = await jsonOf(res);
    if (res.status !== 201) {
      const retry = Number(res.headers.get("retry-after"));
      this.log(`servers: pairing with ${e.label} refused (${res.status})`);
      return {
        ok: false,
        code: typeof body?.code === "string" ? body.code : `http_${res.status}`,
        message: typeof body?.error === "string" ? body.error : null,
        retryAfterS: Number.isFinite(retry) && retry > 0 ? retry : null,
      };
    }
    const token = typeof body?.token === "string" ? body.token : "";
    if (!/^[0-9a-f]{16,128}$/i.test(token)) {
      return { ok: false, code: "no_token", message: null, retryAfterS: null };
    }
    this.tokens.set(id, token);
    await this.prefs.set({ key: KEYS.token(id), value: token });
    const device = (body?.device ?? {}) as Record<string, unknown>;
    e.role = "hub";
    await this.save();
    this.log(`servers: paired with ${e.label}`);
    return {
      ok: true,
      deviceName: typeof device.name === "string" ? device.name : deviceName,
      admin: device.admin === true,
    };
  }

  /**
   * Forgets the pairing on this phone, after asking the home server to revoke this device
   * (best effort: when the server does not answer, the device stays listed there until it is
   * revoked from the « Appareils » page or `sudo corde-server revoke`).
   */
  async unpair(id: string): Promise<{ revokedOnServer: boolean }> {
    const e = this.entries.find((x) => x.id === id);
    let revoked = false;
    if (e && this.tokens.has(id)) {
      const state = await fetchWithin(
        this.fetchFn,
        `${e.base}/api/hub/pairing`,
        { headers: this.headers(id) },
        PROBE_TIMEOUT_MS,
      );
      const device = state?.ok
        ? ((await jsonOf(state))?.device as Record<string, unknown> | null)
        : null;
      if (device && typeof device.id === "string" && /^[A-Za-z0-9_-]{1,64}$/.test(device.id)) {
        const res = await fetchWithin(
          this.fetchFn,
          `${e.base}/api/hub/devices/${encodeURIComponent(device.id)}`,
          { method: "DELETE", headers: this.headers(id) },
          PROBE_TIMEOUT_MS,
        );
        revoked = res?.status === 204;
      }
    }
    await this.forgetToken(id);
    if (e) this.log(`servers: pairing with ${e.label} forgotten (revoked: ${revoked})`);
    return { revokedOnServer: revoked };
  }
}
