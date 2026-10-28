import { describe, expect, it } from "vitest";
import { languageName, LANGUAGES, twoLetterCode } from "./languages";

describe("languageName", () => {
  it("names ISO 639-2 and 639-1 codes", () => {
    expect(languageName("eng")).toBe("English");
    expect(languageName("ger")).toBe("German");
    expect(languageName("deu")).toBe("German");
    expect(languageName("ja")).toBe("Japanese");
    expect(languageName(" FRE ")).toBe("French");
  });

  it("adds the region", () => {
    expect(languageName("pt-BR")).toBe("Portuguese (BR)");
    expect(languageName("zh_tw")).toBe("Chinese (TW)");
  });

  it("returns unknown codes as they are", () => {
    expect(languageName("xyz")).toBe("XYZ");
    expect(languageName("klingon")).toBe("klingon");
  });

  it("is null without a code", () => {
    expect(languageName(null)).toBeNull();
    expect(languageName("")).toBeNull();
    expect(languageName("   ")).toBeNull();
  });
});

describe("twoLetterCode", () => {
  it("is the two-letter code subtitle sites use", () => {
    expect(twoLetterCode("eng")).toBe("en");
    expect(twoLetterCode("chi")).toBe("zh");
    expect(twoLetterCode("per")).toBe("fa");
    expect(twoLetterCode("de")).toBe("de");
    expect(twoLetterCode("xyz")).toBe("xyz");
    // Every language offered has one.
    for (const l of LANGUAGES) expect(twoLetterCode(l.code)).toHaveLength(2);
  });
});
