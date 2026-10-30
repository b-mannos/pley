import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useNavigate, useParams, useSearchParams } from "react-router";
import { keepPreviousData, useQuery, useQueryClient } from "@tanstack/react-query";
import type HlsType from "hls.js";
import {
  ArrowLeft,
  ChevronDown,
  ChevronUp,
  List,
  LoaderCircle,
  Maximize,
  Minimize,
  Pause,
  PictureInPicture2,
  Play,
  RefreshCw,
  RotateCcw,
  RotateCw,
  Star,
  TriangleAlert,
  Tv2,
  Volume1,
  Volume2,
  VolumeX,
} from "lucide-react";
import { ApiError, errorMessage, isApiError } from "@/api/client";
import {
  CHANNEL_PAGE,
  leaveChannel,
  liveKeys,
  liveQueries,
  parseCategory,
  programProgress,
  useSetFavorite,
  watchChannel,
  type Channel,
  type LiveCategory,
} from "@/api/livetv";
import { useOptionalAudioPlayer } from "@/audio/AudioPlayer";
import { Slider } from "@/components/Slider";
import { cn } from "@/lib/cn";
import { canPlayNativeHls } from "@/lib/caps";
import { loadHls } from "@/lib/hls";
import { useDocumentTitle, useLocalStorage } from "@/lib/hooks";
import { clientId } from "@/lib/progress";
import { ChannelLogo, formatSpan, formatTime, liveUrl, setLastWatched, timeLeft, useNow } from "@/pages/livetv/common";
import { ChannelPanel } from "./ChannelPanel";

/** Controls hide after this long without the pointer moving. */
const HIDE_CONTROLS_AFTER = 3500;
/** What's on, shown with I, goes after this long. */
const BANNER_FOR = 5000;
/** Seconds behind the live edge that still count as live. */
const LIVE_WITHIN = 10;
/** Digits typed for a channel number wait this long for more. */
const NUMBER_WAIT = 1400;

type Trouble = { kind: "busy" | "unavailable" | "stopped" | "missing" | "other"; title: string; message: string };

function trouble(err: unknown, channel: string): Trouble {
  if (isApiError(err, 409)) return { kind: "busy", title: "All connections are in use", message: errorMessage(err) };
  if (isApiError(err, 410)) return { kind: "stopped", title: "Watching stopped", message: errorMessage(err) };
  if (isApiError(err, 404)) return { kind: "missing", title: "This channel isn't here any more", message: errorMessage(err) };
  if (isApiError(err, 403)) return { kind: "other", title: "Live TV isn't open to this account", message: errorMessage(err) };
  if (isApiError(err, 503)) return { kind: "unavailable", title: `${channel} isn't playing`, message: errorMessage(err) };
  return { kind: "other", title: `${channel} didn't start`, message: errorMessage(err) };
}

/** An HLS error answered by the server: its status and its own words. */
function hlsTrouble(response: { code?: number; text?: string } | undefined, channel: string): Trouble | null {
  if (!response?.code || response.code < 400) return null;
  let message = "";
  try {
    message = (JSON.parse(response.text ?? "") as { message?: string }).message ?? "";
  } catch {
    // not the server's JSON
  }
  return trouble(new ApiError(response.code, "hls", message || `the server answered ${response.code}`), channel);
}

