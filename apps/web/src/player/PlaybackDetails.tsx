import type { ReactNode } from "react";
import { usePlaybackStatus } from "@/api/hooks";
import type { MediaFile, MediaStream, PlaybackInfo, PlaybackMethod, TranscodeInfo, TranscodePipeline } from "@/api/types";
import { codecLabel, dynamicRangeLabel, formatBitrate, resolutionLabel } from "@/lib/format";

/** What each way of playing is called (as in Plex). */
export const METHOD_LABEL: Record<PlaybackMethod, string> = {
  direct: "Direct play",
  remux: "Direct stream",
  transcode: "Transcoding",
};

/** How a session plays, in a few words: a direct stream says when its audio is converted. */
export function methodLabel(info: PlaybackInfo): string {
  return info.method === "remux" && info.audio_converted ? `${METHOD_LABEL.remux} · Audio transcoding` : METHOD_LABEL[info.method];
}

/** What it means, when nothing more specific is known; direct play goes without saying. */
const METHOD_TEXT: Record<Exclude<PlaybackMethod, "direct">, string> = {
  remux: "The original video, in a format this player opens.",
  transcode: "Converted for this player.",
};

const ENCODERS: Record<string, string> = {
  libx264: "x264",
  libx265: "x265",
  h264_nvenc: "NVIDIA NVENC",
  hevc_nvenc: "NVIDIA NVENC",
  h264_qsv: "Intel Quick Sync",
  hevc_qsv: "Intel Quick Sync",
  h264_amf: "AMD AMF",
  hevc_amf: "AMD AMF",
  h264_vaapi: "VA-API",
  hevc_vaapi: "VA-API",
  h264_videotoolbox: "VideoToolbox",
  hevc_videotoolbox: "VideoToolbox",
  h264_vulkan: "Vulkan",
  hevc_vulkan: "Vulkan",
  h264_v4l2m2m: "V4L2",
};

const PIPELINE: Record<TranscodePipeline, string> = {
  gpu: "GPU",
  hybrid: "GPU and CPU",
  cpu: "CPU",
  copy: "copy",
};

/** A video stream as `"HEVC 4K Dolby Vision P8.1"`, `"HEVC 4K Dolby Vision P7 FEL"`. */
export function videoLabel(v: MediaStream | undefined): string {
  if (!v) return "";
  return [codecLabel(v.codec), resolutionLabel(v.width, v.height), dynamicRangeLabel(v)].filter(Boolean).join(" ");
}

/**
 * What the screen gets of HDR video: the live transcode's tone mapping when
 * there is one, else what the server said ("HDR10 (Dolby Vision base
 * layer)"), else nothing (SDR video, or an older server).
 */
export function shownAsLabel(info: PlaybackInfo, live: TranscodeInfo | null): string | null {
  if (live?.tonemap) return `SDR (tone-mapped with ${live.tonemap})`;
  return info.video_range ?? null;
}

/** An audio stream as `"TrueHD 7.1"`. */
export function audioLabel(a: MediaStream | undefined): string {
  if (!a) return "";
  const channels = a.channels ? ({ 1: "1.0", 2: "2.0", 3: "2.1", 6: "5.1", 7: "6.1", 8: "7.1" } as Record<number, string>)[a.channels] : null;
  return [codecLabel(a.codec), channels ?? (a.channels ? `${a.channels}ch` : null)].filter(Boolean).join(" ");
}

/** The transcoder's `"H.264 1920×1080"` as `"H.264 1080p"`. */
export function simplified(text: string): string {
  return text.replace(/(\d+)×(\d+)/, (whole, w: string, h: string) => resolutionLabel(Number(w), Number(h)) ?? whole);
}

/** The encoder, and where it runs: `"NVIDIA NVENC (GPU)"`, `"x264 veryfast (CPU)"`. */
export function encoderLabel(t: TranscodeInfo): string {
  const name = t.encoder ? (ENCODERS[t.encoder] ?? t.encoder) : null;
  const where = t.pipeline && t.pipeline !== "copy" ? `(${PIPELINE[t.pipeline]})` : null;
  return [name, t.preset, where].filter(Boolean).join(" ");
}

