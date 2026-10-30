import { memo, useCallback, useLayoutEffect, useState, type ComponentProps, type ReactNode } from "react";
import { Link } from "react-router";
import { Ban, Check, CircleAlert, CircleCheck, Clock, CloudOff, Inbox, LoaderCircle } from "lucide-react";
import { errorMessage } from "@/api/client";
import {
  tmdbDown,
  useDiscoverList,
  type DiscoverList,
  type DiscoverTitle,
  type MediaRequest,
  type RequestDownload,
  type RequestStatus,
  type RequestSummary,
} from "@/api/requests";
import { useAuth } from "@/auth";
import { Artwork } from "@/components/Artwork";
import { ASPECT_CLASS, CardSkeleton } from "@/components/cards";
import { ArrowButton, HubRowSkeleton, useFittedRow } from "@/components/HubRow";
import { Badge, Button, buttonClass, EmptyState, ProgressBar } from "@/components/ui";
import { cn } from "@/lib/cn";
import { formatRelative, pluralize } from "@/lib/format";
import { smoothScroll, useElementWidth } from "@/lib/hooks";
import { titleUrl } from "@/lib/urls";

/*
 * What Discover, a title's page and Requests share: TMDB's titles as
 * cards, rows and grids, and requests' statuses and downloads.
 */

export function kindLabel(kind: DiscoverTitle["kind"]): string {
  return kind === "movie" ? "Movie" : "Show";
}

/** "Season 2", "Seasons 1–3, 5", "Seasons 1–2 and specials", "Specials"; "" for none (movies). */
export function seasonsLabel(seasons: number[] | null): string {
  if (!seasons?.length) return "";
  const numbers = [...new Set(seasons.filter((n) => n > 0))].sort((a, b) => a - b);
  const ranges: string[] = [];
  let start: number | null = null;
  let prev = 0;
  const close = () => {
    if (start !== null) ranges.push(start === prev ? `${start}` : `${start}–${prev}`);
  };
  for (const n of numbers) {
    if (start !== null && n === prev + 1) {
      prev = n;
      continue;
    }
    close();
    start = n;
    prev = n;
  }
  close();
  if (!ranges.length) return "Specials";
  const label = `${numbers.length === 1 ? "Season" : "Seasons"} ${ranges.join(", ")}`;
  return seasons.includes(0) ? `${label} and specials` : label;
}

/**
 * What the account may still ask for in the 7 days ("You can ask for 2 more
 * movies and 5 more seasons this week."), and when it may ask for more of
 * what it has used up; `null` without a limit.
 */
export function allowanceText(summary: RequestSummary): string | null {
  const more: string[] = [];
  const none: { what: string; at: number | null }[] = [];
  if (summary.movies_left !== null) {
    if (summary.movies_left > 0) more.push(pluralize(summary.movies_left, "more movie", "more movies"));
    else none.push({ what: "movies", at: summary.movies_next_at });
  }
  if (summary.seasons_left !== null) {
    if (summary.seasons_left > 0) more.push(pluralize(summary.seasons_left, "more season", "more seasons"));
    else none.push({ what: "seasons", at: summary.seasons_next_at });
  }
  if (more.length) {
    const rest = none.map(({ what, at }) => (at ? `, and for more ${what} ${formatRelative(at)}` : `, and no more ${what}`));
    return `You can ask for ${more.join(" and ")} this week${rest.join("")}.`;
  }
  const [first, second] = none;
  if (first && !second) {
    return `You've asked for all the ${first.what} you can this week${first.at ? `: you can ask for more ${formatRelative(first.at)}` : ""}.`;
  }
  if (second) {
    const when = none.flatMap(({ what, at }) => (at ? [`more ${what} ${formatRelative(at)}`] : []));
    return `You've asked for all you can this week${when.length ? `: you can ask for ${when.join(" and ")}` : ""}.`;
  }
  return null;
}

