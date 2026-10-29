import type { ClientCaps, VideoDecoderCaps } from "pley-tv/api/types";
import type { CapsOverrides } from "pley-tv/platform";
import type { WebosFacts } from "./facts";

/**
 * What an LG TV plays, reported to the server (capabilities version 2) so
 * it sends files as they are whenever the TV's pipeline takes them. From
 * LG's format pages per webOS version, the TV's own HDR and sound flags,
 * and what other players found on real sets (docs/TV_PLATFORMS.md).
 * Pure: the same facts give the same report.
 */

/** Containers the pipeline opens from a URL, by the server's names. */
const CONTAINERS = ["mkv", "mp4", "m4v", "mov", "ts", "m2ts", "mpegts", "avi", "mpg", "mpeg", "mp3", "m4a", "aac", "flac", "ogg", "oga", "wav", "mka"];

/** Subtitles the app draws itself: text as WebVTT from the server, pictures from their .mks. */
const SUBTITLES = ["subrip", "ass", "ssa", "webvtt", "mov_text", "hdmv_pgs_subtitle", "dvd_subtitle", "dvb_subtitle"];

// ffprobe's levels: H.264 5.1 = 51; HEVC 4.1 = 123, 5.1 = 153, 6.1 = 183.
const H264_FHD = 42;
const H264_UHD = 51;
const HEVC_FHD = 123;
const HEVC_UHD = 153;
const HEVC_8K = 183;

export interface CapsResult {
  caps: ClientCaps;
  lines: string[];
  /** Containers the TV shows Dolby Vision from; others play their base layer, so a remux keeps it. */
  dvContainers: string[];
}

