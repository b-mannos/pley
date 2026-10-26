import { afterEach, describe, expect, it, vi } from "vitest";
import type { Api } from "../../src/api/client";
import type { ClientCaps, ItemDetail, MediaFile, MediaStream, PlaybackInfo, PlaybackRequest, ServerEvent } from "../../src/api/types";
import type { KeyValue } from "../../src/auth/tokens";
import { PlaybackController, type PlayerEnv } from "../../src/player/controller";
import type { EngineListener, EngineSource, EngineStatus, PlayerEngine } from "../../src/player/engine";
import { dvContainerMode, dvRetryMode, initialAudio, playbackRequest, type Choices } from "../../src/player/plan";
import { ORIGINAL } from "../../src/player/quality";
import { Reporter } from "../../src/player/reporter";
import { NetworkError } from "../../src/net/transport";
import { OffsetStore } from "../../src/subs/sync";

// ---- fixtures

function stream(over: Partial<MediaStream>): MediaStream {
  return { index: 0, kind: "video", codec: "hevc", language: null, title: null, default: true, forced: false, ...over };
}

function file(over: Partial<MediaFile> = {}): MediaFile {
  return {
    id: 7,
    path: "/media/Film (2024).mkv",
    size: 40_000_000_000,
    container: "mkv",
    duration_ms: 7_200_000,
    bitrate: 40_000_000,
    width: 3840,
    height: 2160,
    video_codec: "hevc",
    audio_codec: "eac3",
    streams: [
      stream({ index: 0, kind: "video", codec: "hevc", width: 3840, height: 2160, bit_depth: 10, profile: "Main 10", dynamic_range: "hdr10", level: 153 }),
      stream({ index: 1, kind: "audio", codec: "eac3", language: "eng", channels: 6 }),
      stream({ index: 2, kind: "audio", codec: "truehd", language: "eng", channels: 8, default: false }),
      stream({ index: 3, kind: "subtitle", codec: "hdmv_pgs_subtitle", language: "eng", default: false, text_based: false }),
      stream({ index: 4, kind: "subtitle", codec: "subrip", language: "eng", default: false, text_based: true }),
    ],
    edition: null,
    selected_audio: null,
    selected_subtitle: null,
    chapters: [],
    ...over,
  };
}

function dvFile(profile: number, container = "mkv"): MediaFile {
  const f = file({ container });
  f.streams[0] = stream({ index: 0, kind: "video", codec: "hevc", width: 3840, height: 2160, bit_depth: 10, profile: "Main 10", dynamic_range: "dolby_vision", dv_profile: profile, dv_bl_compat: profile === 8 ? 1 : 0, level: 153 });
  return f;
}

function item(f: MediaFile, over: Partial<ItemDetail> = {}): ItemDetail {
  return {
    id: 70,
    library_id: 1,
    kind: "movie",
    title: "Film",
    sort_title: "film",
    year: 2024,
    added_at: 0,
    user_state: { position_ms: 600_000, watched: false, play_count: 0 },
    duration_ms: 7_200_000,
    original_title: null,
    tagline: null,
    genres: [],
    studio: null,
    content_rating: null,
    files: [f],
    ...over,
  };
}

/** An LG OLED of 2023 (webOS 23): HDR10, HLG, Dolby Vision 5 and 8, Dolby audio to the receiver. */
const LG: ClientCaps = {
  containers: ["mkv", "mp4", "ts"],
  video_codecs: ["h264", "hevc", "av1"],
  audio_codecs: ["aac", "ac3", "eac3"],
  hdr: true,
  dolby_vision: true,
  caps_version: 2,
  video: [{ codec: "hevc", hardware: true, profiles: ["Main", "Main 10"], max_width: 4096, max_height: 2160, max_fps: 60, max_bit_depth: 10, max_level: 153 }],
  hdr_types: ["hdr10", "hlg", "dolby_vision"],
  dolby_vision_profiles: [{ profile: 5 }, { profile: 8 }],
  audio_decode: ["aac", "ac3", "eac3"],
  audio_passthrough: ["ac3", "eac3"],
  audio_channels: 8,
  subtitle_codecs: ["subrip", "hdmv_pgs_subtitle"],
  audio_track_switching: true,
  picture_subtitles_sideload: true,
  hls: { master_playlist: true, segments: ["ts", "fmp4"] },
  device: { form: "tv", tier: "high", fire_tv: false, model: "OLED65C3PUA" },
};