/** In a library, or asked for already: how a title stands for this account (nothing for neither). */
function TitleStatusBadge({ status }: { status: DiscoverTitle["status"] }) {
  if (status === "none") return null;
  return (
    <span className="absolute left-1.5 top-1.5 inline-flex items-center gap-1 rounded bg-black/80 px-1.5 py-0.5 text-[10px] font-semibold leading-tight tracking-wide text-white shadow">
      {status === "library" ? <Check className="size-3 text-success" strokeWidth={3} /> : <Clock className="size-3 text-accent" strokeWidth={2.5} />}
      {status === "library" ? "In library" : "Requested"}
    </span>
  );
}

/** A TMDB title's poster, to its page, saying whether a library has it or it's asked for. */
export const TitleCard = memo(function TitleCard({ title, width, fixed }: { title: DiscoverTitle; width: number; fixed?: boolean }) {
  const to = titleUrl(title.kind, title.tmdb_id);
  const subtitle = [title.year, kindLabel(title.kind)].filter(Boolean).join(" · ");
  return (
    <div className="group/card min-w-0" style={fixed ? { width } : undefined}>
      <Link
        to={to}
        aria-label={title.title}
        className="relative block overflow-hidden rounded-md bg-surface-2 outline-none ring-1 ring-white/5 transition duration-200 hover:ring-2 hover:ring-accent focus-visible:ring-2 focus-visible:ring-accent"
      >
        <Artwork
          candidates={title.poster_url ? [title.poster_url] : []}
          width={width}
          alt=""
          title={title.title}
          className={cn(ASPECT_CLASS.poster, "w-full")}
          imgClassName="transition-transform duration-300 motion-safe:group-hover/card:scale-[1.04]"
        />
        <div className="pointer-events-none absolute inset-0 bg-black/0 transition-colors duration-200 group-hover/card:bg-black/30" />
        <TitleStatusBadge status={title.status} />
      </Link>
      <div className="mt-2 px-0.5">
        {/* The poster above is the same link: Tab stops there once, not twice. */}
        <Link to={to} tabIndex={-1} className="block truncate text-sm font-medium text-fg transition-colors hover:text-accent" title={title.title}>
          {title.title}
        </Link>
        <div className="truncate text-xs text-muted">{subtitle}</div>
      </div>
    </div>
  );
});

/** Pages of titles as one list, each once (TMDB's order can shift between pages). */
export function uniqueTitles(pages: { results: DiscoverTitle[] }[] | undefined): DiscoverTitle[] {
  const seen = new Set<string>();
  const out: DiscoverTitle[] = [];
  for (const page of pages ?? []) {
    for (const t of page.results) {
      const key = `${t.kind}-${t.tmdb_id}`;
      if (seen.has(key)) continue;
      seen.add(key);
      out.push(t);
    }
  }
  return out;
}

const GRID_GAP = 18;
const GRID_CARD = 150;

function useGridColumns() {
  const [ref, width] = useElementWidth<HTMLDivElement>();
  const cols = width > 0 ? Math.max(2, Math.floor((width + GRID_GAP) / (GRID_CARD + GRID_GAP))) : 6;
  const colWidth = width > 0 ? (width - GRID_GAP * (cols - 1)) / cols : GRID_CARD;
  return { ref, cols, colWidth };
}

/** Titles in whole columns as wide as the page; `more` loads the next page. */
export function TitleGrid({
  titles,
  more,
}: {
  titles: DiscoverTitle[];
  more?: { next: boolean; loading: boolean; load: () => void };
}) {
  const { ref, cols, colWidth } = useGridColumns();
  return (
    <div>
      <div ref={ref} className="grid gap-x-[18px] gap-y-6" style={{ gridTemplateColumns: `repeat(${cols}, minmax(0, 1fr))` }}>
        {titles.map((t) => (
          <TitleCard key={`${t.kind}-${t.tmdb_id}`} title={t} width={colWidth} />
        ))}
      </div>
      {more?.next ? (
        <div className="mt-8 flex justify-center">
          <Button variant="secondary" loading={more.loading} onClick={more.load}>
            Show more
          </Button>
        </div>
      ) : null}
    </div>
  );
}

export function TitleGridSkeleton({ count = 12 }: { count?: number }) {
  const { ref, cols } = useGridColumns();
  return (
    <div ref={ref} className="grid gap-x-[18px] gap-y-6" style={{ gridTemplateColumns: `repeat(${cols}, minmax(0, 1fr))` }}>
      {Array.from({ length: count }, (_, i) => (
        <CardSkeleton key={i} aspect="poster" />
      ))}
    </div>
  );
}

