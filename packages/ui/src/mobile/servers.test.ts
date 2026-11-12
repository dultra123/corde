import { describe, expect, it } from "vitest";
import { FakePrefs } from "./doubles";
import { KEYS } from "./kv";
import { MAX_SERVERS, ServerBook } from "./servers";

const TOKEN = "ab".repeat(32);

interface Call {
  url: string;
  init: RequestInit;
}

const json = (status: number, body: unknown, headers: Record<string, string> = {}) =>
  new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json", ...headers },
  });

function setup(answer: (c: Call) => Response | Promise<Response>) {
  const prefs = new FakePrefs();
  const calls: Call[] = [];
  const logs: string[] = [];
  let n = 0;
  const book = new ServerBook({
    prefs,
    fetch: async (url, init = {}) => {
      const c = { url, init };
      calls.push(c);
      return answer(c);
    },
    log: (l) => logs.push(l),
    now: () => 1_000,
    newId: () => {
      n += 1;
      return `${n}`.padStart(16, "0");
    },
  });
  return { prefs, calls, logs, book };
}

const header = (c: Call | undefined, name: string) =>
  (c?.init.headers as Record<string, string> | undefined)?.[name];

describe("« Mes serveurs »", () => {
  it("adds servers with a valid address only; the first becomes the active one", async () => {
    const { book, prefs } = setup(() => json(200, {}));
    expect(await book.add("8.8.8.8")).toEqual({ ok: false, error: "public_http" });
    expect(await book.add("")).toEqual({ ok: false, error: "empty" });
    const a = await book.add("192.168.1.20", "PC de jeu");
    expect(a.ok && a.entry).toMatchObject({ base: "http://192.168.1.20:8471", name: "PC de jeu" });
    expect(book.active()?.label).toBe("192.168.1.20:8471");
    expect(await book.add("http://192.168.1.20:8471/")).toEqual({ ok: false, error: "duplicate" });
    const b = await book.add("10.8.0.1");
    expect(b.ok).toBe(true);
    expect(book.active()?.label).toBe("192.168.1.20:8471");
    if (b.ok) await book.setActive(b.entry.id);
    // Read back after a restart.
    const again = new ServerBook({ prefs, fetch: async () => json(200, {}) });
    await again.load();
    expect(again.list().map((s) => s.label)).toEqual(["192.168.1.20:8471", "10.8.0.1:8471"]);
    expect(again.active()?.label).toBe("10.8.0.1:8471");
    for (let i = 3; i <= MAX_SERVERS; i += 1) await book.add(`192.168.2.${i}`);
    expect(await book.add("192.168.3.1")).toEqual({ ok: false, error: "full" });
  });

  it("tells a PC, a home server to pair, a refusal and no answer apart", async () => {
    const answers: Record<string, () => Response> = {
      "http://192.168.1.20:8471/api/health": () =>
        json(200, { ok: true, version: "0.8.0", role: "desktop" }),
      "http://192.168.1.30:8471/api/health": () =>
        json(401, { error: "pas appairé", code: "unpaired" }),
      "http://192.168.1.40:8471/api/health": () => json(403, { error: "hôte non autorisé" }),
      "http://192.168.1.50:8471/api/health": () => new Response("<html>router</html>"),
    };
    const { book } = setup(({ url }) => {
      const f = answers[url];
      if (!f) throw new TypeError("Load failed");
      return f();
    });
    const ids: string[] = [];
    for (const ip of ["20", "30", "40", "50", "60"]) {
      const r = await book.add(`192.168.1.${ip}`);
      if (r.ok) ids.push(r.entry.id);
    }
    const kinds = [];
    for (const id of ids) kinds.push((await book.probe(id)).kind);
    expect(kinds).toEqual(["ok", "unpaired", "refused", "not_corde", "unreachable"]);
    expect(book.get(ids[0] as string)?.role).toBe("desktop");
    expect(book.get(ids[1] as string)?.role).toBe("hub");
  });

  it("pairs with a home server: the token is stored apart, sent as Bearer, never shown nor logged", async () => {
    let revoked = false;
    const { book, calls, logs, prefs } = setup(({ url, init }) => {
      if (url.endsWith("/api/hub/pair")) {
        const body = JSON.parse(String(init.body)) as Record<string, unknown>;
        if (body.code !== "12345678") return json(400, { error: "code faux", code: "wrong_code" });
        expect(body.kind).toBe("app");
        return json(201, { device: { id: "d1", name: body.name, admin: true }, token: TOKEN });
      }
      if (url.endsWith("/api/hub/pairing"))
        return json(200, { paired: true, device: { id: "d1" } });
      if (url.endsWith("/api/hub/devices/d1") && init.method === "DELETE") {
        revoked = true;
        return new Response(null, { status: 204 });
      }
      const auth = header({ url, init }, "Authorization");
      return auth === `Bearer ${TOKEN}` && !revoked
        ? json(200, { ok: true, version: "0.8.0", role: "hub" })
        : json(401, { code: "unpaired" });
    });
    const added = await book.add("10.8.0.1", "Pi");
    const id = added.ok ? added.entry.id : "";
    expect((await book.probe(id)).kind).toBe("unpaired");

    expect(await book.pair(id, "1234", "iPhone")).toMatchObject({ ok: false, code: "invalid" });
    expect(await book.pair(id, "8765 4321", "iPhone")).toMatchObject({
      ok: false,
      code: "wrong_code",
      message: "code faux",
    });
    expect(await book.pair(id, "1234-5678", "iPhone de Test")).toEqual({
      ok: true,
      deviceName: "iPhone de Test",
      admin: true,
    });
    expect(book.isPaired(id)).toBe(true);
    expect(book.activeToken()).toBe(TOKEN);
    expect(prefs.data.get(KEYS.token(id))).toBe(TOKEN);
    expect(prefs.data.get(KEYS.servers)).not.toContain(TOKEN);
    expect(JSON.stringify(book.list())).not.toContain(TOKEN);

    expect(await book.probe(id)).toEqual({ kind: "ok", role: "hub", version: "0.8.0" });
    expect(header(calls.at(-1), "Authorization")).toBe(`Bearer ${TOKEN}`);
    // No cookie, ever (the token goes in the header).
    expect(calls.every((c) => c.init.credentials === "omit")).toBe(true);

    // Forgetting the pairing revokes this phone on the server first.
    expect(await book.unpair(id)).toEqual({ revokedOnServer: true });
    expect(book.isPaired(id)).toBe(false);
    expect(prefs.data.has(KEYS.token(id))).toBe(false);

    for (const line of logs) expect(line).not.toContain(TOKEN);
    expect(logs.join("\n")).not.toMatch(/[0-9a-f]{32}/);
  });

  it("forgets a token the server no longer knows (revoked), and the token of a removed server", async () => {
    let known = true;
    const { book, prefs } = setup(({ url }) => {
      if (url.endsWith("/api/hub/pair")) return json(201, { device: { name: "x" }, token: TOKEN });
      if (url.endsWith("/api/hub/pairing")) return new Response(null, { status: 503 });
      return known ? json(200, { ok: true, version: "0.8.0", role: "hub" }) : json(401, {});
    });
    const a = await book.add("192.168.1.9");
    const id = a.ok ? a.entry.id : "";
    await book.pair(id, "12345678", "iPhone");
    known = false;
    expect((await book.probe(id)).kind).toBe("unpaired");
    expect(book.activeToken()).toBeNull();
    expect(prefs.data.has(KEYS.token(id))).toBe(false);

    await book.pair(id, "12345678", "iPhone");
    await book.remove(id);
    expect(book.list()).toEqual([]);
    expect(book.active()).toBeNull();
    expect([...prefs.data.keys()].some((k) => k.startsWith("corde.servers.token."))).toBe(false);
  });
});
