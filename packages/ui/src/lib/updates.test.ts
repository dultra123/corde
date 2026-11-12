import { describe, expect, it } from "vitest";
import { en } from "../i18n/en";
import { fr } from "../i18n/fr";
import type { UpdateErrorKind, UpdateStatus } from "./desktop";
import {
  hasUpdate,
  noteLines,
  noticeKey,
  progressPercent,
  releaseDate,
  updateView,
} from "./updates";

function status(over: Partial<UpdateStatus> = {}): UpdateStatus {
  return {
    current_version: "0.6.0",
    auto_check: true,
    phase: "idle",
    last_check_ms: null,
    last_check_ok: null,
    available: null,
    progress: null,
    error: null,
    updated_from: null,
    ...over,
  };
}

const NEXT = { version: "0.6.1", date: "2026-11-20T18:00:00Z", notes: "## Nouveautés\n- un" };

describe("updateView", () => {
  it("in a browser, only says that updates happen on the PC", () => {
    const v = updateView(status({ available: NEXT }), false);
    expect(v.state).toBe("browser");
    expect(v.canCheck).toBe(false);
    expect(v.canInstall).toBe(false);
    expect(updateView(null, false).state).toBe("browser");
  });

  it("waits for the first answer of the shell", () => {
    const v = updateView(null, true);
    expect(v.state).toBe("loading");
    expect(v.canCheck).toBe(false);
  });

  it("tells never checked / up to date / available apart", () => {
    expect(updateView(status(), true).state).toBe("never");
    const upToDate = updateView(status({ last_check_ms: 5, last_check_ok: true }), true);
    expect(upToDate.state).toBe("upToDate");
    expect(upToDate.lastCheckMs).toBe(5);
    expect(upToDate.canCheck).toBe(true);
    expect(upToDate.canInstall).toBe(false);

    const available = updateView(
      status({ last_check_ms: 5, last_check_ok: true, available: NEXT }),
      true,
    );
    expect(available.state).toBe("available");
    expect(available.available?.version).toBe("0.6.1");
    expect(available.canInstall).toBe(true);
    expect(available.current).toBe("0.6.0");
  });

  it("a failed check is a simple message and points to the releases page", () => {
    const v = updateView(
      status({
        last_check_ms: 5,
        last_check_ok: false,
        error: { kind: "check", message: "Could not fetch a valid release JSON from the remote" },
      }),
      true,
    );
    expect(v.state).toBe("checkFailed");
    expect(v.notice).toBe("update.err.check");
    expect(v.noticeIsError).toBe(true);
    expect(v.suggestReleasesPage).toBe(true);
    // never the technical text
    expect(fr["update.err.check"]).toMatch(/^Impossible de vérifier pour l'instant/);
    expect(fr["update.err.check"]).not.toMatch(/JSON|404|remote/);
  });

  it("keeps a known update when a later check fails", () => {
    const v = updateView(
      status({
        last_check_ok: false,
        last_check_ms: 9,
        available: NEXT,
        error: { kind: "check", message: "offline" },
      }),
      true,
    );
    expect(v.state).toBe("available");
    expect(v.canInstall).toBe(true);
    expect(v.notice).toBe("update.err.check");
  });

  it("refusals (game running, recording) are instructions, not failures", () => {
    for (const kind of ["game_running", "recording", "activity_unknown"] as const) {
      const v = updateView(
        status({ available: NEXT, error: { kind, message: "refusé" }, last_check_ok: true }),
        true,
      );
      expect(v.notice).toBe(noticeKey(kind));
      expect(v.noticeIsError).toBe(false);
      expect(v.suggestReleasesPage).toBe(false);
      expect(v.canInstall).toBe(true);
    }
  });

  it("shows the download progress and blocks the buttons", () => {
    const v = updateView(
      status({
        phase: "downloading",
        available: NEXT,
        progress: { downloaded: 12_345_678, total: 24_691_356 },
      }),
      true,
    );
    expect(v.state).toBe("downloading");
    expect(v.percent).toBe(50);
    expect(v.downloadedMb).toBe(12.3);
    expect(v.canCheck).toBe(false);
    expect(v.canInstall).toBe(false);

    const unknownSize = updateView(
      status({
        phase: "downloading",
        available: NEXT,
        progress: { downloaded: 3_000_000, total: null },
      }),
      true,
    );
    expect(unknownSize.percent).toBeNull();
    expect(unknownSize.downloadedMb).toBe(3);

    const installing = updateView(status({ phase: "installing", available: NEXT }), true);
    expect(installing.state).toBe("installing");
    expect(installing.percent).toBeNull();
    expect(installing.canCheck).toBe(false);
  });

  it("does not show an error while busy", () => {
    const v = updateView(
      status({ phase: "checking", error: { kind: "check", message: "x" } }),
      true,
    );
    expect(v.state).toBe("checking");
    expect(v.notice).toBeNull();
  });

  it("reports the automatic check setting and a just-installed update", () => {
    const v = updateView(status({ auto_check: false, updated_from: "0.5.1" }), true);
    expect(v.autoCheck).toBe(false);
    expect(v.updatedFrom).toBe("0.5.1");
  });
});

describe("badge and home line", () => {
  it("only when an update is known and not being installed", () => {
    expect(hasUpdate(null)).toBe(false);
    expect(hasUpdate(status())).toBe(false);
    expect(hasUpdate(status({ available: NEXT }))).toBe(true);
    expect(hasUpdate(status({ available: NEXT, phase: "downloading" }))).toBe(true);
    expect(hasUpdate(status({ available: NEXT, phase: "installing" }))).toBe(false);
  });
});

describe("helpers", () => {
  it("progressPercent is bounded and needs a total", () => {
    expect(progressPercent(null)).toBeNull();
    expect(progressPercent({ downloaded: 5, total: null })).toBeNull();
    expect(progressPercent({ downloaded: 0, total: 10 })).toBe(0);
    expect(progressPercent({ downloaded: 9, total: 10 })).toBe(90);
    expect(progressPercent({ downloaded: 15, total: 10 })).toBe(100);
  });

  it("every error kind has a message in both languages", () => {
    const kinds: UpdateErrorKind[] = [
      "check",
      "download",
      "install",
      "game_running",
      "recording",
      "activity_unknown",
      "busy",
      "nothing_to_install",
    ];
    for (const kind of kinds) {
      const key = noticeKey(kind);
      expect(fr[key], kind).toBeTruthy();
      expect(en[key], kind).toBeTruthy();
    }
    expect(noticeKey("game_running")).toBe("update.err.game");
  });

  it("releaseDate reads RFC 3339 and gives up on anything else", () => {
    expect(releaseDate("2026-11-20T18:00:00Z", "fr")).toMatch(/2026/);
    expect(releaseDate("2026-11-20T18:00:00Z", "fr")).toMatch(/novembre/);
    expect(releaseDate(null, "fr")).toBeNull();
    expect(releaseDate("bientôt", "fr")).toBeNull();
  });

  it("noteLines turns the changelog section into readable lines", () => {
    expect(noteLines(null)).toEqual([]);
    expect(
      noteLines("### Nouveautés\n\n- **Mises à jour** depuis les réglages\n  - `latest.json`\n"),
    ).toEqual(["Nouveautés", "• Mises à jour depuis les réglages", "  • latest.json"]);
    // Wrapped list items (as in CHANGELOG.md) are joined back.
    expect(
      noteLines("- **Mise à jour** : Réglages, puis\n  « Installer et redémarrer ».\n- Deux"),
    ).toEqual(["• Mise à jour : Réglages, puis « Installer et redémarrer ».", "• Deux"]);
    const long = Array.from({ length: 40 }, (_, i) => `- ligne ${i}`).join("\n");
    const lines = noteLines(long, 30);
    expect(lines).toHaveLength(31);
    expect(lines[30]).toBe("…");
  });
});
