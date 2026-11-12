import { describe, expect, it } from "vitest";
import { en } from "../i18n/en";
import { fr } from "../i18n/fr";
import {
  codeDigits,
  formatCode,
  guessDeviceName,
  type HubLinkState,
  type HubLinkStatus,
} from "../net/hub-api";
import { hubPageUrl, linkStateKey, linkTone, liveUrl, pendingCount } from "./lib";

const status = (patch: Partial<HubLinkStatus> = {}): HubLinkStatus => ({
  url: "http://192.168.1.50:8471",
  paired: true,
  device_name: "PC de jeu",
  state: "connected",
  message: null,
  last_sync_ms: 1,
  last_attempt_ms: 1,
  pending_sessions: 0,
  pending_results: 0,
  server_version: "0.7.0",
  last_report: null,
  ...patch,
});

describe("pairing code", () => {
  it("keeps the digits only, eight at most, and groups them by four", () => {
    expect(codeDigits("1234 5678")).toBe("12345678");
    expect(codeDigits("12-34-56-78-99")).toBe("12345678");
    expect(codeDigits("abc")).toBe("");
    expect(formatCode("123")).toBe("123");
    expect(formatCode("12345")).toBe("1234 5");
    expect(formatCode("12345678")).toBe("1234 5678");
  });
});

describe("device name", () => {
  it("is a product name, or nothing", () => {
    expect(guessDeviceName("Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X)")).toBe(
      "iPhone",
    );
    expect(guessDeviceName("Mozilla/5.0 (Linux; Android 15; Pixel) Mobile")).toBe("Android");
    expect(guessDeviceName("Mozilla/5.0 (Windows NT 10.0; Win64; x64)")).toBe("Windows");
    expect(guessDeviceName("curl/8")).toBeNull();
  });
});

describe("link state", () => {
  const states: HubLinkState[] = [
    "unpaired",
    "connected",
    "syncing",
    "unreachable",
    "server_outdated",
    "pc_outdated",
    "revoked",
    "error",
  ];
  it("has a label in both languages for every state", () => {
    for (const s of states) {
      const key = linkStateKey(s);
      expect(fr[key], s).toBeTruthy();
      expect(en[key], s).toBeTruthy();
    }
  });
  it("tones", () => {
    expect(linkTone("connected")).toBe("ok");
    expect(linkTone("unreachable")).toBe("warn");
    expect(linkTone("server_outdated")).toBe("bad");
    expect(linkTone("unpaired")).toBe("off");
  });
  it("counts what is pending", () => {
    expect(pendingCount(status({ pending_sessions: 2, pending_results: 1 }))).toBe(3);
    expect(pendingCount(status({ pending_sessions: null, pending_results: null }))).toBeNull();
  });
});

describe("links between the PC and the server", () => {
  it("points the server's « Direct » tab at the PC's live view when known", () => {
    expect(liveUrl(null)).toBeNull();
    const pc = {
      device_id: "a",
      device_name: "PC",
      version: "0.7.0",
      urls: [],
      synced_ms: 1,
    };
    expect(liveUrl(pc)).toBeNull();
    expect(liveUrl({ ...pc, urls: ["http://192.168.1.20:8471"] })).toBe(
      "http://192.168.1.20:8471/#/",
    );
    expect(liveUrl({ ...pc, urls: ["javascript:alert(1)"] })).toBeNull();
  });
  it("points the PC at the server only once paired", () => {
    expect(hubPageUrl(status(), "#/races")).toBe("http://192.168.1.50:8471/#/races");
    expect(hubPageUrl(status({ paired: false }), "#/races")).toBeNull();
    expect(hubPageUrl(null, "#/races")).toBeNull();
  });
});
