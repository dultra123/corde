#!/usr/bin/env node
// SPDX-License-Identifier: GPL-3.0-or-later
//
// Passe les cibles de l'application et de son extension de widgets (configuration Release) en
// signature manuelle, chacune avec son profil, pour la compilation TestFlight de la CI. Rien de
// ceci n'est enregistré dans le dépôt : le projet garde la signature automatique, l'équipe et
// les profils ne sont écrits que sur la machine de la CI.
//
//   node xcode-signing.mjs <project.pbxproj> --team <équipe> \
//        --bundle-id <application> --profile "<profil de l'application>" \
//        [--bundle-id <extension> --profile "<profil de l'extension>" …]
//
// Chaque `--bundle-id` va avec le `--profile` qui le suit.
//
// Pourquoi modifier le fichier plutôt que passer les réglages à xcodebuild : sur la ligne de
// commande ils s'appliqueraient aussi aux paquets Swift (Capacitor et ses modules), qui refusent
// un profil de provisionnement.

import { readFileSync, writeFileSync } from "node:fs";
import { pathToFileURL } from "node:url";

const SIGNING_KEYS =
  /^\t\t\t\t"?(?:CODE_SIGN_STYLE|CODE_SIGN_IDENTITY(?:\[[^\]]*\])?|DEVELOPMENT_TEAM|PROVISIONING_PROFILE(?:_SPECIFIER)?)"? = [^\n]*;\n/gm;

const quote = (value) => `"${String(value).replace(/\\/g, "\\\\").replace(/"/g, '\\"')}"`;

const escapeRegExp = (value) => value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

/** Le texte du projet avec la configuration Release de l'application en signature manuelle. */
export function manualSigning(pbx, { bundleId, team, profile, identity = "Apple Distribution" }) {
  for (const [name, value] of Object.entries({ bundleId, team, profile })) {
    if (!value || /[\n\r]/.test(value)) throw new Error(`valeur invalide pour ${name}`);
  }
  const block = /^\t\t[0-9A-F]{24} \/\* [^\n]*? \*\/ = \{\n\t\t\tisa = XCBuildConfiguration;\n[\s\S]*?^\t\t\};\n/gm;
  const ownsBundle = new RegExp(
    `^\\t\\t\\t\\tPRODUCT_BUNDLE_IDENTIFIER = "?${escapeRegExp(bundleId)}"?;$`,
    "m",
  );
  let changed = 0;
  const out = pbx.replace(block, (text) => {
    if (!ownsBundle.test(text) || !/^\t\t\tname = Release;$/m.test(text)) return text;
    const opening = "\t\t\tbuildSettings = {\n";
    if (!text.includes(opening)) return text;
    changed++;
    const settings = [
      `CODE_SIGN_IDENTITY = ${quote(identity)};`,
      `CODE_SIGN_STYLE = Manual;`,
      `DEVELOPMENT_TEAM = ${quote(team)};`,
      `PROVISIONING_PROFILE_SPECIFIER = ${quote(profile)};`,
    ];
    return text
      .replace(SIGNING_KEYS, "")
      .replace(opening, opening + settings.map((line) => `\t\t\t\t${line}\n`).join(""));
  });
  if (changed !== 1) {
    throw new Error(
      `configuration Release de ${bundleId} : ${changed} bloc(s) trouvé(s) dans le projet, 1 attendu`,
    );
  }
  return out;
}

/** `--team`, puis des paires `--bundle-id` / `--profile`, dans l'ordre. */
export function parseSigningArgs(rest) {
  let team = "";
  const pairs = [];
  for (let i = 0; i < rest.length; i += 2) {
    const key = rest[i];
    const value = rest[i + 1];
    if (value === undefined) throw new Error(`valeur manquante après ${key}`);
    if (key === "--team") team = value;
    else if (key === "--bundle-id") pairs.push({ bundleId: value, profile: "" });
    else if (key === "--profile") {
      const last = pairs.at(-1);
      if (!last || last.profile) throw new Error("--profile doit suivre son --bundle-id");
      last.profile = value;
    } else throw new Error(`argument inattendu : ${key}`);
  }
  if (!team || pairs.length === 0 || pairs.some((p) => !p.profile)) {
    throw new Error(
      "usage : xcode-signing.mjs <project.pbxproj> --team … --bundle-id … --profile … [--bundle-id … --profile …]",
    );
  }
  return { team, pairs };
}

/** Toutes les cibles demandées, l'une après l'autre. */
export function manualSigningAll(pbx, { team, pairs }) {
  return pairs.reduce((text, p) => manualSigning(text, { bundleId: p.bundleId, team, profile: p.profile }), pbx);
}

function main() {
  const [file, ...rest] = process.argv.slice(2);
  if (!file) throw new Error("usage : xcode-signing.mjs <project.pbxproj> --team … --bundle-id … --profile …");
  const options = parseSigningArgs(rest);
  writeFileSync(file, manualSigningAll(readFileSync(file, "utf8"), options));
  console.log(
    `Projet Xcode : configuration Release en signature manuelle pour ${options.pairs.map((p) => p.bundleId).join(", ")}.`,
  );
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? "").href) {
  try {
    main();
  } catch (error) {
    console.error(String(error?.message ?? error));
    process.exit(1);
  }
}
