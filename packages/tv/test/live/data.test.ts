import { describe, expect, it } from "vitest";
import type { LiveChannel, LiveProgram } from "../../src/api/types";
import {
  behindText,
  cellAt,
  channelByNumber,
  chunkStarts,
  effectiveFilter,
  filterChannels,
  GUIDE_CHUNK,
  groupsOf,
  guideCells,
  mergeGuide,
  nowAndNext,
  parseFilter,
  filterKey,
  recentChannels,
  stepChannel,
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

describe("filters", () => {
  const list = [channel(1, { group: "News" }), channel(2, { group: "Sport", favorite: true }), channel(3, { group: "News" }), channel(4, { group: "" })];

  it("lists the groups in their order with their counts, the ungrouped left out", () => {
    expect(groupsOf(list)).toEqual([
      { name: "News", count: 2 },
      { name: "Sport", count: 1 },
    ]);
  });

  it("round-trips a filter through its key, and falls back to every channel when its group or favourites went", () => {
    for (const f of [{ kind: "all" }, { kind: "favorites" }, { kind: "group", group: "News: UK" }] as const) expect(parseFilter(filterKey(f))).toEqual(f);
    expect(parseFilter(null)).toEqual({ kind: "all" });
    expect(effectiveFilter({ kind: "group", group: "Films" }, list)).toEqual({ kind: "all" });
    expect(effectiveFilter({ kind: "favorites" }, [channel(1)])).toEqual({ kind: "all" });
    expect(filterChannels(list, { kind: "favorites" }).map((c) => c.id)).toEqual([2]);
    expect(filterChannels(list, { kind: "group", group: "News" }).map((c) => c.id)).toEqual([1, 3]);
  });

  it("keeps the channels watched last that are still here, newest first", () => {
    expect(recentChannels(list, [3, 9, 1, 2], 2).map((c) => c.id)).toEqual([3, 1]);
  });
});

describe("changing channels", () => {
  const list = [channel(1), channel(2), channel(3)];

  it("goes round the ends", () => {
    expect(stepChannel(list, 3, 1)?.id).toBe(1);
    expect(stepChannel(list, 1, -1)?.id).toBe(3);
    expect(stepChannel(list, 2, 1)?.id).toBe(3);
    expect(stepChannel([], 1, 1)).toBeNull();
  });

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
