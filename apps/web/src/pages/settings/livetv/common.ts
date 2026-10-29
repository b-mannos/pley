import type { LivePlan, XtreamAccount } from "@/api/livetv";

const dateFormat = new Intl.DateTimeFormat(undefined, { day: "numeric", month: "short", year: "numeric" });

/** "Active · expires 12 Mar 2027 · 2 connections". */
export function accountLine(a: XtreamAccount): string {
  const parts = [a.trial ? `${a.status} (trial)` : a.status];
  if (a.expires_at) parts.push(`${a.expires_at * 1000 < Date.now() ? "expired" : "expires"} ${dateFormat.format(new Date(a.expires_at * 1000))}`);
  else parts.push("doesn't expire");
  if (a.max_connections != null) parts.push(`${a.max_connections} ${a.max_connections === 1 ? "connection" : "connections"}`);
  return parts.join(" · ");
}

/** Days left before an account runs out, when that's soon (two weeks). */
export function expiresSoon(a: XtreamAccount | null): number | null {
  if (!a?.expires_at) return null;
  const days = Math.ceil((a.expires_at * 1000 - Date.now()) / 86_400_000);
  return days <= 14 ? days : null;
}

const CODECS: Record<string, string> = {
  h264: "H.264",
  hevc: "HEVC",
  mpeg2video: "MPEG-2",
  aac: "AAC",
  ac3: "Dolby Digital",
  eac3: "Dolby Digital Plus",
  mp2: "MP2",
  mp3: "MP3",
};

const ENCODERS: Record<string, string> = {
  h264_nvenc: "NVIDIA",
  h264_qsv: "Intel Quick Sync",
  h264_amf: "AMD",
  h264_videotoolbox: "VideoToolbox",
  libx264: "the processor",
};

/** What the transcoder makes of a channel: "1080i MPEG-2 → H.264 on NVIDIA · MP2 → AAC". */
export function planLine(p: LivePlan): string {
  const lines = p.height ? `${p.height}${p.interlaced ? "i" : "p"}` : "";
  const source = [lines, CODECS[p.video_codec ?? ""] ?? p.video_codec].filter(Boolean).join(" ");
  const video =
    p.video === "none"
      ? "radio"
      : p.video === "copy"
        ? `${source} as it is`
        : `${source} → H.264 on ${ENCODERS[p.video] ?? p.video}${p.interlaced ? ", deinterlaced" : ""}`;
  const audioIn = CODECS[p.audio_codec ?? ""] ?? p.audio_codec ?? "";
  const audio = p.audio === "none" ? "" : p.audio === "copy" ? `${audioIn} as it is` : `${audioIn} → AAC`;
  return [video, audio].filter(Boolean).join(" · ");
}

/** "4.2 Mbit/s". */
export function formatKbps(kbps: number): string {
  if (kbps >= 1000) return `${(kbps / 1000).toFixed(1)} Mbit/s`;
  return `${kbps} kbit/s`;
}

/** "for 12 min", since `since`. */
export function formatFor(since: number): string {
  const minutes = Math.max(0, Math.floor((Date.now() / 1000 - since) / 60));
  if (minutes < 1) return "just now";
  if (minutes < 60) return `for ${minutes} min`;
  const h = Math.floor(minutes / 60);
  return `for ${h} h ${minutes % 60} min`;
}