function choices(over: Partial<Choices> = {}): Choices {
  const f = file();
  return { file: f, startMs: 600_000, audio: f.streams[1]!, subtitle: null, quality: ORIGINAL, mode: "auto", streaming: false, sideload: true, ...over };
}

function info(over: Partial<PlaybackInfo> = {}): PlaybackInfo {
  return {
    session_id: "s1",
    method: "direct",
    url: "/api/media/pley_mk_KEY/file.mkv",
    hls: false,
    file_id: 7,
    duration_ms: 7_200_000,
    audio_index: 1,
    subtitle_index: null,
    burn_subtitles: false,
    subtitles: [],
    reason: null,
    ...over,
  };
}

// ---- the request

describe("the audio to start with", () => {
  /** A remux's tracks: TrueHD Atmos first and default, its E-AC-3, a commentary. */
  function remux(): MediaFile {
    const f = file();
    f.streams = [
      f.streams[0]!,
      stream({ index: 1, kind: "audio", codec: "truehd", language: "eng", channels: 8, default: true }),
      stream({ index: 2, kind: "audio", codec: "ac3", language: "eng", channels: 2, default: false, title: "Director's Commentary" }),
      stream({ index: 3, kind: "audio", codec: "eac3", language: "eng", channels: 6, default: false }),
      stream({ index: 4, kind: "audio", codec: "eac3", language: "fra", channels: 6, default: false }),
    ];
    return f;
  }

  it("is the E-AC-3 in the same language when the TrueHD first in the file doesn't play here, never the commentary", () => {
    expect(initialAudio(remux(), null, LG)?.index).toBe(3);
  });

  it("stays the server's when it plays here, or when nothing better does", () => {
    expect(initialAudio(remux(), null, { ...LG, audio_passthrough: ["ac3", "eac3", "truehd"] })?.index).toBe(1);
    const lone = remux();
    lone.streams = lone.streams.filter((s) => s.index !== 3);
    expect(initialAudio(lone, null, LG)?.index).toBe(1);
    expect(initialAudio(remux(), 2, LG)?.index).toBe(2);
  });

  it("is named in the request, since the server judges a file played as it is by its own pick", () => {
    const f = remux();
    const req = playbackRequest(70, choices({ file: f, audio: initialAudio(f, null, LG) }), "webos-abc", LG, true);
    expect(req.audio_index).toBe(3);
    expect(playbackRequest(70, choices(), "webos-abc", LG, true).audio_index).toBeUndefined();
  });
});

describe("the play request", () => {
  it("asks for keyed addresses and sends the TV's capabilities, with no track indexes for a direct play", () => {
    const req = playbackRequest(70, choices(), "webos-abc", LG, true);
    expect(req).toEqual({ item_id: 70, file_id: 7, start_ms: 600_000, mode: "auto", client_id: "webos-abc", caps: LG, keyed_urls: true });
  });

  it("names the audio track when it doesn't play here, even in a direct play", () => {
    const f = file();
    const req = playbackRequest(70, choices({ audio: f.streams[2]! }), "c", LG, true);
    expect(req.audio_index).toBe(2);
  });

  it("names picture subtitles to burn in only when the TV can't draw them from their .mks", () => {
    const f = file();
    const pgs = f.streams[3]!;
    expect(playbackRequest(70, choices({ subtitle: pgs, streaming: true, sideload: true }), "c", LG, true).subtitle_index).toBeUndefined();
    expect(playbackRequest(70, choices({ subtitle: pgs, streaming: true, sideload: false }), "c", LG, true).subtitle_index).toBe(3);
    expect(playbackRequest(70, choices({ subtitle: f.streams[4]!, streaming: true, sideload: false }), "c", LG, true).subtitle_index).toBeUndefined();
  });
});

