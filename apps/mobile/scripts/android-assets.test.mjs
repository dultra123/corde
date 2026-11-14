/**
 * The images and the sound of the Android project are those `make-assets.mjs` writes
 * (`android-assets.mjs`): regenerate with `node apps/mobile/scripts/make-assets.mjs --android`.
 *
 *   node --test apps/mobile/scripts/android-assets.test.mjs
 */
import assert from "node:assert/strict";
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { androidResources, resDir, SOUND_SOURCE, SOUND_TARGET } from "./android-assets.mjs";

const mobile = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const res = resDir(mobile);

test("the vector drawables of the project are the generated ones", () => {
  for (const [path, text] of Object.entries(androidResources())) {
    const file = join(res, path);
    assert.ok(existsSync(file), `${path} missing: node apps/mobile/scripts/make-assets.mjs --android`);
    assert.equal(readFileSync(file, "utf8"), text, `${path} differs from make-assets.mjs`);
  }
});

test("the raw sound is the reminders' sound, under a valid resource name", () => {
  assert.deepEqual(readFileSync(join(mobile, SOUND_TARGET)), readFileSync(join(mobile, SOUND_SOURCE)));
  assert.match(SOUND_TARGET, /\/res\/raw\/[a-z0-9_]+\.wav$/);
});

test("no bitmap launcher icon nor template splash is left (adaptive icon only, SDK 26+)", () => {
  const dirs = readdirSync(res);
  assert.deepEqual(
    dirs.filter((d) => d.startsWith("mipmap")),
    ["mipmap-anydpi"],
  );
  assert.ok(!dirs.some((d) => /^drawable-(land|port)/.test(d)), "template splash images left");
  for (const file of readdirSync(join(res, "drawable"))) {
    assert.match(file, /\.xml$/, `bitmap in drawable/: ${file}`);
  }
});

test("each resource name is valid for Android", () => {
  for (const path of Object.keys(androidResources())) {
    assert.match(path, /^[a-z-]+\/[a-z0-9_]+\.xml$/);
  }
});
