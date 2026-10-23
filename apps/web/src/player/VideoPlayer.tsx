import { useCallback, useEffect, useEffectEvent, useMemo, useRef, useState } from "react";
import { useNavigate } from "react-router";
import { useQueryClient } from "@tanstack/react-query";
import type HlsType from "hls.js";
import type { ErrorData } from "hls.js";
import {
  ArrowLeft,
  Captions,
  LoaderCircle,
  Maximize,
  Minimize,
  Pause,
  Play,
  RotateCcw,
  RotateCw,
  SkipForward,
  TriangleAlert,
  Volume1,
  Volume2,
  VolumeX,
} from "lucide-react";
import { errorMessage } from "@/api/client";
import { fetchNext, invalidateWatchState, useTrickplay } from "@/api/hooks";
import type { ItemDetail, ItemSummary, PlaybackInfo, PlaybackMode, PlaybackState } from "@/api/types";
import { useOptionalAudioPlayer } from "@/audio/AudioPlayer";
import { Slider } from "@/components/Slider";
import { useToast } from "@/components/Toast";
import { canPlayNativeHls, videoCaps } from "@/lib/caps";
import { cn } from "@/lib/cn";
import { episodeLabel, fileResolution, formatBitrate, formatClock, streamLabel } from "@/lib/format";
import { loadHls } from "@/lib/hls";
import { useDocumentTitle, useLocalStorage } from "@/lib/hooks";
import { endSession, reportProgress, saveStreamChoice, startPlayback, stopSession } from "@/lib/progress";
import { playUrl } from "@/lib/usePlayItem";
import { nextInQueue, queuePlayUrl, type QueuePosition } from "@/lib/videoQueue";
import { modeFor, qualitiesFor, qualityById } from "./quality";
import { ChaptersMenu } from "./ChaptersMenu";
import { SeekBar } from "./SeekBar";
import { METHOD_LABEL, SettingsPopover } from "./SettingsPopover";
import { fileMarkers, SkipButton, useSkipMarkers } from "./SkipButton";
import { UpNext } from "./UpNext";

const PROGRESS_INTERVAL = 10_000;
const HIDE_CONTROLS_AFTER = 3_000;

type WebkitVideo = HTMLVideoElement & { webkitEnterFullscreen?: () => void };

function rangesOf(tr: TimeRanges): [number, number][] {
  const out: [number, number][] = [];
  for (let i = 0; i < tr.length; i++) out.push([tr.start(i), tr.end(i)]);
  return out;
}

export interface VideoPlayerProps {
  item: ItemDetail;
  startMs: number;
  fileId: number | null;
  audio: number | null;
  sub: number | null;
  /** Playing through a playlist or collection: "next" is its next entry. */
  queue?: QueuePosition | null;
}

