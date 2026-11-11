/**
 * The reminder sound, synthesised with WebAudio (no audio file): two soft notes for an early
 * reminder, a quick insistent pattern played twice for ≤ 5 min.
 *
 * Browsers only let a page make sound after the user touched it. `installAudioUnlock` wakes the
 * audio up on the first click or key press anywhere; « Tester la sonnerie » does it too. The
 * desktop shell's WebView2 already allows sound without a gesture (`--autoplay-policy`, a
 * default of the Tauri runtime): there the very first reminder rings even if the window was
 * never touched.
 */

type Ctor = typeof AudioContext;

let ctx: AudioContext | null = null;

function context(): AudioContext | null {
  if (ctx) return ctx;
  if (typeof window === "undefined") return null;
  const C: Ctor | undefined =
    window.AudioContext ?? (window as unknown as { webkitAudioContext?: Ctor }).webkitAudioContext;
  if (!C) return null;
  try {
    ctx = new C();
  } catch {
    return null;
  }
  return ctx;
}

/** Wake the audio up (call from a click). Resolves `true` when sound can play. */
export async function unlockAudio(): Promise<boolean> {
  const c = context();
  if (!c) return false;
  if (c.state === "suspended") {
    try {
      await c.resume();
    } catch {
      return false;
    }
  }
  return c.state === "running";
}

/** Notes of the two patterns: [start (s), frequency (Hz)]. */
export const PATTERNS = {
  soft: [
    [0, 880],
    [0.22, 1174.66],
  ],
  urgent: [
    [0, 1318.51],
    [0.13, 987.77],
    [0.26, 1318.51],
    [0.62, 1318.51],
    [0.75, 987.77],
    [0.88, 1318.51],
  ],
} as const;

/**
 * Play the reminder sound. Returns `false` when the page cannot make sound (no WebAudio, or the
 * browser still waits for a click).
 */
export function playChime(urgent: boolean): boolean {
  const c = context();
  if (!c) return false;
  if (c.state === "suspended") void c.resume().catch(() => {});
  const t0 = c.currentTime + 0.03;
  for (const [at, freq] of urgent ? PATTERNS.urgent : PATTERNS.soft) {
    const osc = c.createOscillator();
    const gain = c.createGain();
    osc.type = "triangle";
    osc.frequency.value = freq;
    const start = t0 + at;
    const len = urgent ? 0.11 : 0.32;
    gain.gain.setValueAtTime(0.0001, start);
    gain.gain.exponentialRampToValueAtTime(urgent ? 0.32 : 0.24, start + 0.012);
    gain.gain.exponentialRampToValueAtTime(0.0001, start + len);
    osc.connect(gain).connect(c.destination);
    osc.start(start);
    osc.stop(start + len + 0.02);
  }
  return c.state !== "suspended";
}

let installed = false;

/** Unlock the audio on the first user interaction with the page (once). */
export function installAudioUnlock(): void {
  if (installed || typeof window === "undefined") return;
  installed = true;
  const once = () => {
    void unlockAudio();
    window.removeEventListener("pointerdown", once, true);
    window.removeEventListener("keydown", once, true);
  };
  window.addEventListener("pointerdown", once, true);
  window.addEventListener("keydown", once, true);
}
