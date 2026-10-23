import { describe, expect, it } from "vitest";
import { cn } from "./cn";

describe("cn", () => {
  it("joins truthy class names", () => {
    expect(cn("a", null, undefined, false, "", "b")).toBe("a b");
  });

  it("flattens arrays and objects", () => {
    expect(cn(["a", ["b", { c: true, d: false }]], { e: null, f: true })).toBe("a b c f");
  });

  it("keeps zero", () => {
    expect(cn(0, 1)).toBe("0 1");
  });
});
