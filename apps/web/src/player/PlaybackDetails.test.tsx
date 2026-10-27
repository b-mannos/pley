import { describe, expect, it } from "vitest";
import type { MediaStream, PlaybackInfo, TranscodeInfo } from "@/api/types";
import { methodLabel, shownAsLabel, videoLabel } from "./PlaybackDetails";

const info = (method: PlaybackInfo["method"], audio_converted?: boolean) => ({ method, audio_converted }) as PlaybackInfo;

describe("methodLabel", () => {
  it("says when a direct stream converts the audio", () => {
    expect(methodLabel(info("remux", true))).toBe("Direct stream · Audio transcoding");
    expect(methodLabel(info("remux", false))).toBe("Direct stream");
    expect(methodLabel(info("direct"))).toBe("Direct play");
    expect(methodLabel(info("transcode", true))).toBe("Transcoding");
  });
});

describe("videoLabel", () => {
  const video = (s: Partial<MediaStream>) =>
    ({ index: 0, kind: "video", codec: "hevc", width: 3840, height: 2160, ...s }) as MediaStream;

  it("names Dolby Vision as the server does", () => {
    expect(videoLabel(video({ dynamic_range: "dolby_vision", dv_profile: 8, dv_bl_compat: 1 }))).toBe(
      "HEVC 4K Dolby Vision P8.1",
    );
    expect(videoLabel(video({ dynamic_range: "dolby_vision", dv_profile: 7, dv_bl_compat: 6, dv_el: "fel" }))).toBe(
      "HEVC 4K Dolby Vision P7 FEL",
    );
    expect(videoLabel(video({ dynamic_range: "dolby_vision", dv_profile: 8, dv_bl_compat: 1, hdr10plus: true }))).toBe(
      "HEVC 4K Dolby Vision P8.1 + HDR10+",
    );
    expect(videoLabel(video({ dynamic_range: "hdr10plus" }))).toBe("HEVC 4K HDR10+");
  });
});

describe("shownAsLabel", () => {
  it("says what the screen gets", () => {
    const direct = { ...info("direct"), video_range: "HDR10 (Dolby Vision base layer)" } as PlaybackInfo;
    expect(shownAsLabel(direct, null)).toBe("HDR10 (Dolby Vision base layer)");
    expect(shownAsLabel(info("direct"), null)).toBeNull();
    // The live transcode knows best when it tone-maps.
    const live = { tonemap: "libplacebo" } as TranscodeInfo;
    expect(shownAsLabel({ ...info("transcode"), video_range: "SDR (tone-mapped)" }, live)).toBe(
      "SDR (tone-mapped with libplacebo)",
    );
  });
});
