#!/usr/bin/env node
// SPDX-License-Identifier: GPL-3.0-or-later
//
// Certificat et profils de distribution, créés par l'API App Store Connect (aucune dépendance).
//
//   ASC_KEY_PATH=…/AuthKey_XXXX.p8 ASC_KEY_ID=… ASC_ISSUER_ID=… \
//     node asc-api.mjs prepare --bundle-id <application> [--bundle-id <extension> …] \
//                              --csr <demande.csr> --out <dossier> \
//                              --profile-name "Corde CI 12.1" --profile-prefix "Corde CI "
//
// `prepare` fait, dans l'ordre :
//   1. retrouve chaque identifiant dans le compte. Le premier est celui de l'application : s'il
//      manque, arrêt avec un message clair (il se crée à la main, docs/IOS.md, étape 3). Les
//      suivants (l'extension de widgets) sont créés par l'API s'ils manquent (plateforme iOS,
//      nom « Corde Widgets ») ;
//   2. le ménage de l'exécution précédente : chaque profil dont le nom commence par le préfixe est
//      supprimé, et le certificat de distribution qui lui était attaché est révoqué. Le nom du
//      profil sert d'étiquette : seuls les certificats créés par ce script sont touchés. Le
//      certificat d'une exécution n'est donc révoqué qu'à l'exécution suivante, jamais pendant
//      qu'Apple traite le build qui vient d'être envoyé ;
//   3. un seul certificat « Apple Distribution » à partir de la demande (CSR) : la clé privée
//      reste sur la machine qui a créé la demande, elle ne passe jamais par ce script ;
//   4. un profil « App Store » par identifiant, lié à ce certificat : « Corde CI 12.1 » pour
//      l'application, « Corde CI 12.1 widgets » pour l'extension (même préfixe, donc retrouvés
//      par le ménage suivant).
// Il écrit dans <dossier> : dist.cer, et pour chaque identifiant un dossier <identifiant>/ avec
// profile.mobileprovision, profile-name.txt, profile-uuid.txt.
//
// Le nom du certificat contient le nom du titulaire du compte : il n'est jamais affiché, et sous
// GitHub Actions il est masqué dans la suite du journal (`::add-mask::`).

import { createPrivateKey, sign } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

const DEFAULT_BASE = "https://api.appstoreconnect.apple.com";

const b64url = (value) => Buffer.from(value).toString("base64url");

/** Jeton ES256 de l'API App Store Connect (20 minutes au plus ; 10 ici). */
export function makeToken({ keyPem, keyId, issuerId, now = Date.now() }) {
  const iat = Math.floor(now / 1000);
  const header = b64url(JSON.stringify({ alg: "ES256", kid: keyId, typ: "JWT" }));
  const payload = b64url(
    JSON.stringify({ iss: issuerId, iat, exp: iat + 600, aud: "appstoreconnect-v1" }),
  );
  const signature = sign("sha256", Buffer.from(`${header}.${payload}`), {
    key: createPrivateKey(keyPem),
    dsaEncoding: "ieee-p1363",
  });
  return `${header}.${payload}.${signature.toString("base64url")}`;
}

export class ApiError extends Error {
  constructor(method, path, status, body) {
    const errors = Array.isArray(body?.errors) ? body.errors : [];
    const detail = errors.length
      ? errors.map((e) => [e.code, e.title, e.detail].filter(Boolean).join(" — ")).join(" | ")
      : "réponse sans détail";
    super(`${method} ${path.split("?")[0]} → ${status} : ${detail}`);
    this.status = status;
    this.codes = errors.map((e) => String(e.code ?? ""));
  }
}

export function client({ keyPem, keyId, issuerId, base = DEFAULT_BASE }) {
  let token = "";
  let tokenAt = 0;
  const bearer = () => {
    if (Date.now() - tokenAt > 5 * 60 * 1000) {
      token = makeToken({ keyPem, keyId, issuerId });
      tokenAt = Date.now();
    }
    return token;
  };

  async function request(method, path, body) {
    // Le jeton ne part que vers l'API : une adresse absolue (page suivante) doit être chez elle.
    const url = /^https?:/.test(path) ? path : base + path;
    if (!url.startsWith(`${base}/`)) throw new Error(`adresse hors de l'API : refusée`);
    let res;
    try {
      res = await fetch(url, {
        method,
        headers: {
          Authorization: `Bearer ${bearer()}`,
          Accept: "application/json",
          ...(body ? { "Content-Type": "application/json" } : {}),
        },
        body: body ? JSON.stringify(body) : undefined,
      });
    } catch (error) {
      const why = error?.cause?.code ?? error?.cause?.message ?? error?.message ?? "erreur réseau";
      throw new Error(`${method} ${path.split("?")[0]} : l'API d'Apple n'a pas répondu (${why})`);
    }
    const text = await res.text();
    let json = null;
    try {
      json = text ? JSON.parse(text) : null;
    } catch {
      json = null;
    }
    if (!res.ok) throw new ApiError(method, path, res.status, json);
    return json;
  }

  /** Toutes les pages d'une liste. */
  async function list(path) {
    const items = [];
    let next = path;
    for (let page = 0; next && page < 20; page++) {
      const json = await request("GET", next);
      items.push(...(json?.data ?? []));
      next = json?.links?.next ?? null;
    }
    return items;
  }

  return { request, list };
}

