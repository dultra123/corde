#!/usr/bin/env node
// SPDX-License-Identifier: GPL-3.0-or-later
//
// Passe la cible de l'application (configuration Release) en signature manuelle, pour la
// compilation TestFlight de la CI. Rien de ceci n'est enregistré dans le dépôt : le projet garde
// la signature automatique, l'équipe et le profil ne sont écrits que sur la machine de la CI.
//
//   node xcode-signing.mjs <project.pbxproj> --bundle-id <identifiant> --team <équipe> \
//                          --profile "<nom du profil>"
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

function main() {
  const [file, ...rest] = process.argv.slice(2);
  const args = {};
  for (let i = 0; i + 1 < rest.length; i += 2) args[rest[i].replace(/^--/, "")] = rest[i + 1];
  if (!file || !args["bundle-id"] || !args.team || !args.profile) {
    throw new Error("usage : xcode-signing.mjs <project.pbxproj> --bundle-id … --team … --profile …");
  }
  const next = manualSigning(readFileSync(file, "utf8"), {
    bundleId: args["bundle-id"],
    team: args.team,
    profile: args.profile,
  });
  writeFileSync(file, next);
  console.log("Projet Xcode : configuration Release de l'application en signature manuelle.");
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? "").href) {
  try {
    main();
  } catch (error) {
    console.error(String(error?.message ?? error));
    process.exit(1);
  }
}
