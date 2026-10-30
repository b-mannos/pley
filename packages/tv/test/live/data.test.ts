import { describe, expect, it } from "vitest";
import type { LiveChannel, LiveProgram } from "../../src/api/types";
import {
  behindText,
  categoryKey,
  categoryName,
  cellAt,
  channelByNumber,
  channelName,
  chunkStarts,
  GUIDE_CHUNK,
  groupTree,
  guideCells,
  mergeGuide,
  nowAndNext,
  parseCategory,
  splitRegion,
  timeLeft,
} from "../../src/live/data";

function program(channel: number, start: number, stop: number, title = `P${start}`): LiveProgram {
  return {
    channel_id: channel,
    start,
    stop,
    title,
    subtitle: null,
    description: null,
    categories: [],
    image: null,
    season: null,
    episode: null,
    is_new: false,
    is_live: false,
    rating: null,
  };
}

function channel(id: number, over: Partial<LiveChannel> = {}): LiveChannel {
  return { id, source_id: 1, name: `Ch ${id}`, number: String(100 + id), group: "News", logo: null, favorite: false, has_guide: true, now: null, next: null, ...over };
}

describe("what's on", () => {
  it("is the channel's now while it airs, then its next once the list is a minute old", () => {
    const c = channel(1, { now: program(1, 0, 600), next: program(1, 600, 1200) });
    expect(nowAndNext(c, 300).on?.start).toBe(0);
    expect(nowAndNext(c, 300).next?.start).toBe(600);
    // The list was asked for before 600: its next is on now, and nothing is known after it.
    expect(nowAndNext(c, 700)).toEqual({ on: c.next, next: null });
    // Nothing on yet: what comes first is next.
    expect(nowAndNext(channel(2, { now: program(2, 900, 1200) }), 300)).toEqual({ on: null, next: program(2, 900, 1200) });
  });

  it("says how long is left as the web does", () => {
    expect(timeLeft({ stop: 600 }, 0)).toBe("10 min left");
    expect(timeLeft({ stop: 3900 }, 0)).toBe("1 h 5 min left");
    expect(timeLeft({ stop: 7200 }, 0)).toBe("2 h left");
    expect(timeLeft({ stop: 10 }, 0)).toBe("1 min left");
  });
});

describe("categories", () => {
  const group = (name: string, region: string | null, label: string, count: number) => ({ name, region, label, count });
  const groups = [group("UK| Sport", "UK", "Sport", 40), group("News", null, "News", 2), group("UK| News", "UK", "News", 10), group("FR| Films", "FR", "Films", 7)];

  it("round-trip through their keys", () => {
    for (const c of [{ kind: "all" }, { kind: "favorites" }, { kind: "recent" }, { kind: "group", group: "UK| Sport" }] as const) expect(parseCategory(categoryKey(c))).toEqual(c);
    expect(parseCategory(null)).toEqual({ kind: "all" });
  });

  it("gather a region's groups under it, a region of one staying a group, in the provider's order", () => {
    const tree = groupTree(groups);
    expect(tree.map((e) => (e.kind === "group" ? e.group.name : `${e.region} (${e.groups.length}, ${e.count})`))).toEqual(["UK (2, 50)", "News", "FR| Films"]);
  });

  it("are named as the web names them", () => {
    expect(categoryName({ kind: "group", group: "UK| Sport" }, groups)).toBe("UK · Sport");
    expect(categoryName({ kind: "group", group: "News" }, groups)).toBe("News");
    expect(categoryName({ kind: "recent" }, groups)).toBe("Recently watched");
    expect(categoryName({ kind: "group", group: "Gone" }, null)).toBe("Gone");
  });

  it("leave out of a channel's name, in a group, the region the group already says", () => {
    const sport = { kind: "group", group: "UK| Sport" } as const;
    expect(channelName({ name: "UK| Sky Sports 1", group: "UK| Sport" }, sport)).toBe("Sky Sports 1");
    expect(channelName({ name: "[UK] Sky Sports 1", group: "UK| Sport" }, sport)).toBe("Sky Sports 1");
    // Another region, or not in a group: the name as it is.
    expect(channelName({ name: "AR| Sport 01", group: "BEE| Sport" }, { kind: "group", group: "BEE| Sport" })).toBe("AR| Sport 01");
    expect(channelName({ name: "UK| Sky Sports 1", group: "UK| Sport" }, { kind: "all" })).toBe("UK| Sky Sports 1");
    expect(splitRegion("UK: Sport")).toEqual(["UK", "Sport"]);
    expect(splitRegion("Sport")).toBeNull();
  });
});

describe("changing channels", () => {
  it("finds a number typed, with or without its leading zeros", () => {
    const numbered = [channel(1, { number: "007" }), channel(2, { number: "101" }), channel(3, { number: "1.1" })];
    expect(channelByNumber(numbered, "101")?.id).toBe(2);
    expect(channelByNumber(numbered, "7")?.id).toBe(1);
    expect(channelByNumber(numbered, "11")).toBeNull();
  });
});

describe("the guide", () => {
  it("asks for the chunks covering what shows, within the two days it spans", () => {
    expect(chunkStarts(0, 3600, 0)).toEqual([0]);
    expect(chunkStarts(GUIDE_CHUNK - 60, GUIDE_CHUNK + 60, 0)).toEqual([0, GUIDE_CHUNK]);
    expect(chunkStarts(-3600, 60, 0)).toEqual([0]);
    expect(chunkStarts(47 * 3600, 60 * 3600, 0)).toEqual([7 * GUIDE_CHUNK]);
  });

  it("merges chunks each programme once, in order", () => {
    const into = new Map<number, LiveProgram[]>();
    mergeGuide(into, { from: 0, to: 100, programs: { "1": [program(1, 60, 120), program(1, 0, 60)] } });
    mergeGuide(into, { from: 100, to: 200, programs: { "1": [program(1, 60, 120), program(1, 120, 300)], "2": [program(2, 0, 300)] } });
    expect(into.get(1)!.map((p) => p.start)).toEqual([0, 60, 120]);
    expect(into.get(2)!.map((p) => p.start)).toEqual([0]);
  });

  it("fills what the guide says nothing of, so the remote doesn't jump over it", () => {
    const cells = guideCells([program(1, 0, 1800), program(1, 3600, 5400), program(1, 5430, 7200)], 900, 9000);
    expect(cells.map((c) => [c.key, c.start, c.stop])).toEqual([
      ["0", 900, 1800],
      ["gap/1800", 1800, 3600],
      ["3600", 3600, 5400],
      // 30 s is no gap of its own.
      ["5430", 5430, 7200],
      ["gap/7200", 7200, 9000],
    ]);
    expect(cellAt(cells, 1000)).toBe(0);
    expect(cellAt(cells, 3600)).toBe(2);
    expect(cellAt(cells, 99_999)).toBe(4);
    // A channel without a guide is one cell.
    expect(guideCells(undefined, 0, 3600)).toEqual([{ key: "gap/0", start: 0, stop: 3600, program: null }]);
  });
});

describe("the timeshift", () => {
  it("says how far behind live", () => {
    expect(behindText(83_000)).toBe("1:23");
    expect(behindText(5_000)).toBe("0:05");
    expect(behindText(12 * 60_000)).toBe("12 min");
  });
});
