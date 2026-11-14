// SPDX-License-Identifier: GPL-3.0-or-later
//
// Essais des scripts de signature de la CI iOS (asc-api.mjs, xcode-signing.mjs).
//   node --test apps/mobile/scripts/signing.test.mjs
// L'API d'Apple est remplacée par un petit serveur local : ces essais vérifient l'enchaînement et
// la forme des requêtes, pas le comportement réel d'Apple. Les certificats sont fabriqués avec
// `openssl` (présent sur le Mac de la CI comme sous Linux).

import assert from "node:assert/strict";
import { execFile, execFileSync } from "node:child_process";
import { X509Certificate, createPublicKey, generateKeyPairSync, verify } from "node:crypto";
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { after, test } from "node:test";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";

import {
  ApiError,
  RENEW_MARGIN_MS,
  certificateInfo,
  cleanPrevious,
  client,
  extensionName,
  makeToken,
  parseArgs,
  prepare,
  stableKeyFrom,
  subjectNames,
} from "./asc-api.mjs";
import { manualSigning, manualSigningAll, parseSigningArgs } from "./xcode-signing.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const execFileAsync = promisify(execFile);
const { privateKey, publicKey } = generateKeyPairSync("ec", { namedCurve: "P-256" });
const keyPem = privateKey.export({ type: "pkcs8", format: "pem" });
const BUNDLE = "io.github.dultra123.corde";
const WIDGETS = "io.github.dultra123.corde.widgets";
const CSR = "-----BEGIN CERTIFICATE REQUEST-----\nMIIB\n-----END CERTIFICATE REQUEST-----\n";

test("le jeton est un JWT ES256 vérifiable, valable 10 minutes", () => {
  const now = Date.UTC(2026, 10, 14, 10, 0, 0);
  const token = makeToken({ keyPem, keyId: "ABCDE12345", issuerId: "issuer-1", now });
  const [header, payload, signature] = token.split(".");
  assert.deepEqual(JSON.parse(Buffer.from(header, "base64url")), {
    alg: "ES256",
    kid: "ABCDE12345",
    typ: "JWT",
  });
  const claims = JSON.parse(Buffer.from(payload, "base64url"));
  assert.equal(claims.iss, "issuer-1");
  assert.equal(claims.aud, "appstoreconnect-v1");
  assert.equal(claims.iat, now / 1000);
  assert.equal(claims.exp - claims.iat, 600);
  const raw = Buffer.from(signature, "base64url");
  assert.equal(raw.length, 64);
  assert.ok(
    verify(
      "sha256",
      Buffer.from(`${header}.${payload}`),
      { key: publicKey, dsaEncoding: "ieee-p1363" },
      raw,
    ),
  );
});

// --- Clés et certificats jetables, créés pour ces essais ----------------------------------------
// Aucune vraie clé, aucun vrai certificat : des clés RSA 2048 tirées ici, et des certificats
// auto-signés fabriqués par `openssl req -x509` dans un dossier temporaire effacé à la fin. Le
// sujet imite celui d'Apple (« Apple Distribution: <nom> (<équipe>) ») avec un nom inventé, pour
// vérifier qu'il n'est jamais affiché.

const certDir = mkdtempSync(join(tmpdir(), "corde-certs-"));
after(() => rmSync(certDir, { recursive: true, force: true }));
const rsaPem = () =>
  generateKeyPairSync("rsa", { modulusLength: 2048 }).privateKey.export({ type: "pkcs8", format: "pem" });
let certCount = 0;
/** Certificat auto-signé de la clé `pem`, valable `days` jours : { content (base64 DER), expires }. */
function makeCert(pem, days) {
  certCount++;
  const key = join(certDir, `k${certCount}.pem`);
  const out = join(certDir, `c${certCount}.der`);
  writeFileSync(key, pem, { mode: 0o600 });
  execFileSync(
    "openssl",
    ["req", "-x509", "-new", "-key", key, "-subj", "/CN=Apple Distribution: Camille Exemple (ZYXWV98765)",
      "-days", String(days), "-outform", "DER", "-out", out],
    { stdio: ["ignore", "ignore", "pipe"] },
  );
  const der = readFileSync(out);
  return { content: der.toString("base64"), expires: new Date(new X509Certificate(der).validTo).toISOString() };
}
const spkiOf = (content) =>
  new X509Certificate(Buffer.from(content, "base64")).publicKey.export({ type: "spki", format: "der" });

const STABLE = rsaPem(); // la clé du secret APPLE_DIST_KEY_BASE64, dans ces essais
const STABLE_SPKI = createPublicKey(STABLE).export({ type: "spki", format: "der" });
const OTHER = rsaPem(); // la clé d'une exécution de l'ancien mode
const certs = {
  stable: makeCert(STABLE, 365),
  stableSoon: makeCert(STABLE, 10), // expire dans moins de 30 jours
  other: makeCert(OTHER, 365),
  other2: makeCert(rsaPem(), 365),
};

/**
 * Faux App Store Connect : un compte avec l'identifiant de l'application, et un profil et un
 * certificat laissés par l'exécution précédente.
 */
