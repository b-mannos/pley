import { describe, expect, it } from "vitest";
import type { Library } from "@/api/types";
import { ageBadge, ageOptionLabel, ageToSelect, librariesLabel, selectToAge, toggleId } from "./access";

const library = (id: number, name: string): Library => ({
  id,
  name,
  kind: "movies",
  paths: [],
  item_count: 0,
  created_at: 0,
  scanned_at: null,
  scanning: false,
});
const libraries = [library(1, "Movies"), library(2, "Shows")];

describe("age limits", () => {
  it("labels limits", () => {
    expect(ageBadge(13)).toBe("Up to 13");
    expect(ageBadge(null)).toBeNull();
    expect(ageOptionLabel(13)).toBe("Teens (PG-13)");
    expect(ageOptionLabel(null)).toBe("No limit");
    expect(ageOptionLabel(undefined)).toBe("No limit");
    expect(ageOptionLabel(12)).toBe("Up to 12");
  });

  it("round-trips through the select", () => {
    for (const age of [null, 0, 7, 17]) expect(selectToAge(ageToSelect(age))).toBe(age);
  });
});

describe("librariesLabel", () => {
  it("summarizes access", () => {
    expect(librariesLabel({ all_libraries: true, library_ids: [] }, libraries)).toBe("All libraries");
    expect(librariesLabel({ all_libraries: false, library_ids: [] }, libraries)).toBe("No libraries");
    expect(librariesLabel({ all_libraries: false, library_ids: [2] }, libraries)).toBe("Shows");
    expect(librariesLabel({ all_libraries: false, library_ids: [9] }, libraries)).toBe("1 library");
    expect(librariesLabel({ all_libraries: false, library_ids: [1, 2] }, undefined)).toBe("2 libraries");
  });
});

describe("toggleId", () => {
  it("adds and removes", () => {
    expect(toggleId([1, 2], 3)).toEqual([1, 2, 3]);
    expect(toggleId([1, 2, 3], 2)).toEqual([1, 3]);
  });
});
