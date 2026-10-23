import { describe, expect, it } from "vitest";
import { activeCues, cueAtTop, DEFAULT_SUBTITLE_STYLE, parseVtt, subtitleFontSize, subtitleStyle, type SubtitleCue } from "./subtitles";

function cue(text: string, extra: Partial<SubtitleCue> = {}): SubtitleCue {
  return {
    startTime: 0,
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
    expect(subtitleStyle({ size: "large", background: "box" })).toEqual({ size: "large", background: "box" });
    expect(subtitleStyle({ size: "gigantic", background: 3 })).toEqual(DEFAULT_SUBTITLE_STYLE);
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

describe("activeCues", () => {
  it("lists the cues showing now, in order", () => {
    const later = cue("second", { startTime: 5 });
    const earlier = cue("first", { startTime: 2 });
    const notVtt = { startTime: 1 };
    const list = Object.assign([later, notVtt, earlier], { getCueById: () => null });
    const track = { activeCues: list } as unknown as TextTrack;
    expect(activeCues(track).map((c) => c.text)).toEqual(["first", "second"]);
    expect(activeCues({ activeCues: null } as unknown as TextTrack)).toEqual([]);
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
