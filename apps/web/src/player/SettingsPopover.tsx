import { useRef, useState, type ReactNode } from "react";
import { Check, ChevronLeft, ChevronRight, Minus, Plus, Settings2 } from "lucide-react";
import type { MediaStream } from "@/api/types";
import { cn } from "@/lib/cn";
import { formatClock, streamLabel } from "@/lib/format";
import { useDismiss } from "@/lib/hooks";
import { qualityById, type QualityOption } from "./quality";
import { SLEEP_MINUTES, type SleepChoice } from "./sleepTimer";
import {
  clampSubtitleOffset,
  formatSubtitleOffset,
  SUBTITLE_BACKGROUNDS,
  SUBTITLE_OFFSET_MAX,
  SUBTITLE_OFFSET_STEP,
  SUBTITLE_SIZES,
  type SubtitleStyle,
} from "./subtitles";

type Panel = "root" | "quality" | "audio" | "subtitles" | "style" | "sleep";

const PANEL_TITLE: Record<Exclude<Panel, "root">, string> = {
  quality: "Quality",
  audio: "Audio",
  subtitles: "Subtitles",
  style: "Subtitle style",
  sleep: "Sleep timer",
};

function GroupLabel({ children }: { children: ReactNode }) {
  return <div className="px-4 pb-1 pt-2 text-xs font-medium text-white/50">{children}</div>;
}

function Option({
  selected,
  onClick,
  hint,
  children,
}: {
  selected: boolean;
  onClick: () => void;
  hint?: string;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      role="menuitemradio"
      aria-checked={selected}
      onClick={onClick}
      className="flex w-full items-center gap-2.5 px-4 py-2 text-left text-sm transition-colors hover:bg-white/10"
    >
      <Check className={cn("size-4 shrink-0 text-accent", !selected && "invisible")} />
      <span className="min-w-0 flex-1">
        <span className="block truncate">{children}</span>
        {hint ? <span className="block truncate text-xs text-white/50">{hint}</span> : null}
      </span>
    </button>
  );
}

/** `extra`: said after the value, never cut off ("+0.5 s"). */
function Nav({ label, value, extra, onClick }: { label: string; value: string; extra?: string; onClick: () => void }) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="flex w-full items-center gap-3 px-4 py-2.5 text-left text-sm transition-colors hover:bg-white/10"
    >
      <span className="flex-1">{label}</span>
      <span className="flex min-w-0 max-w-44 items-center gap-1.5 text-white/60">
        <span className="truncate">{value}</span>
        {extra ? <span className="shrink-0 tabular-nums text-accent">{extra}</span> : null}
      </span>
      <ChevronRight className="size-4 shrink-0 text-white/50" />
    </button>
  );
}

function StepButton({ label, disabled, onClick, children }: { label: string; disabled?: boolean; onClick: () => void; children: ReactNode }) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      disabled={disabled}
      onClick={onClick}
      className="flex size-8 shrink-0 items-center justify-center rounded-full bg-white/10 transition-colors hover:bg-white/20 disabled:opacity-40"
    >
      {children}
    </button>
  );
}

/** Moves text subtitles earlier or later, 0.1 s at a time. */
function SubtitleSync({ offset, onChange }: { offset: number; onChange: (offset: number) => void }) {
  const step = (by: number) => onChange(clampSubtitleOffset(offset + by));
  return (
    <>
      <div className="my-1 h-px bg-white/10" />
      <GroupLabel>Timing</GroupLabel>
      <div className="flex items-center gap-2 px-4 py-1.5">
        <StepButton label="Show subtitles earlier" disabled={offset <= -SUBTITLE_OFFSET_MAX} onClick={() => step(-SUBTITLE_OFFSET_STEP)}>
          <Minus className="size-4" />
        </StepButton>
        <span className="min-w-16 flex-1 text-center text-sm tabular-nums" aria-live="polite" data-testid="subtitle-offset">
          {formatSubtitleOffset(offset)}
        </span>
        <StepButton label="Show subtitles later" disabled={offset >= SUBTITLE_OFFSET_MAX} onClick={() => step(SUBTITLE_OFFSET_STEP)}>
          <Plus className="size-4" />
        </StepButton>
        <button
          type="button"
          disabled={offset === 0}
          onClick={() => onChange(0)}
          className="ml-1 h-8 rounded-md px-2.5 text-xs font-semibold transition-colors hover:bg-white/10 disabled:opacity-40"
        >
          Reset
        </button>
      </div>
    </>
  );
}

