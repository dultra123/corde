#!/usr/bin/env node
/**
 * Regenerates the images and the sound of the iPhone application from the vector logo of the
 * desktop application (`apps/desktop/assets/corde-icon.svg`, same sign and colours):
 *
 *   ios/App/App/Assets.xcassets/AppIcon.appiconset/AppIcon-512@2x.png
 *       1024 × 1024, **opaque** (App Store Connect refuses an icon with transparency), full
 *       bleed: iOS draws the rounded corners itself, so the dark background fills the square.
 *       A single 1024 px image is the whole icon set for Xcode 14 and later (the size the
 *       Capacitor template uses; Xcode derives the others).
 *   ios/App/App/Assets.xcassets/Splash.imageset/splash-2732x2732{,-1,-2}.png
 *       The launch screen: the sign on the interface's dark background (#0b0e12), drawn by
 *       `LaunchScreen.storyboard` (aspect fill: the centre stays on every screen).
 *   assets/corde-reminder.wav
 *       The sound of the race reminders (two soft notes, 1.1 s, 16-bit PCM): copied into the
 *       web assets by `scripts/finish-web.mjs`, where the notifications plugin looks for it.
 *
 * Every image is rendered by Chromium (Playwright) from SVG. Needs `playwright` with its
 * Chromium (not installed by this repository).
 *
 *   node apps/mobile/scripts/make-assets.mjs
 */
import { mkdirSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const mobile = resolve(here, "..");
const require = createRequire(import.meta.url);
const { chromium } = require("playwright");

const catalog = join(mobile, "ios/App/App/Assets.xcassets");
const BG_TOP = "#151a21";
const BG = "#0b0e12";
const ACCENT = "#ffb454";

/** The sign of the favicon (viewBox 32), drawn at `scale` around the centre of a `size` square. */
function sign(size, scale) {
  const offset = (size - 32 * scale) / 2;
  return `<g transform="translate(${offset} ${offset}) scale(${scale})" fill="none" stroke="${ACCENT}">
    <path d="M23.39 9.11A9 9 0 1 0 23.39 22.89" stroke-width="5" stroke-linecap="round"/>
    <circle cx="14.7" cy="16" r="2.8" fill="${ACCENT}" stroke="none"/>
  </g>`;
}

/** The icon: full-bleed gradient (no corner, no margin: iOS masks it), the sign at 2/3. */
const ICON = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024" width="1024" height="1024">
  <defs><linearGradient id="bg" x1="0" y1="0" x2="0" y2="1">
    <stop offset="0" stop-color="${BG_TOP}"/><stop offset="1" stop-color="${BG}"/>
  </linearGradient></defs>
  <rect width="1024" height="1024" fill="url(#bg)"/>
  ${sign(1024, 22)}
</svg>`;

/** The launch screen: flat background, a small sign in the middle. */
const SPLASH = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 2732 2732" width="2732" height="2732">
  <rect width="2732" height="2732" fill="${BG}"/>
  ${sign(2732, 9)}
</svg>`;

async function render(browser, svg, size, file) {
  const page = await browser.newPage({ viewport: { width: size, height: size }, deviceScaleFactor: 1 });
  await page.setContent(
    `<!doctype html><html><body style="margin:0;background:${BG}">${svg}</body></html>`,
  );
  // `omitBackground: false`: an opaque PNG (no alpha channel content).
  await page.screenshot({ path: file, omitBackground: false, clip: { x: 0, y: 0, width: size, height: size } });
  await page.close();
}

/** Two soft notes (A5 then E6) with a quick attack and an exponential fade, mono 44.1 kHz. */
function chime() {
  const rate = 44_100;
  const seconds = 1.1;
  const n = Math.round(rate * seconds);
  const pcm = new Int16Array(n);
  const notes = [
    { f: 880, start: 0, len: 0.55 },
    { f: 1318.5, start: 0.32, len: 0.78 },
  ];
  for (let i = 0; i < n; i += 1) {
    const t = i / rate;
    let v = 0;
    for (const { f, start, len } of notes) {
      const u = t - start;
      if (u < 0 || u > len) continue;
      const attack = Math.min(1, u / 0.012);
      const env = attack * Math.exp(-u * 5.5);
      v += env * (Math.sin(2 * Math.PI * f * u) + 0.25 * Math.sin(4 * Math.PI * f * u));
    }
    pcm[i] = Math.max(-1, Math.min(1, v * 0.42)) * 32767;
  }
  const data = Buffer.from(pcm.buffer);
  const head = Buffer.alloc(44);
  head.write("RIFF", 0);
  head.writeUInt32LE(36 + data.length, 4);
  head.write("WAVE", 8);
  head.write("fmt ", 12);
  head.writeUInt32LE(16, 16);
  head.writeUInt16LE(1, 20); // PCM
  head.writeUInt16LE(1, 22); // mono
  head.writeUInt32LE(rate, 24);
  head.writeUInt32LE(rate * 2, 28);
  head.writeUInt16LE(2, 32);
  head.writeUInt16LE(16, 34);
  head.write("data", 36);
  head.writeUInt32LE(data.length, 40);
  return Buffer.concat([head, data]);
}

const browser = await chromium.launch();
try {
  await render(browser, ICON, 1024, join(catalog, "AppIcon.appiconset/AppIcon-512@2x.png"));
  for (const name of ["splash-2732x2732.png", "splash-2732x2732-1.png", "splash-2732x2732-2.png"]) {
    await render(browser, SPLASH, 2732, join(catalog, "Splash.imageset", name));
  }
} finally {
  await browser.close();
}
mkdirSync(join(mobile, "assets"), { recursive: true });
writeFileSync(join(mobile, "assets/corde-reminder.wav"), chime());
console.log("icon, launch screen and reminder sound written");
