// SPDX-License-Identifier: GPL-3.0-or-later
//
// Cohérence du projet Xcode (apps/mobile/ios/App/App.xcodeproj/project.pbxproj), édité à la main :
// personne dans le projet n'a Xcode. Lit le fichier (format « plist OpenStep ») et vérifie que
// chaque identifiant référencé existe, que les deux cibles (l'application et l'extension de
// widgets CordeWidgets) sont complètes, que l'extension est embarquée, et que les réglages qui
// comptent pour App Store Connect (version, identifiants, iOS minimum, droits) sont les bons.
//   node --test apps/mobile/scripts/xcodeproj.test.mjs

import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const iosApp = join(here, "..", "ios", "App");
const pbxPath = join(iosApp, "App.xcodeproj", "project.pbxproj");
const version = JSON.parse(readFileSync(join(here, "..", "package.json"), "utf8")).version;

const APP_ID = "io.github.dultra123.corde";
const WIDGETS_ID = "io.github.dultra123.corde.widgets";
const GROUP = "$(AppIdentifierPrefix)io.github.dultra123.corde.shared";

/** Lecteur du format plist « OpenStep » d'Xcode : dictionnaires, listes, chaînes. */
export function parsePbx(text) {
  let i = 0;
  const fail = (what) => {
    throw new Error(`${what} à la position ${i} : « ${text.slice(i, i + 30)} »`);
  };
  const skip = () => {
    for (;;) {
      while (i < text.length && /\s/.test(text[i])) i++;
      if (text.startsWith("//", i)) {
        while (i < text.length && text[i] !== "\n") i++;
      } else if (text.startsWith("/*", i)) {
        const end = text.indexOf("*/", i + 2);
        if (end < 0) fail("commentaire non fermé");
        i = end + 2;
      } else return;
    }
  };
  const expect = (c) => {
    skip();
    if (text[i] !== c) fail(`« ${c} » attendu`);
    i++;
  };
  const string = () => {
    skip();
    if (text[i] === '"') {
      i++;
      let out = "";
      while (text[i] !== '"') {
        if (i >= text.length) fail("chaîne non fermée");
        if (text[i] === "\\") {
          const n = text[i + 1];
          out += n === "n" ? "\n" : n === "t" ? "\t" : n;
          i += 2;
        } else out += text[i++];
      }
      i++;
      return out;
    }
    const m = /^[A-Za-z0-9_$/.:\-+]+/.exec(text.slice(i, i + 400));
    if (!m) fail("valeur attendue");
    i += m[0].length;
    return m[0];
  };
  const value = () => {
    skip();
    if (text[i] === "{") {
      i++;
      const obj = {};
      for (;;) {
        skip();
        if (text[i] === "}") {
          i++;
          return obj;
        }
        const key = string();
        if (Object.hasOwn(obj, key)) fail(`clé en double ${key}`);
        expect("=");
        obj[key] = value();
        expect(";");
      }
    }
    if (text[i] === "(") {
      i++;
      const list = [];
      for (;;) {
        skip();
        if (text[i] === ")") {
          i++;
          return list;
        }
        list.push(value());
        skip();
        if (text[i] === ",") i++;
        else if (text[i] !== ")") fail("« , » ou « ) » attendu");
      }
    }
    return string();
  };
  const root = value();
  skip();
  if (i !== text.length) fail("texte après la fin");
  return root;
}

const pbx = parsePbx(readFileSync(pbxPath, "utf8"));
const objects = pbx.objects;
const ofIsa = (isa) => Object.entries(objects).filter(([, o]) => o.isa === isa);
const ID = /^[0-9A-F]{24}$/;

function targetNamed(name) {
  const found = ofIsa("PBXNativeTarget").find(([, t]) => t.name === name);
  assert.ok(found, `cible ${name} absente`);
  return { id: found[0], ...found[1] };
}

function configs(target) {
  const list = objects[target.buildConfigurationList];
  return Object.fromEntries(list.buildConfigurations.map((id) => [objects[id].name, objects[id].buildSettings]));
}

function phase(target, isa) {
  return target.buildPhases.map((id) => ({ id, ...objects[id] })).filter((p) => p.isa === isa);
}

