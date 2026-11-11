#!/usr/bin/env node
/**
 * Screenshots of the session browser against a real corde-server (simulator source).
 *
 *   # terminal 1 — short laps so a few are recorded within a minute
 *   cargo run -p corde-server -- --source sim --sim-track-length 1500 --sim-lap-time 30 \
 *       --sim-speed 3 --record-rate 20 --port 8471
 *   # terminal 2
 *   pnpm --filter @corde/ui build && node scripts/screenshot-sessions.mjs
 *
 * Or let the script start the server itself (debug binary from `cargo build -p corde-server`):
 *
 *   node scripts/screenshot-sessions.mjs --spawn
 *
 * Serves `dist/` on a local port, reaches the server through `?server=host:port` (REST + WS go
 * straight to it, CORS is permissive), waits until the latest session holds `MIN_LAPS` laps,
 * then writes screenshots/sessions.png, session.png, lap-compare.png (1920×1080) and
 * lap-mobile.png (390×844). When the server has imported results files (`--spawn` passes
 * `--lmu-results-dir reference/samples`), it also writes profile.png, result.png (1920×1080)
 * and profile-mobile.png (390×844). `--profile-only` skips the session shots (no wait for
 * laps). The engineer page (`#/sessions/<id>/engineer`) is shot once the session holds enough
 * clean laps for an analysis: engineer.png (1920×1080, scrolled to the feedback / iteration
 * area), engineer-full.png (1920 wide, whole page) and engineer-mobile.png (390 wide, whole
 * page), after posting two sample feedbacks through the API (each one is interpreted and
 * answered by a plan iteration) and typing a third one in the page itself, so the result block
 * under the form is visible; `--engineer-only` shoots just those three (~1 min with `--spawn`,
 * since the simulator must lap three times). With `--spawn` the server also gets a fake game
 * folder (`--game-user-data`) holding the real `.svm` fixtures of `crates/corde-lmu` plus two
 * setups of the simulator's car at its track, so the engineer page shows "Ton réglage", every
 * change before → after on the real setup, and the files it writes: engineer.png is framed on
 * that block after clicking "Créer les fichiers de réglage" (safe + aggressive plans);
 * engineer-feedback.png keeps the feedback / iteration framing. Fails on any browser console
 * error.
 *
 *   CORDE_SERVER=127.0.0.1:8471   server to use (default)
 *   SCREENSHOT_MIN_LAPS=4         laps to wait for
 *   SCREENSHOT_TIMEOUT_MS=90000   how long to wait for them
 */
