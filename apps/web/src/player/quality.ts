import type { PlaybackMode } from "@/api/types";

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
  { id: "20m", label: "20 Mbps 1080p", maxBitrate: 20_000, maxHeight: 1080 },
  { id: "12m", label: "12 Mbps 1080p", maxBitrate: 12_000, maxHeight: 1080 },
  { id: "8m", label: "8 Mbps 1080p", maxBitrate: 8_000, maxHeight: 1080 },
  { id: "4m", label: "4 Mbps 720p", maxBitrate: 4_000, maxHeight: 720 },
  { id: "2m", label: "2 Mbps 480p", maxBitrate: 2_000, maxHeight: 480 },
];

export function qualityById(id: string | null | undefined): QualityOption {
  return QUALITIES.find((q) => q.id === id) ?? ORIGINAL;
}

export interface ModeOption {
  id: PlaybackMode;
  label: string;
  hint: string;
}

const AUTO: ModeOption = { id: "auto", label: "Auto", hint: "Best method for this browser" };

export const PLAYBACK_MODES: ModeOption[] = [
  AUTO,
  { id: "direct", label: "Direct Play", hint: "The original file, unchanged" },
  { id: "remux", label: "Remux", hint: "Repackaged, original video" },
  { id: "transcode", label: "Transcode", hint: "Converted video, most compatible" },
];

export function modeById(id: string | null | undefined): ModeOption {
  return PLAYBACK_MODES.find((m) => m.id === id) ?? AUTO;
}