export function LivePlayerPage() {
  const { channelId } = useParams();
  const id = Number(channelId);
  const [params] = useSearchParams();
  const category = useMemo(() => parseCategory(params.get("in")), [params]);
  const navigate = useNavigate();
  const qc = useQueryClient();
  const current = useQuery(liveQueries.channel(id));
  const channel = current.data ?? null;
  const zap = useZap(category, id);
  useDocumentTitle(channel ? `${channel.name} · Live TV` : "Live TV");

  const tune = useCallback(
    (c: Channel, inCategory: LiveCategory = category) => {
      // What the list knows of it shows at once.
      qc.setQueryData(liveKeys.channel(c.id), c);
      void navigate(liveUrl(c.id, inCategory), { replace: true });
    },
    [navigate, qc, category],
  );
  const step = useCallback(
    (by: 1 | -1) => {
      const next = zap(by);
      if (next) tune(next);
    },
    [zap, tune],
  );
  // A channel number: every channel's, as a remote's would.
  const byNumber = useCallback(async (digits: string) => {
    const page = await qc.fetchQuery(liveQueries.page({ kind: "all" }, digits, 0));
    return page.channels.find((c) => c.number === digits) ?? null;
  }, [qc]);

  if (current.isError) {
    const missing = isApiError(current.error, 404);
    return (
      <Blackout>
        <TroubleCard
          trouble={
            missing
              ? { kind: "missing", title: "This channel isn't here", message: "It may have been hidden, or its source removed." }
              : { kind: "other", title: "Live TV didn't load", message: errorMessage(current.error) }
          }
          onRetry={missing ? undefined : () => void current.refetch()}
          onBack={() => void navigate("/live")}
        />
      </Blackout>
    );
  }
  if (!channel) {
    return (
      <Blackout>
        <LoaderCircle className="size-12 animate-spin text-white/70" />
      </Blackout>
    );
  }
  return (
    <LivePlayer
      channel={channel}
      category={category}
      onTune={tune}
      onStep={step}
      byNumber={byNumber}
    />
  );
}

/**
 * Channel up and down through `category`: the page around the channel
 * watched, and the next or last page at its ends (round to the other end
 * of the list). "Recently watched" keeps the order it had as the player
 * opened: it changes with every channel watched.
 */
function useZap(category: LiveCategory, id: number): (by: 1 | -1) => Channel | null {
  const recent = category.kind === "recent";
  const around = useQuery({ ...liveQueries.around(category, id), enabled: !recent, placeholderData: keepPreviousData });
  const recentPage = useQuery({ ...liveQueries.page(category, "", 0), enabled: recent });
  const [recentOrder, setRecentOrder] = useState<Channel[] | null>(null);
  if (recent && !recentOrder && recentPage.data) setRecentOrder(recentPage.data.channels);

  const page = around.data;
  const at = page ? page.channels.findIndex((c) => c.id === id) : -1;
  const pages = page ? Math.ceil(page.total / CHANNEL_PAGE) : 0;
  const pageNo = page ? Math.floor(page.offset / CHANNEL_PAGE) : 0;
  const edge = page != null && pages > 1;
  const before = useQuery({
    ...liveQueries.page(category, "", (pageNo - 1 + pages) % Math.max(1, pages)),
    enabled: edge && at === 0,
  });
  const after = useQuery({
    ...liveQueries.page(category, "", (pageNo + 1) % Math.max(1, pages)),
    enabled: edge && at === (page?.channels.length ?? 0) - 1,
  });

  return useCallback(
    (by: 1 | -1) => {
      if (recent) {
        const list = recentOrder ?? [];
        if (list.length < 2) return null;
        const i = list.findIndex((c) => c.id === id);
        return list[(Math.max(0, i) + by + list.length) % list.length]!;
      }
      if (!page || page.channels.length === 0) return null;
      // Not in this category (tuned by number, say): its first channel.
      if (at < 0) return page.channels[0]!;
      const j = at + by;
      if (j >= 0 && j < page.channels.length) return page.channels[j]!;
      if (pages <= 1) return page.channels[(j + page.channels.length) % page.channels.length]!;
      const other = by < 0 ? before.data : after.data;
      if (!other?.channels.length) return null;
      return by < 0 ? other.channels[other.channels.length - 1]! : other.channels[0]!;
    },
    [recent, recentOrder, id, page, at, pages, before.data, after.data],
  );
}

function Blackout({ children }: { children: React.ReactNode }) {
  return <div className="fixed inset-0 z-50 flex items-center justify-center bg-black p-4 text-white">{children}</div>;
}