const log = (line) => console.log(line);

/** Sous GitHub Actions, masque une valeur dans la suite du journal. */
function mask(value) {
  if (process.env.GITHUB_ACTIONS === "true" && value && value.length >= 4) {
    console.log(`::add-mask::${value}`);
  }
}

/** Supprime les profils de ce script et révoque leurs certificats de distribution. */
export async function cleanPrevious(api, prefix) {
  if (!prefix) return { profiles: 0, certificates: 0 };
  const profiles = await api.list("/v1/profiles?limit=200&fields[profiles]=name");
  let removed = 0;
  let revoked = 0;
  const handled = new Set();
  for (const profile of profiles) {
    if (!String(profile.attributes?.name ?? "").startsWith(prefix)) continue;
    const certificates = await api.list(
      `/v1/profiles/${profile.id}/certificates?limit=50&fields[certificates]=certificateType`,
    );
    for (const certificate of certificates) {
      // Jamais un certificat géré par Apple, ni de développement : seulement « DISTRIBUTION ».
      if (certificate.attributes?.certificateType !== "DISTRIBUTION") continue;
      // Les profils d'une même exécution (application, widgets) portent le même certificat :
      // il n'est révoqué qu'une fois.
      if (handled.has(certificate.id)) continue;
      handled.add(certificate.id);
      try {
        await api.request("DELETE", `/v1/certificates/${certificate.id}`);
        revoked++;
      } catch (error) {
        // 404 : il n'existe plus. 409 : Apple le liste encore sur le profil alors qu'il est déjà
        // révoqué (« not in an "issued" state ») — un ménage interrompu, ou un certificat révoqué
        // à la main. Dans les deux cas il n'y a plus rien à révoquer.
        if (!(error instanceof ApiError) || (error.status !== 404 && error.status !== 409)) throw error;
      }
    }
    try {
      await api.request("DELETE", `/v1/profiles/${profile.id}`);
      removed++;
    } catch (error) {
      if (!(error instanceof ApiError) || error.status !== 404) throw error;
    }
  }
  return { profiles: removed, certificates: revoked };
}

/** Noms donnés aux identifiants créés par ce script (seulement ceux des extensions). */
export const EXTENSION_NAMES = { widgets: "Corde Widgets" };

/** « Corde Widgets » pour `….widgets` ; le dernier mot de l'identifiant sinon. */
export function extensionName(bundleId) {
  const last = bundleId.split(".").at(-1) ?? "";
  return EXTENSION_NAMES[last] ?? `Corde ${last.charAt(0).toUpperCase()}${last.slice(1)}`;
}

/** Nom du profil d'un identifiant : celui de l'application tel quel, « … widgets » pour l'extension. */
export function profileNameFor(profileName, bundleIds, bundleId) {
  if (bundleId === bundleIds[0]) return profileName;
  return `${profileName} ${bundleId.split(".").at(-1)}`;
}

const BUNDLE_ID = /^[A-Za-z0-9][A-Za-z0-9.-]{0,154}$/;

/**
 * L'identifiant `bundleId` du compte. Absent : erreur pour l'application (`create` faux), création
 * par l'API pour une extension.
 */
async function findBundle(api, bundleId, create) {
  const bundles = await api.list(
    `/v1/bundleIds?filter[identifier]=${encodeURIComponent(bundleId)}&limit=200&fields[bundleIds]=identifier,platform`,
  );
  const bundle = bundles.find((b) => b.attributes?.identifier === bundleId);
  if (bundle) return bundle;
  if (!create) {
    throw new Error(
      `l'identifiant ${bundleId} n'existe pas dans le compte développeur (docs/IOS.md, étape 3)`,
    );
  }
  const name = extensionName(bundleId);
  const created = (
    await api.request("POST", "/v1/bundleIds", {
      data: {
        type: "bundleIds",
        attributes: { identifier: bundleId, name, platform: "IOS" },
      },
    })
  ).data;
  log(`Identifiant ${bundleId} créé dans le compte (« ${name} »).`);
  return created;
}