import { spawn } from "node:child_process";
import {
  createReadStream,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  statSync,
  utimesSync,
  writeFileSync,
} from "node:fs";
import { createServer } from "node:http";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { dirname, extname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const require = createRequire(import.meta.url);
const { chromium } = require("playwright");

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const dist = join(root, "dist");
const outDir = join(root, "screenshots");
const MIN_LAPS = Number(process.env.SCREENSHOT_MIN_LAPS ?? 4);
const TIMEOUT_MS = Number(process.env.SCREENSHOT_TIMEOUT_MS ?? 90_000);
const SPAWN = process.argv.includes("--spawn");
const PROFILE_ONLY = process.argv.includes("--profile-only");
const ENGINEER_ONLY = process.argv.includes("--engineer-only");
let serverHost = process.env.CORDE_SERVER ?? "127.0.0.1:8471";

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

// ---------------------------------------------------------------- fake game folder
/**
 * `UserData/player/Settings` with the real fixtures (BMW at Long Beach, Ligier at Road Atlanta)
 * and two setups of the simulator's car ("Mercedes-AMG GT3", "Paul Ricard - 1A") made from the
 * BMW file: the older one has another camber and rear bar, so labels can be learnt from it.
 */
function makeUserData(root) {
  const fixtures = join(root, "..", "..", "crates", "corde-lmu", "tests", "fixtures", "svm");
  const settings = join(dataDir, "UserData", "player", "Settings");
  const put = (folder, name, text, ageS) => {
    mkdirSync(join(settings, folder), { recursive: true });
    const file = join(settings, folder, name);
    writeFileSync(file, text);
    const t = Date.now() / 1000 - ageS;
    utimesSync(file, t, t);
  };
  const read = (name) => readFileSync(join(fixtures, name), "utf8");
  put("Longbeach", "BMW_LongBeach.svm", read("BMW_LongBeach.svm"), 5 * 86400);
  put("Longbeach", "BMW_LongBeach_v2.svm", read("BMW_LongBeach_v2.svm"), 4 * 86400);
  put(
    "Road Atlanta",
    "Ligier_LMP3_RoadAtlanta_v2.svm",
    read("Ligier_LMP3_RoadAtlanta_v2.svm"),
    6 * 86400,
  );
  const merc = read("BMW_LongBeach.svm")
    .replace(
      'VehicleClassSetting="BMW_M4_LMGT3 GT3 WEC2026"',
      'VehicleClassSetting="Mercedes_AMG_LMGT3 GT3 WEC2026"',
    )
    .replace(/Notes="[^"]*"/, 'Notes="Paul Ricard - base sprint, aileron +2, repartition 51.5"');
  const older = merc
    .replace(/Notes="[^"]*"/, 'Notes="Paul Ricard - premier essai"')
    .replaceAll("CamberSetting=22//-2.20 deg", "CamberSetting=25//-2.00 deg")
    .replace("RearAntiSwaySetting=2//P2", "RearAntiSwaySetting=1//P1");
  put("Paulricard", "Mercedes_PaulRicard_v1.svm", older, 2 * 86400);
  put("Paulricard", "Mercedes_PaulRicard_v2.svm", merc, 3600);
  return join(dataDir, "UserData");
}

// ---------------------------------------------------------------- optional server
let child = null;
let dataDir = null;
if (SPAWN) {
  const bin = join(
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
  dataDir = mkdtempSync(join(tmpdir(), "corde-shots-"));
  child = spawn(
    bin,
    [
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
      "--port",
      String(port),
      "--bind",
      "127.0.0.1",
      "--data-dir",
      dataDir,
      "--lmu-results-dir",
      join(root, "..", "..", "reference", "samples"),
      "--game-user-data",
      makeUserData(root),
    ],
    { stdio: ["ignore", "ignore", "inherit"] },
  );
  console.log(`server: spawned on ${serverHost} (data in ${dataDir})`);
}

const api = async (path, init) => {
  const res = await fetch(`http://${serverHost}${path}`, init);
  if (!res.ok) throw new Error(`${path}: HTTP ${res.status}`);
  return res.json();
};

/** Waits until the engineer can analyse the session (422 while it has < 1 clean lap). */
async function waitForAnalysis(id) {
  const deadline = Date.now() + TIMEOUT_MS;
  let last = null;
  while (Date.now() < deadline) {
    try {
      const report = await api(`/api/sessions/${encodeURIComponent(id)}/analysis`);
      if (report.data_quality.valid_laps >= 3) return report;
      last = `${report.data_quality.valid_laps} clean lap(s)`;
    } catch (e) {
      last = String(e);
    }
    await sleep(1500);
  }
  throw new Error(`no analysis with 3 clean laps for ${id} after ${TIMEOUT_MS} ms (last: ${last})`);
}

/**
 * Two sample feedbacks through the API, so the engineer page shows a non-empty list and an
 * iteration history (the server interprets each one and iterates the plan).
 */
async function seedFeedback(id) {
  const path = `/api/sessions/${encodeURIComponent(id)}/feedback`;
  const existing = await api(path);
  if (existing.length) return;
  const post = (body) =>
    api(path, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
    });
  await post({
    plan: "safe",
    text: "L'avant ne tourne pas en entrée des virages lents, surtout au T3.",
  });
  await post({ text: "Instable au freinage en bout de ligne droite, l'arrière bouge." });
}

/**
 * The feedback typed in the page itself: desktop = mixed sentence (two symptoms, one without
 * corner speed → a clarification question); mobile = a symptom gone (struck through) + another.
 */
const TYPED_FEEDBACK =
  "Le train avant pousse un peu en entrée mais l'arrière part à la remise des gaz dans les lents.";
const TYPED_FEEDBACK_MOBILE =
  "Plus de sous-virage en entrée, par contre l'arrière bloque beaucoup au freinage.";

/** Types a feedback in the form and waits for the interpretation + iteration block. */
async function typeFeedback(page, text) {
  await page.waitForSelector(".feedback-text", { timeout: 30_000 });
  await page.fill(".feedback-text", text);
  await page.selectOption(".feedback-plan select", "safe");
  await page.click(".feedback-row button[type=submit]");
  await page.waitForSelector(".feedback-result .iteration", { timeout: 60_000 });
  await page.waitForTimeout(300);
}

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function waitForLaps() {
  const deadline = Date.now() + TIMEOUT_MS;
  let last = null;
  while (Date.now() < deadline) {
    try {
      const sessions = await api("/api/sessions?limit=5");
      const live = sessions.find((s) => s.lap_count >= MIN_LAPS) ?? null;
      if (live) return live;
      last = sessions[0]?.lap_count ?? 0;
    } catch (e) {
      last = String(e);
    }
    await sleep(1500);
  }
  throw new Error(
    `no session with ${MIN_LAPS} laps on ${serverHost} after ${TIMEOUT_MS} ms (last: ${last})`,
  );
}

// ---------------------------------------------------------------- static dist
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
const base = `http://127.0.0.1:${port}/?server=${encodeURIComponent(serverHost)}`;

/** The imported race result to show, once the start-up import scan is done (≈ 1 s). */
async function waitForResults() {
  const deadline = Date.now() + 20_000;
  while (Date.now() < deadline) {
    try {
      const profile = await api("/api/profile");
      if (profile.import.last_scan_ms) {
        const races = await api("/api/results?kind=race&limit=1");
        return races[0] ?? (await api("/api/results?limit=1"))[0] ?? null;
      }
    } catch {
      // server still starting
    }
    await sleep(500);
  }
  return null;
}

let failures = 0;
let browser = null;
try {
  const shots = [];
  if (!PROFILE_ONLY) {
    console.log(`waiting for a session with ${MIN_LAPS} laps on ${serverHost}…`);
    const session = await waitForLaps();
    const detail = await api(`/api/sessions/${encodeURIComponent(session.id)}`);
    const laps = detail.laps
      .filter((l) => l.valid && !l.in_lap && !l.out_lap)
      .sort((x, y) => x.lap_time_s - y.lap_time_s);
    const a = laps[0]?.lap_number ?? detail.laps[0].lap_number;
    const b = laps.at(-1)?.lap_number ?? detail.laps.at(-1).lap_number;
    console.log(`session ${session.id}: ${session.lap_count} laps → compare ${a} vs ${b}`);
    if (!ENGINEER_ONLY) {
      shots.push(
        { name: "sessions", hash: "#/sessions", ready: ".sessions-tbl tbody tr" },
        { name: "session", hash: `#/sessions/${session.id}`, ready: ".laps-tbl tbody tr" },
        {
          name: "lap-compare",
          hash: `#/sessions/${session.id}/compare?a=${a}&b=${b}`,
          ready: ".cp-plot canvas",
          minCanvases: 10,
        },
        {
          name: "lap-mobile",
          hash: `#/sessions/${session.id}/laps/${a}`,
          ready: ".cp-plot canvas",
          minCanvases: 9,
          mobile: true,
        },
      );
    }
    const report = await waitForAnalysis(session.id);
    await seedFeedback(session.id);
    console.log(
      `engineer: ${report.data_quality.valid_laps} clean laps, ${report.findings.length} findings`,
    );
    const engineer = `#/sessions/${session.id}/engineer`;
    shots.push(
      { name: "engineer", hash: engineer, ready: ".feedback-item", setupFiles: true },
      { name: "engineer-feedback", hash: engineer, ready: ".feedback-item", feedback: true },
      { name: "engineer-full", hash: engineer, ready: ".feedback-item", fullPage: true },
      {
        name: "engineer-mobile",
        hash: engineer,
        ready: ".feedback-item",
        mobile: true,
        feedback: true,
      },
    );
  }

  const result = ENGINEER_ONLY ? null : await waitForResults();
  if (result) {
    console.log(`result #${result.id}: ${result.track_course} (${result.session_kind})`);
    shots.push(
      { name: "profile", hash: "#/profile", ready: ".tracks-tbl tbody tr" },
      { name: "result", hash: `#/results/${result.id}`, ready: ".standings-tbl tbody tr" },
      { name: "profile-mobile", hash: "#/profile", ready: ".tracks-tbl tbody tr", mobile: true },
    );
  } else if (!ENGINEER_ONLY) {
    console.log("no imported result on the server: profile screenshots skipped");
  }

  browser = await chromium.launch();
  for (const shot of shots) {
    const mobile = !!shot.mobile;
    const context = await browser.newContext({
      viewport: mobile ? { width: 390, height: 844 } : { width: 1920, height: 1080 },
      deviceScaleFactor: mobile ? 2 : 1,
      isMobile: mobile,
      hasTouch: mobile,
      locale: "fr-FR",
    });
    const page = await context.newPage();
    const errors = [];
    page.on("console", (msg) => {
      if (msg.type() === "error") errors.push(msg.text());
    });
    page.on("pageerror", (err) => errors.push(String(err)));
    await page.goto(`${base}${shot.hash}`, { waitUntil: "load" });
    await page.waitForSelector(shot.ready, { timeout: 30_000 });
    if (shot.minCanvases) {
      await page.waitForFunction(
        (n) => document.querySelectorAll(".cp-plot canvas").length >= n,
        shot.minCanvases,
        { timeout: 30_000 },
      );
    }
    await page.waitForFunction(() => !document.querySelector(".spinner"), null, {
      timeout: 30_000,
    });
    await page.waitForTimeout(800);
    if (shot.name === "lap-compare") {
      // hover the speed chart so the synced cursor and legend values show up
      const plot = await page.$(".cp-plot");
      const box = await plot?.boundingBox();
      if (box) await page.mouse.move(box.x + box.width * 0.42, box.y + box.height / 2);
      await page.waitForTimeout(200);
    }
    if (shot.name === "engineer-full") {
      // open the detected-corners list so the whole-page capture shows it
      await page.$eval(".eng-details", (d) => {
        d.open = true;
      });
      await page.waitForTimeout(150);
    }
    if (shot.setupFiles && (await page.$(".setup-create"))) {
      // "Ton réglage": write the safe + aggressive plans, then frame the block and the plans.
      const aggressive = await page.$(".setup-choice.is-aggressive input");
      if (aggressive && !(await aggressive.isChecked())) await aggressive.check();
      await page.click(".setup-create");
      await page.waitForSelector(".setup-done, .setup .form-error", { timeout: 30_000 });
      await page.$eval(".setup", (el) => {
        const top = el.getBoundingClientRect().top + window.scrollY - 56;
        window.scrollTo(0, Math.max(0, top));
      });
      await page.waitForTimeout(300);
    }
    if (shot.feedback) {
      await typeFeedback(page, mobile ? TYPED_FEEDBACK_MOBILE : TYPED_FEEDBACK);
      if (!mobile) {
        // frame the feedback → interpretation → iteration area in the viewport
        await page.$eval(".feedback", (el) => {
          // below the fixed top bar, so the card title stays visible
          const top = el.getBoundingClientRect().top + window.scrollY - 56;
          window.scrollTo(0, Math.max(0, top));
        });
        await page.waitForTimeout(200);
      }
    }
    if (shot.name === "profile") {
      // hover the progression chart so the session under the cursor is described
      const plot = await page.$(".bestlap .laptimes-plot");
      const box = await plot?.boundingBox();
      if (box) await page.mouse.move(box.x + box.width * 0.5, box.y + box.height / 2);
      await page.waitForTimeout(200);
    }
    const file = join(outDir, `${shot.name}.png`);
    await page.screenshot({ path: file, fullPage: mobile || !!shot.fullPage });
    console.log(`${shot.name}: ${file}`);
    if (errors.length) {
      failures += errors.length;
      console.error(`${shot.name}: ${errors.length} console error(s)`);
      for (const e of errors) console.error(`  ${e}`);
    }
    await context.close();
  }
} catch (e) {
  failures += 1;
  console.error(String(e));
} finally {
  await browser?.close();
  server.close();
  if (child) {
    child.kill("SIGTERM");
    await sleep(500);
    if (dataDir) rmSync(dataDir, { recursive: true, force: true });
  }
}
process.exit(failures ? 1 : 0);
