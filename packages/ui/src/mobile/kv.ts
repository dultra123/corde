import type { PrefsPort } from "./ports";

/**
 * JSON values in the phone's native storage (Capacitor Preferences: `UserDefaults` on iOS,
 * `SharedPreferences` on Android, in the application's own sandbox). Every key of the
 * application starts with `corde.`.
 */
export const KEYS = {
  servers: "corde.servers.v1",
  activeServer: "corde.servers.active",
  /** One key per server: the device token is never stored with the list (never shown). */
  token: (serverId: string) => `corde.servers.token.${serverId}`,
  racesEnabled: "corde.races.enabled",
  picks: "corde.races.picks.v1",
  raceSettings: "corde.races.settings.v1",
  /** Notifications already asked for (the system asks the driver once). */
  notifAsked: "corde.races.notifications.asked",
  /** Android: « Alarmes et rappels » already offered (once, after the notifications). */
  exactAsked: "corde.races.exactAlarms.asked",
} as const;

/** The value of `key`, `null` when absent or unreadable (broken JSON = absent). */
export async function readJson(prefs: PrefsPort, key: string): Promise<unknown> {
  try {
    const { value } = await prefs.get({ key });
    if (value == null) return null;
    return JSON.parse(value) as unknown;
  } catch {
    return null;
  }
}

export async function writeJson(prefs: PrefsPort, key: string, value: unknown): Promise<void> {
  await prefs.set({ key, value: JSON.stringify(value) });
}

export async function readText(prefs: PrefsPort, key: string): Promise<string | null> {
  try {
    const { value } = await prefs.get({ key });
    return value ?? null;
  } catch {
    return null;
  }
}
