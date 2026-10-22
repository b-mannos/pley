import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { useQueryClient } from "@tanstack/react-query";
import type HlsType from "hls.js";
import type { ItemSummary } from "@/api/types";
import { errorMessage } from "@/api/client";
import { invalidateWatchState } from "@/api/hooks";
import { audioCaps, canPlayNativeHls } from "@/lib/caps";
import { loadHls } from "@/lib/hls";
import { getStored, writeStorage } from "@/lib/hooks";
import { reportProgress, startPlayback, stopSession } from "@/lib/progress";

export interface AudioPlayerApi {
  queue: ItemSummary[];
  index: number;
  current: ItemSummary | null;
  playing: boolean;
  loading: boolean;
  volume: number;
  muted: boolean;
  shuffle: boolean;
  error: string | null;
  playQueue: (tracks: ItemSummary[], startIndex?: number, opts?: { shuffle?: boolean }) => void;
  toggle: () => void;
  pause: () => void;
  next: () => void;
  prev: () => void;
  jumpTo: (index: number) => void;
  seek: (seconds: number) => void;
  setVolume: (v: number) => void;
  toggleMute: () => void;
  toggleShuffle: () => void;
  close: () => void;
}

export interface AudioTime {
  position: number;
  duration: number;
  buffered: number;
}

const AudioContext = createContext<AudioPlayerApi | null>(null);
const AudioTimeContext = createContext<AudioTime>({ position: 0, duration: 0, buffered: 0 });

const PROGRESS_INTERVAL = 10_000;

function shuffled<T>(arr: T[]): T[] {
  const a = arr.slice();
  for (let i = a.length - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * (i + 1));
    [a[i], a[j]] = [a[j], a[i]];
  }
  return a;
}

interface Session {
  sessionId: string;
  itemId: number;
  durationMs: number;
}

