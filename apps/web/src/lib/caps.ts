import type { ClientCaps } from "@/api/types";

let videoCache: ClientCaps | null = null;
let audioCache: ClientCaps | null = null;

function probe(el: HTMLMediaElement, type: string): boolean {
  try {
    return el.canPlayType(type) !== "";
  } catch {
    return false;
  }
}

function audioCodecs(el: HTMLMediaElement): string[] {
  const codecs: string[] = [];
  if (probe(el, 'audio/mp4; codecs="mp4a.40.2"')) codecs.push("aac");
  if (probe(el, "audio/mpeg") || probe(el, 'audio/mp4; codecs="mp3"')) codecs.push("mp3");
  if (probe(el, 'audio/webm; codecs="opus"') || probe(el, 'audio/ogg; codecs="opus"') || probe(el, 'audio/mp4; codecs="opus"'))
    codecs.push("opus");
  if (probe(el, "audio/flac") || probe(el, 'audio/mp4; codecs="flac"') || probe(el, 'audio/ogg; codecs="flac"'))
    codecs.push("flac");
  if (probe(el, 'audio/ogg; codecs="vorbis"') || probe(el, 'audio/webm; codecs="vorbis"')) codecs.push("vorbis");
  if (probe(el, 'audio/mp4; codecs="ac-3"')) codecs.push("ac3");
  if (probe(el, 'audio/mp4; codecs="ec-3"')) codecs.push("eac3");
  if (probe(el, 'audio/mp4; codecs="alac"')) codecs.push("alac");
  if (probe(el, 'audio/wav; codecs="1"') || probe(el, "audio/wav")) codecs.push("pcm_s16le", "pcm_s24le");
  return codecs;
}

/** What this browser can direct-play in a `<video>` element. */
export function videoCaps(): ClientCaps {
  if (videoCache) return videoCache;
  const v = document.createElement("video");
  const containers: string[] = [];
  if (probe(v, "video/mp4")) containers.push("mp4", "m4v", "mov");
  if (probe(v, "video/webm")) containers.push("webm");

  const video: string[] = [];
  if (probe(v, 'video/mp4; codecs="avc1.640028"') || probe(v, 'video/mp4; codecs="avc1.42E01E"')) video.push("h264");
  if (probe(v, 'video/mp4; codecs="hvc1.1.6.L120.90"') || probe(v, 'video/mp4; codecs="hev1.1.6.L120.90"'))
    video.push("hevc");
  if (probe(v, 'video/webm; codecs="vp9"') || probe(v, 'video/mp4; codecs="vp09.00.10.08"')) video.push("vp9");
  if (probe(v, 'video/mp4; codecs="av01.0.08M.08"') || probe(v, 'video/webm; codecs="av01.0.08M.08"')) video.push("av1");
  if (probe(v, 'video/webm; codecs="vp8"')) video.push("vp8");

  videoCache = { containers, video_codecs: video, audio_codecs: audioCodecs(v) };
  return videoCache;
}

/** What this browser can direct-play in an `<audio>` element. */
export function audioCaps(): ClientCaps {
  if (audioCache) return audioCache;
  const a = document.createElement("audio");
  const containers: string[] = [];
  if (probe(a, "audio/mpeg")) containers.push("mp3");
  if (probe(a, "audio/mp4")) containers.push("m4a", "mp4", "m4b");
  if (probe(a, "audio/flac")) containers.push("flac");
  if (probe(a, "audio/ogg")) containers.push("ogg", "oga", "opus");
  if (probe(a, "audio/wav")) containers.push("wav");
  if (probe(a, "audio/webm")) containers.push("webm");
  audioCache = { containers, video_codecs: [], audio_codecs: audioCodecs(a) };
  return audioCache;
}

/** Caps that force the server to transcode everything. */
export const NO_CAPS: ClientCaps = { containers: [], video_codecs: [], audio_codecs: [] };

export function canPlayNativeHls(el: HTMLMediaElement): boolean {
  return probe(el, "application/vnd.apple.mpegurl");
}
