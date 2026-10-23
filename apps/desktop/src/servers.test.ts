import { describe, expect, it } from "vitest";
import {
  candidates,
  displayAddress,
  isLocalHost,
  lastUsed,
  mergeServers,
  normalizeUrl,
  unencrypted,
  type FoundServer,
  type SavedServer,
} from "./servers";

describe("normalizeUrl", () => {
  it("fills in http and pley's port", () => {
    expect(normalizeUrl("nas")).toBe("http://nas:7539");
    expect(normalizeUrl(" 192.168.1.20 ")).toBe("http://192.168.1.20:7539");
    expect(normalizeUrl("nas:8080")).toBe("http://nas:8080");
    expect(normalizeUrl("[fd00::5]")).toBe("http://[fd00::5]:7539");
  });

  it("keeps what was spelled out, as an origin", () => {
    expect(normalizeUrl("https://pley.example.com/")).toBe("https://pley.example.com");
    expect(normalizeUrl("http://nas")).toBe("http://nas");
    expect(normalizeUrl("HTTP://NAS:7539/library/x")).toBe("http://nas:7539");
  });

  it("refuses what can't be a server", () => {
    for (const bad of ["", "   ", "ftp://nas", "file:///C:/x", "http://", "two words", "javascript:alert(1)"]) {
      expect(normalizeUrl(bad)).toBeNull();
    }
  });
});

describe("unencrypted addresses", () => {
  it("are fine on this computer, the local network and Tailscale", () => {
    for (const host of ["localhost", "nas", "nas.local", "pley.lan", "192.168.1.20", "10.0.0.7", "172.20.0.1", "100.101.102.103", "box.tail1234.ts.net", "[fd00::5]", "[fe80::1]", "[::1]"]) {
      expect(isLocalHost(host), host).toBe(true);
    }
    for (const host of ["pley.example.com", "203.0.113.9", "172.32.0.1", "100.128.0.1", "[2001:db8::1]"]) {
      expect(isLocalHost(host), host).toBe(false);
    }
  });

  it("are flagged across the internet", () => {
    expect(unencrypted("http://pley.example.com:7539")).toBe(true);
    expect(unencrypted("https://pley.example.com")).toBe(false);
    expect(unencrypted("http://192.168.1.20:7539")).toBe(false);
  });

  it("are tried after https for names on the internet", () => {
    expect(candidates("pley.example.com")).toEqual(["https://pley.example.com", "http://pley.example.com:7539"]);
    expect(candidates("nas")).toEqual(["http://nas:7539"]);
    expect(candidates("192.168.1.20")).toEqual(["http://192.168.1.20:7539"]);
    expect(candidates("http://pley.example.com")).toEqual(["http://pley.example.com"]);
    expect(candidates("pley.example.com:8443")).toEqual(["http://pley.example.com:8443"]);
    expect(candidates("two words")).toEqual([]);
  });
});

describe("displayAddress", () => {
  it("leaves pley's port out", () => {
    expect(displayAddress("http://192.168.1.20:7539")).toBe("192.168.1.20");
    expect(displayAddress("http://nas:8080")).toBe("nas:8080");
    expect(displayAddress("https://pley.example.com")).toBe("pley.example.com");
  });
});

describe("mergeServers", () => {
  const saved: SavedServer[] = [
    { url: "http://nas:7539", name: "NAS", id: "a1", last_used: 10 },
    { url: "http://office:7539", name: "Office", id: null, last_used: 20 },
  ];

  it("puts saved servers first, the last opened on top, then new ones", () => {
    const found: FoundServer[] = [
      { url: "http://192.168.1.20:7539", name: "NAS", id: "a1", version: "0.1.0" },
      { url: "http://192.168.1.30:7539", name: "Den", id: "b2", version: "0.1.0" },
    ];
    expect(mergeServers(saved, found)).toEqual([
      { url: "http://office:7539", name: "Office", id: null, saved: true, found: false, lookalike: false },
      { url: "http://nas:7539", name: "NAS", id: "a1", saved: true, found: true, lookalike: false },
      { url: "http://192.168.1.30:7539", name: "Den", id: "b2", saved: false, found: true, lookalike: false },
    ]);
  });

  it("flags a new server named like a saved one", () => {
    const found: FoundServer[] = [{ url: "http://192.168.1.66:7539", name: "nas ", id: "evil", version: "0.1.0" }];
    expect(mergeServers(saved, found).at(-1)).toMatchObject({ name: "nas ", saved: false, lookalike: true });
  });

  it("matches servers without an id by address", () => {
    const found: FoundServer[] = [{ url: "http://office:7539", name: "Office", id: "c3", version: "0.1.0" }];
    expect(mergeServers(saved, found).map((e) => [e.name, e.saved, e.found])).toEqual([
      ["Office", true, true],
      ["NAS", true, false],
    ]);
  });

  it("knows the last one opened", () => {
    expect(lastUsed(saved)?.name).toBe("Office");
    expect(lastUsed([])).toBeNull();
  });
});
