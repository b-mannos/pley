import { memo, useLayoutEffect, useRef, useState } from "react";
import { Link } from "react-router";
import { Info, Play, Star } from "lucide-react";
import { programProgress, useSetFavorite, type Channel, type Program } from "@/api/livetv";
import { cn } from "@/lib/cn";
import { ChannelLogo, formatTime, liveUrl } from "./common";

/** Channels as cards: the logo, what's on with how far it is, what's next. */
export function ChannelGrid({
  channels,
  now,
  onDetails,
}: {
  channels: Channel[];
  now: number;
  onDetails: (channel: Channel, program: Program) => void;
}) {
  return (
    <ul className="grid grid-cols-[repeat(auto-fill,minmax(15.5rem,1fr))] gap-3 sm:gap-4">
      {channels.map((c) => (
        <li key={c.id}>
          <ChannelCard channel={c} now={now} onDetails={onDetails} />
        </li>
      ))}
    </ul>
  );
}

/** One line of cards as wide as the grid's, as many as fit; on phones a row
 *  scrolled sideways. */
export function ChannelRow({
  channels,
  now,
  onDetails,
}: {
  channels: Channel[];
  now: number;
  onDetails: (channel: Channel, program: Program) => void;
}) {
  const ref = useRef<HTMLUListElement>(null);
  const [columns, setColumns] = useState(channels.length);
  useLayoutEffect(() => {
    const ul = ref.current;
    if (!ul) return;
    const measure = () => {
      const style = getComputedStyle(ul);
      // Below sm the row is a flex row, and every card shows.
      setColumns(style.display === "grid" ? style.gridTemplateColumns.split(" ").length : Infinity);
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(ul);
    return () => observer.disconnect();
  }, []);
  return (
    <ul
      ref={ref}
      className="scrollbar-none -mx-4 flex gap-3 overflow-x-auto px-4 pb-1 sm:mx-0 sm:grid sm:grid-cols-[repeat(auto-fill,minmax(15.5rem,1fr))] sm:gap-4 sm:px-0"
    >
      {channels.slice(0, columns).map((c) => (
        <li key={c.id} className="w-60 shrink-0 sm:w-auto">
          <ChannelCard channel={c} now={now} onDetails={onDetails} />
        </li>
      ))}
    </ul>
  );
}

export const ChannelCard = memo(function ChannelCard({
  channel,
  now,
  onDetails,
  className,
}: {
  channel: Channel;
  now: number;
  onDetails: (channel: Channel, program: Program) => void;
  className?: string;
}) {
  const favorite = useSetFavorite();
  const on = channel.now && channel.now.stop > now ? channel.now : null;
  const next = on ? channel.next : (channel.next ?? channel.now);
  return (
    <div
      className={cn(
        "group relative flex h-full flex-col overflow-hidden rounded-lg border border-border bg-surface transition-[border-color,transform,box-shadow] duration-200 hover:-translate-y-0.5 hover:border-border-strong hover:shadow-lg hover:shadow-black/30 focus-within:border-accent",
        className,
      )}
    >
      <Link
        to={liveUrl(channel.id)}
        className="relative block outline-none"
        aria-label={`Watch ${channel.number ? `${channel.number} ` : ""}${channel.name}${on ? `, now: ${on.title}` : ""}`}
      >
        <ChannelLogo
          channel={channel}
          width={200}
          className="aspect-[16/8] w-full bg-gradient-to-br from-surface-3 to-surface-2"
        />
        <div className="absolute inset-0 flex items-center justify-center bg-black/55 opacity-0 transition-opacity duration-200 group-hover:opacity-100 group-focus-within:opacity-100">
          <span className="flex size-12 items-center justify-center rounded-full bg-accent text-accent-fg shadow-lg shadow-black/40">
            <Play className="ml-0.5 size-5 fill-current" />
          </span>
        </div>
        {channel.number ? (
          <span className="absolute left-2 top-2 rounded bg-black/60 px-1.5 py-0.5 text-xs font-semibold tabular-nums text-white/90 backdrop-blur-sm">
            {channel.number}
          </span>
        ) : null}
      </Link>
      <button
        type="button"
        onClick={() => favorite.mutate({ id: channel.id, favorite: !channel.favorite })}
        aria-label={channel.favorite ? `Remove ${channel.name} from favourites` : `Add ${channel.name} to favourites`}
        aria-pressed={channel.favorite}
        className={cn(
          "absolute right-2 top-2 flex size-8 items-center justify-center rounded-full bg-black/50 text-white/80 backdrop-blur-sm transition-[opacity,color] hover:text-white",
          channel.favorite ? "opacity-100" : "opacity-0 group-hover:opacity-100 focus-visible:opacity-100",
        )}
      >
        <Star className={cn("size-4", channel.favorite && "fill-accent text-accent")} />
      </button>
      <div className="flex min-h-0 flex-1 flex-col gap-1.5 p-3">
        <div className="truncate text-sm font-semibold" title={channel.name}>
          {channel.name}
        </div>
        {on ? (
          <button
            type="button"
            onClick={() => onDetails(channel, on)}
            className="-mx-1 -my-0.5 flex items-start gap-1 rounded px-1 py-0.5 text-left text-sm text-fg/90 transition-colors hover:bg-white/[0.05]"
          >
            <span className="line-clamp-1 flex-1">{on.title}</span>
            <Info className="mt-0.5 size-3.5 shrink-0 text-subtle opacity-0 transition-opacity group-hover:opacity-100" />
          </button>
        ) : (
          <div className="text-sm text-subtle">{channel.has_guide ? "Nothing in the guide now" : "No guide"}</div>
        )}
        {on ? (
          <div className="flex items-center gap-2">
            <div className="h-1 flex-1 overflow-hidden rounded-full bg-white/10">
              <div
                className="h-full rounded-full bg-accent transition-[width] duration-700"
                style={{ width: `${programProgress(on, now) * 100}%` }}
              />
            </div>
            <span className="shrink-0 text-[11px] tabular-nums text-subtle">{formatTime(on.stop)}</span>
          </div>
        ) : null}
        {next ? (
          <button
            type="button"
            onClick={() => onDetails(channel, next)}
            className="mt-auto truncate pt-1 text-left text-xs text-subtle transition-colors hover:text-muted"
          >
            <span className="tabular-nums">{formatTime(next.start)}</span> · {next.title}
          </button>
        ) : null}
      </div>
    </div>
  );
});
