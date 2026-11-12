#!/usr/bin/env node
/**
 * Screenshots and end-to-end checks of the home page (`#/home`), against two real corde-server
 * processes started by this script (debug binary from `cargo build -p corde-server`, UI from
 * `pnpm --filter @corde/ui build`, served by the servers themselves with `--ui-dir`):
 *
 *   A. simulator, short laps, results folder `reference/samples`: the driver is on track, a
 *      session is being recorded, two game results are imported;
 *   B. `--source lmu` with an empty data folder: what a first launch looks like — no game, no
 *      session, no result.
 *
 *   node scripts/screenshot-home.mjs        (≈ 1 min: the simulator must complete a few laps)
 *
 * Writes screenshots/home.png (1920×1080, server A; copied to ../../docs/images/accueil.png),
 * home-mobile.png (390 wide, whole page, server A), home-empty.png (1920×1080, server B) and
 * one picture of the carousel per slide (home-live-<n>.png, home-mobile-live-<n>.png). Two answers of server A are adjusted for
 * the pictures, because this machine cannot produce them:
 *   - `GET /api/network` says the LAN is open at http://192.168.1.20:8471 (the sandbox has no
 *     home network), so the phone card shows its address and QR code;
 *   - in `GET /api/results?limit=5`, the sample race is given the start time of the simulator
 *     session, so the "last session" card shows a classification like after a real race.
 *
 * The online schedule is simulated as in screenshot-races.mjs (sample pages of
 * `scripts/race-pages.mjs`, start times computed from the time of the capture, pictures served
 * by the script — one of them deliberately unreadable), and one start is picked on server A, so
 * the « Courses en ligne » carousel under the last session has slides; server B keeps the
 * schedule off. Checks: the carousel is there, right under the last session (both widths), with
 * 2 to 8 slides, the pick first; it turns to the next slide by itself, holds still under the
 * pointer, follows the arrow keys; pictures are `https://`, lazy, without referrer, and a picture
 * that fails leaves the drawn stand-in (no broken image); moving to the Courses page does not ask
 * the server for the schedule again; with the schedule off it says so and the server reads nothing.
 *
 * Also checks, and fails on: any browser console error; an app opened without a page while the
 * driver is on track must jump to the dashboard, and stay on the home page otherwise; a page
 * opened on `#/home` must not move; the "access from my phone" button must write
 * `settings.json` and ask for a restart; the web manifest and its icons must be served.
 *
 * Two more scenarios need what this machine does not have, so the script plays the missing part:
 *   - a game that goes from the garage to the track and back: the page's WebSocket is answered
 *     by the script (real frames of server A, with `in_realtime` switched), REST stays real —
 *     checks the automatic navigation end to end and writes home-garage.png;
 *   - the desktop shell: a fake `window.__TAURI__.core.invoke` records the commands sent by the
 *     settings dialog and the restart button (the contract of `src/lib/desktop.ts`).
 *
 *   SCREENSHOT_MIN_LAPS=5         laps to wait for on server A
 *   SCREENSHOT_TIMEOUT_MS=120000  how long to wait for them
 */
import { spawn } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { lmuToolsPage, raceControlPage } from "./race-pages.mjs";

const require = createRequire(import.meta.url);
const { chromium } = require("playwright");

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const repo = join(root, "..", "..");
const dist = join(root, "dist");
const outDir = join(root, "screenshots");
const docImage = join(repo, "docs", "images", "accueil.png");
const TZ = "Europe/Zurich";
const MIN_LAPS = Number(process.env.SCREENSHOT_MIN_LAPS ?? 5);
const TIMEOUT_MS = Number(process.env.SCREENSHOT_TIMEOUT_MS ?? 120_000);
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
const MIN = 60e3;
const ceilTo = (ms, step) => Math.ceil(ms / step) * step;

/**
 * A stand-in for a race picture (no real photo here): a dark track-like scene, drawn as SVG and
 * served at the `https://img.example/…` addresses of the sample page.
 */