function Row({ label, children }: { label: string; children: ReactNode }) {
  return (
    <>
      <dt className="text-white/45">{label}</dt>
      <dd className="min-w-0 text-white/85">{children}</dd>
    </>
  );
}

/**
 * How this video plays, at the top of the player's menu: the way (direct
 * play, direct stream, transcoding), why in a sentence, then what goes in
 * and what comes out. A transcode's numbers come live while the menu is open.
 */
export function PlaybackDetails({
  info,
  file,
  audioIndex,
  note,
  positionMs,
  open,
}: {
  info: PlaybackInfo | null;
  file: MediaFile | null;
  /** The audio track playing. */
  audioIndex: number | null;
  /** Why it plays this way, from the player (a fallback after the original didn't play). */
  note: string | null;
  positionMs: number;
  open: boolean;
}) {
  const status = usePlaybackStatus(info?.session_id ?? null, open && !!info && info.method !== "direct");
  if (!info) return null;
  const live = info.method === "direct" ? null : (status.data?.transcode ?? null);
  const streams = file?.streams ?? [];
  const video = streams.find((s) => s.kind === "video");
  const audios = streams.filter((s) => s.kind === "audio");
  const audio =
    audios.find((s) => s.index === (audioIndex ?? info.audio_index)) ?? audios.find((s) => s.default) ?? audios[0];
  const why = info.method === "direct" ? note : (note ?? info.reasons?.join(" ") ?? info.reason ?? METHOD_TEXT[info.method]);
  const pending = "…";

  const videoIn = live?.video_in ? simplified(live.video_in) : videoLabel(video);
  const audioIn = live?.audio_in ?? audioLabel(audio);
  const converting = info.method !== "direct";
  const videoRow =
    converting && live?.video !== "copy"
      ? `${videoIn} → ${live?.video_out ? `${simplified(live.video_out)}${live.tonemap ? " SDR" : ""}` : pending}`
      : videoIn;
  const audioRow =
    converting && live?.audio !== "copy" ? `${audioIn} → ${live?.audio_out ?? pending}` : audioIn;
  const bitrate = formatBitrate(file?.bitrate);
  const bitrateRow = live?.bitrate_out && bitrate ? `${bitrate} → ${formatBitrate(live.bitrate_out)}` : bitrate;
  const ahead = live ? Math.max(0, Math.round((live.transcoded_ms - positionMs) / 1000)) : 0;
  const shownAs = video ? shownAsLabel(info, live) : null;

  return (
    <div className="px-4 pb-3 pt-1">
      <div className="flex items-center justify-between gap-2">
        <span className="text-sm font-semibold text-accent">{methodLabel(info)}</span>
        {live?.pipeline && live.video === "transcode" ? (
          <span className="rounded bg-white/10 px-1.5 py-0.5 text-[10px] font-semibold tracking-wide text-white/75">
            {PIPELINE[live.pipeline]}
          </span>
        ) : null}
      </div>
      {why ? <p className="mt-0.5 text-xs leading-snug text-white/65">{why}</p> : null}
      <dl className="mt-2.5 grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 text-xs tabular-nums">
        {video ? <Row label="Video">{videoRow}</Row> : null}
        {audio ? <Row label="Audio">{audioRow}</Row> : null}
        {bitrateRow ? <Row label="Bitrate">{bitrateRow}</Row> : null}
        {live?.video === "transcode" && live.encoder ? <Row label="Encoder">{encoderLabel(live)}</Row> : null}
        {shownAs ? <Row label="Shown as">{shownAs}</Row> : null}
        {live?.subtitle_burn ? <Row label="Subtitles">Drawn onto the video</Row> : null}
        {live?.speed ? (
          <Row label="Speed">
            {live.speed.toFixed(1)}× real time{ahead > 0 ? ` · ${ahead} s ready` : ""}
          </Row>
        ) : null}
      </dl>
    </div>
  );
}
