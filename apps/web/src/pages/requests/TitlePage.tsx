import { useCallback, useLayoutEffect, useState } from "react";
import { Link, useParams } from "react-router";
import { Clock, Play, Plus } from "lucide-react";
import { isApiError } from "@/api/client";
import {
  mayRequest,
  requestsUnavailable,
  useDiscoverTitle,
  useRequestSummary,
  type DiscoverDetail,
  type DiscoverSeason,
  type TitleKind,
} from "@/api/requests";
import { Artwork } from "@/components/Artwork";
import { ArrowButton, useFittedRow } from "@/components/HubRow";
import { PersonPhoto } from "@/components/PersonCard";
import { Badge, Button, buttonClass, SectionTitle, Skeleton } from "@/components/ui";
import { cn } from "@/lib/cn";
import { formatDate, formatDuration, pluralize } from "@/lib/format";
import { smoothScroll, useDocumentTitle } from "@/lib/hooks";
import { itemUrl } from "@/lib/urls";
import { NotFoundPage } from "../NotFoundPage";
import { ContentRating, Genres, MetaLine, RatingBadge, Summary } from "../item/shared";
import { RequestItem } from "./RequestItem";
import { askableSeasons, RequestModal, seasonName, seasonState, wholeInLibrary } from "./RequestModal";
import { allowanceText, kindLabel, LoadError, RequestsUnavailable } from "./shared";

function TitleSkeleton() {
  return (
    <div className="px-4 pb-16 pt-6 sm:px-8 sm:pt-[10vh] lg:pt-[14vh]">
      <div className="flex flex-col gap-8 sm:flex-row sm:gap-10">
        <Skeleton className="hidden aspect-[2/3] w-60 shrink-0 rounded-lg sm:block" />
        <div className="flex-1 space-y-4">
          <Skeleton className="h-10 w-2/3" />
          <Skeleton className="h-4 w-1/3" />
          <Skeleton className="h-12 w-48" />
          <Skeleton className="h-4 w-full max-w-2xl" />
          <Skeleton className="h-4 w-5/6 max-w-2xl" />
        </div>
      </div>
    </div>
  );
}

function Backdrop({ title }: { title: DiscoverDetail }) {
  return (
    <div className="pointer-events-none absolute inset-x-0 top-0 h-[78vh] max-h-[760px] overflow-hidden" aria-hidden>
      <Artwork
        candidates={title.backdrop_url ? [title.backdrop_url] : []}
        width={1280}
        alt=""
        title={title.title}
        bare
        eager
        className="size-full opacity-70"
        imgClassName="object-cover object-[center_20%]"
      />
      <div className="absolute inset-0 bg-linear-to-t from-bg via-bg/75 to-bg/10" />
      <div className="absolute inset-0 bg-linear-to-r from-bg/95 via-bg/50 to-transparent" />
    </div>
  );
}

/** "Nov 4, 2026" for a release still to come, else its year. */
function released(title: DiscoverDetail): string | null {
  const date = title.release_date;
  if (date && new Date(`${date}T00:00:00`).getTime() > Date.now()) return `Coming ${formatDate(date)}`;
  return title.year ? String(title.year) : null;
}

function SeasonRow({ season }: { season: DiscoverSeason }) {
  const whole = wholeInLibrary(season);
  return (
    <li className="flex items-center gap-3 rounded-lg border border-border bg-surface/80 px-3 py-2.5">
      <Artwork
        candidates={season.poster_url ? [season.poster_url] : []}
        width={40}
        alt=""
        title=""
        bare
        className="aspect-[2/3] w-10 shrink-0 rounded ring-1 ring-white/5"
      />
      <div className="min-w-0 flex-1">
        <div className="truncate text-sm font-medium">{seasonName(season)}</div>
        <div className="truncate text-xs text-subtle">
          {[pluralize(season.episodes, "episode"), season.air_date ? season.air_date.slice(0, 4) : null].filter(Boolean).join(" · ")}
        </div>
      </div>
      {whole ? (
        <Badge tone="success">In library</Badge>
      ) : season.requested ? (
        <Badge>
          <Clock className="size-3" /> Requested
        </Badge>
      ) : (
        <span className="shrink-0 text-xs text-subtle">{seasonState(season)}</span>
      )}
    </li>
  );
}

