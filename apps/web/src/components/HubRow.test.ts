import { describe, expect, it } from "vitest";
import type { Hub } from "@/api/types";
import { makeItem } from "@/test/fixtures";
import { fitCards, hubAspect } from "./HubRow";

describe("fitCards", () => {
  it("fits whole cards into the row", () => {
    expect(fitCards(1000, 160, 16)).toEqual({ width: 187, perPage: 5 });
    expect(fitCards(100, 160, 16)).toEqual({ width: 100, perPage: 1 });
  });

  it("grows cards on big screens, up to 35%", () => {
    expect(fitCards(3000, 160, 16)).toEqual({ width: 216, perPage: 13 });
  });

  it("never cuts a card off and never shrinks one below the minimum", () => {
    for (let available = 160; available < 4000; available += 37) {
      const { width, perPage } = fitCards(available, 160, 16);
      expect(perPage * width + (perPage - 1) * 16).toBeLessThanOrEqual(available);
      expect(width).toBeGreaterThanOrEqual(160);
    }
  });
});

describe("hubAspect", () => {
  const hub = (key: string, items = [makeItem()]): Hub => ({ key, title: key, library_id: 1, items });

  it("shows Continue Watching as thumbnails", () => {
    expect(hubAspect(hub("continue_watching"))).toBe("thumb");
  });

  it("follows the first item", () => {
    expect(hubAspect(hub("recently_added_1"))).toBe("poster");
    expect(hubAspect(hub("recently_added_2", [makeItem({ kind: "episode" })]))).toBe("thumb");
    expect(hubAspect(hub("recently_added_3", [makeItem({ kind: "album" })]))).toBe("square");
    expect(hubAspect(hub("empty", []))).toBe("poster");
  });
});
