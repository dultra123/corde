#!/usr/bin/env node
/**
 * Screenshots and end-to-end checks of the home server (« Corde serveur », `--role hub`, ADR 0010),
 * against two real corde-server processes started by this script (debug binary from
 * `cargo build -p corde-server`, UI from `pnpm --filter @corde/ui build`, served by the servers
 * with `--ui-dir`):
 *
 *   - « server »: `--role hub`, empty data folder;
 *   - « PC »: the simulator, which records a short session.
 *
 *   node scripts/screenshot-hub.mjs        (≈ 40 s)
 *
 * The phone (390 px, Chromium) opens the server and meets the pairing screen; the script reads a
 * code with `corde-server pair` (as the driver would, on the server), types it in the page, and
 * the app opens. The PC is paired through its own route (`POST /api/hub-link/pair`, what the
 * settings dialog calls) and sends its session; the server's home page and devices page are then
 * captured, and the PC's settings dialog with its « Serveur Corde » section.
 *
 * Writes screenshots/hub-pairing.png, hub-home.png, hub-devices.png, hub-pc-settings.png and
 * ../../docs/images/serveur.png (the four side by side). Two answers are adjusted for the pictures
 * (this machine has no home network): the address of the server in `GET /api/hub-link` becomes
 * `http://192.168.1.50:8471`, and the PC's live address in `GET /api/hub/status` becomes
 * `http://192.168.1.20:8471`. Device names are made up.
 *
 * Fails on: a browser console error once paired (before pairing, the 401 answers of the API are
 * expected and ignored); a pairing that does not open the app; the session not reaching the
 * server; horizontal overflow at 390 px; the device token showing anywhere in a page.
 */
