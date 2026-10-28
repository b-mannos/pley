import type { EngineListener, EngineSource, EngineStats, EngineStatus, EngineTrack, PlayerEngine } from "./engine";
import type { EngineError } from "./recovery";

/** The MIME type HLS goes by on TVs. */
export const HLS_MIME = "application/vnd.apple.mpegurl";

/** A picture held across a switch (`holdPicture`) is kept at most this wide: the app's screen is 1920 pixels. */
const HOLD_MAX_WIDTH = 1920;

interface AudioTrackLike {
  enabled: boolean;
  language: string;
  label: string;
  kind: string;
  id: string;
}

interface AudioTrackListLike {
  length: number;
  [index: number]: AudioTrackLike;
}

type VideoWithTracks = HTMLVideoElement & {
  audioTracks?: AudioTrackListLike;
  webkitDroppedFrameCount?: number;
  webkitDecodedFrameCount?: number;
};

/**
 * A player on a <video> element: on LG TVs the web engine hands its
 * address to the TV's own media pipeline (hardware decoders, HDR and
 * Dolby Vision from the stream's metadata, Dolby audio sent as it is);
 * in a desktop browser it plays what the browser plays. Subclasses say
 * how a source is given to the element (LG's mediaOption).
 */
export class VideoEngine implements PlayerEngine {
  readonly name: string;
  protected readonly video: VideoWithTracks;
  protected listener: EngineListener | null = null;
  protected source: EngineSource | null = null;
  private state: EngineStatus = "idle";
  private shown = false;
  private wantPlay = false;
  private pendingSeek: number | null = null;
  private generation = 0;
  private readonly off: (() => void)[] = [];
  /** The last picture of a source left for one made ready beside it, shown until that one shows its own. */
  private held: HTMLCanvasElement | null = null;

  constructor(host: HTMLElement, name = "Video element") {
    this.name = name;
    const video = document.createElement("video") as VideoWithTracks;
    video.className = "engine-video";
    video.preload = "auto";
    video.setAttribute("playsinline", "");
    video.controls = false;
    host.appendChild(video);
    this.video = video;
    const on = (type: string, fn: () => void) => {
      video.addEventListener(type, fn);
      this.off.push(() => video.removeEventListener(type, fn));
    };
    on("waiting", () => this.set("buffering"));
    on("seeking", () => this.set("buffering"));
    on("canplay", () => {
      if (this.state !== "ended") this.set("ready");
    });
    on("playing", () => {
      this.set("ready");
      this.firstPicture();
    });
    on("seeked", () => {
      if (this.state === "buffering" && this.video.readyState >= 3) this.set("ready");
    });
    on("loadedmetadata", () => {
      // Engines that can't start at a position seek once the duration is known.
      if (this.pendingSeek !== null) {
        const at = this.pendingSeek;
        this.pendingSeek = null;
        if (at > 0) this.video.currentTime = at / 1000;
      }
      this.listener?.tracks();
    });
    on("loadeddata", () => {
      if (!this.wantPlay) this.firstPicture();
    });
    on("ended", () => this.set("ended"));
    on("error", () => this.failed());
  }

  setListener(listener: EngineListener): void {
    this.listener = listener;
  }

  /** How the element is given `source` (LG adds its mediaOption here). */
  protected attach(source: EngineSource): void {
    this.pendingSeek = source.startMs > 0 ? source.startMs : null;
    this.video.src = source.url;
  }

  prepare(source: EngineSource, play: boolean): void {
    this.generation++;
    this.detach();
    this.source = source;
    this.shown = false;
    this.wantPlay = play;
    this.set("buffering");
    this.attach(source);
    this.video.load();
    if (play) this.startPlaying();
  }

  private startPlaying(): void {
    const mine = this.generation;
    const p = this.video.play() as Promise<void> | undefined;
    if (p && typeof p.catch === "function") {
      p.catch((e: unknown) => {
        // A pause or a new source interrupts play(): nothing went wrong.
        if (mine !== this.generation || (e instanceof Error && e.name === "AbortError")) return;
        if (e instanceof Error && e.name === "NotSupportedError") this.failed();
      });
    }
  }

