import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { apiError, mockApi } from "@/test/render";
import { prefetchPlay, resetPrefetchPlay } from "./prefetchPlay";

/** Lets the fire-and-forget request go out. */
const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

describe("prefetchPlay", () => {
  beforeEach(() => resetPrefetchPlay());
  afterEach(() => vi.unstubAllGlobals());

  it("asks the server to read videos ahead, once every two minutes per item", async () => {
    const requests = mockApi({ "POST /api/items/7/prefetch": () => new Response(null, { status: 202 }) });
    const t0 = 1_000_000;
    prefetchPlay({ id: 7, kind: "movie" }, t0);
    prefetchPlay({ id: 7, kind: "movie" }, t0 + 60_000);
    await settle();
    expect(requests.map((r) => `${r.method} ${r.path}`)).toEqual(["POST /api/items/7/prefetch"]);
    prefetchPlay({ id: 7, kind: "movie" }, t0 + 120_000);
    await settle();
    expect(requests).toHaveLength(2);
  });

  it("covers what Play starts a video for, and nothing else", async () => {
    const requests = mockApi({});
    const kinds = ["episode", "extra", "show", "season", "album", "artist", "track"] as const;
    kinds.forEach((kind, i) => prefetchPlay({ id: i + 1, kind }));
    await settle();
    expect(requests.map((r) => r.path)).toEqual([
      "/api/items/1/prefetch",
      "/api/items/2/prefetch",
      "/api/items/3/prefetch",
      "/api/items/4/prefetch",
    ]);
  });

  it("is only a hint: errors are ignored", async () => {
    const requests = mockApi({ "POST /api/items/3/prefetch": () => apiError(401, "unauthorized", "Sign in") });
    expect(() => prefetchPlay({ id: 3, kind: "episode" })).not.toThrow();
    await settle();
    expect(requests).toHaveLength(1);
  });
});
