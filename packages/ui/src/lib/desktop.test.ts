import { afterEach, describe, expect, it, vi } from "vitest";
import {
  appVersion,
  checkUpdate,
  DESKTOP_COMMANDS,
  DesktopError,
  getAutostart,
  getUpdateAutoCheck,
  installUpdate,
  isDesktop,
  isLocalPage,
  onUpdateStatus,
  openReleasesPage,
  parseUpdateStatus,
  restartApp,
  setAutostart,
  setUpdateAutoCheck,
  UPDATE_EVENT,
  updateStatus,
} from "./desktop";

type Invoke = (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;

function shell(invoke: Invoke) {
  const spy = vi.fn(invoke);
  window.__TAURI__ = { core: { invoke: spy } };
  return spy;
}

afterEach(() => {
  delete window.__TAURI__;
});

describe("in a browser", () => {
  it("is not the desktop application", () => {
    expect(isDesktop()).toBe(false);
    window.__TAURI__ = {};
    expect(isDesktop()).toBe(false);
    window.__TAURI__ = { core: {} };
    expect(isDesktop()).toBe(false);
  });

  it("rejects every command with a DesktopError instead of throwing a TypeError", async () => {
    await expect(getAutostart()).rejects.toBeInstanceOf(DesktopError);
    await expect(setAutostart(true)).rejects.toBeInstanceOf(DesktopError);
    await expect(restartApp()).rejects.toBeInstanceOf(DesktopError);
    await expect(appVersion()).rejects.toBeInstanceOf(DesktopError);
  });
});

describe("in the desktop shell", () => {
  it("uses the command names and arguments of the contract", async () => {
    // Frozen on purpose: the shell is written against these strings.
    expect(DESKTOP_COMMANDS).toEqual({
      getAutostart: "get_autostart",
      setAutostart: "set_autostart",
      restartApp: "restart_app",
      appVersion: "app_version",
      updateStatus: "update_status",
      checkUpdate: "check_update",
      installUpdate: "install_update",
      openReleasesPage: "open_releases_page",
      getUpdateAutoCheck: "get_update_auto_check",
      setUpdateAutoCheck: "set_update_auto_check",
    });
    expect(UPDATE_EVENT).toBe("update-status");
    const invoke = shell(async (cmd) => {
      if (cmd === "get_autostart") return true;
      if (cmd === "app_version") return " 0.3.0 ";
      return null;
    });
    expect(isDesktop()).toBe(true);
    expect(await getAutostart()).toBe(true);
    expect(await appVersion()).toBe("0.3.0");
    await setAutostart(false);
    await restartApp();
    expect(invoke.mock.calls).toEqual([
      ["get_autostart", undefined],
      ["app_version", undefined],
      ["set_autostart", { enabled: false }],
      ["restart_app", undefined],
    ]);
  });

  it("keeps `this` for invoke (it is a method of `core`)", async () => {
    const core = {
      answer: true,
      invoke(this: { answer: boolean }, _cmd: string) {
        return Promise.resolve(this.answer);
      },
    };
    window.__TAURI__ = { core };
    expect(await getAutostart()).toBe(true);
  });

  it("turns any rejection into a readable DesktopError", async () => {
    const cases: [unknown, string][] = [
      ["autostart: access denied", "autostart: access denied"],
      [new Error("boom"), "boom"],
      [{ message: "command not found" }, "command not found"],
      [{ code: 5 }, '{"code":5}'],
    ];
    for (const [thrown, message] of cases) {
      shell(async () => {
        throw thrown;
      });
      const err = await setAutostart(true).catch((e: unknown) => e);
      expect(err).toBeInstanceOf(DesktopError);
      expect((err as DesktopError).message).toBe(message);
      expect((err as DesktopError).command).toBe("set_autostart");
    }
  });

  it("refuses answers of the wrong type", async () => {
    shell(async () => "yes");
    await expect(getAutostart()).rejects.toBeInstanceOf(DesktopError);
    shell(async () => 3);
    await expect(appVersion()).rejects.toBeInstanceOf(DesktopError);
    shell(async () => "  ");
    await expect(appVersion()).rejects.toBeInstanceOf(DesktopError);
  });
});

/** What the shell sends (`updates::Snapshot`, see its `snapshot_json_is_the_ui_contract` test). */
const SNAPSHOT = {
  current_version: "0.6.0",
  auto_check: true,
  phase: "idle",
  last_check_ms: 5,
  last_check_ok: true,
  available: { version: "0.6.1", date: "2026-11-12T10:00:00Z", notes: "- nouveauté" },
  progress: null,
  error: null,
  updated_from: null,
};

describe("updates", () => {
  it("parses the shell's update state", () => {
    expect(parseUpdateStatus(SNAPSHOT)).toEqual(SNAPSHOT);
    expect(parseUpdateStatus(null)).toBeNull();
    expect(parseUpdateStatus({ phase: "idle" })).toBeNull();
    expect(parseUpdateStatus("0.6.0")).toBeNull();
    // A newer shell: unknown values are degraded, not fatal.
    const odd = parseUpdateStatus({
      ...SNAPSHOT,
      phase: "verifying",
      available: { version: "  " },
      progress: { downloaded: 10, total: 0 },
      error: { kind: "quota", message: 3 },
      auto_check: "yes",
    });
    expect(odd?.phase).toBe("idle");
    expect(odd?.available).toBeNull();
    expect(odd?.progress).toEqual({ downloaded: 10, total: null });
    expect(odd?.error).toEqual({ kind: "check", message: "" });
    expect(odd?.auto_check).toBe(true);
  });

  it("calls the update commands of the contract", async () => {
    const invoke = shell(async (cmd) => {
      if (cmd === "update_status" || cmd === "check_update") return SNAPSHOT;
      if (cmd === "get_update_auto_check") return false;
      return null;
    });
    expect((await updateStatus()).available?.version).toBe("0.6.1");
    expect((await checkUpdate()).current_version).toBe("0.6.0");
    expect(await getUpdateAutoCheck()).toBe(false);
    await setUpdateAutoCheck(true);
    await openReleasesPage();
    await installUpdate();
    expect(invoke.mock.calls).toEqual([
      ["update_status", undefined],
      ["check_update", undefined],
      ["get_update_auto_check", undefined],
      ["set_update_auto_check", { enabled: true }],
      ["open_releases_page", undefined],
      ["install_update", undefined],
    ]);
  });

  it("passes the shell's refusal through", async () => {
    shell(async () => {
      throw "Le jeu est ouvert : ferme Le Mans Ultimate, puis relance l'installation.";
    });
    const err = await installUpdate().catch((e: unknown) => e);
    expect(err).toBeInstanceOf(DesktopError);
    expect((err as DesktopError).message).toMatch(/^Le jeu est ouvert/);
    shell(async () => ({ nope: true }));
    await expect(updateStatus()).rejects.toBeInstanceOf(DesktopError);
  });

  it("listens to the update event when the shell offers it", async () => {
    expect(await onUpdateStatus(() => {})).toBeNull(); // browser
    shell(async () => null);
    expect(await onUpdateStatus(() => {})).toBeNull(); // no event API

    const box: { handler?: (e: { payload: unknown }) => void } = {};
    const unlisten = vi.fn();
    const listen = vi.fn(async (_name: string, h: (e: { payload: unknown }) => void) => {
      box.handler = h;
      return unlisten;
    });
    window.__TAURI__ = { core: { invoke: async () => null }, event: { listen } };
    const seen: string[] = [];
    const off = await onUpdateStatus((s) => seen.push(s.phase));
    expect(listen.mock.calls[0]?.[0]).toBe("update-status");
    box.handler?.({ payload: { ...SNAPSHOT, phase: "downloading" } });
    box.handler?.({ payload: "garbage" });
    expect(seen).toEqual(["downloading"]);
    off?.();
    expect(unlisten).toHaveBeenCalledOnce();
  });
});

describe("isLocalPage", () => {
  it("recognises the PC that runs Corde", () => {
    for (const host of ["127.0.0.1", "localhost", "LOCALHOST", "[::1]", "::1", "tauri.localhost"]) {
      expect(isLocalPage(host), host).toBe(true);
    }
    for (const host of ["192.168.1.20", "pc-jeu", "10.0.0.4", "127.0.0.1.evil.example", ""]) {
      expect(isLocalPage(host), host).toBe(false);
    }
  });
});
