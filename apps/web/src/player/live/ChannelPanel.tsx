import { useEffect, useRef, useState } from "react";
import { Search, Star, X } from "lucide-react";
import { programProgress, type Channel } from "@/api/livetv";
import { cn } from "@/lib/cn";
import { ChannelLogo, formatTime } from "@/pages/livetv/common";

/** The channels beside the picture: what's on each, to change to one. */
export function ChannelPanel({
  open,
  channels,
  current,
  now,
  onClose,
  onTune,
}: {
  open: boolean;
  channels: Channel[];
  current: number;
  now: number;
  onClose: () => void;
  onTune: (c: Channel) => void;
}) {
  const [search, setSearch] = useState("");
  const [favorites, setFavorites] = useState(false);
  const listRef = useRef<HTMLUListElement>(null);
  const hasFavorites = channels.some((c) => c.favorite);
  const q = search.trim().toLowerCase();
  const shown = channels.filter(
    (c) =>
      (!favorites || c.favorite) &&
      (!q || c.name.toLowerCase().includes(q) || (c.number?.startsWith(q) ?? false) || (c.now?.title.toLowerCase().includes(q) ?? false)),
  );

  // Opens on the channel being watched.
  useEffect(() => {
    if (!open) return;
    const el = listRef.current?.querySelector<HTMLElement>('[aria-current="true"]');
    el?.scrollIntoView({ block: "center" });
    el?.focus({ preventScroll: true });
  }, [open]);

  return (
    <aside
      className={cn(
        "absolute inset-y-0 right-0 z-20 flex w-full max-w-sm flex-col border-l border-white/10 bg-black/90 shadow-2xl transition-transform duration-300 ease-out",
        open ? "translate-x-0" : "pointer-events-none translate-x-full",
      )}
      aria-label="Channels"
      aria-hidden={!open}
      onKeyDown={(e) => {
        // The player's keys stay outside the list.
        if (e.key !== "Escape") e.stopPropagation();
      }}
    >
      <div className="flex items-center gap-2 border-b border-white/10 p-3">
        <div className="relative flex-1">
          <Search className="pointer-events-none absolute left-3 top-1/2 size-4 -translate-y-1/2 text-white/40" />
          <input
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder="Find a channel"
            aria-label="Find a channel"
            tabIndex={open ? 0 : -1}
            className="h-9 w-full rounded-md border border-white/10 bg-white/[0.06] pl-9 pr-3 text-sm text-white placeholder:text-white/40 focus:border-accent focus:outline-none"
          />
        </div>
        {hasFavorites ? (
          <button
            type="button"
            aria-label="Favourites only"
            aria-pressed={favorites}
            tabIndex={open ? 0 : -1}
            onClick={() => setFavorites((f) => !f)}
            className={cn("flex size-9 items-center justify-center rounded-md transition hover:bg-white/10", favorites && "bg-accent/20 text-accent")}
          >
            <Star className={cn("size-4", favorites && "fill-current")} />
          </button>
        ) : null}
        <button
          type="button"
          aria-label="Close"
          tabIndex={open ? 0 : -1}
          onClick={onClose}
          className="flex size-9 items-center justify-center rounded-md transition hover:bg-white/10"
        >
          <X className="size-5" />
        </button>
      </div>
      <ul ref={listRef} className="flex-1 overflow-y-auto overscroll-contain p-2 [scrollbar-width:thin]">
        {shown.map((c) => {
          const on = c.now && c.now.start <= now && c.now.stop > now ? c.now : null;
          const active = c.id === current;
          return (
            <li key={c.id}>
              <button
                type="button"
                aria-current={active}
                tabIndex={open ? 0 : -1}
                onClick={() => onTune(c)}
                className={cn(
                  "flex w-full items-center gap-3 rounded-lg p-2 text-left transition-colors focus:outline-none focus-visible:ring-2 focus-visible:ring-accent",
                  active ? "bg-accent/15" : "hover:bg-white/[0.07]",
                )}
              >
                <span className="w-8 shrink-0 text-right text-xs tabular-nums text-white/50">{c.number ?? ""}</span>
                <ChannelLogo channel={c} width={64} showName={false} className="h-9 w-14 shrink-0 rounded-md bg-white/[0.07]" />
                <span className="min-w-0 flex-1">
                  <span className={cn("block truncate text-sm font-medium", active && "text-accent")}>{c.name}</span>
                  {on ? (
                    <>
                      <span className="block truncate text-xs text-white/60">{on.title}</span>
                      <span className="mt-1 block h-0.5 overflow-hidden rounded-full bg-white/10">
                        <span className="block h-full bg-accent/80" style={{ width: `${programProgress(on, now) * 100}%` }} />
                      </span>
                    </>
                  ) : c.next ? (
                    <span className="block truncate text-xs text-white/45">
                      {formatTime(c.next.start)} · {c.next.title}
                    </span>
                  ) : null}
                </span>
                {c.favorite ? <Star className="size-3.5 shrink-0 fill-accent text-accent" /> : null}
              </button>
            </li>
          );
        })}
        {shown.length === 0 ? <li className="p-6 text-center text-sm text-white/50">No channels match.</li> : null}
      </ul>
    </aside>
  );
}
