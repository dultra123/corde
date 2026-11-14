import { App } from "@capacitor/app";
import { Capacitor, CapacitorCookies, CapacitorHttp, registerPlugin } from "@capacitor/core";
import { Keyboard, KeyboardResize, KeyboardStyle } from "@capacitor/keyboard";
import { LocalNotifications } from "@capacitor/local-notifications";
import { Preferences } from "@capacitor/preferences";
import { ScreenOrientation } from "@capacitor/screen-orientation";
import type { AndroidNotificationsPort } from "./android";
import { type PhoneTraits, traitsOf } from "./device";
import type { CookiesPort, HttpPort, NativePort, NotificationsPort, PrefsPort } from "./ports";

/**
 * The real Capacitor plugins behind the ports of `mobile/ports.ts`. Only `mobile/boot.ts`
 * imports this module, so only the application's bundle carries Capacitor.
 */

/** The phone: what the application has on it (`device.ts`). */
export const traits: PhoneTraits = traitsOf(Capacitor.getPlatform());

export const prefs: PrefsPort = Preferences;
export const http: HttpPort = CapacitorHttp;
export const cookies: CookiesPort = CapacitorCookies;
export const notifications: NotificationsPort = LocalNotifications;
/** The methods of the notifications plugin that only Android has (`mobile/android.ts`). */
export const androidNotifications: AndroidNotificationsPort = LocalNotifications;
/**
 * The application's own module (`CordeNativePlugin.swift`, registered by
 * `CordeBridgeViewController`): widgets and Live Activity. No JavaScript package: the bridge
 * calls the Swift methods by name. iPhone only: on Android (`traits.widgets` false) nothing
 * calls it (`registerPlugin` itself calls nothing).
 */
export const native: NativePort = registerPlugin<NativePort>("CordeNative");
export { App, Keyboard, KeyboardResize, KeyboardStyle, LocalNotifications, ScreenOrientation };

/**
 * The QR code scanner of `@capacitor/barcode-scanner` (official plugin, Swift Package), called
 * through the native bridge without the plugin's JavaScript: that module also loads a web
 * scanner (`html5-qrcode`) the iPhone never uses. The options are those the plugin's own
 * JavaScript fills in (`dist/esm/index.js`) and its Swift side requires
 * (`OSBARCScanParameters+Decodable.swift`): `hint` 0 = QR code (`Html5QrcodeSupportedFormats`),
 * camera 1 = back, orientation 3 = adaptive. On Android the scanning library is ZXing (the
 * other one, Google's ML Kit, is left out of the application: `android/app/build.gradle`).
 */
interface ScannerPlugin {
  scanBarcode(options: {
    hint: number;
    scanInstructions: string;
    scanButton: boolean;
    scanText: string;
    cameraDirection: number;
    scanOrientation: number;
    android?: { scanningLibrary: "zxing" };
  }): Promise<{ ScanResult: string; format: number }>;
}

const Scanner = registerPlugin<ScannerPlugin>("CapacitorBarcodeScanner");

export async function scanQrCode(instructions: string): Promise<string | null> {
  try {
    const r = await Scanner.scanBarcode({
      hint: 0,
      scanInstructions: instructions,
      scanButton: false,
      scanText: " ",
      cameraDirection: 1,
      scanOrientation: 3,
      ...(traits.os === "android" ? { android: { scanningLibrary: "zxing" as const } } : {}),
    });
    return typeof r.ScanResult === "string" && r.ScanResult ? r.ScanResult : null;
  } catch {
    // cancelled, camera refused, no camera (simulator)
    return null;
  }
}

/** One line of diagnosis in the device log (`console` goes to the native log in Capacitor). */
export function log(line: string): void {
  console.info(`[corde] ${line}`);
}
