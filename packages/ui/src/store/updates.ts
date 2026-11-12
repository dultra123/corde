import { create } from "zustand";
import {
  checkUpdate,
  DesktopError,
  installUpdate,
  isDesktop,
  onUpdateStatus,
  openReleasesPage,
  setUpdateAutoCheck,
  type UpdateStatus,
  updateStatus,
} from "../lib/desktop";

/** Refresh period of the update state: fast while something happens, slow otherwise. */
const POLL_BUSY_MS = 1000;
const POLL_IDLE_MS = 60_000;

interface UpdatesState {
  /** Last state received from the shell (null in a browser, or until the first answer). */
  status: UpdateStatus | null;
  /** A command rejected without the state saying why (opening the browser, saving the setting). */
  actionError: string | null;
  started: boolean;

  /** Starts listening to the shell (desktop only; idempotent). */
  start(): void;
  refresh(): Promise<void>;
  check(): Promise<void>;
  install(): Promise<void>;
  setAutoCheck(enabled: boolean): Promise<void>;
  openReleases(): Promise<void>;
}

function messageOf(e: unknown): string {
  return e instanceof DesktopError || e instanceof Error ? e.message : String(e);
}

/**
 * The desktop application's update state, shared by the settings dialog, the settings badge and
 * the home page. Fed by the shell's `update-status` event, with a slow poll as a safety net (and
 * a fast one while a check or a download runs, in case events do not arrive).
 */
export const useUpdates = create<UpdatesState>((set, get) => {
  let timer: ReturnType<typeof setTimeout> | null = null;

  /** Next refresh: soon while something happens (or is about to), otherwise in a minute. */
  const schedule = (soon = false) => {
    if (timer) clearTimeout(timer);
    const status = get().status;
    const busy = soon || (status !== null && status.phase !== "idle");
    timer = setTimeout(
      () => {
        void get()
          .refresh()
          .finally(() => schedule());
      },
      busy ? POLL_BUSY_MS : POLL_IDLE_MS,
    );
  };

  const receive = (status: UpdateStatus) => {
    set({ status });
    if (status.phase !== "idle") schedule();
  };

  return {
    status: null,
    actionError: null,
    started: false,

    start() {
      if (get().started || !isDesktop()) return;
      set({ started: true });
      void onUpdateStatus(receive);
      void get()
        .refresh()
        .finally(() => schedule());
    },

    async refresh() {
      try {
        set({ status: await updateStatus() });
      } catch {
        // the shell is restarting, or this is not the desktop application: keep the last state
      }
    },

    async check() {
      set({ actionError: null });
      schedule(true);
      try {
        receive(await checkUpdate());
      } catch (e) {
        set({ actionError: messageOf(e) });
      }
      schedule();
    },

    async install() {
      set({ actionError: null });
      schedule(true);
      try {
        // Never settles on success: Corde quits, the installer restarts it.
        await installUpdate();
      } catch {
        // Refused or failed: the shell put the reason in the state.
      }
      await get().refresh();
      schedule();
    },

    async setAutoCheck(enabled: boolean) {
      const before = get().status;
      set({
        actionError: null,
        status: before ? { ...before, auto_check: enabled } : before,
      });
      try {
        await setUpdateAutoCheck(enabled);
      } catch (e) {
        set({ status: before, actionError: messageOf(e) });
      }
    },

    async openReleases() {
      set({ actionError: null });
      try {
        await openReleasesPage();
      } catch (e) {
        set({ actionError: messageOf(e) });
      }
    },
  };
});
