import type { TranslationKey } from "../i18n";
import type { HubLinkState, HubLinkStatus, LastPc } from "../net/hub-api";

/** Label of a link state in the PC's settings. */
export function linkStateKey(state: HubLinkState): TranslationKey {
  return `hub.link.state.${state}` as TranslationKey;
}

/** Colour of the state chip: fine, working, needs the driver, broken. */
export function linkTone(state: HubLinkState): "ok" | "busy" | "warn" | "bad" | "off" {
  switch (state) {
    case "connected":
      return "ok";
    case "syncing":
      return "busy";
    case "unreachable":
      return "warn";
    case "unpaired":
      return "off";
    default:
      return "bad";
  }
}

/** Sessions + results the server still lacks, `null` until a round has looked. */
export function pendingCount(s: HubLinkStatus): number | null {
  if (s.pending_sessions === null && s.pending_results === null) return null;
  return (s.pending_sessions ?? 0) + (s.pending_results ?? 0);
}

/**
 * Where to watch the live view: the address the last PC that synchronised gave (the home server
 * never serves the live view itself). `null` when that PC listens on itself only.
 */
export function liveUrl(pc: LastPc | null): string | null {
  const url = pc?.urls.find((u) => /^http:\/\/[^/?#@\s]+$/.test(u));
  return url ? `${url}/#/` : null;
}

/** The home server's address, for a link from the PC (`…/#/races`). */
export function hubPageUrl(link: HubLinkStatus | null, hash: string): string | null {
  if (!link?.paired || !link.url) return null;
  return `${link.url}/${hash}`;
}
