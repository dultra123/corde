/**
 * The device token of the iPhone application for the home server (« Corde serveur »).
 *
 * A browser holds its token in an `HttpOnly` cookie it never sees. The application's pages
 * come from `capacitor://localhost`, another origin than the server's, so that cookie would
 * not be sent: the application keeps the token in the phone's storage and sends it itself —
 * `Authorization: Bearer` on every API call, a `corde.token.<token>` subprotocol entry on the
 * WebSocket (a web page cannot put headers on a WebSocket; a token in the address would end up
 * in the server's request log). The server answers `corde.v1` only (docs/SECURITY-MODEL.md).
 *
 * In the browser no provider is installed: nothing changes.
 */
let provider: (() => string | null) | null = null;

/** The application sets where the token of the active server comes from (`null`: none). */
export function setTokenProvider(fn: (() => string | null) | null): void {
  provider = fn;
}

function token(): string | null {
  const t = provider?.() ?? null;
  return t && /^[0-9a-f]{16,128}$/i.test(t) ? t : null;
}

/** Headers to add to an API call. */
export function authHeaders(): Record<string, string> {
  const t = token();
  return t ? { Authorization: `Bearer ${t}` } : {};
}

/** The subprotocol the application asks for. */
export const WS_PROTOCOL = "corde.v1";

/** Subprotocols of the WebSocket: none in the browser, the token entry when there is one. */
export function wsProtocols(): string[] | undefined {
  const t = token();
  return t ? [WS_PROTOCOL, `corde.token.${t}`] : undefined;
}