function pictureSvg(hue) {
  return `<svg xmlns="http://www.w3.org/2000/svg" width="1200" height="600" viewBox="0 0 1200 600">
    <defs>
      <linearGradient id="sky" x1="0" y1="0" x2="0" y2="1">
        <stop offset="0" stop-color="hsl(${hue} 45% 34%)"/><stop offset="0.55" stop-color="hsl(${hue} 30% 16%)"/>
        <stop offset="1" stop-color="#0b0e12"/>
      </linearGradient>
      <radialGradient id="sun" cx="0.78" cy="0.3" r="0.5">
        <stop offset="0" stop-color="hsl(${hue + 30} 90% 70% / 0.55)"/><stop offset="1" stop-color="transparent"/>
      </radialGradient>
    </defs>
    <rect width="1200" height="600" fill="url(#sky)"/><rect width="1200" height="600" fill="url(#sun)"/>
    <path d="M0 420 C 260 360 520 380 760 330 S 1100 280 1200 300 L1200 600 L0 600Z" fill="#14181e"/>
    <path d="M-40 560 C 300 470 600 470 900 420 S 1180 380 1260 372" stroke="#2a3038" stroke-width="70" fill="none"/>
    <path d="M-40 560 C 300 470 600 470 900 420 S 1180 380 1260 372" stroke="#e9edf2" stroke-opacity="0.5" stroke-width="3" stroke-dasharray="28 22" fill="none"/>
    <path d="M0 600 L0 520 L60 510 L120 600Z" fill="#d23a33"/><path d="M60 510 L120 600 L180 600 L110 502Z" fill="#f2f2f2"/>
  </svg>`;
}

/** Starts one server on a free-ish port; no `--bind`: the default must be this PC only. */
function startServer(name, args) {
  const port = 8400 + Math.floor(Math.random() * 500);
  const dataDir = mkdtempSync(join(tmpdir(), `corde-home-${name}-`));
  const child = spawn(
    bin,
    ["--port", String(port), "--data-dir", dataDir, "--ui-dir", dist, "--log", "warn", ...args],
    { stdio: ["ignore", "ignore", "inherit"] },
  );
  const base = `http://127.0.0.1:${port}`;
  console.log(`server ${name}: ${base} (data in ${dataDir})`);
  return {
    name,
    base,
    dataDir,
    child,
    async api(path, init) {
      const res = await fetch(`${base}${path}`, init);
      if (!res.ok) throw new Error(`${name} ${path}: HTTP ${res.status}`);
      return res.json();
    },
    async stop() {
      child.kill("SIGTERM");
      await sleep(400);
      rmSync(dataDir, { recursive: true, force: true });
    },
  };
}

async function waitFor(what, fn, timeoutMs = TIMEOUT_MS) {
  const deadline = Date.now() + timeoutMs;
  let last = null;
  while (Date.now() < deadline) {
    try {
      const v = await fn();
      if (v) return v;
    } catch (e) {
      last = String(e);
    }
    await sleep(1000);
  }
  throw new Error(`timeout waiting for ${what}${last ? ` (last: ${last})` : ""}`);
}

let failures = 0;
const check = (ok, what) => {
  console.log(`${ok ? "ok  " : "FAIL"} ${what}`);
  if (!ok) failures += 1;
};