export interface SettingsPopoverProps {
  open: boolean;
  setOpen: (open: boolean) => void;
  /** How the video plays now, at the top of the menu ([`PlaybackDetails`]). */
  details?: ReactNode;
  /** The qualities offered for this file, the original first. */
  qualities: QualityOption[];
  quality: string;
  /** What the original is ("1080p · 8.2 Mbps"). */
  originalHint: string | null;
  /** The quality the original is converted at here, when it has to be ("30m"). */
  autoQuality?: string | null;
  onQuality: (id: string) => void;
  audioStreams: MediaStream[];
  audioIndex: number | null;
  onAudio: (index: number) => void;
  subtitleStreams: MediaStream[];
  subtitleIndex: number | null;
  onSubtitle: (index: number | null) => void;
  /** How text subtitles look on this device. */
  subtitleStyle: SubtitleStyle;
  onSubtitleStyle: (style: SubtitleStyle) => void;
  /** Seconds text subtitles are moved by (positive: later), for this file. */
  subtitleOffset: number;
  onSubtitleOffset: (offset: number) => void;
  /** The sleep timer, if set, and how long it has left (seconds; for minutes). */
  sleep: SleepChoice | null;
  sleepLeft: number | null;
  /** "End of this episode". */
  sleepEndLabel: string;
  onSleep: (choice: SleepChoice | null) => void;
}

