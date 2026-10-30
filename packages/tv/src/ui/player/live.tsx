import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "preact/hooks";
import type { LiveChannel } from "../../api/types";
import { behindText, channelByNumber, formatSpan, formatTime, LIVE_WITHIN_MS, nowAndNext, programProgress, stepChannel, timeLeft } from "../../live/data";
import { LiveController, type LiveState } from "../../live/session";
import { initMusic } from "../../player/music";
import type { Key } from "../keys";
import { useUi } from "../context";
import { Pill, TextButton } from "../kit/buttons";
import { F, G } from "../kit/focusable";
import { Icon } from "../kit/icons";
import { useTrap } from "../kit/panel";
import { Toast } from "../kit/toast";
import { failedTitle, problem, sentence, useApp, useLoad } from "../screens/common";
import { ChannelLogo, NumberBox, ProgramMarks, ProgressLine, useNow, useNumberEntry } from "../screens/live-kit";
import { PlayerButton, TimeOfDay } from "./player";

/** Controls hide this long after the last key while a channel plays. */
const HIDE_MS = 5_000;
/** What's on shows this long after a channel starts. */
const BANNER_MS = 5_000;
/** The ring shows after this long of waiting. */
const RING_MS = 500;
/** Left and Right move this far in the timeshift; held, three times as far. */
const STEP_MS = 10_000;
/** The scrubber's line, in canvas px (the title player's). */
const BAR_LEFT = 48;
const BAR_WIDTH = 864;

/** Keys the player takes whatever has focus. */
const MEDIA_KEYS: Key[] = ["playPause", "play", "pause", "stop", "fastForward", "rewind", "next", "previous", "channelUp", "channelDown", "channelList", "guide", "info"];

/**
 * A live channel playing (the web's LivePlayerPage on a TV): the TV's own
 * player under the page, what's on, the Live badge and the timeshift, the
 * channel list, and changing channels with CH+ and CH−, Up and Down, or
 * the channel's number. Back leaves; the server is told at once.
 */
export function LivePlayerScreen(p: { channelId: number; pressedAt: number }) {
  const app = useApp();
  const ui = useUi();
  const list = useLoad((c) => ui.api.liveChannels(c), [app.liveChanges.value], { initial: app.liveChannels });
  useEffect(() => {
    if (list.data) app.liveChannels = list.data;
  }, [list.data]);
  const channels = list.data;
  const first = useRef<LiveChannel | null>(null);
  if (!first.current && channels) first.current = channels.filter((c) => c.id === p.channelId)[0] ?? null;
  if (!channels) {
    if (list.error) {
      return <Blackout title={failedTitle(list.error, "Live TV didn't load")} message={problem(list.error)} onRetry={list.reload} />;
    }
    return (
      <div class="player live-player">
        <div class="player-ring" role="status" aria-label="Loading">
          <div />
        </div>
      </div>
    );
  }
  if (!first.current) return <Blackout title="This channel isn't here" message="It may have been hidden, or its source removed." />;
  return <LivePlayerView first={first.current} channels={channels} pressedAt={p.pressedAt} />;
}

/** A message on black with Back (and Try again): the player had nothing to play. */
function Blackout(p: { title: string; message: string; onRetry?: () => void }) {
  const ui = useUi();
  useEffect(
    () =>
      ui.input.push((key, e) => {
        if (key !== "back") return false;
        if (e.type === "down") ui.nav.pop();
        return true;
      }),
    [],
  );
  return (
    <div class="player live-player">
      <div class="player-error">
        <div class="t-page c-fg" style="text-align:center">
          {p.title}
        </div>
        <div class="t-body c-muted lines-4" style="margin-top:12px;text-align:center">
          {sentence(p.message)}
        </div>
        <G kind="row" class="buttons" style="margin-top:32px;justify-content:center">
          {p.onRetry && <Pill label="Try again" icon="refresh-cw" fid="retry" initial onSelect={p.onRetry} />}
          <TextButton label="Back" fid="back" initial={!p.onRetry} onSelect={() => ui.nav.pop()} />
        </G>
      </div>
    </div>
  );
}