export function webosCaps(f: WebosFacts, o: CapsOverrides = { truehd: false, dts: false }): CapsResult {
  const v = f.webos;
  const uhd = f.uhd;
  const video: VideoDecoderCaps[] = [];
  const h264Profiles = ["Constrained Baseline", "Baseline", "Main", "High"];
  if (uhd) {
    // 4K H.264 at 60 only on webOS 4.0 sets; later ones promise 30 (some 60).
    video.push({ codec: "h264", hardware: true, profiles: h264Profiles, max_width: 3840, max_height: 2160, max_fps: v === 4 ? 60 : 30, max_bit_depth: 8, max_level: H264_UHD });
  }
  video.push({ codec: "h264", hardware: true, profiles: h264Profiles, max_width: 1920, max_height: 1080, max_fps: 60, max_bit_depth: 8, max_level: H264_FHD });
  const hevcProfiles = ["Main", "Main 10"];
  if (f.uhd8k) video.push({ codec: "hevc", hardware: true, profiles: hevcProfiles, max_width: 7680, max_height: 4320, max_fps: 60, max_bit_depth: 10, max_level: HEVC_8K });
  else if (uhd) video.push({ codec: "hevc", hardware: true, profiles: hevcProfiles, max_width: 4096, max_height: 2160, max_fps: 60, max_bit_depth: 10, max_level: HEVC_UHD });
  else video.push({ codec: "hevc", hardware: true, profiles: hevcProfiles, max_width: 1920, max_height: 1080, max_fps: 60, max_bit_depth: 10, max_level: HEVC_FHD });
  if (v >= 5) {
    const big = f.uhd8k ? { max_width: 7680, max_height: 4320 } : uhd ? { max_width: 3840, max_height: 2160 } : { max_width: 1920, max_height: 1080 };
    video.push({ codec: "vp9", hardware: true, profiles: ["Profile 0", "Profile 2"], ...big, max_fps: 60, max_bit_depth: 10 });
    video.push({ codec: "av1", hardware: true, profiles: ["Main"], ...big, max_fps: 60, max_bit_depth: 10 });
  } else {
    // webOS 4: VP9 from Matroska only, with no size promised.
    video.push({ codec: "vp9", hardware: true, profiles: ["Profile 0"], max_width: 1920, max_height: 1080, max_fps: 60, max_bit_depth: 8 });
  }
  video.push({ codec: "mpeg2video", hardware: true, max_width: 1920, max_height: 1080, max_fps: 60, max_bit_depth: 8 });
  video.push({ codec: "mpeg4", hardware: true, max_width: 1920, max_height: 1080, max_fps: 30, max_bit_depth: 8 });

  const hdrTypes: ClientCaps["hdr_types"] = [];
  if (f.hdr10) hdrTypes.push("hdr10", "hlg");
  if (f.dolbyVision && v >= 4) hdrTypes.push("dolby_vision");
  const dvProfiles = f.dolbyVision && v >= 4 ? [{ profile: 5 }, { profile: 8 }] : [];

  // Sound: what the TV decodes, and what goes to a receiver as it is.
  const decode = ["aac", "ac3", "eac3", "mp3", "mp2", "flac", "vorbis", "pcm_s16le", "pcm_s24le"];
  if (v >= 4) decode.push("opus");
  // DTS: decoded by 2018–2019 sets; dropped in 2020, back on some 2023–2024 sets, dropped again in 2025.
  const dtsDecoded = v === 4;
  if (dtsDecoded) decode.push("dts");
  const toReceiver = !!f.soundOutput && /^external_(arc|optical)/.test(f.soundOutput) && f.digitalOutput !== "pcm";
  const passthrough: string[] = [];
  if (toReceiver) {
    passthrough.push("ac3", "eac3");
    const edid = (f.edid ?? "").toLowerCase();
    if (o.dts || (!dtsDecoded && edid.indexOf("dts") >= 0 && (v === 23 || v === 24))) passthrough.push("dts");
    // TrueHD from an app isn't known to reach a receiver on any LG: only when asked for.
    if (o.truehd) passthrough.push("truehd");
  } else if (o.dts && !dtsDecoded) {
    passthrough.push("dts");
  }
  const channels = toReceiver || f.atmos ? 8 : f.soundOutput && f.soundOutput.indexOf("tv_") === 0 ? 2 : 6;

  const caps: ClientCaps = {
    containers: CONTAINERS,
    video_codecs: unique(video.map((x) => x.codec)),
    audio_codecs: unique(decode.concat(passthrough)),
    hdr: f.hdr10,
    dolby_vision: dvProfiles.length > 0,
    caps_version: 2,
    video,
    hdr_types: hdrTypes,
    dolby_vision_profiles: dvProfiles,
    audio_decode: decode,
    audio_passthrough: unique(passthrough),
    audio_channels: channels,
    subtitle_codecs: SUBTITLES,
    audio_track_switching: v >= 4,
    picture_subtitles_sideload: true,
    // One size per stream (no `variants`): LG's pipeline may go black moving between sizes, as between streams (docs/QUALITY.md).
    hls: { master_playlist: true, segments: v >= 5 ? ["ts", "fmp4"] : ["ts"] },
    display: f.screen ? { width: f.screen.width, height: f.screen.height, refresh_rates: [50, 60] } : undefined,
    device: { form: "tv", tier: tierOf(f), fire_tv: false, model: f.model },
  };
  const dvContainers = v >= 25 ? ["mp4", "m4v", "mov", "ts", "m2ts", "mpegts", "mkv"] : ["mp4", "m4v", "mov", "ts", "m2ts", "mpegts"];
  return { caps, lines: lines(f, caps, dvContainers), dvContainers };
}

/** Low for 2018–2019 sets and small-memory ones; high from webOS 22 on. */
export function tierOf(f: WebosFacts): "low" | "mid" | "high" {
  const gb = parseFloat((f.memory ?? "").replace(/[^0-9.]/g, ""));
  if (f.webos <= 4 || (gb > 0 && gb < 1.5)) return "low";
  return f.webos >= 22 ? "high" : "mid";
}

function unique(list: string[]): string[] {
  return list.filter((x, i) => list.indexOf(x) === i);
}

