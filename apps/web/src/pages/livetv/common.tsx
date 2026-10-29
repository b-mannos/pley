import { memo, useEffect, useMemo, useState } from "react";
import type { Channel, Program } from "@/api/livetv";
import { cn } from "@/lib/cn";
import { placeholderGradient, sizedImage } from "@/lib/images";

/** The current time in unix seconds, updated every `everyMs`. */
export function useNow(everyMs = 15_000): number {
  const [now, setNow] = useState(() => Date.now() / 1000);
  useEffect(() => {
    const id = window.setInterval(() => setNow(Date.now() / 1000), everyMs);
    return () => window.clearInterval(id);
  }, [everyMs]);
  return now;
}

const timeFormat = new Intl.DateTimeFormat(undefined, { hour: "numeric", minute: "2-digit" });
const dayFormat = new Intl.DateTimeFormat(undefined, { weekday: "short", day: "numeric", month: "short" });

/** "20:00" (or "8:00 PM"), as the browser's locale writes times. */
export function formatTime(ts: number): string {
  return timeFormat.format(new Date(ts * 1000));
}

/** "20:00 – 21:00". */
export function formatSpan(p: Pick<Program, "start" | "stop">): string {
  return `${formatTime(p.start)} – ${formatTime(p.stop)}`;
}

/** "Today", "Tomorrow", or "Fri 30 Oct". */
export function formatGuideDay(ts: number, now: number): string {
  const day = (t: number) => new Date(t * 1000).toDateString();
  if (day(ts) === day(now)) return "Today";
  if (day(ts) === day(now + 86_400)) return "Tomorrow";
  if (day(ts) === day(now - 86_400)) return "Yesterday";
  return dayFormat.format(new Date(ts * 1000));
}

/** Minutes left of a programme: "12 min left". */
export function timeLeft(p: Pick<Program, "stop">, now: number): string {
  const minutes = Math.max(1, Math.round((p.stop - now) / 60));
  if (minutes >= 60) {
    const h = Math.floor(minutes / 60);
    const m = minutes % 60;
    return m ? `${h} h ${m} min left` : `${h} h left`;
  }
  return `${minutes} min left`;
}

/** A channel's logo on a dark tile, or its number and name when it has none. */
export const ChannelLogo = memo(function ChannelLogo({
  channel,
  width,
  className,
  showName = true,
}: {
  channel: Pick<Channel, "logo" | "name" | "number">;
  width: number;
  className?: string;
  /** The name on the placeholder (small tiles show the number only). */
  showName?: boolean;
}) {
  const [failed, setFailed] = useState(false);
  const src = !failed ? sizedImage(channel.logo, width) : null;
  return (
    <div
      className={cn("relative flex items-center justify-center overflow-hidden", className)}
      style={src ? undefined : { backgroundImage: placeholderGradient(channel.name) }}
    >
      {src ? (
        <img
          src={src}
          alt=""
          loading="lazy"
          decoding="async"
          draggable={false}
          onError={() => setFailed(true)}
          className="max-h-[78%] max-w-[82%] object-contain drop-shadow-[0_1px_6px_rgba(0,0,0,0.45)]"
        />
      ) : (
        <span className="line-clamp-2 px-1.5 text-center text-xs font-bold leading-tight text-white/80 [text-wrap:balance]">
          {showName ? channel.name : (channel.number ?? channel.name.slice(0, 3))}
        </span>
      )}
    </div>
  );
});

export type ChannelFilter = { kind: "all" } | { kind: "favorites" } | { kind: "group"; group: string };

export function filterKey(f: ChannelFilter): string {
  return f.kind === "group" ? `group:${f.group}` : f.kind;
}

export function parseFilter(key: string): ChannelFilter {
  if (key === "favorites") return { kind: "favorites" };
  if (key.startsWith("group:")) return { kind: "group", group: key.slice(6) };
  return { kind: "all" };
}

/** Channels that pass `filter` and match `search` (name, number, or what's on). */
export function useFilteredChannels(channels: Channel[] | undefined, filter: ChannelFilter, search: string): Channel[] {
  return useMemo(() => {
    const q = search.trim().toLowerCase();
    return (channels ?? []).filter((c) => {
      if (filter.kind === "favorites" && !c.favorite) return false;
      if (filter.kind === "group" && c.group !== filter.group) return false;
      if (!q) return true;
      return (
        c.name.toLowerCase().includes(q) ||
        c.number === q ||
        (c.number?.startsWith(q) ?? false) ||
        (c.now?.title.toLowerCase().includes(q) ?? false)
      );
    });
  }, [channels, filter, search]);
}

/** The groups channels are in, in their order, with how many each has. */
export function useGroups(channels: Channel[] | undefined): { name: string; count: number }[] {
  return useMemo(() => {
    const counts = new Map<string, number>();
    for (const c of channels ?? []) {
      if (c.group) counts.set(c.group, (counts.get(c.group) ?? 0) + 1);
    }
    return [...counts].map(([name, count]) => ({ name, count }));
  }, [channels]);
}

/** `/live/:id`. */
export function liveUrl(channelId: number): string {
  return `/live/${channelId}`;
}