/** Chemin sur le disque d'une référence de fichier (en remontant les groupes). */
function filePath(refId) {
  const parents = new Map();
  for (const [id, o] of Object.entries(objects)) {
    for (const child of o.children ?? []) parents.set(child, id);
  }
  const parts = [];
  let cur = refId;
  while (cur) {
    const o = objects[cur];
    if (o.path && o.sourceTree !== "BUILT_PRODUCTS_DIR") parts.unshift(o.path);
    if (o.sourceTree === "SOURCE_ROOT") break;
    cur = parents.get(cur);
  }
  return join(iosApp, ...parts);
}

const sourceFiles = (target) =>
  phase(target, "PBXSourcesBuildPhase")
    .flatMap((p) => p.files)
    .map((bf) => objects[bf].fileRef)
    .map((ref) => objects[ref].path);

test("le fichier se lit, chaque identifiant référencé existe", () => {
  assert.equal(pbx.objectVersion, "60");
  assert.ok(objects[pbx.rootObject]);
  const missing = [];
  const walk = (v, where) => {
    if (typeof v === "string") {
      if (ID.test(v) && !objects[v]) missing.push(`${v} (${where})`);
    } else if (Array.isArray(v)) v.forEach((x) => walk(x, where));
    else if (v && typeof v === "object") for (const [k, x] of Object.entries(v)) walk(x, `${where}.${k}`);
  };
  for (const [id, o] of Object.entries(objects)) {
    assert.match(id, ID, `identifiant ${id}`);
    walk(o, id);
  }
  assert.deepEqual(missing, []);
});

test("deux cibles : l'application et l'extension de widgets", () => {
  const project = objects[pbx.rootObject];
  const names = project.targets.map((id) => objects[id].name);
  assert.deepEqual(names, ["App", "CordeWidgets"]);
  assert.equal(targetNamed("App").productType, "com.apple.product-type.application");
  const ext = targetNamed("CordeWidgets");
  assert.equal(ext.productType, "com.apple.product-type.app-extension");
  assert.equal(objects[ext.productReference].path, "CordeWidgets.appex");
  assert.equal(objects[ext.productReference].explicitFileType, "wrapper.app-extension");
});

test("l'extension est compilée avant l'application et embarquée dans PlugIns", () => {
  const app = targetNamed("App");
  const ext = targetNamed("CordeWidgets");
  const deps = app.dependencies.map((id) => objects[id]);
  assert.ok(deps.some((d) => d.target === ext.id && objects[d.targetProxy].remoteGlobalIDString === ext.id));
  const embed = phase(app, "PBXCopyFilesBuildPhase");
  assert.equal(embed.length, 1);
  assert.equal(embed[0].name, "Embed Foundation Extensions");
  assert.equal(embed[0].dstSubfolderSpec, "13"); // PlugIns
  assert.deepEqual(
    embed[0].files.map((bf) => objects[bf].fileRef),
    [ext.productReference],
  );
  assert.deepEqual(objects[embed[0].files[0]].settings, { ATTRIBUTES: ["RemoveHeadersOnCopy"] });
});

test("sources : le code partagé dans les deux cibles, chaque fichier existe", () => {
  const app = sourceFiles(targetNamed("App"));
  const ext = sourceFiles(targetNamed("CordeWidgets"));
  const shared = [
    "SharedStore.swift",
    "WidgetData.swift",
    "CordeRaceAttributes.swift",
    "CordeStyle.swift",
    "WidgetViews.swift",
    "ActivityViews.swift",
    "SampleData.swift",
  ];
  for (const f of shared) {
    assert.ok(app.includes(f), `${f} dans l'application`);
    assert.ok(ext.includes(f), `${f} dans l'extension`);
  }
  for (const f of ["AppDelegate.swift", "SceneDelegate.swift", "CordeNativePlugin.swift", "CordeBridgeViewController.swift", "WidgetShots.swift"]) {
    assert.ok(app.includes(f), f);
    assert.ok(!ext.includes(f), `${f} hors de l'extension`);
  }
  assert.ok(ext.includes("CordeWidgets.swift") && ext.includes("CordeLiveActivity.swift"));
  assert.ok(!app.includes("CordeWidgets.swift"), "le @main de l'extension hors de l'application");
  // (Les produits et les fichiers écrits par `cap sync` — public, config.xml — ne sont pas
  // dans le dépôt : seuls le code et les fichiers de réglage sont contrôlés.)
  for (const [id, ref] of ofIsa("PBXFileReference")) {
    if (ref.lastKnownFileType === "sourcecode.swift" || /entitlements|Info\.plist/.test(ref.path)) {
      assert.ok(existsSync(filePath(id)), `fichier absent : ${filePath(id)}`);
    }
  }
});

