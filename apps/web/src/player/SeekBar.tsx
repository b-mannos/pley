import { useRef, useState, type PointerEvent as ReactPointerEvent } from "react";
import { cn } from "@/lib/cn";
import { formatClock } from "@/lib/format";

export interface SeekBarProps {
  current: number;
  duration: number;
  /** Buffered ranges in seconds. */
  buffered: [number, number][];
  onSeek: (seconds: number) => void;
  /** Called while scrubbing (null when scrubbing ends). */
  onScrub?: (seconds: number | null) => void;
}

export function SeekBar({ current, duration, buffered, onSeek, onScrub }: SeekBarProps) {
  const ref = useRef<HTMLDivElement>(null);
  const [hover, setHover] = useState<{ x: number; t: number } | null>(null);
  const [drag, setDrag] = useState<number | null>(null);

  const timeAt = (clientX: number) => {
    const el = ref.current;
    if (!el || duration <= 0) return { x: 0, t: 0 };
    const rect = el.getBoundingClientRect();
    const x = Math.min(Math.max(clientX - rect.left, 0), rect.width);
    return { x, t: (x / rect.width) * duration };
  };

  const shown = drag ?? current;
  const pct = duration > 0 ? Math.min(100, (shown / duration) * 100) : 0;

  const onPointerDown = (e: ReactPointerEvent<HTMLDivElement>) => {
    if (duration <= 0 || e.button !== 0) return;
    e.currentTarget.setPointerCapture(e.pointerId);
    const { t } = timeAt(e.clientX);
    setDrag(t);
    onScrub?.(t);
  };
  const onPointerMove = (e: ReactPointerEvent<HTMLDivElement>) => {
    const pos = timeAt(e.clientX);
    setHover(pos);
    if (drag != null) {
      setDrag(pos.t);
      onScrub?.(pos.t);
    }
  };
  const onPointerUp = (e: ReactPointerEvent<HTMLDivElement>) => {
    if (drag == null) return;
    const { t } = timeAt(e.clientX);
    setDrag(null);
    onScrub?.(null);
    onSeek(t);
  };

  return (
    <div
      ref={ref}
      role="slider"
      tabIndex={0}
      aria-label="Seek"
      aria-valuemin={0}
      aria-valuemax={Math.round(duration)}
      aria-valuenow={Math.round(shown)}
      aria-valuetext={`${formatClock(shown)} of ${formatClock(duration)}`}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onPointerCancel={() => {
        setDrag(null);
        onScrub?.(null);
      }}
      onPointerLeave={() => setHover(null)}
      onKeyDown={(e) => {
        // Arrow keys are handled globally by the player; stop the page from scrolling.
        if (e.key === "Home") {
          e.preventDefault();
          onSeek(0);
        } else if (e.key === "End") {
          e.preventDefault();
          onSeek(Math.max(0, duration - 1));
        }
      }}
      className="group/seek relative flex h-6 cursor-pointer touch-none items-center"
    >
      <div
        className={cn(
          "relative h-1 w-full overflow-hidden rounded-full bg-white/20 transition-[height] duration-150 group-hover/seek:h-1.5",
          drag != null && "h-1.5",
        )}
      >
        {duration > 0
          ? buffered.map(([s, e], i) => (
              <div
                key={i}
                className="absolute inset-y-0 bg-white/30"
                style={{ left: `${(s / duration) * 100}%`, width: `${Math.max(0, ((e - s) / duration) * 100)}%` }}
              />
            ))
          : null}
        {hover && drag == null ? (
          <div className="absolute inset-y-0 left-0 bg-white/20" style={{ width: `${hover.x}px` }} />
        ) : null}
        <div className="absolute inset-y-0 left-0 bg-accent" style={{ width: `${pct}%` }} />
      </div>
      <div
        className={cn(
          "pointer-events-none absolute top-1/2 size-3.5 -translate-x-1/2 -translate-y-1/2 rounded-full bg-accent shadow-md transition-transform duration-150",
          drag != null ? "scale-125" : "scale-0 group-hover/seek:scale-100",
        )}
        style={{ left: `${pct}%` }}
      />
      {(hover || drag != null) && duration > 0 ? (
        <div
          className="pointer-events-none absolute bottom-full mb-2 -translate-x-1/2 rounded bg-black/85 px-2 py-1 text-xs font-medium tabular-nums text-white shadow"
          style={{ left: drag != null ? `${pct}%` : `${hover?.x ?? 0}px` }}
        >
          {formatClock(drag ?? hover?.t ?? 0)}
        </div>
      ) : null}
    </div>
  );
}
