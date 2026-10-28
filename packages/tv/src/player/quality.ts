import type { MediaFile, PlaybackMode } from "../api/types";
import { formatBitrate, resolutionLabel } from "../format/format";
import type { Rung } from "./autoquality";

/**
 * The qualities the player offers (docs/QUALITY.md, the same on every
 * app): the original, Automatic, or a conversion capped at a bitrate and a
 * height. Ids are what the server's `auto_quality` names.
 */
export interface QualityPreset {
  id: string;
  label: string;
  maxBitrateKbps: number | null;
  maxHeight: number | null;
}

export const ORIGINAL: QualityPreset = { id: "original", label: "Original", maxBitrateKbps: null, maxHeight: null };
/** The original, lower only when it doesn't play well (autoquality.ts). */
export const AUTO: QualityPreset = { id: "auto", label: "Automatic", maxBitrateKbps: null, maxHeight: null };

/** The 4K qualities first, then 1080p and below. */
export const PRESETS: QualityPreset[] = [
  ORIGINAL,
  AUTO,
  { id: "30m", label: "30 Mbps 4K", maxBitrateKbps: 30000, maxHeight: 2160 },
  { id: "15m", label: "15 Mbps 4K", maxBitrateKbps: 15000, maxHeight: 2160 },
  { id: "20m", label: "20 Mbps 1080p", maxBitrateKbps: 20000, maxHeight: 1080 },
  { id: "12m", label: "12 Mbps 1080p", maxBitrateKbps: 12000, maxHeight: 1080 },
  { id: "8m", label: "8 Mbps 1080p", maxBitrateKbps: 8000, maxHeight: 1080 },
  { id: "4m", label: "4 Mbps 720p", maxBitrateKbps: 4000, maxHeight: 720 },
  { id: "2m", label: "2 Mbps 480p", maxBitrateKbps: 2000, maxHeight: 480 },
];

export function presetById(id: string | null | undefined): QualityPreset | null {
  return PRESETS.filter((p) => p.id === id)[0] ?? null;
}

/** The mode a quality asks for: the original lets the server pick, the others convert. Automatic asks as what it plays. */
export function modeOf(preset: QualityPreset): PlaybackMode {
  return preset.maxBitrateKbps === null ? "auto" : "transcode";
}

function heightOf(file: MediaFile | null): number | null {
  const video = file?.streams.filter((s) => s.kind === "video")[0];
  return file?.height ?? video?.height ?? null;
}

function widthOf(file: MediaFile | null): number | null {
  const video = file?.streams.filter((s) => s.kind === "video")[0];
  return file?.width ?? video?.width ?? null;
}

/**
 * The qualities worth offering for `file`: the original and Automatic, and
 * each preset below the file's bitrate or height; every one when neither
 * is known.
 */
export function qualitiesFor(file: MediaFile | null): QualityPreset[] {
  const kbps = file?.bitrate ? file.bitrate / 1000 : null;
  const height = heightOf(file);
  if (kbps == null && height == null) return PRESETS.slice();
  return PRESETS.filter(
    (p) =>
      p.maxBitrateKbps === null || (kbps != null && p.maxBitrateKbps < kbps) || (height != null && p.maxHeight! < height),
  );
}

/**
 * The quality to start with: the remembered one when this file offers it;
 * the original when it doesn't need that one (an 8 Mbps cap on a 5 Mbps
 * file); Automatic when none was ever chosen.
 */
export function chooseQuality(remembered: string | null, offered: QualityPreset[]): QualityPreset {
  if (!remembered || !presetById(remembered)) return AUTO;
  return offered.filter((p) => p.id === remembered)[0] ?? ORIGINAL;
}

/**
 * What the server converts the original at (its closest_quality): the
 * file's lines, the highest bitrate up to 1.5 times the file's, else that
 * resolution's lowest. Automatic never goes above it.
 */
export function serverQuality(file: MediaFile | null): QualityPreset {
  const w = widthOf(file) ?? 0;
  const h = heightOf(file) ?? 0;
  const lines = w >= 3800 || h >= 2100 ? 2160 : w >= 1800 || h >= 1000 || (w <= 0 && h <= 0) ? 1080 : w >= 1200 || h >= 700 ? 720 : 480;
  const cap = file?.bitrate && file.bitrate > 0 ? (file.bitrate * 1.5) / 1000 : null;
  const tier = PRESETS.filter((p) => p.maxHeight === lines);
  return tier.filter((p) => cap === null || p.maxBitrateKbps! <= cap)[0] ?? tier[tier.length - 1]!;
}

/**
 * The qualities Automatic steps through below the original, highest first:
 * those offered for the file, at no more lines and bitrate than the
 * server's own quality for it, each lower in bitrate than the one above
 * (a 4K file goes 30 Mbps 4K, 15 Mbps 4K, then 12 Mbps 1080p).
 */
export function autoLadder(file: MediaFile | null): Rung[] {
  const top = serverQuality(file);
  const out: Rung[] = [];
  let below = Infinity;
  for (const p of qualitiesFor(file)) {
    if (p.maxBitrateKbps === null || p.maxHeight === null) continue;
    if (p.maxHeight > top.maxHeight! || p.maxBitrateKbps > top.maxBitrateKbps! || p.maxBitrateKbps >= below) continue;
    out.push({ id: p.id, label: p.label, kbps: p.maxBitrateKbps, height: p.maxHeight });
    below = p.maxBitrateKbps;
  }
  return out;
}

/** "1080p · 8.2 Mbps": what a file is, for the Original row's hint. */
export function fileHint(file: MediaFile | null): string {
  if (!file) return "";
  const video = file.streams.filter((s) => s.kind === "video")[0];
  return [resolutionLabel(file.width ?? video?.width, file.height ?? video?.height), formatBitrate(file.bitrate)].filter(Boolean).join(" · ");
}
