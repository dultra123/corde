#!/usr/bin/env node
/**
 * Renders the application icons of the web page from the logo of Corde (a rev counter: a grey
 * ring that turns amber in the red zone, a bold amber « C », a white needle; the drawing of the
 * design system, see `apps/desktop/assets/corde-icon.svg`) into `public/`:
 *
 *   icon-192.png, icon-512.png        dark rounded tile, for the web manifest (`purpose: any`)
 *   icon-maskable-512.png             full-bleed dark ground, sign inside the safe zone (`maskable`)
 *   apple-touch-icon.png (180)        full-bleed dark ground (iOS rounds the corners itself)
 *
 * `public/favicon.svg` is the mark itself (thicker needle, for small sizes): a copy of
 * `apps/desktop/assets/corde-mark.svg`. The PNGs are committed: run this only when the logo changes.
 *
 *   node scripts/make-icons.mjs
 */
import { mkdirSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const require = createRequire(import.meta.url);
const { chromium } = require("playwright");

const out = join(resolve(dirname(fileURLToPath(import.meta.url)), ".."), "public");
mkdirSync(out, { recursive: true });

const INK = "#0b0e12";
/** The sign in its 128-unit box, fine needle (64 px and more). Never redrawn nor recoloured. */
const SIGN =
  `<path d="M101.48 101.48A53 53 0 1 1 45.87 14.20" fill="none" stroke="#44526a" stroke-width="4"/>` +
  `<path d="M56.62 11.52A53 53 0 0 1 101.48 26.52" fill="none" stroke="#ffb454" stroke-width="4"/>` +
  `<path d="M75.24 91.82A30 30 0 1 1 75.24 36.18" fill="none" stroke="#ffb454" stroke-width="11"/>` +
  `<path d="M92.24 21.53L91.32 20.93L62.41 62.97L55.20 72.59L59.73 75.53L65.59 65.03Z" fill="#e9edf2"/>` +
  `<circle cx="64" cy="64" r="6.6" fill="#10141a" stroke="#44526a" stroke-width="1.3"/>` +
  `<circle cx="64" cy="64" r="1.7" fill="#2b3440"/>`;

/**
 * A dark tile of 128 units (corner `radius`; 0 = full bleed) and the sign at `scale` around the
 * centre: 0.82 is the size it has in the application icon, less keeps a maskable safe zone.
 */
const svg = (radius, scale) =>
  `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 128 128" width="100%" height="100%">` +
  `<defs><linearGradient id="bg" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#151a21"/>` +
  `<stop offset="1" stop-color="${INK}"/></linearGradient></defs>` +
  `<rect width="128" height="128" rx="${radius}" fill="url(#bg)"/>` +
  (radius
    ? `<rect x="1.5" y="1.5" width="125" height="125" rx="${radius - 1.5}" fill="none" stroke="#2b3440" stroke-width="3"/>`
    : "") +
  `<g transform="translate(64 64) scale(${scale}) translate(-64 -64)">${SIGN}</g></svg>`;

const icons = [
  { file: "icon-192.png", size: 192, radius: 26, scale: 0.82 },
  { file: "icon-512.png", size: 512, radius: 26, scale: 0.82 },
  { file: "icon-maskable-512.png", size: 512, radius: 0, scale: 0.62 },
  { file: "apple-touch-icon.png", size: 180, radius: 0, scale: 0.74 },
];

const browser = await chromium.launch();
try {
  for (const icon of icons) {
    const page = await browser.newPage({
      viewport: { width: icon.size, height: icon.size },
      deviceScaleFactor: 1,
    });
    await page.setContent(
      `<body style="margin:0;background:transparent">${svg(icon.radius, icon.scale)}</body>`,
    );
    const file = join(out, icon.file);
    await page.screenshot({ path: file, omitBackground: true });
    console.log(file);
    await page.close();
  }
} finally {
  await browser.close();
}
