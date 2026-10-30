import { signal } from "@preact/signals";
import { errorMessage, isApiError, unreachable, unreachableReason } from "../api/client";
import type { LiveChannel, LiveWatching } from "../api/types";
import { Cancel, NetworkError } from "../net/transport";
import type { EngineStatus, PlayerEngine } from "../player/engine";
import type { EngineError } from "../player/recovery";
import { HLS_MIME } from "../player/video-engine";

/** What a live player needs of the server (the Api; tests give their own). */
export interface LiveServer {
  watchChannel(channelId: number, clientId: string, cancel?: Cancel): Promise<LiveWatching>;
  leaveChannel(session: string): Promise<unknown>;
  /** A playlist or segment, as the TV's player asks for it. */
  media(method: "GET" | "HEAD", url: string, cancel?: Cancel, timeoutMs?: number): Promise<string>;
  url(path: string): string;
}

export interface LiveEnv {
  server: LiveServer;
  engine: () => PlayerEngine;
  /** This TV's own id: watching on it ends what it watched before, so changing channels holds one connection. */
  clientId: string;
}

/** Why a channel isn't playing, in words for the person; `canRetry` offers Try again. */
export interface Trouble {
  kind: "busy" | "unavailable" | "stopped" | "missing" | "older" | "away" | "other";
  title: string;
  message: string;
  canRetry: boolean;
}

/** The server's answer as a trouble (the web's, with the TVs' words for a server out of reach). */
export function troubleOf(e: unknown, channel: string): Trouble {
  const message = errorMessage(e);
  if (isApiError(e, 409)) return { kind: "busy", title: "All connections are in use", message, canRetry: true };
  // Stopped on the server, or let go after the TV asked for nothing for a while: watching again may well work.
  if (isApiError(e, 410)) return { kind: "stopped", title: "Watching stopped", message, canRetry: true };
  if (isApiError(e, 404)) return { kind: "missing", title: "This channel isn't here any more", message, canRetry: false };
  if (isApiError(e, 403)) return { kind: "other", title: "Live TV isn't open to this profile", message, canRetry: false };
  if (isApiError(e, 503)) return { kind: "unavailable", title: `${channel} isn't playing`, message, canRetry: true };
  if (unreachable(e)) {
    return {
      kind: "away",
      title: "Can't reach your server",
      message: `${unreachableReason(e)} Check that the server is on and that this device is on its network.`,
      canRetry: true,
    };
  }
  return { kind: "other", title: `${channel} didn't start`, message, canRetry: true };
}

export type LivePhase = "tuning" | "playing" | "buffering" | "paused" | "error";

export interface LiveState {
  channel: LiveChannel;
  phase: LivePhase;
  trouble: Trouble | null;
  /** The channel's picture (or sound) has started. */
  shown: boolean;
  /** It has sound and no picture (a radio channel). */
  audioOnly: boolean;
  /** Counts each channel tuned by the person (not a recovery), so the banner shows anew. */
  tunes: number;
}

export interface LiveClock {
  positionMs: number;
  /** The timeshift window that can be sought in, on the engine's clock. */
  window: { startMs: number; endMs: number } | null;
  /** How far behind the live edge it plays. */
  behindMs: number;
}

/** The engine's place this far from the window's end is live: the TVs' players stay three 2 s segments behind it. */
export const LIVE_EDGE_MS = 6_000;
/** The server waits for the channel's first segments before it answers the playlist. */
const PLAYLIST_WAIT_MS = 45_000;
/** No picture this long after the playlist came: the channel didn't start. */
const START_WAIT_MS = 30_000;
/** Paused, the player asks for nothing; the server lets a viewer go after 45 s of that, so the playlist is asked for this often. */
const KEEP_WATCHING_MS = 20_000;
/** Trying again after the engine fails, this many times, a second, then three later. */
const RETRIES = 2;
/** The audio is checked for a picture this long after it starts. */
const PICTURE_CHECK_MS = 1_500;
const CLOCK_MS = 500;

/**
 * A live channel playing on the TV's own player (the web's LivePlayerPage
 * without the page): watching it on the server with this TV's id and a
 * keyed playlist address, the engine on that playlist, changing channels,
 * pause and the timeshift, the server told at once when watching ends.
 */
export class LiveController {
  readonly state = signal<LiveState>(null as unknown as LiveState);
  readonly clock = signal<LiveClock>({ positionMs: 0, window: null, behindMs: 0 });
  private readonly env: LiveEnv;
  private engineRef: PlayerEngine | null = null;
  private session: string | null = null;
  private playlist: string | null = null;
  private generation = 0;
  private cancel: Cancel | null = null;
  private retries = 0;
  private wantPlay = true;
  private startTimer: ReturnType<typeof setTimeout> | null = null;
  private pictureTimer: ReturnType<typeof setTimeout> | null = null;
  private retryTimer: ReturnType<typeof setTimeout> | null = null;
  private clockTimer: ReturnType<typeof setInterval> | null = null;
  private keepTimer: ReturnType<typeof setInterval> | null = null;
  private closed = false;

