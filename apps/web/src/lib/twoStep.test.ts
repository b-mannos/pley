import { describe, expect, it } from "vitest";
import { ApiError } from "@/api/client";
import { codeKind, groupSecret, isTwoStepError, recoveryCodesText } from "./twoStep";

describe("two-step sign-in helpers", () => {
  it("knows the server asking for the second step", () => {
    expect(isTwoStepError(new ApiError(401, "two_step", "enter the 6-digit code"))).toBe(true);
    expect(isTwoStepError(new ApiError(401, "unauthorized", "wrong password"))).toBe(false);
    expect(isTwoStepError(new Error("two_step"))).toBe(false);
  });

  it("tells codes from the app from recovery codes, as people type them", () => {
    expect(codeKind("123456")).toBe("app");
    expect(codeKind("123 456")).toBe("app");
    expect(codeKind("12345")).toBeNull();
    expect(codeKind("7k2m-q9xd-4hpt-3vwr")).toBe("recovery");
    expect(codeKind("7K2M Q9XD 4HPT 3VWR")).toBe("recovery");
    expect(codeKind("7k2m-q9xd")).toBeNull();
  });

  it("groups secrets in fours", () => {
    expect(groupSecret("JBSWY3DPEHPK3PXP")).toBe("JBSW Y3DP EHPK 3PXP");
    expect(groupSecret("ABCDEF")).toBe("ABCD EF");
  });

  it("writes the codes down with what they're for", () => {
    const text = recoveryCodesText(["aaaa-bbbb-cccc-dddd", "eeee-ffff-gggg-hhhh"], "bob", "Living room");
    expect(text.split("\n")).toEqual([
      "Pley recovery codes for bob on Living room",
      "Each code signs in once, instead of a code from your authenticator app.",
      "",
      "aaaa-bbbb-cccc-dddd",
      "eeee-ffff-gggg-hhhh",
      "",
    ]);
  });
});
