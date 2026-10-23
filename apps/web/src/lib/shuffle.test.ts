import { describe, expect, it } from "vitest";
import { shuffled } from "./shuffle";

describe("shuffled", () => {
  it("returns a permutation and leaves the list alone", () => {
    const list = Array.from({ length: 50 }, (_, i) => i);
    const out = shuffled(list);
    expect(out).not.toBe(list);
    expect(list).toEqual(Array.from({ length: 50 }, (_, i) => i));
    expect([...out].sort((a, b) => a - b)).toEqual(list);
  });

  it("swaps with the index the random number picks", () => {
    // Always the first candidate: every element moves one place left.
    expect(shuffled([1, 2, 3, 4], () => 0)).toEqual([2, 3, 4, 1]);
    // Always the last candidate: nothing moves.
    expect(shuffled([1, 2, 3, 4], () => 0.999)).toEqual([1, 2, 3, 4]);
  });

  it("handles empty and single lists", () => {
    expect(shuffled([])).toEqual([]);
    expect(shuffled(["a"])).toEqual(["a"]);
  });
});
