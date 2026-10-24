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

  it("keeps the code of a TV signing in", () => {
    expect(safeNext("/link?code=BCDFGHJK")).toBe("/link?code=BCDFGHJK");
    expect(safeNext("/link?code=BCDF-GHJK")).toBe("/link?code=BCDF-GHJK");
    expect(safeNext("/link")).toBe("/link");
    // As it comes back from sign-in and the profile picker.
    const next = new URLSearchParams(`next=${encodeURIComponent("/link?code=BCDFGHJK")}`).get("next");
    expect(safeNext(next)).toBe("/link?code=BCDFGHJK");
  });

  it("doesn't loop back to sign-in or the profile picker", () => {
    expect(safeNext("/login?next=/x")).toBe("/");
    expect(safeNext("/profiles")).toBe("/");
    expect(safeNext("/a/../login")).toBe("/");
  });
});