function LivePlayer({
  channel,
  category,
  onTune,
  onStep,
  byNumber,
}: {
  channel: Channel;
  category: LiveCategory;
  onTune: (c: Channel, category?: LiveCategory) => void;
  onStep: (by: 1 | -1) => void;
  byNumber: (digits: string) => Promise<Channel | null>;
}) {
  const navigate = useNavigate();
  const containerRef = useRef<HTMLDivElement>(null);
  const videoRef = useRef<HTMLVideoElement>(null);
  const hlsRef = useRef<HlsType | null>(null);
  const sessionRef = useRef<{ id: string; url: string } | null>(null);
  const audioPlayer = useOptionalAudioPlayer();
  const favorite = useSetFavorite();
  const now = useNow(5000);

  const [attempt, setAttempt] = useState(0);
  const [status, setStatus] = useState<"tuning" | "playing" | "error">("tuning");
  const [problem, setProblem] = useState<Trouble | null>(null);
  const [buffering, setBuffering] = useState(false);
  const [playing, setPlaying] = useState(false);
  const [volume, setVolume] = useLocalStorage<number>("pley.volume", 1);
  const [muted, setMuted] = useState(false);
  const [fullscreen, setFullscreen] = useState(false);
  const [panel, setPanel] = useState(false);
  const [showControls, setShowControls] = useState(true);
  const [banner, setBanner] = useState(false);
  const [digits, setDigits] = useState("");
  const [notFound, setNotFound] = useState<string | null>(null);
  const [position, setPosition] = useState({ current: 0, start: 0, end: 0 });
  const hideTimer = useRef<number | undefined>(undefined);

  useEffect(() => setLastWatched(channel.id), [channel.id]);

  useEffect(() => {
    audioPlayer?.pause();
    // eslint-disable-next-line react-hooks/exhaustive-deps -- once, as the player opens
  }, []);

  // Tune: start watching, then play the channel's live stream.
  useEffect(() => {
    const video = videoRef.current;
    if (!video) return;
    let cancelled = false;
    let networkRetries = 0;
    let mediaRetries = 0;
    setStatus("tuning");
    setProblem(null);
    setBanner(false);
    const fail = (t: Trouble) => {
      if (cancelled) return;
      setProblem(t);
      setStatus("error");
      hlsRef.current?.destroy();
      hlsRef.current = null;
    };
    void (async () => {
      let watching;
      try {
        watching = await watchChannel(channel.id, clientId());
      } catch (err) {
        fail(trouble(err, channel.name));
        return;
      }
      if (cancelled) {
        void leaveChannel(watching.session_id);
        return;
      }
      sessionRef.current = { id: watching.session_id, url: watching.url };
      const start = () => {
        video.play().catch(() => {
          // Autoplay with sound refused: muted, then.
          video.muted = true;
          setMuted(true);
          void video.play().catch(() => undefined);
        });
      };
      const Hls = await loadHls().catch(() => null);
      if (cancelled) return;
      if (Hls?.isSupported()) {
        const hls = new Hls({
          liveSyncDurationCount: 3,
          liveMaxLatencyDurationCount: 10,
          maxBufferLength: 20,
          backBufferLength: 600,
          // The server waits for the channel's first segments before it answers.
          manifestLoadPolicy: {
            default: {
              maxTimeToFirstByteMs: 45_000,
              maxLoadTimeMs: 50_000,
              timeoutRetry: { maxNumRetry: 1, retryDelayMs: 0, maxRetryDelayMs: 0 },
              errorRetry: { maxNumRetry: 1, retryDelayMs: 1000, maxRetryDelayMs: 4000 },
            },
          },
        });
        hlsRef.current = hls;
        hls.on(Hls.Events.MANIFEST_PARSED, start);
        hls.on(Hls.Events.ERROR, (_e, data) => {
          if (!data.fatal) return;
          const answered = hlsTrouble(data.response, channel.name);
          if (answered && (answered.kind !== "other" || data.type === Hls.ErrorTypes.NETWORK_ERROR)) {
            if (answered.kind !== "other" || networkRetries >= 3) return fail(answered);
          }
          if (data.type === Hls.ErrorTypes.NETWORK_ERROR && networkRetries < 4) {
            networkRetries++;
            window.setTimeout(() => hls.startLoad(-1), 1000 * networkRetries);
          } else if (data.type === Hls.ErrorTypes.MEDIA_ERROR && mediaRetries < 2) {
            mediaRetries++;
            hls.recoverMediaError();
          } else {
            fail(answered ?? { kind: "other", title: `${channel.name} stopped`, message: data.details });
          }
        });
        hls.loadSource(watching.url);
        hls.attachMedia(video);
      } else if (canPlayNativeHls(video)) {
        video.src = watching.url;
        start();
      } else {
        fail({ kind: "other", title: "This browser can't play live TV", message: "It plays neither HLS nor Media Source Extensions." });
      }
    })();
    return () => {
      cancelled = true;
      hlsRef.current?.destroy();
      hlsRef.current = null;
      video.removeAttribute("src");
      video.load();
      if (sessionRef.current) void leaveChannel(sessionRef.current.id);
      sessionRef.current = null;
    };
  }, [channel.id, channel.name, attempt]);

  // Leaving the page (a closed tab) lets the connection go at once.
  useEffect(() => {
    const leave = () => {
      if (sessionRef.current) void leaveChannel(sessionRef.current.id, true);
    };
    window.addEventListener("pagehide", leave);
    return () => window.removeEventListener("pagehide", leave);
  }, []);

  // Paused, the player asks for nothing: the server is told it's still watched.
  useEffect(() => {
    if (playing || status !== "playing") return;
    const id = window.setInterval(() => {
      const session = sessionRef.current;
      if (session) void fetch(session.url, { credentials: "same-origin" }).catch(() => undefined);
    }, 20_000);
    return () => window.clearInterval(id);
  }, [playing, status]);

  // The video element's state.
  useEffect(() => {
    const video = videoRef.current;
    if (!video) return;
    video.volume = volume;
  }, [volume]);
  useEffect(() => {
    const video = videoRef.current;
    if (!video) return;
    video.muted = muted;
  }, [muted]);
  useEffect(() => {
    const video = videoRef.current;
    if (!video) return;
    const update = () => {
      const r = video.seekable;
      const start = r.length ? r.start(0) : 0;
      const end = hlsRef.current?.liveSyncPosition ?? (r.length ? r.end(r.length - 1) : video.currentTime);
      setPosition({ current: video.currentTime, start, end: Math.max(end, video.currentTime) });
    };
    const onPlaying = () => {
      setStatus("playing");
      setBuffering(false);
      setPlaying(true);
    };
    const handlers: [string, () => void][] = [
      ["playing", onPlaying],
      ["pause", () => setPlaying(false)],
      ["play", () => setPlaying(true)],
      ["waiting", () => setBuffering(true)],
      ["canplay", () => setBuffering(false)],
      ["timeupdate", update],
      ["progress", update],
    ];
    for (const [name, fn] of handlers) video.addEventListener(name, fn);
    return () => {
      for (const [name, fn] of handlers) video.removeEventListener(name, fn);
    };
  }, []);

  // The banner goes after a few seconds.
  useEffect(() => {
    if (!banner || status !== "playing") return;
    const id = window.setTimeout(() => setBanner(false), BANNER_FOR);
    return () => window.clearTimeout(id);
  }, [banner, status]);

  // Controls hide while playing and untouched.
  const poke = useCallback(() => {
    setShowControls(true);
    window.clearTimeout(hideTimer.current);
    hideTimer.current = window.setTimeout(() => setShowControls(false), HIDE_CONTROLS_AFTER);
  }, []);
  useEffect(() => {
    hideTimer.current = window.setTimeout(() => setShowControls(false), HIDE_CONTROLS_AFTER);
    return () => window.clearTimeout(hideTimer.current);
  }, []);
  const controlsVisible = showControls || !playing || panel || status !== "playing";

  useEffect(() => {
    const onChange = () => setFullscreen(document.fullscreenElement != null);
    document.addEventListener("fullscreenchange", onChange);
    return () => document.removeEventListener("fullscreenchange", onChange);
  }, []);

  const behind = position.end - position.current;
  const atLive = behind < LIVE_WITHIN;

  const togglePlay = useCallback(() => {
    const video = videoRef.current;
    if (!video) return;
    if (video.paused) void video.play().catch(() => undefined);
    else video.pause();
  }, []);
  const seekBy = useCallback((by: number) => {
    const video = videoRef.current;
    if (!video || !video.seekable.length) return;
    const start = video.seekable.start(0);
    const end = hlsRef.current?.liveSyncPosition ?? video.seekable.end(video.seekable.length - 1);
    video.currentTime = Math.min(end, Math.max(start, video.currentTime + by));
  }, []);
  const goLive = useCallback(() => {
    const video = videoRef.current;
    if (!video) return;
    const end = hlsRef.current?.liveSyncPosition ?? (video.seekable.length ? video.seekable.end(video.seekable.length - 1) : null);
    if (end != null) video.currentTime = end;
    if (video.paused) void video.play().catch(() => undefined);
  }, []);
  const toggleFullscreen = useCallback(() => {
    if (document.fullscreenElement) void document.exitFullscreen().catch(() => undefined);
    else void containerRef.current?.requestFullscreen?.().catch(() => undefined);
  }, []);
  const togglePip = useCallback(() => {
    const video = videoRef.current;
    if (!video) return;
    if (document.pictureInPictureElement) void document.exitPictureInPicture().catch(() => undefined);
    else void video.requestPictureInPicture?.().catch(() => undefined);
  }, []);
  const goBack = useCallback(() => {
    const idx = (window.history.state as { idx?: number } | null)?.idx ?? 0;
    if (idx > 0) void navigate(-1);
    else void navigate("/live", { replace: true });
  }, [navigate]);

  // A channel number typed: tuned once the digits stop.
  useEffect(() => {
    if (!digits) return;
    let cancelled = false;
    const id = window.setTimeout(() => {
      setDigits("");
      void byNumber(digits)
        .catch(() => null)
        .then((match) => {
          if (cancelled) return;
          if (match) onTune(match, { kind: "all" });
          else setNotFound(digits);
        });
    }, NUMBER_WAIT);
    return () => {
      cancelled = true;
      window.clearTimeout(id);
    };
  }, [digits, byNumber, onTune]);
  useEffect(() => {
    if (!notFound) return;
    const id = window.setTimeout(() => setNotFound(null), 2000);
    return () => window.clearTimeout(id);
  }, [notFound]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.ctrlKey || e.metaKey || e.altKey) return;
      const target = e.target as HTMLElement | null;
      if (target && (target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.isContentEditable)) return;
      poke();
      if (/^[0-9]$/.test(e.key)) {
        setDigits((d) => (d + e.key).slice(0, 5));
        return;
      }
      switch (e.key) {
        case " ":
        case "k":
          e.preventDefault();
          togglePlay();
          break;
        // Up is channel up, the next channel, as on a TV's remote.
        case "ArrowUp":
        case "PageUp":
        case "ChannelUp":
          e.preventDefault();
          onStep(1);
          break;
        case "ArrowDown":
        case "PageDown":
        case "ChannelDown":
          e.preventDefault();
          onStep(-1);
          break;
        case "ArrowLeft":
        case "j":
          e.preventDefault();
          seekBy(-10);
          break;
        case "ArrowRight":
        case "l":
          e.preventDefault();
          seekBy(10);
          break;
        case "m":
          setMuted((m) => !m);
          break;
        case "f":
          toggleFullscreen();
          break;
        case "g":
        case "c":
          setPanel((p) => !p);
          break;
        case "i":
          setBanner((b) => !b);
          break;
        case "Escape":
          if (panel) setPanel(false);
          else if (!document.fullscreenElement) goBack();
          break;
        case "Backspace":
        case "BrowserBack":
        case "GoBack":
          e.preventDefault();
          if (panel) setPanel(false);
          else goBack();
          break;
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [poke, togglePlay, onStep, seekBy, toggleFullscreen, panel, goBack]);

  const on = channel.now && channel.now.stop > now && channel.now.start <= now ? channel.now : null;
  const next = on ? channel.next : (channel.next ?? channel.now);
  const VolumeIcon = muted || volume === 0 ? VolumeX : volume < 0.5 ? Volume1 : Volume2;
  const button = "flex size-10 items-center justify-center rounded-full transition hover:bg-white/15 disabled:opacity-40 disabled:hover:bg-transparent";

  return (
    <div
      ref={containerRef}
      className={cn("fixed inset-0 z-50 select-none overflow-hidden bg-black text-white", !controlsVisible && "cursor-none")}
      onMouseMove={poke}
      onTouchStart={poke}
    >
      {/* The picture and its controls, beside the channel list when it is open. */}
      <div
        className={cn(
          "absolute inset-y-0 left-0 right-0 overflow-hidden transition-[right] duration-300 ease-out",
          panel && "sm:right-[24rem]",
        )}
      >
        <video
          ref={videoRef}
          className="absolute inset-0 size-full object-contain"
          playsInline
          onClick={() => (panel ? setPanel(false) : togglePlay())}
          onDoubleClick={toggleFullscreen}
        />

        {/* Tuning: the channel's logo while it starts. */}
        {status === "tuning" ? (
          <div className="absolute inset-0 flex flex-col items-center justify-center gap-6 bg-black animate-fade-in">
            <ChannelLogo channel={channel} width={240} className="h-28 w-48 rounded-2xl bg-white/[0.06]" />
            <LoaderCircle className="size-7 animate-spin text-accent" aria-label="Tuning" />
            <div className="text-center">
              <div className="text-lg font-semibold">
                {channel.number ? <span className="mr-2 tabular-nums text-white/60">{channel.number}</span> : null}
                {channel.name}
              </div>
              <div className="mt-1 text-sm text-white/60">{on ? on.title : "Tuning…"}</div>
            </div>
          </div>
        ) : null}

        {status === "playing" && buffering ? (
          <div className="pointer-events-none absolute inset-0 flex items-center justify-center">
            <LoaderCircle className="size-12 animate-spin text-white/80 drop-shadow" />
          </div>
        ) : null}

        {status === "error" && problem ? (
          <div className="absolute inset-0 flex items-center justify-center bg-black/85 p-4">
            <TroubleCard
              trouble={problem}
              channel={channel}
              onRetry={problem.kind === "stopped" || problem.kind === "missing" ? undefined : () => setAttempt((a) => a + 1)}
              onBack={goBack}
              onChannels={() => setPanel(true)}
            />
          </div>
        ) : null}

        {/* A channel number being typed. */}
        {digits || notFound ? (
          <div className="absolute right-6 top-6 z-30 rounded-xl bg-black/85 px-5 py-3 text-4xl font-bold tabular-nums tracking-widest shadow-2xl animate-pop-in">
            {digits || <span className="text-xl tracking-normal text-white/70">No channel {notFound}</span>}
          </div>
        ) : null}

        {/* Top bar */}
        <div
          className={cn(
            "absolute inset-x-0 top-0 z-10 flex items-start gap-3 bg-linear-to-b from-black/80 via-black/40 to-transparent px-3 pb-16 pt-3 transition-opacity duration-300 sm:px-6 sm:pt-5",
            controlsVisible ? "opacity-100" : "pointer-events-none opacity-0",
          )}
        >
          <button type="button" aria-label="Back" onClick={goBack} className={cn(button, "shrink-0")}>
            <ArrowLeft className="size-6" />
          </button>
          <ChannelLogo channel={channel} width={80} showName={false} className="mt-0.5 hidden h-10 w-16 shrink-0 rounded-md bg-white/10 sm:flex" />
          <div className="min-w-0 flex-1 pt-0.5">
            <div className="flex items-center gap-2 truncate text-lg font-semibold drop-shadow sm:text-xl">
              {channel.number ? <span className="tabular-nums text-white/60">{channel.number}</span> : null}
              <span className="truncate">{channel.name}</span>
            </div>
            {on ? <div className="truncate text-sm text-white/75">{on.title}</div> : null}
          </div>
          <button
            type="button"
            aria-label={channel.favorite ? "Remove from favourites" : "Add to favourites"}
            aria-pressed={channel.favorite}
            onClick={() => favorite.mutate({ id: channel.id, favorite: !channel.favorite })}
            className={button}
          >
            <Star className={cn("size-5", channel.favorite && "fill-accent text-accent")} />
          </button>
          <button type="button" aria-label="Channels" title="Channels (G)" onClick={() => setPanel((p) => !p)} className={cn(button, panel && "bg-white/15")}>
            <List className="size-5" />
          </button>
        </div>

        {/* What's on: shown with I. */}
        <div
          className={cn(
            "pointer-events-none absolute bottom-20 left-3 right-3 z-10 transition-all duration-500 sm:bottom-24 sm:left-6 sm:right-auto sm:w-[30rem]",
            banner && status === "playing" ? "translate-y-0 opacity-100" : "translate-y-3 opacity-0",
          )}
          aria-hidden={!banner}
        >
          <div className="flex gap-4 rounded-xl border border-white/10 bg-black/85 p-4 shadow-2xl">
            <ChannelLogo channel={channel} width={96} className="h-14 w-24 shrink-0 rounded-lg bg-white/[0.07]" />
            <div className="min-w-0 flex-1">
              {on ? (
                <>
                  <div className="truncate font-semibold">{on.title}</div>
                  <div className="mt-0.5 text-xs tabular-nums text-white/60">
                    {formatSpan(on)} · {timeLeft(on, now)}
                  </div>
                  <div className="mt-2 h-1 overflow-hidden rounded-full bg-white/15">
                    <div className="h-full rounded-full bg-accent" style={{ width: `${programProgress(on, now) * 100}%` }} />
                  </div>
                </>
              ) : (
                <div className="pt-1 text-sm text-white/70">{channel.has_guide ? "Nothing in the guide now" : "No guide for this channel"}</div>
              )}
              {next ? (
                <div className="mt-2 truncate text-xs text-white/55">
                  Next <span className="tabular-nums">{formatTime(next.start)}</span> · {next.title}
                </div>
              ) : null}
            </div>
          </div>
        </div>

        {/* Bottom bar */}
        <div
          className={cn(
            "absolute inset-x-0 bottom-0 z-10 bg-linear-to-t from-black/90 via-black/55 to-transparent px-3 pb-3 pt-16 transition-opacity duration-300 sm:px-6 sm:pb-5",
            controlsVisible ? "opacity-100" : "pointer-events-none opacity-0",
          )}
        >
          <div className="flex items-center gap-0.5 sm:gap-1.5">
            <button type="button" aria-label={playing ? "Pause" : "Play"} onClick={togglePlay} className={cn(button, "size-11")}>
              {playing ? <Pause className="size-6 fill-current" /> : <Play className="ml-0.5 size-6 fill-current" />}
            </button>
            <button type="button" aria-label="Back 10 seconds" title="Back 10 s (←)" onClick={() => seekBy(-10)} className={button}>
              <RotateCcw className="size-5" />
            </button>
            <button
              type="button"
              aria-label="Forward 10 seconds"
              title="Forward 10 s (→)"
              onClick={() => seekBy(10)}
              disabled={atLive}
              className={button}
            >
              <RotateCw className="size-5" />
            </button>
            <div className="group/vol hidden items-center sm:flex">
              <button type="button" aria-label={muted ? "Unmute" : "Mute"} onClick={() => setMuted((m) => !m)} className={button}>
                <VolumeIcon className="size-5" />
              </button>
              <div className="hidden w-0 overflow-hidden transition-[width] duration-200 group-hover/vol:w-24 group-focus-within/vol:w-24 sm:block">
                <Slider
                  label="Volume"
                  value={muted ? 0 : volume}
                  max={1}
                  onPreview={(v) => {
                    setVolume(v);
                    setMuted(v === 0);
                  }}
                  className="ml-1 w-20"
                />
              </div>
            </div>
            <button
              type="button"
              onClick={goLive}
              disabled={atLive}
              className={cn(
                "ml-1 flex h-7 shrink-0 items-center gap-1.5 rounded-full px-2.5 text-xs font-bold uppercase tracking-wide transition sm:ml-2",
                atLive ? "bg-danger/90 text-white" : "bg-white/15 text-white/85 hover:bg-white/25",
              )}
              title={atLive ? "Live" : "Go live"}
            >
              <span className={cn("size-1.5 rounded-full", atLive ? "bg-white animate-pulse" : "bg-white/60")} />
              {atLive ? "Live" : `−${formatBehind(behind)}`}
            </button>
            {on ? (
              <div className="ml-2 hidden min-w-0 truncate text-sm text-white/70 md:block">
                <span className="tabular-nums">{formatSpan(on)}</span>
              </div>
            ) : null}
            <div className="flex-1" />
            <button type="button" aria-label="Channel up" title="Channel up (↑)" onClick={() => onStep(1)} className={button}>
              <ChevronUp className="size-5" />
            </button>
            <button type="button" aria-label="Channel down" title="Channel down (↓)" onClick={() => onStep(-1)} className={button}>
              <ChevronDown className="size-5" />
            </button>
            {"pictureInPictureEnabled" in document && document.pictureInPictureEnabled ? (
              <button type="button" aria-label="Picture in picture" onClick={togglePip} className={cn(button, "hidden sm:flex")}>
                <PictureInPicture2 className="size-5" />
              </button>
            ) : null}
            <button type="button" aria-label={fullscreen ? "Exit full screen" : "Full screen"} title="Full screen (F)" onClick={toggleFullscreen} className={button}>
              {fullscreen ? <Minimize className="size-5" /> : <Maximize className="size-5" />}
            </button>
          </div>
        </div>
      </div>

      <ChannelPanel
        open={panel}
        category={category}
        current={channel.id}
        now={now}
        onClose={() => setPanel(false)}
        onTune={(c, inCategory) => {
          onTune(c, inCategory);
          setPanel(false);
        }}
      />
    </div>
  );
}

