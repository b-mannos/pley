import { DvbDecoder } from "./dvb";
import type { PictureTrack } from "./mks";
import { PgsDecoder } from "./pgs";
import type { PictureDecoder, PictureFrame } from "./pictures";
import { VobSubDecoder } from "./vobsub";

/** Codec ids of the picture subtitles this app draws, and the server's (ffprobe's) names for them. */
export const PICTURE_CODECS: Record<string, string> = {
  "S_HDMV/PGS": "hdmv_pgs_subtitle",
  S_VOBSUB: "dvd_subtitle",
  S_DVBSUB: "dvb_subtitle",
};

export function decoderFor(track: PictureTrack): PictureDecoder | null {
  const data = track.samples.map((s) => s.data);
  switch (track.codecId) {
    case "S_HDMV/PGS":
      return new PgsDecoder(data);
    case "S_VOBSUB":
      return new VobSubDecoder(data, track.codecPrivate);
    case "S_DVBSUB":
      return new DvbDecoder(data);
    default:
      return null;
  }
}

/**
 * A picture track ready to show: a sample shows from its time until the
 * next sample, or for its own duration when it carries one (VobSub's stop
 * command, DVB's page time-out, a Matroska block duration); an empty
 * sample clears. Decoded only when shown.
 */
export class PictureSubtitles {
  private readonly track: PictureTrack;
  private readonly decoder: PictureDecoder;
  private readonly times: number[];

  constructor(track: PictureTrack, decoder: PictureDecoder) {
    this.track = track;
    this.decoder = decoder;
    this.times = track.samples.map((s) => s.timeMs);
  }

  static of(track: PictureTrack): PictureSubtitles | null {
    const decoder = decoderFor(track);
    return decoder ? new PictureSubtitles(track, decoder) : null;
  }

  /** The sample showing at `ms` (track time), or -1. */
  sampleAt(ms: number): number {
    // The last sample starting at or before `ms`.
    let lo = 0;
    let hi = this.times.length;
    while (lo < hi) {
      const mid = (lo + hi) >> 1;
      if (this.times[mid]! <= ms) lo = mid + 1;
      else hi = mid;
    }
    const i = lo - 1;
    if (i < 0) return -1;
    const end = this.endOf(i);
    return ms < end ? i : -1;
  }

  /** When sample `i` stops showing. */
  endOf(i: number): number {
    const start = this.times[i]!;
    const next = i + 1 < this.times.length ? this.times[i + 1]! : Infinity;
    const own = this.track.samples[i]!.durationMs ?? this.decoder.ownDurationMs(i);
    return own !== null && own > 0 ? Math.min(next, start + own) : next;
  }

  /** What shows at `ms`: a frame (maybe empty), or `null` when nothing does. */
  frameAt(ms: number): { index: number; frame: PictureFrame } | null {
    const i = this.sampleAt(ms);
    if (i < 0) return null;
    const frame = this.decoder.decode(i);
    return frame && frame.bitmaps.length ? { index: i, frame } : null;
  }

  /** When what shows changes after `ms`; `null` when nothing does any more. */
  nextChange(ms: number): number | null {
    const i = this.sampleAt(ms);
    if (i >= 0) {
      const end = this.endOf(i);
      return end === Infinity ? null : end;
    }
    let lo = 0;
    let hi = this.times.length;
    while (lo < hi) {
      const mid = (lo + hi) >> 1;
      if (this.times[mid]! <= ms) lo = mid + 1;
      else hi = mid;
    }
    return lo < this.times.length ? this.times[lo]! : null;
  }
}