export async function prepare(api, { bundleIds, csrPem, profileName, profilePrefix, outDir }) {
  const ids = Array.isArray(bundleIds) ? bundleIds : [bundleIds];
  if (ids.length === 0) throw new Error("aucun identifiant");
  for (const id of ids) {
    if (!BUNDLE_ID.test(id) || id.includes("..")) throw new Error(`identifiant invalide : ${id}`);
  }
  if (new Set(ids).size !== ids.length) throw new Error("identifiant donné deux fois");

  // 1. Les identifiants, avant de toucher à quoi que ce soit : l'application doit exister.
  const bundles = [];
  for (const [i, id] of ids.entries()) bundles.push(await findBundle(api, id, i > 0));

  // 2. Le ménage de l'exécution précédente (règle inchangée : seuls les certificats
  //    DISTRIBUTION attachés à un profil dont le nom commence par le préfixe).
  const cleaned = await cleanPrevious(api, profilePrefix);
  log(
    `Ménage de l'exécution précédente : ${cleaned.profiles} profil(s) supprimé(s), ${cleaned.certificates} certificat(s) révoqué(s).`,
  );

  // 3. Un seul certificat pour tous les profils.
  let certificate;
  try {
    certificate = (
      await api.request("POST", "/v1/certificates", {
        data: {
          type: "certificates",
          attributes: { certificateType: "DISTRIBUTION", csrContent: csrPem },
        },
      })
    ).data;
  } catch (error) {
    if (error instanceof ApiError && error.status === 409) {
      throw new Error(
        `${error.message}\nLe compte a sans doute atteint le nombre maximal de certificats de distribution : en révoquer un sur developer.apple.com → Certificates, IDs & Profiles → Certificates.`,
      );
    }
    throw error;
  }
  for (const key of ["name", "displayName"]) {
    const value = String(certificate.attributes?.[key] ?? "");
    mask(value);
    // « Apple Distribution: Prénom Nom (ÉQUIPE) » : masquer aussi le nom seul.
    const bare = value.replace(/^[^:]*:\s*/, "").replace(/\s*\([A-Z0-9]{10}\)\s*$/, "");
    if (bare !== value) mask(bare);
  }
  log(
    `Certificat de distribution créé (valable jusqu'au ${String(certificate.attributes?.expirationDate ?? "?").slice(0, 10)}).`,
  );

  // 4. Un profil App Store par identifiant.
  mkdirSync(outDir, { recursive: true, mode: 0o700 });
  writeFileSync(join(outDir, "dist.cer"), Buffer.from(certificate.attributes.certificateContent, "base64"), {
    mode: 0o600,
  });
  const profiles = [];
  for (const [i, id] of ids.entries()) {
    const name = profileNameFor(profileName, ids, id);
    const profile = (
      await api.request("POST", "/v1/profiles", {
        data: {
          type: "profiles",
          attributes: { name, profileType: "IOS_APP_STORE" },
          relationships: {
            bundleId: { data: { type: "bundleIds", id: bundles[i].id } },
            certificates: { data: [{ type: "certificates", id: certificate.id }] },
          },
        },
      })
    ).data;
    log(`Profil App Store « ${name} » créé (${id}).`);
    const dir = join(outDir, id);
    mkdirSync(dir, { recursive: true, mode: 0o700 });
    const write = (file, data) => writeFileSync(join(dir, file), data, { mode: 0o600 });
    write("profile.mobileprovision", Buffer.from(profile.attributes.profileContent, "base64"));
    write("profile-name.txt", `${profile.attributes.name}\n`);
    write("profile-uuid.txt", `${profile.attributes.uuid}\n`);
    profiles.push(profile.id);
  }
  return { certificateId: certificate.id, profileIds: profiles };
}

/** `--bundle-id` peut être répété ; les autres options une fois. */
export function parseArgs(argv) {
  const args = { "bundle-id": [] };
  for (let i = 0; i < argv.length; i++) {
    const key = argv[i];
    if (!key.startsWith("--") || i + 1 >= argv.length) throw new Error(`argument inattendu : ${key}`);
    const name = key.slice(2);
    const value = argv[++i];
    if (name === "bundle-id") args[name].push(value);
    else if (Object.hasOwn(args, name)) throw new Error(`--${name} donné deux fois`);
    else args[name] = value;
  }
  return args;
}

async function main() {
  const [command, ...rest] = process.argv.slice(2);
  if (command !== "prepare") {
    throw new Error("usage : asc-api.mjs prepare --bundle-id … [--bundle-id …] --csr … --out …");
  }
  const args = parseArgs(rest);
  if (args["bundle-id"].length === 0) throw new Error("il manque --bundle-id");
  for (const name of ["csr", "out", "profile-name"]) {
    if (!args[name]) throw new Error(`il manque --${name}`);
  }
  for (const name of ["ASC_KEY_PATH", "ASC_KEY_ID", "ASC_ISSUER_ID"]) {
    if (!process.env[name]) throw new Error(`il manque la variable ${name}`);
  }
  const api = client({
    keyPem: readFileSync(process.env.ASC_KEY_PATH, "utf8"),
    keyId: process.env.ASC_KEY_ID,
    issuerId: process.env.ASC_ISSUER_ID,
    base: process.env.ASC_API_BASE || DEFAULT_BASE,
  });
  await prepare(api, {
    bundleIds: args["bundle-id"],
    csrPem: readFileSync(args.csr, "utf8"),
    profileName: args["profile-name"],
    profilePrefix: args["profile-prefix"] ?? "",
    outDir: args.out,
  });
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? "").href) {
  main().catch((error) => {
    const message = String(error?.message ?? error);
    console.error(process.env.GITHUB_ACTIONS === "true" ? `::error::${message.split("\n").join("%0A")}` : message);
    process.exit(1);
  });
}
