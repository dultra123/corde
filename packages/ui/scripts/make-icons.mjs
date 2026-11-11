#!/usr/bin/env node
/**
 * Renders the application icons from the brand mark of the top bar (the dark racing line touching
 * its apex, a « C », on its amber tile, `public/favicon.svg`) into `public/`:
 *
 *   icon-192.png, icon-512.png        rounded tile, for the web manifest (`purpose: any`)
 *   icon-maskable-512.png             full-bleed tile, mark inside the safe zone (`maskable`)
 *   apple-touch-icon.png (180)        full-bleed tile (iOS rounds the corners itself)
 *
 * The PNGs are committed: run this only when the mark changes.
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

const AMBER = "#ffb454";
const INK = "#0b0e12";
/** Same mark as `public/favicon.svg`: the racing line (a « C ») and its apex. */
const mark = (fill) =>
  `<path d="M23.39 9.11A9 9 0 1 0 23.39 22.89" fill="none" stroke="${fill}" stroke-width="5" stroke-linecap="round"/>` +
  `<circle cx="14.7" cy="16" r="2.8" fill="${fill}"/>`;

/** `scale` < 1 shrinks the mark around the centre (maskable icons keep a safe zone). */
const svg = (radius, scale) =>
  `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32" width="100%" height="100%">` +
  `<rect width="32" height="32" rx="${radius}" fill="${AMBER}"/>` +
  `<g transform="translate(16 16) scale(${scale}) translate(-16 -16)">${mark(INK)}</g></svg>`;

const icons = [
  { file: "icon-192.png", size: 192, radius: 7, scale: 1 },
  { file: "icon-512.png", size: 512, radius: 7, scale: 1 },
  { file: "icon-maskable-512.png", size: 512, radius: 0, scale: 0.72 },
  { file: "apple-touch-icon.png", size: 180, radius: 0, scale: 0.86 },
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
