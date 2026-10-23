import { useRef, useState, type ReactNode } from "react";
import { Check, ChevronLeft, ChevronRight, Settings2 } from "lucide-react";
import type { MediaStream } from "@/api/types";
import { cn } from "@/lib/cn";
import { streamLabel } from "@/lib/format";
import { useDismiss } from "@/lib/hooks";
import { Toggle } from "@/components/ui";
import type { QualityOption } from "./quality";
import { SUBTITLE_BACKGROUNDS, SUBTITLE_SIZES, type SubtitleStyle } from "./subtitles";

type Panel = "root" | "quality" | "audio" | "subtitles" | "style";

const PANEL_TITLE: Record<Exclude<Panel, "root">, string> = {
  quality: "Quality",
  audio: "Audio",
  subtitles: "Subtitles",
  style: "Subtitle style",
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

function Nav({ label, value, onClick }: { label: string; value: string; onClick: () => void }) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="flex w-full items-center gap-3 px-4 py-2.5 text-left text-sm transition-colors hover:bg-white/10"
    >
      <span className="flex-1">{label}</span>
      <span className="max-w-40 truncate text-white/60">{value}</span>
      <ChevronRight className="size-4 text-white/50" />
    </button>
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
  /** Skip intros without waiting for a click. */
  autoSkip: boolean;
  onAutoSkip: (on: boolean) => void;
}

export function SettingsPopover(props: SettingsPopoverProps) {
  const { open, setOpen, details, qualities, quality, originalHint, onQuality } = props;
  const { audioStreams, audioIndex, onAudio, subtitleStreams, subtitleIndex, onSubtitle, autoSkip, onAutoSkip } = props;
  const { subtitleStyle, onSubtitleStyle } = props;
  const [panel, setPanel] = useState<Panel>("root");
  const ref = useRef<HTMLDivElement>(null);
  useDismiss(ref, open, () => {
    setOpen(false);
    setPanel("root");
  });

  const currentAudio = audioStreams.find((s) => s.index === audioIndex) ?? audioStreams.find((s) => s.default) ?? audioStreams[0];
  const currentSub = subtitleStreams.find((s) => s.index === subtitleIndex);
  const qualityLabel = qualities.find((q) => q.id === quality)?.label ?? "Original";

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
                onClick={() => setPanel("subtitles")}
              />
              {subtitleStreams.some((s) => s.text_based !== false) ? (
                <Nav
                  label="Subtitle style"
                  value={SUBTITLE_SIZES.find((s) => s.id === subtitleStyle.size)?.label ?? "Normal"}
                  onClick={() => setPanel("style")}
                />
              ) : null}
              <div className="my-1 h-px bg-white/10" />
              <div className="flex items-center gap-3 px-4 py-2.5 text-sm">
                <span className="flex-1">Skip intros automatically</span>
                <Toggle checked={autoSkip} onChange={onAutoSkip} label="Skip intros automatically" />
              </div>
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
                      hint={i === 0 ? (originalHint ?? undefined) : "Converted"}
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
                </>
              ) : null}
            </>
          )}
        </div>
      ) : null}
    </div>
  );
}