/** "1:23" behind live, or "12 min". */
function formatBehind(seconds: number): string {
  const s = Math.max(0, Math.round(seconds));
  if (s >= 600) return `${Math.round(s / 60)} min`;
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
}

function TroubleCard({
  trouble,
  channel,
  onRetry,
  onBack,
  onChannels,
}: {
  trouble: Trouble;
  channel?: Channel;
  onRetry?: () => void;
  onBack: () => void;
  onChannels?: () => void;
}) {
  const Icon = trouble.kind === "busy" ? Tv2 : TriangleAlert;
  return (
    <div className="w-full max-w-md rounded-2xl border border-white/10 bg-surface/95 p-6 text-center shadow-2xl animate-pop-in" role="alert">
      {channel ? (
        <ChannelLogo channel={channel} width={120} className="mx-auto mb-4 h-14 w-24 rounded-lg bg-white/[0.06]" />
      ) : (
        <Icon className={cn("mx-auto mb-4 size-10", trouble.kind === "busy" ? "text-accent" : "text-danger")} />
      )}
      <h2 className="text-lg font-semibold">{trouble.title}</h2>
      {trouble.message ? <p className="mt-2 text-sm text-muted [text-wrap:pretty]">{trouble.message}</p> : null}
      <div className="mt-6 flex flex-wrap justify-center gap-2">
        {onRetry ? (
          <button type="button" onClick={onRetry} className="flex h-10 items-center gap-2 rounded-md bg-accent px-4 text-sm font-medium text-accent-fg hover:bg-accent-hover">
            <RefreshCw className="size-4" /> Try again
          </button>
        ) : null}
        {onChannels ? (
          <button type="button" onClick={onChannels} className="flex h-10 items-center gap-2 rounded-md bg-surface-3 px-4 text-sm font-medium hover:bg-border">
            <List className="size-4" /> Other channels
          </button>
        ) : null}
        <button type="button" onClick={onBack} className="flex h-10 items-center rounded-md px-4 text-sm font-medium text-muted hover:bg-white/5 hover:text-fg">
          Back to the guide
        </button>
      </div>
    </div>
  );
}