export function AudioPlayerProvider({ children }: { children: ReactNode }) {
  const qc = useQueryClient();
  const audioRef = useRef<HTMLAudioElement>(null);
  const hlsRef = useRef<HlsType | null>(null);
  const sessionRef = useRef<Session | null>(null);

  const [queue, setQueue] = useState<ItemSummary[]>([]);
  const [original, setOriginal] = useState<ItemSummary[]>([]);
  const [index, setIndex] = useState(0);
  // Bumped whenever playback of the current index must (re)start.
  const [playToken, setPlayToken] = useState(0);
  const [playing, setPlaying] = useState(false);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [shuffle, setShuffle] = useState(false);
  const [volume, setVolumeState] = useState(() => getStored("pley.audio.volume", 1));
  const [muted, setMuted] = useState(() => getStored("pley.audio.muted", false));
  const [time, setTime] = useState<AudioTime>({ position: 0, duration: 0, buffered: 0 });

  const queueRef = useRef(queue);
  queueRef.current = queue;
  const indexRef = useRef(index);
  indexRef.current = index;

  const current = queue[index] ?? null;

  const destroyHls = () => {
    hlsRef.current?.destroy();
    hlsRef.current = null;
  };

  /** Report "stopped" for the active session and tear it down. */
  const endSession = useCallback((keepalive = false) => {
    const s = sessionRef.current;
    const audio = audioRef.current;
    if (!s) return;
    sessionRef.current = null;
    const pos = audio ? Math.round(audio.currentTime * 1000) : 0;
    void reportProgress(
      s.itemId,
      { position_ms: pos, duration_ms: s.durationMs || undefined, state: "stopped", session_id: s.sessionId },
      keepalive,
    ).then(() => stopSession(s.sessionId, keepalive));
  }, []);

  const goTo = useCallback((i: number) => {
    setIndex(i);
    setPlayToken((t) => t + 1);
  }, []);

  // Start playback whenever the current track (or token) changes.
  useEffect(() => {
    const audio = audioRef.current;
    if (!audio || !current) return;
    const controller = new AbortController();
    let cancelled = false;

    endSession();
    destroyHls();
    audio.removeAttribute("src");
    audio.load();
    setLoading(true);
    setError(null);
    setTime({ position: 0, duration: (current.duration_ms ?? 0) / 1000, buffered: 0 });

    (async () => {
      try {
        const info = await startPlayback({ item_id: current.id, caps: audioCaps(), start_ms: 0 }, controller.signal);
        if (cancelled) {
          void stopSession(info.session_id);
          return;
        }
        sessionRef.current = { sessionId: info.session_id, itemId: current.id, durationMs: info.duration_ms };
        setTime((t) => ({ ...t, duration: info.duration_ms / 1000 || t.duration }));
        if (info.hls && !canPlayNativeHls(audio)) {
          const Hls = await loadHls();
          if (cancelled) return;
          if (!Hls.isSupported()) throw new Error("This browser can't play HLS streams");
          const hls = new Hls({ enableWorker: true, maxBufferLength: 60 });
          hlsRef.current = hls;
          hls.on(Hls.Events.ERROR, (_e, data) => {
            if (!data.fatal) return;
            if (data.type === Hls.ErrorTypes.NETWORK_ERROR) hls.startLoad();
            else if (data.type === Hls.ErrorTypes.MEDIA_ERROR) hls.recoverMediaError();
            else setError("Playback failed");
          });
          hls.loadSource(info.url);
          hls.attachMedia(audio);
        } else {
          audio.src = info.url;
        }
        await audio.play().catch(() => {
          // Autoplay may be blocked until the user interacts; state stays paused.
        });
      } catch (err) {
        if (cancelled || (err instanceof DOMException && err.name === "AbortError")) return;
        setError(errorMessage(err));
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();

    return () => {
      cancelled = true;
      controller.abort();
    };
    // `current` is captured by id on purpose: re-renders with the same track must not restart it.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [current?.id, playToken, endSession]);

  // Media element events.
  useEffect(() => {
    const audio = audioRef.current;
    if (!audio) return;
    const onPlay = () => setPlaying(true);
    const onPause = () => {
      setPlaying(false);
      const s = sessionRef.current;
      if (s && !audio.ended) {
        void reportProgress(s.itemId, {
          position_ms: Math.round(audio.currentTime * 1000),
          duration_ms: s.durationMs || undefined,
          state: "paused",
          session_id: s.sessionId,
        });
      }
    };
    const onTime = () => {
      let buffered = 0;
      const b = audio.buffered;
      if (b.length) buffered = b.end(b.length - 1);
      setTime((t) => ({
        position: audio.currentTime,
        duration: Number.isFinite(audio.duration) && audio.duration > 0 ? audio.duration : t.duration,
        buffered,
      }));
    };
    const onEnded = () => {
      const s = sessionRef.current;
      if (s) {
        sessionRef.current = null;
        const dur = s.durationMs || Math.round(audio.duration * 1000) || 0;
        void reportProgress(s.itemId, {
          position_ms: dur,
          duration_ms: dur || undefined,
          state: "stopped",
          session_id: s.sessionId,
        })
          .then(() => stopSession(s.sessionId))
          .then(() => invalidateWatchState(qc));
      }
      const i = indexRef.current;
      if (i + 1 < queueRef.current.length) goTo(i + 1);
      else setPlaying(false);
    };
    const onError = () => {
      if (audio.getAttribute("src") || hlsRef.current) setError("Playback failed");
    };
    audio.addEventListener("play", onPlay);
    audio.addEventListener("pause", onPause);
    audio.addEventListener("timeupdate", onTime);
    audio.addEventListener("progress", onTime);
    audio.addEventListener("durationchange", onTime);
    audio.addEventListener("ended", onEnded);
    audio.addEventListener("error", onError);
    return () => {
      audio.removeEventListener("play", onPlay);
      audio.removeEventListener("pause", onPause);
      audio.removeEventListener("timeupdate", onTime);
      audio.removeEventListener("progress", onTime);
      audio.removeEventListener("durationchange", onTime);
      audio.removeEventListener("ended", onEnded);
      audio.removeEventListener("error", onError);
    };
  }, [qc, goTo]);

  // Periodic progress while playing.
  useEffect(() => {
    if (!playing) return;
    const timer = window.setInterval(() => {
      const s = sessionRef.current;
      const audio = audioRef.current;
      if (!s || !audio) return;
      void reportProgress(s.itemId, {
        position_ms: Math.round(audio.currentTime * 1000),
        duration_ms: s.durationMs || undefined,
        state: "playing",
        session_id: s.sessionId,
      });
    }, PROGRESS_INTERVAL);
    return () => window.clearInterval(timer);
  }, [playing]);

  // Volume.
  useEffect(() => {
    const audio = audioRef.current;
    if (!audio) return;
    audio.volume = volume;
    audio.muted = muted;
  }, [volume, muted]);

  // Report "stopped" when the tab goes away.
  useEffect(() => {
    const onHide = () => endSession(true);
    window.addEventListener("pagehide", onHide);
    return () => window.removeEventListener("pagehide", onHide);
  }, [endSession]);

  const playQueue = useCallback(
    (tracks: ItemSummary[], startIndex = 0, opts?: { shuffle?: boolean }) => {
      if (tracks.length === 0) return;
      const start = Math.min(Math.max(startIndex, 0), tracks.length - 1);
      setOriginal(tracks);
      if (opts?.shuffle) {
        setQueue(shuffled(tracks));
        setShuffle(true);
        goTo(0);
      } else {
        setQueue(tracks);
        setShuffle(false);
        goTo(start);
      }
    },
    [goTo],
  );

  const toggle = useCallback(() => {
    const audio = audioRef.current;
    if (!audio || !queueRef.current.length) return;
    if (audio.paused) void audio.play().catch(() => undefined);
    else audio.pause();
  }, []);

  const pause = useCallback(() => {
    audioRef.current?.pause();
  }, []);

  const next = useCallback(() => {
    const i = indexRef.current;
    if (i + 1 < queueRef.current.length) goTo(i + 1);
  }, [goTo]);

  const prev = useCallback(() => {
    const audio = audioRef.current;
    const i = indexRef.current;
    if ((audio && audio.currentTime > 3) || i === 0) {
      if (audio) audio.currentTime = 0;
      return;
    }
    goTo(i - 1);
  }, [goTo]);

  const jumpTo = useCallback(
    (i: number) => {
      if (i < 0 || i >= queueRef.current.length) return;
      goTo(i);
    },
    [goTo],
  );

  const seek = useCallback((seconds: number) => {
    const audio = audioRef.current;
    if (!audio) return;
    const target = Math.max(0, seconds);
    audio.currentTime = target;
    setTime((t) => ({ ...t, position: target }));
  }, []);

  const setVolume = useCallback((v: number) => {
    const clamped = Math.max(0, Math.min(1, v));
    setVolumeState(clamped);
    writeStorage("pley.audio.volume", clamped);
    if (clamped > 0) {
      setMuted(false);
      writeStorage("pley.audio.muted", false);
    }
  }, []);

  const toggleMute = useCallback(() => {
    setMuted((m) => !m);
  }, []);

  useEffect(() => {
    writeStorage("pley.audio.muted", muted);
  }, [muted]);

  // Reordering the queue keeps the current track playing (no token bump).
  const toggleShuffle = useCallback(() => {
    const q = queueRef.current;
    const i = indexRef.current;
    const cur = q[i];
    if (!cur) return;
    if (!shuffle) {
      const rest = shuffled(q.filter((_, j) => j !== i));
      setQueue([cur, ...rest]);
      setIndex(0);
      setShuffle(true);
    } else {
      const base = original.length ? original : q;
      const j = base.findIndex((t) => t.id === cur.id);
      setQueue(base);
      setIndex(j >= 0 ? j : 0);
      setShuffle(false);
    }
  }, [original, shuffle]);

  const close = useCallback(() => {
    const audio = audioRef.current;
    endSession();
    destroyHls();
    if (audio) {
      audio.pause();
      audio.removeAttribute("src");
      audio.load();
    }
    setQueue([]);
    setOriginal([]);
    setIndex(0);
    setPlaying(false);
    setError(null);
    invalidateWatchState(qc);
  }, [endSession, qc]);

  const api = useMemo<AudioPlayerApi>(
    () => ({
      queue,
      index,
      current,
      playing,
      loading,
      volume,
      muted,
      shuffle,
      error,
      playQueue,
      toggle,
      pause,
      next,
      prev,
      jumpTo,
      seek,
      setVolume,
      toggleMute,
      toggleShuffle,
      close,
    }),
    [
      queue,
      index,
      current,
      playing,
      loading,
      volume,
      muted,
      shuffle,
      error,
      playQueue,
      toggle,
      pause,
      next,
      prev,
      jumpTo,
      seek,
      setVolume,
      toggleMute,
      toggleShuffle,
      close,
    ],
  );

  return (
    <AudioContext.Provider value={api}>
      <AudioTimeContext.Provider value={time}>
        {children}
        <audio ref={audioRef} preload="auto" className="hidden" />
      </AudioTimeContext.Provider>
    </AudioContext.Provider>
  );
}

export function useAudioPlayer(): AudioPlayerApi {
  const ctx = useContext(AudioContext);
  if (!ctx) throw new Error("useAudioPlayer must be used inside <AudioPlayerProvider>");
  return ctx;
}

/** Same as useAudioPlayer but returns null outside the provider. */
export function useOptionalAudioPlayer(): AudioPlayerApi | null {
  return useContext(AudioContext);
}

export function useAudioTime(): AudioTime {
  return useContext(AudioTimeContext);
}
