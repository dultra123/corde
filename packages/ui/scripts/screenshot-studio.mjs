#!/usr/bin/env node
/**
 * Screenshots of the Studio (its tab in the top bar, the « Format » panel, a round dashboard)
 * against a real corde-server with the simulator source, which also serves `dist/`.
 *
 *   cargo build -p corde-server && pnpm --filter @corde/ui build
 *   node scripts/screenshot-studio.mjs --spawn
 *
 * `--spawn` starts the debug binary (`CORDE_SERVER_BIN` to name another one) on a free port, with
 * an empty data folder of its own; without it, `CORDE_SERVER=host:port` names a server that is
 * already running **and serves this `dist/`** (its dashboards `volant-rond`, `ecran-arrondi` and
 * `relais` are replaced). The three example dashboards are sent through `PUT /api/dashes/{id}`,
 * like the Studio does. Writes, at 1440 × 900 and twice the pixels:
 *
 *   screenshots/studio-onglet.png   the library under the top bar, the « Studio » tab current
 *   screenshots/studio-nouveau.png  the « Nouveau tableau de bord » dialog (model and format)
 *   screenshots/studio-format.png   the editor with the « Format » panel open (rounded corners)
 *   screenshots/studio-rond.png     a round dashboard in the editor, the outside hatched
 *   screenshots/studio-volant-moza.png   the model « Écran de volant MOZA » in the editor
 *   screenshots/studio-export-moza.png   what « Exporter pour le volant MOZA » says (ADR 0021)
 *
 * and checks what the docs say about a small extra screen: at `#/dash`, a 480 × 480 round
 * dashboard fills a 480 × 480 window exactly (black corners) and sits centred, black around, in
 * an 800 × 480 one (screenshots/studio-ecran-480.png, studio-ecran-800.png). Fails on any
 * browser console error or when a check does not hold.
 */
import { spawn } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, rmSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const require = createRequire(import.meta.url);
const { chromium } = require("playwright");

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const dist = join(root, "dist");
const outDir = join(root, "screenshots");
const SPAWN = process.argv.includes("--spawn");
let serverHost = process.env.CORDE_SERVER ?? "127.0.0.1:8471";

if (!existsSync(join(dist, "index.html"))) {
  console.error("dist/index.html missing — run `pnpm --filter @corde/ui build` first");
  process.exit(1);
}
mkdirSync(outDir, { recursive: true });

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

// ---------------------------------------------------------------- optional server
let child = null;
let dataDir = null;
if (SPAWN) {
  const bin =
    process.env.CORDE_SERVER_BIN ??
    join(
      root,
      "..",
      "..",
      "target",
      "debug",
      `corde-server${process.platform === "win32" ? ".exe" : ""}`,
    );
  if (!existsSync(bin)) {
    console.error(`${bin} missing — run \`cargo build -p corde-server\` first`);
    process.exit(1);
  }
  const port = 8400 + Math.floor(Math.random() * 500);
  serverHost = `127.0.0.1:${port}`;
  dataDir = mkdtempSync(join(tmpdir(), "corde-studio-shots-"));
  child = spawn(
    bin,
    [
      "--source",
      "sim",
      "--sim-seed",
      "7",
      "--port",
      String(port),
      "--bind",
      "127.0.0.1",
      "--data-dir",
      dataDir,
      "--ui-dir",
      dist,
      "--no-record",
      "--no-import",
      "--no-game-api",
    ],
    { stdio: ["ignore", "ignore", "inherit"] },
  );
  console.log(`server: spawned on ${serverHost} (data in ${dataDir})`);
}

const base = `http://${serverHost}`;

async function waitForServer() {
  for (let i = 0; i < 60; i += 1) {
    try {
      const res = await fetch(`${base}/api/status`);
      if (res.ok) return;
    } catch {
      // not up yet
    }
    await sleep(500);
  }
  throw new Error(`no server at ${base}`);
}

// ---------------------------------------------------------------- example dashboards
const THEME = {
  background: "#000000",
  panel: "#0d1116",
  line: "#1f2731",
  text: "#f2f5f8",
  dim: "#93a0ae",
  accent: "#ffb454",
};
const el = (id, type, x, y, w, h, props = {}) => ({ id, type, x, y, w, h, props });

