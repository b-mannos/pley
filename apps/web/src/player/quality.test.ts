import { describe, expect, it } from "vitest";
import { modeFor, QUALITIES, qualitiesFor, qualityById } from "./quality";

describe("quality choices", () => {
  it("finds them by id, falling back to the original", () => {
    expect(qualityById("4m")).toMatchObject({ maxBitrate: 4_000, maxHeight: 720 });
    expect(qualityById("nope")).toBe(QUALITIES[0]);
    expect(qualityById(null).id).toBe("original");
  });

  it("lists qualities from best to worst", () => {
    const bitrates = QUALITIES.slice(1).map((q) => q.maxBitrate ?? 0);
    expect(bitrates).toEqual([...bitrates].sort((a, b) => b - a));
  });

  it("transcodes every quality but the original", () => {
    expect(modeFor(qualityById("original"))).toBe("auto");
    expect(modeFor(qualityById("8m"))).toBe("transcode");
  });

  it("offers only the qualities that make a file smaller", () => {
    const ids = (file: { bitrate: number | null; height: number | null } | null) => qualitiesFor(file).map((q) => q.id);
    expect(ids({ bitrate: 9_000_000, height: 1080 })).toEqual(["original", "8m", "4m", "2m"]);
    expect(ids({ bitrate: 3_000_000, height: 720 })).toEqual(["original", "2m"]);
    // A 4K file at a low bitrate still gets smaller at 1080p.
    expect(ids({ bitrate: 6_000_000, height: 2160 })).toEqual(["original", "20m", "12m", "8m", "4m", "2m"]);
    // A 4K remux: 4K at 30 Mbps too (never offered to files it wouldn't make smaller).
    expect(ids({ bitrate: 60_000_000, height: 2160 })).toEqual(["original", "30m", "20m", "15m", "12m", "8m", "4m", "2m"]);
    // 4K at 20 Mbps: 4K at 15 is smaller, 4K at 30 isn't.
    expect(ids({ bitrate: 20_000_000, height: 2160 })).toEqual(["original", "20m", "15m", "12m", "8m", "4m", "2m"]);
    expect(ids({ bitrate: null, height: null })).toHaveLength(QUALITIES.length);
    expect(ids(null)).toHaveLength(QUALITIES.length);
  });
});