function LivePlayerView(p: { first: LiveChannel; channels: LiveChannel[]; pressedAt: number }) {
  const app = useApp();
  const ui = useUi();
  const host = useRef<HTMLDivElement>(null);
  const controller = useMemo(
    () =>
      new LiveController(p.first, {
        server: ui.api,
        engine: () => app.platform.createEngine(host.current!),
        clientId: app.installId(),
      }),
    [],
  );
  const c = controller;
  const s = c.state.value;
  const clock = c.clock.value;
  const channel = s.channel;
  const channels = p.channels.map((x) => (x.id === channel.id ? { ...x, ...channel } : x));
  const now = useNow(5_000);
  const hours24 = app.platform.clock24();
  const [controls, setControls] = useState(false);
  const [list, setList] = useState(false);
  const [banner, setBanner] = useState(true);
  const [ring, setRing] = useState(false);
  const [toast, setToast] = useState<{ text: string; n: number; error: boolean } | null>(null);
  const [missing, setMissing] = useState<string | null>(null);
  const lastInput = useRef(Date.now());
  const say = (text: string, error = false) => setToast({ text, n: Date.now(), error });

  useLayoutEffect(() => {
    initMusic(app).yieldToVideo();
    document.documentElement.classList.add("playing");
    c.tune(p.first);
    return () => {
      c.close();
      document.documentElement.classList.remove("playing");
    };
  }, []);

  // The TV hides the app (Home, another app): watching ends on the server, and starts again from live when it shows.
  useEffect(() => {
    let away = false;
    return app.visible.subscribe((visible) => {
      if (!visible && !away) {
        away = true;
        c.suspend();
      } else if (visible && away) {
        away = false;
        c.resume();
      }
    });
  }, []);

  const playing = s.phase === "playing" || s.phase === "buffering" || s.phase === "tuning";
  useEffect(() => {
    app.platform.keepAwake(playing);
  }, [playing]);
  useEffect(() => () => app.platform.keepAwake(false), []);

  // The time to the first picture, for measuring against the other apps.
  useEffect(() => {
    if (s.shown && s.tunes === 1) console.info(`pley: live picture ${Date.now() - p.pressedAt} ms after the press`);
  }, [s.shown]);

  // What's on shows as each channel starts, for 5 s once it plays.
  useEffect(() => setBanner(true), [s.tunes]);
  useEffect(() => {
    if (!banner || s.phase !== "playing") return;
    const t = setTimeout(() => setBanner(false), BANNER_MS);
    return () => clearTimeout(t);
  }, [banner, s.phase, s.tunes]);

  // The ring after half a second of waiting, once the channel's own card has gone.
  const waiting = s.phase === "buffering" || (s.phase === "tuning" && s.shown);
  useEffect(() => {
    if (!waiting) {
      setRing(false);
      return;
    }
    const t = setTimeout(() => setRing(true), RING_MS);
    return () => clearTimeout(t);
  }, [waiting]);

  // Controls hide 5 s after the last key while it plays.
  useEffect(() => {
    if (!controls || list || s.phase !== "playing") return;
    const t = setInterval(() => {
      if (Date.now() - lastInput.current >= HIDE_MS) setControls(false);
    }, 250);
    return () => clearInterval(t);
  }, [controls, list, s.phase]);

  useEffect(() => {
    if (!toast) return;
    const t = setTimeout(() => setToast(null), toast.error ? 6_000 : 3_500);
    return () => clearTimeout(t);
  }, [toast?.n]);
  useEffect(() => {
    if (!missing) return;
    const t = setTimeout(() => setMissing(null), 2_000);
    return () => clearTimeout(t);
  }, [missing]);

  const leave = () => ui.nav.pop();
  const focusFid = (fid: string) =>
    setTimeout(() => {
      const el = document.querySelector<HTMLElement>(`.live-player [data-fid="${fid}"]`);
      if (el) ui.focus.focus(el);
    }, 0);
  const show = (fid: string) => {
    setControls(true);
    focusFid(fid);
  };
  const tune = (to: LiveChannel | null) => {
    if (!to) return;
    if (to.id !== channel.id || s.phase === "error") c.tune(to);
    else setBanner(true);
  };
  const zap = (by: number) => tune(stepChannel(channels, channel.id, by));
  const seek = (ms: number) => {
    if (!c.canSeek) return;
    c.seekBy(ms);
    if (!controls) show("scrubber");
  };
  const toggleFavorite = () => {
    const on = !channel.favorite;
    ui.api.setLiveFavorite(channel.id, on).then(
      () => {
        app.liveChannels = (app.liveChannels ?? []).map((x) => (x.id === channel.id ? { ...x, favorite: on } : x));
        c.state.value = { ...c.state.value, channel: { ...c.state.value.channel, favorite: on } };
        say(on ? `${channel.name} is a favourite` : `${channel.name} isn't a favourite any more`);
      },
      () => say(on ? "Couldn't add it to your favourites" : "Couldn't take it out of your favourites", true),
    );
  };

  const digits = useNumberEntry((d) => {
    const found = channelByNumber(channels, d);
    if (found) tune(found);
    else setMissing(d);
  }, !list);

  // The remote, in the title player's order: media keys, an error, the channel list, then the controls.
  const keys = (key: Key, e: { repeat: boolean; type: "down" | "up" }): boolean => {
    lastInput.current = Date.now();
    const down = e.type === "down" && !e.repeat;
    if (MEDIA_KEYS.indexOf(key) >= 0) {
      if (e.type !== "down") return true;
      switch (key) {
        case "playPause":
          if (down) c.togglePause();
          break;
        case "play":
          c.play();
          break;
        case "pause":
          c.pause();
          if (!controls) show("playpause");
          break;
        case "stop":
        case "guide":
          if (down) leave();
          break;
        case "fastForward":
          seek(e.repeat ? 3 * STEP_MS : STEP_MS);
          break;
        case "rewind":
          seek(e.repeat ? -3 * STEP_MS : -STEP_MS);
          break;
        case "channelUp":
        case "next":
          if (!list) zap(1);
          break;
        case "channelDown":
        case "previous":
          if (!list) zap(-1);
          break;
        case "channelList":
          if (down) setList((l) => !l);
          break;
        case "info":
          if (down) setBanner((b) => !b);
          break;
      }
      return true;
    }
    const st = c.state.value;
    if (list) return false;
    if (st.phase === "error") {
      if (key === "back") {
        if (down) leave();
        return true;
      }
      // Another channel from here: Up and Down as CH+ and CH−, the menu key the list.
      if (key === "up" || key === "down") {
        if (e.type === "down") zap(key === "up" ? 1 : -1);
        return true;
      }
      if (key === "menu") {
        if (e.type === "up") setList(true);
        return true;
      }
      return false;
    }
    const focused = ui.focus.current?.getAttribute("data-fid") ?? null;
    if (!controls) {
      if (key === "ok") {
        if (down) show("playpause");
        return true;
      }
      if (key === "up" || key === "down") {
        if (e.type === "down") zap(key === "up" ? 1 : -1);
        return true;
      }
      if (key === "left" || key === "right") {
        if (e.type === "down") seek((key === "right" ? 1 : -1) * (e.repeat ? 3 * STEP_MS : STEP_MS));
        return true;
      }
      if (key === "menu") {
        if (e.type === "up") setList(true);
        return true;
      }
      if (key === "back") {
        if (down) leave();
        return true;
      }
      return false;
    }
    if (key === "back") {
      if (down) {
        setControls(false);
        focusFid("video");
      }
      return true;
    }
    if (focused === "scrubber") {
      if (key === "left" || key === "right") {
        if (e.type === "down") seek((key === "right" ? 1 : -1) * (e.repeat ? 3 * STEP_MS : STEP_MS));
        return true;
      }
      if (key === "ok") {
        if (down) c.togglePause();
        return true;
      }
      if (key === "down") {
        if (e.type === "down") focusFid("playpause");
        return true;
      }
      if (key === "up") return true;
    }
    return false;
  };
  const keysRef = useRef(keys);
  keysRef.current = keys;
  useEffect(() => ui.input.push((key, e) => keysRef.current(key, e)), []);

  // The pointer (the Magic Remote) moving shows the controls and keeps them up, as a key does.
  useEffect(() => {
    const moved = () => {
      if (!ui.input.pointerMoved) return;
      lastInput.current = Date.now();
      if (!controlsRef.current && !listRef.current && c.state.value.phase !== "error") show("playpause");
    };
    document.addEventListener("mousemove", moved);
    return () => document.removeEventListener("mousemove", moved);
  }, []);
  const controlsRef = useRef(controls);
  controlsRef.current = controls;
  const listRef = useRef(list);
  listRef.current = list;

  // Hidden controls leave focus on the video's holder; an error, on its first button.
  useEffect(() => {
    if (controls || list) return;
    focusFid(s.phase === "error" ? (s.trouble?.canRetry ? "retry" : "back") : "video");
  }, [controls, list, s.phase === "error", s.trouble]);
  // An error closes the controls.
  useEffect(() => {
    if (s.phase === "error") setControls(false);
  }, [s.phase === "error"]);

  const { on, next } = nowAndNext(channel, now);
  const atLive = !clock.window || clock.behindMs < LIVE_WITHIN_MS;
  const trouble = s.phase === "error" ? s.trouble : null;
  const infoShown = (controls || banner) && !trouble && !(s.phase === "tuning" && !s.shown);
  return (
    <div class="player live-player">
      <div
        ref={(el) => {
          host.current = el;
        }}
        class="engine-host"
      />
      <div class={`player-scrim${controls || infoShown ? " in" : ""}`} />
      <F class="video-holder" fid="video" disabled={controls} label={`${channel.name}, live`} />
      {s.phase === "tuning" && !s.shown && <TuningCard channel={channel} on={on} />}
      {s.shown && s.audioOnly && !trouble && <RadioCard channel={channel} on={on} hours24={hours24} />}
      <div class={`live-info${infoShown ? " in" : ""}`} aria-hidden={!infoShown}>
        <NowCard channel={channel} on={on} next={next} now={now} hours24={hours24} />
      </div>
      <div class={`player-controls${controls ? " in" : ""}`} aria-hidden={!controls}>
        <div class="player-top">
          <div style="flex:1" />
          <div class="player-clock">
            <div class="player-clock-row">
              <LiveBadge atLive={atLive} behindMs={clock.behindMs} />
              <TimeOfDay hours24={hours24} />
            </div>
          </div>
        </div>
        <LiveBar state={s} clock={clock} atLive={atLive} onSeek={(ms) => c.seekTo(ms)} />
        <G kind="row" class="player-row">
          <div class="row-left">
            <PlayerButton fid="channels" icon="list" label="Channels" onSelect={() => setList(true)} />
            <PlayerButton fid="favorite" icon={channel.favorite ? "star" : "star-outline"} label={channel.favorite ? "Remove from favourites" : "Add to favourites"} onSelect={toggleFavorite} />
          </div>
          <div class="row-centre">
            {/* Without a window to seek in (the TV's player hasn't said one), only pause. */}
            {clock.window && <PlayerButton fid="back10" icon="rotate-ccw" skipSeconds={STEP_MS / 1000} label="Back 10 seconds" onSelect={() => seek(-STEP_MS)} />}
            <PlayerButton fid="playpause" icon={c.wantsToPlay ? "pause" : "play"} label={c.wantsToPlay ? "Pause" : "Play"} big onSelect={() => c.togglePause()} />
            {clock.window && <PlayerButton fid="fwd10" icon="rotate-cw" skipSeconds={STEP_MS / 1000} label="Forward 10 seconds" onSelect={() => seek(STEP_MS)} />}
          </div>
          <div class="row-right">
            {!atLive && <PlayerButton fid="golive" icon="radio" label="Go live" onSelect={() => c.goLive()} />}
            <PlayerButton fid="chdown" icon="chevron-down" label="Previous channel" onSelect={() => zap(-1)} />
            <PlayerButton fid="chup" icon="chevron-up" label="Next channel" onSelect={() => zap(1)} />
          </div>
        </G>
      </div>
      {ring && !trouble && (
        <div class="player-ring" role="status" aria-label="Loading">
          <div />
        </div>
      )}
      {trouble && (
        <div class="player-error live-error">
          <ChannelLogo channel={channel} width={128} height={72} />
          <div class="t-label c-muted" style="margin-top:12px">
            {channel.number && <span class="live-number">{channel.number}</span>}
            {channel.name}
          </div>
          <div class="t-page c-fg" style="text-align:center;margin-top:16px">
            {trouble.title}
          </div>
          {trouble.message && (
            <div class="t-body c-muted lines-4" style="margin-top:12px;text-align:center">
              {sentence(trouble.message)}
            </div>
          )}
          <G kind="row" class="buttons" style="margin-top:32px;justify-content:center">
            {trouble.canRetry && <Pill label="Try again" icon="refresh-cw" fid="retry" onSelect={() => c.retry()} />}
            <Pill label="Other channels" icon="list" fid="others" onSelect={() => setList(true)} />
            <TextButton label="Back" fid="back" onSelect={leave} />
          </G>
        </div>
      )}
      {list && (
        <ChannelList
          channels={channels}
          current={channel.id}
          now={now}
          hours24={hours24}
          onClose={() => setList(false)}
          onTune={(to) => {
            setList(false);
            tune(to);
          }}
        />
      )}
      <NumberBox digits={digits} missing={missing} />
      <Toast toast={toast ? { text: toast.text, error: toast.error, id: toast.n } : null} low={controls} />
    </div>
  );
}