function fakeApple() {
  const state = {
    calls: [],
    // Les mêmes requêtes avec leurs paramètres (« ?… »), pour comparer à l'identique.
    urls: [],
    profiles: [
      { id: "P-OLD", name: "Corde CI 4.1", certificates: ["C-OLD", "C-MANAGED"] },
      { id: "P-OLD-W", name: "Corde CI 4.1 widgets", certificates: ["C-OLD"] },
      { id: "P-OTHER", name: "Profil d'un autre outil", certificates: ["C-OTHER"] },
    ],
    // Un certificat : son type, son contenu (base64 du DER) et son expiration.
    certificates: {
      "C-OLD": { type: "DISTRIBUTION", ...certs.other },
      // Géré par Apple, mais avec la clé stable : ni réutilisé, ni révoqué.
      "C-MANAGED": { type: "DISTRIBUTION_MANAGED", ...certs.stable },
      "C-OTHER": { type: "DISTRIBUTION", ...certs.other2 },
    },
    // `filter[identifier]` d'Apple ne compare pas à l'identique : un identifiant plus long revient
    // aussi (d'où la recherche exacte du script).
    bundles: [
      { id: "B-LONGER", identifier: `${BUNDLE}.widgetsdemo` },
      { id: "B-APP", identifier: BUNDLE },
    ],
    // Comme chez Apple : un certificat révoqué reste listé sur les profils qui le portaient, et
    // le révoquer une seconde fois est refusé (409).
    revoked: new Set(),
    created: { profiles: [], bundles: [], certificates: [] },
    failCertificate: false,
    // Certificat rendu par POST /v1/certificates : { content, expires } ; par défaut un faux.
    issue: null,
    // Formes de réponse qu'on ne connaît pas encore pour sûr chez Apple :
    omitContentInLists: false, // les listes ne donnent pas `certificateContent`
    ignoreTypeFilter: false, // `filter[certificateType]` sans effet
    rejectTypeFilter: false, // `filter[certificateType]` refusé (400)
    omitNames: false, // `name` et `displayName` jamais rendus, même demandés
  };
  // Comme chez Apple : avec `fields[certificates]=…`, seuls les champs demandés reviennent.
  const certificateJson = (id, withContent = true, fields = null) => {
    const c = state.certificates[id];
    const all = {
      certificateType: c.type,
      ...(state.omitNames
        ? {}
        : { name: "Apple Distribution: Camille Exemple (ZYXWV98765)", displayName: "Camille Exemple" }),
      expirationDate: c.expires,
      ...(withContent && c.content ? { certificateContent: c.content } : {}),
    };
    const wanted = fields ? fields.split(",") : null;
    const attributes = wanted ? Object.fromEntries(Object.entries(all).filter(([k]) => wanted.includes(k))) : all;
    return { type: "certificates", id, attributes };
  };
  const server = createServer((req, res) => {
    let body = "";
    req.on("data", (chunk) => (body += chunk));
    req.on("end", () => {
      const url = new URL(req.url, "http://localhost");
      const path = url.pathname;
      state.calls.push(`${req.method} ${path}`);
      state.urls.push(`${req.method} ${decodeURIComponent(path + url.search)}`);
      const send = (status, json) => {
        res.writeHead(status, { "Content-Type": "application/json" });
        res.end(json === undefined ? "" : JSON.stringify(json));
      };
      if (!/^Bearer [\w-]+\.[\w-]+\.[\w-]+$/.test(req.headers.authorization ?? "")) {
        return send(401, { errors: [{ code: "NOT_AUTHORIZED", title: "no token" }] });
      }
      let match;
      if (req.method === "GET" && path === "/v1/profiles") {
        // Une page par profil, pour exercer la pagination.
        const page = Number(url.searchParams.get("page") ?? "0");
        const item = state.profiles[page];
        const next = page + 1 < state.profiles.length ? `${base}/v1/profiles?page=${page + 1}` : undefined;
        return send(200, {
          data: item ? [{ type: "profiles", id: item.id, attributes: { name: item.name } }] : [],
          links: { next },
        });
      }
      if (req.method === "GET" && (match = path.match(/^\/v1\/profiles\/([\w-]+)\/certificates$/))) {
        const profile = state.profiles.find((p) => p.id === match[1]);
        return send(200, {
          data: (profile?.certificates ?? [])
            .filter((id) => state.certificates[id])
            .map((id) => certificateJson(id, !state.omitContentInLists, url.searchParams.get("fields[certificates]"))),
        });
      }
      if (req.method === "GET" && path === "/v1/certificates") {
        const type = url.searchParams.get("filter[certificateType]");
        if (type && state.rejectTypeFilter) {
          return send(400, { errors: [{ code: "PARAMETER_ERROR.INVALID", title: "bad filter" }] });
        }
        // Un certificat révoqué n'est plus dans la liste du compte.
        return send(200, {
          data: Object.keys(state.certificates)
            .filter((id) => !state.revoked.has(id))
            .filter((id) => !type || state.ignoreTypeFilter || state.certificates[id].type === type)
            .map((id) => certificateJson(id, !state.omitContentInLists, url.searchParams.get("fields[certificates]"))),
        });
      }
      if (req.method === "GET" && (match = path.match(/^\/v1\/certificates\/([\w-]+)$/))) {
        if (!state.certificates[match[1]]) return send(404, { errors: [{ code: "NOT_FOUND" }] });
        return send(200, { data: certificateJson(match[1], true, url.searchParams.get("fields[certificates]")) });
      }
      if (req.method === "DELETE" && (match = path.match(/^\/v1\/certificates\/([\w-]+)$/))) {
        if (!state.certificates[match[1]]) return send(404, { errors: [{ code: "NOT_FOUND" }] });
        if (state.revoked.has(match[1])) {
          return send(409, {
            errors: [
              {
                code: "ENTITY_ERROR",
                title: "There is a problem with the request entity",
                detail: 'This certificate cannot be revoked because it is not in an "issued" state.',
              },
            ],
          });
        }
        state.revoked.add(match[1]);
        return send(204);
      }
      if (req.method === "DELETE" && (match = path.match(/^\/v1\/profiles\/([\w-]+)$/))) {
        state.profiles = state.profiles.filter((p) => p.id !== match[1]);
        return send(204);
      }
      if (req.method === "GET" && path === "/v1/bundleIds") {
        const wanted = url.searchParams.get("filter[identifier]") ?? "";
        return send(200, {
          data: state.bundles
            .filter((b) => b.identifier.startsWith(wanted))
            .map((b) => ({ type: "bundleIds", id: b.id, attributes: { identifier: b.identifier } })),
        });
      }
      if (req.method === "POST" && path === "/v1/bundleIds") {
        const data = JSON.parse(body).data;
        state.created.bundles.push(data);
        const id = `B-NEW-${state.bundles.length}`;
        state.bundles.push({ id, identifier: data.attributes.identifier });
        return send(201, { data: { type: "bundleIds", id, attributes: data.attributes } });
      }
      if (req.method === "POST" && path === "/v1/certificates") {
        if (state.failCertificate) {
          return send(409, {
            errors: [{ code: "ENTITY_ERROR", title: "There is a problem with the request entity", detail: "You already have a current Distribution certificate" }],
          });
        }
        state.created.certificate = JSON.parse(body);
        state.created.certificates.push(state.created.certificate);
        const n = state.created.certificates.length;
        const id = n === 1 ? "C-NEW" : `C-NEW-${n}`;
        const issued = state.issue?.() ?? {
          content: Buffer.from("faux certificat DER").toString("base64"),
          expires: "2027-11-14T10:00:00.000+00:00",
        };
        state.certificates[id] = { type: "DISTRIBUTION", ...issued };
        return send(201, { data: certificateJson(id) });
      }
      if (req.method === "POST" && path === "/v1/profiles") {
        const created = JSON.parse(body);
        state.created.profiles.push(created);
        const n = state.created.profiles.length;
        const name = created.data.attributes.name;
        const attached = created.data.relationships.certificates.data.map((c) => c.id);
        state.profiles.push({ id: `P-NEW-${n}`, name, certificates: attached });
        return send(201, {
          data: {
            type: "profiles",
            id: `P-NEW-${n}`,
            attributes: {
              name,
              uuid: `${n}1111111-2222-3333-4444-555555555555`,
              profileContent: Buffer.from(`faux profil ${n}`).toString("base64"),
            },
          },
        });
      }
      return send(404, { errors: [{ code: "NOT_FOUND", title: "unknown path" }] });
    });
  });
  let base = "";
  return {
    state,
    async start() {
      await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
      base = `http://127.0.0.1:${server.address().port}`;
      return base;
    },
    stop: () => new Promise((resolve) => server.close(resolve)),
  };
}

