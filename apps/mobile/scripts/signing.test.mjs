// SPDX-License-Identifier: GPL-3.0-or-later
//
// Essais des scripts de signature de la CI iOS (asc-api.mjs, xcode-signing.mjs).
//   node --test apps/mobile/scripts/signing.test.mjs
// L'API d'Apple est remplacée par un petit serveur local : ces essais vérifient l'enchaînement et
// la forme des requêtes, pas le comportement réel d'Apple.

import assert from "node:assert/strict";
import { generateKeyPairSync, verify } from "node:crypto";
import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { ApiError, cleanPrevious, client, extensionName, makeToken, parseArgs, prepare } from "./asc-api.mjs";
import { manualSigning, manualSigningAll, parseSigningArgs } from "./xcode-signing.mjs";

const here = dirname(fileURLToPath(import.meta.url));
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

/**
 * Faux App Store Connect : un compte avec l'identifiant de l'application, et un profil et un
 * certificat laissés par l'exécution précédente.
 */
function fakeApple() {
  const state = {
    calls: [],
    profiles: [
      { id: "P-OLD", name: "Corde CI 4.1", certificates: ["C-OLD", "C-MANAGED"] },
      { id: "P-OLD-W", name: "Corde CI 4.1 widgets", certificates: ["C-OLD"] },
      { id: "P-OTHER", name: "Profil d'un autre outil", certificates: ["C-OTHER"] },
    ],
    certificates: {
      "C-OLD": "DISTRIBUTION",
      "C-MANAGED": "DISTRIBUTION_MANAGED",
      "C-OTHER": "DISTRIBUTION",
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
    created: { profiles: [], bundles: [] },
    failCertificate: false,
  };
  const server = createServer((req, res) => {
    let body = "";
    req.on("data", (chunk) => (body += chunk));
    req.on("end", () => {
      const url = new URL(req.url, "http://localhost");
      const path = url.pathname;
      state.calls.push(`${req.method} ${path}`);
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
            .map((id) => ({ type: "certificates", id, attributes: { certificateType: state.certificates[id] } })),
        });
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
        state.certificates["C-NEW"] = "DISTRIBUTION";
        return send(201, {
          data: {
            type: "certificates",
            id: "C-NEW",
            attributes: {
              name: "Apple Distribution: Camille Exemple (ZYXWV98765)",
              displayName: "Camille Exemple",
              expirationDate: "2027-11-14T10:00:00.000+00:00",
              certificateContent: Buffer.from("faux certificat DER").toString("base64"),
            },
          },
        });
      }
      if (req.method === "POST" && path === "/v1/profiles") {
        const created = JSON.parse(body);
        state.created.profiles.push(created);
        const n = state.created.profiles.length;
        const name = created.data.attributes.name;
        state.profiles.push({ id: `P-NEW-${n}`, name, certificates: ["C-NEW"] });
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
    await run({ apple, api, out });
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
