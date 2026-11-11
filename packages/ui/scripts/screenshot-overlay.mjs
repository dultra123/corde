#!/usr/bin/env node
/**
 * The OBS overlay mode end to end, against a real corde-server running the simulator (debug
 * binary from `cargo build -p corde-server`, UI from `pnpm --filter @corde/ui build`, served by
 * the server itself with `--ui-dir`):
 *
 *   node scripts/screenshot-overlay.mjs        (≈ 2 min)
 *
 * 1. Opens the « OBS » page (`#/obs`) and reads, for every widget and overlay layout, the
 *    address and the advised size it offers. The server answer `GET /api/network` is replaced
 *    so that the page shows the usual port 8471 (the server of this run listens elsewhere).
 * 2. Captures every widget, and the default overlay layout, alone in a page of the advised size
 *    with a really transparent background (`omitBackground`), then checks in the picture that
 *    the four corners are fully transparent (alpha 0) and that something was drawn; checks
 *    that the page itself is transparent (html, body, #root), without cursor nor scroll bar.
 * 3. Composes three widgets (inputs, standings, tyres), with `bg=none` and `bg=glass`, over a
 *    dark and a light background drawn here (gradients only) → screenshots/overlay-dark.png
 *    and overlay-light.png, to check legibility by eye.
 * 4. `hide=offtrack` against a second server without game: the overlay must stay empty;
 *    reconnection: the server is stopped and restarted on the same port, frames must come back
 *    to the same page without a reload.
 * 5. Measures an overlay layout over SCREENSHOT_PERF_S seconds (default 60): frames received,
 *    CPU time of the page, JS heap after garbage collection, DOM nodes.
 *
 * Writes docs/images/obs-overlay.png (a scene: drawn background + the default overlay layout,
 * 1920 × 1080, whole) and docs/images/obs-page.png (the « OBS » page, whole), and fails on any
 * browser console error (but the expected « WebSocket connection failed » while the server is
 * down during the reconnection test).
 */
import { spawn } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const require = createRequire(import.meta.url);
const { chromium } = require("playwright");

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const repo = join(root, "..", "..");
const dist = join(root, "dist");
const outDir = join(root, "screenshots");
const docsImages = join(repo, "docs", "images");
const PERF_S = Number(process.env.SCREENSHOT_PERF_S ?? 45);
const bin = join(
  repo,
  "target",
  "debug",
  `corde-server${process.platform === "win32" ? ".exe" : ""}`,
);

if (!existsSync(join(dist, "index.html"))) {
  console.error("dist/index.html missing — run `pnpm --filter @corde/ui build` first");
  process.exit(1);
}
if (!existsSync(bin)) {
  console.error(`${bin} missing — run \`cargo build -p corde-server\` first`);
  process.exit(1);
}
mkdirSync(outDir, { recursive: true });

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

let failures = 0;
const check = (ok, what) => {
  console.log(`${ok ? "ok  " : "FAIL"} ${what}`);
  if (!ok) failures += 1;
};

/** Starts one server; `port` fixed for a restart on the same address. */
function startServer(name, args, port = 8400 + Math.floor(Math.random() * 500), dataDir = null) {
  const dir = dataDir ?? mkdtempSync(join(tmpdir(), `corde-overlay-${name}-`));
  const child = spawn(
    bin,
    ["--port", String(port), "--data-dir", dir, "--ui-dir", dist, "--log", "warn", ...args],
    { stdio: ["ignore", "ignore", "inherit"] },
  );
  const base = `http://127.0.0.1:${port}`;
  console.log(`server ${name}: ${base}`);
  return {
    name,
    base,
    port,
    dataDir: dir,
    child,
    async api(path) {
      const res = await fetch(`${base}${path}`);
      if (!res.ok) throw new Error(`${name} ${path}: HTTP ${res.status}`);
      return res.json();
    },
    async stop(keepData = false) {
      const exited = new Promise((r) => child.once("exit", r));
      child.kill("SIGTERM");
      await Promise.race([exited, sleep(5000)]);
      if (!keepData) rmSync(dir, { recursive: true, force: true });
    },
  };
}