describe("Dolby Vision", () => {
  it("asks for a remux of Dolby Vision in Matroska where the TV shows it only from MP4 and TS (LG before webOS 25)", () => {
    expect(dvContainerMode(dvFile(8), LG, ["mp4", "ts"])).toBe("remux");
    expect(dvContainerMode(dvFile(8, "mp4"), LG, ["mp4", "ts"])).toBeNull();
    expect(dvContainerMode(dvFile(8), LG, ["mp4", "ts", "mkv"])).toBeNull();
    expect(dvContainerMode(dvFile(8), LG, null)).toBeNull();
  });

  it("leaves files alone on a TV that shows no Dolby Vision, and HDR10 files everywhere", () => {
    const samsung = { ...LG, hdr_types: ["hdr10", "hdr10plus", "hlg"] as ClientCaps["hdr_types"], dolby_vision: false, dolby_vision_profiles: [] };
    expect(dvContainerMode(dvFile(8), samsung, ["mp4"])).toBeNull();
    expect(dvContainerMode(file(), LG, ["mp4"])).toBeNull();
  });

  it("keeps the Android app's guards: profile 7 played as it is becomes a remux to its HDR10 base", () => {
    expect(dvRetryMode(info({ method: "direct" }), dvFile(7), LG)).toBe("remux");
    expect(dvRetryMode(info({ method: "remux", hls: true }), dvFile(5), LG)).toBeNull();
    const noMaster = { ...LG, hls: { master_playlist: false, segments: ["ts"] } };
    expect(dvRetryMode(info({ method: "remux", hls: true }), dvFile(5), noMaster)).toBe("transcode");
  });
});

// ---- the controller, with a pretend server and player

class FakeEngine implements PlayerEngine {
  readonly name = "Fake";
  listener: EngineListener | null = null;
  prepared: EngineSource[] = [];
  state: EngineStatus = "idle";
  released = false;
  setListener(l: EngineListener) {
    this.listener = l;
  }
  prepare(source: EngineSource) {
    this.prepared.push(source);
    this.state = "buffering";
  }
  play() {}
  pause() {}
  seekTo() {}
  stop() {
    this.state = "idle";
  }
  release() {
    this.released = true;
  }
  positionMs() {
    return 600_000;
  }
  durationMs() {
    return 7_200_000;
  }
  bufferedMs() {
    return 600_000;
  }
  status() {
    return this.state;
  }
  playing() {
    return this.state === "ready";
  }
  audioTracks() {
    return [];
  }
  selectAudio() {
    return true;
  }
  frameCounts() {
    return null;
  }
  stats() {
    return null;
  }
}

function memory(): KeyValue {
  const m = new Map<string, string>();
  return { get: (k) => m.get(k) ?? null, set: (k, v) => void m.set(k, v), remove: (k) => void m.delete(k) };
}

function world(detail: ItemDetail, answer: (req: PlaybackRequest) => PlaybackInfo, dvContainers: string[] | null = null) {
  const calls: string[] = [];
  const requests: PlaybackRequest[] = [];
  const listeners: ((e: ServerEvent) => void)[] = [];
  /** `down`: nothing answers (the Wi-Fi dropped, the server restarts). */
  const net = { down: false };
  const answered = <T>(value: () => T): Promise<T> => (net.down ? Promise.reject(new NetworkError("Can't reach the server")) : Promise.resolve(value()));
  const api = {
    server: "http://nas:7539",
    url: (p: string) => (/^https?:/.test(p) ? p : `http://nas:7539${p}`),
    item: async () => detail,
    play: (req: PlaybackRequest) =>
      answered(() => {
        requests.push(req);
        calls.push(`start ${req.mode}`);
        return answer(req);
      }),
    progress: async (_id: number, pos: number, _d: number | null, state: string) => void calls.push(`progress ${state} ${Math.round(pos)}`),
    stopSession: async (id: string) => void calls.push(`delete ${id}`),
    sessionStatus: () => answered(() => ({ session_id: "s1", method: "direct", transcode: null })),
    reachable: () => answered(() => ({})),
    setStreams: async () => undefined,
    trickplay: async () => {
      throw new Error("not ready");
    },
    next: async () => ({ item: null }),
  } as unknown as Api;
  const engine = new FakeEngine();
  const env: PlayerEnv = {
    api,
    reporter: new Reporter(),
    engine: () => engine,
    caps: async () => LG,
    keyedUrls: true,
    dvContainers: () => dvContainers,
    clientId: "webos-abc",
    offsets: new OffsetStore(memory()),
    quality: { remembered: () => "original", remember: () => undefined },
    autoplayNext: () => true,
    events: (l) => {
      listeners.push(l);
      return () => undefined;
    },
  };
  return { env, engine, calls, requests, net, send: (e: ServerEvent) => listeners.forEach((l) => l(e)) };
}

