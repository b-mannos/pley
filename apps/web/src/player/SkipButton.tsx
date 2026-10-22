import { useEffect, useRef } from "react";
import { ChevronsRight } from "lucide-react";
import type { ItemDetail, Marker } from "@/api/types";
import { cn } from "@/lib/cn";

/** The button disappears this long before the segment ends. */
const TAIL_MS = 1_000;

const LABEL: Record<Marker["kind"], string> = {
  intro: "Skip Intro",
  credits: "Skip Credits",
};

/** Markers of the file being played (the item's first file when unknown). */
export function fileMarkers(item: ItemDetail, fileId: number | null | undefined): Marker[] {
  const id = fileId ?? item.files[0]?.id;
  return (item.markers ?? []).filter((m) => m.file_id === id);
}

/** The marker the playhead (in seconds) is in, if any. */
export function activeMarker(markers: Marker[], seconds: number): Marker | null {
  const ms = seconds * 1000;
  return markers.find((m) => ms >= m.start_ms && ms < m.end_ms - TAIL_MS) ?? null;
}

/**
 * Skip Intro / Skip Credits behaviour. Returns the marker the playhead is in.
 * Entering an intro skips it when `autoSkip` is on, entering credits calls
 * `onCredits` (Up Next for episodes); each happens once per segment, so
 * seeking back into it later lets it play.
 */
export function useSkipMarkers({
  markers,
  seconds,
  enabled,
  autoSkip,
  onAutoSkip,
  onCredits,
}: {
  markers: Marker[];
  seconds: number;
  enabled: boolean;
  autoSkip: boolean;
  onAutoSkip: (marker: Marker) => void;
  onCredits: (marker: Marker) => void;
}): Marker | null {
  const marker = enabled ? activeMarker(markers, seconds) : null;
  const handled = useRef(new Set<string>());
  const key = marker ? `${marker.file_id}:${marker.kind}:${marker.start_ms}` : null;
  const callbacks = useRef({ onAutoSkip, onCredits });
  callbacks.current = { onAutoSkip, onCredits };

  useEffect(() => {
    if (!marker || !key || handled.current.has(key)) return;
    if (marker.kind === "intro" && !autoSkip) return;
    handled.current.add(key);
    if (marker.kind === "intro") callbacks.current.onAutoSkip(marker);
    else callbacks.current.onCredits(marker);
    // Runs when the playhead enters a different segment.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key, autoSkip]);

  return marker;
}

export function SkipButton({ marker, onSkip, lifted }: { marker: Marker; onSkip: () => void; lifted: boolean }) {
  return (
    <button
      type="button"
      onClick={(e) => {
        e.stopPropagation();
        onSkip();
      }}
      onDoubleClick={(e) => e.stopPropagation()}
      title={`${LABEL[marker.kind]} (s)`}
      className={cn(
        "absolute right-4 z-20 flex h-11 animate-pop-in items-center gap-2 rounded-md border border-white/25 bg-black/70 px-5 text-sm font-semibold text-white shadow-2xl backdrop-blur-md transition-[bottom,background-color,border-color,color] duration-300 hover:border-accent hover:bg-accent hover:text-accent-fg focus-visible:border-accent sm:right-8",
        lifted ? "bottom-28" : "bottom-10",
      )}
    >
      {LABEL[marker.kind]}
      <ChevronsRight className="size-4" />
    </button>
  );
}
