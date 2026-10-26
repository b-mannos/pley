import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "preact/hooks";
import { formatClock, formatTimeOfDay } from "../../format/format";
import { initMusic } from "../../player/music";
import { PlaybackController, type PlayerEnv, type SavedPlayback } from "../../player/controller";
import type { PlayRequest } from "../../player/plan";
import { thumbnailAt, sheetPath } from "../../player/nextup";
import { badgeText, MEDIA_STEP_MS, Seeker, STEP_MS } from "../../player/seeker";
import { LEAVE_AFTER_MS } from "../../player/sleep";
import type { Key } from "../keys";
import { useUi } from "../context";
import { Pill, TextButton } from "../kit/buttons";
import { F, G } from "../kit/focusable";
import { Icon, type IconName } from "../kit/icons";
import { useApp } from "../screens/common";
import { chapterName, CompactReadout, PlayerPanels, type PlayerPanel } from "./panels";
import { PictureSubtitles, TextSubtitles } from "./subtitles";

/** Controls hide this long after the last key while a video plays. */
const HIDE_MS = 5_000;
/** The buffering ring shows after this long. */
const RING_MS = 500;
/** The player leaves this long after the server stopped it. */
const STOPPED_LEAVE_MS = 20_000;

const MEDIA_KEYS: Key[] = ["playPause", "play", "pause", "stop", "fastForward", "rewind", "next", "previous", "captions", "audio"];

/** The scrubber's line, in canvas px. */
const BAR_LEFT = 48;
const BAR_WIDTH = 864;

/**
 * The player (TvPlayerScreen): the TV's own player under the page, the
 * subtitles drawn above it, the controls and their panels, Skip, Up Next,
 * the sleep notice, and the remote's keys as the Android TV app reads them.
 */
export function PlayerScreen(p: { request: PlayRequest; pressedAt: number }) {
  const app = useApp();
  const ui = useUi();
  const host = useRef<HTMLDivElement>(null);
  const probe = useRef<string[] | null>(null);
  const [saved, setSaved] = useState<SavedPlayback | null>(null);
  const [run, setRun] = useState(0);

  // One controller per run: a new one when the TV shows the app again after hiding it.
  const controller = useMemo(() => {
    const request = saved ? saved.request : p.request;
    const env: PlayerEnv = {
      api: ui.api,
      reporter: app.reporter,
      engine: () => app.platform.createEngine(host.current!),
      caps: () =>
        app.caps().then((r) => {
          probe.current = r?.dvContainers ?? null;
          return r ? r.caps : null;
        }),
      keyedUrls: true,
      dvContainers: () => probe.current,
      clientId: app.installId(),
      offsets: app.offsets,
      quality: { remembered: () => saved?.quality ?? app.settings.value.quality, remember: (id) => app.updateSettings({ quality: id }) },
      autoplayNext: () => app.settings.value.autoplayNext,
      events: (l) => app.onEvent(l),
      cachedItem: (id) => app.cachedItem(id),
    };
    return new PlaybackController(request, env, saved ? undefined : p.pressedAt);
  }, [run]);

  useLayoutEffect(() => {
    initMusic(app).yieldToVideo();
    document.documentElement.classList.add("playing");
    controller.start();
    if (saved && !saved.playing) setTimeout(() => controller.pause(), 0);
    return () => {
      controller.close();
      document.documentElement.classList.remove("playing");
    };
  }, [controller]);

  // While it plays, Home and the pages wait to ask the server again; after it, they do (its reports moved resume points).
  useLayoutEffect(() => {
    app.holdRefresh(true);
    return () => app.holdRefresh(false);
  }, []);

  // The TV hides the app (Home, another app): the playback ends, to start again where it was.
  useEffect(() => {
    let away: SavedPlayback | null = null;
    const off = app.visible.subscribe((visible) => {
      if (!visible && !away) {
        away = controller.saveState();
        controller.close();
      } else if (visible && away) {
        setSaved(away);
        away = null;
        setRun((n) => n + 1);
      }
    });
    return off;
  }, [controller]);

  return <PlayerView key={run} controller={controller} host={host} />;
}