/** TMDB's cast: no pages of their own here (people have them only in a library). */
function CastRow({ cast }: { cast: DiscoverDetail["cast"] }) {
  const { ref, width, perPage, gap } = useFittedRow([96, 120]);
  const [canLeft, setCanLeft] = useState(false);
  const [canRight, setCanRight] = useState(false);

  const update = useCallback(() => {
    const el = ref.current;
    if (!el) return;
    setCanLeft(el.scrollLeft > 4);
    setCanRight(el.scrollLeft + el.clientWidth < el.scrollWidth - 4);
  }, [ref]);

  useLayoutEffect(() => {
    update();
  }, [update, cast.length, width]);

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
        Cast
      </SectionTitle>
      <div
        ref={ref}
        onScroll={update}
        className="scrollbar-none -mx-1 flex snap-x snap-mandatory scroll-px-1 gap-3.5 overflow-x-auto px-1 pt-1 pb-2 sm:gap-4"
      >
        {cast.map((c, i) => (
          <div key={`${c.name}-${i}`} className="shrink-0 snap-start text-center" style={{ width }}>
            <PersonPhoto name={c.name} url={c.photo_url} size={width} className="w-full" />
            <div className="mt-2 truncate px-0.5 text-sm font-medium" title={c.name}>
              {c.name}
            </div>
            {c.character ? (
              <div className="truncate px-0.5 text-xs text-muted" title={c.character}>
                {c.character}
              </div>
            ) : (
              <div className="h-4" />
            )}
          </div>
        ))}
      </div>
    </section>
  );
}

