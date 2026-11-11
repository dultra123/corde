#!/usr/bin/env node
/**
 * Serves `dist/`, opens the dashboard in demo mode at two sizes and writes
 * screenshots/desktop.png and screenshots/mobile.png, plus screenshots/edit.png (the desktop
 * dashboard in edit mode: handles, remove buttons, widget palette). Also fails on browser
 * console errors.
 *
 *   pnpm --filter @corde/ui build && pnpm --filter @corde/ui screenshot
 *
 *   SCREENSHOT_WAIT_MS=8000     time left to the demo before each shot
 *   SCREENSHOT_FULL_TRACK=1     desktop shots wait until the track map has learnt a whole lap
 *                               (≈ 2 min in demo mode) — for the pictures of the README
 */
import { createReadStream, existsSync, mkdirSync, statSync } from "node:fs";
import { createServer } from "node:http";
import { createRequire } from "node:module";
import { dirname, extname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const require = createRequire(import.meta.url);
const { chromium } = require("playwright");

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const dist = join(root, "dist");
const outDir = join(root, "screenshots");
const WAIT_MS = Number(process.env.SCREENSHOT_WAIT_MS ?? 8000);
const FULL_TRACK = process.env.SCREENSHOT_FULL_TRACK === "1";

const MIME = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript",
  ".css": "text/css",
  ".svg": "image/svg+xml",
  ".woff2": "font/woff2",
  ".woff": "font/woff",
  ".png": "image/png",
  ".json": "application/json",
};

if (!existsSync(join(dist, "index.html"))) {
  console.error("dist/index.html missing — run `pnpm --filter @corde/ui build` first");
  process.exit(1);
}
mkdirSync(outDir, { recursive: true });

const server = createServer((req, res) => {
  const url = new URL(req.url ?? "/", "http://localhost");
  let file = join(dist, decodeURIComponent(url.pathname));
  if (!file.startsWith(dist)) {
    res.writeHead(403).end();
    return;
  }
  if (!existsSync(file) || statSync(file).isDirectory()) file = join(dist, "index.html");
  res.writeHead(200, { "Content-Type": MIME[extname(file)] ?? "application/octet-stream" });
  createReadStream(file).pipe(res);
});

await new Promise((r) => server.listen(0, "127.0.0.1", r));
const port = server.address().port;
const base = `http://127.0.0.1:${port}/?demo=1`;

const shots = [
  { name: "desktop", width: 1920, height: 1080, mobile: false },
  { name: "edit", width: 1920, height: 1080, mobile: false, edit: true },
  { name: "mobile", width: 390, height: 844, mobile: true },
];

const browser = await chromium.launch();
let failures = 0;
try {
  for (const shot of shots) {
    const context = await browser.newContext({
      viewport: { width: shot.width, height: shot.height },
      deviceScaleFactor: shot.mobile ? 2 : 1,
      isMobile: shot.mobile,
      hasTouch: shot.mobile,
      locale: "fr-FR",
    });
    const page = await context.newPage();
    const errors = [];
    page.on("console", (msg) => {
      if (msg.type() === "error") errors.push(msg.text());
    });
    page.on("pageerror", (err) => errors.push(String(err)));
    await page.goto(base, { waitUntil: "load" });
    await page.waitForTimeout(WAIT_MS);
    if (FULL_TRACK && !shot.mobile) {
      // The map widget says "Apprentissage du tracé n %" until the first whole lap is known.
      await page
        .waitForFunction(
          () => !/Apprentissage du tracé/.test(document.body.textContent ?? ""),
          null,
          {
            timeout: 240_000,
          },
        )
        .catch(() => console.error(`${shot.name}: track map still learning after 4 min`));
      await page.waitForTimeout(1500);
    }
    if (shot.edit) {
      await page.getByRole("button", { name: /Modifier/ }).click();
      await page.waitForTimeout(600);
    }
    const file = join(outDir, `${shot.name}.png`);
    await page.screenshot({ path: file, fullPage: shot.mobile });
    console.log(`${shot.name}: ${file}`);
    if (errors.length) {
      failures += errors.length;
      console.error(`${shot.name}: ${errors.length} console error(s)`);
      for (const e of errors) console.error(`  ${e}`);
    }
    await context.close();
  }
} finally {
  await browser.close();
  server.close();
}
process.exit(failures ? 1 : 0);
