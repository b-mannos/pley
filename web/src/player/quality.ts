export interface QualityOption {
  id: string;
  label: string;
  /** kbps */
  maxBitrate: number | null;
  maxHeight: number | null;
}

export const QUALITIES: QualityOption[] = [
  { id: "original", label: "Original", maxBitrate: null, maxHeight: null },
  { id: "20m", label: "20 Mbps 1080p", maxBitrate: 20_000, maxHeight: 1080 },
  { id: "12m", label: "12 Mbps 1080p", maxBitrate: 12_000, maxHeight: 1080 },
  { id: "8m", label: "8 Mbps 1080p", maxBitrate: 8_000, maxHeight: 1080 },
  { id: "4m", label: "4 Mbps 720p", maxBitrate: 4_000, maxHeight: 720 },
  { id: "2m", label: "2 Mbps 480p", maxBitrate: 2_000, maxHeight: 480 },
];

export function qualityById(id: string | null | undefined): QualityOption {
  return QUALITIES.find((q) => q.id === id) ?? QUALITIES[0];
}