  play(): void {
    this.wantPlay = true;
    if (this.state === "ended") this.video.currentTime = 0;
    this.startPlaying();
  }

  pause(): void {
    this.wantPlay = false;
    this.video.pause();
  }

  seekTo(ms: number): void {
    if (this.video.readyState < 1) {
      this.pendingSeek = ms;
      return;
    }
    if (this.state === "ended") this.set("buffering");
    this.video.currentTime = Math.max(0, ms) / 1000;
  }

  /**
   * Draws the picture showing now onto a canvas over the element, kept
   * until the next source shows its first picture. A desktop browser
   * draws it; where the TV's pipeline keeps its pictures to itself (LG's,
   * on a plane of its own) nothing is drawn, and the screen is black
   * meanwhile, as without it.
   */
  holdPicture(): void {
    const v = this.video;
    if (!v.videoWidth || !v.videoHeight || v.readyState < 2) return;
    let canvas = this.held;
    if (!canvas) {
      canvas = document.createElement("canvas");
      canvas.className = "engine-held";
      canvas.style.display = "none";
      v.parentNode?.insertBefore(canvas, v.nextSibling);
      this.held = canvas;
    }
    const scale = Math.min(1, HOLD_MAX_WIDTH / v.videoWidth);
    canvas.width = Math.max(1, Math.round(v.videoWidth * scale));
    canvas.height = Math.max(1, Math.round(v.videoHeight * scale));
    try {
      const context = canvas.getContext("2d");
      if (!context) throw new Error("no 2d context");
      context.drawImage(v, 0, 0, canvas.width, canvas.height);
    } catch {
      this.letGo();
      return;
    }
    canvas.style.display = "block";
  }

  /** The held picture goes: the new source shows, failed, or stopped. */
  private letGo(): void {
    const canvas = this.held;
    if (!canvas) return;
    canvas.style.display = "none";
    // Its pixels go too: a 1080p picture is 8 MB a TV can use elsewhere.
    if (canvas.width > 1) {
      canvas.width = 1;
      canvas.height = 1;
    }
  }

  stop(): void {
    this.generation++;
    this.wantPlay = false;
    this.letGo();
    this.detach();
    this.source = null;
    this.set("idle");
  }

  /** Empties the element so the pipeline lets the source go. */
  protected detach(): void {
    this.video.pause();
    while (this.video.firstChild) this.video.removeChild(this.video.firstChild);
    this.video.removeAttribute("src");
    try {
      this.video.load();
    } catch {
      // Nothing loaded.
    }
  }

  release(): void {
    this.stop();
    for (const f of this.off) f();
    this.off.length = 0;
    this.video.parentNode?.removeChild(this.video);
    this.held?.parentNode?.removeChild(this.held);
    this.held = null;
    this.listener = null;
  }

  /** The element's own rate: its default one too, which each new source starts at; browsers keep the voices' pitch. */
  setSpeed(rate: number): boolean {
    this.video.defaultPlaybackRate = rate;
    this.video.playbackRate = rate;
    return true;
  }

  positionMs(): number {
    return Math.max(0, (this.video.currentTime || 0) * 1000);
  }

  durationMs(): number {
    const d = this.video.duration;
    return isFinite(d) && d > 0 ? d * 1000 : 0;
  }

  bufferedMs(): number {
    const b = this.video.buffered;
    const at = this.video.currentTime;
    for (let i = 0; i < b.length; i++) {
      if (b.start(i) <= at + 0.25 && b.end(i) >= at) return b.end(i) * 1000;
    }
    return at * 1000;
  }

  status(): EngineStatus {
    return this.state;
  }

  playing(): boolean {
    return !this.video.paused && !this.video.ended;
  }

