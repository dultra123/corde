#!/usr/bin/env node
/**
 * Screenshots and end-to-end checks of the « Courses » page (`#/races`), against a real
 * corde-server started by this script (debug binary from `cargo build -p corde-server`, UI from
 * `pnpm --filter @corde/ui build`, served by the server with `--ui-dir`):
 *
 *   node scripts/screenshot-races.mjs        (≈ 40 s: one reminder is waited for)
 *
 * This machine cannot reach racecontrol.gg nor lmu.tools, so the answers of
 * `GET /api/races/sources` are replaced in the browser by sample pages (`race-pages.mjs`), in
 * the format the parser reads (racecontrol.gg: the format seen on the real page on 11 Nov 2026,
 * in UTC−4; lmu.tools: the format assumed from the owner's script, in UTC+1), with start times
 * computed from the current time — the times on the pictures are plausible. Everything else is
 * the real server: the `online_schedule` setting, the picks, the race settings and the reminder
 * sent on the WebSocket.
 *
 * Writes screenshots/races-off.png, races.png (1920×1080, with a reminder banner),
 * races-mobile.png (390 wide, whole page) and ../../docs/images/courses.png (desktop + phone).
 * Fails on any browser console error, and checks: nothing is fetched while the setting is off;
 * the button turns it on (`settings.json`); picking a start writes it on the server; the home
 * page starts its carousel with the next race; a reminder due in 15 s reaches the page as a
 * banner; « Tester la sonnerie » unlocks the sound; the page has no horizontal overflow at 390 px; a page without
 * races is reported to the server for diagnosis; the HTML of the sites never reaches the DOM.
 */
import { spawn } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
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
const docImage = join(repo, "docs", "images", "courses.png");
const bin = join(
  repo,
  "target",
  "debug",
  `corde-server${process.platform === "win32" ? ".exe" : ""}`,
);
const TZ = "Europe/Zurich";

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

let failures = 0;
const check = (ok, what) => {
  console.log(`${ok ? "ok  " : "FAIL"} ${what}`);
  if (!ok) failures += 1;
};

/* ------------------------------------------------------------------ server */

const port = 8400 + Math.floor(Math.random() * 500);
const dataDir = mkdtempSync(join(tmpdir(), "corde-races-"));
const child = spawn(
  bin,
  [
    "--port",
    String(port),
    "--data-dir",
    dataDir,
    "--ui-dir",
    dist,
    "--log",
    "warn",
    "--source",
    "sim",
    "--no-record",
    "--no-game-api",
    "--no-import",
  ],
  { stdio: ["ignore", "ignore", "inherit"] },
);
const base = `http://127.0.0.1:${port}`;
console.log(`server: ${base} (data in ${dataDir})`);
const api = async (path, init) => {
  const res = await fetch(`${base}${path}`, init);
  if (!res.ok) throw new Error(`${path}: HTTP ${res.status}`);
  return res.status === 204 ? null : res.json();
};

