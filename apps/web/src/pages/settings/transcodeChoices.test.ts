import { describe, expect, it } from "vitest";
import type { HwAccel, SystemInfo, ToneMapping } from "@/api/types";
import { decodingReason, fitToAccel, presetNote, tonemapGoesWith } from "./transcodeChoices";

/** Pley on Windows with an NVIDIA card whose NVENC encodes HEVC, but not in 10 bits. */
const nvidia = {
  os: "windows",
  transcoder: {
    available: true,
    managed: true,
    encoders: ["h264_nvenc", "hevc_nvenc", "libx264", "libx265"],
    accels: ["nvenc"],
    hardware_decoding: true,
    tonemapping: ["zscale"],
    with_accel: {
      auto: { hardware_decoding: true, software: ["hdr"] },
      nvenc: { hardware_decoding: true, software: ["hdr"] },
      none: { hardware_decoding: true, software: ["h264", "hevc", "hdr"] },
    },
  },
} as unknown as SystemInfo;

describe("what goes with the hardware acceleration chosen", () => {
  it("keeps Vulkan's tone mapping on libplacebo, and takes every tone mapper with the others", () => {
    const modes: ToneMapping[] = ["auto", "libplacebo", "opencl", "zscale", "none"];
    expect(modes.filter((mode) => tonemapGoesWith("vulkan", mode))).toEqual(["auto", "libplacebo"]);
    const others: HwAccel[] = ["auto", "none", "nvenc", "qsv", "amf", "vaapi", "videotoolbox"];
    for (const accel of others) expect(modes.every((mode) => tonemapGoesWith(accel, mode))).toBe(true);
  });

  it("falls back to what goes with it, and says so", () => {
    expect(fitToAccel({ tonemapping: "opencl", hardware_decoding: false }, "vulkan")).toEqual({
      changes: { tonemapping: "auto", hardware_decoding: true },
      notes: {
        tonemapping: "Set to Automatic: Vulkan tone maps with libplacebo only.",
        hardware_decoding: "Turned on: Vulkan decodes on the GPU.",
      },
    });
    expect(fitToAccel({ tonemapping: "libplacebo", hardware_decoding: true }, "vulkan")).toEqual({ changes: {}, notes: {} });
    expect(fitToAccel({ tonemapping: "opencl", hardware_decoding: false }, "nvenc")).toEqual({ changes: {}, notes: {} });
  });

  it("decodes with CUDA on an NVIDIA computer, software encoding too", () => {
    expect(decodingReason("none", nvidia)).toBeNull();
    expect(decodingReason("nvenc", nvidia)).toBeNull();
    // Nobody knows before the transcoder says.
    expect(decodingReason("none", undefined)).toBeNull();
  });

  it("says what the software preset applies to", () => {
    expect(presetNote("nvenc", nvidia)).toBe("NVIDIA NVENC encodes here: this applies to transcodes that keep HDR, and when software encodes instead.");
    expect(presetNote("none", nvidia)).toBeNull();
  });
});