const settle = () => new Promise((r) => setTimeout(r, 20));

describe("a playback session", () => {
  it("starts at the resume point from the keyed address the server gave, with the container's type", async () => {
    const w = world(item(file()), () => info());
    const c = new PlaybackController({ itemId: 70 }, w.env, 0);
    c.start();
    await settle();
    expect(w.requests[0]).toMatchObject({ item_id: 70, file_id: 7, start_ms: 600_000, keyed_urls: true, client_id: "webos-abc" });
    expect(w.engine.prepared[0]).toMatchObject({ url: "http://nas:7539/api/media/pley_mk_KEY/file.mkv", hls: false, mime: "video/x-matroska", startMs: 600_000 });
    c.close();
  });

  it("asks for a remux of Dolby Vision in Matroska on a TV that shows it only from MP4 and TS", async () => {
    const w = world(item(dvFile(8)), (req) => info({ method: req.mode === "remux" ? "remux" : "direct", hls: req.mode === "remux", url: "/api/media/pley_mk_KEY/master.m3u8" }), ["mp4", "ts"]);
    const c = new PlaybackController({ itemId: 70 }, w.env, 0);
    c.start();
    await settle();
    expect(w.requests.map((r) => r.mode)).toEqual(["remux"]);
    expect(w.engine.prepared[0]).toMatchObject({ hls: true });
    c.close();
  });

  it("refuses a server too old for keyed addresses, ending its session unplayed", async () => {
    const w = world(item(file()), () => info({ url: "/api/files/7/stream" }));
    const c = new PlaybackController({ itemId: 70 }, w.env, 0);
    c.start();
    await settle();
    expect(c.state.value.phase).toMatchObject({ kind: "error", canRetry: false });
    expect(w.engine.prepared).toEqual([]);
    expect(w.calls).toContain("delete s1");
    expect(w.calls.filter((x) => x.indexOf("progress") === 0)).toEqual([]);
    c.close();
  });

  it("reports the end, then ends the session, when the player closes", async () => {
    const w = world(item(file()), () => info());
    const c = new PlaybackController({ itemId: 70 }, w.env, 0);
    c.start();
    await settle();
    c.close();
    await settle();
    expect(w.calls).toEqual(["start auto", "progress stopped 600000", "delete s1"]);
    expect(w.engine.released).toBe(true);
  });

  it("stops for good when the server stops the session, keeping the resume point", async () => {
    const w = world(item(file()), () => info());
    const c = new PlaybackController({ itemId: 70 }, w.env, 0);
    c.start();
    await settle();
    w.send({ type: "session_stopped", session_id: "s1", message: "Bedtime", by_admin: true });
    await settle();
    const phase = c.state.value.phase;
    expect(phase).toMatchObject({ kind: "error", canRetry: false });
    expect(phase.kind === "error" && phase.message).toContain("Bedtime");
    expect(c.state.value.stoppedByServer).toBe(true);
    expect(w.calls.filter((x) => x.indexOf("delete") === 0)).toEqual([]);
    c.close();
  });

  it("tries again with a conversion when the TV can't decode the file", async () => {
    const w = world(item(file()), (req) =>
      info(req.mode === "auto" ? {} : { session_id: "s2", method: req.mode === "remux" ? "remux" : "transcode", hls: true, url: "/api/media/pley_mk_KEY2/master.m3u8" }),
    );
    const c = new PlaybackController({ itemId: 70 }, w.env, 0);
    c.start();
    await settle();
    w.engine.listener?.error({ kind: "unsupported", audio: false, container: false, detail: "decoder" });
    await new Promise((r) => setTimeout(r, 2_200));
    expect(w.requests.map((r) => r.mode)).toEqual(["auto", "transcode"]);
    expect(w.engine.prepared[1]).toMatchObject({ hls: true, url: "http://nas:7539/api/media/pley_mk_KEY2/master.m3u8" });
    c.close();
  });
});