function PlayerView(p: { controller: PlaybackController; host: { current: HTMLDivElement | null } }) {
  const app = useApp();
  const ui = useUi();
  const c = p.controller;
  const s = c.state.value;
  const clock = c.clock.value;
  const shown = c.subs.value;
  const settings = app.settings.value;
  const [controls, setControls] = useState(false);
  const [panel, setPanel] = useState<PlayerPanel | null>(null);
  const [toast, setToast] = useState<{ text: string; ms: number; n: number } | null>(null);
  const [, setSeekTick] = useState(0);
  const [ring, setRing] = useState(false);
  const lastInput = useRef(Date.now());
  const rememberedSubtitle = useRef<number | null>(null);
  const seeker = useMemo(
    () =>
      new Seeker({
        onSeek: (ms) => c.seekTo(ms),
        onChange: () => setSeekTick((n) => n + 1),
      }),
    [c],
  );
  const playing = s.phase.kind === "playing" || s.phase.kind === "buffering" || s.phase.kind === "preparing";
  const error = s.phase.kind === "error" ? s.phase : null;

  const leave = () => {
    c.close();
    ui.nav.pop();
  };
  const say = (text: string, ms = 3500) => setToast({ text, ms, n: Date.now() });

  // The TV keeps the screen on while a video plays.
  useEffect(() => {
    app.platform.keepAwake(playing);
  }, [playing]);
  useEffect(() => () => app.platform.keepAwake(false), []);

  // Time to the first frame, for measuring against the Android TV app.
  useEffect(() => {
    if (s.firstFrameMs !== null) console.info(`pley: first frame ${s.firstFrameMs} ms after the press`);
  }, [s.firstFrameMs]);

  // The controller's one-shot notices become toasts.
  useEffect(() => {
    if (!s.notice) return;
    say(s.notice);
    c.dismissNotice();
  }, [s.notice]);
  useEffect(() => {
    if (!toast) return;
    const t = setTimeout(() => setToast(null), toast.ms);
    return () => clearTimeout(t);
  }, [toast?.n]);

  // The ring after half a second of preparing or buffering.
  const waiting = s.phase.kind === "preparing" || s.phase.kind === "buffering";
  useEffect(() => {
    if (!waiting) {
      setRing(false);
      return;
    }
    const t = setTimeout(() => setRing(true), RING_MS);
    return () => clearTimeout(t);
  }, [waiting]);

  // The end with nothing next, a server stop after 20 s, the sleep timer's pause after 10 min: the player leaves.
  useEffect(() => {
    if (s.phase.kind === "ended") leave();
  }, [s.phase.kind]);
  useEffect(() => {
    if (!s.stoppedByServer) return;
    const t = setTimeout(leave, STOPPED_LEAVE_MS);
    return () => clearTimeout(t);
  }, [s.stoppedByServer]);
  useEffect(() => {
    if (!s.sleep.fired) return;
    const t = setTimeout(leave, LEAVE_AFTER_MS);
    return () => clearTimeout(t);
  }, [s.sleep.fired]);

  // Up Next or an error takes over: the pending seek goes, the panel and controls close.
  useEffect(() => {
    if (!s.upNext && !error) return;
    seeker.cancel();
    setPanel(null);
    setControls(false);
  }, [!!s.upNext, !!error]);

  // Controls hide 5 s after the last key while a video plays.
  useEffect(() => {
    if (!controls || panel || !playing || seeker.target !== null) return;
    const t = setInterval(() => {
      if (Date.now() - lastInput.current >= HIDE_MS) setControls(false);
    }, 250);
    return () => clearInterval(t);
  }, [controls, panel, playing, seeker.target]);

  const focusFid = (fid: string) =>
    setTimeout(() => {
      const el = document.querySelector<HTMLElement>(`.player [data-fid="${fid}"]`);
      if (el) ui.focus.focus(el);
    }, 0);
  const show = (focus: string) => {
    setControls(true);
    focusFid(focus);
  };
  const playPause = () => {
    if (c.wantsToPlay) {
      c.pause();
      if (!controls) show("scrubber");
    } else c.play();
  };
  const skipShown = !!s.skipLabel && !s.upNext && !error && !(controls && seeker.target !== null);
  const sleepShown = s.sleep.secondsLeft !== null && !error;

  // A new Skip or sleep notice takes focus while nothing else is up; the hidden-controls focus holder otherwise.
  useEffect(() => {
    if (controls || panel || s.upNext || error) return;
    if (skipShown) focusFid("skip");
    else if (sleepShown) focusFid("keep");
    else focusFid("video");
  }, [skipShown, sleepShown, controls, !!panel, !!s.upNext, !!error]);
  useEffect(() => {
    if (s.upNext) focusFid("playnow");
  }, [s.upNext?.itemId]);
  useEffect(() => {
    if (error) focusFid(error.canRetry ? "retry" : "back");
  }, [!!error]);

  const position = clock.positionMs;
  const duration = clock.durationMs;
  const chapters = s.chapters;
  const episode = s.subtitle !== null;
  const hasPrevNext = episode || chapters.length > 1;

  const media = (key: Key) => {
    switch (key) {
      case "playPause":
        playPause();
        return;
      case "play":
        c.play();
        return;
      case "pause":
        c.pause();
        if (!controls && !s.upNext && !error) show("scrubber");
        return;
      case "stop":
        leave();
        return;
      case "fastForward":
        seeker.press(1, MEDIA_STEP_MS, position, duration);
        return;
      case "rewind":
        seeker.press(-1, MEDIA_STEP_MS, position, duration);
        return;
      case "next": {
        if (episode) {
          c.playNext();
          return;
        }
        const from = seeker.target ?? position;
        const next = chapters.filter((ch) => ch.startMs > from + 1000)[0];
        if (next) c.seekTo(next.startMs);
        return;
      }
      case "previous": {
        const from = seeker.target ?? position;
        const at = chapters.reduce((found, ch, i) => (ch.startMs <= from ? i : found), -1);
        if (at < 0) c.seekTo(0);
        else if (from - chapters[at]!.startMs > 3000 || at === 0) c.seekTo(chapters[at]!.startMs);
        else c.seekTo(chapters[at - 1]!.startMs);
        return;
      }
      case "captions": {
        const on = s.subtitles.filter((t) => t.selected && t.index !== null)[0];
        if (on) {
          rememberedSubtitle.current = on.index;
          c.selectSubtitle(null);
          say("Subtitles off", 2000);
        } else {
          const tracks = s.subtitles.filter((t) => t.index !== null);
          const pick = tracks.filter((t) => t.index === rememberedSubtitle.current)[0] ?? tracks.filter((t) => !t.burnIn)[0] ?? tracks[0];
          if (pick) {
            c.selectSubtitle(pick.index);
            say(`Subtitles: ${pick.label}`, 2000);
          }
        }
        return;
      }
      case "audio": {
        const tracks = s.audio;
        if (tracks.length < 2) return;
        const at = tracks.findIndex((t) => t.selected);
        const next = tracks[(at + 1) % tracks.length]!;
        if (next.index !== null) c.selectAudio(next.index);
        say(`Audio: ${next.label}`, 2000);
        return;
      }
      default:
        return;
    }
  };

  // The remote, in the Android app's order: media keys, an error, Up Next, a panel, then the controls.
  useEffect(
    () =>
      ui.input.push((key, e) => {
        lastInput.current = Date.now();
        if (MEDIA_KEYS.indexOf(key) >= 0) {
          if (e.type === "down" && !e.repeat) media(key);
          return true;
        }
        const st = c.state.value;
        if (st.phase.kind === "error") {
          if (key === "back") {
            if (e.type === "down") leave();
            return true;
          }
          return false;
        }
        if (st.upNext) {
          if (key === "up" || key === "down") return true;
          if (key === "back") {
            if (e.type === "down") c.cancelUpNext();
            return true;
          }
          return false;
        }
        if (panelRef.current) return false;
        const busy = st.phase.kind === "preparing";
        const focused = ui.focus.current?.getAttribute("data-fid") ?? null;
        if (!controlsRef.current) {
          if (key === "ok") {
            if (focused === "skip" || focused === "keep") return false;
            if (e.type === "down" && !e.repeat && !busy) playPause();
            return true;
          }
          if (key === "left" || key === "right") {
            if (busy) return true;
            if (e.type === "up") {
              if (seeker.holding) seeker.release();
              return true;
            }
            const dir = key === "right" ? 1 : -1;
            if (e.repeat) {
              if (!seeker.holding) show("scrubber");
              seeker.repeat(dir, c.clock.value.positionMs, c.clock.value.durationMs);
            } else seeker.press(dir, STEP_MS, c.clock.value.positionMs, c.clock.value.durationMs);
            return true;
          }
          if (e.type !== "down") return key === "up" || key === "down" || key === "back" || key === "menu" || key === "info";
          if (key === "up") {
            show("scrubber");
            return true;
          }
          if (key === "down") {
            show(st.subtitles.length ? "subtitles" : "info");
            return true;
          }
          if (key === "menu" || key === "info") {
            show(skipShownRef.current ? "skip" : "scrubber");
            return true;
          }
          if (key === "back") {
            leave();
            return true;
          }
          return false;
        }
        // Controls shown.
        if (key === "back") {
          if (e.type === "down") {
            setControls(false);
            focusFid(skipShownRef.current ? "skip" : "video");
          }
          return true;
        }
        if (focused === "scrubber") {
          if (key === "left" || key === "right") {
            if (busy) return true;
            if (e.type === "up") {
              if (seeker.holding) seeker.release();
              return true;
            }
            const dir = key === "right" ? 1 : -1;
            if (e.repeat) seeker.repeat(dir, c.clock.value.positionMs, c.clock.value.durationMs);
            else seeker.press(dir, STEP_MS, c.clock.value.positionMs, c.clock.value.durationMs);
            return true;
          }
          if (key === "ok") {
            if (e.type === "down" && !e.repeat && !busy) playPause();
            return true;
          }
          if (key === "down") {
            if (e.type === "down") focusFid("playpause");
            return true;
          }
          if (key === "up") {
            if (e.type === "down" && skipShownRef.current) focusFid("skip");
            return true;
          }
        }
        return false;
      }),
    [c],
  );
  const controlsRef = useRef(controls);
  controlsRef.current = controls;
  const panelRef = useRef(panel);
  panelRef.current = panel;
  const skipShownRef = useRef(skipShown);
  skipShownRef.current = skipShown;

  const openPanel = (which: PlayerPanel) => setPanel(which);
  const closePanel = () => {
    const opener = panel;
    setPanel(null);
    if (opener) focusFid(opener === "subtitles" ? "subtitles" : opener);
  };

  const target = seeker.target;
  const thumbPos = target ?? position;
  const endsAt = duration > 0 ? formatTimeOfDay(new Date(Date.now() + Math.max(0, duration - position)), app.platform.clock24()) : null;
  const engineStats = () => c.engine?.stats() ?? null;
  const readoutShown = settings.playbackDetails && !controls && !panel && !s.upNext && !error;
  return (
    <div class="player">
      <div
        ref={(el) => {
          p.host.current = el;
        }}
        class="engine-host"
      />
      {shown.picture && <PictureSubtitles frame={shown.picture} />}
      {shown.cues.length > 0 && <TextSubtitles cues={shown.cues} size={settings.subtitleSize} box={settings.subtitleBox} lifted={controls} />}
      <F class="video-holder" fid="video" disabled={controls} label="Video" />
      <div class={`player-controls${controls ? " in" : ""}`} aria-hidden={!controls}>
        <div class="scrim-top" />
        <div class="scrim-bottom" />
        <div class="player-top">
          <div style="min-width:0;flex:1;padding-right:300px">
            <div class="t-title c-fg one-line">{s.title}</div>
            {s.subtitle && <div class="t-label c-muted one-line" style="margin-top:4px">{s.subtitle.replace(" — ", " · ")}</div>}
          </div>
          <div class="player-clock">
            <div class="t-label c-muted">{formatTimeOfDay(new Date(), app.platform.clock24())}</div>
            {endsAt && <div class="t-label c-muted">{`Ends at ${endsAt}`}</div>}
          </div>
        </div>
        {target !== null && controls && <SeekPreview state={s} targetMs={target} durationMs={duration} />}
        <div class="player-times t-caption c-fg">
          <span>{formatClock(position)}</span>
          <span>{`−${formatClock(Math.max(0, duration - position))}`}</span>
        </div>
        <Scrubber positionMs={position} bufferedMs={clock.bufferedMs} durationMs={duration} targetMs={thumbPos} markers={s.markers} chapters={chapters.map((ch) => ch.startMs)} />
        <G kind="row" class="player-row">
          <div class="row-left">
            {chapters.length > 1 && <PlayerButton fid="chapters" icon="list-video" label="Chapters" onSelect={() => openPanel("chapters")} />}
            <PlayerButton fid="sleep" icon="moon" label="Sleep timer" onSelect={() => openPanel("sleep")} />
          </div>
          <div class="row-centre">
            {hasPrevNext && <PlayerButton fid="previous" icon="skip-back" label="Previous" onSelect={() => media("previous")} />}
            <PlayerButton fid="back10" icon="rotate-ccw" label="Back 10 seconds" onSelect={() => seeker.press(-1, STEP_MS, position, duration)} />
            <PlayerButton fid="playpause" icon={playing ? "pause" : "play"} label={playing ? "Pause" : "Play"} big onSelect={playPause} />
            <PlayerButton fid="fwd10" icon="rotate-cw" label="Forward 10 seconds" onSelect={() => seeker.press(1, STEP_MS, position, duration)} />
            {hasPrevNext && <PlayerButton fid="next" icon="skip-forward" label="Next" onSelect={() => media("next")} />}
          </div>
          <div class="row-right">
            {s.subtitles.length > 0 && <PlayerButton fid="subtitles" icon="captions" label="Subtitles" onSelect={() => openPanel("subtitles")} />}
            {s.audio.length > 1 && <PlayerButton fid="audio" icon="audio-lines" label="Audio" onSelect={() => openPanel("audio")} />}
            {s.qualities.length > 0 && <PlayerButton fid="quality" icon="gauge" label="Quality" onSelect={() => openPanel("quality")} />}
            <PlayerButton fid="info" icon="info" label="Info" onSelect={() => openPanel("info")} />
          </div>
        </G>
      </div>
      {readoutShown && <CompactReadout state={s} controller={c} stats={engineStats} />}
      {skipShown && (
        <G kind="row" class={`skip-box${controls ? " lifted" : ""}`}>
          <F class="skip-pill t-label" fid="skip" label={s.skipLabel!} onSelect={() => c.skip()}>
            <Icon name="chevrons-right" size={22} />
            <span style="margin-left:10px">{s.skipLabel}</span>
          </F>
        </G>
      )}
      {sleepShown && (
        <G kind="row" class={`sleep-notice${controls ? " lifted" : ""}`}>
          <Icon name="moon" size={20} color="var(--muted)" />
          <span class="t-label c-fg">{`Pausing in ${s.sleep.secondsLeft} s`}</span>
          <F class="keep-btn t-label" fid="keep" label="Keep watching" onSelect={() => c.keepWatching()}>
            Keep watching
          </F>
        </G>
      )}
      {s.upNext && <UpNextCard card={s.upNext} reduced={ui.reducedMotion} onPlay={() => c.playNext()} onCancel={() => c.cancelUpNext()} />}
      {!controls && seeker.badgeMs !== 0 && (
        <div class="seek-badge t-label" style={`left:${seeker.badgeMs < 0 ? 149.3 : 746.7}px`}>
          {badgeText(seeker.badgeMs)}
        </div>
      )}
      {ring && !error && (
        <div class="player-ring" role="status" aria-label="Loading">
          <div />
        </div>
      )}
      {panel && <PlayerPanels panel={panel} state={s} controller={c} positionMs={position} onClose={closePanel} stats={engineStats} engineName={c.engine?.name ?? ""} />}
      {error && (
        <div class="player-error">
          <div class="t-page c-fg" style="text-align:center">
            {s.stoppedByServer ? "Playback stopped" : "Can't play this item"}
          </div>
          <div class="t-body c-muted lines-4" style="margin-top:12px;text-align:center;white-space:pre-line">
            {error.message}
          </div>
          <G kind="row" class="buttons" style="margin-top:32px;justify-content:center">
            {error.canRetry ? (
              <>
                <Pill label="Try again" icon="refresh-cw" fid="retry" onSelect={() => c.retry()} />
                <TextButton label="Back" fid="back" onSelect={leave} />
              </>
            ) : (
              <Pill label="Back" icon="arrow-left" fid="back" onSelect={leave} />
            )}
          </G>
        </div>
      )}
      {toast && <div class={`toast in t-label${controls ? " low" : ""}`}>{toast.text}</div>}
    </div>
  );
}

