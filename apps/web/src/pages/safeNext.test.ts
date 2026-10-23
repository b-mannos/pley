import { describe, expect, it } from "vitest";
import { safeNext } from "./ProfilesPage";

describe("safeNext", () => {
  it("keeps paths on this site", () => {
    expect(safeNext("/library/3?tab=library")).toBe("/library/3?tab=library");
    expect(safeNext("/item/5#cast")).toBe("/item/5#cast");
    expect(safeNext("/search?q=a b")).toBe("/search?q=a%20b");
  });

  it("goes home without a usable path", () => {
    expect(safeNext(null)).toBe("/");
    expect(safeNext("")).toBe("/");
    expect(safeNext("library/3")).toBe("/");
  });

  it("never leaves the site", () => {
    for (const next of ["//evil.example", "/\\evil.example", "/\t/evil.example", "/\n/evil.example", "https://evil.example/"]) {
      expect(safeNext(next)).toBe("/");
    }
  });

  it("doesn't loop back to sign-in or the profile picker", () => {
    expect(safeNext("/login?next=/x")).toBe("/");
    expect(safeNext("/profiles")).toBe("/");
    expect(safeNext("/a/../login")).toBe("/");
  });
});