/** "LIVE" in red at the live edge, else how far behind it. */
function LiveBadge(p: { atLive: boolean; behindMs: number }) {
  return (
    <span class={`live-badge t-badge${p.atLive ? " at-live" : ""}`}>
      <span class="live-dot" />
      {p.atLive ? "LIVE" : `−${behindText(p.behindMs)}`}
    </span>
  );
}

/** The channel, what's on with its time and progress, and what's next. */
function NowCard(p: { channel: LiveChannel; on: ReturnType<typeof nowAndNext>["on"]; next: ReturnType<typeof nowAndNext>["next"]; now: number; hours24: boolean }) {
  const c = p.channel;
  return (
    <div class="live-now">
      <ChannelLogo channel={c} width={112} height={63} class="live-now-logo" />
      <div class="live-now-text">
        <div class="t-label c-muted one-line">
          {c.number && <span class="live-number">{c.number}</span>}
          {c.name}
        </div>
        {p.on ? (
          <>
            <div class="live-now-title">
              <ProgramMarks program={p.on} />
              <span class="t-section c-fg one-line">{p.on.title}</span>
            </div>
            <div class="live-now-time">
              <span class="t-caption c-fg nowrap">{formatSpan(p.on, p.hours24)}</span>
              <ProgressLine fraction={programProgress(p.on, p.now)} class="wide" />
              <span class="t-caption c-muted nowrap">{timeLeft(p.on, p.now)}</span>
            </div>
          </>
        ) : (
          <div class="t-title c-muted one-line live-now-title">{c.has_guide ? "Nothing in the guide now" : "No guide for this channel"}</div>
        )}
        {p.next && <div class="t-caption c-muted one-line" style="margin-top:6px">{`Next  ${formatTime(p.next.start, p.hours24)} · ${p.next.title}`}</div>}
      </div>
    </div>
  );
}

