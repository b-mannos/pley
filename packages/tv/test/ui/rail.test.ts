import { describe, expect, it } from "vitest";
import type { Library } from "../../src/api/types";
import { currentEntry, railEntries } from "../../src/ui/shell/rail";

/** The Android app's ShellRulesTest, for the TVs' rail. */
const libraries = (n: number): Library[] =>
  Array.from({ length: n }, (_, i) => ({ id: i + 1, name: `Library ${i + 1}`, kind: (i + 1) % 2 === 0 ? "shows" : "movies" }) as Library);
const ids = (n: number, nowPlaying = false) => railEntries(libraries(n), nowPlaying).top.map((e) => e.id);

describe("the rail", () => {
  it("lists up to three libraries, then Collections", () => {
    expect(ids(3)).toEqual(["search", "home", "library:1", "library:2", "library:3", "playlists", "collections"]);
    expect(railEntries(libraries(3)).top.at(-1)!.key).toEqual({ name: "collections", libraryId: 1 });
  });

  it("puts Libraries in Collections' place with more than three", () => {
    expect(ids(5)).toEqual(["search", "home", "library:1", "library:2", "library:3", "libraries", "playlists"]);
  });

  it("has no Collections without libraries", () => {
    expect(ids(0)).toEqual(["search", "home", "playlists"]);
  });

  it("puts Now Playing last, within seven entries, while music has a queue", () => {
    // In Collections' place.
    expect(ids(3, true)).toEqual(["search", "home", "library:1", "library:2", "library:3", "playlists", "now-playing"]);
    // In the last library's, which Libraries lists.
    expect(ids(5, true)).toEqual(["search", "home", "library:1", "library:2", "libraries", "playlists", "now-playing"]);
    // Added where there is room.
    expect(ids(2, true)).toEqual(["search", "home", "library:1", "library:2", "playlists", "collections", "now-playing"]);
  });

  it("marks the entry that opened the stack's first page", () => {
    const rail = railEntries(libraries(5), true);
    expect(currentEntry({ name: "nowPlaying" }, rail)).toBe("now-playing");
    expect(currentEntry({ name: "library", id: 2 }, rail)).toBe("library:2");
    // Library 3 gave its place to Now Playing: Libraries lists it.
    expect(currentEntry({ name: "library", id: 3 }, rail)).toBe("libraries");
    expect(currentEntry({ name: "search", query: "blade" }, rail)).toBe("search");
    expect(currentEntry({ name: "profiles" }, rail)).toBe("profile");
    expect(currentEntry({ name: "settings" }, rail)).toBe("settings");
    // Collections without its own entry: the library's.
    expect(currentEntry({ name: "collections", libraryId: 1 }, rail)).toBe("library:1");
    expect(currentEntry({ name: "collections", libraryId: 1 }, railEntries(libraries(3)))).toBe("collections");
    expect(currentEntry({ name: "nowPlaying" }, railEntries(libraries(3)))).toBeNull();
    expect(currentEntry(null, rail)).toBeNull();
  });
});