/** A round wheel screen: everything placed inside the circle (24 × 24 cells, radius 12). */
const ROUND = {
  v: 1,
  id: "volant-rond",
  name: "Volant rond",
  format: { preset: "wheel-round", width: 480, height: 480, shape: "round" },
  theme: THEME,
  pages: [
    {
      id: "p1",
      name: "Course",
      cols: 24,
      rows: 24,
      elements: [
        el("e1", "rpm", 5, 3, 14, 2, { segments: 12 }),
        el("e2", "corde.gear", 8, 5, 8, 10),
        el("e3", "value", 2, 7, 6, 4, { field: "position" }),
        el("e4", "value", 2, 11, 6, 4, { field: "lap_best" }),
        el("e5", "value", 16, 7, 6, 4, { field: "fuel_l" }),
        el("e6", "value", 16, 11, 6, 4, { field: "gap_ahead" }),
        el("e7", "corde.delta", 6, 15, 12, 4),
        el("e8", "clock", 9, 19, 6, 3, { mode: "left" }),
      ],
    },
    {
      id: "p2",
      name: "Vitesse",
      cols: 24,
      rows: 24,
      elements: [
        el("e1", "arc", 5, 3, 14, 14, { field: "speed_kmh", max: 320 }),
        el("e2", "value", 8, 17, 8, 4, { field: "fuel_l" }),
      ],
    },
  ],
};

/** A small 5:3 screen with rounded corners and a page colour: the corners are cut out. */
const ROUNDED = {
  v: 1,
  id: "ecran-arrondi",
  name: "Écran arrondi",
  format: { preset: "custom", width: 800, height: 480, shape: "rounded", radius: 56 },
  theme: { ...THEME, accent: "#4ea1e6" },
  pages: [
    {
      id: "p1",
      name: "Relais",
      cols: 24,
      rows: 12,
      background: { color: "#10202e" },
      elements: [
        el("e1", "corde.rev", 1, 0, 22, 1, { flags: true }),
        el("e2", "corde.gear", 1, 1, 6, 8),
        el("e3", "corde.delta", 7, 1, 10, 3),
        el("e4", "value", 17, 1, 6, 3, { field: "position" }),
        el("e5", "value", 7, 4, 5, 3, { field: "lap_current" }),
        el("e6", "value", 12, 4, 5, 3, { field: "lap_best" }),
        el("e7", "value", 17, 4, 6, 3, { field: "gap_ahead" }),
        el("e8", "value", 7, 7, 5, 2, { field: "fuel_l" }),
        el("e9", "value", 12, 7, 5, 2, { field: "fuel_laps" }),
        el("e10", "value", 17, 7, 6, 2, { field: "session_left" }),
        el("e11", "value", 2, 9, 4, 3, { field: "tc" }),
        el("e12", "value", 6, 9, 4, 3, { field: "abs" }),
        el("e13", "value", 10, 9, 4, 3, { field: "brake_bias" }),
        el("e14", "corde.mini-tyres", 14, 9, 4, 3),
        el("e15", "clock", 18, 9, 4, 3, { mode: "left" }),
      ],
    },
  ],
};

/** A phone in landscape, as the dashboards made before the formats. */
const LANDSCAPE = {
  v: 1,
  id: "relais",
  name: "Relais de nuit",
  format: { preset: "phone-landscape", width: 852, height: 393 },
  theme: THEME,
  pages: [
    {
      id: "p1",
      name: "Relais",
      cols: 24,
      rows: 12,
      elements: [
        el("e1", "corde.rev", 0, 0, 24, 1, { flags: true }),
        el("e2", "corde.gear", 0, 1, 6, 8),
        el("e3", "corde.delta", 6, 1, 10, 3),
        el("e4", "value", 16, 1, 4, 3, { field: "position" }),
        el("e5", "value", 20, 1, 4, 3, { field: "laps_left" }),
        el("e6", "value", 6, 4, 5, 3, { field: "lap_current" }),
        el("e7", "value", 11, 4, 5, 3, { field: "lap_best" }),
        el("e8", "value", 16, 4, 4, 3, { field: "gap_ahead" }),
        el("e9", "value", 20, 4, 4, 3, { field: "gap_behind" }),
        el("e10", "value", 6, 7, 5, 2, { field: "fuel_l" }),
        el("e11", "value", 11, 7, 5, 2, { field: "fuel_laps" }),
        el("e12", "value", 16, 7, 8, 2, { field: "session_left" }),
        el("e13", "value", 0, 9, 4, 3, { field: "tc" }),
        el("e14", "value", 4, 9, 4, 3, { field: "abs" }),
        el("e15", "value", 8, 9, 5, 3, { field: "brake_bias" }),
        el("e16", "corde.mini-tyres", 13, 9, 5, 3),
        el("e17", "value", 18, 9, 6, 3, { field: "track_c" }),
      ],
    },
  ],
};

