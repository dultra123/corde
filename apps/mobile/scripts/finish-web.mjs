#!/usr/bin/env node
/**
 * After `vite build --mode mobile` (packages/ui → apps/mobile/www): adds what only the
 * application needs to its web assets — the sound of the reminders, which the notifications
 * plugin looks for at the top of the web folder (`LocalNotificationsPlugin.swift`,
 * `resolveSound`: `public/<name>` in the application bundle) — and checks the result.
 */
import { copyFileSync, existsSync, readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const mobile = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const www = join(mobile, "www");
const index = join(www, "index.html");
if (!existsSync(index)) {
  console.error(`missing ${index}: run the mobile build of the interface first`);
  process.exit(1);
}
copyFileSync(join(mobile, "assets/corde-reminder.wav"), join(www, "corde-reminder.wav"));
const html = readFileSync(index, "utf8");
if (!/<script type="module"[^>]*src="\.\/assets\//.test(html)) {
  console.error("www/index.html does not load ./assets/…: unexpected build");
  process.exit(1);
}
console.log("apps/mobile/www ready");