/** A button of the control row: 48 px (Play/Pause 64), its label under it while focused. */
function PlayerButton(p: { fid: string; icon: IconName; label: string; big?: boolean; onSelect: () => void }) {
  return (
    <div class={`pbtn-box${p.big ? " big" : ""}`}>
      <F class={`pbtn${p.big ? " big" : ""}`} fid={p.fid} label={p.label} onSelect={p.onSelect}>
        <Icon name={p.icon} size={p.big ? 32 : 24} />
      </F>
      <div class="pbtn-label t-caption" aria-hidden="true">
        {p.label}
      </div>
    </div>
  );
}

/** The scrubber: played, buffered, chapter gaps, the thumb at the seek target, the marker's name under it. */
function Scrubber(p: { positionMs: number; bufferedMs: number; durationMs: number; targetMs: number; markers: { kind: string; start_ms: number; end_ms: number }[]; chapters: number[] }) {
  const d = p.durationMs;
  const frac = (ms: number) => (d > 0 ? Math.max(0, Math.min(1, ms / d)) : 0);
  const marker = p.markers.filter((m) => p.targetMs >= m.start_ms && p.targetMs < m.end_ms)[0];
  const markerX = marker ? Math.max(BAR_LEFT, Math.min(BAR_LEFT + BAR_WIDTH, BAR_LEFT + BAR_WIDTH * frac((marker.start_ms + marker.end_ms) / 2))) : 0;
  return (
    <>
      <F class="scrubber" fid="scrubber" label={`${formatClock(p.positionMs)} of ${formatClock(d)}`}>
        <div class="bar">
          <div class="bar-buffered" style={`width:${frac(p.bufferedMs) * 100}%`} />
          <div class="bar-played" style={`width:${frac(p.positionMs) * 100}%`} />
          {p.chapters
            .filter((ms) => ms > 0 && frac(ms) * BAR_WIDTH > 2 && frac(ms) * BAR_WIDTH < BAR_WIDTH - 2)
            .map((ms) => (
              <div key={ms} class="bar-gap" style={`left:${frac(ms) * 100}%`} />
            ))}
        </div>
        <div class="thumb" style={`left:${frac(p.targetMs) * 100}%`} />
      </F>
      {marker && (
        <div class="marker-label t-caption c-muted" style={`left:${markerX}px`}>
          {marker.kind === "intro" ? "Intro" : "Credits"}
        </div>
      )}
    </>
  );
}