async function captureLog(run) {
  const lines = [];
  const original = console.log;
  console.log = (line) => lines.push(String(line));
  try {
    await run();
  } finally {
    console.log = original;
  }
  return lines;
}

async function withApple(run) {
  const apple = fakeApple();
  const base = await apple.start();
  const out = mkdtempSync(join(tmpdir(), "corde-signing-"));
  const previous = process.env.GITHUB_ACTIONS;
  process.env.GITHUB_ACTIONS = "true";
  try {
    const api = client({ keyPem, keyId: "ABCDE12345", issuerId: "issuer-1", base });
    await run({ apple, api, out, base });
  } finally {
    if (previous === undefined) delete process.env.GITHUB_ACTIONS;
    else process.env.GITHUB_ACTIONS = previous;
    rmSync(out, { recursive: true, force: true });
    await apple.stop();
  }
}

test("prepare : identifiant des widgets créé, ménage, un certificat, un profil par identifiant", async () => {
  await withApple(async ({ apple, api, out }) => {
    const lines = await captureLog(() =>
      prepare(api, {
        bundleIds: [BUNDLE, WIDGETS],
        csrPem: CSR,
        profileName: "Corde CI 5.1",
        profilePrefix: "Corde CI ",
        outDir: out,
      }),
    );

    // L'identifiant des widgets manquait : créé, plateforme iOS, nom « Corde Widgets ».
    assert.deepEqual(apple.state.created.bundles, [
      { type: "bundleIds", attributes: { identifier: WIDGETS, name: "Corde Widgets", platform: "IOS" } },
    ]);
    // Identifiants retrouvés avant tout ménage.
    const firstDelete = apple.state.calls.findIndex((c) => c.startsWith("DELETE"));
    assert.ok(apple.state.calls.indexOf("POST /v1/bundleIds") < firstDelete);

    // Ménage : seuls les profils « Corde CI … » partent, avec leur seul certificat DISTRIBUTION
    // (révoqué une fois, même attaché à deux profils).
    assert.deepEqual([...apple.state.revoked], ["C-OLD"]);
    assert.equal(apple.state.calls.filter((c) => c === "DELETE /v1/certificates/C-OLD").length, 1);
    assert.deepEqual(apple.state.profiles.map((p) => p.id).sort(), ["P-NEW-1", "P-NEW-2", "P-OTHER"]);
    const firstCreate = apple.state.calls.indexOf("POST /v1/certificates");
    assert.ok(apple.state.calls.indexOf("DELETE /v1/certificates/C-OLD") < firstCreate);
    assert.ok(apple.state.calls.indexOf("DELETE /v1/profiles/P-OLD") < firstCreate);
    assert.ok(apple.state.calls.indexOf("DELETE /v1/profiles/P-OLD-W") < firstCreate);
    assert.ok(!apple.state.calls.includes("GET /v1/profiles/P-OTHER/certificates"));
    assert.equal(apple.state.calls.filter((c) => c === "POST /v1/certificates").length, 1);

    // Un certificat, deux profils liés à lui.
    assert.equal(apple.state.created.certificate.data.attributes.certificateType, "DISTRIBUTION");
    assert.match(apple.state.created.certificate.data.attributes.csrContent, /BEGIN CERTIFICATE REQUEST/);
    const [app, widgets] = apple.state.created.profiles.map((p) => p.data);
    assert.deepEqual(app.attributes, { name: "Corde CI 5.1", profileType: "IOS_APP_STORE" });
    assert.deepEqual(app.relationships.bundleId.data, { type: "bundleIds", id: "B-APP" });
    assert.deepEqual(widgets.attributes, { name: "Corde CI 5.1 widgets", profileType: "IOS_APP_STORE" });
    assert.deepEqual(widgets.relationships.bundleId.data, { type: "bundleIds", id: "B-NEW-2" });
    for (const p of [app, widgets]) {
      assert.deepEqual(p.relationships.certificates.data, [{ type: "certificates", id: "C-NEW" }]);
      assert.ok(p.attributes.name.startsWith("Corde CI "), "le ménage suivant les retrouvera");
    }

    // Fichiers : le certificat, et un dossier par identifiant.
    assert.equal(readFileSync(join(out, "dist.cer"), "utf8"), "faux certificat DER");
    assert.equal(readFileSync(join(out, BUNDLE, "profile.mobileprovision"), "utf8"), "faux profil 1");
    assert.equal(readFileSync(join(out, BUNDLE, "profile-name.txt"), "utf8"), "Corde CI 5.1\n");
    assert.equal(readFileSync(join(out, BUNDLE, "profile-uuid.txt"), "utf8"), "11111111-2222-3333-4444-555555555555\n");
    assert.equal(readFileSync(join(out, WIDGETS, "profile.mobileprovision"), "utf8"), "faux profil 2");
    assert.equal(readFileSync(join(out, WIDGETS, "profile-name.txt"), "utf8"), "Corde CI 5.1 widgets\n");
    assert.equal(readFileSync(join(out, WIDGETS, "profile-uuid.txt"), "utf8"), "21111111-2222-3333-4444-555555555555\n");

    // Le nom du titulaire n'apparaît que dans des commandes de masquage.
    assert.ok(lines.includes("::add-mask::Apple Distribution: Camille Exemple (ZYXWV98765)"));
    assert.ok(lines.includes("::add-mask::Camille Exemple"));
    for (const line of lines) {
      if (!line.startsWith("::add-mask::")) assert.doesNotMatch(line, /Camille/);
    }
  });
});