/** The channel being tuned: its logo and what's on, on black, with the ring. */
function TuningCard(p: { channel: LiveChannel; on: ReturnType<typeof nowAndNext>["on"] }) {
  return (
    <div class="live-tuning" role="status" aria-label={`Tuning ${p.channel.name}`}>
      <div style="position:relative">
        <ChannelLogo channel={p.channel} width={192} height={108} showName />
        <div class="live-tuning-ring">
          <div />
        </div>
      </div>
      <div class="t-title c-fg" style="margin-top:24px">
        {p.channel.number && <span class="live-number">{p.channel.number}</span>}
        {p.channel.name}
      </div>
      <div class="t-label c-muted" style="margin-top:6px">
        {p.on ? p.on.title : "Tuning…"}
      </div>
    </div>
  );
}

/** A channel with sound and no picture: its logo and what's on, in place of the black. */
function RadioCard(p: { channel: LiveChannel; on: ReturnType<typeof nowAndNext>["on"]; hours24: boolean }) {
  return (
    <div class="live-radio" aria-hidden="true">
      <ChannelLogo channel={p.channel} width={240} height={135} showName />
      <div class="t-title c-fg" style="margin-top:20px">
        {p.channel.number && <span class="live-number">{p.channel.number}</span>}
        {p.channel.name}
      </div>
      <div class="live-radio-line t-label c-muted">
        <Icon name="radio" size={18} />
        <span>{p.on ? `${p.on.title} · ${formatSpan(p.on, p.hours24)}` : "Radio"}</span>
      </div>
    </div>
  );
}

