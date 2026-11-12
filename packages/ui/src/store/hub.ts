import { create } from "zustand";
import { onUnauthorized } from "../net/auth-events";
import { fetchHubPairing, type HubDeviceRef } from "../net/hub-api";
import { ApiError } from "../net/sessions-api";
import { useTelemetry } from "./telemetry";

/**
 * Which kind of server serves this page: the PC (`desktop`, the default — `/api/hub/pairing`
 * does not exist there) or the home server (`hub`), and on the home server whether this device
 * is paired. Asked once at start-up, again after a pairing, and whenever an API call is
 * answered 401 (revoked meanwhile).
 */
export type ServerRole = "unknown" | "desktop" | "hub";

interface HubState {
  role: ServerRole;
  paired: boolean;
  device: HubDeviceRef | null;
  version: string | null;
  /** The first answer came (or failed). */
  checked: boolean;
  check(): Promise<void>;
}

export const useHub = create<HubState>((set) => ({
  role: "unknown",
  paired: false,
  device: null,
  version: null,
  checked: false,
  async check() {
    try {
      const p = await fetchHubPairing();
      set({
        role: p.role === "hub" ? "hub" : "desktop",
        paired: p.paired,
        device: p.device,
        version: p.version,
        checked: true,
      });
    } catch (e) {
      // 404 (a PC), or no answer: the usual application, which shows its own offline state.
      if (e instanceof ApiError && e.status !== 404 && e.status !== 0) {
        set({ checked: true });
        return;
      }
      set((s) => ({ checked: true, role: s.role === "hub" ? "hub" : "desktop" }));
    }
  },
}));

onUnauthorized(() => {
  const s = useHub.getState();
  if (s.role === "hub" && s.paired) useHub.setState({ paired: false, device: null });
});

// The server closes the live socket of a device the moment it is revoked: ask again, so that an
// open page goes back to the pairing screen at once rather than at its next request.
useTelemetry.subscribe((s, prev) => {
  if (prev.socket === "open" && s.socket !== "open" && useHub.getState().role === "hub") {
    void useHub.getState().check();
  }
});

/** `true` on the home server. */
export function isHub(): boolean {
  return useHub.getState().role === "hub";
}

/** `true` on the home server, for a device marked administrator. */
export function isHubAdmin(): boolean {
  const s = useHub.getState();
  return s.role === "hub" && s.device?.admin === true;
}
