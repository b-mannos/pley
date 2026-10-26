import { describe, expect, it } from "vitest";
import { versionLabel, WEB_VERSION } from "./build";

describe("versionLabel", () => {
  it("puts the commit beside the version", () => {
    expect(versionLabel("0.2.0", "1a2b3c4d5")).toBe("0.2.0 (1a2b3c4d5)");
  });

  it("leaves out a commit that isn't known", () => {
    expect(versionLabel("0.2.0", "unknown")).toBe("0.2.0");
    expect(versionLabel("0.2.0", null)).toBe("0.2.0");
  });

  it("knows the web app's own version", () => {
    expect(WEB_VERSION).toMatch(/^\d+\.\d+\.\d+$/);
  });
});