/** The picture at the seek target: a seek preview, else the chapter's picture; its time and chapter under it. */
function SeekPreview(p: { state: ReturnType<PlaybackController["state"]["peek"]>; targetMs: number; durationMs: number }) {
  const ui = useUi();
  const tp = p.state.trickplay;
  const bucket = Math.floor(p.targetMs / 2000) * 2000;
  const tile = tp && tp.info.count > 0 ? thumbnailAt(tp.info, bucket) : null;
  const [sheetUrl, setSheetUrl] = useState<string | null>(null);
  const sheet = tile && tp ? ui.api.url(sheetPath(tp.fileId, tp.info, tile.sheet)) : null;
  useEffect(() => {
    if (!sheet) return;
    return ui.images.load(sheet, (u) => setSheetUrl(u), -1);
  }, [sheet]);
  const chapters = p.state.chapters;
  const at = chapters.reduce((found, ch, i) => (ch.startMs <= p.targetMs ? i : found), -1);
  const chapter = at >= 0 ? chapters[at]! : null;
  const chapterImage = !tile && chapter?.imageUrl ? chapter.imageUrl : null;
  const x = Math.max(BAR_LEFT, Math.min(BAR_LEFT + BAR_WIDTH, BAR_LEFT + (p.durationMs > 0 ? (BAR_WIDTH * p.targetMs) / p.durationMs : 0)));
  const picture = (tile && sheetUrl) || chapterImage;
  const scale = tp ? 240 / tp.info.width : 1;
  return (
    <div class="seek-preview" style={`left:${Math.max(48 + 120, Math.min(912 - 120, x)) - 120}px;top:${picture ? 184 : 325}px`}>
      {tile && sheetUrl && tp ? (
        <div class="preview-box">
          <div
            style={`width:${tp.info.width}px;height:${tp.info.height}px;transform-origin:0 0;transform:scale(${scale});background-image:url("${sheetUrl}");background-position:${-tile.x}px ${-tile.y}px;background-repeat:no-repeat`}
          />
        </div>
      ) : chapterImage ? (
        <div class="preview-box">
          <ChapterImage path={chapterImage} />
        </div>
      ) : null}
      <div class="t-label c-fg" style="margin-top:6px;text-align:center">
        {formatClock(p.targetMs)}
      </div>
      {chapter && (
        <div class="t-caption c-muted one-line" style="text-align:center">
          {chapterName(chapter.title, at + 1)}
        </div>
      )}
    </div>
  );
}