async function waitFor(what, fn, timeoutMs = 60_000) {
  const deadline = Date.now() + timeoutMs;
  let last = null;
  while (Date.now() < deadline) {
    try {
      const v = await fn();
      if (v) return v;
    } catch (e) {
      last = String(e);
    }
    await sleep(500);
  }
  throw new Error(`timeout waiting for ${what}${last ? ` (last: ${last})` : ""}`);
}

/** Network answer shown by the OBS page: the usual port, LAN closed (nothing to hide). */
const FAKE_NETWORK = { lan_access_setting: false, listening_on_lan: false, port: 8471, urls: [] };

const SIM_ARGS = [
  "--source",
  "sim",
  "--sim-track-length",
  "1500",
  "--sim-lap-time",
  "30",
  "--sim-speed",
  "3",
  "--no-record",
  "--no-import",
];

const servers = [];
let browser = null;
const errors = [];
/** Console errors tolerated while the server is down on purpose. */
let downtime = false;

async function newPage(viewport, { transparent = false } = {}) {
  const context = await browser.newContext({
    viewport,
    deviceScaleFactor: 1,
    locale: "fr-FR",
  });
  const page = await context.newPage();
  const label = () => page.url().replace(/^https?:\/\/[^/]+/, "");
  page.on("console", (msg) => {
    if (msg.type() !== "error") return;
    const text = msg.text();
    if (downtime && /WebSocket connection to .* failed|ERR_CONNECTION_REFUSED/.test(text)) return;
    errors.push(`${label()}: ${text}`);
  });
  page.on("pageerror", (err) => errors.push(`${label()}: ${String(err)}`));
  await page.route("**/api/network", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify(FAKE_NETWORK),
    }),
  );
  return { context, page, transparent };
}

/** Decodes a PNG in the browser: size, alpha of the four corners, share of drawn pixels. */
async function inspectPng(page, png) {
  return page.evaluate(async (b64) => {
    const img = new Image();
    img.src = `data:image/png;base64,${b64}`;
    await img.decode();
    const c = document.createElement("canvas");
    c.width = img.naturalWidth;
    c.height = img.naturalHeight;
    const ctx = c.getContext("2d");
    ctx.drawImage(img, 0, 0);
    const { data } = ctx.getImageData(0, 0, c.width, c.height);
    const alpha = (x, y) => data[(y * c.width + x) * 4 + 3];
    const w = c.width - 1;
    const h = c.height - 1;
    let drawn = 0;
    for (let i = 3; i < data.length; i += 4) if (data[i] > 0) drawn += 1;
    return {
      width: c.width,
      height: c.height,
      corners: [alpha(0, 0), alpha(w, 0), alpha(0, h), alpha(w, h)],
      drawn: drawn / (c.width * c.height),
    };
  }, png.toString("base64"));
}

/**
 * Draws a background (gradients only, nothing taken from a game or a third party) and lays
 * transparent pictures over it, each at 1:1 → PNG buffer.
 */
