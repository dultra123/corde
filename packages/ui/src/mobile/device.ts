import type { TranslationKey } from "../i18n";

/**
 * The phone the application runs on. The same bundle (`vite build --mode mobile`) is copied
 * into the Xcode project and into the Android project; what differs is decided here, from
 * `Capacitor.getPlatform()` (read once by `mobile/platform.ts`), and nowhere else.
 */
export type PhoneOs = "ios" | "android";

/** `Capacitor.getPlatform()` → the phone. Anything but `android` is the iPhone application. */
export function phoneOs(platform: string): PhoneOs {
  return platform === "android" ? "android" : "ios";
}

export interface PhoneTraits {
  os: PhoneOs;
  /**
   * The application's own native module `CordeNative` (iOS only, `CordeNativePlugin.swift`):
   * widgets, their pictures, the Live Activity, their diagnosis. Without it, nothing of
   * `widget-sync.ts` is created: no call to the module, no setting, no button.
   */
  widgets: boolean;
  /**
   * Android's notifications: a channel of their own (with Corde's sound), a monochrome small
   * icon, exact alarms when the driver allows them (`mobile/android.ts`).
   */
  channels: boolean;
  /** iOS asks for the local network at the first contact with a server (Android does not). */
  localNetworkPrompt: boolean;
  /** The back button / gesture of the system goes back in the interface (Android). */
  backButton: boolean;
  /** The proposed name of this phone when pairing with a home server. */
  deviceName: string;
}

export function traitsOf(platform: string): PhoneTraits {
  const os = phoneOs(platform);
  if (os === "android") {
    return {
      os,
      widgets: false,
      channels: true,
      localNetworkPrompt: false,
      backButton: true,
      deviceName: "Android",
    };
  }
  return {
    os,
    widgets: true,
    channels: false,
    localNetworkPrompt: true,
    backButton: false,
    deviceName: "iPhone",
  };
}

/**
 * The sentences that name the phone's system (« Réglages de l'iPhone → … ») have an Android
 * version: `<key>.android`. On the iPhone the key is unchanged.
 */
const ANDROID_TEXT = new Set<TranslationKey>([
  "races.reminders.hintPhone",
  "mobile.notif.denied",
  "mobile.notif.prompt",
]);

export function phoneTextKey(key: TranslationKey, os: PhoneOs): TranslationKey {
  return os === "android" && ANDROID_TEXT.has(key) ? (`${key}.android` as TranslationKey) : key;
}