  audioTracks(): EngineTrack[] {
    const list = this.video.audioTracks;
    if (!list) return [];
    const out: EngineTrack[] = [];
    for (let i = 0; i < list.length; i++) {
      const t = list[i]!;
      out.push({ language: t.language || null, codec: null, channels: null });
    }
    return out;
  }

  /**
   * The element's tracks follow the pipeline's own order, which isn't
   * always the file's (LG moves the default first): the ordinal is taken
   * when its language agrees, else the first track in that language.
   */
  selectAudio(ordinal: number, language: string | null): boolean {
    const list = this.video.audioTracks;
    if (!list || !list.length) return false;
    const want = norm(language);
    let pick = -1;
    const at = list[ordinal];
    if (at && (!want || norm(at.language) === want || !at.language)) pick = ordinal;
    if (pick < 0 && want) {
      for (let i = 0; i < list.length; i++) {
        if (norm(list[i]!.language) === want) {
          pick = i;
          break;
        }
      }
    }
    if (pick < 0) return false;
    for (let i = 0; i < list.length; i++) list[i]!.enabled = i === pick;
    return true;
  }

  frameCounts(): { dropped: number; total: number } | null {
    const v = this.video as VideoWithTracks & { getVideoPlaybackQuality?: () => { droppedVideoFrames: number; totalVideoFrames: number } };
    if (v.getVideoPlaybackQuality) {
      const q = v.getVideoPlaybackQuality();
      return { dropped: q.droppedVideoFrames, total: q.totalVideoFrames };
    }
    if (typeof v.webkitDroppedFrameCount === "number" && typeof v.webkitDecodedFrameCount === "number") {
      return { dropped: v.webkitDroppedFrameCount, total: v.webkitDecodedFrameCount };
    }
    return null;
  }

  stats(): EngineStats | null {
    const v = this.video;
    if (!v.videoWidth) return null;
    const ahead = this.bufferedMs() - this.positionMs();
    return { width: v.videoWidth, height: v.videoHeight, bufferedAheadMs: ahead > 0 ? ahead : 0 };
  }

  protected set(status: EngineStatus): void {
    if (this.state === status) return;
    this.state = status;
    this.listener?.status(status);
  }

  protected firstPicture(): void {
    if (this.shown) return;
    this.shown = true;
    this.letGo();
    this.listener?.firstFrame();
  }

  /** The element's error, as the session's recovery reads it. */
  protected failed(): void {
    const e = this.video.error;
    if (!e || !this.source) return;
    this.letGo();
    this.listener?.error(mediaError(e.code, e.message || "", this.source));
  }
}

function norm(language: string | null | undefined): string {
  const l = (language ?? "").toLowerCase();
  // Two- and three-letter codes of the same language compare equal.
  return LANG3[l] ?? l;
}

const LANG3: Record<string, string> = {
  en: "eng", fr: "fre", fra: "fre", de: "ger", deu: "ger", es: "spa", it: "ita", ja: "jpn", ko: "kor", zh: "chi", zho: "chi",
  pt: "por", ru: "rus", nl: "dut", nld: "dut", sv: "swe", no: "nor", da: "dan", fi: "fin", pl: "pol", tr: "tur", ar: "ara",
  he: "heb", hi: "hin", cs: "cze", ces: "cze", el: "gre", ell: "gre", hu: "hun", ro: "rum", ron: "rum", th: "tha",
};

/** MediaError codes as the recovery ladder reads them. */
export function mediaError(code: number, message: string, source: EngineSource): EngineError {
  switch (code) {
    case 2:
      return { kind: "network", detail: message || "the network failed while loading" };
    case 3:
      return { kind: "unsupported", audio: /audio/i.test(message), container: false, detail: message || "the TV couldn't decode it" };
    case 4:
      return source.hls
        ? { kind: "playlist", detail: message || "the stream couldn't be opened" }
        : { kind: "unsupported", audio: /audio/i.test(message), container: /demux|container|format/i.test(message), detail: message || "the TV can't play this file" };
    default:
      return { kind: "other", detail: message || "playback failed" };
  }
}
