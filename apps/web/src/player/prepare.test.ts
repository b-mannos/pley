import { afterEach, describe, expect, it, vi } from "vitest";
import { segmentAt, variantsOf, warmStream } from "./prepare";

const MASTER = [
  "#EXTM3U",
  '#EXT-X-STREAM-INF:BANDWIDTH=10752000,RESOLUTION=1920x1080,CODECS="avc1.640028,mp4a.40.2"',
  "index.m3u8",
  '#EXT-X-STREAM-INF:BANDWIDTH=5502000,RESOLUTION=1280x720,CODECS="avc1.64001f,mp4a.40.2"',
  "v1.m3u8",
  '#EXT-X-STREAM-INF:BANDWIDTH=1170750,RESOLUTION=640x360,CODECS="avc1.64001e,mp4a.40.2"',
  "v2.m3u8",
].join("\n");

const media = (prefix: string) =>
  ["#EXTM3U", "#EXT-X-TARGETDURATION:2", "#EXTINF:2.000,", `${prefix}0.ts`, "#EXTINF:2.000,", `${prefix}1.ts`, "#EXTINF:1.500,", `${prefix}2.ts`, "#EXT-X-ENDLIST"].join("\n");

const BASE = "http://pley.test/api/playback/s1/master.m3u8";

describe("preparing a session's stream", () => {
  afterEach(() => vi.unstubAllGlobals());

  it("reads a multivariant playlist's sizes in its order", () => {
    expect(variantsOf(MASTER, BASE)).toEqual([
      { uri: "http://pley.test/api/playback/s1/index.m3u8", width: 1920, height: 1080 },
      { uri: "http://pley.test/api/playback/s1/v1.m3u8", width: 1280, height: 720 },
      { uri: "http://pley.test/api/playback/s1/v2.m3u8", width: 640, height: 360 },
    ]);
    expect(variantsOf(media(""), BASE)).toEqual([]);
  });

  it("finds the segment that holds a time", () => {
    const list = media("v1-");
    expect(segmentAt(list, BASE, 0)).toBe("http://pley.test/api/playback/s1/v1-0.ts");
    expect(segmentAt(list, BASE, 3.9)).toBe("http://pley.test/api/playback/s1/v1-1.ts");
    expect(segmentAt(list, BASE, 4)).toBe("http://pley.test/api/playback/s1/v1-2.ts");
    expect(segmentAt(list, BASE, 99), "past the end: the last").toBe("http://pley.test/api/playback/s1/v1-2.ts");
  });

  it("asks for the playlist, the size the player picks and the segment it starts with", async () => {
    const asked: string[] = [];
    vi.stubGlobal(
      "fetch",
      vi.fn((url: string) => {
        asked.push(url);
        const body = url.endsWith("master.m3u8") ? MASTER : url.endsWith("v1.m3u8") ? media("v1-") : "segment";
        return Promise.resolve(new Response(body));
      }),
    );
    // Smallest first: 360p, 720p, 1080p; the server listed 1080p first (index 2).
    const pick = vi.fn((sizes: { height: number }[], listed: number) => {
      expect(sizes.map((s) => s.height)).toEqual([360, 720, 1080]);
      expect(listed).toBe(2);
      return 1;
    });
    await warmStream(BASE, 2.5, pick, new AbortController().signal);
    expect(asked).toEqual([BASE, "http://pley.test/api/playback/s1/v1.m3u8", "http://pley.test/api/playback/s1/v1-1.ts"]);
  });

  it("fails when the server refuses", async () => {
    vi.stubGlobal("fetch", vi.fn(() => Promise.resolve(new Response("gone", { status: 410 }))));
    await expect(warmStream(BASE, 0, () => 0, new AbortController().signal)).rejects.toThrow("410");
  });
});
