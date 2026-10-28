import type { ComponentChildren } from "preact";
import { useEffect, useRef, useState } from "preact/hooks";
import { channelsLabel, codecLabel, formatBitrate, formatClock } from "../../format/format";
import { mediaFacts, streamBlock, type FactBlock } from "../../format/media-info";
import type { PlaybackController, PlayerState } from "../../player/controller";
import type { EngineStats } from "../../player/engine";
import { SLEEP_CHOICES, sameChoice, sleepLabel } from "../../player/sleep";
import { speedLabel } from "../../player/speed";
import { formatOffset, SYNC_STEP_MS } from "../../subs/sync";
import { PanelRow } from "../kit/buttons";
import { F } from "../kit/focusable";
import { Icon } from "../kit/icons";
import { PanelTitle, SidePanel } from "../kit/panel";

export type PlayerPanel = "subtitles" | "audio" | "quality" | "chapters" | "speed" | "sleep" | "info";

/** A chapter's name: its title, or "Chapter N" (its title after it, unless it is one already). */
export function chapterName(title: string | null, n: number): string {
  if (!title) return `Chapter ${n}`;
  return /^chapter/i.test(title) ? title : `Chapter ${n} · ${title}`;
}

/** The player's side panels (TvPlayerPanels): pick a row, it applies and closes; Back closes. */
export function PlayerPanels(p: { panel: PlayerPanel; state: PlayerState; controller: PlaybackController; positionMs: number; onClose: () => void; stats: () => EngineStats | null; engineName: string }) {
  const s = p.state;
  const c = p.controller;
  const pick = (fn: () => void) => () => {
    fn();
    p.onClose();
  };
  switch (p.panel) {
    case "subtitles":
      return (
        <SidePanel onClose={p.onClose} label="Subtitles">
          <PanelTitle>Subtitles</PanelTitle>
          {s.subtitleSync && <SyncRow offsetMs={s.subtitleOffsetMs} onChange={(ms) => c.setSubtitleOffset(ms)} />}
          {s.subtitles.map((t) => (
            <PanelRow
              key={String(t.index)}
              label={t.label}
              sub={t.burnIn ? "Drawn into the picture by the server" : null}
              checked={t.selected}
              initial={t.selected}
              onSelect={pick(() => c.selectSubtitle(t.index))}
            />
          ))}
        </SidePanel>
      );
    case "audio":
      return (
        <SidePanel onClose={p.onClose} label="Audio">
          <PanelTitle>Audio</PanelTitle>
          {s.audio.map((t) => (
            <PanelRow key={String(t.index)} label={t.label} checked={t.selected} initial={t.selected} onSelect={pick(() => t.index !== null && c.selectAudio(t.index))} />
          ))}
        </SidePanel>
      );
    case "quality":
      return (
        <SidePanel onClose={p.onClose} label="Quality">
          <PanelTitle>Quality</PanelTitle>
          {s.note && <div class="t-body c-muted panel-note">{s.note}</div>}
          {s.canPlayOriginal && <PanelRow label="Play original" sub="Try the original again" icon="rotate-ccw" onSelect={pick(() => c.playOriginal())} />}
          {s.qualities.map((q) => (
            <PanelRow key={q.id} label={q.label} sub={q.hint} checked={q.selected} initial={q.selected} onSelect={pick(() => c.selectQuality(q.id))} />
          ))}
        </SidePanel>
      );
    case "chapters": {
      const at = s.chapters.reduce((found, ch, i) => (ch.startMs <= p.positionMs ? i : found), -1);
      return (
        <SidePanel onClose={p.onClose} label="Chapters">
          <PanelTitle>Chapters</PanelTitle>
          {s.chapters.map((ch, i) => (
            <PanelRow
              key={ch.startMs}
              label={ch.title || `Chapter ${i + 1}`}
              sub={formatClock(ch.startMs)}
              checked={i === at}
              initial={i === Math.max(0, at)}
              onSelect={pick(() => c.seekTo(ch.startMs))}
            />
          ))}
        </SidePanel>
      );
    }
    case "speed":
      return (
        <SidePanel onClose={p.onClose} label="Speed">
          <PanelTitle>Speed</PanelTitle>
          {s.speeds.map((rate) => (
            <PanelRow key={String(rate)} label={speedLabel(rate)} checked={rate === s.speed} initial={rate === s.speed} onSelect={pick(() => c.setSpeed(rate))} />
          ))}
        </SidePanel>
      );
    case "sleep":
      return (
        <SidePanel onClose={p.onClose} label="Sleep timer">
          <PanelTitle>Sleep timer</PanelTitle>
          {SLEEP_CHOICES.map((choice) => {
            const on = sameChoice(choice, s.sleep.choice);
            return (
              <PanelRow
                key={sleepLabel(choice)}
                label={sleepLabel(choice, s.kind)}
                sub={on && choice.kind === "minutes" && s.sleep.minutesLeft !== null ? `${s.sleep.minutesLeft} min left` : null}
                checked={on}
                initial={on}
                onSelect={pick(() => c.setSleep(choice))}
              />
            );
          })}
        </SidePanel>
      );
    case "info":
      return <InfoPanel state={s} controller={c} onClose={p.onClose} stats={p.stats} engineName={p.engineName} />;
  }
}