test("ménage : un certificat déjà révoqué (409 d'Apple) n'arrête rien, son profil part quand même", async () => {
  await withApple(async ({ apple, api }) => {
    // L'état laissé par un ménage interrompu : le profil de l'application est parti, celui des
    // widgets reste, attaché à un certificat qu'Apple liste encore mais qui est déjà révoqué.
    apple.state.profiles = apple.state.profiles.filter((p) => p.id !== "P-OLD");
    apple.state.revoked.add("C-OLD");
    const cleaned = await cleanPrevious(api, "Corde CI ");
    assert.deepEqual(cleaned, { profiles: 1, certificates: 0 });
    assert.ok(apple.state.calls.includes("DELETE /v1/certificates/C-OLD"));
    assert.deepEqual(apple.state.profiles.map((p) => p.id), ["P-OTHER"]);
    assert.ok(!apple.state.revoked.has("C-OTHER"));
  });
});

test("prepare : l'identifiant des widgets existant n'est pas recréé", async () => {
  await withApple(async ({ apple, api, out }) => {
    apple.state.bundles.push({ id: "B-W", identifier: WIDGETS });
    await captureLog(() =>
      prepare(api, { bundleIds: [BUNDLE, WIDGETS], csrPem: CSR, profileName: "Corde CI 6.1", profilePrefix: "Corde CI ", outDir: out }),
    );
    assert.deepEqual(apple.state.created.bundles, []);
    assert.deepEqual(apple.state.created.profiles[1].data.relationships.bundleId.data, { type: "bundleIds", id: "B-W" });
  });
});

test("prepare : application absente → erreur claire, rien créé ni révoqué", async () => {
  await withApple(async ({ apple, api, out }) => {
    apple.state.bundles = apple.state.bundles.filter((b) => b.id !== "B-APP");
    await assert.rejects(
      captureLog(() =>
        prepare(api, { bundleIds: [BUNDLE, WIDGETS], csrPem: CSR, profileName: "Corde CI 7.1", profilePrefix: "Corde CI ", outDir: out }),
      ),
      /l'identifiant io\.github\.dultra123\.corde n'existe pas[\s\S]*étape 3/,
    );
    assert.deepEqual(apple.state.created.bundles, [], "jamais l'identifiant de l'application");
    assert.ok(!apple.state.calls.some((c) => c.startsWith("DELETE") || c.startsWith("POST")));
    assert.ok(!existsSync(join(out, "dist.cer")));
  });
});