function lines(f: WebosFacts, caps: ClientCaps, dvContainers: string[]): string[] {
  const out = [
    `TV: ${f.model}${f.firmware ? ` (firmware ${f.firmware})` : ""}`,
    `webOS ${f.webos}${f.chromium ? ` · Chromium ${f.chromium}` : ""}`,
    `Panel: ${f.uhd8k ? "8K" : f.uhd ? "4K" : "HD"}${f.oled ? " OLED" : ""}`,
    `HDR: ${caps.hdr_types.length ? caps.hdr_types.map(hdrName).join(", ") : "none"}`,
  ];
  if (caps.dolby_vision_profiles.length) {
    out.push(`Dolby Vision: profiles ${caps.dolby_vision_profiles.map((p) => p.profile).join(", ")}, from ${dvContainers.join(", ").toUpperCase()}`);
  }
  out.push(`Video: ${caps.video.map((x) => `${x.codec} ${x.max_width}×${x.max_height}@${x.max_fps}`).join(", ")}`);
  out.push(`Audio decoded: ${caps.audio_decode.join(", ")}`);
  out.push(`Sent to the receiver: ${caps.audio_passthrough.length ? caps.audio_passthrough.join(", ") : "nothing"}`);
  out.push(`Sound: ${f.soundOutput ?? "unknown"}${f.digitalOutput ? ` (digital ${f.digitalOutput})` : ""}${f.atmos ? " · Atmos" : ""}`);
  out.push(`Streams: HLS ${caps.hls.segments.join(", ")}`);
  return out;
}

function hdrName(t: string): string {
  return t === "hdr10" ? "HDR10" : t === "hdr10plus" ? "HDR10+" : t === "hlg" ? "HLG" : "Dolby Vision";
}

/**
 * What a desktop browser plays (development only): asked of the browser
 * itself, since no LG table applies.
 */
export function browserCaps(): CapsResult {
  const v = document.createElement("video");
  const can = (type: string) => v.canPlayType(type) !== "";
  const video: VideoDecoderCaps[] = [{ codec: "h264", hardware: true, profiles: ["Constrained Baseline", "Baseline", "Main", "High"], max_width: 3840, max_height: 2160, max_fps: 60, max_bit_depth: 8 }];
  if (can('video/mp4; codecs="hvc1.1.6.L153.B0"')) video.push({ codec: "hevc", hardware: true, profiles: ["Main", "Main 10"], max_width: 3840, max_height: 2160, max_fps: 60, max_bit_depth: 10 });
  if (can('video/webm; codecs="vp09.00.40.08"')) video.push({ codec: "vp9", hardware: false, profiles: ["Profile 0"], max_width: 3840, max_height: 2160, max_fps: 60, max_bit_depth: 8 });
  if (can('video/mp4; codecs="av01.0.08M.08"')) video.push({ codec: "av1", hardware: false, profiles: ["Main"], max_width: 3840, max_height: 2160, max_fps: 60, max_bit_depth: 10 });
  const audio = ["aac", "mp3", "opus", "flac", "vorbis"];
  const caps: ClientCaps = {
    containers: ["mp4", "m4v", "mov", "webm", "mkv", "mp3", "m4a", "flac", "ogg", "wav"],
    video_codecs: video.map((x) => x.codec),
    audio_codecs: audio,
    hdr: false,
    dolby_vision: false,
    caps_version: 2,
    video,
    hdr_types: [],
    dolby_vision_profiles: [],
    audio_decode: audio,
    audio_passthrough: [],
    audio_channels: 2,
    subtitle_codecs: SUBTITLES,
    audio_track_switching: false,
    picture_subtitles_sideload: true,
    hls: { master_playlist: true, segments: ["ts", "fmp4"] },
    device: { form: "tv", tier: "high", fire_tv: false, model: "Browser" },
  };
  return { caps, lines: ["A desktop browser, for development: it plays what it says it plays."], dvContainers: [] };
}
