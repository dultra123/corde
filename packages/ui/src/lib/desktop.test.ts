import { afterEach, describe, expect, it, vi } from "vitest";
import {
  appVersion,
  DESKTOP_COMMANDS,
  DesktopError,
  getAutostart,
  isDesktop,
  isLocalPage,
  restartApp,
  setAutostart,
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
    });
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