/** Subtitle sync: Left and Right move the subtitles 0.1 s, OK puts them back. */
function SyncRow(p: { offsetMs: number; onChange: (ms: number) => void }) {
  return (
    <F
      class="panel-row sync-row"
      label={`Sync ${formatOffset(p.offsetMs)}`}
      onSelect={() => p.onChange(0)}
      onKey={(key) => {
        if (key === "left") {
          p.onChange(p.offsetMs - SYNC_STEP_MS);
          return true;
        }
        if (key === "right") {
          p.onChange(p.offsetMs + SYNC_STEP_MS);
          return true;
        }
        return false;
      }}
    >
      <div class="grow">
        <div class="t-label">Sync</div>
        <div class="t-caption sub">OK resets</div>
      </div>
      <Icon name="chevron-left" size={20} />
      <span class="t-label" style="padding:0 6px">
        {formatOffset(p.offsetMs)}
      </span>
      <Icon name="chevron-right" size={20} />
    </F>
  );
}

/** The readout's sections: how it plays, the file, its streams, and what this TV says. */
export function readoutSections(state: PlayerState, stats: EngineStats | null, engineName: string): FactBlock[] {
  const d = state.details;
  const out: FactBlock[] = [];
  if (!d) return out;
  const playback = { label: "Playback", value: d.pipeline ? `${d.method} · ${d.pipeline}` : d.method };
  const how: FactBlock = { title: "How it plays", facts: d.why ? [playback, { label: "Why", value: d.why }] : [playback] };
  for (const r of d.rows) how.facts.push({ label: r.label, value: r.value });
  how.facts.push({ label: "Quality", value: state.quality });
  out.push(how);
  if (d.file) {
    const file = mediaFacts(d.file)[0];
    if (file) out.push(file);
  }
  if (d.video) out.push(streamBlock(d.video, "Video"));
  if (d.audio) out.push(streamBlock(d.audio, "Audio"));
  const subs: FactBlock = { title: "Subtitles", facts: [] };
  if (!d.subtitle) subs.facts.push({ label: "Track", value: "Off" });
  else {
    subs.facts.push({ label: "Format", value: `${codecLabel(d.subtitle.codec)}, ${d.subtitle.text_based === false ? "picture" : "text"}` });
    if (d.subtitle.language) subs.facts.push({ label: "Language", value: d.subtitle.language });
    subs.facts.push({
      label: "Shown",
      value:
        d.subtitleDelivery === "burned_in"
          ? "Burned into the picture by the server"
          : d.subtitleDelivery === "server_pictures"
            ? "As pictures from the server, drawn by this TV"
            : "As WebVTT from the server, drawn by this TV",
    });
  }
  out.push(subs);
  const tv: FactBlock = { title: "This TV", facts: [{ label: "Player", value: engineName }] };
  if (stats) {
    const decoded = [stats.width && stats.height ? `${stats.width}×${stats.height}` : "", stats.frameRate ? `${stats.frameRate.toFixed(3).replace(/\.?0+$/, "")} fps` : "", stats.hdr ?? "", stats.codec ? codecLabel(stats.codec) : ""]
      .filter(Boolean)
      .join(" · ");
    if (decoded) tv.facts.push({ label: "Decoded", value: decoded });
    if (stats.decoder) tv.facts.push({ label: "Video decoder", value: `${stats.decoder} (${stats.hardware === false ? "software" : "hardware"})` });
    if (stats.audioCodec) tv.facts.push({ label: "Audio", value: [codecLabel(stats.audioCodec), channelsLabel(stats.audioChannels ?? null)].filter(Boolean).join(" ") });
    if (stats.audioOutput) tv.facts.push({ label: "Output", value: stats.audioOutput });
    if (stats.bitrate) tv.facts.push({ label: "Current bitrate", value: formatBitrate(stats.bitrate) });
    if (stats.dropped !== undefined && stats.frames) tv.facts.push({ label: "Dropped frames", value: `${stats.dropped} of ${stats.frames}` });
    if (stats.bufferedAheadMs !== undefined) tv.facts.push({ label: "Buffered ahead", value: `${Math.round(stats.bufferedAheadMs / 1000)} s` });
    tv.facts.push({ label: "Stalls", value: String(state.stalls) });
    for (const line of stats.extra ?? []) {
      const i = line.indexOf(":");
      if (i > 0) tv.facts.push({ label: line.slice(0, i), value: line.slice(i + 1).trim() });
    }
  }
  if (state.firstFrameMs !== null) tv.facts.push({ label: "First frame", value: `${state.firstFrameMs} ms` });
  out.push(tv);
  return out;
}

