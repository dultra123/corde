#!/usr/bin/env node
/**
 * Regenerates every icon of the desktop app from the vector logo.
 *
 *   assets/corde-icon.svg ──Chromium (Playwright)──▶ assets/corde-icon-1024.png
 *                                                    src-tauri/icons/*.png (each size drawn from the SVG)
 *                         ──Pillow (pack-icons.py)──▶ src-tauri/icons/icon.ico (16…256), icon.icns (16…1024)
 *
 * Every size is rendered from the vector, not scaled down from the big PNG, so the small ones
 * (16 / 24 / 32 px: notification area, task bar) stay sharp. Needs the `playwright` package of
 * the UI workspace (with its Chromium) and Python 3 with Pillow.
 *
 *   node apps/desktop/scripts/make-icons.mjs
 */
import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const desktop = resolve(here, "..");
const repo = resolve(desktop, "../..");
const require = createRequire(join(repo, "packages/ui/package.json"));
const { chromium } = require("playwright");

const svg = readFileSync(join(desktop, "assets/corde-icon.svg"), "utf8");
const icons = join(desktop, "src-tauri/icons");
mkdirSync(icons, { recursive: true });

/** Files Tauri expects in `src-tauri/icons` (tauri.conf.json + Windows Store logos). */
const pngs = [
  ["32x32.png", 32],
  ["64x64.png", 64],
  ["128x128.png", 128],
  ["128x128@2x.png", 256],
  ["icon.png", 512],
  ["Square30x30Logo.png", 30],
  ["Square44x44Logo.png", 44],
  ["Square71x71Logo.png", 71],
  ["Square89x89Logo.png", 89],
  ["Square107x107Logo.png", 107],
  ["Square142x142Logo.png", 142],
  ["Square150x150Logo.png", 150],
  ["Square284x284Logo.png", 284],
  ["Square310x310Logo.png", 310],
  ["StoreLogo.png", 50],
];
/** Frames packed into icon.ico / icon.icns. */
const frames = [16, 24, 32, 48, 64, 128, 256, 512, 1024];

const tmp = mkdtempSync(join(tmpdir(), "corde-icons-"));
const browser = await chromium.launch();
try {
  const render = async (size, file) => {
    const page = await browser.newPage({
      viewport: { width: size, height: size },
      deviceScaleFactor: 1,
    });
    const sized = svg.replace(/width="1024" height="1024"/, `width="${size}" height="${size}"`);
    await page.setContent(
      `<body style="margin:0;background:transparent;line-height:0">${sized}</body>`,
    );
    await page.screenshot({ path: file, omitBackground: true });
    await page.close();
    console.log(file);
  };
  await render(1024, join(desktop, "assets/corde-icon-1024.png"));
  for (const [name, size] of pngs) await render(size, join(icons, name));
  for (const size of frames) await render(size, join(tmp, `${size}.png`));
} finally {
  await browser.close();
}

execFileSync("python3", [join(here, "pack-icons.py"), tmp, icons], { stdio: "inherit" });
rmSync(tmp, { recursive: true, force: true });
