import { describe, expect, it } from "vitest";
import type { MediaFile, MediaStream, PlaybackInfo, TranscodeInfo } from "../src/api/types";
import { dynamicRangeBadges, qualityBadges, streamRange } from "../src/format/format";
import { dolbyVisionDetails, streamBlock } from "../src/format/media-info";
import { buildDetails, shownAs, videoLabel } from "../src/player/details";

const video = (s: Partial<MediaStream>): MediaStream =>
  ({ index: 0, kind: "video", codec: "hevc", width: 3840, height: 2160, language: null, title: null, default: true, forced: false, ...s }) as MediaStream;

const p81 = video({ dynamic_range: "dolby_vision", dv_profile: 8, dv_bl_compat: 1, dv_level: 6 });
const p7 = video({ dynamic_range: "dolby_vision", dv_profile: 7, dv_bl_compat: 6, dv_level: 6 });

describe("formats, named as the server, the web and Android name them", () => {
  it("names each Dolby Vision profile", () => {
    expect(streamRange(p81)).toBe("Dolby Vision P8.1");
    expect(streamRange({ ...p81, dv_bl_compat: 4 })).toBe("Dolby Vision P8.4");
    expect(streamRange({ ...p81, dv_profile: 5, dv_bl_compat: 0 })).toBe("Dolby Vision P5");
    // Profile 7's base layer (Blu-ray, compatibility 6) isn't named: never "P7.6".
    expect(streamRange(p7)).toBe("Dolby Vision P7");
    expect(streamRange({ ...p7, dv_el: "fel" })).toBe("Dolby Vision P7 FEL");
    expect(streamRange({ ...p7, dv_el: "mel" })).toBe("Dolby Vision P7 MEL");
  });

  it("names HDR10+, alone and with Dolby Vision", () => {
    expect(streamRange({ ...p81, hdr10plus: true })).toBe("Dolby Vision P8.1 + HDR10+");
    expect(streamRange(video({ dynamic_range: "hdr10plus", hdr10plus: true }))).toBe("HDR10+");
    expect(streamRange(video({ dynamic_range: "hdr10", hdr10plus: false }))).toBe("HDR10");
    expect(streamRange(video({ dynamic_range: "hlg" }))).toBe("HLG");
    expect(streamRange(video({ dynamic_range: "sdr" }))).toBeNull();
  });

  it("badges a hybrid twice", () => {
    expect(dynamicRangeBadges({ ...p81, hdr10plus: true })).toEqual(["DV P8.1", "HDR10+"]);
    expect(dynamicRangeBadges({ ...p7, dv_el: "fel" })).toEqual(["DV P7 FEL"]);
    const file = { id: 1, path: "a.mkv", size: 1, width: 3840, height: 2160, streams: [{ ...p81, hdr10plus: true }] } as unknown as MediaFile;
    expect(qualityBadges(file)).toEqual(["4K", "DV P8.1", "HDR10+"]);
  });

  it("gives the Info panel the profile, level, base layer and enhancement layer", () => {
    expect(dolbyVisionDetails({ ...p7, dv_el: "fel" })).toBe(
      "Profile 7 · Level 6 · Blu-ray HDR10-compatible base layer · full enhancement layer",
    );
    const facts = streamBlock({ ...p81, hdr10plus: true }, "Video").facts;
    expect(facts.filter((f) => f.label === "HDR")[0]?.value).toBe("Dolby Vision P8.1 + HDR10+");
  });
});

describe("the playback details", () => {
  it("label the video as the item page does", () => {
    expect(videoLabel(p81)).toBe("HEVC 4K Dolby Vision P8.1");
    expect(videoLabel({ ...p7, dv_el: "fel" })).toBe("HEVC 4K Dolby Vision P7 FEL");
  });

  it("say what the screen gets", () => {
    const info = { method: "direct", video_range: "HDR10 (Dolby Vision base layer)" } as PlaybackInfo;
    expect(shownAs(info, null)).toBe("HDR10 (Dolby Vision base layer)");
    expect(shownAs({ ...info, video_range: null }, null)).toBeNull();
    expect(shownAs({ ...info, method: "transcode" }, { tonemap: "libplacebo" } as TranscodeInfo)).toBe(
      "SDR (tone-mapped with libplacebo)",
    );
    const file = { id: 1, path: "a.mkv", size: 1, streams: [p81] } as unknown as MediaFile;
    const details = buildDetails({
      info: { ...info, audio_index: null, reasons: [], reason: null } as PlaybackInfo,
      file,
      audioIndex: null,
      note: null,
      positionMs: 0,
      live: null,
      subtitle: null,
      sideload: false,
    });
    expect(details.rows.filter((r) => r.label === "Shown as")[0]?.value).toBe("HDR10 (Dolby Vision base layer)");
  });
});