describe("the server out of reach", () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  /** A playback that has shown its picture, on a clock the test moves. */
  async function playing() {
    vi.useFakeTimers();
    const w = world(item(file()), () => info());
    w.env.now = () => Date.now();
    const c = new PlaybackController({ itemId: 70 }, w.env, 0);
    c.start();
    await vi.advanceTimersByTimeAsync(50);
    w.engine.state = "ready";
    w.engine.listener?.status("ready");
    return { w, c };
  }

  it("keeps the place and waits for the server, then starts again where it was", async () => {
    const { w, c } = await playing();
    w.net.down = true;
    w.engine.state = "buffering";
    w.engine.listener?.error({ kind: "network", detail: "connection failed" });
    await vi.advanceTimersByTimeAsync(10);
    expect(c.state.value.reconnecting).toBe(true);
    expect(c.state.value.phase.kind).toBe("buffering");
    // Away 30 s: no session spent, no conversion, no error.
    await vi.advanceTimersByTimeAsync(30_000);
    expect(w.requests.length).toBe(1);
    expect(c.state.value.phase.kind).toBe("buffering");
    w.net.down = false;
    await vi.advanceTimersByTimeAsync(2_100);
    expect(c.state.value.reconnecting).toBe(false);
    expect(w.requests.map((r) => [r.mode, r.start_ms])).toEqual([
      ["auto", 600_000],
      ["auto", 600_000],
    ]);
    c.close();
  });

  it("says so after 90 s, and Try again starts over", async () => {
    const { w, c } = await playing();
    w.net.down = true;
    w.engine.listener?.error({ kind: "network", detail: "connection failed" });
    await vi.advanceTimersByTimeAsync(89_000);
    expect(c.state.value.phase.kind).toBe("buffering");
    await vi.advanceTimersByTimeAsync(3_000);
    expect(c.state.value.phase).toMatchObject({ kind: "error", title: "Can't reach your server", canRetry: true });
    expect(c.state.value.reconnecting).toBe(false);
    w.net.down = false;
    c.retry();
    await vi.advanceTimersByTimeAsync(2_500);
    expect(w.requests.length).toBe(2);
    expect(c.state.value.phase.kind).not.toBe("error");
    c.close();
  });

  it("says so at once when the server is off as play is pressed", async () => {
    vi.useFakeTimers();
    const w = world(item(file()), () => info());
    w.net.down = true;
    const c = new PlaybackController({ itemId: 70 }, w.env, 0);
    c.start();
    await vi.advanceTimersByTimeAsync(50);
    expect(c.state.value.phase).toMatchObject({ kind: "error", title: "Can't reach your server", canRetry: true });
    c.close();
  });

  it("waits for the server, rather than converting the video, when a stall finds it gone", async () => {
    const { w, c } = await playing();
    w.net.down = true;
    w.engine.state = "buffering";
    // The TV's own player says nothing: 12 s without moving.
    await vi.advanceTimersByTimeAsync(13_000);
    expect(c.state.value.reconnecting).toBe(true);
    w.net.down = false;
    await vi.advanceTimersByTimeAsync(2_100);
    expect(w.requests.map((r) => r.mode)).toEqual(["auto", "auto"]);
    c.close();
  });
});
