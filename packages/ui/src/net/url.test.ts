import { describe, expect, it } from "vitest";
import { hostFromQuery, resolveEndpoint } from "./url";

const page = { host: "127.0.0.1:8471", protocol: "http:" };

describe("the server an address of the page may name (?server=)", () => {
  it("follows an address of the home network", () => {
    for (const [value, host] of [
      ["192.168.1.20:8471", "192.168.1.20:8471"],
      ["192.168.1.20", "192.168.1.20:8471"],
      ["127.0.0.1:9000", "127.0.0.1:9000"],
      ["10.8.0.1:8471", "10.8.0.1:8471"],
      ["100.100.20.3", "100.100.20.3:8471"],
      ["pc-jeu:8471", "pc-jeu:8471"],
      ["pc-jeu.local", "pc-jeu.local:8471"],
      ["localhost:5173", "localhost:5173"],
      ["http://192.168.1.20:8471/", "192.168.1.20:8471"],
      ["[::1]:8471", "[::1]:8471"],
    ] as const) {
      expect(hostFromQuery(value), value).toBe(host);
      const e = resolveEndpoint("", { ...page, search: `?server=${encodeURIComponent(value)}` });
      expect(e.origin, value).toBe("query");
      expect(e.http, value).toBe(`http://${host}`);
    }
  });

  it("ignores an address of the Internet: a link cannot point the page at somebody's server", () => {
    for (const value of [
      "evil.example",
      "evil.example:8471",
      "https://evil.example",
      "8.8.8.8",
      "8.8.8.8:8471",
      "203.0.113.7:80",
      "192.168.1.20.evil.example",
      "127.0.0.1.nip.io:8471",
      "user:pass@192.168.1.20",
      "[2001:db8::1]:8471",
      "",
      "   ",
    ]) {
      expect(hostFromQuery(value), value).toBeNull();
      // The page keeps talking to the server that gave it.
      const e = resolveEndpoint("", { ...page, search: `?server=${encodeURIComponent(value)}` });
      expect(e.origin, value).not.toBe("query");
      expect(e.http, value).toBe("http://127.0.0.1:8471");
    }
  });

  it("leaves the address typed in the settings alone", () => {
    const e = resolveEndpoint("corde.example.org:8471", { ...page, search: "" });
    expect(e.origin).toBe("setting");
    expect(e.http).toBe("http://corde.example.org:8471");
  });
});
