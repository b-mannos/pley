import { describe, expect, it } from "vitest";
import type { Hub, ItemSummary } from "../../src/api/types";
import { check, connect, hostPort, subnetOf } from "../../src/auth/servers";
import { TokenError, type OAuthError } from "../../src/auth/tokens";
import type { HttpRequest, Transport } from "../../src/net/transport";
import { NetworkError } from "../../src/net/transport";
import { shuffled } from "../../src/player/music";
import { heroOf, rowKind, seeAllOf } from "../../src/ui/screens/home";
import { AlphaIndex, countLabel, defaultOrder, fitParams, letterOf, showsJump, sortsFor } from "../../src/ui/screens/library-data";
import { codeHalves, displayCode, typedAddress } from "../../src/ui/screens/onboarding/link";
import { switchRefusal } from "../../src/ui/screens/onboarding/profiles";
import { relativeTime } from "../../src/ui/screens/settings";

function summary(over: Partial<ItemSummary>): ItemSummary {
  return { id: 1, library_id: 1, kind: "movie", title: "A", sort_title: "a", added_at: 0, user_state: { position_ms: 0, watched: false, play_count: 0 }, ...over };
}

describe("the A–Z jump", () => {
  const items = ["Alien 1", "Alien 2", "Batman 1", "Ébloui", "Zorro 1", "300", "ünder"].map((t) => ({ title: t, sort_title: t.toLowerCase() }));
  const sorted = items.slice().sort((a, b) => letterOf(a).localeCompare(letterOf(b)));

  it("files titles by their first letter, accents removed, the rest under #", () => {
    expect(letterOf({ title: "Ébloui", sort_title: "" })).toBe("E");
    expect(letterOf({ title: "300", sort_title: "300" })).toBe("#");
    expect(letterOf({ title: "Matrix", sort_title: "matrix" })).toBe("M");
  });

  it("jumps to a letter's first title, else the next letter's, else the one before", () => {
    const alpha = new AlphaIndex(sorted);
    expect(alpha.jumpTarget("A")).toBe(sorted.findIndex((x) => x.title === "Alien 1"));
    // No C: the next letter with titles.
    expect(sorted[alpha.jumpTarget("C")]!.title).toBe("Ébloui");
    // Nothing after Z: the nearest before.
    expect(alpha.jumpTarget("Z")).toBe(sorted.findIndex((x) => x.title === "Zorro 1"));
    expect(alpha.letterAt(sorted.findIndex((x) => x.title === "Batman 1"))).toBe("B");
  });

  it("shows only for title sorts of a whole listing of more than 30", () => {
    const p = fitParams(null, "movies");
    expect(showsJump(p, 31, false)).toBe(true);
    expect(showsJump(p, 30, false)).toBe(false);
    expect(showsJump(p, 9_000, true)).toBe(false);
    expect(showsJump({ ...p, sort: "year" }, 100, false)).toBe(false);
  });
});

describe("a library's sort and filters", () => {
  it("offers each kind its own sorts, in order", () => {
    expect(sortsFor("music")).toEqual(["title", "added", "user_rating", "last_played"]);
    expect(sortsFor("shows")).not.toContain("duration");
    expect(sortsFor("movies")).toContain("bitrate");
  });

  it("puts titles A to Z and everything else newest or highest first", () => {
    expect(defaultOrder("title")).toBe("asc");
    expect(defaultOrder("added")).toBe("desc");
  });

  it("drops what a kind doesn't have", () => {
    expect(fitParams({ sort: "duration", watched: false, genre: "Rock" }, "music")).toEqual({ sort: "title", order: "asc", watched: null, genre: null, year: null });
    expect(fitParams({ sort: "year", order: "asc", watched: false }, "movies")).toMatchObject({ sort: "year", order: "asc", watched: false });
  });

  it("counts by kind", () => {
    expect(countLabel(1500, "movies")).toBe("1,500 movies");
    expect(countLabel(1, "shows")).toBe("1 show");
    expect(countLabel(12, "music")).toBe("12 artists");
  });
});