export function VideoPlayer({ item, startMs, fileId, audio: initialAudio, sub: initialSub, queue }: VideoPlayerProps) {
  const navigate = useNavigate();
  const qc = useQueryClient();
  const toast = useToast();
  const audioPlayer = useOptionalAudioPlayer();

  const containerRef = useRef<HTMLDivElement>(null);
  const videoRef = useRef<HTMLVideoElement>(null);
  const hlsRef = useRef<HlsType | null>(null);
  const sessionRef = useRef<PlaybackInfo | null>(null);
  const startMsRef = useRef(startMs);
  /** Last known playhead (s). Kept while a session (re)starts and the element sits at 0. */
  const positionRef = useRef(startMs / 1000);
  /** Mode used instead of the preference after direct play failed to decode (this player only). */
  const fallbackModeRef = useRef<PlaybackMode | null>(null);
  /** `network`/`media` count per session; `recreate` (new sessions) is reset once media loads again. */
  const recoveries = useRef({ network: 0, media: 0, recreate: 0 });
  /** POST /api/playback of the previous run, which ends its own session if that run was cancelled. */
  const pendingStartRef = useRef<Promise<void>>(Promise.resolve());

  const file = item.files.find((f) => f.id === fileId) ?? item.files[0] ?? null;
  const audioStreams = useMemo(() => file?.streams.filter((s) => s.kind === "audio") ?? [], [file]);
  const subStreams = useMemo(() => file?.streams.filter((s) => s.kind === "subtitle") ?? [], [file]);

  // Explicit tracks from the URL win; otherwise the remembered choice for the show / movie.
  const trickplay = useTrickplay(file?.id);
  const [audioIndex, setAudioIndex] = useState<number | null>(initialAudio ?? file?.selected_audio ?? null);
  const [subIndex, setSubIndex] = useState<number | null>(initialSub ?? file?.selected_subtitle ?? null);
  const [quality, setQuality] = useLocalStorage<string>("pley.quality", "original");
  // One menu: the original plays the best way the browser can, every other
  // quality is a transcode. A remembered quality this file doesn't need (an
  // 8 Mbps cap on a 5 Mbps file) plays the original.
  const qualities = useMemo(() => qualitiesFor(file), [file]);
  const selectedQuality = qualities.find((q) => q.id === quality) ?? qualityById("original");
  const mode = modeFor(selectedQuality);
  const [loadToken, setLoadToken] = useState(0);

  const selectedSub = subStreams.find((s) => s.index === subIndex) ?? null;
  const burnIndex = selectedSub && selectedSub.text_based === false ? selectedSub.index : null;
  const shownTextIndex = selectedSub && selectedSub.text_based !== false ? selectedSub.index : null;

  const [info, setInfo] = useState<PlaybackInfo | null>(null);
  const [fellBack, setFellBack] = useState(false);
  const [loading, setLoading] = useState(true);
  const [buffering, setBuffering] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [needsGesture, setNeedsGesture] = useState(false);
  const [playing, setPlaying] = useState(false);
  const [current, setCurrent] = useState(startMs / 1000);
  const [mediaDuration, setMediaDuration] = useState(0);
  const [buffered, setBuffered] = useState<[number, number][]>([]);
  const [volume, setVolume] = useLocalStorage<number>("pley.volume", 1);
  const [muted, setMuted] = useLocalStorage<boolean>("pley.muted", false);
  const [fullscreen, setFullscreen] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [chaptersOpen, setChaptersOpen] = useState(false);
  const [active, setActive] = useState(true);
  const [hoveringControls, setHoveringControls] = useState(false);
  const [ended, setEnded] = useState(false);
  const [upNext, setUpNext] = useState<ItemSummary | null>(null);
  const [autoSkip, setAutoSkip] = useLocalStorage<boolean>("pley.autoSkipIntro", false);

  const duration = (info?.duration_ms ?? item.duration_ms ?? 0) / 1000 || mediaDuration;
  const isEpisode = item.kind === "episode";
  // The next episode, or the queue's next entry.
  const hasNext = isEpisode || !!queue;
  const findNext = () => (queue ? nextInQueue(qc, queue) : fetchNext(qc, item.id));
  const nextUrl = (next: ItemSummary) => {
    const at = next.user_state.watched ? 0 : next.user_state.position_ms;
    return queue ? queuePlayUrl(queue.queue, queue.index + 1, next.id, at) : playUrl(next.id, { startMs: at });
  };
  const endUrl = queue?.queue.returnTo ?? `/item/${item.id}`;
  const title = isEpisode ? (item.grandparent_title ?? item.title) : item.year ? `${item.title} (${item.year})` : item.title;
  const subtitle = isEpisode ? [episodeLabel(item), item.title].filter(Boolean).join(" — ") : null;
  useDocumentTitle(isEpisode ? `${title} ${episodeLabel(item)}` : item.title);

  const destroyHls = () => {
    hlsRef.current?.destroy();
    hlsRef.current = null;
  };

  /** The playhead in seconds: the element's time once it shows the session's media, else the last known one. */
  const livePosition = () => {
    const v = videoRef.current;
    return v && sessionRef.current && v.readyState >= HTMLMediaElement.HAVE_CURRENT_DATA ? v.currentTime : positionRef.current;
  };
  const currentMs = () => Math.round(livePosition() * 1000);

  const restartAt = useCallback((seconds: number) => {
    startMsRef.current = Math.max(0, Math.round(seconds * 1000));
    setLoadToken((t) => t + 1);
  }, []);

  const report = useCallback(
    (state: PlaybackState, keepalive = false) => {
      const s = sessionRef.current;
      if (!s) return;
      void reportProgress(
        item.id,
        {
          position_ms: Math.round(positionRef.current * 1000),
          duration_ms: s.duration_ms || undefined,
          state,
          session_id: s.session_id,
        },
        keepalive,
      );
    },
    [item.id],
  );

  const goBack = useCallback(() => {
    const idx = (window.history.state as { idx?: number } | null)?.idx ?? 0;
    if (idx > 0) void navigate(-1);
    else void navigate(`/item/${item.id}`, { replace: true });
  }, [navigate, item.id]);

  // Pause any music when a video starts.
  useEffect(() => {
    audioPlayer?.pause();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // ---------------------------------------------------------------------
  // Session lifecycle: (re)start whenever the stream parameters change.
  // ---------------------------------------------------------------------
  useEffect(() => {
    const video = videoRef.current;
    if (!video || !file) return;
    let cancelled = false;
    const q = selectedQuality;
    const startSec = startMsRef.current / 1000;
    const requestMode = fallbackModeRef.current ?? mode;
    recoveries.current.network = 0;
    recoveries.current.media = 0;
    setLoading(true);
    setError(null);
    setNeedsGesture(false);
    setEnded(false);
    setBuffering(false);

    // Release the previous session before asking for the next one, so a
    // quality or track switch never trips the server's transcode limit.
    // Started right away (not in the async part) so it also happens when this
    // run is cancelled; it reports the last real position, not the reset element's 0.
    const prev = sessionRef.current;
    sessionRef.current = null;
    const released = prev ? endSession(item.id, prev, positionRef.current * 1000) : Promise.resolve();
    const previousStart = pendingStartRef.current;
    destroyHls();
    video.pause();
    video.removeAttribute("src");
    video.load();
    positionRef.current = startSec;
    setCurrent(startSec);

    const seekOnLoad = (seconds: number) => {
      if (seconds <= 0) return;
      const onMeta = () => {
        video.currentTime = seconds;
      };
      video.addEventListener("loadedmetadata", onMeta, { once: true });
    };

    // Most plays end up on hls.js: fetch it while the server prepares the stream.
    loadHls().catch(() => undefined);

    // The new session is recorded as soon as it exists, so the next run (or
    // unmount) ends it; a run cancelled before that ends it here instead.
    const acquired = (async (): Promise<PlaybackInfo | null> => {
      await Promise.all([released, previousStart]);
      if (cancelled) return null;
      const pinfo = await startPlayback({
        item_id: item.id,
        file_id: file.id,
        audio_index: audioIndex,
        subtitle_index: burnIndex,
        max_bitrate: q.maxBitrate,
        max_height: q.maxHeight,
        start_ms: Math.round(startSec * 1000),
        mode: requestMode,
        caps: videoCaps(),
      });
      if (cancelled) {
        await stopSession(pinfo.session_id);
        return null;
      }
      sessionRef.current = pinfo;
      return pinfo;
    })();
    pendingStartRef.current = acquired.then(
      () => undefined,
      () => undefined,
    );

    void (async () => {
      try {
        const pinfo = await acquired;
        if (!pinfo || cancelled) return;
        setInfo(pinfo);
        setFellBack(requestMode !== mode);

        if (pinfo.hls) {
          const hasMse = typeof window.MediaSource !== "undefined" || "ManagedMediaSource" in window;
          if (!hasMse && canPlayNativeHls(video)) {
            video.src = pinfo.url;
            seekOnLoad(startSec);
          } else {
            const Hls = await loadHls();
            if (cancelled) return;
            if (!Hls.isSupported()) {
              if (!canPlayNativeHls(video)) throw new Error("This browser can't play HLS streams.");
              video.src = pinfo.url;
              seekOnLoad(startSec);
            } else {
              const hls = new Hls({
                startPosition: startSec,
                maxBufferLength: 30,
                maxMaxBufferLength: 120,
                backBufferLength: 60,
                enableWorker: true,
                enableCEA708Captions: false,
                fragLoadingMaxRetry: 6,
                manifestLoadingMaxRetry: 2,
                levelLoadingMaxRetry: 2,
              });
              hlsRef.current = hls;
              hls.on(Hls.Events.ERROR, (_evt, data: ErrorData) => {
                if (!data.fatal || cancelled) return;
                const r = recoveries.current;
                if (data.type === Hls.ErrorTypes.NETWORK_ERROR) {
                  const code = data.response?.code;
                  const playlist =
                    data.details === Hls.ErrorDetails.MANIFEST_LOAD_ERROR ||
                    data.details === Hls.ErrorDetails.MANIFEST_LOAD_TIMEOUT ||
                    data.details === Hls.ErrorDetails.MANIFEST_PARSING_ERROR;
                  if (code === 404 || code === 410 || playlist) {
                    // The session expired (idle timeout, server restart) or its
                    // playlist can't be loaded: only a new session helps.
                    if (r.recreate < 3) {
                      r.recreate += 1;
                      restartAt(livePosition());
                      return;
                    }
                  } else if (r.network < 4) {
                    r.network += 1;
                    // Without a position hls.js would restart from the beginning.
                    hls.startLoad(livePosition());
                    return;
                  }
                } else if (data.type === Hls.ErrorTypes.MEDIA_ERROR) {
                  if (r.media < 2) {
                    r.media += 1;
                    hls.recoverMediaError();
                    return;
                  }
                }
                setError(`Playback failed (${data.details}).`);
              });
              hls.on(Hls.Events.FRAG_LOADED, () => {
                // Media flows again: the next incident (e.g. the session expiring
                // during a long pause) gets a fresh retry budget.
                recoveries.current.recreate = 0;
                recoveries.current.network = 0;
              });
              hls.loadSource(pinfo.url);
              hls.attachMedia(video);
            }
          }
        } else {
          video.src = pinfo.url;
          seekOnLoad(startSec);
        }

        try {
          await video.play();
        } catch (err) {
          if (!cancelled && err instanceof DOMException && err.name === "NotAllowedError") setNeedsGesture(true);
        }
      } catch (err) {
        if (cancelled) return;
        setError(errorMessage(err));
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();

    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [item.id, file?.id, audioIndex, burnIndex, selectedQuality.id, loadToken]);

  // Final teardown on unmount: report "stopped" and release the transcode.
  useEffect(() => {
    return () => {
      const s = sessionRef.current;
      sessionRef.current = null;
      hlsRef.current?.destroy();
      hlsRef.current = null;
      const done = s ? endSession(item.id, s, positionRef.current * 1000, true) : Promise.resolve();
      void done.then(() => invalidateWatchState(qc));
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Tab closed / navigated away: best-effort stop with keepalive requests (in
  // parallel: nothing can wait for a response while the page unloads).
  // Restored from the back/forward cache: start again where we were.
  useEffect(() => {
    const onHide = () => {
      const s = sessionRef.current;
      if (!s) return;
      sessionRef.current = null;
      void endSession(item.id, s, positionRef.current * 1000, true);
    };
    const onShow = (e: PageTransitionEvent) => {
      if (e.persisted && !sessionRef.current) restartAt(positionRef.current);
    };
    window.addEventListener("pagehide", onHide);
    window.addEventListener("pageshow", onShow);
    return () => {
      window.removeEventListener("pagehide", onHide);
      window.removeEventListener("pageshow", onShow);
    };
  }, [item.id, restartAt]);

  // ---------------------------------------------------------------------
  // Media element events.
  // ---------------------------------------------------------------------
  const onMediaEnded = useEffectEvent(() => {
    setEnded(true);
    setPlaying(false);
    const s = sessionRef.current;
    const durMs = s?.duration_ms || Math.round(duration * 1000);
    positionRef.current = durMs / 1000;
    const done = s
      ? reportProgress(item.id, { position_ms: durMs, duration_ms: durMs || undefined, state: "stopped", session_id: s.session_id })
      : Promise.resolve();
    void done.then(() => invalidateWatchState(qc));
    if (!hasNext) {
      void done.then(() => navigate(endUrl, { replace: true }));
      return;
    }
    findNext()
      .then((next) => {
        if (next) setUpNext(next);
        else void navigate(endUrl, { replace: true });
      })
      .catch(() => navigate(endUrl, { replace: true }));
  });

  const onMediaError = useEffectEvent(() => {
    const v = videoRef.current;
    if (!v || !v.error || hlsRef.current) return; // hls.js reports its own errors
    const s = sessionRef.current;
    if (!s) return; // the element was reset for the next session
    const code = v.error.code;
    const undecodable = code === MediaError.MEDIA_ERR_DECODE || code === MediaError.MEDIA_ERR_SRC_NOT_SUPPORTED;
    if (s.method === "direct" && undecodable && !fallbackModeRef.current) {
      // The browser claimed support but can't decode this file: transcode it instead.
      fallbackModeRef.current = "transcode";
      restartAt(livePosition());
      return;
    }
    if (code === MediaError.MEDIA_ERR_NETWORK && recoveries.current.recreate < 3) {
      // Server restart or connection drop: the same method will most likely work again.
      recoveries.current.recreate += 1;
      restartAt(livePosition());
      return;
    }
    setError(v.error.message || "This video can't be played in your browser.");
  });

  useEffect(() => {
    const v = videoRef.current;
    if (!v) return;
    const onPlay = () => {
      setPlaying(true);
      setNeedsGesture(false);
      setEnded(false);
    };
    const onPause = () => {
      setPlaying(false);
      if (!v.ended) report("paused");
    };
    // While a session (re)starts the element is reset to 0 before it seeks to
    // the start position: only trust its time once it shows the session's media.
    const showsMedia = () => !!sessionRef.current && v.readyState >= HTMLMediaElement.HAVE_CURRENT_DATA;
    const onTime = () => {
      if (!showsMedia()) return;
      positionRef.current = v.currentTime;
      setCurrent(v.currentTime);
    };
    const onProgress = () => setBuffered(rangesOf(v.buffered));
    const onDuration = () => setMediaDuration(Number.isFinite(v.duration) ? v.duration : 0);
    const onWaiting = () => setBuffering(true);
    const onPlaying = () => {
      setBuffering(false);
      // hls.js resets the budget per loaded fragment; direct play once it plays again.
      if (!hlsRef.current) recoveries.current.recreate = 0;
    };
    const onCanPlay = () => setBuffering(false);
    const onSeeked = () => {
      setBuffering(false);
      if (!sessionRef.current) return;
      positionRef.current = v.currentTime;
      report(v.paused ? "paused" : "playing");
    };
    const onEnded = () => onMediaEnded();
    const onError = () => onMediaError();

    v.addEventListener("play", onPlay);
    v.addEventListener("pause", onPause);
    v.addEventListener("timeupdate", onTime);
    v.addEventListener("progress", onProgress);
    v.addEventListener("durationchange", onDuration);
    v.addEventListener("waiting", onWaiting);
    v.addEventListener("playing", onPlaying);
    v.addEventListener("canplay", onCanPlay);
    v.addEventListener("seeked", onSeeked);
    v.addEventListener("ended", onEnded);
    v.addEventListener("error", onError);
    return () => {
      v.removeEventListener("play", onPlay);
      v.removeEventListener("pause", onPause);
      v.removeEventListener("timeupdate", onTime);
      v.removeEventListener("progress", onProgress);
      v.removeEventListener("durationchange", onDuration);
      v.removeEventListener("waiting", onWaiting);
      v.removeEventListener("playing", onPlaying);
      v.removeEventListener("canplay", onCanPlay);
      v.removeEventListener("seeked", onSeeked);
      v.removeEventListener("ended", onEnded);
      v.removeEventListener("error", onError);
    };
  }, [report]);

  // Periodic progress while playing.
  useEffect(() => {
    if (!playing) return;
    const timer = window.setInterval(() => report("playing"), PROGRESS_INTERVAL);
    return () => window.clearInterval(timer);
  }, [playing, report]);

  // Volume.
  useEffect(() => {
    const v = videoRef.current;
    if (!v) return;
    v.volume = Math.max(0, Math.min(1, volume));
    v.muted = muted;
  }, [volume, muted]);

  // Text subtitles rendered by the browser from <track> elements.
  const textTracks = useMemo(() => (info?.subtitles ?? []).filter((s) => s.text_based && s.url), [info]);
  useEffect(() => {
    const v = videoRef.current;
    if (!v) return;
    const apply = () => {
      for (const t of Array.from(v.textTracks)) {
        const want = shownTextIndex != null && t.id === `sub-${shownTextIndex}`;
        const mode: TextTrackMode = want ? "showing" : "disabled";
        if (t.mode !== mode) t.mode = mode;
      }
    };
    apply();
    v.textTracks.addEventListener("addtrack", apply);
    return () => v.textTracks.removeEventListener("addtrack", apply);
  }, [textTracks, shownTextIndex]);

  // Fullscreen state.
  useEffect(() => {
    const onChange = () => setFullscreen(!!document.fullscreenElement);
    document.addEventListener("fullscreenchange", onChange);
    return () => document.removeEventListener("fullscreenchange", onChange);
  }, []);

  // ---------------------------------------------------------------------
  // Controls.
  // ---------------------------------------------------------------------
  const hideTimer = useRef<number | undefined>(undefined);
  const poke = useCallback(() => {
    setActive(true);
    window.clearTimeout(hideTimer.current);
    hideTimer.current = window.setTimeout(() => setActive(false), HIDE_CONTROLS_AFTER);
  }, []);
  // Controls show at start and hide after a while (`active` starts true).
  useEffect(() => {
    hideTimer.current = window.setTimeout(() => setActive(false), HIDE_CONTROLS_AFTER);
    return () => window.clearTimeout(hideTimer.current);
  }, []);

  const showControls =
    active || hoveringControls || !playing || settingsOpen || chaptersOpen || loading || !!error || needsGesture || ended;

  const togglePlay = () => {
    const v = videoRef.current;
    if (!v || error) return;
    if (v.paused || v.ended) void v.play().catch(() => setNeedsGesture(true));
    else v.pause();
  };

  const seekTo = (seconds: number) => {
    const v = videoRef.current;
    if (!v) return;
    const max = duration > 0 ? Math.max(0, duration - 0.25) : Number.POSITIVE_INFINITY;
    const t = Math.max(0, Math.min(seconds, max));
    v.currentTime = t;
    positionRef.current = t;
    setCurrent(t);
    if (ended) setEnded(false);
    if (upNext) setUpNext(null);
  };
  const seekBy = (delta: number) => seekTo((videoRef.current?.currentTime ?? current) + delta);

  const changeVolume = (delta: number) => {
    const next = Math.max(0, Math.min(1, (muted ? 0 : volume) + delta));
    setVolume(next);
    setMuted(next === 0);
  };
  const toggleMute = () => {
    if (muted && volume === 0) setVolume(0.5);
    setMuted(!muted);
  };

  const toggleFullscreen = () => {
    const el = containerRef.current;
    if (!el) return;
    if (document.fullscreenElement) void document.exitFullscreen().catch(() => undefined);
    else if (el.requestFullscreen) void el.requestFullscreen().catch(() => undefined);
    else {
      // iOS Safari only has fullscreen on the video element itself.
      const video: WebkitVideo | null = videoRef.current;
      video?.webkitEnterFullscreen?.();
    }
  };

  /** Store the choice so the next episode (or the next viewing) starts with the same tracks. */
  const rememberTracks = (audio: number | null, sub: number | null) => {
    void saveStreamChoice(item.id, { file_id: file?.id ?? null, audio_index: audio, subtitle_index: sub }).then(() =>
      qc.invalidateQueries({ queryKey: ["item"], predicate: (q) => q.queryKey[1] !== item.id }),
    );
  };
  const changeAudio = (index: number) => {
    if (index === (audioIndex ?? audioStreams.find((s) => s.default)?.index ?? audioStreams[0]?.index)) return;
    startMsRef.current = currentMs();
    setAudioIndex(index);
    rememberTracks(index, subIndex);
  };
  const changeQuality = (id: string) => {
    if (id === selectedQuality.id) return;
    startMsRef.current = currentMs();
    fallbackModeRef.current = null;
    setQuality(id);
  };
  const changeSubtitle = (index: number | null) => {
    const s = subStreams.find((x) => x.index === index);
    const nextBurn = s && s.text_based === false ? s.index : null;
    if (nextBurn !== burnIndex) startMsRef.current = currentMs();
    setSubIndex(index);
    rememberTracks(audioIndex, index);
  };
  const cycleSubtitles = () => {
    const texts = subStreams.filter((s) => s.text_based !== false);
    if (!texts.length) return;
    const pos = texts.findIndex((s) => s.index === shownTextIndex);
    const next = pos + 1 < texts.length ? texts[pos + 1] : null;
    changeSubtitle(next ? next.index : null);
    toast.info(next ? `Subtitles: ${streamLabel(next)}` : "Subtitles off");
  };

  const playNext = async () => {
    try {
      const next = upNext ?? (await findNext());
      if (!next) {
        toast.info(queue ? "This is the last one" : "This is the last episode");
        return;
      }
      void navigate(nextUrl(next), { replace: true });
    } catch (err) {
      toast.error(errorMessage(err));
    }
  };

  // Skip Intro / Skip Credits; an episode's credits bring up Up Next.
  const markers = useMemo(() => fileMarkers(item, info?.file_id ?? file?.id), [item, info?.file_id, file?.id]);
  const marker = useSkipMarkers({
    markers,
    seconds: current,
    enabled: !!info && !loading && !error && !ended,
    autoSkip,
    onAutoSkip: (m) => {
      seekTo(m.end_ms / 1000);
      toast.info("Skipped intro");
    },
    onCredits: () => {
      if (!hasNext || upNext) return;
      findNext()
        .then((next) => {
          if (next) setUpNext(next);
        })
        .catch(() => undefined);
    },
  });
  const skipMarker = () => {
    if (marker) seekTo(Math.min(marker.end_ms / 1000, duration));
  };

  const retry = () => {
    setError(null);
    recoveries.current.recreate = 0;
    // Without a session (its start failed) the element knows nothing: use the intended start.
    restartAt(sessionRef.current ? livePosition() : startMsRef.current / 1000);
  };

  // Keyboard shortcuts.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.defaultPrevented || e.metaKey || e.ctrlKey || e.altKey) return;
      const target = e.target as HTMLElement | null;
      const tag = target?.tagName;
      if (tag === "INPUT" || tag === "SELECT" || tag === "TEXTAREA") return;
      switch (e.key) {
        case " ":
          if (tag === "BUTTON") return; // let the focused button handle it
          e.preventDefault();
          togglePlay();
          break;
        case "k":
        case "K":
          togglePlay();
          break;
        case "ArrowLeft":
        case "j":
        case "J":
          e.preventDefault();
          seekBy(-10);
          break;
        case "ArrowRight":
        case "l":
        case "L":
          e.preventDefault();
          seekBy(10);
          break;
        case "ArrowUp":
          e.preventDefault();
          changeVolume(0.05);
          break;
        case "ArrowDown":
          e.preventDefault();
          changeVolume(-0.05);
          break;
        case "m":
        case "M":
          toggleMute();
          break;
        case "f":
        case "F":
          toggleFullscreen();
          break;
        case "c":
        case "C":
          cycleSubtitles();
          break;
        case "s":
        case "S":
          if (!marker) return;
          skipMarker();
          break;
        case "Escape":
          if (settingsOpen) return;
          if (!document.fullscreenElement) goBack();
          break;
        default:
          return;
      }
      poke();
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  });

  // Surface clicks: mouse click toggles play (double-click = fullscreen); taps toggle the controls.
  const clickTimer = useRef<number | undefined>(undefined);
  const pointerType = useRef("mouse");
  const onSurfaceClick = () => {
    if (pointerType.current !== "mouse") {
      if (active && playing) setActive(false);
      else poke();
      return;
    }
    window.clearTimeout(clickTimer.current);
    clickTimer.current = window.setTimeout(togglePlay, 220);
    poke();
  };
  const onSurfaceDoubleClick = () => {
    window.clearTimeout(clickTimer.current);
    toggleFullscreen();
  };

  const VolumeIcon = muted || volume === 0 ? VolumeX : volume < 0.5 ? Volume1 : Volume2;
  // After a failed direct play the server only knows that a transcode was requested.
  const methodReason = fellBack ? "This browser couldn't play the original file" : (info?.reason ?? null);
  const subtitleLabel = (s: PlaybackInfo["subtitles"][number]) =>
    streamLabel({ index: s.index, kind: "subtitle", codec: s.codec, language: s.language, title: s.title, default: s.default, forced: s.forced, external: s.external, text_based: s.text_based });

  return (
    <div
      ref={containerRef}
      className={cn("fixed inset-0 z-50 select-none overflow-hidden bg-black text-white", !showControls && "cursor-none")}
      onPointerMove={(e) => {
        if (e.pointerType === "mouse") poke();
      }}
    >
      <video
        ref={videoRef}
        className="absolute inset-0 size-full bg-black object-contain"
        playsInline
        preload="auto"
        onPointerDown={(e) => {
          pointerType.current = e.pointerType;
        }}
        onClick={onSurfaceClick}
        onDoubleClick={onSurfaceDoubleClick}
      >
        {info
          ? textTracks.map((s) => (
              <track
                key={`${info.session_id}-${s.index}`}
                id={`sub-${s.index}`}
                kind="subtitles"
                src={s.url ?? undefined}
                srcLang={s.language ?? undefined}
                label={subtitleLabel(s)}
              />
            ))
          : null}
      </video>

      {(loading || buffering) && !error && !needsGesture ? (
        <div className="pointer-events-none absolute inset-0 flex items-center justify-center">
          <LoaderCircle className="size-14 animate-spin text-white/80" />
        </div>
      ) : null}

      {needsGesture && !error ? (
        <button
          type="button"
          aria-label="Play"
          onClick={togglePlay}
          className="absolute left-1/2 top-1/2 z-20 flex size-20 -translate-x-1/2 -translate-y-1/2 items-center justify-center rounded-full bg-accent text-accent-fg shadow-2xl transition hover:scale-105"
        >
          <Play className="ml-1 size-9 fill-current" />
        </button>
      ) : null}

      {error ? (
        <div className="absolute inset-0 z-30 flex flex-col items-center justify-center gap-4 bg-black/85 p-6 text-center">
          <TriangleAlert className="size-12 text-accent" />
          <div className="max-w-lg">
            <div className="text-lg font-semibold">Playback error</div>
            <p className="mt-1 text-sm text-white/70">{error}</p>
          </div>
          <div className="flex gap-2">
            <button type="button" onClick={retry} className="rounded-md bg-accent px-5 py-2 text-sm font-semibold text-accent-fg hover:bg-accent-hover">
              Retry
            </button>
            <button type="button" onClick={goBack} className="rounded-md bg-white/10 px-5 py-2 text-sm font-semibold hover:bg-white/20">
              Back
            </button>
          </div>
        </div>
      ) : null}

      {/* Top bar */}
      <div
        className={cn(
          "absolute inset-x-0 top-0 z-10 flex items-start gap-3 bg-linear-to-b from-black/80 via-black/40 to-transparent px-3 pb-16 pt-3 transition-opacity duration-300 sm:px-6 sm:pt-5",
          showControls ? "opacity-100" : "pointer-events-none opacity-0",
        )}
        onMouseEnter={() => setHoveringControls(true)}
        onMouseLeave={() => setHoveringControls(false)}
      >
        <button
          type="button"
          aria-label="Back"
          onClick={goBack}
          className="flex size-10 shrink-0 items-center justify-center rounded-full transition hover:bg-white/15"
        >
          <ArrowLeft className="size-6" />
        </button>
        <div className="min-w-0 pt-1">
          <div className="truncate text-lg font-semibold drop-shadow sm:text-xl">{title}</div>
          {subtitle ? <div className="truncate text-sm text-white/75">{subtitle}</div> : null}
        </div>
      </div>

      {/* Bottom bar */}
      <div
        className={cn(
          "absolute inset-x-0 bottom-0 z-10 bg-linear-to-t from-black/90 via-black/55 to-transparent px-3 pb-3 pt-20 transition-opacity duration-300 sm:px-6 sm:pb-5",
          showControls ? "opacity-100" : "pointer-events-none opacity-0",
        )}
        onMouseEnter={() => setHoveringControls(true)}
        onMouseLeave={() => setHoveringControls(false)}
      >
        <SeekBar
          current={current}
          duration={duration}
          buffered={buffered}
          onSeek={seekTo}
          previews={file && trickplay.data ? { fileId: file.id, trickplay: trickplay.data } : null}
          chapters={file?.chapters ?? []}
        />
        <div className="mt-1 flex items-center gap-0.5 sm:gap-1.5">
          <button
            type="button"
            aria-label={playing ? "Pause" : "Play"}
            onClick={togglePlay}
            className="flex size-11 items-center justify-center rounded-full transition hover:bg-white/15"
          >
            {playing ? <Pause className="size-6 fill-current" /> : <Play className="ml-0.5 size-6 fill-current" />}
          </button>
          <button
            type="button"
            aria-label="Back 10 seconds"
            title="Back 10 s (←)"
            onClick={() => seekBy(-10)}
            className="flex size-10 items-center justify-center rounded-full transition hover:bg-white/15"
          >
            <RotateCcw className="size-5" />
          </button>
          <button
            type="button"
            aria-label="Forward 30 seconds"
            title="Forward 30 s"
            onClick={() => seekBy(30)}
            className="flex size-10 items-center justify-center rounded-full transition hover:bg-white/15"
          >
            <RotateCw className="size-5" />
          </button>
          <div className="group/vol flex items-center">
            <button
              type="button"
              aria-label={muted ? "Unmute" : "Mute"}
              onClick={toggleMute}
              className="flex size-10 items-center justify-center rounded-full transition hover:bg-white/15"
            >
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
          <div className="ml-2 text-sm tabular-nums text-white/85">
            {formatClock(current)} <span className="text-white/45">/ {formatClock(duration)}</span>
          </div>

          <div className="flex-1" />

          {info ? (
            <span
              title={methodReason ?? undefined}
              className={cn(
                "mr-1 hidden rounded px-2 py-0.5 text-xs font-semibold md:inline",
                info.method === "direct" ? "bg-success/20 text-success" : info.method === "remux" ? "bg-info/20 text-info" : "bg-accent/20 text-accent",
              )}
            >
              {METHOD_LABEL[info.method]}
            </span>
          ) : null}
          {hasNext ? (
            <button
              type="button"
              aria-label={queue ? "Next" : "Next episode"}
              title={queue ? "Next" : "Next episode"}
              onClick={() => void playNext()}
              className="flex size-10 items-center justify-center rounded-full transition hover:bg-white/15"
            >
              <SkipForward className="size-5 fill-current" />
            </button>
          ) : null}
          {subStreams.some((s) => s.text_based !== false) ? (
            <button
              type="button"
              aria-label="Toggle subtitles (c)"
              title="Subtitles (c)"
              onClick={cycleSubtitles}
              className={cn(
                "flex size-10 items-center justify-center rounded-full transition hover:bg-white/15",
                shownTextIndex != null && "text-accent",
              )}
            >
              <Captions className="size-5" />
            </button>
          ) : null}
          {file && (file.chapters?.length ?? 0) > 1 ? (
            <ChaptersMenu
              chapters={file.chapters}
              current={current}
              open={chaptersOpen}
              setOpen={setChaptersOpen}
              onSeek={seekTo}
            />
          ) : null}
          <SettingsPopover
            open={settingsOpen}
            setOpen={setSettingsOpen}
            info={info}
            reason={methodReason}
            qualities={qualities}
            quality={selectedQuality.id}
            originalHint={[fileResolution(file), formatBitrate(file?.bitrate)].filter(Boolean).join(" · ") || null}
            onQuality={changeQuality}
            audioStreams={audioStreams}
            audioIndex={audioIndex ?? info?.audio_index ?? null}
            onAudio={changeAudio}
            subtitleStreams={subStreams}
            subtitleIndex={subIndex}
            onSubtitle={changeSubtitle}
            autoSkip={autoSkip}
            onAutoSkip={setAutoSkip}
          />
          <button
            type="button"
            aria-label={fullscreen ? "Exit fullscreen" : "Fullscreen"}
            title="Fullscreen (f)"
            onClick={toggleFullscreen}
            className="flex size-10 items-center justify-center rounded-full transition hover:bg-white/15"
          >
            {fullscreen ? <Minimize className="size-5" /> : <Maximize className="size-5" />}
          </button>
        </div>
      </div>

      {marker && !upNext && !ended ? <SkipButton marker={marker} onSkip={skipMarker} lifted={active || !playing} /> : null}

      {upNext ? (
        <UpNext
          next={upNext}
          onPlay={() => void playNext()}
          onCancel={() => {
            setUpNext(null);
          }}
        />
      ) : null}
    </div>
  );
}
