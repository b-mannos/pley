import type { MediaFile, PlaybackMode } from "@/api/types";

export interface QualityOption {
  id: string;
  label: string;
  /** kbps */
  maxBitrate: number | null;
  maxHeight: number | null;
}

const ORIGINAL: QualityOption = { id: "original", label: "Original", maxBitrate: null, maxHeight: null };

export const QUALITIES: QualityOption[] = [
  ORIGINAL,
  { id: "30m", label: "30 Mbps 4K", maxBitrate: 30_000, maxHeight: 2160 },
  { id: "20m", label: "20 Mbps 1080p", maxBitrate: 20_000, maxHeight: 1080 },
  { id: "15m", label: "15 Mbps 4K", maxBitrate: 15_000, maxHeight: 2160 },
  { id: "12m", label: "12 Mbps 1080p", maxBitrate: 12_000, maxHeight: 1080 },
  { id: "8m", label: "8 Mbps 1080p", maxBitrate: 8_000, maxHeight: 1080 },
  { id: "4m", label: "4 Mbps 720p", maxBitrate: 4_000, maxHeight: 720 },
  { id: "2m", label: "2 Mbps 480p", maxBitrate: 2_000, maxHeight: 480 },
];

export function qualityById(id: string | null | undefined): QualityOption {
  return QUALITIES.find((q) => q.id === id) ?? ORIGINAL;
}

/**
 * The qualities worth offering for a file: the original, then those that
 * make it smaller (every one when its bitrate and size are unknown).
 */
export function qualitiesFor(file: Pick<MediaFile, "bitrate" | "height"> | null): QualityOption[] {
  const kbps = file?.bitrate ? file.bitrate / 1000 : null;
  const height = file?.height ?? null;
  if (kbps == null && height == null) return QUALITIES;
  return QUALITIES.filter(
    (q) => q === ORIGINAL || (kbps != null && (q.maxBitrate ?? 0) < kbps) || (height != null && (q.maxHeight ?? 0) < height),
  );
}

/**
 * How a quality is asked for: the original lets the server choose (direct
 * play when the browser can); any other quality is a transcode.
 */
export function modeFor(quality: QualityOption): PlaybackMode {
  return quality.id === ORIGINAL.id ? "auto" : "transcode";
}
