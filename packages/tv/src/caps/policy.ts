import type { ClientCaps, DynamicRange, ItemDetail, MediaFile, MediaStream, VideoDecoderCaps } from "../api/types";

/** Whether a video plays as it is here, and if not, what the server will do about it. */
export type CapsDecision =
  | { kind: "direct" }
  /** Dolby Vision this TV can't decode, on an HDR10 screen: the server sends the HDR10 base layer. */
  | { kind: "remux_base_layer" }
  /** The screen can't show its HDR: the server converts it. */
  | { kind: "server_tone_maps" }
  | { kind: "unsupported"; reason: string };

const BL_HDR10 = 1;
const BL_SDR = 2;
const BL_HLG = 4;
const RATE_SLACK = 0.01;

/** Profiles both ffprobe and the TVs name the same way: missing from a decoder's list, they refuse a stream. */
const KNOWN_PROFILES = [
  "main", "main 10", "main still picture", "rext", "baseline", "constrained baseline", "high", "high 10",
  "high 4:2:2", "high 4:4:4 predictive", "profile 0", "profile 1", "profile 2", "profile 3", "professional",
];

/** The main video stream of a file. */
export function videoOf(file: MediaFile | null | undefined): MediaStream | null {
  const videos = (file?.streams ?? []).filter((s) => s.kind === "video");
  return videos.filter((s) => s.default)[0] ?? videos[0] ?? null;
}

/** The video's dynamic range, as the server reads it. */
export function rangeOf(video: MediaStream | null): DynamicRange {
  if (!video) return "sdr";
  if (video.dynamic_range) return video.dynamic_range;
  if (video.dv_profile != null) return "dolby_vision";
  return "sdr";
}

/** The stream's bit depth, else from its pixel format ("yuv420p10le") or profile ("Main 10"). */
export function bitDepth(video: MediaStream): number {
  if (video.bit_depth) return video.bit_depth;
  const m = /(1[02])(?:le|be)$/.exec(video.pix_fmt ?? "");
  if (m) return Number(m[1]);
  return (video.profile ?? "").indexOf("10") >= 0 ? 10 : 8;
}

/**
 * The client's side of the server's version 2 rules (docs/API.md): what
 * the server will do with a file for this report. Pure.
 */
export function decide(file: MediaFile, caps: ClientCaps): CapsDecision {
  const video = videoOf(file);
  if (!video) return { kind: "direct" };
  const types = caps.hdr_types;
  const range = rangeOf(video);
  switch (range) {
    case "dolby_vision":
      return dolbyVision(video, caps);
    case "hdr10":
    case "hdr10plus":
    case "hlg": {
      const shown =
        range === "hlg" ? types.indexOf("hlg") >= 0 : range === "hdr10plus" ? types.indexOf("hdr10plus") >= 0 || types.indexOf("hdr10") >= 0 : types.indexOf("hdr10") >= 0;
      return shown ? fits(video, video.codec, caps, true) : { kind: "server_tone_maps" };
    }
    default:
      return fits(video, video.codec, caps, false);
  }
}

function dolbyVision(video: MediaStream, caps: ClientCaps): CapsDecision {
  const profile = video.dv_profile ?? null;
  const types = caps.hdr_types;
  const decoded = caps.dolby_vision_profiles.filter((p) => p.profile === profile)[0];
  if (types.indexOf("dolby_vision") >= 0 && decoded) {
    const level = video.dv_level;
    const max = decoded.max_level;
    if (level != null && max != null && level > max) return { kind: "unsupported", reason: `Dolby Vision level ${level} is above this TV's ${max}` };
    return { kind: "direct" };
  }
  const base = profile === 9 ? "h264" : profile === 10 ? "av1" : video.codec === "dvhe" || video.codec === "dvh1" ? "hevc" : video.codec;
  if (profile === 5) return { kind: "server_tone_maps" };
  if (profile === 7) return types.indexOf("hdr10") >= 0 ? { kind: "remux_base_layer" } : { kind: "server_tone_maps" };
  const compat = video.dv_bl_compat ?? (profile === 8 ? BL_HDR10 : null);
  if (compat === BL_HDR10) return types.indexOf("hdr10") >= 0 ? fits(video, base, caps, true) : { kind: "server_tone_maps" };
  if (compat === BL_HLG) return types.indexOf("hlg") >= 0 ? fits(video, base, caps, true) : { kind: "server_tone_maps" };
  if (compat === BL_SDR) return fits(video, base, caps, false);
  return { kind: "server_tone_maps" };
}

