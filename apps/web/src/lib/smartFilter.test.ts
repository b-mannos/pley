import { describe, expect, it } from "vitest";
import { describeSmartFilter, smartFilterOf } from "./smartFilter";

describe("smart filters", () => {
  it("turn the library page's filters into Plex's syntax", () => {
    expect(smartFilterOf({ sort: "year", order: "desc", watched: false, genre: "Science Fiction" })).toBe(
      "genre=Science+Fiction&unwatched=1&sort=year%3Adesc",
    );
    expect(smartFilterOf({ sort: "title", order: "asc", watched: true, year: 1999 })).toBe("year=1999&unwatched=0&sort=titleSort");
  });

  it("are described in words", () => {
    expect(describeSmartFilter("type=1&genre=Comedy&unwatched=1&sort=year%3Adesc")).toEqual([
      "Genre: Comedy",
      "Unwatched",
      "Newest first",
    ]);
    // As python-plexapi writes them.
    expect(describeSmartFilter("year%3E%3E=2019&sort=movie.addedAt:desc&includeGuids=1&limit=20")).toEqual([
      "After 2019",
      "Recently added first",
      "At most 20",
    ]);
    expect(describeSmartFilter("resolution=4k")).toEqual(["resolution: 4k"]);
  });
});
