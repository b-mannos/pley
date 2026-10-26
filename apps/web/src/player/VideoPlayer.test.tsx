import { act, fireEvent, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useServerEvents } from "@/api/events";
import type { ItemDetail, PlaybackInfo, ServerEvent } from "@/api/types";
import { makeItem, makeStream } from "@/test/fixtures";
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

/** The media element methods the player calls, which jsdom only stubs. */
const media = {
  pause: vi.fn<() => void>(),
  load: vi.fn<() => void>(),
};

beforeEach(() => {
  hlsInstances.length = 0;
  // jsdom has media elements, but plays nothing.
  vi.spyOn(HTMLMediaElement.prototype, "play").mockResolvedValue(undefined);
  media.pause = vi.spyOn(HTMLMediaElement.prototype, "pause").mockImplementation(() => undefined);
  media.load = vi.spyOn(HTMLMediaElement.prototype, "load").mockImplementation(() => undefined);
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

describe("VideoPlayer leaving", () => {
  it("lets go of a direct play's file, so its download doesn't compete with what plays next", async () => {
    const requests = serve([session("d1", { method: "direct", hls: false, url: "/api/files/3/stream" })], {
      "DELETE /api/playback/d1": () => undefined,
    });
    const { container, unmount } = renderPlayer(0);
    const video = container.querySelector("video");
    if (!video) throw new Error("no video element");
    await waitFor(() => expect(video.getAttribute("src")).toBe("/api/files/3/stream"));
    media.pause.mockClear();
    media.load.mockClear();

    unmount();

    expect(video.hasAttribute("src")).toBe(false);
    expect(media.pause).toHaveBeenCalled();
    expect(media.load).toHaveBeenCalled();
    // And the session ends, as before.
    await waitFor(() => expect(requests.some((r) => r.method === "DELETE" && r.path === "/api/playback/d1")).toBe(true));
  });

  it("lets go of hls.js", async () => {
    serve([session("s1")], { "DELETE /api/playback/s1": () => undefined });
    const { unmount } = renderPlayer(0);
    await waitFor(() => expect(hlsInstances[0]?.calls).toContain("attachMedia"));
    unmount();
    expect(hlsInstances[0]?.calls.at(-1)).toBe("destroy");
  });
});

describe("VideoPlayer switching sessions", () => {
  const starts = (requests: FakeRequest[]) => requests.filter((r) => r.method === "POST" && r.path === "/api/playback");

  it("asks for the next session without waiting for the old one's release", async () => {
    // The old session's last report and release never get an answer.
    const requests = serve([session("s1"), session("s2")], {
      "POST /api/items/7/progress": () => new Promise(() => undefined),
      "DELETE /api/playback/s1": () => new Promise(() => undefined),
    });
    renderPlayer(0);
    await waitFor(() => expect(hlsInstances[0]?.calls).toContain("attachMedia"));

    // The session expired: the player starts a new one.
    act(() => {
      hlsInstances[0]?.emit("hlsError", { fatal: true, type: "networkError", details: "fragLoadError", response: { code: 404 } });
    });

    await waitFor(() => expect(hlsInstances).toHaveLength(2));
    expect(starts(requests)).toHaveLength(2);
    expect(requests.some((r) => r.method === "DELETE" && r.path === "/api/playback/s1")).toBe(true);
    expect(requests.some((r) => r.path === "/api/items/7/progress" && (r.body as { state: string }).state === "stopped")).toBe(true);
  });

  it("says the server can't be reached when its stream stops answering", async () => {
    serve([session("s1")]);
    renderPlayer(0);
    await waitFor(() => expect(hlsInstances[0]?.calls).toContain("attachMedia"));

    // No answer at all, past the retries.
    act(() => {
      for (let i = 0; i < 5; i++) hlsInstances[0]?.emit("hlsError", { fatal: true, type: "networkError", details: "fragLoadError" });
    });

    expect(await screen.findByText("Can't reach your server")).toBeInTheDocument();
    expect(screen.getByText("Check that the server is on and that this device is on its network.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Retry" })).toBeInTheDocument();
  });

  it("still waits for a start in flight, which ends its own session", async () => {
    let answer: (info: PlaybackInfo) => void = () => undefined;
    const first = new Promise<PlaybackInfo>((resolve) => (answer = resolve));
    let posts = 0;
    const requests = mockApi({
      "POST /api/playback": () => (posts++ === 0 ? first : session("s2")),
      "DELETE /api/playback/s1": () => undefined,
    });
    renderPlayer(0);
    await waitFor(() => expect(starts(requests)).toHaveLength(1));

    // Back from the back/forward cache before the first start answered.
    act(() => {
      window.dispatchEvent(new PageTransitionEvent("pageshow", { persisted: true }));
    });
    await new Promise((resolve) => setTimeout(resolve, 20));
    expect(starts(requests)).toHaveLength(1);

    answer(session("s1"));
    await waitFor(() => expect(starts(requests)).toHaveLength(2));
    const paths = requests.map((r) => `${r.method} ${r.path}`);
    expect(paths.indexOf("DELETE /api/playback/s1")).toBeGreaterThanOrEqual(0);
    expect(paths.indexOf("DELETE /api/playback/s1")).toBeLessThan(paths.lastIndexOf("POST /api/playback"));
    await waitFor(() => expect(hlsInstances.map((h) => h.calls[0])).toEqual(["loadSource /api/playback/s2/index.m3u8"]));
  });
});

/** Stands in for the browser's EventSource: `send` delivers a server event. */
class FakeEventSource {
  static readonly CLOSED = 2;
  static last: FakeEventSource | null = null;
  readyState = 1;
  onopen: (() => void) | null = null;
  onmessage: ((msg: { data: string }) => void) | null = null;
  onerror: (() => void) | null = null;
  constructor() {
    FakeEventSource.last = this;
  }
  close() {
    this.readyState = FakeEventSource.CLOSED;
  }
  send(event: ServerEvent) {
    this.onmessage?.({ data: JSON.stringify(event) });
  }
}

/** The player with the app's event stream open, as signed in. */
function PlayerWithEvents() {
  useServerEvents(true);
  return <VideoPlayer item={detail()} startMs={0} fileId={null} audio={null} sub={null} />;
}

describe("VideoPlayer stopped on the server", () => {
  beforeEach(() => {
    vi.stubGlobal("EventSource", FakeEventSource);
  });
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  const starts = (requests: FakeRequest[]) => requests.filter((r) => r.method === "POST" && r.path === "/api/playback");

  it("stops, and says who stopped it and why", async () => {
    const requests = serve([session("s1")], { "DELETE /api/playback/s1": () => undefined });
    const { unmount } = renderWithProviders(<PlayerWithEvents />);
    await waitFor(() => expect(hlsInstances[0]?.calls).toContain("attachMedia"));

    // Someone else's session: nothing happens here.
    act(() => FakeEventSource.last?.send({ type: "session_stopped", session_id: "other", message: null, by_admin: true }));
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();

    act(() => FakeEventSource.last?.send({ type: "session_stopped", session_id: "s1", message: "Server restarting", by_admin: true }));
    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent("Playback was stopped by the server's administrator");
    expect(alert).toHaveTextContent("Server restarting");
    expect(hlsInstances[0]?.calls.at(-1)).toBe("destroy");

    unmount();
    // Nothing starts again, and there is nothing left to release.
    expect(starts(requests)).toHaveLength(1);
    expect(requests.some((r) => r.method === "DELETE" && r.path === "/api/playback/s1")).toBe(false);
  });

  it("stops when the stream is gone (410), with the server's message", async () => {
    const requests = serve([session("s1")], {
      "GET /api/playback/s1/index.m3u8": () =>
        new Response(JSON.stringify({ error: "session_stopped", message: "Too many streams" }), {
          status: 410,
          headers: { "content-type": "application/json" },
        }),
    });
    renderWithProviders(<PlayerWithEvents />);
    await waitFor(() => expect(hlsInstances[0]?.calls).toContain("attachMedia"));

    act(() => {
      hlsInstances[0]?.emit("hlsError", { fatal: true, type: "networkError", details: "fragLoadError", response: { code: 410 } });
    });

    const alert = await screen.findByRole("alert");
    await waitFor(() => expect(alert).toHaveTextContent("Too many streams"));
    expect(alert).toHaveTextContent("Playback was stopped by the server's administrator");
    expect(starts(requests)).toHaveLength(1);
  });

  it("says so when it was stopped from another of the user's devices", async () => {
    serve([session("s1")]);
    renderWithProviders(<PlayerWithEvents />);
    await waitFor(() => expect(hlsInstances[0]?.calls).toContain("attachMedia"));
    act(() => FakeEventSource.last?.send({ type: "session_stopped", session_id: "s1", message: null, by_admin: false }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Playback was stopped from another device");
  });
});

/** Stands in for the browser's VTTCue. */
class FakeCue {
  line: number | "auto" = "auto";
  snapToLines = true;
  startTime: number;
  endTime: number;
  text: string;
  constructor(startTime: number, endTime: number, text: string) {
    this.startTime = startTime;
    this.endTime = endTime;
    this.text = text;
  }
  getCueAsHTML() {
    const fragment = document.createDocumentFragment();
    fragment.append(this.text);
    return fragment;
  }
}

describe("VideoPlayer subtitle timing", () => {
  beforeEach(() => {
    vi.stubGlobal("VTTCue", FakeCue);
  });
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  const english = makeStream({ index: 3, kind: "subtitle", codec: "subrip", language: "eng", default: false, text_based: true });

  it("shows the lines moved by the offset remembered for the file, until it is reset", async () => {
    const user = userEvent.setup();
    window.localStorage.setItem("pley.subtitleOffsets", JSON.stringify({ f3: 1 }));
    serve(
      [
        session("s1", {
          subtitles: [
            { index: 3, codec: "subrip", language: "eng", title: null, default: false, forced: false, external: false, text_based: true, url: "/api/subs/3.vtt" },
          ],
        }),
      ],
      { "GET /api/subs/3.vtt": () => new Response("WEBVTT\n\n00:00:01.000 --> 00:00:02.000\nHello", { status: 200 }) },
    );
    const item = detail();
    const file = item.files[0];
    if (!file) throw new Error("no file");
    file.streams = [english];
    const { container } = renderWithProviders(<VideoPlayer item={item} startMs={0} fileId={null} audio={null} sub={3} />);
    const video = container.querySelector("video");
    if (!video) throw new Error("no video element");
    Object.defineProperty(video, "currentTime", { value: 2.5, configurable: true, writable: true });

    // At 2.5 s, one second later than the file has it.
    await waitFor(() => {
      fireEvent.timeUpdate(video);
      expect(screen.getByTestId("subtitles")).toHaveTextContent("Hello");
    });

    await user.click(screen.getByRole("button", { name: "Playback settings" }));
    await user.click(screen.getByRole("button", { name: /Subtitles.*\+1\.0 s/ }));
    await user.click(screen.getByRole("button", { name: "Reset" }));
    fireEvent.timeUpdate(video);
    expect(screen.queryByTestId("subtitles")).not.toBeInTheDocument();
    expect(window.localStorage.getItem("pley.subtitleOffsets")).toBe("{}");
  });

  it("moves the track's own cues in native fullscreen, where the browser draws them", async () => {
    window.localStorage.setItem("pley.subtitleOffsets", JSON.stringify({ f3: 1 }));
    const onTrack: FakeCue[] = [];
    const track = {
      get cues() {
        return onTrack;
      },
      mode: "disabled",
      addCue: (cue: FakeCue) => onTrack.push(cue),
      removeCue: (cue: FakeCue) => onTrack.splice(onTrack.indexOf(cue), 1),
      addEventListener: () => undefined,
      removeEventListener: () => undefined,
    };
    vi.spyOn(HTMLMediaElement.prototype, "addTextTrack").mockImplementation(() => track as unknown as TextTrack);
    serve(
      [
        session("s1", {
          subtitles: [
            { index: 3, codec: "subrip", language: "eng", title: null, default: false, forced: false, external: false, text_based: true, url: "/api/subs/3.vtt" },
          ],
        }),
      ],
      { "GET /api/subs/3.vtt": () => new Response("WEBVTT\n\n00:00:01.000 --> 00:00:02.000\nHello", { status: 200 }) },
    );
    const item = detail();
    const file = item.files[0];
    if (!file) throw new Error("no file");
    file.streams = [english];
    const { container } = renderWithProviders(<VideoPlayer item={item} startMs={0} fileId={null} audio={null} sub={3} />);
    const video = container.querySelector("video");
    if (!video) throw new Error("no video element");

    // Drawn by pley: the cues stay at the file's times, hidden.
    await waitFor(() => expect(onTrack.map((c) => [c.startTime, c.endTime])).toEqual([[1, 2]]));
    expect(track.mode).toBe("hidden");

    // Drawn by the browser: moved one second later on the track.
    fireEvent(video, new Event("webkitbeginfullscreen"));
    await waitFor(() => expect(onTrack.map((c) => [c.startTime, c.endTime])).toEqual([[2, 3]]));
    expect(track.mode).toBe("showing");
    expect(onTrack[0]?.text).toBe("Hello");
  });
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