function fits(video: MediaStream, codec: string, caps: ClientCaps, hdr: boolean): CapsDecision {
  const entries = caps.video.filter((e) => e.codec.toLowerCase() === codec.toLowerCase());
  if (!entries.length) return { kind: "unsupported", reason: `No decoder for ${codec} here` };
  const depth = bitDepth(video);
  if (hdr && !entries.some((e) => e.hardware && (e.max_bit_depth ?? 8) >= 10)) {
    return { kind: "unsupported", reason: `No 10-bit hardware decoder for ${codec} here` };
  }
  if (entries.some((e) => takes(e, video, depth))) return { kind: "direct" };
  const size = [video.width, video.height].filter((v) => v != null).join("×");
  const fps = video.frame_rate ? ` at ${video.frame_rate.toFixed(3)} fps` : "";
  return { kind: "unsupported", reason: `${codec} ${size}${fps}, ${depth}-bit${video.profile ? `, ${video.profile}` : ""}, is beyond this TV's decoders` };
}

export function takes(e: VideoDecoderCaps, video: MediaStream, depth: number): boolean {
  const w = video.width;
  const h = video.height;
  if (w != null && h != null && e.max_width != null && e.max_height != null) {
    const asIs = w <= e.max_width && h <= e.max_height;
    const turned = h <= e.max_width && w <= e.max_height;
    if (!asIs && !turned) return false;
  }
  if (video.frame_rate != null && e.max_fps != null && video.frame_rate > e.max_fps + RATE_SLACK) return false;
  if (e.max_bit_depth != null && depth > e.max_bit_depth) return false;
  if (video.level != null && video.level > 0 && e.max_level != null && video.level > e.max_level) return false;
  const profile = video.profile;
  const listed = e.profiles ?? [];
  if (profile && listed.length && !listed.some((p) => p.toLowerCase() === profile.toLowerCase())) {
    if (KNOWN_PROFILES.indexOf(profile.toLowerCase()) >= 0) return false;
  }
  if (listed.length && video.codec === "hevc" && depth >= 10 && !listed.some((p) => /^(main 10|rext)$/i.test(p))) return false;
  return true;
}

function rank(d: CapsDecision): number {
  switch (d.kind) {
    case "direct":
      return 0;
    case "remux_base_layer":
      return 1;
    case "server_tone_maps":
      return 2;
    default:
      return 3;
  }
}

function rangeRank(range: DynamicRange): number {
  switch (range) {
    case "dolby_vision":
      return 4;
    case "hdr10plus":
      return 3;
    case "hdr10":
      return 2;
    case "hlg":
      return 1;
    default:
      return 0;
  }
}

/**
 * The version of `item` that plays best here: the one that plays as it
 * is, then one no bigger than the screen, then the tallest, then (playing
 * as they are) the richest dynamic range, then the highest bitrate.
 */
export function bestFile(item: ItemDetail, caps: ClientCaps): MediaFile | null {
  const screen = caps.display ? Math.min(caps.display.width, caps.display.height) : null;
  const height = (f: MediaFile) => videoOf(f)?.height ?? f.height ?? 0;
  const scored = item.files.map((file) => ({ file, decision: decide(file, caps) }));
  scored.sort(
    (a, b) =>
      rank(a.decision) - rank(b.decision) ||
      (screen === null || height(a.file) <= screen ? 0 : 1) - (screen === null || height(b.file) <= screen ? 0 : 1) ||
      height(b.file) - height(a.file) ||
      (b.decision.kind === "direct" ? rangeRank(rangeOf(videoOf(b.file))) : 0) - (a.decision.kind === "direct" ? rangeRank(rangeOf(videoOf(a.file))) : 0) ||
      (b.file.bitrate ?? 0) - (a.file.bitrate ?? 0),
  );
  return scored[0]?.file ?? null;
}