describe("Home", () => {
  const cw: Hub = { key: "continue_watching", title: "Continue Watching", library_id: null, items: [summary({ id: 5, kind: "episode", added_at: 10 })] };
  const movies: Hub = { key: "recently_added:1", title: "Recently Added in Movies", library_id: 1, items: [summary({ id: 6, added_at: 99 })] };
  const music: Hub = { key: "recently_added:3", title: "Recently Added in Music", library_id: 3, items: [summary({ id: 7, kind: "album", added_at: 200 })] };

  it("draws Continue Watching wide, music square, the rest as posters", () => {
    expect(rowKind(cw, [3])).toBe("landscape");
    expect(rowKind(music, [3])).toBe("square");
    expect(rowKind(movies, [3])).toBe("poster");
  });

  it("offers See all for a library's newest only", () => {
    expect(seeAllOf(movies)).toBe(1);
    expect(seeAllOf(cw)).toBeNull();
  });

  it("puts Continue Watching's first in the hero, else the newest video", () => {
    expect(heroOf([cw, movies])!.id).toBe(5);
    expect(heroOf([movies, music])!.id).toBe(6);
  });
});

describe("signing in", () => {
  it("shows a code as two halves", () => {
    expect(displayCode("bcdfghjk")).toBe("BCDF-GHJK");
    expect(codeHalves("BCDF-GHJK")).toEqual(["BCDF", "GHJK"]);
    expect(codeHalves("ABC")).toEqual(["ABC"]);
    expect(typedAddress("http://192.168.1.20:7539/link")).toBe("192.168.1.20:7539/link");
  });

  it("says why a profile switch was refused", () => {
    const refusal = (reason: string | null, error = "invalid_grant", description: string | null = null) =>
      switchRefusal(new TokenError(400, { error, description, reason } as OAuthError));
    expect(refusal("wrong_pin").message).toBe("Wrong PIN.");
    expect(refusal("pin_locked").locked).toBe(true);
    expect(refusal("browser").approve).toBe(true);
    expect(refusal(null, "invalid_request").letThrough).toBe(true);
    expect(refusal("throttled", "invalid_grant", "too many tries: try again in 5 minutes").message).toBe("Too many tries: try again in 5 minutes.");
  });
});

describe("finding a server", () => {
  function http(answers: Record<string, number | "down" | "slow" | object>): Transport {
    return async (req: HttpRequest) => {
      const a = answers[req.url];
      if (a === undefined || a === "down") throw new NetworkError("Can't reach the server");
      if (a === "slow") throw new NetworkError("The server took too long to answer");
      if (typeof a === "number") return { status: a, header: () => null, body: null };
      return { status: 200, header: () => "application/json", body: a };
    };
  }

  it("names servers by host and port", () => {
    expect(hostPort("http://192.168.1.20:7539")).toBe("192.168.1.20:7539");
    expect(hostPort("https://pley.example.com")).toBe("pley.example.com");
    expect(subnetOf("192.168.1.42")).toBe("192.168.1.");
    expect(subnetOf("127.0.0.1")).toBeNull();
    expect(subnetOf(null)).toBeNull();
  });

  it("tries https first for a name on the internet, then pley's port", async () => {
    const t = http({ "http://pley.example.com:7539/api/server": { name: "Home", version: "1.0.0" } });
    await expect(connect(t, "pley.example.com")).resolves.toMatchObject({ url: "http://pley.example.com:7539", name: "Home" });
  });

  it("says what answered when it isn't pley, and why nothing did", async () => {
    await expect(connect(http({ "http://router:7539/api/server": 404 }), "router")).rejects.toMatchObject({ problem: { kind: "not_pley", address: "router:7539" } });
    await expect(connect(http({}), "192.168.1.77")).rejects.toMatchObject({ problem: { kind: "unreachable", address: "192.168.1.77:7539", reason: "nothing answers there" } });
    await expect(check(http({ "http://nas:7539/api/server": "slow" }), "http://nas:7539")).rejects.toMatchObject({ problem: { reason: "no answer in time" } });
    await expect(connect(http({}), "not an address")).rejects.toMatchObject({ problem: { kind: "bad_address" } });
  });
});

describe("music", () => {
  it("shuffles with the chosen track first", () => {
    const list = [1, 2, 3, 4, 5, 6];
    const s = shuffled(list, 3, () => 0.5);
    expect(s[0]).toBe(4);
    expect(s.slice().sort()).toEqual(list);
  });
});

describe("watch history", () => {
  it("says when, in words", () => {
    const now = 1_800_000_000_000;
    expect(relativeTime(now / 1000 - 10, now)).toBe("now");
    expect(relativeTime(now / 1000 - 3 * 3600, now)).toBe("3 hours ago");
    expect(relativeTime(now / 1000 - 26 * 3600, now)).toBe("yesterday");
  });
});
