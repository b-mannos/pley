import type { HwAccel, Settings, SystemInfo, ToneMapping, TranscoderInfo } from "@/api/types";

/**
 * What Settings → Transcoder offers on this server: each choice
 * with why it can't work here, or with the hardware acceleration chosen, as
 * the server judges when saving (settings.rs, `hwaccel_problem`,
 * `tonemap_problem` and `fit_to_accel`). A choice saved before (another
 * GPU, another FFmpeg) stays chosen and says what happens instead.
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
  vulkan: "Vulkan",
  none: "Software only",
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
  libplacebo: "libplacebo",
  opencl: "OpenCL",
  zscale: "zscale",
  none: "Disabled",
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
        label = tc ? `Automatic · ${first ? ACCEL_NAME[first] : "software encoding"}` : "Automatic";
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

/** A choice's text in its menu: with why it is greyed out, when it is. */
export const choiceLabel = (choice: Choice<unknown>) => (choice.reason ? `${choice.label} (${choice.reason})` : choice.label);

/** The tone mappers to offer with `accel` chosen, each with why it can't work here or with it. */
export function tonemapChoices(system: SystemInfo | undefined, accel: HwAccel): Choice<ToneMapping>[] {
  const tc = knownTranscoder(system);
  return TONEMAPS.map((value) => {
    const reason = tonemapReason(value, system) ?? tonemapAccelReason(value, accel, system);
    let label = TONEMAP_NAME[value];
    if (value === "auto" && tc) {
      const first = tc.tonemapping[0] as ToneMapping | undefined;
      label = `Automatic · ${first ? (TONEMAP_SHORT[first] ?? first) : "none available"}`;
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

/**
 * Whether the tone mapper `mode` goes with `accel`: Vulkan encodes on the
 * GPU only what libplacebo tone maps there (the transcoder's Vulkan
 * pipeline), and the rest in software. The others tone map before their
 * encoder, with any tone mapper.
 */
export const tonemapGoesWith = (accel: HwAccel, mode: ToneMapping) => accel !== "vulkan" || mode === "auto" || mode === "libplacebo";

/**
 * Why the tone mapper `mode` doesn't go with `accel`, or `null`: Vulkan's
 * rule, and the tone mappers whose frames didn't reach a VAAPI encoder in
 * the transcoder's test (it tone maps with another then).
 */
export function tonemapAccelReason(mode: ToneMapping, accel: HwAccel, system?: SystemInfo): string | null {
  if (!tonemapGoesWith(accel, mode)) return `not with ${ACCEL_NAME[accel]}`;
  const works = knownTranscoder(system)?.with_accel?.[accel]?.tonemappers;
  if (!works || mode === "auto" || mode === "none" || works.includes(mode)) return null;
  return `not with ${ACCEL_NAME[accel]}`;
}

/**
 * Why hardware decoding changes nothing with `accel` chosen, or `null`: as
 * the transcoder says each choice decodes here (a software encoder still
 * gets CUDA's decoder on an NVIDIA computer). Vulkan's are its own: it
 * decodes on the GPU, so hardware decoding is on with it.
 */
export function decodingReason(accel: HwAccel, system: SystemInfo | undefined): string | null {
  const tc = knownTranscoder(system);
  if (!tc || accel === "vulkan") return null;
  if (!tc.hardware_decoding) return "no graphics card here that FFmpeg decodes on";
  if (tc.with_accel?.[accel]?.hardware_decoding !== false) return null;
  const encodes = accelInUse(accel, system);
  return `not with ${encodes ? ACCEL_NAME[encodes] : "software encoding"}`;
}

/** What follows from choosing `accel`, for what `form` holds that doesn't go with it. */
export interface AccelFit {
  changes: Partial<Pick<Settings, "tonemapping" | "hardware_decoding">>;
  /** What changed and why, by setting. */
  notes: { tonemapping?: string; hardware_decoding?: string };
}

/**
 * What `form` changes to with `accel` chosen, as the server does when it
 * saves (`fit_to_accel`): a tone mapper that doesn't go with it gives way to
 * Automatic, and hardware decoding is turned on for Vulkan.
 */
export function fitToAccel(form: Pick<Settings, "tonemapping" | "hardware_decoding">, accel: HwAccel, system?: SystemInfo): AccelFit {
  const fit: AccelFit = { changes: {}, notes: {} };
  if (!tonemapGoesWith(accel, form.tonemapping)) {
    fit.changes.tonemapping = "auto";
    fit.notes.tonemapping = `Set to Automatic: ${ACCEL_NAME[accel]} tone maps with libplacebo only.`;
  } else if (tonemapAccelReason(form.tonemapping, accel, system)) {
    fit.changes.tonemapping = "auto";
    fit.notes.tonemapping = `Set to Automatic: ${TONEMAP_SHORT[form.tonemapping] ?? form.tonemapping} doesn't reach ${ACCEL_NAME[accel]} here.`;
  }
  if (accel === "vulkan" && !form.hardware_decoding) {
    fit.changes.hardware_decoding = true;
    fit.notes.hardware_decoding = `Turned on: ${ACCEL_NAME[accel]} decodes on the GPU.`;
  }
  return fit;
}

/**
 * What the software encoder preset applies to with `accel` chosen, when a
 * GPU encodes: what the transcoder leaves to software with it (HEVC, or
 * HDR kept in 10-bit HEVC), and the transcodes software takes over.
 * `null` with software encoding, where it applies to everything.
 */
export function presetNote(accel: HwAccel, system: SystemInfo | undefined): string | null {
  const encodes = accelInUse(accel, system);
  if (!encodes) return null;
  const name = ACCEL_NAME[encodes];
  const software = knownTranscoder(system)?.with_accel?.[accel]?.software ?? [];
  if (software.includes("hevc")) return `${name} encodes H.264 here: this applies to HEVC, and when software encodes instead.`;
  if (software.includes("hdr")) return `${name} encodes here: this applies to transcodes that keep HDR, and when software encodes instead.`;
  return `${name} encodes here: this applies only when software encodes instead.`;
}

/**
 * Why transcodes' segments can't be kept in memory here, or `null`: as the
 * transcoder says (nobody knows while it isn't running), and only
 * throttled transcodes are, since their lead on the player is bounded.
 */
export function memoryReason(system: SystemInfo | undefined, throttle: boolean): string | null {
  const tc = system?.transcoder;
  if (tc?.available) {
    if (!tc.memory) return "needs a newer transcoder";
    if (!tc.memory.available) return tc.memory.reason ?? "not available here";
  }
  return !throttle || knownTranscoder(system)?.throttling === false ? "needs throttling" : null;
}

/** What a saved tone mapper that can't work here gives instead: Automatic's choice. */
export function tonemapFallback(system: SystemInfo | undefined): string {
  const first = knownTranscoder(system)?.tonemapping[0] as ToneMapping | undefined;
  return first
    ? `HDR is tone mapped with ${TONEMAP_SHORT[first] ?? first} instead.`
    : "No tone mapper works here, so HDR may look washed out.";
}
