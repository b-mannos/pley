import { describe, expect, it } from "vitest";
import { activeCues, cueAtTop, DEFAULT_SUBTITLE_STYLE, subtitleFontSize, subtitleStyle, type SubtitleCue } from "./subtitles";

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