export function SettingsPopover(props: SettingsPopoverProps) {
  const { open, setOpen, details, qualities, quality, originalHint, autoQuality, onQuality } = props;
  const { audioStreams, audioIndex, onAudio, subtitleStreams, subtitleIndex, onSubtitle } = props;
  const { subtitleStyle, onSubtitleStyle, subtitleOffset, onSubtitleOffset } = props;
  const { sleep, sleepLeft, sleepEndLabel, onSleep } = props;
  const [panel, setPanel] = useState<Panel>("root");
  const ref = useRef<HTMLDivElement>(null);
  useDismiss(ref, open, () => {
    setOpen(false);
    setPanel("root");
  });

  const currentAudio = audioStreams.find((s) => s.index === audioIndex) ?? audioStreams.find((s) => s.default) ?? audioStreams[0];
  const currentSub = subtitleStreams.find((s) => s.index === subtitleIndex);
  // Picture subtitles are burned into the video: they can't be moved.
  const textSub = currentSub && currentSub.text_based !== false ? currentSub : undefined;
  const sleepValue =
    sleep === null ? "Off" : sleep === "end" ? sleepEndLabel : sleepLeft !== null ? `${formatClock(sleepLeft)} left` : `${sleep} min`;
  // The original, converted: at the closest quality, which the menu names.
  const auto = quality === "original" && autoQuality ? qualityById(autoQuality) : null;
  const qualityLabel = auto && auto.id !== "original" ? `${auto.label} (auto)` : (qualities.find((q) => q.id === quality)?.label ?? "Original");

  const close = () => {
    setOpen(false);
    setPanel("root");
  };

  return (
    <div ref={ref} className="relative">
      <button
        type="button"
        aria-label="Playback settings"
        aria-expanded={open}
        onClick={() => {
          setOpen(!open);
          setPanel("root");
        }}
        className={cn(
          "flex size-10 items-center justify-center rounded-full text-white/90 transition hover:bg-white/15 hover:text-white",
          open && "bg-white/15",
        )}
      >
        <Settings2 className={cn("size-5 transition-transform duration-300", open && "rotate-45")} />
      </button>
      {open ? (
        <div
          role="menu"
          className="absolute bottom-full right-0 mb-3 max-h-[60vh] w-80 animate-pop-in overflow-y-auto rounded-xl border border-white/10 bg-black/90 py-2 text-white shadow-2xl backdrop-blur-md"
        >
          {panel === "root" ? (
            <>
              {details ? (
                <>
                  {details}
                  <div className="my-1 h-px bg-white/10" />
                </>
              ) : null}
              <Nav label="Quality" value={qualityLabel} onClick={() => setPanel("quality")} />
              {audioStreams.length > 1 ? (
                <Nav label="Audio" value={currentAudio ? streamLabel(currentAudio) : "Default"} onClick={() => setPanel("audio")} />
              ) : null}
              <Nav
                label="Subtitles"
                value={currentSub ? streamLabel(currentSub) : subtitleStreams.length ? "Off" : "None"}
                extra={textSub && subtitleOffset !== 0 ? formatSubtitleOffset(subtitleOffset) : undefined}
                onClick={() => setPanel("subtitles")}
              />
              {subtitleStreams.some((s) => s.text_based !== false) ? (
                <Nav
                  label="Subtitle style"
                  value={SUBTITLE_SIZES.find((s) => s.id === subtitleStyle.size)?.label ?? "Normal"}
                  onClick={() => setPanel("style")}
                />
              ) : null}
              <Nav label="Sleep timer" value={sleepValue} onClick={() => setPanel("sleep")} />
            </>
          ) : (
            <>
              <button
                type="button"
                onClick={() => setPanel("root")}
                className="flex w-full items-center gap-2 px-3 py-2 text-sm font-semibold transition-colors hover:bg-white/10"
              >
                <ChevronLeft className="size-4" />
                {PANEL_TITLE[panel]}
              </button>
              <div className="my-1 h-px bg-white/10" />
              {panel === "quality"
                ? qualities.map((q, i) => (
                    <Option
                      key={q.id}
                      selected={q.id === quality}
                      hint={
                        i === 0
                          ? auto && auto.id !== "original"
                            ? `Plays as ${auto.label} here${originalHint ? ` (the file: ${originalHint})` : ""}`
                            : (originalHint ?? undefined)
                          : "Converted"
                      }
                      onClick={() => {
                        onQuality(q.id);
                        close();
                      }}
                    >
                      {q.label}
                    </Option>
                  ))
                : null}
              {panel === "audio"
                ? audioStreams.map((s) => (
                    <Option
                      key={s.index}
                      selected={s.index === currentAudio?.index}
                      onClick={() => {
                        onAudio(s.index);
                        close();
                      }}
                    >
                      {streamLabel(s)}
                    </Option>
                  ))
                : null}
              {panel === "style" ? (
                <>
                  <GroupLabel>Size</GroupLabel>
                  {SUBTITLE_SIZES.map((s) => (
                    <Option
                      key={s.id}
                      selected={s.id === subtitleStyle.size}
                      onClick={() => onSubtitleStyle({ ...subtitleStyle, size: s.id })}
                    >
                      {s.label}
                    </Option>
                  ))}
                  <GroupLabel>Background</GroupLabel>
                  {SUBTITLE_BACKGROUNDS.map((b) => (
                    <Option
                      key={b.id}
                      selected={b.id === subtitleStyle.background}
                      onClick={() => onSubtitleStyle({ ...subtitleStyle, background: b.id })}
                    >
                      {b.label}
                    </Option>
                  ))}
                </>
              ) : null}
              {panel === "subtitles" ? (
                <>
                  <Option
                    selected={subtitleIndex == null}
                    onClick={() => {
                      onSubtitle(null);
                      close();
                    }}
                  >
                    Off
                  </Option>
                  {subtitleStreams.map((s) => (
                    <Option
                      key={s.index}
                      selected={s.index === subtitleIndex}
                      onClick={() => {
                        onSubtitle(s.index);
                        close();
                      }}
                    >
                      {streamLabel(s)}
                      {s.text_based === false ? <span className="ml-1 text-xs text-white/50">(burn-in)</span> : null}
                    </Option>
                  ))}
                  {textSub ? <SubtitleSync offset={subtitleOffset} onChange={onSubtitleOffset} /> : null}
                </>
              ) : null}
              {panel === "sleep"
                ? ([null, ...SLEEP_MINUTES, "end"] as const).map((choice) => (
                    <Option
                      key={String(choice)}
                      selected={choice === sleep}
                      hint={choice !== null && choice !== "end" && choice === sleep ? sleepValue : undefined}
                      onClick={() => {
                        onSleep(choice);
                        close();
                      }}
                    >
                      {choice === null ? "Off" : choice === "end" ? sleepEndLabel : `${choice} minutes`}
                    </Option>
                  ))
                : null}
            </>
          )}
        </div>
      ) : null}
    </div>
  );
}