  constructor(channel: LiveChannel, env: LiveEnv) {
    this.env = env;
    this.state.value = { channel, phase: "tuning", trouble: null, shown: false, audioOnly: false, tunes: 0 };
  }

  get engine(): PlayerEngine | null {
    return this.engineRef;
  }

  /** Whether it plays (or will once it has tuned), rather than paused. */
  get wantsToPlay(): boolean {
    return this.wantPlay;
  }

  private patch(p: Partial<LiveState>): void {
    this.state.value = { ...this.state.value, ...p };
  }

  private ensureEngine(): PlayerEngine {
    if (this.engineRef) return this.engineRef;
    const engine = this.env.engine();
    engine.setListener({
      status: (s) => this.onStatus(s),
      firstFrame: () => this.onFirstFrame(),
      error: (e) => this.onEngineError(e),
      tracks: () => undefined,
    });
    this.engineRef = engine;
    return engine;
  }

  /** Watches `channel` from its live edge; what was watched before ends. */
  tune(channel: LiveChannel): void {
    this.retries = 0;
    this.wantPlay = true;
    this.patch({ tunes: this.state.value.tunes + 1 });
    void this.start(channel);
  }

  /** The same channel again (Try again). */
  retry(): void {
    this.tune(this.state.value.channel);
  }

  private async start(channel: LiveChannel): Promise<void> {
    if (this.closed) return;
    const mine = ++this.generation;
    this.clearTimers();
    this.cancel?.cancel();
    const cancel = new Cancel();
    this.cancel = cancel;
    this.engineRef?.stop();
    this.leave();
    this.patch({ channel, phase: "tuning", trouble: null, shown: false, audioOnly: false });
    this.clock.value = { positionMs: 0, window: null, behindMs: 0 };
    const server = this.env.server;
    let watching: LiveWatching;
    try {
      watching = await server.watchChannel(channel.id, this.env.clientId, cancel);
    } catch (e) {
      if (mine === this.generation && !isCancelled(e)) this.fail(troubleOf(e, channel.name));
      return;
    }
    if (mine !== this.generation) {
      void server.leaveChannel(watching.session_id).catch(() => undefined);
      return;
    }
    this.session = watching.session_id;
    this.patch({ channel: { ...channel, ...watching.channel } });
    // An older server's address needs the token, which the TV's player can't send.
    if (!/\/api\/live\/media\//.test(watching.url)) {
      this.fail({
        kind: "older",
        title: "Live TV needs a newer Pley on the server",
        message: "This server can't send live channels to the TV's own player. Update Pley on the server, then try again.",
        canRetry: false,
      });
      return;
    }
    const playlist = server.url(watching.url);
    this.playlist = playlist;
    try {
      // The channel's first segments, before the player asks: a TV's player gives up sooner than the server takes to tune.
      await server.media("GET", playlist, cancel, PLAYLIST_WAIT_MS);
    } catch (e) {
      if (mine === this.generation && !isCancelled(e)) this.fail(troubleOf(e, channel.name));
      return;
    }
    if (mine !== this.generation) return;
    const engine = this.ensureEngine();
    engine.prepare({ url: playlist, hls: true, live: true, mime: HLS_MIME, startMs: 0, durationHintMs: null, video: null, audioCodec: null }, true);
    if (!this.wantPlay) engine.pause();
    this.startTimer = setTimeout(() => {
      if (mine === this.generation && !this.state.value.shown) this.onEngineError({ kind: "other", detail: "it didn't start in time" });
    }, START_WAIT_MS);
    this.clockTimer = setInterval(() => this.tick(), CLOCK_MS);
  }

  private onStatus(s: EngineStatus): void {
    const st = this.state.value;
    if (st.phase === "error" || st.phase === "tuning") return;
    if (s === "buffering") this.patch({ phase: "buffering" });
    else if (s === "ready") this.patch({ phase: this.wantPlay ? "playing" : "paused" });
    else if (s === "ended") this.onEngineError({ kind: "other", detail: "the stream ended" });
  }

  private onFirstFrame(): void {
    if (this.startTimer !== null) clearTimeout(this.startTimer);
    this.startTimer = null;
    this.patch({ shown: true, phase: this.wantPlay ? "playing" : "paused" });
    const mine = this.generation;
    if (this.pictureTimer !== null) clearTimeout(this.pictureTimer);
    this.pictureTimer = setTimeout(() => {
      this.pictureTimer = null;
      if (mine !== this.generation) return;
      const stats = this.engineRef?.stats() ?? null;
      this.patch({ audioOnly: !(stats && stats.width) });
    }, PICTURE_CHECK_MS);
  }

