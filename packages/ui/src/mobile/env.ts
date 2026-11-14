/**
 * The iPhone application (`apps/mobile`, Capacitor) is the same interface built with
 * `vite build --mode mobile`. `APP_BUILD` is a build-time constant: in the browser build every
 * branch that tests it is removed, and the modules of `src/mobile/` that only the application
 * loads (`boot.ts` and what it imports) are not even emitted.
 */
export const APP_BUILD: boolean = import.meta.env.MODE === "mobile";

/** Version of the interface (`package.json`), sent in the `User-Agent` of the phone. */
export const APP_VERSION: string =
  typeof __CORDE_VERSION__ === "string" ? __CORDE_VERSION__ : "0.0.0";

/** `Corde/1.0.0 (+https://github.com/dultra123/corde)` — same form as the server's. */
export function userAgent(version: string = APP_VERSION): string {
  return `Corde/${version} (+https://github.com/dultra123/corde)`;
}
