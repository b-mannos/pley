import { useCallback, useLayoutEffect, useState } from "react";
import type { ItemSummary } from "@/api/types";
import { MediaCard } from "@/components/cards";
import { ArrowButton, useFittedRow } from "@/components/HubRow";
import { SectionTitle } from "@/components/ui";
import { smoothScroll } from "@/lib/hooks";
import { formatDuration } from "@/lib/format";
import { playUrl } from "@/lib/usePlayItem";

const EXTRA_LABELS: Record<string, string> = {
  trailer: "Trailer",
  behind_the_scenes: "Behind the Scenes",
  deleted_scene: "Deleted Scene",
  featurette: "Featurette",
  interview: "Interview",
  scene: "Scene",
  short: "Short",
  other: "Extra",
};

/** "Trailer · 2m" */
export function extraSubtitle(item: Pick<ItemSummary, "extra_type" | "duration_ms">): string {
  const label = EXTRA_LABELS[item.extra_type ?? "other"] ?? "Extra";
  return [label, formatDuration(item.duration_ms)].filter(Boolean).join(" · ");
}

/** Smallest card width: [mobile, desktop]. */
const CARD_WIDTHS: [number, number] = [200, 260];

/** Trailers and other bonus material; a card plays its extra. */
export function ExtrasRow({ extras }: { extras?: ItemSummary[] }) {
  const { ref, width, perPage, gap } = useFittedRow(CARD_WIDTHS);
  const [canLeft, setCanLeft] = useState(false);
  const [canRight, setCanRight] = useState(false);
  const count = extras?.length ?? 0;

  const update = useCallback(() => {
    const el = ref.current;
    if (!el) return;
    setCanLeft(el.scrollLeft > 4);
    setCanRight(el.scrollLeft + el.clientWidth < el.scrollWidth - 4);
  }, [ref]);

  useLayoutEffect(() => {
    update();
  }, [update, count, width]);

  if (!extras?.length) return null;
  const scrollPage = (dir: 1 | -1) => ref.current?.scrollBy({ left: dir * perPage * (width + gap), behavior: smoothScroll() });

  return (
    <section className="mt-12">
      <SectionTitle
        action={
          canLeft || canRight ? (
            <div className="flex shrink-0 items-center gap-1">
              <ArrowButton side="left" enabled={canLeft} onClick={() => scrollPage(-1)} />
              <ArrowButton side="right" enabled={canRight} onClick={() => scrollPage(1)} />
            </div>
          ) : null
        }
      >
        Extras
      </SectionTitle>
      <div
        ref={ref}
        onScroll={update}
        className="scrollbar-none -mx-1 flex snap-x snap-mandatory scroll-px-1 gap-3.5 overflow-x-auto px-1 pt-1 pb-2 sm:gap-4"
      >
        {extras.map((extra) => (
          <div key={extra.id} className="shrink-0 snap-start">
            <MediaCard
              item={extra}
              aspect="thumb"
              width={width}
              fixed
              href={playUrl(extra.id)}
              subtitle={extraSubtitle(extra)}
            />
          </div>
        ))}
      </div>
    </section>
  );
}