const servers = [];
let browser = null;
try {
  const sim = startServer("A", [
    "--source",
    "sim",
    "--sim-track-length",
    "1500",
    "--sim-lap-time",
    "30",
    "--sim-speed",
    "3",
    "--record-rate",
    "20",
    "--lmu-results-dir",
    join(repo, "reference", "samples"),
  ]);
  servers.push(sim);
  const empty = startServer("B", ["--source", "lmu"]);
  servers.push(empty);

  // ---- what the servers say before any page is opened
  const netB = await waitFor("server B", () => empty.api("/api/network"), 20_000);
  check(
    netB.listening_on_lan === false && netB.urls.length === 0 && !netB.lan_access_setting,
    "default: the server listens on this PC only",
  );
  console.log(`waiting for a session with ${MIN_LAPS} laps on server A…`);
  const session = await waitFor("laps", async () => {
    const rows = await sim.api("/api/sessions?limit=1");
    return rows[0] && rows[0].lap_count >= MIN_LAPS ? rows[0] : null;
  });
  await waitFor(
    "imported results",
    async () => (await sim.api("/api/profile")).totals.sessions >= 2,
    30_000,
  );
  console.log(`session ${session.id}: ${session.lap_count} laps`);

  const manifest = await fetch(`${sim.base}/manifest.webmanifest`);
  const manifestJson = manifest.ok ? await manifest.json() : null;
  check(
    manifest.ok && manifestJson?.name === "Corde" && manifestJson?.display === "standalone",
    `web manifest served (${manifest.headers.get("content-type")})`,
  );
  for (const icon of manifestJson?.icons ?? []) {
    const res = await fetch(`${sim.base}/${icon.src}`);
    check(res.ok, `icon ${icon.src} served`);
  }

  browser = await chromium.launch();

  /** Opens a page and collects its console errors. */
  async function open(url, { mobile = false, prepare } = {}) {
    const context = await browser.newContext({
      viewport: mobile ? { width: 390, height: 844 } : { width: 1920, height: 1080 },
      deviceScaleFactor: mobile ? 2 : 1,
      isMobile: mobile,
      hasTouch: mobile,
      locale: "fr-FR",
      timezoneId: TZ,
    });
    const page = await context.newPage();
    const errors = [];
    page.on("console", (msg) => {
      if (msg.type() === "error") errors.push(msg.text());
    });
    page.on("pageerror", (err) => errors.push(String(err)));
    if (prepare) await prepare(page);
    await page.goto(url, { waitUntil: "load" });
    return {
      page,
      async close(label) {
        check(errors.length === 0, `${label}: no console error`);
        for (const e of errors) console.error(`     ${e}`);
        await context.close();
      },
    };
  }

  // one start picked on server A: the next LMP3 Fixed of the sample page (« Mes courses »)
  {
    const start = ceilTo(Date.now() + 9 * MIN, 5 * MIN);
    const pick = {
      id: `lmp3-fixed-beginner@${start}`,
      race_id: "lmp3-fixed-beginner",
      cat: "daily",
      src: "rc",
      name: "LMP3 Fixed",
      level: "beginner",
      track: "Monza",
      duration: "32 min",
      image: null,
      start_ms: start,
    };
    await sim.api(`/api/races/picks/${encodeURIComponent(pick.id)}`, {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(pick),
    });
  }

  /** `GET /api/races/sources` answered with the sample pages; pictures served here. */
  let sourcesServed = 0;
  const scheduleData = async (page) => {
    await page.route("**/api/races/sources**", (route) => {
      sourcesServed += 1;
      const now = Date.now();
      const one = (id, u, html) => ({
        id,
        url: u,
        state: "ok",
        fetched_at_ms: now,
        attempted_at_ms: now,
        error: null,
        html,
      });
      return route.fulfill({
        json: {
          enabled: true,
          sources: [
            one("racecontrol", "https://racecontrol.gg/", raceControlPage(now, { images: true })),
            one("lmutools", "https://lmu.tools/schedule", lmuToolsPage(now)),
          ],
        },
      });
    });
    await page.route("https://img.example/**", (route) => {
      const url = route.request().url();
      const headers = route.request().headers();
      if (headers.referer) check(false, `a picture was asked with a referrer: ${url}`);
      if (url.includes("missing-picture")) {
        // answers, but not with a picture: the page must fall back to its drawn stand-in
        return route.fulfill({ status: 200, contentType: "image/webp", body: "not a picture" });
      }
      const hue = url.includes("lmp3") ? 205 : 25;
      return route.fulfill({ contentType: "image/svg+xml", body: pictureSvg(hue) });
    });
  };

  /** The two adjusted answers (see the header), and the schedule. */
  const pictureData = async (page) => {
    await scheduleData(page);
    await page.route("**/api/network", (route) =>
      route.fulfill({
        json: {
          lan_access_setting: true,
          listening_on_lan: true,
          port: 8471,
          urls: ["http://192.168.1.20:8471"],
        },
      }),
    );
    await page.route("**/api/results?limit=5", async (route) => {
      const response = await route.fetch();
      const rows = await response.json();
      for (const r of rows) {
        if (r.session_kind === session.session_type && r.track_course === session.track_name) {
          r.started_wall_ms = session.started_wall_ms - 40_000;
        }
      }
      await route.fulfill({ response, json: rows });
    });
  };

  /** The « Courses en ligne » carousel of server A (see the header). */
  async function checkCarousel(page, name, mobile) {
    const live = page.locator(".home-live");
    const slides = await live.locator(".live-slide").count();
    check(slides >= 2 && slides <= 8, `${name}: carousel with ${slides} slides (2 to 8)`);
    const box = async (sel) => page.locator(sel).first().boundingBox();
    const last = await box(".home-last");
    const card = await box(".home-live");
    const races = await box(".home-races-card");
    check(
      !!last &&
        !!card &&
        Math.abs(card.x - last.x) < 2 &&
        card.y > last.y + last.height &&
        card.y - (last.y + last.height) < 30 &&
        (!mobile || (!!races && races.y > card.y)),
      `${name}: the carousel sits right under the last session`,
    );
    const label = () => live.locator(".live-slide.is-current").getAttribute("aria-label");
    const first = (await live.locator(".live-slide.is-current").textContent()) ?? "";
    check(
      /Choisie/.test(first) && /LMP3 Fixed/.test(first) && /dans \d+ (min|s)/.test(first),
      `${name}: the driver's pick comes first, with its countdown (${first.replace(/\s+/g, " ").slice(0, 90)}…)`,
    );
    check(
      (await live.getAttribute("aria-roledescription")) === "carrousel" &&
        (await label()) === `1 sur ${slides}`,
      `${name}: carousel roles and labels (« 1 sur ${slides} »)`,
    );
    const imgs = await live.locator(".live-art img").evaluateAll((els) =>
      els.map((e) => ({
        src: e.getAttribute("src"),
        loading: e.getAttribute("loading"),
        referrer: e.getAttribute("referrerpolicy"),
      })),
    );
    check(
      imgs.length > 0 &&
        imgs.every(
          (i) =>
            i.src?.startsWith("https://") && i.loading === "lazy" && i.referrer === "no-referrer",
        ),
      `${name}: ${imgs.length} pictures, https, lazy, no referrer`,
    );
    // every slide must have been seen for its picture to load: walk through them (one picture
    // of the card per slide: screenshots/<name>-live-<n>.png)
    for (let i = 0; i < slides; i++) {
      await live.locator(".live-dot").nth(i).click();
      await page.waitForTimeout(500);
      await live.screenshot({ path: join(outDir, `${name}-live-${i + 1}.png`) });
    }
    await page.waitForTimeout(800);
    const broken = await live
      .locator(".live-art img")
      .evaluateAll((els) => els.filter((e) => e.complete && e.naturalWidth === 0).length);
    const proto = live.locator(".live-slide", { hasText: "Prototype Fixed" });
    check(
      broken === 0 &&
        (await proto.locator("img").count()) === 0 &&
        (await proto.locator(".live-art-mark").count()) === 1,
      `${name}: a picture that fails leaves the drawn stand-in (broken: ${broken})`,
    );
    if (mobile) return;
    // turns by itself every 8 s…
    await live.locator(".live-dot").nth(0).click();
    await page.mouse.move(0, 0);
    await page.locator("body").evaluate((b) => b.focus());
    await page.evaluate(() => document.activeElement?.blur?.());
    const t0 = Date.now();
    await page.waitForFunction(
      () =>
        document.querySelector(".home-live .live-slide.is-current")?.getAttribute("aria-label") ===
        `2 sur ${document.querySelectorAll(".home-live .live-slide").length}`,
      null,
      { timeout: 12_000 },
    );
    const waited = ((Date.now() - t0) / 1000).toFixed(1);
    check(true, `${name}: turns to the next slide by itself (after ${waited} s)`);
    // …but not under the pointer
    await page.hover(".home-live .live-viewport");
    const held = await label();
    await page.waitForTimeout(9_500);
    check((await label()) === held, `${name}: holds still under the pointer (${held})`);
    await page.mouse.move(0, 0);
    // arrow keys
    await live.locator(".live-dot").nth(0).click();
    await page.keyboard.press("ArrowRight");
    check((await label()) === `2 sur ${slides}`, `${name}: → shows the next slide`);
    await page.keyboard.press("ArrowLeft");
    await page.keyboard.press("ArrowLeft");
    check((await label()) === `${slides} sur ${slides}`, `${name}: ← wraps to the last slide`);
    // the Courses page uses the same copy of the schedule
    const before = sourcesServed;
    await page.click('.navtab[href="#/races"]');
    await page.waitForSelector(".race-card", { timeout: 15_000 });
    check(
      sourcesServed === before,
      `${name}: the Courses page does not ask for the schedule again`,
    );
  }

  async function settle(page) {
    await page.evaluate(() => document.fonts.ready);
    await page.waitForFunction(() => !document.querySelector(".spinner"), null, {
      timeout: 30_000,
    });
    await page.waitForTimeout(700);
  }

  // ---- screenshots: server A (on track, recording, results, LAN open)
  for (const mobile of [false, true]) {
    const name = mobile ? "home-mobile" : "home";
    const { page, close } = await open(`${sim.base}/#/home`, { mobile, prepare: pictureData });
    await page.waitForSelector(".home-status.is-track", { timeout: 30_000 });
    await page.waitForSelector(".home-last .laptimes-plot canvas", { timeout: 30_000 });
    await page.waitForSelector(".home-race-link", { timeout: 30_000 });
    await page.waitForSelector(".home-phone .qr", { timeout: 30_000 });
    await page.waitForSelector(".home-result", { timeout: 30_000 });
    await page.waitForSelector(".home-rec.is-live", { timeout: 30_000 });
    await page.waitForSelector(".home-live .live-slide.is-current", { timeout: 30_000 });
    await settle(page);
    // the carousel rests on its first slide (the pick) for the picture
    await page.hover(".home-live .live-viewport");
    await page.click(".home-live .live-dot >> nth=0");
    await page.mouse.move(0, 0);
    await page.waitForTimeout(600);
    const file = join(outDir, `${name}.png`);
    await page.screenshot({ path: file, fullPage: mobile });
    console.log(`${name}: ${file}`);
    if (!mobile) copyFileSync(file, docImage);
    check(new URL(page.url()).hash === "#/home", `${name}: a page opened on #/home stays there`);
    const overflow = await page.evaluate(
      () => document.documentElement.scrollWidth - document.documentElement.clientWidth,
    );
    check(overflow <= 0, `${name}: no horizontal overflow (${overflow}px)`);
    await checkCarousel(page, name, mobile);
    await close(name);
  }

  // ---- screenshot: server B (first launch)
  {
    let asked = 0;
    const { page, close } = await open(`${empty.base}/#/home`, {
      prepare: (p) =>
        p.route("**/api/races/sources**", (route) => {
          asked += 1;
          return route.continue();
        }),
    });
    await page.waitForSelector(".home-live", { timeout: 30_000 });
    await page.waitForFunction(
      () => /désactivé/.test(document.querySelector(".home-live")?.textContent ?? ""),
      null,
      { timeout: 15_000 },
    );
    check(
      (await page.getAttribute(".home-live .home-card-empty a", "href")) === "#/races" &&
        /Activer dans Courses/.test((await page.textContent(".home-live")) ?? ""),
      "schedule off: the carousel says so, with a link to the Courses page",
    );
    const off = await empty.api("/api/races/sources");
    check(
      asked <= 1 && off.sources.every((s) => s.state === "disabled" && s.html === null),
      `schedule off: the server reads no website (${asked} request from the page)`,
    );
    await page.waitForSelector(".home-status.is-waiting", { timeout: 30_000 });
    await page.waitForSelector(".home-steps", { timeout: 30_000 });
    await page.waitForSelector(".home-phone-enable", { timeout: 30_000 });
    await page.waitForSelector(".home-races-card .home-card-empty", { timeout: 30_000 });
    await settle(page);
    const file = join(outDir, "home-empty.png");
    await page.screenshot({ path: file });
    console.log(`home-empty: ${file}`);

    // "Activer l'accès depuis mon téléphone" → saved, restart asked, nothing listening yet
    await page.click(".home-phone-enable");
    await page.waitForSelector(".home-notice", { timeout: 10_000 });
    const saved = JSON.parse(readFileSync(join(empty.dataDir, "settings.json"), "utf8"));
    check(saved.lan_access === true, "phone access button writes settings.json (lan_access: true)");
    const net = await empty.api("/api/network");
    check(
      net.lan_access_setting === true && net.listening_on_lan === false,
      "…and the running server keeps its address until the restart",
    );
    const notice = await page.textContent(".home-notice");
    check(/Redémarre Corde pour appliquer/.test(notice ?? ""), "…and the page asks for a restart");
    check(
      /Quitte Corde, puis relance-la/.test(notice ?? ""),
      "…with an instruction instead of a button outside the desktop app",
    );
    await page.screenshot({ path: join(outDir, "home-restart.png") });
    await page.click(".home-notice .link");
    await page.waitForSelector(".home-phone-enable", { timeout: 10_000 });
    const back = JSON.parse(readFileSync(join(empty.dataDir, "settings.json"), "utf8"));
    check(back.lan_access === false, "« Annuler » turns the setting off again");
    await close("home-empty");
  }

  // ---- default page and automatic navigation at start-up
  {
    const { page, close } = await open(`${sim.base}/`);
    await page
      .waitForFunction(() => window.location.hash === "#/", null, { timeout: 10_000 })
      .catch(() => {});
    check(
      new URL(page.url()).hash === "#/",
      "opened without a page while on track → the dashboard",
    );
    await page.waitForSelector(".grid-stack-item", { timeout: 10_000 });
    await close("boot on track");
  }
  {
    const { page, close } = await open(`${empty.base}/`);
    await page.waitForSelector(".home-status.is-waiting", { timeout: 15_000 });
    await page.waitForTimeout(1500);
    check(new URL(page.url()).hash === "", "opened without a page, no game → the home page");
    const current = await page.textContent(".navtab.is-current");
    check((current ?? "").trim() === "Accueil", "« Accueil » is the current tab");
    await close("boot waiting");
  }
  {
    const { page, close } = await open(`${sim.base}/?autonav=0`);
    await page.waitForSelector(".home-status.is-track", { timeout: 15_000 });
    await page.waitForTimeout(1500);
    check(new URL(page.url()).hash === "", "?autonav=0 pins the page (no jump while on track)");
    await close("autonav off");
  }

  // ---- automatic navigation, the game being played by this script
  {
    const frame = await sim.api("/api/frame");
    const status = await sim.api("/api/status");
    const recording = await sim.api("/api/recording");
    const garage = { ...frame, player: null, session: { ...frame.session, in_realtime: false } };
    const say = (ws, type, body) => ws.send(JSON.stringify({ type, ...body }));
    let game = null;
    const { page, close } = await open(`${sim.base}/`, {
      prepare: (p) =>
        p.routeWebSocket(/\/ws(\?|$)/, (ws) => {
          game = ws;
          ws.onMessage(() => {});
          say(ws, "hello", {
            server_version: "0.4.0",
            protocol_version: 1,
            status: { ...status, in_realtime: false },
          });
          say(ws, "frame", garage);
        }),
    });
    const hash = () => page.evaluate(() => window.location.hash);
    const hashBecomes = (test, timeout) =>
      page
        .waitForFunction(test, null, { timeout })
        .then(() => true)
        .catch(() => false);
    await page.waitForSelector(".home-status.is-garage", { timeout: 15_000 });
    await settle(page);
    await page.screenshot({ path: join(outDir, "home-garage.png") });
    check((await hash()) === "", "game in the garage → stays on the home page");

    say(game, "status", { ...status, in_realtime: true });
    say(game, "frame", frame);
    check(
      await hashBecomes(() => window.location.hash === "#/", 3000),
      "the driver takes the track → the dashboard opens",
    );

    const left = Date.now();
    say(game, "status", { ...status, in_realtime: false });
    say(game, "frame", garage);
    await page.waitForTimeout(7000);
    check((await hash()) === "#/", "back in the garage: nothing moves for the first seconds");
    const summary = await hashBecomes(() => window.location.hash.startsWith("#/sessions/"), 8000);
    const after = ((Date.now() - left) / 1000).toFixed(1);
    check(
      summary && (await hash()) === `#/sessions/${recording.session_id}`,
      `…then the recorded session opens (after ${after} s, expected ≈ 10 s)`,
    );
    await page.waitForSelector(".laps-tbl tbody tr", { timeout: 15_000 });

    // a click protects the page for 15 s
    await page.click('.navtab[href="#/profile"]');
    say(game, "status", { ...status, in_realtime: true });
    say(game, "frame", frame);
    await page.waitForTimeout(3000);
    check((await hash()) === "#/profile", "a page chosen by hand is not taken away (15 s grace)");
    await close("automatic navigation");
  }

  // ---- desktop shell hooks (fake window.__TAURI__)
  {
    const { page, close } = await open(`${sim.base}/#/home`, {
      prepare: async (p) => {
        await p.addInitScript(() => {
          window.__calls = [];
          window.__TAURI__ = {
            core: {
              invoke: async (cmd, args) => {
                window.__calls.push([cmd, args ?? null]);
                if (cmd === "get_autostart") return false;
                if (cmd === "app_version") return "0.4.0";
                return null;
              },
            },
          };
        });
        // the setting was just switched on: a restart is pending
        await p.route("**/api/network", (route) =>
          route.fulfill({
            json: { lan_access_setting: true, listening_on_lan: false, port: 8471, urls: [] },
          }),
        );
      },
    });
    await page.waitForSelector(".home-notice .home-btn", { timeout: 15_000 });
    await page.click(".home-notice .home-btn");
    await page.click('button[aria-label="Réglages"]');
    await page.waitForSelector("dialog.sheet[open]");
    const autostart = page.locator("dialog.sheet[open] .switch", {
      hasText: "démarrage de Windows",
    });
    check((await autostart.count()) === 1, "desktop: the « Application » section is shown");
    await page.waitForFunction(() => window.__calls.some(([c]) => c === "app_version"));
    await autostart.locator("input").click();
    await page.waitForFunction(() => window.__calls.some(([c]) => c === "set_autostart"));
    const dialogText = await page.textContent("dialog.sheet[open]");
    check(/0\.4\.0/.test(dialogText ?? ""), "desktop: the application version is shown");
    const calls = await page.evaluate(() => window.__calls);
    const sent = (cmd, args) =>
      calls.some(([c, a]) => c === cmd && JSON.stringify(a) === JSON.stringify(args));
    check(sent("restart_app", null), "desktop: « Redémarrer Corde » → invoke('restart_app')");
    check(sent("get_autostart", null), "desktop: invoke('get_autostart')");
    check(
      sent("set_autostart", { enabled: true }),
      "desktop: the switch → invoke('set_autostart', { enabled: true })",
    );
    await close("desktop shell");
  }
  {
    const { page, close } = await open(`${sim.base}/#/home`);
    await page.waitForSelector(".home-status.is-track", { timeout: 15_000 });
    await page.click('button[aria-label="Réglages"]');
    await page.waitForSelector("dialog.sheet[open]");
    const text = await page.textContent("dialog.sheet[open]");
    check(
      /Navigation automatique/.test(text ?? "") && !/démarrage de Windows/.test(text ?? ""),
      "browser: automatic navigation options, no « Application » section",
    );
    await close("settings in a browser");
  }
} catch (e) {
  failures += 1;
  console.error(String(e));
} finally {
  await browser?.close();
  for (const s of servers) await s.stop();
}
process.exit(failures ? 1 : 0);