let browser = null;
try {
  for (let i = 0; i < 40; i++) {
    try {
      await api("/api/health");
      break;
    } catch {
      await sleep(250);
    }
  }
  const off = await api("/api/races/sources?force=1");
  check(
    off.enabled === false && off.sources.every((s) => s.state === "disabled" && s.html === null),
    "default: the schedule is off and the server reads nothing",
  );

  browser = await chromium.launch();
  let unreadablePosts = 0;
  let sourcesServed = 0;
  /** Opens the page; `GET /api/races/sources` answered with the sample pages (see the header). */
  async function open(hash, { mobile = false, broken = false } = {}) {
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
    await page.route("**/api/races/sources**", async (route) => {
      const url = route.request().url();
      if (url.includes("/unreadable")) {
        unreadablePosts += 1;
        return route.continue();
      }
      const settings = await api("/api/settings");
      if (!settings.online_schedule) return route.continue();
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
            one(
              "racecontrol",
              "https://racecontrol.gg/",
              broken ? "<html><body><h1>Maintenance</h1></body></html>" : raceControlPage(now),
            ),
            one("lmutools", "https://lmu.tools/schedule", lmuToolsPage(now)),
          ],
        },
      });
    });
    await page.goto(`${base}/${hash}`, { waitUntil: "load" });
    return {
      page,
      async close(label) {
        check(errors.length === 0, `${label}: no console error`);
        for (const e of errors) console.error(`     ${e}`);
        await context.close();
      },
    };
  }
  async function settle(page) {
    await page.evaluate(() => document.fonts.ready);
    await page.waitForTimeout(600);
  }

  // ---- off: two sentences and a button; on: the schedule
  {
    const { page, close } = await open("#/races");
    await page.waitForSelector(".races-off-card", { timeout: 15_000 });
    const text = await page.textContent(".races-off-card");
    check(
      /racecontrol\.gg et lmu\.tools depuis ce PC, au plus toutes les 10 minutes/.test(text ?? ""),
      "off: the page says what turning it on does",
    );
    await settle(page);
    await page.screenshot({ path: join(outDir, "races-off.png") });
    check(sourcesServed === 0, "off: no schedule served to the page");
    await page.click(".races-off-card .home-btn");
    await page.waitForSelector(".race-card", { timeout: 15_000 });
    const saved = JSON.parse(readFileSync(join(dataDir, "settings.json"), "utf8"));
    check(saved.online_schedule === true, "« Activer le planning » writes settings.json");
    const cats = await page.$$eval(".races-cat-title", (els) => els.map((e) => e.textContent));
    check(cats.length === 4, `four categories shown (${cats.length})`);
    const leaked = await page.evaluate(
      () =>
        document.body.innerHTML.includes("Upcoming Daily Races") ||
        !!document.querySelector("main main, section.daily"),
    );
    check(!leaked, "the HTML of the sites never reaches the DOM");

    // pick the first LMP3 start (≈ 12 min from now), a weekly race and the special event
    const lmp3 = page.locator(".race-card", { hasText: "LMP3 Fixed" }).first();
    await lmp3.locator(".race-start").first().click();
    await page.waitForSelector(".race-start.is-on", { timeout: 10_000 });
    await page
      .locator(".race-card", { hasText: "GT3 Sprint Weekly" })
      .locator(".race-start")
      .nth(1)
      .click();
    await page
      .locator(".race-card", { hasText: "Sebring 1000 Miles" })
      .locator(".race-start")
      .click();
    await page.waitForFunction(
      () => document.querySelectorAll(".races-mine-row").length === 3,
      null,
      {
        timeout: 10_000,
      },
    );
    const picks = await api("/api/races/picks");
    check(picks.length === 3, `three picks saved on the server (${picks.length})`);
    const special = picks.find((p) => p.cat === "special");
    check(
      !!special &&
        special.eve_ms < special.start_ms &&
        new Date(special.start_ms).getUTCHours() >= 8,
      "the special event is picked on its day, with the evening before",
    );

    // driver profile: a select writes the server settings
    await page
      .locator(".races-card", { hasText: "Mon profil pilote" })
      .locator("select")
      .first()
      .selectOption("S1");
    await page
      .locator(".races-card", { hasText: "Mon profil pilote" })
      .locator("select")
      .nth(1)
      .selectOption("S2");
    await page.waitForTimeout(500);
    const rs = await api("/api/races/settings");
    check(
      rs.profile.dr === "S1" && rs.profile.sr === "S2",
      "the driver's ratings are saved on the server",
    );

    // the sound: the click unlocks it
    await page.click(".races-test");
    await page.waitForFunction(
      () => /prêt/.test(document.querySelector(".races-sound + .races-hint")?.textContent ?? ""),
      null,
      { timeout: 5000 },
    );
    check(true, "« Tester la sonnerie » unlocks the sound");

    // a reminder due in 15 s, through the real WebSocket
    const start = Date.now() + 5 * MIN + 15_000;
    const due = {
      id: `prototype-fixed-intermediate@${start}`,
      race_id: "prototype-fixed-intermediate",
      cat: "daily",
      src: "rc",
      name: "Prototype Fixed",
      level: "intermediate",
      track: "Circuit De Barcelona Catalunya",
      duration: "44 min",
      image: null,
      start_ms: start,
    };
    await api(`/api/races/picks/${encodeURIComponent(due.id)}`, {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(due),
    });
    const shown = page.locator(".race-banner", { hasText: "Prototype Fixed" });
    await shown.waitFor({ timeout: 40_000 });
    const banner = await shown.textContent();
    check(
      /Départ dans 5 min — Prototype Fixed, Circuit De Barcelona Catalunya/.test(banner ?? ""),
      `banner: ${banner}`,
    );
    check(
      (await shown.getAttribute("class"))?.includes("is-urgent") === true,
      "≤ 5 min: the urgent banner",
    );
    check((await page.$$(".race-banner")).length === 1, "one banner (no other reminder due)");
    await page.evaluate(() => window.scrollTo(0, 0));
    await settle(page);
    await page.screenshot({ path: join(outDir, "races.png") });
    console.log(`races: ${join(outDir, "races.png")}`);
    await close("races (desktop)");
  }

  // ---- home: the carousel starts with the driver's next race
  {
    const { page, close } = await open("#/home");
    await page.waitForSelector(".home-live .live-slide.is-current", { timeout: 15_000 });
    const line = (await page.textContent(".home-live .live-slide.is-current")) ?? "";
    check(
      /Choisie.*(LMP3 Fixed|Prototype Fixed).*dans \d+ (min|s)/.test(line),
      `home: ${line.replace(/\s+/g, " ")}`,
    );
    check((await page.$$(".home-nextrace")).length === 0, "home: no separate « next race » line");
    await close("home");
  }

  // ---- phone
  let mobileShot = null;
  {
    const { page, close } = await open("#/races", { mobile: true });
    await page.waitForSelector(".race-card", { timeout: 15_000 });
    await page.waitForSelector(".races-next", { timeout: 15_000 });
    await settle(page);
    mobileShot = join(outDir, "races-mobile.png");
    await page.screenshot({ path: mobileShot, fullPage: true });
    const overflow = await page.evaluate(
      () => document.documentElement.scrollWidth - document.documentElement.clientWidth,
    );
    check(overflow <= 0, `phone: no horizontal overflow (${overflow}px)`);
    await close("races (phone)");
  }

  // ---- a page without races: notice + report to the server
  {
    const before = unreadablePosts;
    const { page, close } = await open("#/races", { broken: true });
    await page.waitForSelector(".races-notice.is-warn", { timeout: 15_000 });
    const text = await page.textContent(".races-notice.is-warn");
    check(/racecontrol\.gg n'a pas pu être lue/.test(text ?? ""), "unreadable page: clear message");
    check(
      (await page.getAttribute(".races-notice.is-warn a", "href")) === "https://racecontrol.gg/",
      "…with a link to the site",
    );
    await page.waitForTimeout(800);
    check(unreadablePosts > before, "…and the server is asked to keep its copy");
    check((await page.$$(".race-card")).length > 0, "…the other site still gives its races");
    await close("unreadable page");
  }

  // ---- the picture of the README: desktop + phone (first screen)
  {
    const desk = readFileSync(join(outDir, "races.png")).toString("base64");
    const phone = readFileSync(mobileShot).toString("base64");
    const context = await browser.newContext({ viewport: { width: 1920, height: 940 } });
    const page = await context.newPage();
    await page.setContent(`<!doctype html><html><body style="margin:0;background:#0b0e14">
      <div style="position:relative;width:1920px;height:940px;overflow:hidden">
        <img src="data:image/png;base64,${desk}" style="position:absolute;left:36px;top:36px;width:1460px;border-radius:10px;box-shadow:0 18px 50px rgba(0,0,0,.6);outline:1px solid #262d3a">
        <div style="position:absolute;left:1536px;top:36px;width:348px;height:752px;border-radius:34px;background:#262d3a;padding:8px;box-sizing:border-box;box-shadow:0 18px 50px rgba(0,0,0,.6)">
          <div style="width:332px;height:736px;border-radius:27px;overflow:hidden;background:#0b0e12">
            <img src="data:image/png;base64,${phone}" style="width:332px;display:block">
          </div>
        </div>
      </div></body></html>`);
    await page.waitForTimeout(300);
    await page.screenshot({ path: docImage, clip: { x: 0, y: 0, width: 1920, height: 940 } });
    console.log(`courses: ${docImage}`);
    await context.close();
  }
} catch (e) {
  failures += 1;
  console.error(String(e?.stack ?? e));
} finally {
  await browser?.close();
  child.kill("SIGTERM");
  await sleep(400);
  rmSync(dataDir, { recursive: true, force: true });
}
process.exit(failures ? 1 : 0);
