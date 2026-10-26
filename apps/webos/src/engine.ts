import type { EngineSource, EngineStats } from "pley-tv/player/engine";
import { HLS_MIME, VideoEngine } from "pley-tv/player/video-engine";

interface UmsInfo {
  type?: string;
  info?: Record<string, unknown>;
}

/**
 * LG's media pipeline behind a <video> element. The address goes to the
 * pipeline with a mediaOption: where to start (LG's way, rather than
 * seeking once loaded, which buffers from the beginning first), and the
 * pipeline's own reports (umsmediainfo) for the details panel.
 */
export class WebosEngine extends VideoEngine {
  private ums: { video: Record<string, unknown> | null; audio: Record<string, unknown> | null; source: Record<string, unknown> | null } = {
    video: null,
    audio: null,
    source: null,
  };
  private readonly onUms = (e: Event) => {
    const detail = (e as CustomEvent<string>).detail;
    let msg: UmsInfo;
    try {
      msg = typeof detail === "string" ? (JSON.parse(detail) as UmsInfo) : (detail as UmsInfo);
    } catch {
      return;
    }
    // Each report names its kind once ({videoInfo: {…}}) or as {type, info}.
    const record = msg as Record<string, unknown>;
    const pick = (key: string) => (msg.type === key ? msg.info : (record[key] as Record<string, unknown> | undefined)) ?? null;
    this.ums = {
      video: pick("videoInfo") ?? this.ums.video,
      audio: pick("audioInfo") ?? this.ums.audio,
      source: pick("sourceInfo") ?? this.ums.source,
    };
  };

  constructor(host: HTMLElement) {
    super(host, "webOS media pipeline");
    this.video.addEventListener("umsmediainfo", this.onUms);
  }

  protected override attach(source: EngineSource): void {
    this.ums = { video: null, audio: null, source: null };
    const option: Record<string, unknown> = {
      htmlMediaOption: { useUMSMediaInfo: true },
      option: {
        transmission: { playTime: { start: Math.max(0, Math.round(source.startMs)) } },
        ...(source.hls ? { adaptiveStreaming: { maxWidth: 3840, maxHeight: 2160 } } : {}),
      },
    };
    if (source.hls) option.mediaTransportType = "HLS";
    const el = document.createElement("source");
    el.src = source.url;
    const mime = source.hls ? HLS_MIME : source.mime ?? "video/mp4";
    el.type = `${mime};mediaOption=${encodeURIComponent(JSON.stringify(option))}`;
    this.video.appendChild(el);
  }

  /**
   * LG's pipeline counts frames its own way: until a real TV shows the
   * counts mean what they say, they don't move a playback to a conversion.
   */
  override frameCounts(): { dropped: number; total: number } | null {
    return null;
  }

  override release(): void {
    this.video.removeEventListener("umsmediainfo", this.onUms);
    super.release();
  }

  override stats(): EngineStats | null {
    const base = super.stats() ?? {};
    const v = this.ums.video;
    const a = this.ums.audio;
    const out: EngineStats = { ...base };
    if (v) {
      if (typeof v.width === "number") out.width = v.width;
      if (typeof v.height === "number") out.height = v.height;
      if (typeof v.codec === "string") out.codec = v.codec;
      if (typeof v.bitRate === "number" && v.bitRate > 0) out.bitrate = v.bitRate;
      const fr = v.frameRate as { num?: number; den?: number } | number | undefined;
      if (typeof fr === "number") out.frameRate = fr;
      else if (fr && fr.num && fr.den) out.frameRate = fr.num / fr.den;
      out.hardware = true;
    }
    if (a) {
      if (typeof a.codec === "string") out.audioCodec = a.codec;
      if (typeof a.channels === "number") out.audioChannels = a.channels;
    }
    const hdr = this.ums.source && (this.ums.source.hdrType as string | undefined);
    if (hdr && hdr !== "none") out.hdr = hdr;
    return Object.keys(out).length ? out : null;
  }
}
