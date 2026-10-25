import { waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ItemDetail, PlaybackInfo } from "@/api/types";
import { makeItem } from "@/test/fixtures";
import { mockApi, renderWithProviders, type FakeRequest } from "@/test/render";
import { VideoPlayer } from "./VideoPlayer";

/** A stand-in for hls.js: records its config and calls, and fires events on demand. */
const { FakeHls, hlsInstances } = vi.hoisted(() => {
  type Handler = (event: string, data: unknown) => void;
  const hlsInstances: FakeHls[] = [];
  class FakeHls {
    static isSupported = () => true;
    static Events = { ERROR: "hlsError", FRAG_LOADED: "hlsFragLoaded" };
    static ErrorTypes = { NETWORK_ERROR: "networkError", MEDIA_ERROR: "mediaError" };
    static ErrorDetails = {
      MANIFEST_LOAD_ERROR: "manifestLoadError",
      MANIFEST_LOAD_TIMEOUT: "manifestLoadTimeOut",
      MANIFEST_PARSING_ERROR: "manifestParsingError",
    };
    readonly calls: string[] = [];
    private readonly handlers = new Map<string, Handler[]>();
    readonly config: Record<string, unknown>;
    constructor(config: Record<string, unknown>) {
      this.config = config;
      hlsInstances.push(this);
    }
    on(event: string, handler: Handler) {
      this.handlers.set(event, [...(this.handlers.get(event) ?? []), handler]);
    }
    emit(event: string, data: unknown) {
      for (const handler of this.handlers.get(event) ?? []) handler(event, data);
    }
    loadSource(url: string) {
      this.calls.push(`loadSource ${url}`);
    }
    attachMedia() {
      this.calls.push("attachMedia");
    }
    startLoad() {
      this.calls.push("startLoad");
    }
    recoverMediaError() {
      this.calls.push("recoverMediaError");
    }
    destroy() {
      this.calls.push("destroy");
    }
  }
  return { FakeHls, hlsInstances };
});

vi.mock("@/lib/hls", () => ({ loadHls: () => Promise.resolve(FakeHls) }));

function detail(overrides: Partial<ItemDetail> = {}): ItemDetail {
  return {
    ...makeItem({ id: 7, kind: "movie", title: "Heat", duration_ms: 6_000_000 }),
    original_title: null,
    tagline: null,
    genres: [],
    studio: null,
    content_rating: null,
    tmdb_id: null,
    imdb_id: null,
    files: [
      {
        id: 3,
        path: "/movies/Heat.mkv",
        size: 1,
        container: "mkv",
        duration_ms: 6_000_000,
        bitrate: null,
        width: 1920,
        height: 1080,
        video_codec: "h264",
        audio_codec: "aac",
        streams: [],
        edition: null,
        selected_audio: null,
        selected_subtitle: null,
        chapters: [],
      },
    ],
    ...overrides,
  };
}

function session(id: string, overrides: Partial<PlaybackInfo> = {}): PlaybackInfo {
  return {
    session_id: id,
    method: "transcode",
    url: `/api/playback/${id}/index.m3u8`,
    hls: true,
    file_id: 3,
    duration_ms: 6_000_000,
    audio_index: null,
    subtitle_index: null,
    burn_subtitles: false,
    subtitles: [],
    reason: null,
    ...overrides,
  };
}

/** The server: each POST /api/playback answers the next session in `sessions`. */
function serve(sessions: PlaybackInfo[], more: Parameters<typeof mockApi>[0] = {}): FakeRequest[] {
  let next = 0;
  return mockApi({
    "POST /api/playback": () => sessions[next++] ?? session(`extra-${next}`),
    "POST /api/items/7/progress": () => undefined,
    ...more,
  });
}

function renderPlayer(startMs = 0) {
  return renderWithProviders(<VideoPlayer item={detail()} startMs={startMs} fileId={null} audio={null} sub={null} />);
}

beforeEach(() => {
  hlsInstances.length = 0;
  // jsdom has media elements, but plays nothing.
  vi.spyOn(HTMLMediaElement.prototype, "play").mockResolvedValue(undefined);
  vi.spyOn(HTMLMediaElement.prototype, "pause").mockImplementation(() => undefined);
  vi.spyOn(HTMLMediaElement.prototype, "load").mockImplementation(() => undefined);
  vi.spyOn(HTMLMediaElement.prototype, "addTextTrack").mockImplementation(
    () =>
      ({
        cues: null,
        mode: "disabled",
        addCue: () => undefined,
        removeCue: () => undefined,
        addEventListener: () => undefined,
        removeEventListener: () => undefined,
      }) as unknown as TextTrack,
  );
});

describe("VideoPlayer with hls.js", () => {
  it("starts just past 0 from the beginning, so hls.js seeks to the first frame", async () => {
    serve([session("s1")]);
    renderPlayer(0);
    await waitFor(() => expect(hlsInstances).toHaveLength(1));
    expect(hlsInstances[0]?.config.startPosition).toBe(0.001);
  });

  it("starts at the resume point when there is one", async () => {
    serve([session("s1")]);
    renderPlayer(90_000);
    await waitFor(() => expect(hlsInstances).toHaveLength(1));
    expect(hlsInstances[0]?.config.startPosition).toBe(90);
  });

  it("asks for the first segment without waiting for the video element", async () => {
    serve([session("s1")]);
    renderPlayer(0);
    await waitFor(() => expect(hlsInstances[0]?.calls).toContain("attachMedia"));
    expect(hlsInstances[0]?.config.startFragPrefetch).toBe(true);
  });
});