test("réglages : version, identifiants, iOS 17, droits, Info.plist", () => {
  const expected = {
    App: { id: APP_ID, plist: "App/Info.plist", ent: "App/App.entitlements" },
    CordeWidgets: { id: WIDGETS_ID, plist: "CordeWidgets/Info.plist", ent: "CordeWidgets/CordeWidgets.entitlements" },
  };
  for (const [name, want] of Object.entries(expected)) {
    const all = configs(targetNamed(name));
    assert.deepEqual(Object.keys(all).sort(), ["Debug", "Release"]);
    for (const [conf, s] of Object.entries(all)) {
      const where = `${name}/${conf}`;
      assert.equal(s.PRODUCT_BUNDLE_IDENTIFIER, want.id, where);
      assert.equal(s.MARKETING_VERSION, version, `${where} : même version que package.json`);
      assert.equal(s.IPHONEOS_DEPLOYMENT_TARGET, "17.0", where);
      assert.equal(s.INFOPLIST_FILE, want.plist, where);
      assert.equal(s.CODE_SIGN_ENTITLEMENTS, want.ent, where);
      assert.equal(s.CODE_SIGN_STYLE, "Automatic", `${where} : signature manuelle seulement sur la CI`);
      assert.equal(s.TARGETED_DEVICE_FAMILY, "1", where);
      assert.ok(!s.DEVELOPMENT_TEAM, `${where} : aucune équipe dans le dépôt`);
      assert.ok(!s.PROVISIONING_PROFILE_SPECIFIER, where);
    }
    const plist = readFileSync(join(iosApp, want.plist), "utf8");
    assert.match(plist, /<key>CFBundleShortVersionString<\/key>\s*<string>\$\(MARKETING_VERSION\)<\/string>/);
    assert.match(plist, /<key>CFBundleVersion<\/key>\s*<string>\$\(CURRENT_PROJECT_VERSION\)<\/string>/);
    assert.ok(plist.includes(`<key>CordeKeychainGroup</key>\n\t<string>${GROUP}</string>`), `${want.plist} : groupe`);
    const ent = readFileSync(join(iosApp, want.ent), "utf8");
    assert.match(ent, /<key>keychain-access-groups<\/key>\s*<array>\s*<string>([^<]+)<\/string>\s*<\/array>/);
    assert.ok(ent.includes(`<string>${GROUP}</string>`), want.ent);
    // Rien d'autre : pas d'App Group, pas de notification push (ADR 0012).
    assert.equal((ent.match(/<key>/g) ?? []).length, 1, `${want.ent} : un seul droit`);
  }
  const ext = configs(targetNamed("CordeWidgets"));
  for (const s of Object.values(ext)) {
    assert.equal(s.APPLICATION_EXTENSION_API_ONLY, "YES");
    assert.equal(s.SKIP_INSTALL, "YES");
  }
  const project = configs(objects[pbx.rootObject]);
  for (const s of Object.values(project)) assert.equal(s.IPHONEOS_DEPLOYMENT_TARGET, "17.0");

  const widgetsPlist = readFileSync(join(iosApp, "CordeWidgets/Info.plist"), "utf8");
  assert.match(widgetsPlist, /<key>NSExtensionPointIdentifier<\/key>\s*<string>com\.apple\.widgetkit-extension<\/string>/);
  const appPlist = readFileSync(join(iosApp, "App/Info.plist"), "utf8");
  assert.match(appPlist, /<key>NSSupportsLiveActivities<\/key>\s*<true\/>/);
});

test("le storyboard et la scène utilisent le contrôleur de Corde", () => {
  const storyboard = readFileSync(join(iosApp, "App/Base.lproj/Main.storyboard"), "utf8");
  assert.match(storyboard, /customClass="CordeBridgeViewController" customModule="App"/);
  const scene = readFileSync(join(iosApp, "App/SceneDelegate.swift"), "utf8");
  assert.match(scene, /rootViewController = CordeBridgeViewController\(\)/);
});