async function compose(page, width, height, kind, items) {
  const b64 = await page.evaluate(
    async ({ width, height, kind, items }) => {
      const c = document.createElement("canvas");
      c.width = width;
      c.height = height;
      const ctx = c.getContext("2d");
      const lin = (y0, y1, stops) => {
        const g = ctx.createLinearGradient(0, y0, 0, y1);
        for (const [o, col] of stops) g.addColorStop(o, col);
        return g;
      };
      const horizon = height * 0.46;
      if (kind === "light") {
        // midday: bright sky, haze, green verge, light asphalt, red and white kerb
        ctx.fillStyle = lin(0, horizon, [
          [0, "#7cc3f5"],
          [0.7, "#cfe9fb"],
          [1, "#f4f8fb"],
        ]);
        ctx.fillRect(0, 0, width, horizon);
        const sun = ctx.createRadialGradient(
          width * 0.78,
          height * 0.12,
          0,
          width * 0.78,
          height * 0.12,
          height * 0.5,
        );
        sun.addColorStop(0, "rgba(255,255,255,1)");
        sun.addColorStop(0.15, "rgba(255,252,235,0.9)");
        sun.addColorStop(1, "rgba(255,255,255,0)");
        ctx.fillStyle = sun;
        ctx.fillRect(0, 0, width, horizon);
        ctx.fillStyle = lin(horizon, height, [
          [0, "#8fbf6a"],
          [1, "#5f9a45"],
        ]);
        ctx.fillRect(0, horizon, width, height - horizon);
        ctx.fillStyle = lin(horizon, height, [
          [0, "#c9ccd0"],
          [1, "#9a9ea4"],
        ]);
      } else {
        // night: deep sky, floodlights, dark asphalt
        ctx.fillStyle = lin(0, horizon, [
          [0, "#03050a"],
          [1, "#1b2436"],
        ]);
        ctx.fillRect(0, 0, width, horizon);
        for (const [x, r] of [
          [0.12, 0.35],
          [0.5, 0.28],
          [0.86, 0.4],
        ]) {
          const g = ctx.createRadialGradient(
            width * x,
            horizon * 0.8,
            0,
            width * x,
            horizon * 0.8,
            height * r,
          );
          g.addColorStop(0, "rgba(255,226,170,0.85)");
          g.addColorStop(0.2, "rgba(255,190,110,0.35)");
          g.addColorStop(1, "rgba(255,190,110,0)");
          ctx.fillStyle = g;
          ctx.fillRect(0, 0, width, height);
        }
        ctx.fillStyle = lin(horizon, height, [
          [0, "#0d1410"],
          [1, "#070b08"],
        ]);
        ctx.fillRect(0, horizon, width, height - horizon);
        ctx.fillStyle = lin(horizon, height, [
          [0, "#2a2d33"],
          [1, "#121418"],
        ]);
      }
      // the road: a trapezoid to the horizon, with a kerb on each side
      ctx.beginPath();
      ctx.moveTo(width * 0.44, horizon);
      ctx.lineTo(width * 0.56, horizon);
      ctx.lineTo(width * 1.05, height);
      ctx.lineTo(width * -0.05, height);
      ctx.closePath();
      ctx.fill();
      for (const side of [-1, 1]) {
        for (let i = 0; i < 14; i += 1) {
          const t0 = i / 14;
          const t1 = (i + 1) / 14;
          const y0 = horizon + (height - horizon) * t0 ** 1.6;
          const y1 = horizon + (height - horizon) * t1 ** 1.6;
          const x = (t) => width * (0.5 + side * (0.06 + 0.55 * t ** 1.6));
          const wd = (t) => width * (0.004 + 0.03 * t ** 1.6);
          ctx.fillStyle = i % 2 ? "#f2f2f2" : "#d23a2c";
          ctx.beginPath();
          ctx.moveTo(x(t0), y0);
          ctx.lineTo(x(t0) + side * wd(t0), y0);
          ctx.lineTo(x(t1) + side * wd(t1), y1);
          ctx.lineTo(x(t1), y1);
          ctx.closePath();
          ctx.fill();
        }
      }
      for (const it of items) {
        const img = new Image();
        img.src = `data:image/png;base64,${it.b64}`;
        await img.decode();
        ctx.drawImage(img, it.x, it.y);
      }
      return c.toDataURL("image/png").slice("data:image/png;base64,".length);
    },
    { width, height, kind, items },
  );
  return Buffer.from(b64, "base64");
}

/** Checks the overlay page itself: transparent from the root, no cursor, no scroll bar. */
async function checkBareOverlay(page, label) {
  const s = await page.evaluate(() => {
    const bg = (el) => (el ? getComputedStyle(el).backgroundColor : "none");
    const de = document.documentElement;
    return {
      html: bg(de),
      body: bg(document.body),
      root: bg(document.getElementById("root")),
      cursor: getComputedStyle(document.body).cursor,
      scroll: de.scrollHeight <= de.clientHeight && de.scrollWidth <= de.clientWidth,
      overflow: getComputedStyle(de).overflow,
      chrome: document.querySelectorAll(
        ".topbar, .navtabs, .race-banner, .reminder-banner, button, select, input, [title]",
      ).length,
    };
  });
  const clear = (c) => c === "rgba(0, 0, 0, 0)" || c === "transparent";
  check(
    clear(s.html) && clear(s.body) && clear(s.root),
    `${label}: html, body, #root transparent (${s.html} / ${s.body} / ${s.root})`,
  );
  check(s.cursor === "none", `${label}: no cursor`);
  check(s.scroll && s.overflow === "hidden", `${label}: no scroll bar`);
  check(s.chrome === 0, `${label}: no navigation, banner, control nor tooltip (${s.chrome})`);
}