function ChapterImage(p: { path: string }) {
  const ui = useUi();
  const [src, setSrc] = useState<string | null>(null);
  useEffect(() => ui.images.load(ui.images.url(p.path, 240 * ui.scale), (u) => setSrc(u), -1), [p.path]);
  return src ? <img src={src} alt="" style="width:100%;height:100%;object-fit:cover" /> : null;
}

/** Up Next (TvUpNextCard): the next one's picture and title, Play now with its countdown ring, Cancel. */
function UpNextCard(p: { card: NonNullable<ReturnType<PlaybackController["state"]["peek"]>["upNext"]>; reduced: boolean; onPlay: () => void; onCancel: () => void }) {
  const ui = useUi();
  const [src, setSrc] = useState<string | null>(null);
  useEffect(() => {
    if (!p.card.imageUrl) return;
    return ui.images.load(p.card.imageUrl, (u) => setSrc(u), -1);
  }, [p.card.imageUrl]);
  const left = p.card.secondsLeft;
  const sweep = left === null ? 0 : (360 * (10 - left)) / 10;
  return (
    <div class="upnext-card">
      <div class="upnext-image">{src && <img src={src} alt="" />}</div>
      <div style="padding:16px">
        <div class="t-caption c-muted one-line">{p.card.heading}</div>
        <div class="t-title c-fg lines-2">{p.card.title}</div>
        <G kind="row" class="upnext-buttons">
          <F class="playnow t-label" fid="playnow" label="Play now" onSelect={p.onPlay}>
            <Icon name="play" size={22} />
            <span style="margin-left:10px">Play now</span>
            {left !== null &&
              (p.reduced ? (
                <span class="t-caption" style="margin-left:12px">{`Next in ${left}`}</span>
              ) : (
                <svg width="22" height="22" viewBox="0 0 22 22" style="margin-left:12px" aria-hidden="true">
                  <circle cx="11" cy="11" r="9.5" fill="none" stroke="currentColor" stroke-opacity="0.25" stroke-width="3" />
                  <path d={arc(11, 11, 9.5, sweep)} fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" />
                </svg>
              ))}
          </F>
          <TextButton label="Cancel" onSelect={p.onCancel} />
        </G>
      </div>
    </div>
  );
}

/** An SVG arc from 12 o'clock, clockwise, `deg` degrees. */
function arc(cx: number, cy: number, r: number, deg: number): string {
  if (deg <= 0) return "";
  const d = Math.min(359.99, deg);
  const rad = ((d - 90) * Math.PI) / 180;
  const x = cx + r * Math.cos(rad);
  const y = cy + r * Math.sin(rad);
  return `M ${cx} ${cy - r} A ${r} ${r} 0 ${d > 180 ? 1 : 0} 1 ${x.toFixed(2)} ${y.toFixed(2)}`;
}