async function seed() {
  const now = Date.now();
  // The most recent first in the library: the round one, the rounded one, the landscape one.
  for (const [i, def] of [LANDSCAPE, ROUNDED, ROUND].entries()) {
    const res = await fetch(`${base}/api/dashes/${def.id}`, {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ ...def, updated_ms: now + i }),
    });
    if (!res.ok) throw new Error(`PUT ${def.id}: HTTP ${res.status} ${await res.text()}`);
    const stored = await res.json();
    if (JSON.stringify(stored.format) !== JSON.stringify(def.format)) {
      throw new Error(`${def.id}: the server changed the format: ${JSON.stringify(stored.format)}`);
    }
  }
}

// ---------------------------------------------------------------- shots
let failures = 0;
const fail = (msg) => {
  failures += 1;
  console.error(`FAILED: ${msg}`);
};

/** `?autonav=0`: the simulator is on track, the page must stay where it is put. */
const url = (hash) => `${base}/?autonav=0${hash}`;

async function open(browser, viewport, scale, hash, before) {
  const context = await browser.newContext({ viewport, deviceScaleFactor: scale, locale: "fr-FR" });
  const page = await context.newPage();
  page.on("console", (msg) => {
    if (msg.type() === "error") fail(`console: ${msg.text()}`);
  });
  page.on("pageerror", (err) => fail(`page error: ${err}`));
  if (before) await page.addInitScript(before);
  await page.goto(url(hash), { waitUntil: "load" });
  return { context, page };
}

async function shoot(page, name) {
  const file = join(outDir, `${name}.png`);
  await page.screenshot({ path: file });
  console.log(`${name}: ${file}`);
}

async function studioShots(browser) {
  const view = { width: 1440, height: 900 };

  // The library, under the top bar.
  let s = await open(browser, view, 2, "#/dash-studio");
  await s.page.locator(".studio-card").nth(3).waitFor();
  await s.page.waitForTimeout(2500);
  const tab = await s.page.locator(".navtab.is-current").textContent();
  if (tab !== "Studio") fail(`the current tab is « ${tab} », not « Studio »`);
  await shoot(s.page, "studio-onglet");
  // « Nouveau » : the model and the format, before anything is created.
  await s.page.getByRole("button", { name: "Nouveau", exact: true }).click();
  await s.page.locator("dialog.studio-new form").waitFor();
  await s.page.locator('dialog.studio-new [data-preset="wheel-round"]').click();
  await s.page.waitForTimeout(300);
  const create = await s.page.locator('dialog.studio-new button[type="submit"]').textContent();
  if (create !== "Créer · Rond 480 × 480") fail(`the create button reads « ${create} »`);
  await shoot(s.page, "studio-nouveau");
  await s.context.close();

  // The « Format » panel, on a page with rounded corners.
  s = await open(browser, view, 2, "#/dash-studio/ecran-arrondi");
  await s.page.locator(".studio-canvas .dstage").waitFor();
  await s.page.waitForTimeout(1500);
  await s.page.locator(".fmt-btn").click();
  await s.page.locator(".fmt-panel").waitFor();
  await s.page.mouse.move(40, 500);
  await s.page.waitForTimeout(400);
  await shoot(s.page, "studio-format");
  await s.context.close();

  // A round dashboard in the editor.
  s = await open(browser, view, 2, "#/dash-studio/volant-rond");
  await s.page.locator(".studio-canvas .dstage-mask").waitFor();
  await s.page.waitForTimeout(2500);
  const words = await s.page.locator(".fmt-btn-words").textContent();
  if (words !== "Rond 480 × 480") fail(`the format reads « ${words} »`);
  await shoot(s.page, "studio-rond");
  await s.context.close();

  // The screen of a MOZA wheel (ADR 0021): the model comes with its format, everything in it
  // is exported, and the export is a download made in the page.
  s = await open(browser, view, 2, "#/dash-studio");
  await s.page.getByRole("button", { name: "Nouveau", exact: true }).click();
  await s.page.locator("dialog.studio-new form").waitFor();
  await s.page.getByText("Écran de volant MOZA (KS Pro, CS Pro)", { exact: true }).click();
  const wheel = await s.page.locator('dialog.studio-new button[type="submit"]').textContent();
  if (wheel !== "Créer · 780 × 248") fail(`the create button reads « ${wheel} »`);
  await s.page.locator('dialog.studio-new button[type="submit"]').click();
  await s.page.locator(".studio-canvas .dstage").waitFor();
  await s.page.waitForTimeout(2500);
  await shoot(s.page, "studio-volant-moza");
  await s.page.goto(url("#/dash-studio"), { waitUntil: "load" });
  const exportBtn = s.page.getByRole("button", {
    name: "Exporter pour le volant MOZA",
    exact: true,
  });
  await exportBtn.waitFor();
  if ((await exportBtn.count()) !== 1) fail("the MOZA export is offered on more than one card");
  const [download] = await Promise.all([s.page.waitForEvent("download"), exportBtn.click()]);
  if (download.suggestedFilename() !== "Volant MOZA.mzdash") {
    fail(`the exported file is named « ${download.suggestedFilename()} »`);
  }
  const said = await s.page.locator("dialog.studio-moza p").first().textContent();
  const counts = /: (\d+) élément\(s\) écrit\(s\) sur (\d+)\.$/.exec(said ?? "");
  if (!counts || counts[1] !== counts[2] || Number(counts[1]) < 30) {
    fail(`the export says « ${said} »`);
  }
  if ((await s.page.locator("dialog.studio-moza .studio-moza-notes").count()) !== 0) {
    fail("the model for the wheel leaves something out");
  }
  await s.page.waitForTimeout(400);
  await shoot(s.page, "studio-export-moza");
  await s.context.close();
}

