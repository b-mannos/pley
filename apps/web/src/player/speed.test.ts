import { describe, expect, it } from "vitest";
import { SPEEDS, speedLabel, stepSpeed } from "./speed";

describe("playback speed", () => {
  it("offers half to twice the speed, named as players name them", () => {
    expect(SPEEDS.map(speedLabel)).toEqual(["0.5×", "0.75×", "Normal", "1.25×", "1.5×", "2×"]);
  });

  it("steps one speed at a time, and no further than either end", () => {
    expect(stepSpeed(1, 1)).toBe(1.25);
    expect(stepSpeed(1, -1)).toBe(0.75);
    expect(stepSpeed(2, 1)).toBe(2);
    expect(stepSpeed(0.5, -1)).toBe(0.5);
    // A speed not offered steps from normal.
    expect(stepSpeed(3, -1)).toBe(0.75);
  });
});