function TitleView({ title }: { title: DiscoverDetail }) {
  const summary = useRequestSummary();
  const [asking, setAsking] = useState(false);
  const show = title.kind === "show";
  const askable = askableSeasons(title.seasons);
  const numbered = (title.seasons ?? []).filter((s) => s.number > 0);
  const open = mayRequest(summary.data);
  // A movie no library has and nothing asks for; a show with a season still to ask for.
  const canAsk = open && (show ? askable.length > 0 : title.status === "none");
  const left = show ? summary.data?.seasons_left : summary.data?.movies_left;
  const usedUp = left === 0;
  const allowance = summary.data && canAsk ? allowanceText(summary.data) : null;

  return (
    <div className="relative min-h-[calc(100vh-4rem)] animate-fade-in">
      <Backdrop title={title} />
      <div className="relative px-4 pb-16 pt-6 sm:px-8 sm:pt-[10vh] lg:pt-[14vh]">
        <div className="flex flex-col gap-6 sm:flex-row sm:gap-10">
          <div className="hidden w-52 shrink-0 sm:block md:w-60 lg:w-64">
            <Artwork
              candidates={title.poster_url ? [title.poster_url] : []}
              width={256}
              alt={title.title}
              title={title.title}
              eager
              className="aspect-[2/3] w-full rounded-lg shadow-2xl shadow-black/60 ring-1 ring-white/10"
            />
          </div>
          <div className="min-w-0 flex-1 space-y-4">
            <div>
              <h1 className="text-3xl font-bold leading-tight tracking-tight text-balance sm:text-4xl">{title.title}</h1>
              {title.original_title ? <p className="mt-1 text-sm text-muted">{title.original_title}</p> : null}
            </div>
            <MetaLine
              parts={[
                kindLabel(title.kind),
                released(title),
                title.runtime_minutes ? formatDuration(title.runtime_minutes * 60_000) : null,
                show && numbered.length ? pluralize(numbered.length, "season") : null,
                title.certification ? <ContentRating key="cr" value={title.certification} /> : null,
                title.rating != null && title.rating > 0 ? <RatingBadge key="r" rating={title.rating} /> : null,
                title.network,
                title.show_status,
              ]}
            />
            <Genres genres={title.genres} />
            <div className="flex flex-wrap items-center gap-2.5 pt-1">
              {title.item_id !== null ? (
                <Link to={itemUrl(title.item_id)} className={buttonClass("primary", "lg")}>
                  <Play className="size-5 fill-current" />
                  Open in library
                </Link>
              ) : null}
              {canAsk ? (
                <Button
                  variant={title.item_id !== null ? "secondary" : "primary"}
                  size="lg"
                  icon={<Plus className="size-5" />}
                  disabled={usedUp}
                  onClick={() => setAsking(true)}
                >
                  {title.status === "none" ? "Request" : "Request more"}
                </Button>
              ) : title.status === "requested" ? (
                <span className={buttonClass("secondary", "lg", "pointer-events-none text-muted")}>
                  <Clock className="size-5" />
                  Requested
                </span>
              ) : title.status === "library" && title.item_id === null ? (
                <Badge tone="success">In a library</Badge>
              ) : null}
            </div>
            {allowance ? <p className={cn("text-xs", usedUp ? "text-danger" : "text-subtle")}>{allowance}</p> : null}
            {title.tagline ? <p className="text-base italic text-fg/75">{title.tagline}</p> : null}
            <Summary text={title.overview} />
          </div>
        </div>

        {title.requests.length ? (
          <section className="mt-12">
            <SectionTitle>
              {title.requests.length === 1 ? "Request" : "Requests"}{" "}
              {title.requests.length > 1 ? <span className="ml-1 text-sm font-normal text-subtle">{title.requests.length}</span> : null}
            </SectionTitle>
            <ul className="divide-y divide-white/[0.05] overflow-hidden rounded-lg border border-border bg-surface/80">
              {title.requests.map((r) => (
                <RequestItem key={r.id} request={r} showTitle={false} />
              ))}
            </ul>
          </section>
        ) : null}

        {show && title.seasons?.length ? (
          <section className="mt-12">
            <SectionTitle>Seasons</SectionTitle>
            <ul className="grid gap-2 sm:grid-cols-2 xl:grid-cols-3">
              {[...numbered, ...title.seasons.filter((s) => s.number === 0)].map((s) => (
                <SeasonRow key={s.number} season={s} />
              ))}
            </ul>
          </section>
        ) : null}

        {title.cast.length ? <CastRow cast={title.cast} /> : null}

        <p className="mt-12 text-xs text-subtle">Details and pictures from TMDB.</p>
      </div>
      {canAsk ? <RequestModal title={title} summary={summary.data} open={asking} onClose={() => setAsking(false)} /> : null}
    </div>
  );
}

/** A movie or show on TMDB (`/discover/movie/603`): what it is, whether a library has it, and asking for it. */
export function TitlePage() {
  const params = useParams();
  const kind: TitleKind | null = params.kind === "movie" || params.kind === "show" ? params.kind : null;
  const tmdbId = Number(params.tmdbId);
  const valid = kind !== null && Number.isInteger(tmdbId) && tmdbId > 0;
  const title = useDiscoverTitle(kind ?? "movie", valid ? tmdbId : 0);
  useDocumentTitle(title.data?.title);

  if (!valid) return <NotFoundPage />;
  if (title.isPending) return <TitleSkeleton />;
  // Failed before anything came (a refresh that fails keeps what's shown).
  if (!title.data) {
    const why = requestsUnavailable(title.error);
    if (why) return <RequestsUnavailable reason={why} />;
    if (isApiError(title.error, 404)) return <NotFoundPage />;
    return (
      <LoadError
        className="min-h-[60vh]"
        title="Couldn't load this title"
        error={title.error}
        retrying={title.isFetching}
        onRetry={() => void title.refetch()}
      />
    );
  }
  // Keyed, so what was being asked for doesn't carry over to another title.
  return <TitleView key={`${title.data.kind}-${title.data.tmdb_id}`} title={title.data} />;
}
