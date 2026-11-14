#!/usr/bin/env node
// SPDX-License-Identifier: GPL-3.0-or-later
//
// Certificat et profil de distribution, créés par l'API App Store Connect (aucune dépendance).
//
//   ASC_KEY_PATH=…/AuthKey_XXXX.p8 ASC_KEY_ID=… ASC_ISSUER_ID=… \
//     node asc-api.mjs prepare --bundle-id <identifiant> --csr <demande.csr> --out <dossier> \
//                              --profile-name "Corde CI 12.1" --profile-prefix "Corde CI "
//
// `prepare` fait, dans l'ordre :
//   1. le ménage de l'exécution précédente : chaque profil dont le nom commence par le préfixe est
//      supprimé, et le certificat de distribution qui lui était attaché est révoqué. Le nom du
//      profil sert d'étiquette : seuls les certificats créés par ce script sont touchés. Le
//      certificat d'une exécution n'est donc révoqué qu'à l'exécution suivante, jamais pendant
//      qu'Apple traite le build qui vient d'être envoyé ;
//   2. un certificat « Apple Distribution » à partir de la demande (CSR) : la clé privée reste sur
//      la machine qui a créé la demande, elle ne passe jamais par ce script ;
//   3. un profil « App Store » pour l'identifiant de l'application, lié à ce certificat.
// Il écrit dans <dossier> : dist.cer, profile.mobileprovision, profile-name.txt, profile-uuid.txt.
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
  for (const profile of profiles) {
    if (!String(profile.attributes?.name ?? "").startsWith(prefix)) continue;
    const certificates = await api.list(
      `/v1/profiles/${profile.id}/certificates?limit=50&fields[certificates]=certificateType`,
    );
    for (const certificate of certificates) {
      // Jamais un certificat géré par Apple, ni de développement : seulement « DISTRIBUTION ».
      if (certificate.attributes?.certificateType !== "DISTRIBUTION") continue;
      try {
        await api.request("DELETE", `/v1/certificates/${certificate.id}`);
        revoked++;
      } catch (error) {
        if (!(error instanceof ApiError) || error.status !== 404) throw error;
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

export async function prepare(api, { bundleId, csrPem, profileName, profilePrefix, outDir }) {
  const cleaned = await cleanPrevious(api, profilePrefix);
  log(
    `Ménage de l'exécution précédente : ${cleaned.profiles} profil(s) supprimé(s), ${cleaned.certificates} certificat(s) révoqué(s).`,
  );

  const bundles = await api.list(
    `/v1/bundleIds?filter[identifier]=${encodeURIComponent(bundleId)}&limit=200&fields[bundleIds]=identifier,platform`,
  );
  const bundle = bundles.find((b) => b.attributes?.identifier === bundleId);
  if (!bundle) {
    throw new Error(
      `l'identifiant ${bundleId} n'existe pas dans le compte développeur (docs/IOS.md, étape 3)`,
    );
  }

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

  const profile = (
    await api.request("POST", "/v1/profiles", {
      data: {
        type: "profiles",
        attributes: { name: profileName, profileType: "IOS_APP_STORE" },
        relationships: {
          bundleId: { data: { type: "bundleIds", id: bundle.id } },
          certificates: { data: [{ type: "certificates", id: certificate.id }] },
        },
      },
    })
  ).data;
  log(`Profil App Store « ${profileName} » créé.`);

  mkdirSync(outDir, { recursive: true, mode: 0o700 });
  const write = (name, data) => writeFileSync(join(outDir, name), data, { mode: 0o600 });
  write("dist.cer", Buffer.from(certificate.attributes.certificateContent, "base64"));
  write("profile.mobileprovision", Buffer.from(profile.attributes.profileContent, "base64"));
  write("profile-name.txt", `${profile.attributes.name}\n`);
  write("profile-uuid.txt", `${profile.attributes.uuid}\n`);
  return { certificateId: certificate.id, profileId: profile.id };
}

function parseArgs(argv) {
  const args = {};
  for (let i = 0; i < argv.length; i++) {
    const key = argv[i];
    if (!key.startsWith("--") || i + 1 >= argv.length) throw new Error(`argument inattendu : ${key}`);
    args[key.slice(2)] = argv[++i];
  }
  return args;
}

async function main() {
  const [command, ...rest] = process.argv.slice(2);
  if (command !== "prepare") throw new Error("usage : asc-api.mjs prepare --bundle-id … --csr … --out …");
  const args = parseArgs(rest);
  for (const name of ["bundle-id", "csr", "out", "profile-name"]) {
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
    bundleId: args["bundle-id"],
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