test("prepare : compte au maximum de certificats → message clair", async () => {
  await withApple(async ({ apple, api, out }) => {
    apple.state.failCertificate = true;
    await assert.rejects(
      captureLog(() =>
        prepare(api, { bundleIds: [BUNDLE, WIDGETS], csrPem: CSR, profileName: "Corde CI 6.1", profilePrefix: "Corde CI ", outDir: out }),
      ),
      /409 : ENTITY_ERROR[\s\S]*nombre maximal de certificats/,
    );
  });
});

// --- Clé stable (secret APPLE_DIST_KEY_BASE64) -------------------------------------------------

/** Une exécution de `prepare` (application et widgets) dans le dossier `out`. */
const runPrepare = (api, out, options) =>
  captureLog(() =>
    prepare(api, { bundleIds: [BUNDLE, WIDGETS], csrPem: CSR, profilePrefix: "Corde CI ", outDir: out, ...options }),
  );

/** Ni le nom du titulaire (hors masquage), ni le contenu d'un certificat, ni une clé dans le journal. */
function assertNoLeak(lines) {
  for (const line of lines) {
    if (!line.startsWith("::add-mask::")) assert.doesNotMatch(line, /Camille|ZYXWV98765/);
    assert.doesNotMatch(line, /BEGIN|PRIVATE/);
    for (const c of Object.values(certs)) assert.ok(!line.includes(c.content.slice(40, 80)), "contenu d'un certificat");
  }
}

/** Les appels faits pendant `run` seulement. */
async function callsDuring(apple, run) {
  const before = apple.state.calls.length;
  const lines = await run();
  return { calls: apple.state.calls.slice(before), lines };
}

const certificatesOf = (profile) => profile.data.relationships.certificates.data.map((c) => c.id);

test("clé stable : deux exécutions de suite, la seconde ne crée ni ne révoque aucun certificat", async () => {
  await withApple(async ({ apple, api, out }) => {
    apple.state.issue = () => certs.stable;
    const first = await callsDuring(apple, () =>
      runPrepare(api, join(out, "r1"), { profileName: "Corde CI 5.1", stableKeyPem: STABLE }),
    );
    assert.equal(first.calls.filter((c) => c === "POST /v1/certificates").length, 1);

    const second = await callsDuring(apple, () =>
      runPrepare(api, join(out, "r2"), { profileName: "Corde CI 6.1", stableKeyPem: STABLE }),
    );
    assert.ok(!second.calls.includes("POST /v1/certificates"), "aucun certificat créé");
    assert.ok(!second.calls.some((c) => c.startsWith("DELETE /v1/certificates")), "aucun certificat révoqué");
    // Les profils de la première exécution partent quand même (E) ; les nouveaux portent le même
    // certificat.
    assert.deepEqual(
      second.calls.filter((c) => c.startsWith("DELETE /v1/profiles")).sort(),
      ["DELETE /v1/profiles/P-NEW-1", "DELETE /v1/profiles/P-NEW-2"],
    );
    for (const p of apple.state.created.profiles.slice(2)) assert.deepEqual(certificatesOf(p), ["C-NEW"]);
    assert.deepEqual(readFileSync(join(out, "r2", "dist.cer")), Buffer.from(certs.stable.content, "base64"));
    assert.ok(second.lines.some((l) => /1 certificat\(s\) gardé\(s\) \(clé stable\)/.test(l)));
    assert.ok(second.lines.some((l) => /Certificat de distribution réutilisé/.test(l)));
    // Réutilisé : son nom est masqué comme celui d'un certificat neuf (sinon codesign ou
    // xcodebuild pourraient l'afficher dans le journal public).
    assert.ok(second.lines.includes("::add-mask::Apple Distribution: Camille Exemple (ZYXWV98765)"));
    assert.ok(second.lines.includes("::add-mask::Camille Exemple"));
    assertNoLeak([...first.lines, ...second.lines]);
  });
});

test("clé stable : certificat réutilisé dont l'API ne donne pas le nom → masqué d'après le certificat lui-même", async () => {
  await withApple(async ({ apple, api, out }) => {
    apple.state.certificates["C-FREE"] = { type: "DISTRIBUTION", ...certs.stable };
    apple.state.omitNames = true;
    const run = await callsDuring(apple, () =>
      runPrepare(api, join(out, "r1"), { profileName: "Corde CI 5.1", stableKeyPem: STABLE }),
    );
    assert.ok(run.lines.some((l) => /Certificat de distribution réutilisé/.test(l)));
    assert.ok(run.lines.includes("::add-mask::Apple Distribution: Camille Exemple (ZYXWV98765)"));
    assert.ok(run.lines.includes("::add-mask::Camille Exemple"));
    assertNoLeak(run.lines);
  });
});

test("subjectNames : les noms du sujet d'un certificat ; rien si le contenu est absent ou illisible", () => {
  assert.deepEqual(subjectNames({ certificateContent: certs.stable.content }), [
    "Apple Distribution: Camille Exemple (ZYXWV98765)",
  ]);
  assert.deepEqual(subjectNames({}), []);
  assert.deepEqual(subjectNames({ certificateContent: "pas un certificat" }), []);
});