function InfoPanel(p: { state: PlayerState; controller: PlaybackController; onClose: () => void; stats: () => EngineStats | null; engineName: string }) {
  const [, setTick] = useState(0);
  const [scroll, setScroll] = useState(0);
  const box = useRef<HTMLDivElement>(null);
  useEffect(() => {
    p.controller.setDetailsOpen(true);
    const t = setInterval(() => setTick((n) => n + 1), 1000);
    return () => {
      clearInterval(t);
      p.controller.setDetailsOpen(false);
    };
  }, []);
  const sections = readoutSections(p.state, p.stats(), p.engineName);
  return (
    <SidePanel onClose={p.onClose} wide label="Playback info">
      <F
        class="info-scroll"
        initial
        label="Playback info"
        onKey={(key) => {
          const max = Math.max(0, (box.current?.offsetHeight ?? 0) - 440);
          if (key === "down") {
            setScroll((s) => Math.min(max, s + 96));
            return true;
          }
          if (key === "up") {
            setScroll((s) => Math.max(0, s - 96));
            return true;
          }
          return key === "left" || key === "right";
        }}
      >
        <div ref={box} class="scroll-y" style={`transform:translateY(${-scroll}px)`}>
          <PanelTitle>Playback info</PanelTitle>
          {!sections.length ? <div class="t-body c-muted panel-note">Asking the server…</div> : sections.map((s) => <Facts key={s.title} block={s} />)}
          <div style="height:24px" />
        </div>
      </F>
    </SidePanel>
  );
}

function Facts(p: { block: FactBlock }) {
  return (
    <div>
      <div class="t-title c-fg readout-heading">{p.block.title}</div>
      {p.block.facts.map((f) => (
        <div key={f.label} class="readout-fact">
          <div class="t-caption c-muted one-line readout-label">{f.label}</div>
          <div class="t-caption c-fg lines-3 readout-value">{f.value}</div>
        </div>
      ))}
    </div>
  );
}

/** The compact readout (Settings → Playback details) over the video while nothing else shows. */
export function CompactReadout(p: { state: PlayerState; controller: PlaybackController; stats: () => EngineStats | null }): ComponentChildren {
  const [, setTick] = useState(0);
  useEffect(() => {
    p.controller.setDetailsOpen(true);
    const t = setInterval(() => setTick((n) => n + 1), 1000);
    return () => {
      clearInterval(t);
      p.controller.setDetailsOpen(false);
    };
  }, []);
  const d = p.state.details;
  if (!d) return null;
  const stats = p.stats();
  const lines: [string, string][] = [["Playback", d.pipeline ? `${d.method} · ${d.pipeline}` : d.method]];
  for (const r of d.rows) lines.push([r.label, r.value]);
  const sub = d.subtitle;
  lines.push(["Subtitles", sub ? [sub.language ?? "", codecLabel(sub.codec)].filter(Boolean).join(" · ") : "Off"]);
  // The file's bitrate, then what arrives now when that differs: "18 Mbps → 8 Mbps".
  const rates = [d.file?.bitrate ? formatBitrate(d.file.bitrate) : "", stats?.bitrate ? formatBitrate(stats.bitrate) : ""].filter((r, i, a) => r && a.indexOf(r) === i);
  if (rates.length) lines.push(["Bitrate", rates.join(" → ")]);
  if (stats?.width && stats.height) lines.push(["Decoded", `${stats.width}×${stats.height}${stats.hdr ? ` · ${stats.hdr}` : ""}`]);
  if (stats?.dropped !== undefined && stats.frames) lines.push(["Dropped", `${stats.dropped} of ${stats.frames}`]);
  if (stats?.bufferedAheadMs !== undefined) lines.push(["Buffer", `${Math.round(stats.bufferedAheadMs / 1000)} s ahead${stats.bandwidth ? ` · ${formatBitrate(stats.bandwidth)}` : ""}`]);
  return (
    <div class="compact-readout">
      {lines.map(([l, v]) => (
        <div key={l} class="readout-fact">
          <div class="t-caption c-muted one-line" style="width:96px;flex:none">
            {l}
          </div>
          <div class="t-caption c-fg one-line">{v}</div>
        </div>
      ))}
    </div>
  );
}

