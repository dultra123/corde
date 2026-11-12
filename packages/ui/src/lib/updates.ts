/**
 * What the « Mises à jour » section, the settings badge and the home line show, computed from
 * the shell's update state (`lib/desktop` → `UpdateStatus`). Pure functions: the components only
 * translate the keys returned here.
 *
 * Rules (decided with the owner, a driver who does not code):
 * - in a browser or on a phone there is nothing to update: one line says it happens on the PC;
 * - a failed check (offline, repository still private → 404) says « impossible de vérifier pour
 *   l'instant » and offers the releases page — never the technical message;
 * - a known update gets a badge on the settings icon and a line on the home page, until it is
 *   being installed.
 */
import type { TranslationKey } from "../i18n";
import type { UpdateErrorKind, UpdateStatus } from "./desktop";

export type UpdateState =
  /** Not the desktop application: updates happen on the PC. */
  | "browser"
  /** Desktop, state not received yet. */
  | "loading"
  /** Never checked since Corde started (automatic check disabled, or not yet due). */
  | "never"
  | "checking"
  | "upToDate"
  | "available"
  | "downloading"
  | "installing"
  /** The last check failed and no update is known. */
  | "checkFailed";

export interface UpdateView {
  state: UpdateState;
  /** Installed version (null until known). */
  current: string | null;
  /** Newer version, with its date and notes, whenever one is known. */
  available: { version: string; date: string | null; notes: string | null } | null;
  /** Download progress, 0–100, or null when the size is unknown / not downloading. */
  percent: number | null;
  /** Megabytes received so far (one decimal), while downloading. */
  downloadedMb: number | null;
  /** Last check, ms since 1970. */
  lastCheckMs: number | null;
  /** Message to show under the buttons (refusal or failure), as a translation key. */
  notice: TranslationKey | null;
  /** The notice is an error (red) rather than an information. */
  noticeIsError: boolean;
  /** Put the releases page link forward (after a failure). */
  suggestReleasesPage: boolean;
  canCheck: boolean;
  canInstall: boolean;
  autoCheck: boolean;
  /** Version before the update, right after one. */
  updatedFrom: string | null;
}

const NOTICES: Record<UpdateErrorKind, TranslationKey> = {
  check: "update.err.check",
  download: "update.err.download",
  install: "update.err.install",
  game_running: "update.err.game",
  recording: "update.err.recording",
  activity_unknown: "update.err.unknown",
  busy: "update.err.busy",
  nothing_to_install: "update.err.nothing",
};

/** Failures after which the releases page is the way out. */
const SUGGEST_PAGE: ReadonlySet<UpdateErrorKind> = new Set(["check", "download", "install"]);

/** Translation key of the message for an error kind (never the technical text). */
export function noticeKey(kind: UpdateErrorKind): TranslationKey {
  return NOTICES[kind] ?? "update.err.check";
}

/** Download progress in whole percent, or null when the total is unknown. */
export function progressPercent(p: UpdateStatus["progress"]): number | null {
  if (!p?.total || p.total <= 0) return null;
  return Math.max(0, Math.min(100, Math.floor((p.downloaded / p.total) * 100)));
}

/** A newer version is known and not being installed yet: badge + home line. */
export function hasUpdate(status: UpdateStatus | null): boolean {
  return !!status?.available && status.phase !== "installing";
}

export function updateView(status: UpdateStatus | null, desktop: boolean): UpdateView {
  const empty: UpdateView = {
    state: desktop ? "loading" : "browser",
    current: null,
    available: null,
    percent: null,
    downloadedMb: null,
    lastCheckMs: null,
    notice: null,
    noticeIsError: false,
    suggestReleasesPage: false,
    canCheck: false,
    canInstall: false,
    autoCheck: true,
    updatedFrom: null,
  };
  if (!desktop || !status) return empty;

  const idle = status.phase === "idle";
  const error = status.error;
  let state: UpdateState;
  switch (status.phase) {
    case "checking":
      state = "checking";
      break;
    case "downloading":
      state = "downloading";
      break;
    case "installing":
      state = "installing";
      break;
    default:
      if (status.available) state = "available";
      else if (status.last_check_ok === false) state = "checkFailed";
      else if (status.last_check_ms !== null) state = "upToDate";
      else state = "never";
  }
  const downloading = state === "downloading" && status.progress;
  return {
    state,
    current: status.current_version,
    available: status.available,
    percent: downloading ? progressPercent(status.progress) : null,
    downloadedMb: downloading
      ? Math.round(((status.progress?.downloaded ?? 0) / 1_000_000) * 10) / 10
      : null,
    lastCheckMs: status.last_check_ms,
    notice: error && idle ? noticeKey(error.kind) : null,
    // Refusals (game running, recording…) are instructions, not failures.
    noticeIsError: !!error && idle && SUGGEST_PAGE.has(error.kind),
    suggestReleasesPage: !!error && idle && SUGGEST_PAGE.has(error.kind),
    canCheck: idle,
    canInstall: idle && !!status.available,
    autoCheck: status.auto_check,
    updatedFrom: status.updated_from,
  };
}

/** Release date (`pub_date`, RFC 3339) as a short local date, or null if unreadable. */
export function releaseDate(date: string | null, lang: string): string | null {
  if (!date) return null;
  const t = Date.parse(date);
  if (!Number.isFinite(t)) return null;
  return new Date(t).toLocaleDateString(lang, { day: "numeric", month: "long", year: "numeric" });
}

/**
 * Release notes as a list of lines, without Markdown headings markers and empty lines; a list
 * item keeps its bullet. Notes come from CHANGELOG.md through `latest.json`.
 */
export function noteLines(notes: string | null, max = 30): string[] {
  if (!notes) return [];
  const lines = notes
    .split(/\r?\n/)
    .map((l) => l.trimEnd())
    .filter((l) => l.trim() !== "")
    .map((l) => l.replace(/^#{1,6}\s+/, "").replace(/^(\s*)[*-]\s+/, "$1• "))
    .map((l) => l.replace(/\*\*(.+?)\*\*/g, "$1").replace(/`([^`]+)`/g, "$1"));
  return lines.length > max ? [...lines.slice(0, max), "…"] : lines;
}