/** One of TMDB's lists as a row that scrolls a page of cards at a time, with a link to all of it. */
export function TitleRow({ list, title, link }: { list: DiscoverList; title: string; link: string }) {
  const query = useDiscoverList(list);
  if (query.isPending) return <HubRowSkeleton />;
  if (!query.data) {
    return (
      <section className="px-4 sm:px-8">
        <h2 className="mb-2 text-lg font-semibold tracking-tight">{title}</h2>
        <div role="alert" className="flex flex-wrap items-center gap-3 text-sm">
          <span className="text-danger">
            {tmdbDown(query.error) ? errorMessage(query.error) : `Couldn't load it: ${errorMessage(query.error)}`}
          </span>
          <Button size="sm" onClick={() => void query.refetch()}>
            Retry
          </Button>
        </div>
      </section>
    );
  }
  const titles = query.data.pages[0]?.results ?? [];
  return titles.length ? <TitleRowCards titles={titles} title={title} link={link} /> : null;
}

/** (Mounted with its cards, so the row is measured.) */
function TitleRowCards({ titles, title, link }: { titles: DiscoverTitle[]; title: string; link: string }) {
  const { ref: scroller, width, perPage, gap } = useFittedRow([118, 160]);
  const [canLeft, setCanLeft] = useState(false);
  const [canRight, setCanRight] = useState(false);

  const update = useCallback(() => {
    const el = scroller.current;
    if (!el) return;
    setCanLeft(el.scrollLeft > 4);
    setCanRight(el.scrollLeft + el.clientWidth < el.scrollWidth - 4);
  }, [scroller]);

  useLayoutEffect(() => {
    update();
  }, [update, titles.length, width]);

  const scrollPage = (dir: 1 | -1) => scroller.current?.scrollBy({ left: dir * perPage * (width + gap), behavior: smoothScroll() });

  return (
    <section>
      <div className="mb-2 flex items-center justify-between gap-4 px-4 sm:px-8">
        <h2 className="truncate text-lg font-semibold tracking-tight">
          <Link to={link} className="transition-colors hover:text-accent">
            {title}
          </Link>
        </h2>
        <div className="flex shrink-0 items-center gap-1">
          <Link to={link} className="mr-2 text-xs font-medium text-muted transition-colors hover:text-accent">
            See all
          </Link>
          {canLeft || canRight ? (
            <>
              <ArrowButton side="left" enabled={canLeft} onClick={() => scrollPage(-1)} />
              <ArrowButton side="right" enabled={canRight} onClick={() => scrollPage(1)} />
            </>
          ) : null}
        </div>
      </div>
      <div className="px-4 sm:px-8">
        <div
          ref={scroller}
          onScroll={update}
          className="scrollbar-none -mx-1 flex snap-x snap-mandatory scroll-px-1 gap-3.5 overflow-x-auto px-1 pt-1 pb-2 sm:gap-4"
        >
          {titles.map((t) => (
            <div key={`${t.kind}-${t.tmdb_id}`} className="shrink-0 snap-start">
              <TitleCard title={t} width={width} fixed />
            </div>
          ))}
        </div>
      </div>
    </section>
  );
}

type Tone = ComponentProps<typeof Badge>["tone"];

export const STATUS_LABELS: Record<RequestStatus, string> = {
  pending: "Pending",
  approved: "Approved",
  declined: "Declined",
  failed: "Failed",
  available: "Available",
};

const STATUS_LOOK: Record<RequestStatus, { tone: Tone; icon: ReactNode }> = {
  pending: { tone: "neutral", icon: <Clock className="size-3" /> },
  approved: { tone: "info", icon: <Check className="size-3" strokeWidth={3} /> },
  declined: { tone: "danger", icon: <Ban className="size-3" /> },
  failed: { tone: "danger", icon: <CircleAlert className="size-3" /> },
  available: { tone: "success", icon: <CircleCheck className="size-3" /> },
};

