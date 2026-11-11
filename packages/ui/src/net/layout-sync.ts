import { useDashboard } from "../store/dashboard";
import { fetchLayouts, mergeLayouts, pushLayouts } from "./layouts-api";

const PUSH_DEBOUNCE_MS = 1500;

let httpBase: string | null = null;
let pushTimer: ReturnType<typeof setTimeout> | null = null;
let lastPushedRevision = -1;

/**
 * Called once a server said hello: pulls its layouts, merges (newest `updated_ms` wins) and
 * pushes the result back when the local side had something newer.
 */
export async function syncLayoutsWithServer(base: string): Promise<void> {
  httpBase = base;
  const dash = useDashboard.getState();
  dash.setSync("syncing");
  const remote = await fetchLayouts(base);
  if (remote === null) {
    dash.setSync("error");
    return;
  }
  const local = useDashboard.getState().layouts;
  const merged = mergeLayouts(local, remote);
  useDashboard.getState().replaceLayouts(merged, true);
  const localNewer = local.some((l) => {
    const r = remote.find((x) => x.id === l.id);
    return !r || r.updated_ms < l.updated_ms;
  });
  const remoteOrphans = remote.length !== merged.length;
  if (localNewer || remoteOrphans) await push(merged);
  else useDashboard.getState().setSync("synced");
}

async function push(layouts = useDashboard.getState().layouts): Promise<void> {
  if (!httpBase) return;
  const dash = useDashboard.getState();
  dash.setSync("syncing");
  const ok = await pushLayouts(httpBase, layouts);
  useDashboard.getState().setSync(ok ? "synced" : "error");
}

/** Debounced push on every local mutation, once a server is known. */
useDashboard.subscribe((cur, prev) => {
  if (cur.revision === prev.revision || !httpBase) return;
  if (cur.revision === lastPushedRevision) return;
  if (pushTimer) clearTimeout(pushTimer);
  pushTimer = setTimeout(() => {
    pushTimer = null;
    lastPushedRevision = useDashboard.getState().revision;
    void push();
  }, PUSH_DEBOUNCE_MS);
});