/**
 * The timeshift: the window that can be sought in (the server's minutes),
 * where it plays in it, how long it is and how far behind live. Left and
 * Right on it go 10 s; the pointer's click goes there.
 */
function LiveBar(p: { state: LiveState; clock: { positionMs: number; window: { startMs: number; endMs: number } | null; behindMs: number }; atLive: boolean; onSeek: (ms: number) => void }) {
  const ui = useUi();
  const w = p.clock.window;
  const span = w ? w.endMs - w.startMs : 0;
  const frac = w && span > 0 ? Math.max(0, Math.min(1, (p.clock.positionMs - w.startMs) / span)) : 1;
  const minutes = Math.round(span / 60_000);
  const seekAt = (at: { clientX: number }) => {
    if (!w) return;
    p.onSeek(w.startMs + span * Math.max(0, Math.min(1, (at.clientX / ui.scale - BAR_LEFT) / BAR_WIDTH)));
  };
  return (
    <>
      <div class="player-times t-caption c-fg">
        <span>{w && minutes >= 1 ? `−${minutes} min` : ""}</span>
        <span class={p.atLive ? "c-fg" : "c-muted"}>{p.atLive ? "Live" : `${behindText(p.clock.behindMs)} behind live`}</span>
      </div>
      <F class="scrubber" fid="scrubber" label={p.atLive ? "Live" : `${behindText(p.clock.behindMs)} behind live`} onClick={seekAt}>
        <div class="bar">
          <div class="bar-buffered" style="width:100%" />
          <div class="bar-played" style={`width:${frac * 100}%`} />
        </div>
        <div class="thumb" style={`left:${Math.max(6, Math.min(BAR_WIDTH - 6, frac * BAR_WIDTH))}px`} />
      </F>
    </>
  );
}

