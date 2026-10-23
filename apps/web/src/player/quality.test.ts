import { describe, expect, it } from "vitest";
import { modeById, PLAYBACK_MODES, QUALITIES, qualityById } from "./quality";

describe("quality and playback mode choices", () => {
  it("finds them by id", () => {
    expect(qualityById("4m")).toMatchObject({ maxBitrate: 4_000, maxHeight: 720 });
    expect(modeById("remux").label).toBe("Remux");
  });

  it("falls back to the original quality and automatic mode", () => {
    expect(qualityById("nope")).toBe(QUALITIES[0]);
    expect(qualityById(null).id).toBe("original");
    expect(modeById(undefined)).toBe(PLAYBACK_MODES[0]);
    expect(modeById("bogus").id).toBe("auto");
  });

  it("lists qualities from best to worst", () => {
    const bitrates = QUALITIES.slice(1).map((q) => q.maxBitrate ?? 0);
    expect(bitrates).toEqual([...bitrates].sort((a, b) => b - a));
  });
});
