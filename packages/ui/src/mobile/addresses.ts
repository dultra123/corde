import { DEFAULT_PORT } from "@corde/protocol";

/**
 * Addresses of Corde servers typed (or scanned) in the application.
 *
 * Corde speaks plain HTTP on the home network (docs/SECURITY-MODEL.md), so `http://` is
 * accepted **only toward a private or local address**: an IPv4 of 10/8, 172.16/12,
 * 192.168/16, the loopback 127/8 or the link-local 169.254/16; an IPv6 loopback, unique local
 * (fc00::/7) or link-local (fe80::/10) address; `localhost`, a `.local` name, or a name without
 * a dot (`pc-jeu`, resolved by the home network). Anything else needs `https://` (a server
 * the driver put behind TLS himself). A VPN address of the home server (`10.8.0.1`, see
 * docs/ACCES-EXTERIEUR.md) is a private address. The carrier-grade range 100.64/10 (used by
 * some mesh VPNs) is not accepted over HTTP: it is not a private range.
 */
export interface ServerAddress {
  /** `http://192.168.1.20:8471` — no trailing slash; the API is `${base}/api/…`. */
  base: string;
  /** `192.168.1.20:8471`, for display. */
  label: string;
  secure: boolean;
}

export type AddressError =
  /** Nothing typed. */
  | "empty"
  /** Not an address. */
  | "invalid"
  /** Another scheme than http / https. */
  | "scheme"
  /** `http://` toward a public address or name. */
  | "public_http"
  /** `user:password@`. */
  | "credentials";

export type AddressResult =
  | { ok: true; address: ServerAddress }
  | { ok: false; error: AddressError };

const NAME_LABEL = /^[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?$/;

function ipv4(host: string): number[] | null {
  const m = /^(\d{1,3})\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})$/.exec(host);
  if (!m) return null;
  const parts = m.slice(1).map(Number);
  return parts.every((n) => n <= 255) ? parts : null;
}

/** `true` for an IPv4 of a private, loopback or link-local range. */
export function isPrivateIpv4(host: string): boolean {
  const p = ipv4(host);
  if (!p) return false;
  const [a = 0, b = 0] = p;
  return (
    a === 10 ||
    (a === 172 && b >= 16 && b <= 31) ||
    (a === 192 && b === 168) ||
    a === 127 ||
    (a === 169 && b === 254)
  );
}

/** Expands an IPv6 literal (no brackets) into 8 groups; `null` when malformed. */
function ipv6Groups(host: string): number[] | null {
  if (!/^[0-9a-f:.]+$/i.test(host) || !host.includes(":")) return null;
  let text = host.toLowerCase();
  // An embedded IPv4 at the end (`::ffff:192.168.1.2`).
  const v4 = /(\d{1,3}(?:\.\d{1,3}){3})$/.exec(text);
  if (v4?.[1]) {
    const p = ipv4(v4[1]);
    if (!p) return null;
    const [a = 0, b = 0, c = 0, d = 0] = p;
    text = `${text.slice(0, -v4[1].length)}${((a << 8) | b).toString(16)}:${((c << 8) | d).toString(16)}`;
  }
  const halves = text.split("::");
  if (halves.length > 2) return null;
  const head = halves[0] ? halves[0].split(":") : [];
  const tail = halves.length === 2 && halves[1] ? halves[1].split(":") : [];
  const missing = 8 - head.length - tail.length;
  if (halves.length === 1 ? missing !== 0 : missing < 1) return null;
  const all = [...head, ...Array<string>(halves.length === 2 ? missing : 0).fill("0"), ...tail];
  if (all.length !== 8 || all.some((g) => !/^[0-9a-f]{1,4}$/.test(g))) return null;
  return all.map((g) => Number.parseInt(g, 16));
}