  private onEngineError(e: EngineError): void {
    if (this.closed) return;
    const channel = this.state.value.channel;
    console.warn(`pley: live ${channel.name}: the player failed (${e.kind}${"detail" in e ? `: ${e.detail}` : ""})`);
    if (e.kind === "stopped" || (e.kind === "http" && e.status === 410)) {
      this.fail({
        kind: "stopped",
        title: "Watching stopped",
        message: (e.kind === "stopped" && e.message) || "The channel was stopped on the server.",
        canRetry: true,
      });
      return;
    }
    if (this.retries < RETRIES) {
      // Watched anew: the server may have let the channel go, or the network blinked.
      this.retries++;
      const mine = ++this.generation;
      this.clearTimers();
      this.engineRef?.stop();
      this.patch({ phase: "tuning" });
      this.retryTimer = setTimeout(() => {
        this.retryTimer = null;
        if (mine === this.generation) void this.start(channel);
      }, this.retries === 1 ? 1_000 : 3_000);
      return;
    }
    const detail = "detail" in e ? e.detail : "";
    this.fail({ kind: "other", title: `${channel.name} stopped`, message: detail ? `The TV's player said: ${detail}` : "", canRetry: true });
  }

  private fail(trouble: Trouble): void {
    this.generation++;
    this.clearTimers();
    this.engineRef?.stop();
    this.leave();
    this.patch({ phase: "error", trouble });
  }

  private tick(): void {
    const engine = this.engineRef;
    if (!engine) return;
    const positionMs = engine.positionMs();
    const window = engine.liveWindow?.() ?? null;
    const behindMs = window ? Math.max(0, window.endMs - LIVE_EDGE_MS - positionMs) : 0;
    const was = this.clock.value;
    if (was.positionMs === positionMs && was.behindMs === behindMs && sameWindow(was.window, window)) return;
    this.clock.value = { positionMs, window, behindMs };
  }

  play(): void {
    this.wantPlay = true;
    this.stopKeeping();
    const st = this.state.value;
    if (st.phase === "error") return;
    this.engineRef?.play();
    if (st.phase === "paused") this.patch({ phase: "playing" });
  }

  pause(): void {
    this.wantPlay = false;
    const st = this.state.value;
    if (st.phase === "error") return;
    this.engineRef?.pause();
    if (st.phase === "playing" || st.phase === "buffering") this.patch({ phase: "paused" });
    this.keepWatching();
  }

  togglePause(): void {
    if (this.wantPlay) this.pause();
    else this.play();
  }

  /** Whether the window can be sought in (the engine said where it is). */
  get canSeek(): boolean {
    return this.clock.value.window !== null && this.state.value.shown;
  }

  /** `ms` further (or back) in the timeshift window, never past the live edge. */
  seekBy(ms: number): void {
    const engine = this.engineRef;
    const w = engine?.liveWindow?.() ?? null;
    if (!engine || !w || !this.state.value.shown) return;
    this.seekTo(engine.positionMs() + ms);
  }

  seekTo(ms: number): void {
    const engine = this.engineRef;
    const w = engine?.liveWindow?.() ?? null;
    if (!engine || !w) return;
    const target = Math.max(w.startMs, Math.min(w.endMs - LIVE_EDGE_MS, ms));
    engine.seekTo(target);
    this.clock.value = { positionMs: target, window: w, behindMs: Math.max(0, w.endMs - LIVE_EDGE_MS - target) };
  }

  /** Back to the live edge, playing. */
  goLive(): void {
    const w = this.engineRef?.liveWindow?.() ?? null;
    if (w) this.seekTo(w.endMs);
    this.play();
  }

  /** The TV hid the app: watching ends on the server, to start again when it shows. */
  suspend(): void {
    this.generation++;
    this.cancel?.cancel();
    this.clearTimers();
    this.engineRef?.stop();
    this.leave();
  }

  /** The TV shows the app again: the channel from its live edge. */
  resume(): void {
    if (this.closed) return;
    this.retries = 0;
    this.wantPlay = true;
    void this.start(this.state.value.channel);
  }

  /** The player goes: watching ends on the server at once. */
  close(): void {
    if (this.closed) return;
    this.closed = true;
    this.generation++;
    this.cancel?.cancel();
    this.clearTimers();
    this.leave();
    this.engineRef?.release();
    this.engineRef = null;
  }

  private leave(): void {
    const session = this.session;
    this.session = null;
    this.playlist = null;
    if (session) void this.env.server.leaveChannel(session).catch(() => undefined);
  }

  private keepWatching(): void {
    this.stopKeeping();
    this.keepTimer = setInterval(() => {
      const playlist = this.playlist;
      if (playlist) void this.env.server.media("GET", playlist, undefined, 10_000).catch(() => undefined);
    }, KEEP_WATCHING_MS);
  }

  private stopKeeping(): void {
    if (this.keepTimer !== null) clearInterval(this.keepTimer);
    this.keepTimer = null;
  }

  private clearTimers(): void {
    if (this.startTimer !== null) clearTimeout(this.startTimer);
    if (this.pictureTimer !== null) clearTimeout(this.pictureTimer);
    if (this.retryTimer !== null) clearTimeout(this.retryTimer);
    if (this.clockTimer !== null) clearInterval(this.clockTimer);
    this.startTimer = this.pictureTimer = this.retryTimer = null;
    this.clockTimer = null;
    this.stopKeeping();
  }
}

function isCancelled(e: unknown): boolean {
  return e instanceof NetworkError && e.cancelled;
}

function sameWindow(a: LiveClock["window"], b: LiveClock["window"]): boolean {
  return a === b || (!!a && !!b && a.startMs === b.startMs && a.endMs === b.endMs);
}