try {
  const sim = startServer("A", SIM_ARGS);
  servers.push(sim);
  await waitFor("server A", () => sim.api("/api/health"), 20_000);
  browser = await chromium.launch();

  // ------------------------------------------------------------------ 1. the OBS page
  const obs = await newPage({ width: 1920, height: 1080 });
  await obs.page.goto(`${sim.base}/#/obs`, { waitUntil: "load" });
  await obs.page.waitForSelector(".obs-item");
  const items = await obs.page.$$eval(".obs-item", (cards) =>
    cards.map((c) => ({
      name: c.querySelector(".obs-item-name")?.textContent ?? "",
      url: c.querySelector(".obs-url")?.value ?? "",
      size: c.querySelector(".obs-size")?.textContent ?? "",
    })),
  );
  check(items.length >= 11, `OBS page lists ${items.length} items (10 widgets + overlay layouts)`);
  for (const it of items) {
    const ok = /^http:\/\/127\.0\.0\.1:8471\/#\/overlay\/(widget|layout)\/[a-z0-9-]+$/.test(it.url);
    check(ok, `address of « ${it.name} »: ${it.url} (${it.size})`);
  }
  const ipLeak = await obs.page.evaluate(() =>
    /\b(?:192\.168|10\.\d+|172\.(?:1[6-9]|2\d|3[01]))\.\d+\.\d+\b/.test(
      // what is on screen (texts and address fields), not the text of hidden nodes
      // biome-ignore lint/performance/useDomNodeTextContent: the rendered text is what a stream shows
      `${document.body.innerText} ${[...document.querySelectorAll("input")].map((i) => i.value).join(" ")}`,
    ),
  );
  check(!ipLeak, "OBS page: no network address shown by default");
  // the copy button
  await obs.context.grantPermissions(["clipboard-read", "clipboard-write"]);
  await obs.page.locator(".obs-item").first().getByRole("button", { name: "Copier" }).click();
  const copied = await obs.page.evaluate(() => navigator.clipboard.readText());
  check(copied === items[0]?.url, `« Copier » puts the address on the clipboard (${copied})`);
  // an option updates every address
  await obs.page.getByRole("button", { name: "Transparent" }).click();
  await obs.page.getByLabel("Masquer quand je ne suis pas en piste (fondu)").check();
  const withOpts = await obs.page.$$eval(".obs-url", (els) => els.map((e) => e.value));
  check(
    withOpts.every((u) => u.endsWith("?bg=none&hide=offtrack")),
    "options update every address (bg=none&hide=offtrack)",
  );
  await obs.page.getByRole("button", { name: "Valeurs par défaut" }).click();

  // Wait for a lap of the simulator so that the map knows the track, then the previews.
  await waitFor(
    "a lap of the simulator",
    async () => ((await sim.api("/api/laps")).length ?? 0) >= 1,
    60_000,
  );
  // the whole page in one picture: let every (lazy) preview load first
  const fullHeight = await obs.page.evaluate(() => document.documentElement.scrollHeight);
  await obs.page.setViewportSize({ width: 1920, height: fullHeight });
  await obs.page.reload({ waitUntil: "load" });
  await obs.page.waitForSelector(".obs-item");
  await obs.page.waitForTimeout(9000);
  const frames = obs.page.frames().filter((f) => f.url().includes("preview=1"));
  check(frames.length === items.length, `every preview loaded (${frames.length}/${items.length})`);
  await obs.page.screenshot({ path: join(docsImages, "obs-page.png"), fullPage: true });
  console.log(`obs page: ${join(docsImages, "obs-page.png")} (1920 × ${fullHeight})`);
  await obs.context.close();

  // ------------------------------------------------------------------ 2. every widget alone
  const tool = await newPage({ width: 400, height: 300 });
  await tool.page.goto(`${sim.base}/#/overlay/widget/none`, { waitUntil: "load" });
  const shots = {};
  for (const it of items) {
    const [w, h] = it.size.match(/\d+/g).map(Number);
    const target = it.url.replace("http://127.0.0.1:8471", sim.base);
    for (const bg of ["glass", "none"]) {
      const url = bg === "glass" ? target : `${target}?bg=none`;
      const p = await newPage({ width: w, height: h });
      await p.page.goto(url, { waitUntil: "load" });
      // the map draws the track after a whole lap seen by this page (10 s with the simulator)
      const slow = it.url.includes("/layout/") || it.url.endsWith("/map");
      await p.page.waitForTimeout(slow ? 12_000 : 3000);
      const png = await p.page.screenshot({ omitBackground: true });
      const info = await inspectPng(tool.page, png);
      const key = `${it.url.split("/").pop()}-${bg}`;
      shots[key] = { png, w, h };
      check(
        info.width === w && info.height === h && info.corners.every((a) => a === 0),
        `${key}: ${w} × ${h}, corners transparent (alpha ${info.corners.join(", ")})`,
      );
      check(
        info.drawn > 0.02,
        `${key}: widget drawn (${Math.round(info.drawn * 100)} % of pixels)`,
      );
      if (bg === "glass") await checkBareOverlay(p.page, key);
      writeFileSync(join(outDir, `overlay-${key}.png`), png);
      await p.context.close();
    }
  }

  // ------------------------------------------------------------------ 3. legibility sheets
  const trio = ["inputs", "standings", "tyres"];
  for (const kind of ["dark", "light"]) {
    const placed = [];
    let y = 40;
    for (const bg of ["none", "glass"]) {
      let x = 40;
      let rowH = 0;
      for (const id of trio) {
        const s = shots[`${id}-${bg}`];
        placed.push({ b64: s.png.toString("base64"), x, y });
        x += s.w + 40;
        rowH = Math.max(rowH, s.h);
      }
      y += rowH + 60;
    }
    const png = await compose(tool.page, 1920, 1080, kind, placed);
    writeFileSync(join(outDir, `overlay-${kind}.png`), png);
    console.log(`legibility: ${join(outDir, `overlay-${kind}.png`)}`);
  }
  // the scene of the README: the default overlay layout over the light background
  const layoutShot = shots["default-overlay-glass"];
  check(!!layoutShot, "default overlay layout captured");
  if (layoutShot) {
    const scene = await compose(tool.page, 1920, 1080, "light", [
      { b64: layoutShot.png.toString("base64"), x: 0, y: 0 },
    ]);
    writeFileSync(join(docsImages, "obs-overlay.png"), scene);
    console.log(`scene: ${join(docsImages, "obs-overlay.png")}`);
    const night = await compose(tool.page, 1920, 1080, "dark", [
      { b64: shots["default-overlay-none"].png.toString("base64"), x: 0, y: 0 },
    ]);
    writeFileSync(join(outDir, "overlay-scene-night-none.png"), night);
  }

  // ------------------------------------------------------------------ 4. hide=offtrack, reconnection
  const empty = startServer("B", ["--source", "lmu", "--no-import", "--no-game-api"]);
  servers.push(empty);
  await waitFor("server B", () => empty.api("/api/health"), 20_000);
  {
    const p = await newPage({ width: 640, height: 384 });
    await p.page.goto(`${empty.base}/#/overlay/widget/inputs?hide=offtrack`, { waitUntil: "load" });
    await p.page.waitForTimeout(3000);
    const info = await inspectPng(tool.page, await p.page.screenshot({ omitBackground: true }));
    const opacity = await p.page.$eval(".ov", (el) => getComputedStyle(el).opacity);
    check(
      info.drawn === 0 && opacity === "0",
      `hide=offtrack, no game: nothing shown (opacity ${opacity})`,
    );
    await p.page.goto(`${empty.base}/#/overlay/layout/default-overlay`, { waitUntil: "load" });
    await p.page.waitForTimeout(3000);
    const png2 = await p.page.screenshot({ omitBackground: true });
    writeFileSync(join(outDir, "overlay-no-game.png"), png2);
    const info2 = await inspectPng(tool.page, png2);
    check(
      info2.drawn === 0,
      `no game: an overlay without data shows no empty panel (${info2.drawn})`,
    );
    await p.context.close();
  }
  {
    const p = await newPage({ width: 640, height: 384 });
    await p.page.goto(`${sim.base}/#/overlay/widget/inputs?hide=offtrack`, { waitUntil: "load" });
    await p.page.waitForTimeout(3000);
    const opacity = await p.page.$eval(".ov", (el) => getComputedStyle(el).opacity);
    check(opacity === "1", `hide=offtrack, on track: shown (opacity ${opacity})`);
    let received = 0;
    p.page.on("websocket", (ws) => ws.on("framereceived", () => (received += 1)));
    downtime = true;
    await sim.stop(true);
    await p.page.waitForTimeout(2500);
    const again = startServer("A2", SIM_ARGS, sim.port, sim.dataDir);
    servers.push(again);
    await waitFor("server A restarted", () => again.api("/api/health"), 20_000);
    await waitFor("frames after the restart", async () => received > 20, 20_000);
    downtime = false;
    check(received > 20, `reconnected without reload (${received} messages after the restart)`);
    const banner = await p.page.evaluate(() => (document.body.textContent ?? "").trim());
    check(
      banner === "" || !/serveur|server|connexion|connection/i.test(banner),
      "no message shown",
    );
    await p.context.close();
  }

  // ------------------------------------------------------------------ 5. performance
  for (const bg of ["glass", "none"]) {
    const live = servers.at(-1);
    const p = await newPage({ width: 1920, height: 1080 });
    const cdp = await p.context.newCDPSession(p.page);
    await cdp.send("Performance.enable");
    let received = 0;
    p.page.on("websocket", (ws) => ws.on("framereceived", () => (received += 1)));
    await p.page.goto(`${live.base}/#/overlay/layout/default-overlay?bg=${bg}`, {
      waitUntil: "load",
    });
    await p.page.waitForTimeout(5000);
    const metric = async () => {
      await cdp.send("HeapProfiler.collectGarbage");
      const { metrics } = await cdp.send("Performance.getMetrics");
      const m = Object.fromEntries(metrics.map((x) => [x.name, x.value]));
      return { heap: m.JSHeapUsedSize, task: m.TaskDuration, nodes: m.Nodes, t: Date.now() };
    };
    const start = await metric();
    const r0 = received;
    const samples = [start];
    const step = Math.max(5, Math.round(PERF_S / 6));
    for (let s = 0; s < PERF_S; s += step) {
      await p.page.waitForTimeout(step * 1000);
      samples.push(await metric());
    }
    const end = samples.at(-1);
    const secs = (end.t - start.t) / 1000;
    const hz = (received - r0) / secs;
    const cpu = ((end.task - start.task) / secs) * 100;
    const mb = (v) => (v / 1048576).toFixed(1);
    console.log(
      `perf bg=${bg} over ${secs.toFixed(0)} s: ${hz.toFixed(1)} messages/s, page CPU ${cpu.toFixed(1)} % of a core, ` +
        `heap ${mb(start.heap)} → ${mb(end.heap)} MiB (samples ${samples.map((x) => mb(x.heap)).join(" / ")}), ` +
        `DOM nodes ${start.nodes} → ${end.nodes}`,
    );
    check(hz > 25 && hz < 36, `overlay receives ≈ 30 Hz (${hz.toFixed(1)})`);
    check(end.heap < start.heap * 1.25 + 2 * 1048576, "heap stays flat after garbage collection");
    check(end.nodes <= start.nodes * 1.1 + 50, "DOM does not grow");
    await p.context.close();
  }
  await tool.context.close();
} catch (e) {
  console.error(e);
  failures += 1;
} finally {
  if (browser) await browser.close();
  for (const s of servers) await s.stop().catch(() => {});
}

check(errors.length === 0, `no console error (${errors.length})`);
for (const e of errors) console.error(`     ${e}`);
process.exit(failures ? 1 : 0);
