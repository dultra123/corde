#!/usr/bin/env node
/**
 * The iPhone application **simulated in Chromium** (390 × 844, device scale 2): the web assets
 * of `apps/mobile/www` (built by `pnpm --filter @corde/mobile build`) with a **fake Capacitor
 * bridge** — the object the iOS shell injects (`window.Capacitor` with `PluginHeaders`,
 * `nativePromise`, `nativeCallback`, and `window.webkit.messageHandlers.bridge`), so that
 * `@capacitor/core` believes it runs on iOS and sends every plugin call here. The fake plugins
 * answer like the real ones (Preferences in memory, CapacitorHttp with sample schedule pages,
 * LocalNotifications that grant and keep what is scheduled, App, Keyboard, ScreenOrientation).
 * A real corde-server (simulator source) is the PC of « Mes serveurs ».
 *
 *   pnpm --filter @corde/mobile build && cargo build -p corde-server
 *   node scripts/screenshot-app.mjs
 *
 * Writes screenshots/app-home.png, app-races.png, app-servers.png and
 * ../../docs/images/iphone.png (the three side by side: a **preview**, not a picture of an
 * iPhone — iOS renders with WebKit, not Chromium). Fails on any console error, and checks:
 * the tab bar is at the bottom; the schedule is read through the native HTTP only, at the two
 * addresses, with Corde's User-Agent; picking a start asks for the notifications once and
 * schedules the reminders; no horizontal overflow; the PC server answers.
 */
