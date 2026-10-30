import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "preact/hooks";
import type { LiveChannel, LiveGroups } from "../../api/types";
import { behindText, categoryKey, categoryName, channelByNumber, channelName, formatSpan, formatTime, LIVE_WITHIN_MS, nowAndNext, parseCategory, programProgress, timeLeft, type LiveCategory } from "../../live/data";
import { ChannelPages } from "../../live/pages";
import { LiveController } from "../../live/session";
import { initMusic } from "../../player/music";
import type { Key } from "../keys";
import { useUi } from "../context";
import { Pill, TextButton } from "../kit/buttons";
import { F, G } from "../kit/focusable";
import { Icon } from "../kit/icons";
import { useTrap } from "../kit/panel";
import { Toast } from "../kit/toast";
import { failedTitle, problem, sentence, useApp, useLoad } from "../screens/common";
import { CategoryPanel } from "../screens/live-categories";
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

/** Keys the player takes whatever has focus. */
const MEDIA_KEYS: Key[] = ["playPause", "play", "pause", "stop", "fastForward", "rewind", "next", "previous", "channelUp", "channelDown", "channelList", "guide", "info"];

/**
 * A live channel playing (the web's LivePlayerPage on a TV): the TV's own
 * player under the page, what's on, Live (or how far behind it), pause and
 * rewind in the timeshift, the channel list with the categories, and
 * changing channels with CH+ and CH−, Up and Down within the category it
 * was opened from, or by a channel's number among every channel. Back
 * leaves, to the channel watched; the server is told at once.
 */
