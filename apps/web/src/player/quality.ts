import type { MediaFile, PlaybackMode } from "@/api/types";
import type { Rung } from "./autoQuality";

/**
 * A quality of the menu (docs/QUALITY.md, the same on every app): the
 * original, Automatic, or a conversion capped at a bitrate and a height.
 */
export interface QualityOption {
  id: string;
  label: string;
  /** kbps */
  maxBitrate: number | null;
  maxHeight: number | null;
}

export const ORIGINAL: QualityOption = { id: "original", label: "Original", maxBitrate: null, maxHeight: null };
/** The original, lower only when it doesn't play well (autoQuality.ts). */
export const AUTO: QualityOption = { id: "auto", label: "Automatic", maxBitrate: null, maxHeight: null };

/** The 4K qualities first, then 1080p and below. */
export const QUALITIES: QualityOption[] = [
  ORIGINAL,
  AUTO,
  { id: "30m", label: "30 Mbps 4K", maxBitrate: 30_000, maxHeight: 2160 },
  { id: "15m", label: "15 Mbps 4K", maxBitrate: 15_000, maxHeight: 2160 },
  { id: "20m", label: "20 Mbps 1080p", maxBitrate: 20_000, maxHeight: 1080 },
  { id: "12m", label: "12 Mbps 1080p", maxBitrate: 12_000, maxHeight: 1080 },
  { id: "8m", label: "8 Mbps 1080p", maxBitrate: 8_000, maxHeight: 1080 },
  { id: "4m", label: "4 Mbps 720p", maxBitrate: 4_000, maxHeight: 720 },
  { id: "2m", label: "2 Mbps 480p", maxBitrate: 2_000, maxHeight: 480 },
];

export function qualityById(id: string | null | undefined): QualityOption {
  return QUALITIES.find((q) => q.id === id) ?? ORIGINAL;
}

/**
 * The qualities worth offering for a file: the original and Automatic, then
 * those that make it smaller (every one when its bitrate and size are unknown).
 */
export function qualitiesFor(file: Pick<MediaFile, "bitrate" | "height"> | null): QualityOption[] {
  const kbps = file?.bitrate ? file.bitrate / 1000 : null;
  const height = file?.height ?? null;
  if (kbps == null && height == null) return QUALITIES;
  return QUALITIES.filter(
    (q) =>
      q.maxBitrate === null || (kbps != null && q.maxBitrate < kbps) || (height != null && (q.maxHeight ?? 0) < height),
  );
}

/**
 * The quality to play: the one picked when this file offers it; the
 * original when it doesn't need that one (an 8 Mbps cap on a 5 Mbps file);
 * Automatic when none was ever picked.
 */
export function chooseQuality(picked: string | null | undefined, offered: QualityOption[]): QualityOption {
  if (!picked || !QUALITIES.some((q) => q.id === picked)) return AUTO;
  return offered.find((q) => q.id === picked) ?? ORIGINAL;
}

/**
 * How a quality is asked for: the original lets the server choose (direct
 * play when the browser can); any other quality is a transcode.
 */
export function modeFor(quality: QualityOption): PlaybackMode {
  return quality.maxBitrate === null ? "auto" : "transcode";
}

/**
 * What the server converts the original at (its closest_quality): the
 * file's lines, the highest bitrate up to 1.5 times the file's, else that
 * resolution's lowest. Automatic never goes above it.
 */
export function serverQuality(file: Pick<MediaFile, "bitrate" | "width" | "height"> | null): QualityOption {
  const w = file?.width ?? 0;
  const h = file?.height ?? 0;
  const lines = w >= 3800 || h >= 2100 ? 2160 : w >= 1800 || h >= 1000 || (w <= 0 && h <= 0) ? 1080 : w >= 1200 || h >= 700 ? 720 : 480;
  const cap = file?.bitrate && file.bitrate > 0 ? (file.bitrate * 1.5) / 1000 : null;
  const tier = QUALITIES.filter((q) => q.maxHeight === lines);
  return tier.find((q) => cap === null || (q.maxBitrate ?? 0) <= cap) ?? tier[tier.length - 1]!;
}

/**
 * The qualities Automatic steps through below the original, highest first:
 * those offered for the file, at no more lines and bitrate than the
 * server's own quality for it, each lower in bitrate than the one above
 * (a 4K file goes 30 Mbps 4K, 15 Mbps 4K, then 12 Mbps 1080p).
 */
export function autoLadder(file: Pick<MediaFile, "bitrate" | "width" | "height"> | null): Rung[] {
  const top = serverQuality(file);
  const out: Rung[] = [];
  let below = Infinity;
  for (const q of qualitiesFor(file)) {
    if (q.maxBitrate === null || q.maxHeight === null) continue;
    if (q.maxHeight > (top.maxHeight ?? 0) || q.maxBitrate > (top.maxBitrate ?? 0) || q.maxBitrate >= below) continue;
    out.push({ id: q.id, label: q.label, kbps: q.maxBitrate, height: q.maxHeight });
    below = q.maxBitrate;
  }
  return out;
}
