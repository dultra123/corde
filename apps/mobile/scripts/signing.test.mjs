// SPDX-License-Identifier: GPL-3.0-or-later
//
// Essais des scripts de signature de la CI iOS (asc-api.mjs, xcode-signing.mjs).
//   node --test apps/mobile/scripts/signing.test.mjs
// L'API d'Apple est remplacée par un petit serveur local : ces essais vérifient l'enchaînement et
// la forme des requêtes, pas le comportement réel d'Apple.

import assert from "node:assert/strict";
import { generateKeyPairSync, verify } from "node:crypto";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { ApiError, client, makeToken, prepare } from "./asc-api.mjs";
import { manualSigning } from "./xcode-signing.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const { privateKey, publicKey } = generateKeyPairSync("ec", { namedCurve: "P-256" });
const keyPem = privateKey.export({ type: "pkcs8", format: "pem" });
const BUNDLE = "io.github.dultra123.corde";

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

/** Faux App Store Connect : un compte avec un profil et un certificat laissés par l'exécution précédente. */
function fakeApple() {
  const state = {
    calls: [],
    profiles: [
      { id: "P-OLD", name: "Corde CI 4.1", certificates: ["C-OLD", "C-MANAGED"] },
      { id: "P-OTHER", name: "Profil d'un autre outil", certificates: ["C-OTHER"] },
    ],
    certificates: {
      "C-OLD": "DISTRIBUTION",
      "C-MANAGED": "DISTRIBUTION_MANAGED",
      "C-OTHER": "DISTRIBUTION",
    },
    created: {},
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
        // Deux pages, pour exercer la pagination.
        const page = url.searchParams.get("page") === "2" ? 1 : 0;
        const item = state.profiles[page];
        const next = page === 0 && state.profiles.length > 1 ? `${base}/v1/profiles?page=2` : undefined;
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
        delete state.certificates[match[1]];
        return send(204);
      }
      if (req.method === "DELETE" && (match = path.match(/^\/v1\/profiles\/([\w-]+)$/))) {
        state.profiles = state.profiles.filter((p) => p.id !== match[1]);
        return send(204);
      }
      if (req.method === "GET" && path === "/v1/bundleIds") {
        assert.equal(url.searchParams.get("filter[identifier]"), BUNDLE);
        return send(200, {
          data: [
            { type: "bundleIds", id: "B-LONGER", attributes: { identifier: `${BUNDLE}.widgets` } },
            { type: "bundleIds", id: "B-APP", attributes: { identifier: BUNDLE } },
          ],
        });
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
        state.created.profile = JSON.parse(body);
        const name = state.created.profile.data.attributes.name;
        state.profiles.push({ id: "P-NEW", name, certificates: ["C-NEW"] });
        return send(201, {
          data: {
            type: "profiles",
            id: "P-NEW",
            attributes: {
              name,
              uuid: "11111111-2222-3333-4444-555555555555",
              profileContent: Buffer.from("faux profil").toString("base64"),
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

test("prepare : ménage de l'exécution précédente, certificat, profil, fichiers", async () => {
  const apple = fakeApple();
  const base = await apple.start();
  const out = mkdtempSync(join(tmpdir(), "corde-signing-"));
  const previous = process.env.GITHUB_ACTIONS;
  process.env.GITHUB_ACTIONS = "true";
  try {
    const api = client({ keyPem, keyId: "ABCDE12345", issuerId: "issuer-1", base });
    const lines = await captureLog(() =>
      prepare(api, {
        bundleId: BUNDLE,
        csrPem: "-----BEGIN CERTIFICATE REQUEST-----\nMIIB\n-----END CERTIFICATE REQUEST-----\n",
        profileName: "Corde CI 5.1",
        profilePrefix: "Corde CI ",
        outDir: out,
      }),
    );

    // Ménage : seul le profil « Corde CI … » part, avec son seul certificat DISTRIBUTION.
    assert.deepEqual(Object.keys(apple.state.certificates).sort(), ["C-MANAGED", "C-NEW", "C-OTHER"]);
    assert.deepEqual(apple.state.profiles.map((p) => p.id).sort(), ["P-NEW", "P-OTHER"]);
    const firstCreate = apple.state.calls.indexOf("POST /v1/certificates");
    assert.ok(apple.state.calls.indexOf("DELETE /v1/certificates/C-OLD") < firstCreate);
    assert.ok(apple.state.calls.indexOf("DELETE /v1/profiles/P-OLD") < firstCreate);
    assert.ok(!apple.state.calls.includes("GET /v1/profiles/P-OTHER/certificates"));

    // Requêtes de création.
    assert.deepEqual(apple.state.created.certificate.data.attributes.certificateType, "DISTRIBUTION");
    assert.match(apple.state.created.certificate.data.attributes.csrContent, /BEGIN CERTIFICATE REQUEST/);
    const profile = apple.state.created.profile.data;
    assert.deepEqual(profile.attributes, { name: "Corde CI 5.1", profileType: "IOS_APP_STORE" });
    assert.deepEqual(profile.relationships.bundleId.data, { type: "bundleIds", id: "B-APP" });
    assert.deepEqual(profile.relationships.certificates.data, [{ type: "certificates", id: "C-NEW" }]);

    // Fichiers.
    assert.equal(readFileSync(join(out, "dist.cer"), "utf8"), "faux certificat DER");
    assert.equal(readFileSync(join(out, "profile.mobileprovision"), "utf8"), "faux profil");
    assert.equal(readFileSync(join(out, "profile-name.txt"), "utf8"), "Corde CI 5.1\n");
    assert.equal(readFileSync(join(out, "profile-uuid.txt"), "utf8"), "11111111-2222-3333-4444-555555555555\n");

    // Le nom du titulaire n'apparaît que dans des commandes de masquage.
    assert.ok(lines.includes("::add-mask::Apple Distribution: Camille Exemple (ZYXWV98765)"));
    assert.ok(lines.includes("::add-mask::Camille Exemple"));
    for (const line of lines) {
      if (!line.startsWith("::add-mask::")) assert.doesNotMatch(line, /Camille/);
    }
  } finally {
    if (previous === undefined) delete process.env.GITHUB_ACTIONS;
    else process.env.GITHUB_ACTIONS = previous;
    rmSync(out, { recursive: true, force: true });
    await apple.stop();
  }
});

test("prepare : compte au maximum de certificats → message clair", async () => {
  const apple = fakeApple();
  const base = await apple.start();
  apple.state.failCertificate = true;
  const out = mkdtempSync(join(tmpdir(), "corde-signing-"));
  try {
    const api = client({ keyPem, keyId: "ABCDE12345", issuerId: "issuer-1", base });
    await assert.rejects(
      captureLog(() =>
        prepare(api, { bundleId: BUNDLE, csrPem: "csr", profileName: "Corde CI 6.1", profilePrefix: "Corde CI ", outDir: out }),
      ),
      /409 : ENTITY_ERROR[\s\S]*nombre maximal de certificats/,
    );
  } finally {
    rmSync(out, { recursive: true, force: true });
    await apple.stop();
  }
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

test("xcode-signing : seule la configuration Release de l'application change", () => {
  const file = join(here, "..", "ios", "App", "App.xcodeproj", "project.pbxproj");
  const before = readFileSync(file, "utf8");
  const after = manualSigning(before, { bundleId: BUNDLE, team: "ZYXWV98765", profile: "Corde CI 5.1" });

  const release = after.slice(after.lastIndexOf("isa = XCBuildConfiguration;"));
  assert.match(release, /^\t\t\t\tCODE_SIGN_STYLE = Manual;$/m);
  assert.match(release, /^\t\t\t\tCODE_SIGN_IDENTITY = "Apple Distribution";$/m);
  assert.match(release, /^\t\t\t\tDEVELOPMENT_TEAM = "ZYXWV98765";$/m);
  assert.match(release, /^\t\t\t\tPROVISIONING_PROFILE_SPECIFIER = "Corde CI 5.1";$/m);
  assert.match(release, /^\t\t\tname = Release;$/m);
  assert.doesNotMatch(release, /CODE_SIGN_STYLE = Automatic/);

  // Tout le reste du fichier est identique, ligne pour ligne.
  const removed = before.split("\n").filter((line) => !after.split("\n").includes(line));
  assert.deepEqual(removed, []);
  const added = after.split("\n").filter((line) => !before.split("\n").includes(line));
  assert.equal(added.length, 4);
  assert.equal(after.match(/CODE_SIGN_STYLE = Automatic;/g).length, 1); // la configuration Debug
  assert.equal((after.match(/\{/g) ?? []).length, (after.match(/\}/g) ?? []).length);

  // Appliqué deux fois : même résultat.
  assert.equal(manualSigning(after, { bundleId: BUNDLE, team: "ZYXWV98765", profile: "Corde CI 5.1" }), after);
  // Identifiant inconnu : refus.
  assert.throws(() => manualSigning(before, { bundleId: "com.example.autre", team: "T", profile: "P" }), /0 bloc/);
});
