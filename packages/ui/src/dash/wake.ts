import { useEffect } from "react";
import { appSlots } from "../mobile/slots";

/** Turns the system's « keep the screen on » on or off. */
export type KeepAwakeSetter = (on: boolean) => Promise<void>;

/**
 * Keeps the screen on while it is wanted **and** the page is visible. The system lets go of a
 * browser's screen lock when the page goes to the background: it is taken again when the page
 * comes back. The setter is only called when the result changes; an error of the system is
 * ignored (the screen then goes to sleep as usual).
 */
export class KeepAwake {
  private wanted = false;
  private visible = true;
  private held = false;

  constructor(private readonly setter: KeepAwakeSetter) {}

  want(on: boolean): void {
    this.wanted = on;
    this.apply();
  }

  setVisible(visible: boolean): void {
    this.visible = visible;
    this.apply();
  }

  private apply(): void {
    const on = this.wanted && this.visible;
    if (on === this.held) return;
    this.held = on;
    this.setter(on).catch(() => {
      // refused (battery saver, no permission…): try again at the next change
      if (on && this.held) this.held = false;
    });
  }
}

interface WakeLockSentinelLike {
  release(): Promise<void>;
}
interface WakeLockLike {
  request(type: "screen"): Promise<WakeLockSentinelLike>;
}

/** `navigator.wakeLock` (Screen Wake Lock API) when the browser has it, else `null`. */
export function browserSetter(nav: { wakeLock?: unknown } = navigator): KeepAwakeSetter | null {
  const lock = nav.wakeLock as WakeLockLike | undefined;
  if (!lock || typeof lock.request !== "function") return null;
  let sentinel: WakeLockSentinelLike | null = null;
  return async (on) => {
    if (on) {
      sentinel = await lock.request("screen");
      return;
    }
    const s = sentinel;
    sentinel = null;
    await s?.release();
  };
}

/**
 * The screen stays on while the phone dashboard is shown and `enabled` (the setting): in the
 * application by the phone's own switch (`appSlots.keepAwake`: `isIdleTimerDisabled` on the
 * iPhone, `FLAG_KEEP_SCREEN_ON` on Android), in a browser by `navigator.wakeLock` when it
 * exists. Let go when the dashboard closes or the setting goes off.
 */
export function useKeepAwake(enabled: boolean): void {
  useEffect(() => {
    if (!enabled) return;
    const setter = appSlots.keepAwake ?? browserSetter();
    if (!setter) return;
    const keep = new KeepAwake(setter);
    keep.setVisible(document.visibilityState !== "hidden");
    keep.want(true);
    const onVisibility = () => keep.setVisible(document.visibilityState !== "hidden");
    document.addEventListener("visibilitychange", onVisibility);
    return () => {
      document.removeEventListener("visibilitychange", onVisibility);
      keep.want(false);
    };
  }, [enabled]);
}
