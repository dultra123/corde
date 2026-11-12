import { describe, expect, it } from "vitest";
import { isPrivateIpv4, isPrivateIpv6, parseServerAddress } from "./addresses";

const base = (input: string) => {
  const r = parseServerAddress(input);
  return r.ok ? r.address.base : r.error;
};

describe("addresses of Corde servers", () => {
  it("reads what the driver types or scans, with Corde's port by default", () => {
    expect(base("192.168.1.20")).toBe("http://192.168.1.20:8471");
    expect(base("  192.168.1.20:8471  ")).toBe("http://192.168.1.20:8471");
    expect(base("http://192.168.1.20:8471/")).toBe("http://192.168.1.20:8471");
    // The PC's QR code, with a path or a fragment.
    expect(base("http://192.168.1.20:8471/#/home")).toBe("http://192.168.1.20:8471");
    expect(base("HTTP://PC-Jeu:8472")).toBe("http://pc-jeu:8472");
    expect(base("pc-jeu.local")).toBe("http://pc-jeu.local:8471");
    expect(base("10.8.0.1")).toBe("http://10.8.0.1:8471");
    expect(base("172.20.3.4:9000")).toBe("http://172.20.3.4:9000");
    expect(base("localhost")).toBe("http://localhost:8471");
    expect(base("[fd00::12]:8471")).toBe("http://[fd00::12]:8471");
    expect(base("https://corde.example.org")).toBe("https://corde.example.org");
    expect(base("https://corde.example.org:8443/x")).toBe("https://corde.example.org:8443");
  });

  it("accepts http:// toward private or local addresses only", () => {
    for (const bad of [
      "8.8.8.8",
      "http://corde.example.org",
      "example.org",
      "172.32.0.1",
      "100.64.1.2",
      "[2001:db8::1]:8471",
      "192.169.1.1",
    ]) {
      expect(base(bad), bad).toBe("public_http");
    }
  });

  it("refuses what is not a server address", () => {
    expect(base("")).toBe("empty");
    expect(base("   ")).toBe("empty");
    expect(base("ftp://192.168.1.20")).toBe("scheme");
    expect(base("ws://192.168.1.20:8471/ws")).toBe("scheme");
    expect(base("javascript://alert(1)")).toBe("scheme");
    expect(base("http://user:pw@192.168.1.20")).toBe("credentials");
    expect(base("192.168.1")).toBe("invalid");
    expect(base("999.1.1.1")).toBe("invalid");
    expect(base("192.168.1.20:")).toBe("invalid");
    expect(base("192.168.1.20:99999")).toBe("invalid");
    expect(base("192.168.1.20:0")).toBe("invalid");
    expect(base("pc jeu")).toBe("invalid");
    expect(base("[::1")).toBe("invalid");
    expect(base("fe80::1")).toBe("invalid");
    expect(base("-bad-.local")).toBe("invalid");
  });

  it("knows the private ranges", () => {
    expect(isPrivateIpv4("10.0.0.1")).toBe(true);
    expect(isPrivateIpv4("172.16.0.1")).toBe(true);
    expect(isPrivateIpv4("172.31.255.255")).toBe(true);
    expect(isPrivateIpv4("192.168.0.1")).toBe(true);
    expect(isPrivateIpv4("127.0.0.1")).toBe(true);
    expect(isPrivateIpv4("169.254.1.1")).toBe(true);
    expect(isPrivateIpv4("172.15.0.1")).toBe(false);
    expect(isPrivateIpv4("1.1.1.1")).toBe(false);
    expect(isPrivateIpv6("::1")).toBe(true);
    expect(isPrivateIpv6("fd12:3456::1")).toBe(true);
    expect(isPrivateIpv6("fe80::1")).toBe(true);
    expect(isPrivateIpv6("::ffff:192.168.1.2")).toBe(true);
    expect(isPrivateIpv6("::ffff:8.8.8.8")).toBe(false);
    expect(isPrivateIpv6("2001:db8::1")).toBe(false);
  });
});
