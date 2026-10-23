import { describe, expect, it } from "vitest";
import { languageName } from "./languages";

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