/** What the guide says about a small extra screen, checked on the pixels. */
async function screenChecks(browser) {
  const pick = () => {
    try {
      localStorage.setItem("corde.dashes.selected", "volant-rond");
    } catch {
      // the check below fails
    }
  };
  for (const [name, width, height, left] of [
    ["studio-ecran-480", 480, 480, 0],
    ["studio-ecran-800", 800, 480, 160],
  ]) {
    const s = await open(browser, { width, height }, 1, "#/dash", pick);
    await s.page.locator(".dash-page .dstage").first().waitFor();
    await s.page.waitForTimeout(1500);
    const box = await s.page.locator(".dash-page:not([inert]) .dstage").evaluate((n) => {
      const b = n.getBoundingClientRect();
      return [b.left, b.top, b.width, b.height];
    });
    if (JSON.stringify(box) !== JSON.stringify([left, 0, 480, 480])) {
      fail(`${name}: the page is drawn at ${JSON.stringify(box)}, not [${left},0,480,480]`);
    }
    const file = join(outDir, `${name}.png`);
    const png = await s.page.screenshot({ path: file });
    console.log(`${name}: ${file}`);
    // Outside the circle: black. Read in the page itself, from the screenshot.
    const points = [
      [left + 3, 3],
      [left + 476, 3],
      [left + 3, 476],
      [left + 476, 476],
      [2, 240],
      [width - 3, 240],
    ];
    const colours = await s.page.evaluate(
      async ({ data, points }) => {
        const img = new Image();
        img.src = `data:image/png;base64,${data}`;
        await img.decode();
        const c = document.createElement("canvas");
        c.width = img.width;
        c.height = img.height;
        const g = c.getContext("2d");
        g.drawImage(img, 0, 0);
        return points.map(([x, y]) => [...g.getImageData(x, y, 1, 1).data.slice(0, 3)]);
      },
      { data: png.toString("base64"), points: width === 480 ? points.slice(0, 4) : points },
    );
    for (const [i, c] of colours.entries()) {
      if (c.some((v) => v !== 0)) fail(`${name}: point ${points[i]} is ${c}, not black`);
    }
    await s.context.close();
  }
}

const browser = await chromium.launch();
try {
  await waitForServer();
  await seed();
  await studioShots(browser);
  await screenChecks(browser);
} catch (e) {
  fail(String(e));
} finally {
  await browser.close();
  if (child) child.kill();
  if (dataDir) rmSync(dataDir, { recursive: true, force: true });
}
process.exit(failures ? 1 : 0);
