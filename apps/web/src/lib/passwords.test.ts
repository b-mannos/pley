import { describe, expect, it } from "vitest";
import { passwordProblem } from "./passwords";

describe("passwordProblem", () => {
  it("wants 8 characters, counted as people see them", () => {
    expect(passwordProblem("seven77")).toBe("Use at least 8 characters for the password.");
    expect(passwordProblem("eight888")).toBeNull();
    expect(passwordProblem("日本語の言葉")).not.toBeNull();
    expect(passwordProblem("日本語の言葉です")).toBeNull();
  });

  it("refuses the username", () => {
    expect(passwordProblem("Bob.Smith", " bob.smith ")).toBe("The password can't be the username.");
    expect(passwordProblem("bob.smith2", "bob.smith")).toBeNull();
  });
});
