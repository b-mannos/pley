import { describe, expect, it, vi } from "vitest";
import {
  clampSubtitleOffset,
  cueAtTop,
  cuesAt,
  DEFAULT_SUBTITLE_STYLE,
  formatSubtitleOffset,
  nextCueChange,
  parseVtt,
  storedSubtitleOffset,
  storeSubtitleOffset,
  subtitleFontSize,
  subtitleStyle,
  type SubtitleCue,
} from "./subtitles";

function cue(text: string, extra: Partial<SubtitleCue> = {}): SubtitleCue {
  return {
    startTime: 0,
    endTime: 1,
    text,
    line: "auto",
    snapToLines: true,
    getCueAsHTML: () => {
      const fragment = document.createDocumentFragment();
      fragment.append(text);
      return fragment;
    },
    ...extra,
  };
}

describe("subtitle style", () => {
  it("comes back from storage, unknown values as the default", () => {
    expect(subtitleStyle({ size: "large", background: "box", font: "mono", position: "raised" })).toEqual({
      size: "large",
      background: "box",
      font: "mono",
      position: "raised",
    });
    // Kept before fonts and positions were offered: those are the default.
    expect(subtitleStyle({ size: "large", background: "box" })).toEqual({ ...DEFAULT_SUBTITLE_STYLE, size: "large", background: "box" });
    expect(subtitleStyle({ size: "gigantic", background: 3, font: "wingdings", position: 9 })).toEqual(DEFAULT_SUBTITLE_STYLE);
    expect(subtitleStyle(null)).toEqual(DEFAULT_SUBTITLE_STYLE);
  });

  it("sizes text as a share of the player's height, never unreadably small", () => {
    expect(subtitleFontSize("normal")).toBe("clamp(15px, 4.8cqh, 96px)");
    expect(subtitleFontSize("huge")).toBe("clamp(15px, 7.6cqh, 96px)");
  });
});

describe("cueAtTop", () => {
  it("keeps dialogue at the bottom and signs placed high at the top", () => {
    expect(cueAtTop(cue("Hello"))).toBe(false);
    expect(cueAtTop(cue("SIGN", { line: 0 }))).toBe(true);
    expect(cueAtTop(cue("SIGN", { line: 10, snapToLines: false }))).toBe(true);
    expect(cueAtTop(cue("Low", { line: 90, snapToLines: false }))).toBe(false);
    expect(cueAtTop(cue("Last line", { line: -1 }))).toBe(false);
  });
});

describe("cuesAt", () => {
  it("lists the cues showing at a time, in order", () => {
    const later = cue("second", { startTime: 5, endTime: 9 });
    const earlier = cue("first", { startTime: 2, endTime: 8 });
    const gone = cue("gone", { startTime: 1, endTime: 5 });
    const cues = [later, gone, earlier];
    expect(cuesAt(cues, 6).map((c) => c.text)).toEqual(["first", "second"]);
    // A cue shows from its start up to (not at) its end.
    expect(cuesAt(cues, 5).map((c) => c.text)).toEqual(["first", "second"]);
    expect(cuesAt(cues, 1).map((c) => c.text)).toEqual(["gone"]);
    expect(cuesAt(cues, 9)).toEqual([]);
  });
});

describe("parseVtt", () => {
  class Cue {
    line: number | "auto" = "auto";
    snapToLines = true;
    startTime: number;
    endTime: number;
    text: string;
    constructor(startTime: number, endTime: number, text: string) {
      this.startTime = startTime;
      this.endTime = endTime;
      this.text = text;
    }
  }

  it("reads times, text and the line, as WebVTT writes them", () => {
    const text = [
      "WEBVTT",
      "",
      "NOTE made by pley",
      "",
      "1",
      "00:00:01.000 --> 00:00:02.500",
      "Hello there",
      "",
      "01:02:03.250 --> 01:02:05.000 line:10% align:center",
      "<i>Up top</i>",
      "second line",
      "",
      "00:10.000 --> 00:11.000 line:0",
      "Short times",
      "",
      "00:00:05.000 --> 00:00:04.000",
      "Backwards: left out",
      "",
      "00:00:06.000 --> 00:00:07.000",
      "",
    ].join("\r\n");
    const cues = parseVtt(text, Cue);
    expect(cues.map((c) => [c.startTime, c.endTime, c.text])).toEqual([
      [1, 2.5, "Hello there"],
      [3723.25, 3725, "<i>Up top</i>\nsecond line"],
      [10, 11, "Short times"],
    ]);
    expect([cues[0]?.line, cues[1]?.line, cues[1]?.snapToLines]).toEqual(["auto", 10, false]);
    expect([cues[2]?.line, cues[2]?.snapToLines]).toEqual([0, true]);
  });

  it("takes SubRip's comma too", () => {
    const cues = parseVtt("WEBVTT\n\n00:00:01,500 --> 00:00:02,000\nComma", Cue);
    expect(cues.map((c) => c.startTime)).toEqual([1.5]);
  });
});

