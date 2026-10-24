import { describe, expect, it } from "vitest";
import { CODE_LETTERS, codeLetters, hasStrayLetter, isWholeCode, showCode } from "./userCode";

describe("the codes TVs show", () => {
  it("are 20 consonants: no vowels, no Y, no digits", () => {
    expect(CODE_LETTERS).toHaveLength(20);
    expect(CODE_LETTERS).toMatch(/^[^AEIOUY0-9]+$/);
  });

  it("take the letters as people type them, in capitals", () => {
    expect(codeLetters("bcdf-ghjk")).toBe("BCDFGHJK");
    expect(codeLetters(" BCDF GHJK ")).toBe("BCDFGHJK");
    expect(codeLetters("bc2d.f_g")).toBe("BCDFG");
    // Only the first 8, and nothing a capital would turn into letters.
    expect(codeLetters("BCDFGHJKLM")).toBe("BCDFGHJK");
    expect(codeLetters("ßbcd")).toBe("BCD");
    expect(codeLetters("")).toBe("");
  });

  it("show as the TV does, as far as they're typed", () => {
    expect(showCode("")).toBe("");
    expect(showCode("BCDF")).toBe("BCDF");
    expect(showCode("BCDFG")).toBe("BCDF-G");
    expect(showCode("BCDFGHJK")).toBe("BCDF-GHJK");
  });

  it("are whole with 8 of their letters", () => {
    expect(isWholeCode("BCDFGHJK")).toBe(true);
    expect(isWholeCode("ZXWVTSRQ")).toBe(true);
    expect(isWholeCode("BCDFGHJ")).toBe(false);
    expect(isWholeCode("BCDFGHJA")).toBe(false);
    expect(hasStrayLetter("BCDFGHJA")).toBe(true);
    expect(hasStrayLetter("BCDY")).toBe(true);
    expect(hasStrayLetter("BCD")).toBe(false);
  });
});