export function RequestStatusBadge({ status }: { status: RequestStatus }) {
  const look = STATUS_LOOK[status];
  return (
    <Badge tone={look.tone}>
      {look.icon}
      {STATUS_LABELS[status]}
    </Badge>
  );
}

const DOWNLOAD_STATES: Record<RequestDownload["state"], string> = {
  queued: "Waiting to download",
  downloading: "Downloading",
  paused: "Download paused",
  importing: "Adding to the library",
  warning: "The download has a problem",
};

/** "in 20 minutes", for a time to come; `null` for one gone by. */
function etaText(eta: number | null): string | null {
  return eta && eta > Date.now() / 1000 ? formatRelative(eta) : null;
}

/** A download in Radarr or Sonarr: what it's doing, how far along, and the queue's estimate. */
export function DownloadProgress({ download, className }: { download: RequestDownload; className?: string }) {
  const eta = etaText(download.eta);
  const problem = download.state === "warning";
  return (
    <div className={cn("space-y-1.5", className)}>
      <div className="flex flex-wrap items-center justify-between gap-x-3 gap-y-0.5 text-xs">
        <span className={cn("flex items-center gap-1.5 font-medium", problem ? "text-danger" : "text-fg/90")}>
          {download.state === "downloading" || download.state === "importing" ? (
            <LoaderCircle className="size-3 animate-spin text-accent" aria-hidden />
          ) : null}
          {DOWNLOAD_STATES[download.state]}
          {download.episodes > 1 ? <span className="font-normal text-subtle">· {pluralize(download.episodes, "episode")}</span> : null}
        </span>
        <span className="tabular-nums text-subtle">
          {Math.round(download.progress * 100)}%{eta ? ` · done ${eta}` : ""}
        </span>
      </div>
      <ProgressBar
        value={download.progress}
        className="h-1.5"
        barClassName={problem ? "bg-danger" : download.state === "paused" ? "bg-subtle" : undefined}
      />
    </div>
  );
}

/** What couldn't be loaded, and Retry; TMDB not answering says so, as asking again may well work. */
export function LoadError({
  title,
  error,
  onRetry,
  retrying,
  className,
}: {
  title: string;
  error: unknown;
  onRetry: () => void;
  retrying?: boolean;
  className?: string;
}) {
  const tmdb = tmdbDown(error);
  return (
    <EmptyState
      className={className}
      icon={tmdb ? <CloudOff /> : undefined}
      title={tmdb ? "TMDB isn't answering" : title}
      description={errorMessage(error)}
      action={
        <Button loading={retrying} onClick={onRetry}>
          Retry
        </Button>
      }
    />
  );
}

/** Why this account can't look for titles here: requests are off, or not for it. */
export function RequestsUnavailable({ reason }: { reason: "setup" | "forbidden" }) {
  const user = useAuth();
  if (reason === "forbidden") {
    return (
      <EmptyState
        className="min-h-[60vh]"
        icon={<Ban />}
        title="Requests aren't open to this account"
        description="An administrator can let it ask for movies and shows."
      />
    );
  }
  return (
    <EmptyState
      className="min-h-[60vh]"
      icon={<Inbox />}
      title="Requests are off"
      description={
        user.is_admin
          ? "Turn them on to let people ask for movies and shows, and send what you approve to Radarr and Sonarr."
          : "This server doesn't take requests for movies and shows."
      }
      action={
        user.is_admin ? (
          <Link to="/settings/requests" className={buttonClass("primary")}>
            Set up requests
          </Link>
        ) : undefined
      }
    />
  );
}

/**
 * What this account may do with a request. Administrators decide on those
 * waiting or failed, and delete any; a maker takes back their own while
 * it isn't approved, and clears away a declined one.
 */
export function requestActions(request: MediaRequest, me: { id: number; is_admin: boolean }) {
  const own = request.requested_by.id === me.id;
  const waiting = request.status === "pending" || request.status === "failed";
  return {
    approve: me.is_admin && request.status === "pending",
    retry: me.is_admin && request.status === "failed",
    decline: me.is_admin && waiting,
    withdraw: own && !me.is_admin && waiting,
    remove: me.is_admin || (own && request.status === "declined"),
  };
}
