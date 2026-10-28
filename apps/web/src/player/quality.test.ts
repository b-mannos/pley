import { describe, expect, it } from "vitest";
import { AUTO, autoLadder, chooseQuality, modeFor, ORIGINAL, QUALITIES, qualitiesFor, qualityById, serverQuality } from "./quality";

type FileSize = { bitrate: number | null; width: number | null; height: number | null };
const file = (width: number | null, height: number | null, mbps: number | null): FileSize => ({
  width,
  height,
  bitrate: mbps === null ? null : mbps * 1_000_000,
});
const ids = (list: { id: string }[]) => list.map((q) => q.id);

describe("quality choices", () => {
  it("finds them by id, falling back to the original", () => {
    expect(qualityById("4m")).toMatchObject({ maxBitrate: 4_000, maxHeight: 720 });
    expect(qualityById("nope")).toBe(QUALITIES[0]);
    expect(qualityById(null).id).toBe("original");
  });

  it("lists Original, Automatic, the 4K qualities, then 1080p and below (docs/QUALITY.md)", () => {
    expect(QUALITIES.map((q) => [q.id, q.label])).toEqual([
      ["original", "Original"],
      ["auto", "Automatic"],
      ["30m", "30 Mbps 4K"],
      ["15m", "15 Mbps 4K"],
      ["20m", "20 Mbps 1080p"],
      ["12m", "12 Mbps 1080p"],
      ["8m", "8 Mbps 1080p"],
      ["4m", "4 Mbps 720p"],
      ["2m", "2 Mbps 480p"],
    ]);
  });

  it("lets the server choose for the original and Automatic, and transcodes the rest", () => {
    expect(modeFor(ORIGINAL)).toBe("auto");
    expect(modeFor(AUTO)).toBe("auto");
    expect(modeFor(qualityById("8m"))).toBe("transcode");
  });

  it("offers the original, Automatic, and only the qualities that make a file smaller", () => {
    const offered = (f: FileSize | null) => ids(qualitiesFor(f));
    expect(offered(file(1920, 1080, 9))).toEqual(["original", "auto", "8m", "4m", "2m"]);
    expect(offered(file(1280, 720, 3))).toEqual(["original", "auto", "2m"]);
    // A 4K file at a low bitrate still gets smaller at 1080p.
    expect(offered(file(3840, 2160, 6))).toEqual(["original", "auto", "20m", "12m", "8m", "4m", "2m"]);
    // A 4K remux: 4K at 30 Mbps too (never offered to files it wouldn't make smaller).
    expect(offered(file(3840, 2160, 60))).toEqual(["original", "auto", "30m", "15m", "20m", "12m", "8m", "4m", "2m"]);
    // 4K at 20 Mbps: 4K at 15 is smaller, 4K at 30 isn't.
    expect(offered(file(3840, 2160, 20))).toEqual(["original", "auto", "15m", "20m", "12m", "8m", "4m", "2m"]);
    expect(offered(file(null, null, null))).toHaveLength(QUALITIES.length);
    expect(offered(null)).toHaveLength(QUALITIES.length);
  });

  it("plays Automatic until a quality is picked, and keeps a pick, Original included", () => {
    const offered = qualitiesFor(file(1920, 1080, 9));
    expect(chooseQuality(null, offered)).toBe(AUTO);
    expect(chooseQuality("nonsense", offered)).toBe(AUTO);
    expect(chooseQuality("original", offered)).toBe(ORIGINAL);
    expect(chooseQuality("8m", offered).id).toBe("8m");
    // A quality this file doesn't need plays the original.
    expect(chooseQuality("20m", offered)).toBe(ORIGINAL);
  });
});

describe("the server's own quality for a file", () => {
  it("matches the server's closest_quality", () => {
    const pick = (w: number, h: number, mbps: number) => serverQuality(file(w, h, mbps)).id;
    expect(pick(3840, 2160, 57)).toBe("30m");
    expect(pick(3840, 2160, 18)).toBe("15m");
    expect(pick(3840, 2160, 6)).toBe("15m");
    expect(pick(1920, 1080, 25)).toBe("20m");
    expect(pick(1920, 1080, 10)).toBe("12m");
    expect(pick(1920, 800, 9)).toBe("12m");
    expect(pick(1280, 720, 3)).toBe("4m");
    expect(pick(720, 480, 2)).toBe("2m");
    expect(serverQuality(file(null, null, null)).id).toBe("20m");
  });
});

describe("Automatic's steps", () => {
  it("go down in bitrate, never above the server's quality nor the file's lines", () => {
    expect(ids(autoLadder(file(3840, 2160, 60)))).toEqual(["30m", "15m", "12m", "8m", "4m", "2m"]);
    expect(ids(autoLadder(file(3840, 2160, 18)))).toEqual(["15m", "12m", "8m", "4m", "2m"]);
    expect(ids(autoLadder(file(1920, 1080, 40)))).toEqual(["20m", "12m", "8m", "4m", "2m"]);
    expect(ids(autoLadder(file(1920, 1080, 5)))).toEqual(["4m", "2m"]);
    expect(ids(autoLadder(file(720, 480, 1.5)))).toEqual([]);
  });
});
