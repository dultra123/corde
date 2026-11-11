import { create } from "zustand";

export type Lang = "fr" | "en";

export interface Settings {
  /** `host:port` typed by the user, or empty to use the page's own host. */
  server: string;
  lang: Lang;
  /** Frame rate requested from the server, Hz. */
  rateHz: number;
  /** Open the live dashboard when the driver takes the track (`nav/auto`). Per device. */
  autoDashboard: boolean;
  /** Show the session summary when the driver is back in the garage (`nav/auto`). Per device. */
  autoSummary: boolean;
}

const KEY = "corde.settings.v1";
/** legacy name (Muret, before 0.3.0): read when the new key is absent. */
const LEGACY_KEY = "muret.settings.v1";

const DEFAULTS: Settings = {
  server: "",
  lang: "fr",
  rateHz: 20,
  autoDashboard: true,
  autoSummary: true,
};

function detectLang(): Lang {
  if (typeof navigator === "undefined") return "fr";
  return navigator.language?.toLowerCase().startsWith("fr") ? "fr" : "en";
}

function load(): Settings {
  try {
    const raw = localStorage.getItem(KEY) ?? localStorage.getItem(LEGACY_KEY);
    if (!raw) return { ...DEFAULTS, lang: detectLang() };
    const parsed = JSON.parse(raw) as Partial<Settings>;
    return {
      server: typeof parsed.server === "string" ? parsed.server : "",
      lang: parsed.lang === "en" ? "en" : "fr",
      rateHz: typeof parsed.rateHz === "number" ? parsed.rateHz : DEFAULTS.rateHz,
      // absent (settings saved by an older version) → on
      autoDashboard: parsed.autoDashboard !== false,
      autoSummary: parsed.autoSummary !== false,
    };
  } catch {
    return { ...DEFAULTS, lang: detectLang() };
  }
}

interface SettingsState extends Settings {
  update(patch: Partial<Settings>): void;
}

export const useSettings = create<SettingsState>((set, get) => ({
  ...load(),
  update(patch) {
    set(patch);
    const { server, lang, rateHz, autoDashboard, autoSummary } = get();
    try {
      localStorage.setItem(
        KEY,
        JSON.stringify({ server, lang, rateHz, autoDashboard, autoSummary }),
      );
    } catch {
      // storage can be unavailable (private mode) — settings then live for the session only
    }
  },
}));