describe("subtitle offset", () => {
  it("moves in tenths of a second, ten seconds at most either way", () => {
    expect(clampSubtitleOffset(0.1 + 0.2)).toBe(0.3);
    expect(clampSubtitleOffset(-0.04)).toBe(0);
    expect(Object.is(clampSubtitleOffset(-0.04), -0)).toBe(false);
    expect(clampSubtitleOffset(12)).toBe(10);
    expect(clampSubtitleOffset(-10.3)).toBe(-10);
    expect(clampSubtitleOffset(Number.NaN)).toBe(0);
  });

  it("is written with its sign", () => {
    expect(formatSubtitleOffset(0.5)).toBe("+0.5 s");
    expect(formatSubtitleOffset(-1.2)).toBe("−1.2 s");
    expect(formatSubtitleOffset(0)).toBe("0.0 s");
  });

  it("is remembered for each file, and forgotten when reset", () => {
    expect(storedSubtitleOffset(1, 3)).toBe(0);
    storeSubtitleOffset(1, 3, 0.5);
    storeSubtitleOffset(1, 4, -1.5);
    expect(storedSubtitleOffset(1, 3)).toBe(0.5);
    expect(storedSubtitleOffset(1, 4)).toBe(-1.5);
    storeSubtitleOffset(1, 3, 0);
    expect(storedSubtitleOffset(1, 3)).toBe(0);
    expect(JSON.parse(window.localStorage.getItem("pley.subtitleOffsets") ?? "")).toEqual({ "u1.f4": -1.5 });
    expect(storedSubtitleOffset(1, null)).toBe(0);
  });

  it("is each profile's own", () => {
    storeSubtitleOffset(1, 3, 2);
    expect(storedSubtitleOffset(2, 3)).toBe(0);
    storeSubtitleOffset(2, 3, -0.5);
    expect([storedSubtitleOffset(1, 3), storedSubtitleOffset(2, 3)]).toEqual([2, -0.5]);
  });

  it("kept by file alone before, goes to the first profile that plays the file", () => {
    window.localStorage.setItem("pley.subtitleOffsets", JSON.stringify({ f3: 1.5, f4: 0.2 }));
    expect(storedSubtitleOffset(2, 3)).toBe(1.5);
    expect(storedSubtitleOffset(1, 3)).toBe(0);
    expect(JSON.parse(window.localStorage.getItem("pley.subtitleOffsets") ?? "")).toEqual({ f4: 0.2, "u2.f3": 1.5 });
  });

  it("keeps working without storage", () => {
    window.localStorage.setItem("pley.subtitleOffsets", "not json");
    expect(storedSubtitleOffset(1, 3)).toBe(0);
    const setItem = vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new DOMException("full", "QuotaExceededError");
    });
    expect(() => storeSubtitleOffset(1, 3, 1)).not.toThrow();
    setItem.mockRestore();
  });

  it("knows when the lines showing change next", () => {
    const cues = [cue("A", { startTime: 1, endTime: 2 }), cue("B", { startTime: 1.5, endTime: 4 })];
    expect(nextCueChange(cues, 0)).toBe(1);
    expect(nextCueChange(cues, 1.2)).toBe(1.5);
    expect(nextCueChange(cues, 1.7)).toBe(2);
    expect(nextCueChange(cues, 3)).toBe(4);
    expect(nextCueChange(cues, 4)).toBeNull();
  });
});
