import { describe, expect, it } from "vitest";
import type { PlaybackInfo } from "@/api/types";
import { methodLabel } from "./PlaybackDetails";

const info = (method: PlaybackInfo["method"], audio_converted?: boolean) => ({ method, audio_converted }) as PlaybackInfo;

describe("methodLabel", () => {
  it("says when a direct stream converts the audio", () => {
    expect(methodLabel(info("remux", true))).toBe("Direct stream · Audio transcoding");
    expect(methodLabel(info("remux", false))).toBe("Direct stream");
    expect(methodLabel(info("direct"))).toBe("Direct play");
    expect(methodLabel(info("transcode", true))).toBe("Transcoding");
  });
});