import { spawn } from "node:child_process";
import { createReadStream, existsSync, mkdirSync, mkdtempSync, rmSync, statSync } from "node:fs";
import { createServer } from "node:http";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { dirname, extname, join, normalize, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { lmuToolsPage, raceControlPage } from "./race-pages.mjs";

const require = createRequire(import.meta.url);
const { chromium } = require("playwright");

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const repo = join(root, "..", "..");
const www = join(repo, "apps", "mobile", "www");
const outDir = join(root, "screenshots");
const docImage = join(repo, "docs", "images", "iphone.png");
const bin = join(
  repo,
  "target",
  "debug",
  `corde-server${process.platform === "win32" ? ".exe" : ""}`,
);

if (!existsSync(join(www, "index.html"))) {
  console.error("apps/mobile/www missing — run `pnpm --filter @corde/mobile build` first");
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

/* ------------------------------------------------------------------ the PC (corde-server) */

const port = 8400 + Math.floor(Math.random() * 500);
const dataDir = mkdtempSync(join(tmpdir(), "corde-app-"));
const child = spawn(
  bin,
  [
    "--port",
    String(port),
    "--data-dir",
    dataDir,
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

/* ------------------------------------------------------------------ the web assets */

const TYPES = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript",
  ".css": "text/css",
  ".svg": "image/svg+xml",
  ".png": "image/png",
  ".woff2": "font/woff2",
  ".woff": "font/woff",
  ".webmanifest": "application/manifest+json",
  ".wav": "audio/wav",
};
const files = createServer((req, res) => {
  const path = normalize(decodeURIComponent((req.url ?? "/").split("?")[0] ?? "/"));
  const file = join(www, path === "/" ? "index.html" : path);
  if (!file.startsWith(www) || !existsSync(file) || !statSync(file).isFile()) {
    res.writeHead(404).end();
    return;
  }
  res.writeHead(200, { "Content-Type": TYPES[extname(file)] ?? "application/octet-stream" });
  createReadStream(file).pipe(res);
});
await new Promise((r) => files.listen(0, "127.0.0.1", r));
const appUrl = `http://127.0.0.1:${files.address().port}/index.html`;

/* ------------------------------------------------------------------ the fake bridge */

/**
 * Runs in the page before any script. `seed`: the native storage at start; `pages`: what
 * CapacitorHttp answers for the two schedule addresses.
 */
function fakeBridge({ seed, pages }) {
  const prefs = new Map(Object.entries(seed));
  // The simulator drives on track: no automatic jump to the live dashboard for these pictures.
  localStorage.setItem(
    "corde.settings.v1",
    JSON.stringify({
      server: "",
      lang: "fr",
      rateHz: 20,
      autoDashboard: false,
      autoSummary: false,
      raceSound: true,
    }),
  );
  const log = { http: [], scheduled: [], requests: 0, cancelled: 0 };
  const pending = [];
  let permission = "prompt";
  let nextCallback = 1;
  const listeners = [];
  window.__fake = { log, prefs, pending, listeners };

  const impl = {
    Preferences: {
      get: ({ key }) => ({ value: prefs.has(key) ? prefs.get(key) : null }),
      set: ({ key, value }) => void prefs.set(key, value),
      remove: ({ key }) => void prefs.delete(key),
      keys: () => ({ keys: [...prefs.keys()] }),
      clear: () => prefs.clear(),
    },
    CapacitorHttp: {
      request: (o) => {
        log.http.push({ url: o.url, method: o.method, headers: o.headers });
        const data = pages[o.url];
        return data
          ? { status: 200, headers: { "content-type": "text/html" }, url: o.url, data }
          : { status: 404, headers: {}, url: o.url, data: "" };
      },
    },
    CapacitorCookies: { clearCookies: () => undefined },
    LocalNotifications: {
      checkPermissions: () => ({ display: permission }),
      requestPermissions: () => {
        log.requests += 1;
        permission = "granted";
        return { display: permission };
      },
      schedule: ({ notifications }) => {
        log.scheduled.push(...notifications.map((n) => ({ id: n.id, title: n.title })));
        for (const n of notifications) {
          const i = pending.findIndex((p) => p.id === n.id);
          if (i >= 0) pending.splice(i, 1);
          pending.push({ id: n.id, title: n.title, body: n.body, extra: n.extra });
        }
        return { notifications: notifications.map((n) => ({ id: n.id })) };
      },
      getPending: () => ({ notifications: pending.slice() }),
      cancel: ({ notifications }) => {
        log.cancelled += notifications.length;
        for (const { id } of notifications) {
          const i = pending.findIndex((p) => p.id === id);
          if (i >= 0) pending.splice(i, 1);
        }
      },
      registerActionTypes: () => undefined,
    },
    App: { getLaunchUrl: () => ({}) },
    Keyboard: { setStyle: () => undefined, setResizeMode: () => undefined },
    ScreenOrientation: { lock: () => undefined, unlock: () => undefined },
  };
  // CapacitorHttp.get = request with method GET (what the native side does).
  impl.CapacitorHttp.get = (o) => impl.CapacitorHttp.request({ ...o, method: "GET" });

  const PluginHeaders = Object.entries(impl).map(([name, methods]) => ({
    name,
    methods: [
      ...Object.keys(methods).map((m) => ({ name: m, rtype: "promise" })),
      { name: "addListener", rtype: "callback" },
      { name: "removeListener", rtype: "promise" },
      { name: "removeAllListeners", rtype: "promise" },
    ],
  }));

  window.webkit = { messageHandlers: { bridge: { postMessage() {} } } };
  window.Capacitor = {
    PluginHeaders,
    nativePromise(plugin, method, options) {
      const f = impl[plugin]?.[method];
      if (method === "removeListener" || method === "removeAllListeners") return Promise.resolve();
      if (!f) return Promise.reject(new Error(`${plugin}.${method} not faked`));
      try {
        return Promise.resolve(f(options ?? {}));
      } catch (e) {
        return Promise.reject(e);
      }
    },
    nativeCallback(plugin, method, options, callback) {
      const id = String(nextCallback++);
      if (method === "addListener") listeners.push({ plugin, event: options?.eventName, callback });
      return id;
    },
  };
}

/* ------------------------------------------------------------------ run */

let browser = null;
let page = null;
const errors = [];
try {
  for (let i = 0; i < 60; i++) {
    try {
      const r = await fetch(`http://127.0.0.1:${port}/api/health`);
      if (r.ok) break;
    } catch {
      // not yet
    }
    await sleep(250);
  }
  const now = Date.now();
  const pages = {
    "https://racecontrol.gg/": raceControlPage(now),
    "https://lmu.tools/schedule": lmuToolsPage(now),
  };
  const seed = {
    "corde.servers.v1": JSON.stringify([
      {
        id: "0123456789abcdef",
        base: `http://127.0.0.1:${port}`,
        label: `127.0.0.1:${port}`,
        name: "PC de jeu",
        added_ms: now,
        role: "desktop",
      },
      {
        id: "fedcba9876543210",
        base: "http://10.8.0.1:8471",
        label: "10.8.0.1:8471",
        name: "Serveur Corde",
        added_ms: now,
        role: "hub",
      },
    ]),
    "corde.servers.active": "0123456789abcdef",
    "corde.races.enabled": "true",
  };

  browser = await chromium.launch();
  const context = await browser.newContext({
    viewport: { width: 390, height: 844 },
    deviceScaleFactor: 2,
    isMobile: true,
    hasTouch: true,
    locale: "fr-FR",
    timezoneId: "Europe/Zurich",
    colorScheme: "dark",
  });
  await context.addInitScript(fakeBridge, { seed, pages });
  page = await context.newPage();
  page.on("console", (msg) => {
    if (msg.type() === "error") errors.push(msg.text());
  });
  page.on("pageerror", (err) => errors.push(String(err)));
  // Nothing of the application may reach the network but the PC and the web assets: the
  // schedule goes through the (fake) native HTTP.
  const outside = [];
  await page.route("**/*", (route) => {
    const u = new URL(route.request().url());
    if (u.hostname === "127.0.0.1") return route.continue();
    outside.push(u.href);
    return route.abort();
  });

  await page.goto(appUrl);
  // The safe areas of an iPhone with a notch (Chromium has none): what the page must leave free.
  await page.addStyleTag({
    content: "html:root { --safe-top: 47px; --safe-bottom: 34px; }",
  });

  const overflow = () =>
    page.evaluate(
      () => document.documentElement.scrollWidth - document.documentElement.clientWidth,
    );

  // Home: the races of the phone, then the server.
  await page.waitForSelector(".app-tabs");
  await page.waitForSelector(".live-slide.is-current", { timeout: 20_000 });
  await page.waitForFunction(
    () => document.querySelector(".app-server-line")?.textContent?.includes("connecté"),
    null,
    { timeout: 20_000 },
  );
  const tabs = await page.$eval(".app-tabs", (el) => {
    const r = el.getBoundingClientRect();
    return { bottom: Math.round(r.bottom), height: Math.round(r.height), n: el.children.length };
  });
  check(
    tabs.bottom === 844 && tabs.n === 5,
    `home: 5 tabs at the bottom (${JSON.stringify(tabs)})`,
  );
  check(
    (await page.$(".topbar .navtabs")) === null ||
      (await page.$eval(".topbar .navtabs", (el) => getComputedStyle(el).display)) === "none",
    "home: no tabs in the top bar",
  );
  check((await overflow()) <= 0, "home: no horizontal overflow");
  await sleep(600);
  await page.screenshot({ path: join(outDir, "app-home.png") });

  const http = await page.evaluate(() => window.__fake.log.http);
  check(
    http.length >= 2 &&
      http.every(
        (c) =>
          ["https://racecontrol.gg/", "https://lmu.tools/schedule"].includes(c.url) &&
          c.method === "GET" &&
          /^Corde\/\d+\.\d+\.\d+ \(\+https:\/\/github\.com\/dultra123\/corde\)$/.test(
            c.headers["User-Agent"],
          ),
      ),
    `schedule read by the native HTTP, two addresses, Corde's User-Agent (${http.length} calls)`,
  );

  // Courses: pick a start → the notifications are asked once, the reminders scheduled.
  await page.evaluate(() => {
    window.location.hash = "#/races";
  });
  await page.waitForSelector(".race-card .race-start", { timeout: 20_000 });
  const before = await page.evaluate(() => window.__fake.log.requests);
  check(before === 0, "no notification permission asked before the first pick");
  const chip = page.locator(".races-cat.cat-daily .race-start:not(.is-on)").nth(1);
  await chip.click();
  await page.waitForFunction(() => window.__fake.pending.length > 0, null, { timeout: 10_000 });
  const fake = await page.evaluate(() => ({
    requests: window.__fake.log.requests,
    pending: window.__fake.pending.map((p) => p.title),
  }));
  check(fake.requests === 1, "the first pick asks for the notifications (once)");
  check(fake.pending.length >= 1, `reminders scheduled: ${fake.pending.join(" / ")}`);
  const stored = await page.evaluate(
    () => JSON.parse(window.__fake.prefs.get("corde.races.picks.v1") ?? "[]").length,
  );
  check(stored === 1, "the pick is kept in the native storage");
  check((await overflow()) <= 0, "courses: no horizontal overflow");
  await page.evaluate(() => window.scrollTo(0, 0));
  await sleep(400);
  await page.screenshot({ path: join(outDir, "app-races.png") });

  // Mes serveurs.
  await page.evaluate(() => {
    window.location.hash = "#/servers";
  });
  await page.waitForSelector(".app-server.is-active");
  check((await page.locator(".app-server").count()) === 2, "servers: the two servers are listed");
  check((await page.locator(".app-pair").count()) === 1, "servers: the home server offers pairing");
  check((await overflow()) <= 0, "servers: no horizontal overflow");
  await sleep(300);
  await page.screenshot({ path: join(outDir, "app-servers.png") });

  check(outside.length === 0, `nothing fetched outside (${outside.slice(0, 3).join(", ")})`);
  check(errors.length === 0, `no console error${errors.length ? `: ${errors.join(" | ")}` : ""}`);
  await context.close();
} catch (e) {
  // What the page showed and said when it went wrong.
  if (page) {
    await page.screenshot({ path: join(outDir, "app-failure.png") }).catch(() => {});
    console.error("console errors:", errors);
    console.error("page:", (await page.textContent("body").catch(() => ""))?.slice(0, 600));
  }
  throw e;
} finally {
  if (browser) await browser.close();
  files.close();
  child.kill();
  rmSync(dataDir, { recursive: true, force: true });
}

if (failures > 0) {
  console.error(`${failures} check(s) failed`);
  process.exit(1);
}
const compose = spawn(
  "python3",
  [
    join(root, "scripts", "compose-mobile.py"),
    docImage,
    join(outDir, "app-home.png"),
    join(outDir, "app-races.png"),
    join(outDir, "app-servers.png"),
  ],
  { stdio: "inherit" },
);
compose.on("exit", (code) => process.exit(code ?? 1));
