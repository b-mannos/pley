import { useCallback, useEffect, useRef, useState } from "react";
import { Link } from "react-router";
import { ChevronLeft, ChevronRight } from "lucide-react";
import type { Hub } from "@/api/types";
import { cn } from "@/lib/cn";
import { useMediaQuery } from "@/lib/hooks";
import { aspectFor, CardSkeleton, MediaCard, type CardAspect } from "./cards";

const WIDTHS: Record<CardAspect, [number, number]> = {
  // [mobile, desktop]
  poster: [118, 160],
  square: [132, 168],
  thumb: [236, 296],
};

export function hubAspect(hub: Hub): CardAspect {
  if (hub.key.startsWith("continue")) return "thumb";
  const first = hub.items[0];
  return first ? aspectFor(first) : "poster";
}

function hubLink(hub: Hub): string | null {
  if (hub.library_id == null) return null;
  if (hub.key.startsWith("recently_added")) return `/library/${hub.library_id}?tab=library&sort=added&order=desc`;
  if (hub.key.startsWith("recently_released")) return `/library/${hub.library_id}?tab=library&sort=release&order=desc`;
  if (hub.key.startsWith("top_rated")) return `/library/${hub.library_id}?tab=library&sort=rating&order=desc`;
  return null;
}

export function HubRow({ hub }: { hub: Hub }) {
  const desktop = useMediaQuery("(min-width: 640px)");
  const aspect = hubAspect(hub);
  const width = WIDTHS[aspect][desktop ? 1 : 0];
  const scroller = useRef<HTMLDivElement>(null);
  const [canLeft, setCanLeft] = useState(false);
  const [canRight, setCanRight] = useState(false);
  const continueStyle = hub.key.startsWith("continue");
  const link = hubLink(hub);

  const update = useCallback(() => {
    const el = scroller.current;
    if (!el) return;
    setCanLeft(el.scrollLeft > 4);
    setCanRight(el.scrollLeft + el.clientWidth < el.scrollWidth - 4);
  }, []);

  useEffect(() => {
    update();
    const el = scroller.current;
    if (!el) return;
    const ro = new ResizeObserver(update);
    ro.observe(el);
    return () => ro.disconnect();
  }, [update, hub.items.length]);

  const scrollBy = (dir: 1 | -1) => {
    const el = scroller.current;
    if (!el) return;
    el.scrollBy({ left: dir * Math.max(el.clientWidth * 0.85, width), behavior: "smooth" });
  };

  if (!hub.items.length) return null;

  return (
    <section className="group/hub relative">
      <div className="mb-3 flex items-end justify-between gap-4 px-4 sm:px-8">
        <h2 className="truncate text-lg font-semibold tracking-tight">
          {link ? (
            <Link to={link} className="transition-colors hover:text-accent">
              {hub.title}
            </Link>
          ) : (
            hub.title
          )}
        </h2>
        {link ? (
          <Link to={link} className="shrink-0 text-xs font-medium text-muted transition-colors hover:text-accent">
            See all
          </Link>
        ) : null}
      </div>
      <div className="relative">
        <div
          ref={scroller}
          onScroll={update}
          className="scrollbar-none flex snap-x snap-mandatory scroll-px-4 gap-3.5 overflow-x-auto px-4 pb-2 sm:scroll-px-8 sm:gap-4 sm:px-8"
        >
          {hub.items.map((item) => (
            <div key={item.id} className="shrink-0 snap-start">
              <MediaCard item={item} aspect={aspect} width={width} fixed continueStyle={continueStyle} />
            </div>
          ))}
        </div>
        <ArrowButton side="left" visible={canLeft} onClick={() => scrollBy(-1)} />
        <ArrowButton side="right" visible={canRight} onClick={() => scrollBy(1)} />
      </div>
    </section>
  );
}

function ArrowButton({ side, visible, onClick }: { side: "left" | "right"; visible: boolean; onClick: () => void }) {
  return (
    <button
      type="button"
      aria-label={side === "left" ? "Scroll left" : "Scroll right"}
      onClick={onClick}
      tabIndex={-1}
      className={cn(
        "absolute top-0 z-10 hidden h-[calc(100%-3.25rem)] w-10 items-center justify-center from-bg/95 to-transparent text-white opacity-0 transition-opacity duration-200 sm:flex",
        side === "left" ? "left-0 bg-linear-to-r" : "right-0 bg-linear-to-l",
        visible ? "group-hover/hub:opacity-100" : "pointer-events-none",
      )}
    >
      {side === "left" ? <ChevronLeft className="size-7" /> : <ChevronRight className="size-7" />}
    </button>
  );
}

export function HubRowSkeleton({ aspect = "poster" }: { aspect?: CardAspect }) {
  const desktop = useMediaQuery("(min-width: 640px)");
  const width = WIDTHS[aspect][desktop ? 1 : 0];
  return (
    <section>
      <div className="mb-3 px-4 sm:px-8">
        <div className="skeleton h-5 w-48 rounded" />
      </div>
      <div className="flex gap-4 overflow-hidden px-4 sm:px-8">
        {Array.from({ length: 8 }, (_, i) => (
          <div key={i} className="shrink-0">
            <CardSkeleton aspect={aspect} width={width} fixed />
          </div>
        ))}
      </div>
    </section>
  );
}
