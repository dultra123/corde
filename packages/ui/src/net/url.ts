import { DEFAULT_PORT, WS_PATH } from "@corde/protocol";
import { parseServerAddress } from "../mobile/addresses";

export interface ServerEndpoint {
  /** `ws://host:port/ws` */
  ws: string;
  /** `http://host:port` — base for `/api/...` */
  http: string;
  /** What was typed / detected, for display. */
  label: string;
  /** Where the address came from. */
  origin: "query" | "setting" | "page" | "dev";
}

/** Normalises what a user may type: `192.168.1.20`, `pc-jeu:8471`, `http://pc:8471/`, `ws://pc:8471/ws`. */
export function normaliseHost(input: string): string | null {
  const raw = input.trim();
  if (!raw) return null;
  const stripped = raw
    .replace(/^(wss?|https?):\/\//i, "")
    .replace(/\/.*$/, "")
    .trim();
  if (!stripped) return null;
  // IPv6 literal without brackets is ambiguous; accept `[::1]:8471` as is.
  if (stripped.includes(":") && !stripped.startsWith("[")) {
    const parts = stripped.split(":");
    if (parts.length === 2 && /^\d+$/.test(parts[1] ?? "")) return stripped;
    // more than one colon and no brackets: treat as bare IPv6
    return `[${stripped}]:${DEFAULT_PORT}`;
  }
  if (/:\d+$/.test(stripped)) return stripped;
  return `${stripped}:${DEFAULT_PORT}`;
}

export function endpointFromHost(
  host: string,
  origin: ServerEndpoint["origin"],
  secure = false,
): ServerEndpoint {
  const wsProto = secure ? "wss" : "ws";
  const httpProto = secure ? "https" : "http";
  return {
    ws: `${wsProto}://${host}${WS_PATH}`,
    http: `${httpProto}://${host}`,
    label: host,
    origin,
  };
}

/**
 * `?server=` comes with the address of the page: anybody can write it in a link. It is only
 * followed toward an address of the home network (a private or loopback address, a `.local`
 * name, a name without a dot — the rule of the application, `mobile/addresses.ts`): a link
 * received from somebody can never make this page talk to their server on the Internet.
 * The address the driver types himself in the settings is not restricted.
 */
export function hostFromQuery(value: string | null): string | null {
  if (!value) return null;
  const host = normaliseHost(value);
  if (!host) return null;
  const parsed = parseServerAddress(host);
  return parsed.ok && !parsed.address.secure ? host : null;
}

/**
 * Resolution order:
 * 1. `?server=host:port` in the URL (phone typing the PC's IP), home network only,
 * 2. the address saved in settings,
 * 3. the page's own host when served by corde-server (production build),
 * 4. `localhost:8471` in `vite dev` (also reachable through the dev proxy).
 */
export function resolveEndpoint(
  settingServer: string,
  loc: Pick<Location, "host" | "protocol" | "search"> = window.location,
): ServerEndpoint {
  const params = new URLSearchParams(loc.search);
  const fromQuery = hostFromQuery(params.get("server"));
  if (fromQuery) return endpointFromHost(fromQuery, "query");
  const fromSetting = normaliseHost(settingServer);
  if (fromSetting) return endpointFromHost(fromSetting, "setting");
  const secure = loc.protocol === "https:";
  if (loc.host && !import.meta.env.DEV) return endpointFromHost(loc.host, "page", secure);
  if (import.meta.env.DEV && loc.host) {
    // dev server proxies /ws and /api to 127.0.0.1:8471
    return endpointFromHost(loc.host, "dev", secure);
  }
  return endpointFromHost(`localhost:${DEFAULT_PORT}`, "dev");
}

export function isDemoForced(search: string = window.location.search): boolean {
  const v = new URLSearchParams(search).get("demo");
  return v === "1" || v === "true";
}
