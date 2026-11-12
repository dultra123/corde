import { describe, expect, it } from "vitest";
import { authHeaders, setTokenProvider, wsProtocols } from "../net/auth";
import { hashOfAppLink } from "./links";

describe("links that open the application", () => {
  it("open a page of a closed list, nothing else", () => {
    expect(hashOfAppLink("corde://races")).toBe("#/races");
    expect(hashOfAppLink("corde://servers/")).toBe("#/servers");
    expect(hashOfAppLink("corde:///home")).toBe("#/home");
    expect(hashOfAppLink("CORDE://live")).toBe("#/");
    expect(hashOfAppLink("corde://sessions/123")).toBeNull();
    expect(hashOfAppLink("corde://races?x=1")).toBeNull();
    expect(hashOfAppLink("corde://evil")).toBeNull();
    expect(hashOfAppLink("https://races")).toBeNull();
  });
});

describe("the device token on the wire", () => {
  it("adds nothing in the browser", () => {
    setTokenProvider(null);
    expect(authHeaders()).toEqual({});
    expect(wsProtocols()).toBeUndefined();
  });

  it("goes in Authorization and in a subprotocol entry, and only a well-formed one", () => {
    const token = "0f".repeat(32);
    setTokenProvider(() => token);
    expect(authHeaders()).toEqual({ Authorization: `Bearer ${token}` });
    expect(wsProtocols()).toEqual(["corde.v1", `corde.token.${token}`]);
    setTokenProvider(() => "not a token, x");
    expect(authHeaders()).toEqual({});
    expect(wsProtocols()).toBeUndefined();
    setTokenProvider(null);
  });
});
