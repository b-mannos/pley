import type { MediaFile, PlaybackMode } from "../api/types";
import { formatBitrate, resolutionLabel } from "../format/format";

/**
 * The qualities the player offers (the Android app's and the web's): the
 * original, or a conversion capped at a bitrate and a height. Ids are what
 * the server's `auto_quality` names.
 */
export interface QualityPreset {
  id: string;
  label: string;
  maxBitrateKbps: number | null;
  maxHeight: number | null;
}

export const ORIGINAL: QualityPreset = { id: "original", label: "Original", maxBitrateKbps: null, maxHeight: null };

export const PRESETS: QualityPreset[] = [
  ORIGINAL,
  { id: "30m", label: "30 Mbps 4K", maxBitrateKbps: 30000, maxHeight: 2160 },
  { id: "20m", label: "20 Mbps 1080p", maxBitrateKbps: 20000, maxHeight: 1080 },
  { id: "15m", label: "15 Mbps 4K", maxBitrateKbps: 15000, maxHeight: 2160 },
  { id: "12m", label: "12 Mbps 1080p", maxBitrateKbps: 12000, maxHeight: 1080 },
  { id: "8m", label: "8 Mbps 1080p", maxBitrateKbps: 8000, maxHeight: 1080 },
  { id: "4m", label: "4 Mbps 720p", maxBitrateKbps: 4000, maxHeight: 720 },
  { id: "2m", label: "2 Mbps 480p", maxBitrateKbps: 2000, maxHeight: 480 },
];

export function presetById(id: string | null | undefined): QualityPreset | null {
  return PRESETS.filter((p) => p.id === id)[0] ?? null;
}

/** The mode a quality asks for: the original lets the server pick, the others convert. */
export function modeOf(preset: QualityPreset): PlaybackMode {
  return preset.id === ORIGINAL.id ? "auto" : "transcode";
}

/**
 * The qualities worth offering for `file`: the original, and each preset
 * below the file's bitrate or height; every one when neither is known.
 */
export function qualitiesFor(file: MediaFile | null): QualityPreset[] {
  const kbps = file?.bitrate ? file.bitrate / 1000 : null;
  const video = file?.streams.filter((s) => s.kind === "video")[0];
  const height = file?.height ?? video?.height ?? null;
  if (kbps == null && height == null) return PRESETS.slice();
  return PRESETS.filter(
    (p) => p === ORIGINAL || (kbps != null && p.maxBitrateKbps! < kbps) || (height != null && p.maxHeight! < height),
  );
}

/** The quality to start with: the remembered one when this file offers it, else the original. */
export function chooseQuality(remembered: string | null, offered: QualityPreset[]): QualityPreset {
  return offered.filter((p) => p.id === remembered)[0] ?? ORIGINAL;
}

/** "1080p · 8.2 Mbps": what a file is, for the Original row's hint. */
export function fileHint(file: MediaFile | null): string {
  if (!file) return "";
  const video = file.streams.filter((s) => s.kind === "video")[0];
  return [resolutionLabel(file.width ?? video?.width, file.height ?? video?.height), formatBitrate(file.bitrate)].filter(Boolean).join(" · ");
}