export function LivePlayerScreen(p: { channelId: number; category: string; pressedAt: number }) {
  const app = useApp();
  const ui = useUi();
  const known = useMemo(() => app.knownLiveChannel(p.channelId), []);
  const fetched = useLoad((c) => (known ? Promise.resolve(known) : ui.api.liveChannel(p.channelId, c)), [], { live: false, initial: known });
  const first = fetched.data;
  if (!first) {
    if (fetched.error) {
      const missing = (fetched.error as { status?: number }).status === 404;
      return missing ? (
        <Blackout title="This channel isn't here" message="It may have been hidden, or its source removed." />
      ) : (
        <Blackout title={failedTitle(fetched.error, "Live TV didn't load")} message={problem(fetched.error)} onRetry={fetched.reload} />
      );
    }
    return (
      <div class="player live-player">
        <div class="player-ring" role="status" aria-label="Loading">
          <div />
        </div>
      </div>
    );
  }
  return <LivePlayerView first={first} category={parseCategory(p.category)} pressedAt={p.pressedAt} />;
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

function LivePlayerView(p: { first: LiveChannel; category: LiveCategory; pressedAt: number }) {
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
  /** Channel up and down stay in this category; its order is the one it had as it opened (Recently watched changes with each channel). */
  const [category, setCategory] = useState<LiveCategory>(p.category);
  const zapPages = useMemo(() => new ChannelPages(ui.api, category, { frozen: true }), [categoryKey(category)]);
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

  // Live TV opens back at the channel watched, in the category it was played in.
  useEffect(() => {
    app.keepLiveChannel(channel);
    app.liveLastWatched = { id: channel.id, category: categoryKey(category) };
  }, [channel.id, categoryKey(category)]);

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
  // The channel before or after this one in the category, round its ends; one change at a time.
  const zapping = useRef(false);
  const zap = (by: number) => {
    if (zapping.current) return;
    zapping.current = true;
    zapPages.neighbor(c.state.value.channel.id, by).then(
      (next) => {
        zapping.current = false;
        tune(next);
      },
      () => {
        zapping.current = false;
        say("Couldn't find the next channel", true);
      },
    );
  };
  const seek = (ms: number) => {
    if (!c.canSeek) return;
    c.seekBy(ms);
    if (!controls) show(ms < 0 ? "back10" : "fwd10");
  };
  const toggleFavorite = () => {
    const on = !channel.favorite;
    ui.api.setLiveFavorite(channel.id, on).then(
      () => {
        c.state.value = { ...c.state.value, channel: { ...c.state.value.channel, favorite: on } };
        say(on ? `${channel.name} is a favourite` : `${channel.name} isn't a favourite any more`);
      },
      () => say(on ? "Couldn't add it to your favourites" : "Couldn't take it out of your favourites", true),
    );
  };

  // A channel's number: among every channel, as a remote's would.
  const digits = useNumberEntry((d) => {
    ui.api.liveBrowse({ category: "all", q: d, limit: 50 }).then(
      (page) => {
        const found = channelByNumber(page.channels, d);
        if (found) tune(found);
        else setMissing(d);
      },
      () => setMissing(d),
    );
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
  const name = channelName(channel, category);
  return (
    <div class="player live-player">
      <div
        ref={(el) => {
          host.current = el;
        }}
        class="engine-host"
      />
      <div class={`player-scrim${controls || infoShown ? " in" : ""}`} />
      <F class="video-holder" fid="video" disabled={controls} label={`${name}, live`} />
      {s.phase === "tuning" && !s.shown && <TuningCard channel={channel} name={name} on={on} />}
      {s.shown && s.audioOnly && !trouble && <RadioCard channel={channel} name={name} on={on} hours24={hours24} />}
      <div class={`live-info${infoShown ? " in" : ""}`} aria-hidden={!infoShown}>
        <NowCard channel={channel} name={name} on={on} next={next} now={now} hours24={hours24} />
      </div>
      <div class={`player-controls${controls ? " in" : ""}`} aria-hidden={!controls}>
        <div class="player-top">
          <div style="flex:1" />
          <div class="player-clock">
            <TimeOfDay hours24={hours24} />
          </div>
        </div>
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
            <LiveChip atLive={atLive} behindMs={clock.behindMs} onGoLive={() => c.goLive()} />
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
            {name}
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
          category={category}
          current={channel.id}
          now={now}
          onClose={() => setList(false)}
          onTune={(to, inCategory) => {
            setList(false);
            if (categoryKey(inCategory) !== categoryKey(category)) setCategory(inCategory);
            tune(to);
          }}
        />
      )}
      <NumberBox digits={digits} missing={missing} />
      <Toast toast={toast ? { text: toast.text, error: toast.error, id: toast.n } : null} low={controls} />
    </div>
  );
}

/**
 * "LIVE" in red at the live edge; behind it, how far ("−0:42"), and OK
 * goes back to live. It sits with the buttons: a live channel has no
 * position bar.
 */
function LiveChip(p: { atLive: boolean; behindMs: number; onGoLive: () => void }) {
  if (p.atLive) {
    return (
      <div class="live-chip-box">
        <span class="live-badge t-badge at-live" aria-label="Live">
          <span class="live-dot" />
          LIVE
        </span>
      </div>
    );
  }
  return (
    <div class="live-chip-box">
      <F class="live-badge live-chip t-badge" fid="golive" label={`${behindText(p.behindMs)} behind live. Go live`} onSelect={p.onGoLive}>
        <span class="live-dot" />
        {`−${behindText(p.behindMs)}`}
      </F>
      <div class="pbtn-label t-caption" aria-hidden="true">
        Go live
      </div>
    </div>
  );
}

type OnNow = ReturnType<typeof nowAndNext>["on"];

/** The channel, what's on with its time and progress, and what's next. */
function NowCard(p: { channel: LiveChannel; name: string; on: OnNow; next: ReturnType<typeof nowAndNext>["next"]; now: number; hours24: boolean }) {
  const c = p.channel;
  return (
    <div class="live-now">
      <ChannelLogo channel={c} width={112} height={63} class="live-now-logo" />
      <div class="live-now-text">
        <div class="t-label c-muted one-line">
          {c.number && <span class="live-number">{c.number}</span>}
          {p.name}
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

/** The channel being tuned: its logo, the spinner centred under it, and what's on, on black. */
function TuningCard(p: { channel: LiveChannel; name: string; on: OnNow }) {
  return (
    <div class="live-tuning" role="status" aria-label={`Tuning ${p.name}`}>
      <ChannelLogo channel={p.channel} width={192} height={108} showName />
      <div class="live-tuning-ring">
        <div />
      </div>
      <div class="t-title c-fg" style="margin-top:20px">
        {p.channel.number && <span class="live-number">{p.channel.number}</span>}
        {p.name}
      </div>
      <div class="t-label c-muted" style="margin-top:6px">
        {p.on ? p.on.title : "Tuning…"}
      </div>
    </div>
  );
}

/** A channel with sound and no picture: its logo and what's on, in place of the black. */
function RadioCard(p: { channel: LiveChannel; name: string; on: OnNow; hours24: boolean }) {
  return (
    <div class="live-radio" aria-hidden="true">
      <ChannelLogo channel={p.channel} width={240} height={135} showName />
      <div class="t-title c-fg" style="margin-top:20px">
        {p.channel.number && <span class="live-number">{p.channel.number}</span>}
        {p.name}
      </div>
      <div class="live-radio-line t-label c-muted">
        <Icon name="radio" size={18} />
        <span>{p.on ? `${p.on.title} · ${formatSpan(p.on, p.hours24)}` : "Radio"}</span>
      </div>
    </div>
  );
}

/** A channel row's pitch in the list. */
const LIST_PITCH = 60;
/** Rows drawn either side of the focused one. */
const LIST_DRAWN = 10;

/**
 * The channel list over the picture (the web's ChannelPanel): a category's
 * channels, a page at a time as they scroll, with what's on and how far it
 * is; the one playing first in focus. Left opens the categories, to list
 * another; OK changes to the channel (and channel up and down then stay in
 * its category). Back closes it.
 */
function ChannelList(p: { category: LiveCategory; current: number; now: number; onClose: () => void; onTune: (c: LiveChannel, category: LiveCategory) => void }) {
  const ui = useUi();
  const trap = useTrap();
  const [category, setCategory] = useState(p.category);
  const pages = useMemo(() => new ChannelPages(ui.api, category), [categoryKey(category)]);
  const total = pages.total.value ?? 0;
  pages.version.value;
  const [groups, setGroups] = useState<LiveGroups | null>(null);
  const [cats, setCats] = useState(false);
  const [at, setAt] = useState(0);
  const [ready, setReady] = useState(false);
  const [inside, setInside] = useState(false);
  const listBox = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    const t = setTimeout(() => setInside(true), 16);
    const off = ui.input.push((key, e) => {
      if (key === "back") {
        if (e.type === "down" && !e.repeat) p.onClose();
        return true;
      }
      if (key === "left" && listBox.current?.contains(ui.focus.current)) {
        if (e.type === "down" && !e.repeat) setCats(true);
        return true;
      }
      return false;
    });
    void ui.api.liveGroups().then(setGroups, () => undefined);
    return () => {
      clearTimeout(t);
      off();
    };
  }, []);
  // It opens on the channel playing (or the category's first, in another category).
  useEffect(() => {
    let live = true;
    setReady(false);
    pages.indexOf(p.current).then(
      (index) => {
        if (!live) return;
        setAt(index ?? 0);
        setReady(true);
      },
      () => live && setReady(true),
    );
    return () => {
      live = false;
    };
  }, [pages]);
  const first = Math.max(0, at - LIST_DRAWN);
  const last = Math.min(total, at + LIST_DRAWN + 6);
  useEffect(() => pages.want(first, last), [first, last, pages]);
  // The row focus was on was drawn anew (its page came): focus to it again.
  useLayoutEffect(() => {
    const current = ui.focus.current;
    if (!ready || (current && current.isConnected && listBox.current?.contains(current)) || cats) return;
    const el = listBox.current?.querySelector<HTMLElement>(`[data-fid="list/at${at}"]`);
    if (el) ui.focus.focus(el);
  });

  const height = 540 - 120;
  const shift = Math.max(0, Math.min(total * LIST_PITCH - height, at * LIST_PITCH - height / 3));
  const rows = [];
  if (ready) {
    for (let i = first; i < last; i++) {
      const ch = pages.at(i);
      if (!ch) {
        rows.push(
          <F key={i} class="live-list-row loading" fid={`list/at${i}`} label="Loading" style={`top:${i * LIST_PITCH}px`} onFocus={() => setAt(i)}>
            <span class="live-list-number" />
            <div class="skeleton" style="width:64px;height:36px;border-radius:4px" />
          </F>,
        );
        continue;
      }
      const { on } = nowAndNext(ch, p.now);
      const playing = ch.id === p.current;
      const name = channelName(ch, category);
      rows.push(
        <F
          key={i}
          class={`live-list-row${playing ? " current" : ""}`}
          fid={`list/at${i}`}
          label={`${ch.number ? `${ch.number} ` : ""}${name}${on ? `, ${on.title}` : ""}${playing ? ", playing" : ""}`}
          style={`top:${i * LIST_PITCH}px`}
          onFocus={() => setAt(i)}
          onSelect={() => p.onTune(ch, category)}
        >
          <span class="live-list-number t-label">{ch.number ?? ""}</span>
          <ChannelLogo channel={ch} width={64} height={36} />
          <div class="live-list-text">
            <div class="live-list-name">
              <span class="t-label one-line">{name}</span>
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
  }
  return (
    <div ref={trap} class="panel-root" style="position:absolute;left:0;top:0;right:0;bottom:0;z-index:30" aria-label="Channels">
      <div class={`scrim${inside ? " in" : ""}`} onClick={p.onClose} />
      <div class={`side-panel wide live-list${inside ? " in" : ""}`}>
        <div class="live-list-head">
          <div class="t-title c-fg one-line">{categoryName(category, groups?.groups)}</div>
          <div class="t-caption c-muted live-list-hint">
            <Icon name="chevron-left" size={14} />
            <span>Categories</span>
          </div>
        </div>
        <div ref={listBox} class="live-list-box">
          <G kind="col" class="live-list-rows" style={`height:${total * LIST_PITCH}px;transform:translateY(${-shift}px)`}>
            {rows}
          </G>
          {ready && total === 0 && <div class="t-body c-muted" style="padding:16px">No channels here.</div>}
        </div>
      </div>
      {cats && (
        <CategoryPanel
          groups={groups}
          selected={category}
          left={0}
          onClose={() => setCats(false)}
          onSelect={(next) => {
            setCats(false);
            setCategory(next);
          }}
        />
      )}
    </div>
  );
}