import { spawn, spawnSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
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
const docImage = join(repo, "docs", "images", "serveur.png");
const bin = join(
  process.env.CARGO_TARGET_DIR ?? join(repo, "target"),
  "debug",
  `corde-server${process.platform === "win32" ? ".exe" : ""}`,
);
const HUB_SHOWN = "http://192.168.1.50:8471";
const PC_SHOWN = "http://192.168.1.20:8471";

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

function startServer(name, args) {
  const port = 8400 + Math.floor(Math.random() * 500);
  const dataDir = mkdtempSync(join(tmpdir(), `corde-hubshot-${name}-`));
  const child = spawn(
    bin,
    [
      "--port",
      String(port),
      "--bind",
      "127.0.0.1",
      "--data-dir",
      dataDir,
      "--ui-dir",
      dist,
      "--log",
      process.env.HUBSHOT_LOGS ? "info,tower_http=debug" : "warn",
      ...args,
    ],
    { stdio: ["ignore", "ignore", process.env.HUBSHOT_LOGS ? "inherit" : "ignore"] },
  );
  const base = `http://127.0.0.1:${port}`;
  console.log(`${name}: ${base} (data in ${dataDir})`);
  return {
    base,
    dataDir,
    async api(path, init) {
      const res = await fetch(`${base}${path}`, init);
      if (!res.ok) throw new Error(`${name} ${path}: HTTP ${res.status} ${await res.text()}`);
      return res.status === 204 ? null : res.json();
    },
    async stop() {
      child.kill("SIGTERM");
      await sleep(500);
      rmSync(dataDir, { recursive: true, force: true });
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

/** A pairing code, read on the « server » as the driver would (`sudo corde-server pair`). */
function pairingCode(hub) {
  const r = spawnSync(bin, ["pair", "--data-dir", hub.dataDir], { encoding: "utf8" });
  if (r.status !== 0) throw new Error(`corde-server pair: ${r.stderr}`);
  const m = /(\d{4}) (\d{4})/.exec(r.stdout);
  if (!m) throw new Error(`no code in: ${r.stdout}`);
  return `${m[1]}${m[2]}`;
}

const servers = [];
let browser = null;
try {
  const hub = startServer("server", ["--role", "hub"]);
  servers.push(hub);
  const pc = startServer("pc", [
    "--source",
    "sim",
    "--sim-track-length",
    "1500",
    "--sim-lap-time",
    "30",
    "--sim-speed",
    "3",
    "--record-rate",
    "10",
    "--no-import",
    "--no-game-api",
  ]);
  servers.push(pc);
  await waitFor("server", async () => {
    const r = await fetch(`${hub.base}/api/hub/pairing`);
    return r.ok;
  });
  await waitFor("PC recording", async () => (await pc.api("/api/recording")).recording);

  browser = await chromium.launch();
  const errors = [];
  let paired = false;
  const phone = await browser.newContext({
    viewport: { width: 390, height: 844 },
    deviceScaleFactor: 2,
    isMobile: true,
    hasTouch: true,
    locale: "fr-FR",
    userAgent:
      "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1",
  });
  const page = await phone.newPage();
  page.on("console", (msg) => {
    if (msg.type() !== "error") return;
    // Before pairing, every API call and the live socket are refused (401), and the wrong code
    // typed on purpose is refused (400): expected.
    if (!paired && /40[01]|WebSocket/i.test(msg.text())) return;
    errors.push(msg.text());
  });
  page.on("pageerror", (err) => errors.push(String(err)));
  await page.route("**/api/hub/status", async (route) => {
    const response = await route.fetch();
    const json = await response.json();
    if (json.last_pc) json.last_pc.urls = [PC_SHOWN];
    await route.fulfill({ response, json });
  });

  const settle = async (p) => {
    await p.evaluate(() => document.fonts.ready);
    await p.waitForTimeout(600);
  };
  const overflow = (p) =>
    p.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);

  // ---- the pairing screen
  await page.goto(`${hub.base}/`, { waitUntil: "load" });
  await page.waitForSelector(".hub-door", { timeout: 20_000 });
  check(
    (await page.inputValue(".hub-door input:not(.hub-code-input)")) === "iPhone",
    "the device name is guessed from the browser",
  );
  await page.fill(".hub-code-input", "1234");
  await settle(page);
  await page.screenshot({ path: join(outDir, "hub-pairing.png") });
  check((await overflow(page)) <= 0, "pairing screen: no horizontal overflow");

  // A wrong code: a clear message, the screen stays.
  const code = pairingCode(hub);
  const wrong = code === "00000000" ? "11111111" : "00000000";
  await page.fill(".hub-code-input", wrong);
  await page.click(".hub-door button[type=submit]");
  await page.waitForSelector(".hub-door .form-error", { timeout: 10_000 });
  const msg = await page.textContent(".hub-door .form-error");
  check(/Code incorrect/.test(msg ?? ""), `wrong code → « ${msg?.slice(0, 60)}… »`);

  // The right one: the app opens.
  await page.fill(".hub-code-input", code);
  await page.click(".hub-door button[type=submit]");
  await page.waitForSelector(".hub-home", { timeout: 15_000 });
  paired = true;
  check(true, "the right code opens the app");
  const cookies = await phone.cookies();
  const device = cookies.find((c) => c.name === "corde_device");
  check(
    !!device && device.httpOnly && device.sameSite === "Strict" && !device.secure,
    "cookie corde_device: HttpOnly, SameSite=Strict, not Secure",
  );
  const token = device?.value ?? "";
  const leaks = await page.evaluate((t) => document.documentElement.outerHTML.includes(t), token);
  check(token.length === 64 && !leaks, "the token never shows in the page");
  check(
    (await page.evaluate(() => document.cookie)).indexOf("corde_device") < 0,
    "…nor in document.cookie (HttpOnly)",
  );

  // ---- the PC sends its session
  await sleep(2500);
  await pc.api("/api/recording/stop", { method: "POST" });
  await waitFor("PC session closed", async () => !(await pc.api("/api/recording")).recording);
  const code2 = pairingCode(hub);
  const link = await pc.api("/api/hub-link/pair", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ url: hub.base, code: code2, name: "PC de jeu" }),
  });
  check(link.paired === true && !JSON.stringify(link).includes(code2), "PC paired");
  await waitFor("the session on the server", async () => {
    const s = await pc.api("/api/hub-link");
    return s.last_sync_ms && s.pending_sessions === 0 ? s : null;
  });

  // ---- the server's home page
  await page.goto(`${hub.base}/#/home`, { waitUntil: "load" });
  await page.reload({ waitUntil: "load" });
  await page.waitForSelector(".hub-session", { timeout: 20_000 }).catch(async (e) => {
    await page.screenshot({ path: join(outDir, "hub-home-failed.png") });
    console.error(JSON.stringify(await pc.api("/api/hub-link")));
    throw e;
  });
  await page.waitForSelector(".hub-sync .home-when", { timeout: 20_000 });
  await settle(page);
  await page.screenshot({ path: join(outDir, "hub-home.png") });
  check((await overflow(page)) <= 0, "server home: no horizontal overflow");
  const liveHref = await page.getAttribute(".hub-live a.home-btn", "href");
  check(liveHref === `${PC_SHOWN}/#/`, "« Direct » links to the PC's live view");

  // ---- the devices page
  await page.goto(`${hub.base}/#/devices`, { waitUntil: "load" });
  await page.waitForSelector(".hub-device.is-current", { timeout: 20_000 });
  const names = await page.$$eval(".hub-device-name b", (els) => els.map((e) => e.textContent));
  check(names.includes("iPhone") && names.includes("PC de jeu"), `devices: ${names.join(", ")}`);
  await settle(page);
  await page.screenshot({ path: join(outDir, "hub-devices.png") });
  check((await overflow(page)) <= 0, "devices: no horizontal overflow");

  // « Direct » tab on the server: an explanation, no dashboard.
  await page.goto(`${hub.base}/#/`, { waitUntil: "load" });
  await page.waitForSelector(".hub-direct", { timeout: 10_000 });
  check((await page.$$(".grid-stack-item")).length === 0, "no live dashboard on the server");

  // ---- the PC's settings: « Serveur Corde »
  const desk = await browser.newContext({
    viewport: { width: 1280, height: 1000 },
    deviceScaleFactor: 2,
    locale: "fr-FR",
  });
  const pcPage = await desk.newPage();
  pcPage.on("console", (m) => {
    if (m.type() === "error") errors.push(`PC: ${m.text()}`);
  });
  await pcPage.route("**/api/hub-link", async (route) => {
    if (route.request().method() !== "GET") return route.continue();
    const response = await route.fetch();
    const json = await response.json();
    json.url = HUB_SHOWN;
    await route.fulfill({ response, json });
  });
  await pcPage.goto(`${pc.base}/#/sessions`, { waitUntil: "load" });
  await pcPage.click(".topbar-right button.btn.is-icon:last-child");
  await pcPage.waitForSelector(".hub-link-state", { timeout: 15_000 });
  await pcPage.locator(".hub-link").scrollIntoViewIfNeeded();
  await settle(pcPage);
  const state = await pcPage.textContent(".hub-link-state b");
  check(state === "Connecté", `PC settings: « ${state} »`);
  const dialog = pcPage.locator("dialog.sheet[open]");
  const box = await pcPage.locator(".sheet-section", { hasText: "Serveur Corde" }).boundingBox();
  const end = await pcPage.locator(".hub-link").boundingBox();
  const dbox = await dialog.boundingBox();
  if (box && end && dbox) {
    await pcPage.screenshot({
      path: join(outDir, "hub-pc-settings.png"),
      clip: {
        x: dbox.x,
        y: Math.max(dbox.y, box.y - 12),
        width: dbox.width,
        height: end.y + end.height - Math.max(dbox.y, box.y - 12) + 16,
      },
    });
  }
  await desk.close();

  check(errors.length === 0, "no console error once paired");
  for (const e of errors) console.error(`     ${e}`);

  // ---- docs/images/serveur.png: the four side by side
  const img = (f) => `data:image/png;base64,${readFileSync(join(outDir, f)).toString("base64")}`;
  const compose = await browser.newContext({
    viewport: { width: 1900, height: 900 },
    deviceScaleFactor: 1,
  });
  const cp = await compose.newPage();
  await cp.setContent(`<!doctype html><html><body style="margin:0;background:#0b0e12">
    <div id="c" style="display:flex;gap:28px;align-items:flex-start;padding:28px">
      <img src="${img("hub-pairing.png")}" style="width:390px;border-radius:18px;border:1px solid #344050">
      <img src="${img("hub-home.png")}" style="width:390px;border-radius:18px;border:1px solid #344050">
      <img src="${img("hub-devices.png")}" style="width:390px;border-radius:18px;border:1px solid #344050">
      <img src="${img("hub-pc-settings.png")}" style="width:560px;border-radius:8px;border:1px solid #344050">
    </div></body></html>`);
  await cp.waitForTimeout(300);
  await cp.locator("#c").screenshot({ path: docImage });
  console.log(`composite: ${docImage}`);
  await compose.close();
} catch (e) {
  console.error(e);
  failures += 1;
} finally {
  if (browser) await browser.close();
  for (const s of servers) await s.stop();
}
if (failures) {
  console.error(`${failures} check(s) failed`);
  process.exit(1);
}
console.log("all checks passed");