/** A channel row's pitch in the list. */
const LIST_PITCH = 60;
/** Rows drawn either side of the focused one. */
const LIST_DRAWN = 10;

/**
 * The channel list over the picture (the web's ChannelPanel): every
 * channel, or the favourites, with what's on and how far it is; the one
 * playing first in focus. OK changes to the channel, Back closes it.
 */
function ChannelList(p: { channels: LiveChannel[]; current: number; now: number; hours24: boolean; onClose: () => void; onTune: (c: LiveChannel) => void }) {
  const ui = useUi();
  const trap = useTrap();
  const [favorites, setFavorites] = useState(false);
  const hasFavorites = p.channels.some((c) => c.favorite);
  const shown = favorites ? p.channels.filter((c) => c.favorite) : p.channels;
  const startAt = Math.max(0, shown.findIndex((c) => c.id === p.current));
  const [at, setAt] = useState(startAt);
  const [inside, setInside] = useState(false);
  useLayoutEffect(() => {
    const t = setTimeout(() => setInside(true), 16);
    const off = ui.input.push((key, e) => {
      if (key === "back") {
        if (e.type === "down") p.onClose();
        return true;
      }
      return false;
    });
    return () => {
      clearTimeout(t);
      off();
    };
  }, []);
  // The list's top keeps the focused row at a third of its height.
  const height = 540 - 120;
  const shift = Math.max(0, Math.min(shown.length * LIST_PITCH - height, at * LIST_PITCH - height / 3));
  const first = Math.max(0, at - LIST_DRAWN);
  const last = Math.min(shown.length, at + LIST_DRAWN + 6);
  const rows = [];
  for (let i = first; i < last; i++) {
    const ch = shown[i]!;
    const { on } = nowAndNext(ch, p.now);
    const playing = ch.id === p.current;
    rows.push(
      <F
        key={ch.id}
        class={`live-list-row${playing ? " current" : ""}`}
        fid={`list/${ch.id}`}
        initial={i === startAt}
        label={`${ch.number ? `${ch.number} ` : ""}${ch.name}${on ? `, ${on.title}` : ""}${playing ? ", playing" : ""}`}
        style={`top:${i * LIST_PITCH}px`}
        onFocus={() => setAt(i)}
        onSelect={() => p.onTune(ch)}
      >
        <span class="live-list-number t-label">{ch.number ?? ""}</span>
        <ChannelLogo channel={ch} width={64} height={36} />
        <div class="live-list-text">
          <div class="live-list-name">
            <span class="t-label one-line">{ch.name}</span>
            {ch.favorite && <Icon name="star" size={12} class="live-star" />}
          </div>
          {on ? (
            <>
              <div class="t-caption one-line live-list-sub">{on.title}</div>
              <ProgressLine fraction={programProgress(on, p.now)} />
            </>
          ) : (
            <div class="t-caption one-line live-list-sub">{ch.has_guide ? "Nothing in the guide now" : "No guide"}</div>
          )}
        </div>
        {playing && <Icon name="audio-lines" size={18} class="live-list-playing" />}
      </F>,
    );
  }
  return (
    <div ref={trap} class="panel-root" style="position:absolute;left:0;top:0;right:0;bottom:0;z-index:30" aria-label="Channels">
      <div class={`scrim${inside ? " in" : ""}`} onClick={p.onClose} />
      <div class={`side-panel wide live-list${inside ? " in" : ""}`}>
        <div class="live-list-head">
          <div class="t-title c-fg">Channels</div>
          {hasFavorites && (
            <G kind="row" class="live-list-filter">
              <F class={`tab t-label${!favorites ? " shown" : ""}`} fid="list/all" label="All channels" onSelect={() => (setFavorites(false), setAt(0))}>
                <span>All</span>
              </F>
              <F class={`tab t-label${favorites ? " shown" : ""}`} fid="list/favorites" label="Favourites" onSelect={() => (setFavorites(true), setAt(0))}>
                <span>Favourites</span>
              </F>
            </G>
          )}
        </div>
        <div class="live-list-box">
          <G kind="col" class="live-list-rows" style={`height:${shown.length * LIST_PITCH}px;transform:translateY(${-shift}px)`}>
            {rows}
          </G>
          {!shown.length && <div class="t-body c-muted" style="padding:16px">No favourites yet.</div>}
        </div>
      </div>
    </div>
  );
}
