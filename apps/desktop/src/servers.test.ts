import { describe, expect, it } from "vitest";
import { displayAddress, lastUsed, mergeServers, normalizeUrl, type FoundServer, type SavedServer } from "./servers";

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
      { url: "http://office:7539", name: "Office", id: null, saved: true, found: false },
      { url: "http://nas:7539", name: "NAS", id: "a1", saved: true, found: true },
      { url: "http://192.168.1.30:7539", name: "Den", id: "b2", saved: false, found: true },
    ]);
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
