import type { HwAccel, SystemInfo, ToneMapping, TranscoderInfo } from "@/api/types";

/**
 * What Settings → Server offers for transcoding on this server: each choice
 * with why it can't work here, as the server judges when saving
 * (settings.rs, `hwaccel_problem` and `tonemap_problem`). A choice saved
 * before (another GPU, another FFmpeg) stays chosen and says what happens
 * instead.
 */
export interface Choice<T> {
  value: T;
  label: string;
  /** Why it can't work here ("needs an NVIDIA graphics card"), or `null`. */
  reason: string | null;
}

const ACCEL_NAME: Record<HwAccel, string> = {
  auto: "Automatic",
  nvenc: "NVIDIA NVENC",
  qsv: "Intel Quick Sync",
  amf: "AMD AMF",
  vaapi: "VAAPI",
  videotoolbox: "VideoToolbox",
  vulkan: "Vulkan (h264_vulkan)",
  none: "Software only (libx264)",
};

const ACCEL_NEEDS: Partial<Record<HwAccel, string>> = {
  nvenc: "needs an NVIDIA graphics card",
  qsv: "needs an Intel graphics chip",
  amf: "needs an AMD graphics card",
  vaapi: "needs an Intel or AMD graphics chip",
  videotoolbox: "needs an FFmpeg with VideoToolbox",
};

/** The menu's order; Vulkan only when saved (it is set through the API). */
const ACCELS: HwAccel[] = ["auto", "nvenc", "qsv", "amf", "vaapi", "videotoolbox", "none"];

const TONEMAP_NAME: Record<ToneMapping, string> = {
  auto: "Automatic",
  libplacebo: "libplacebo (GPU, best quality, Dolby Vision)",
  opencl: "OpenCL (GPU)",
  zscale: "zscale (CPU)",
  none: "Disabled (HDR may look washed out)",
};

const TONEMAP_SHORT: Partial<Record<ToneMapping, string>> = { libplacebo: "libplacebo", opencl: "OpenCL", zscale: "zscale" };

const TONEMAP_NEEDS: Partial<Record<ToneMapping, string>> = {
  libplacebo: "needs libplacebo and a Vulkan GPU",
  opencl: "needs a GPU with OpenCL",
  zscale: "this FFmpeg has no zscale",
};

const TONEMAPS: ToneMapping[] = ["auto", "libplacebo", "opencl", "zscale", "none"];

export const capitalize = (text: string) => text.charAt(0).toUpperCase() + text.slice(1);

/** The transcoder, when it runs and its FFmpeg says what it can do; `null` while nobody knows. */
export function knownTranscoder(system: SystemInfo | undefined): TranscoderInfo | null {
  const tc = system?.transcoder;
  // An FFmpeg that encodes nothing isn't there: what the GPU could do is unknown.
  return tc?.available && Array.isArray(tc.accels) && tc.encoders.length > 0 ? tc : null;
}

/** The transcoder's operating system: pley's when pley starts it, unknown when it runs elsewhere. */
export function transcoderOs(system: SystemInfo | undefined): string | null {
  return system?.os && system.transcoder.managed !== false ? system.os : null;
}

/** Why `accel` can never work on `os`, or `null`. */
function osReason(accel: HwAccel, os: string | null): string | null {
  if (!os) return null;
  if (accel === "vaapi" && os !== "linux") return "on Linux only";
  if (accel === "videotoolbox" && os !== "macos") return "on macOS only";
  if ((accel === "nvenc" || accel === "qsv" || accel === "amf") && os === "macos") return "not on macOS";
  return null;
}

/** Why `accel` can't work on this server, or `null` (as far as anyone knows). */
export function accelReason(accel: HwAccel, system: SystemInfo | undefined): string | null {
  if (accel === "auto" || accel === "none" || accel === "vulkan") return null;
  const os = osReason(accel, transcoderOs(system));
  if (os) return os;
  const tc = knownTranscoder(system);
  return tc && !tc.accels.includes(accel) ? (ACCEL_NEEDS[accel] ?? null) : null;
}

/**
 * The hardware accelerations to offer: those this system never has are left
 * out (VideoToolbox on Windows), unless saved; those it lacks the GPU or
 * FFmpeg for are offered with the reason.
 */
export function accelChoices(saved: HwAccel, system: SystemInfo | undefined): Choice<HwAccel>[] {
  const tc = knownTranscoder(system);
  const os = transcoderOs(system);
  const values = ACCELS.includes(saved) ? ACCELS : [...ACCELS.slice(0, -1), saved, "none" as const];
  return values
    .filter((value) => value === saved || !osReason(value, os))
    .map((value) => {
      const reason = accelReason(value, system);
      let label = ACCEL_NAME[value];
      if (value === "auto") {
        const first = tc?.accels[0];
        label = tc ? `Automatic · ${first ? ACCEL_NAME[first] : "software encoding"}` : "Automatic (best available)";
      } else if (reason) {
        label = `${label} (${reason})`;
      }
      return { value, label, reason };
    });
}

/** The hardware acceleration transcodes get with `accel` chosen: `null` for software or unknown. */
export function accelInUse(accel: HwAccel, system: SystemInfo | undefined): HwAccel | null {
  const tc = knownTranscoder(system);
  if (!tc || accel === "none") return null;
  if (accel === "auto") return tc.accels[0] ?? null;
  return accelReason(accel, system) ? null : accel;
}

export const accelName = (accel: HwAccel) => ACCEL_NAME[accel];

/** The tone mappers to offer, each with why it can't work here. */
export function tonemapChoices(system: SystemInfo | undefined): Choice<ToneMapping>[] {
  const tc = knownTranscoder(system);
  return TONEMAPS.map((value) => {
    const reason = tonemapReason(value, system);
    let label = TONEMAP_NAME[value];
    if (value === "auto" && tc) {
      const first = tc.tonemapping[0] as ToneMapping | undefined;
      label = `Automatic · ${first ? (TONEMAP_SHORT[first] ?? first) : "none available"}`;
    } else if (reason) {
      label = `${TONEMAP_SHORT[value] ?? value} (${reason})`;
    }
    return { value, label, reason };
  });
}

/** Why the tone mapper `mode` names can't work here, or `null`. */
export function tonemapReason(mode: ToneMapping, system: SystemInfo | undefined): string | null {
  const tc = knownTranscoder(system);
  if (!tc || mode === "auto" || mode === "none") return null;
  return tc.tonemapping.includes(mode) ? null : (TONEMAP_NEEDS[mode] ?? null);
}

/** What a saved tone mapper that can't work here gives instead: Automatic's choice. */
export function tonemapFallback(system: SystemInfo | undefined): string {
  const first = knownTranscoder(system)?.tonemapping[0] as ToneMapping | undefined;
  return first
    ? `HDR is tone mapped with ${TONEMAP_SHORT[first] ?? first} instead.`
    : "No tone mapper works here, so HDR may look washed out.";
}