/** `true` for an IPv6 loopback, unique local (fc00::/7) or link-local (fe80::/10) address. */
export function isPrivateIpv6(host: string): boolean {
  const g = ipv6Groups(host);
  if (!g) return false;
  const first = g[0] ?? 0;
  const loopback = g.slice(0, 7).every((x) => x === 0) && g[7] === 1;
  // IPv4-mapped (`::ffff:a.b.c.d`).
  const mapped = g.slice(0, 5).every((x) => x === 0) && g[5] === 0xffff;
  if (mapped) {
    const v4 = `${(g[6] ?? 0) >> 8}.${(g[6] ?? 0) & 255}.${(g[7] ?? 0) >> 8}.${(g[7] ?? 0) & 255}`;
    return isPrivateIpv4(v4);
  }
  return loopback || (first & 0xfe00) === 0xfc00 || (first & 0xffc0) === 0xfe80;
}

/** A host name of the home network: `localhost`, `<name>.local`, or a name without a dot. */
export function isLocalName(host: string): boolean {
  if (host === "localhost") return true;
  const labels = host.split(".");
  if (!labels.every((l) => NAME_LABEL.test(l))) return false;
  return labels.length === 1 || (labels.length === 2 && labels[1] === "local");
}

function validName(host: string): boolean {
  const labels = host.split(".");
  return host.length <= 253 && labels.every((l) => NAME_LABEL.test(l));
}

/**
 * Reads what the driver typed or scanned: `192.168.1.20`, `pc-jeu:8471`,
 * `http://192.168.1.20:8471/`, `https://corde.example.org`. Without a scheme, `http://`.
 * Without a port, 8471 for `http://` (Corde's port) and 443 for `https://`. A path, query or
 * fragment is ignored (the PC's QR code may carry one).
 */
export function parseServerAddress(input: string): AddressResult {
  const raw = input.trim();
  if (!raw) return { ok: false, error: "empty" };
  const schemeMatch = /^([a-z][a-z0-9+.-]*):\/\//i.exec(raw);
  const scheme = schemeMatch?.[1]?.toLowerCase() ?? "http";
  if (scheme !== "http" && scheme !== "https") return { ok: false, error: "scheme" };
  const rest = schemeMatch ? raw.slice(schemeMatch[0].length) : raw;
  const authority = rest.split(/[/?#]/, 1)[0] ?? "";
  if (authority.includes("@")) return { ok: false, error: "credentials" };
  if (!authority || /\s/.test(authority)) return { ok: false, error: "invalid" };

  let host: string;
  let portText: string | null = null;
  if (authority.startsWith("[")) {
    const end = authority.indexOf("]");
    if (end < 0) return { ok: false, error: "invalid" };
    host = authority.slice(1, end).toLowerCase();
    const after = authority.slice(end + 1);
    if (after) {
      if (!after.startsWith(":")) return { ok: false, error: "invalid" };
      portText = after.slice(1);
    }
    if (!ipv6Groups(host)) return { ok: false, error: "invalid" };
  } else {
    const colon = authority.lastIndexOf(":");
    if (colon >= 0) {
      host = authority.slice(0, colon).toLowerCase();
      portText = authority.slice(colon + 1);
    } else {
      host = authority.toLowerCase();
    }
    if (host.includes(":")) return { ok: false, error: "invalid" };
    host = host.replace(/\.$/, "");
    if (!ipv4(host) && !validName(host)) return { ok: false, error: "invalid" };
    // `192.168.1` and the like: digits only, but not an IPv4.
    if (/^[\d.]+$/.test(host) && !ipv4(host)) return { ok: false, error: "invalid" };
  }

  let port: number;
  if (portText === null || portText === "") {
    if (portText === "") return { ok: false, error: "invalid" };
    port = scheme === "https" ? 443 : DEFAULT_PORT;
  } else {
    if (!/^\d{1,5}$/.test(portText)) return { ok: false, error: "invalid" };
    port = Number(portText);
    if (port < 1 || port > 65535) return { ok: false, error: "invalid" };
  }

  const v6 = host.includes(":");
  const local = v6 ? isPrivateIpv6(host) : isPrivateIpv4(host) || isLocalName(host);
  if (scheme === "http" && !local) return { ok: false, error: "public_http" };

  const hostPart = v6 ? `[${host}]` : host;
  const showPort = !(scheme === "https" && port === 443);
  const label = showPort ? `${hostPart}:${port}` : hostPart;
  return {
    ok: true,
    address: { base: `${scheme}://${label}`, label, secure: scheme === "https" },
  };
}
