import { describe, expect, it } from "vitest";
import { categoryKey, parseCategory, type LiveGroup } from "@/api/livetv";
import { categoryName, groupTree } from "./CategoryRail";
import { channelName, liveUrl, splitRegion } from "./common";

const group = (name: string, region: string | null, label: string, count = 1): LiveGroup => ({ name, region, label, count });

describe("groupTree", () => {
  it("gathers a region's groups where its first one is, and leaves a region of one a group", () => {
    const tree = groupTree([
      group("UK| News", "UK", "News", 3),
      group("24/7 Films", null, "24/7 Films"),
      group("UK| Sport", "UK", "Sport", 4),
      group("FR| Cinéma", "FR", "Cinéma"),
    ]);
    expect(tree.map((e) => (e.kind === "region" ? `region ${e.region} ${e.count}` : e.group.name))).toEqual([
      "region UK 7",
      "24/7 Films",
      "FR| Cinéma",
    ]);
  });
});

describe("categories", () => {
  it("go to and from their keys", () => {
    for (const c of [{ kind: "all" }, { kind: "favorites" }, { kind: "recent" }, { kind: "group", group: "UK| Sport" }] as const) {
      expect(parseCategory(categoryKey(c))).toEqual(c);
    }
    expect(parseCategory(null)).toEqual({ kind: "all" });
  });

  it("are named by their region and label", () => {
    const groups = [group("UK| Sport", "UK", "Sport")];
    expect(categoryName({ kind: "group", group: "UK| Sport" }, groups)).toBe("UK · Sport");
    expect(categoryName({ kind: "group", group: "Gone" }, groups)).toBe("Gone");
    expect(categoryName({ kind: "favorites" }, groups)).toBe("Favourites");
  });

  it("leave channel names without the region their group says", () => {
    const sky = { name: "UK| SKY SPORTS F1 HD", group: "UK| SPORTS" };
    expect(channelName(sky, { kind: "group", group: "UK| SPORTS" })).toBe("SKY SPORTS F1 HD");
    expect(channelName(sky, { kind: "all" })).toBe("UK| SKY SPORTS F1 HD");
    expect(channelName({ name: "UK|SKY SPORTS ACTION", group: "UK| SPORTS" }, { kind: "group", group: "x" })).toBe("SKY SPORTS ACTION");
    expect(channelName({ name: "FR| TF1", group: "UK| SPORTS" }, { kind: "group", group: "x" })).toBe("FR| TF1");
    expect(splitRegion("Live: Sport")).toBeNull();
    expect(splitRegion("[DE] Sport")).toEqual(["DE", "Sport"]);
  });

  it("travel with a channel to the player", () => {
    expect(liveUrl(7)).toBe("/live/7");
    expect(liveUrl(7, { kind: "all" })).toBe("/live/7");
    expect(liveUrl(7, { kind: "group", group: "UK| Sport" })).toBe("/live/7?in=group%3AUK%7C%20Sport");
  });
});