test("clé stable : passage de l'ancien mode, l'ancien certificat révoqué une fois, un créé avec la clé stable", async () => {
  await withApple(async ({ apple, api, out }) => {
    // Une exécution de l'ancien mode : certificat d'une autre clé.
    apple.state.issue = () => certs.other;
    await runPrepare(api, join(out, "r1"), { profileName: "Corde CI 5.1" });
    assert.deepEqual(apple.state.created.profiles.flatMap(certificatesOf), ["C-NEW", "C-NEW"]);

    apple.state.issue = () => certs.stable;
    const { calls, lines } = await callsDuring(apple, () =>
      runPrepare(api, join(out, "r2"), { profileName: "Corde CI 6.1", stableKeyPem: STABLE }),
    );
    assert.deepEqual(calls.filter((c) => c.startsWith("DELETE /v1/certificates")), ["DELETE /v1/certificates/C-NEW"]);
    assert.equal(calls.filter((c) => c === "POST /v1/certificates").length, 1);
    assert.ok(calls.indexOf("DELETE /v1/certificates/C-NEW") < calls.indexOf("POST /v1/certificates"));
    for (const p of apple.state.created.profiles.slice(2)) assert.deepEqual(certificatesOf(p), ["C-NEW-2"]);
    assert.ok(spkiOf(readFileSync(join(out, "r2", "dist.cer")).toString("base64")).equals(STABLE_SPKI));
    assert.ok(lines.some((l) => /création d'un certificat avec cette clé/.test(l)));
    assertNoLeak(lines);
  });
});

test("clé stable : certificat qui expire dans moins de 30 jours → un neuf avec la même clé, l'ancien révoqué", async () => {
  await withApple(async ({ apple, api, out }) => {
    apple.state.certificates["C-SOON"] = { type: "DISTRIBUTION", ...certs.stableSoon };
    apple.state.profiles = [
      { id: "P-PREV", name: "Corde CI 7.1", certificates: ["C-SOON"] },
      { id: "P-PREV-W", name: "Corde CI 7.1 widgets", certificates: ["C-SOON"] },
      { id: "P-OTHER", name: "Profil d'un autre outil", certificates: ["C-OTHER"] },
    ];
    apple.state.issue = () => certs.stable;
    const lines = await runPrepare(api, out, { profileName: "Corde CI 8.1", stableKeyPem: STABLE });
    assert.deepEqual([...apple.state.revoked], ["C-SOON"]);
    assert.equal(apple.state.calls.filter((c) => c === "DELETE /v1/certificates/C-SOON").length, 1);
    assert.equal(apple.state.created.certificates.length, 1);
    for (const p of apple.state.created.profiles) assert.deepEqual(certificatesOf(p), ["C-NEW"]);
    assert.ok(spkiOf(apple.state.certificates["C-NEW"].content).equals(STABLE_SPKI));
    assert.ok(!apple.state.revoked.has("C-OTHER"));
    assertNoLeak(lines);
  });
});

test("clé stable : un certificat hors des profils « Corde CI » est réutilisé mais jamais révoqué ; un autre type n'est jamais touché", async () => {
  await withApple(async ({ apple, api, out }) => {
    // Filtre par type sans effet : le script doit vérifier le type lui-même.
    apple.state.ignoreTypeFilter = true;
    apple.state.certificates["C-FREE"] = { type: "DISTRIBUTION", ...certs.stable };
    apple.state.certificates["C-DEV"] = { type: "DEVELOPMENT", ...certs.stable };
    apple.state.profiles.find((p) => p.id === "P-OTHER").certificates.push("C-FREE", "C-DEV");
    for (const [i, name] of ["Corde CI 9.1", "Corde CI 10.1"].entries()) {
      const lines = await runPrepare(api, join(out, `r${i}`), { profileName: name, stableKeyPem: STABLE });
      assertNoLeak(lines);
    }
    assert.deepEqual(apple.state.created.certificates, [], "rien créé : C-FREE est réutilisé");
    for (const p of apple.state.created.profiles) assert.deepEqual(certificatesOf(p), ["C-FREE"]);
    // Seul l'ancien certificat d'une autre clé, attaché à « Corde CI 4.1 », est révoqué.
    assert.deepEqual([...apple.state.revoked], ["C-OLD"]);
    for (const id of ["C-FREE", "C-DEV", "C-MANAGED", "C-OTHER"]) {
      assert.ok(!apple.state.calls.includes(`DELETE /v1/certificates/${id}`), id);
    }
  });
});

test("clé stable : un certificat de même clé bientôt expiré, hors des profils « Corde CI », n'est pas révoqué", async () => {
  await withApple(async ({ apple, api, out }) => {
    apple.state.certificates["C-FREE-SOON"] = { type: "DISTRIBUTION", ...certs.stableSoon };
    apple.state.issue = () => certs.stable;
    await runPrepare(api, out, { profileName: "Corde CI 11.1", stableKeyPem: STABLE });
    assert.equal(apple.state.created.certificates.length, 1);
    assert.deepEqual([...apple.state.revoked], ["C-OLD"]);
    assert.ok(!apple.state.calls.includes("DELETE /v1/certificates/C-FREE-SOON"));
  });
});

test("clé stable : listes sans contenu de certificat et filtre par type refusé → relus un par un", async () => {
  await withApple(async ({ apple, api, out }) => {
    apple.state.omitContentInLists = true;
    apple.state.rejectTypeFilter = true;
    apple.state.certificates["C-FREE"] = { type: "DISTRIBUTION", ...certs.stable };
    const lines = await runPrepare(api, out, { profileName: "Corde CI 12.1", stableKeyPem: STABLE });
    assert.ok(apple.state.calls.includes("GET /v1/certificates/C-OLD"), "relu avant d'être comparé");
    assert.ok(apple.state.calls.includes("GET /v1/certificates/C-FREE"));
    assert.ok(apple.state.urls.some((u) => u.startsWith("GET /v1/certificates?limit=200&")), "liste sans filtre");
    assert.deepEqual([...apple.state.revoked], ["C-OLD"]);
    assert.deepEqual(apple.state.created.certificates, []);
    for (const p of apple.state.created.profiles) assert.deepEqual(certificatesOf(p), ["C-FREE"]);
    assertNoLeak(lines);
  });
});

test("sans clé stable : exactement les mêmes requêtes qu'avant", async () => {
  await withApple(async ({ apple, api, out }) => {
    apple.state.certificates["C-FREE"] = { type: "DISTRIBUTION", ...certs.stable };
    await runPrepare(api, out, { profileName: "Corde CI 5.1", stableKeyPem: "" });
    assert.deepEqual(apple.state.urls, [
      `GET /v1/bundleIds?filter[identifier]=${BUNDLE}&limit=200&fields[bundleIds]=identifier,platform`,
      `GET /v1/bundleIds?filter[identifier]=${WIDGETS}&limit=200&fields[bundleIds]=identifier,platform`,
      "POST /v1/bundleIds",
      "GET /v1/profiles?limit=200&fields[profiles]=name",
      "GET /v1/profiles?page=1",
      "GET /v1/profiles?page=2",
      "GET /v1/profiles/P-OLD/certificates?limit=50&fields[certificates]=certificateType",
      "DELETE /v1/certificates/C-OLD",
      "DELETE /v1/profiles/P-OLD",
      "GET /v1/profiles/P-OLD-W/certificates?limit=50&fields[certificates]=certificateType",
      "DELETE /v1/profiles/P-OLD-W",
      "POST /v1/certificates",
      "POST /v1/profiles",
      "POST /v1/profiles",
    ]);
    // Un certificat de la « clé stable » existe, mais sans le secret il n'est ni lu ni réutilisé.
    for (const p of apple.state.created.profiles) assert.deepEqual(certificatesOf(p), ["C-NEW"]);
  });
});

test("clé stable invalide : erreur claire avant toute requête, sans la valeur", async () => {
  const ec = generateKeyPairSync("ec", { namedCurve: "P-256" }).privateKey.export({ type: "pkcs8", format: "pem" });
  const small = generateKeyPairSync("rsa", { modulusLength: 1024 }).privateKey.export({ type: "pkcs8", format: "pem" });
  const text = "ceci n'est pas une clé, juste une phrase collée dans le secret";
  for (const [value, expected] of [
    [text, /ce n'est pas une clé privée lisible/],
    [ec, /clé ec, RSA 2048 attendue/],
    [small, /RSA de 1024 bits, 2048 attendus/],
  ]) {
    await withApple(async ({ apple, api, out }) => {
      await assert.rejects(runPrepare(api, out, { profileName: "Corde CI 13.1", stableKeyPem: value }), (error) => {
        assert.match(error.message, expected);
        assert.match(error.message, /APPLE_DIST_KEY_BASE64/);
        assert.ok(!error.message.includes(value.slice(30, 60)), "la valeur n'est pas dans le message");
        return true;
      });
      assert.deepEqual(apple.state.calls, []);
    });
  }
});

test("ligne de commande : --stable-key lit la clé et réutilise le certificat", async () => {
  await withApple(async ({ apple, out, base }) => {
    apple.state.certificates["C-FREE"] = { type: "DISTRIBUTION", ...certs.stable };
    const write = (name, data) => {
      writeFileSync(join(out, name), data, { mode: 0o600 });
      return join(out, name);
    };
    const { stdout } = await execFileAsync(
      process.execPath,
      [join(here, "asc-api.mjs"), "prepare", "--bundle-id", BUNDLE, "--bundle-id", WIDGETS,
        "--csr", write("dist.csr", CSR), "--out", join(out, "w"), "--profile-name", "Corde CI 14.1",
        "--profile-prefix", "Corde CI ", "--stable-key", write("dist.key", STABLE)],
      {
        env: {
          ...process.env,
          ASC_KEY_PATH: write("AuthKey_TEST.p8", keyPem),
          ASC_KEY_ID: "ABCDE12345",
          ASC_ISSUER_ID: "issuer-1",
          ASC_API_BASE: base,
          GITHUB_ACTIONS: "true",
        },
      },
    );
    assert.match(stdout, /Certificat de distribution réutilisé/);
    assert.deepEqual(apple.state.created.certificates, []);
    assertNoLeak(stdout.split("\n"));
    assert.deepEqual(readFileSync(join(out, "w", "dist.cer")), Buffer.from(certs.stable.content, "base64"));
  });
});

test("certificateInfo : contenu absent ou illisible → null, jamais d'erreur", () => {
  assert.equal(certificateInfo({}), null);
  assert.equal(certificateInfo({ certificateContent: Buffer.from("faux certificat DER").toString("base64") }), null);
  const info = certificateInfo({ certificateContent: certs.stable.content });
  assert.ok(info.spki.equals(STABLE_SPKI));
  assert.equal(info.expires, Date.parse(certs.stable.expires));
  assert.ok(info.expires - Date.now() > RENEW_MARGIN_MS);
  assert.ok(certificateInfo({ certificateContent: certs.stableSoon.content }).expires - Date.now() < RENEW_MARGIN_MS);
  assert.equal(stableKeyFrom(STABLE).spki.equals(STABLE_SPKI), true);
});

test("prepare : identifiants invalides ou en double refusés", async () => {
  const api = client({ keyPem, keyId: "K", issuerId: "I", base: "http://127.0.0.1:9" });
  const base = { csrPem: CSR, profileName: "Corde CI 1.1", profilePrefix: "Corde CI ", outDir: "/nonexistent" };
  await assert.rejects(prepare(api, { ...base, bundleIds: ["../evil"] }), /invalide/);
  await assert.rejects(prepare(api, { ...base, bundleIds: [BUNDLE, BUNDLE] }), /deux fois/);
  await assert.rejects(prepare(api, { ...base, bundleIds: [] }), /aucun/);
});

test("arguments : --bundle-id répété, nom des extensions", () => {
  const args = parseArgs(["--bundle-id", BUNDLE, "--bundle-id", WIDGETS, "--csr", "a.csr", "--out", "o", "--profile-name", "Corde CI 1.1"]);
  assert.deepEqual(args["bundle-id"], [BUNDLE, WIDGETS]);
  assert.equal(args.csr, "a.csr");
  assert.throws(() => parseArgs(["--csr", "a", "--csr", "b"]), /deux fois/);
  assert.equal(extensionName(WIDGETS), "Corde Widgets");
});

test("le jeton ne part jamais vers une autre adresse que l'API", async () => {
  const api = client({ keyPem, keyId: "K", issuerId: "I", base: "http://127.0.0.1:9" });
  await assert.rejects(api.request("GET", "https://example.org/v1/profiles"), /hors de l'API/);
  await assert.rejects(api.request("GET", "http://127.0.0.1:99/v1/profiles"), /hors de l'API/);
});

test("ApiError : message sans le texte de la requête", () => {
  const error = new ApiError("GET", "/v1/bundleIds?filter[identifier]=x", 403, {
    errors: [{ code: "FORBIDDEN_ERROR", title: "Forbidden", detail: "no access" }],
  });
  assert.equal(error.message, "GET /v1/bundleIds → 403 : FORBIDDEN_ERROR — Forbidden — no access");
});

/** Les blocs XCBuildConfiguration du texte, avec leur nom et leur identifiant d'application. */
function blocks(text) {
  const re = /^\t\t[0-9A-F]{24} \/\* [^\n]*? \*\/ = \{\n\t\t\tisa = XCBuildConfiguration;\n[\s\S]*?^\t\t\};\n/gm;
  return [...text.matchAll(re)].map(([b]) => ({
    text: b,
    name: /^\t\t\tname = (\w+);$/m.exec(b)?.[1],
    bundle: /^\t\t\t\tPRODUCT_BUNDLE_IDENTIFIER = "?([^";]+)"?;$/m.exec(b)?.[1] ?? null,
  }));
}

test("xcode-signing : les configurations Release de l'application et des widgets, seules", () => {
  const file = join(here, "..", "ios", "App", "App.xcodeproj", "project.pbxproj");
  const before = readFileSync(file, "utf8");
  const options = parseSigningArgs([
    "--team", "ZYXWV98765",
    "--bundle-id", BUNDLE, "--profile", "Corde CI 5.1",
    "--bundle-id", WIDGETS, "--profile", "Corde CI 5.1 widgets",
  ]);
  const after = manualSigningAll(before, options);

  const profiles = { [BUNDLE]: "Corde CI 5.1", [WIDGETS]: "Corde CI 5.1 widgets" };
  let manual = 0;
  for (const b of blocks(after)) {
    if (b.name === "Release" && b.bundle && profiles[b.bundle]) {
      manual++;
      assert.match(b.text, /^\t\t\t\tCODE_SIGN_STYLE = Manual;$/m);
      assert.match(b.text, /^\t\t\t\tCODE_SIGN_IDENTITY = "Apple Distribution";$/m);
      assert.match(b.text, /^\t\t\t\tDEVELOPMENT_TEAM = "ZYXWV98765";$/m);
      assert.ok(b.text.includes(`\t\t\t\tPROVISIONING_PROFILE_SPECIFIER = "${profiles[b.bundle]}";\n`), b.bundle);
      assert.doesNotMatch(b.text, /CODE_SIGN_STYLE = Automatic/);
      // Les droits (trousseau partagé) restent.
      assert.match(b.text, /CODE_SIGN_ENTITLEMENTS = /);
    } else {
      assert.doesNotMatch(b.text, /CODE_SIGN_STYLE = Manual|DEVELOPMENT_TEAM|PROVISIONING_PROFILE/);
    }
  }
  assert.equal(manual, 2);

  // Tout le reste du fichier est identique, ligne pour ligne.
  const removed = before.split("\n").filter((line) => !after.split("\n").includes(line));
  assert.deepEqual(removed, []);
  const added = after.split("\n").filter((line) => !before.split("\n").includes(line));
  assert.equal(added.length, 2 * 4); // 4 réglages par cible
  assert.equal(after.match(/CODE_SIGN_STYLE = Automatic;/g).length, 2); // les deux Debug
  assert.equal(after.match(/CODE_SIGN_STYLE = Manual;/g).length, 2);
  assert.equal((after.match(/\{/g) ?? []).length, (after.match(/\}/g) ?? []).length);

  // Appliqué deux fois : même résultat.
  assert.equal(manualSigningAll(after, options), after);
  // Identifiant inconnu : refus.
  assert.throws(() => manualSigning(before, { bundleId: "com.example.autre", team: "T", profile: "P" }), /0 bloc/);
  // Arguments mal formés.
  assert.throws(() => parseSigningArgs(["--team", "T", "--profile", "P"]), /suivre son --bundle-id/);
  assert.throws(() => parseSigningArgs(["--team", "T", "--bundle-id", BUNDLE]), /usage/);
  assert.throws(() => parseSigningArgs(["--bundle-id", BUNDLE, "--profile", "P"]), /usage/);
});
