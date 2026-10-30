import { useEffect, useRef, useState } from "react";
import { Link, useSearchParams } from "react-router";
import { ChevronLeft, Inbox, Search, SearchX, X } from "lucide-react";
import { isApiError } from "@/api/client";
import {
  requestsUnavailable,
  tmdbDown,
  useDiscoverList,
  useDiscoverSearch,
  useRequestSummary,
  type DiscoverList,
  type RequestSummary,
} from "@/api/requests";
import { useAuth } from "@/auth";
import { buttonClass, EmptyState, PageSpinner, Spinner } from "@/components/ui";
import { useDebouncedValue, useDocumentTitle, useOnChange } from "@/lib/hooks";
import { discoverUrl, requestsUrl } from "@/lib/urls";
import { allowanceText, LoadError, RequestsUnavailable, TitleGrid, TitleGridSkeleton, TitleRow, uniqueTitles } from "./shared";

const LISTS: { list: DiscoverList; title: string }[] = [
  { list: "trending", title: "Trending this week" },
  { list: "movies", title: "Popular movies" },
  { list: "shows", title: "Popular shows" },
  { list: "upcoming", title: "Coming to cinemas" },
  { list: "anime", title: "Popular anime" },
];

function listOf(value: string | null): (typeof LISTS)[number] | null {
  return LISTS.find((l) => l.list === value) ?? null;
}

/** Who decides, and how much more may be asked for. */
function intro(summary: RequestSummary, admin: boolean): string {
  const who = admin
    ? "What you ask for is approved as you ask."
    : summary.access === "auto"
      ? "What you ask for is approved right away."
      : "An administrator decides on what you ask for.";
  const allowance = allowanceText(summary);
  return `Find movies and shows to ask for. ${who}${allowance ? ` ${allowance}` : ""}`;
}

/** TMDB's search, kept in the address (`?q=`) as one types. */
function SearchBox({ q }: { q: string }) {
  const [params, setParams] = useSearchParams();
  const [value, setValue] = useState(q);
  const debounced = useDebouncedValue(value, 300);
  const inputRef = useRef<HTMLInputElement>(null);

  // Another query arriving (a link from the libraries' search, Discover in
  // the menu) goes in the box; the one just typed is there already.
  useOnChange(q, (next) => {
    if (next.trim() !== debounced.trim()) setValue(next);
  });

  useEffect(() => {
    const next = debounced.trim();
    if (next === q.trim()) return;
    const p = new URLSearchParams(params);
    if (next) p.set("q", next);
    else p.delete("q");
    p.delete("list");
    setParams(p, { replace: true });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [debounced]);

  return (
    <div className="relative max-w-xl">
      <Search className="pointer-events-none absolute left-3 top-1/2 size-4 -translate-y-1/2 text-subtle" />
      <input
        ref={inputRef}
        type="search"
        value={value}
        onChange={(e) => setValue(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Escape") setValue("");
        }}
        placeholder="Search TMDB for movies and shows"
        aria-label="Search for movies and shows to ask for"
        className="h-11 w-full rounded-full border border-border bg-surface-2 pl-9 pr-9 text-sm text-fg placeholder:text-subtle transition-colors focus:border-accent/70 focus:bg-surface-3 focus:outline-none [&::-webkit-search-cancel-button]:hidden"
      />
      {value ? (
        <button
          type="button"
          aria-label="Clear search"
          onClick={() => {
            setValue("");
            inputRef.current?.focus();
          }}
          className="absolute right-3 top-1/2 -translate-y-1/2 rounded-full p-1 text-subtle hover:text-fg"
        >
          <X className="size-3.5" />
        </button>
      ) : null}
    </div>
  );
}

function SearchResults({ q }: { q: string }) {
  const search = useDiscoverSearch(q);
  const titles = uniqueTitles(search.data?.pages);

  if (search.isPending) return <TitleGridSkeleton />;
  if (search.isError && !search.data) {
    return <LoadError title="Couldn't search TMDB" error={search.error} retrying={search.isFetching} onRetry={() => void search.refetch()} />;
  }
  if (!titles.length) {
    return <EmptyState icon={<SearchX />} title={`Nothing on TMDB for “${q}”`} description="Try another spelling, or the title in its own language." />;
  }
  return (
    <section>
      <div className="mb-3 flex items-center gap-3">
        <h2 className="text-lg font-semibold tracking-tight">Results for “{q}”</h2>
        {search.isFetching && !search.isFetchingNextPage ? <Spinner className="size-4" /> : null}
      </div>
      <TitleGrid
        titles={titles}
        more={{ next: search.hasNextPage, loading: search.isFetchingNextPage, load: () => void search.fetchNextPage() }}
      />
    </section>
  );
}

function ListGrid({ list, title }: { list: DiscoverList; title: string }) {
  const query = useDiscoverList(list);
  const titles = uniqueTitles(query.data?.pages);
  return (
    <section>
      <div className="mb-3 flex items-center gap-2">
        <Link to={discoverUrl()} aria-label="Back to Discover" className="-ml-1.5 rounded-full p-1 text-muted transition-colors hover:bg-white/10 hover:text-fg">
          <ChevronLeft className="size-5" />
        </Link>
        <h2 className="text-lg font-semibold tracking-tight">{title}</h2>
      </div>
      {query.isPending ? (
        <TitleGridSkeleton />
      ) : query.isError && !query.data ? (
        <LoadError title="Couldn't load it" error={query.error} retrying={query.isFetching} onRetry={() => void query.refetch()} />
      ) : (
        <TitleGrid titles={titles} more={{ next: query.hasNextPage, loading: query.isFetchingNextPage, load: () => void query.fetchNextPage() }} />
      )}
    </section>
  );
}

/**
 * TMDB's lists, a row each. TMDB not answering for the first, it says so
 * once rather than in every row.
 */
function DiscoverRows() {
  const first = useDiscoverList("trending");
  if (!first.data && first.isError && tmdbDown(first.error)) {
    return <LoadError title="Couldn't load Discover" error={first.error} retrying={first.isFetching} onRetry={() => void first.refetch()} />;
  }
  return (
    <div className="space-y-8">
      {LISTS.map((l) => (
        <TitleRow key={l.list} list={l.list} title={l.title} link={`/discover?list=${l.list}`} />
      ))}
    </div>
  );
}

/**
 * Discover (`/discover`): TMDB's lists as rows, one of them whole
 * (`?list=`), or what TMDB finds (`?q=`); each title to its page.
 */
export function DiscoverPage() {
  const me = useAuth();
  const [params] = useSearchParams();
  const q = params.get("q") ?? "";
  const list = listOf(params.get("list"));
  const summary = useRequestSummary();
  useDocumentTitle(q.trim() ? `Discover: ${q.trim()}` : "Discover");

  if (summary.isPending) return <PageSpinner />;
  if (!summary.data && !isApiError(summary.error, 404)) {
    const why = requestsUnavailable(summary.error);
    if (why) return <RequestsUnavailable reason={why} />;
    return (
      <LoadError
        className="min-h-[60vh]"
        title="Couldn't load Discover"
        error={summary.error}
        retrying={summary.isFetching}
        onRetry={() => void summary.refetch()}
      />
    );
  }
  // A server without requests answers 404: as good as off.
  if (!summary.data?.enabled) return <RequestsUnavailable reason="setup" />;
  if (summary.data.access === "off") return <RequestsUnavailable reason="forbidden" />;

  return (
    <div className="space-y-8 pb-16 pt-6 sm:pt-8">
      <header className="space-y-4 px-4 sm:px-8">
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div className="min-w-0 max-w-3xl">
            <h1 className="text-2xl font-bold tracking-tight">Discover</h1>
            <p className="mt-1 text-sm text-muted">{intro(summary.data, me.is_admin)}</p>
          </div>
          <Link to={requestsUrl} className={buttonClass("outline", "sm")}>
            <Inbox className="size-4" />
            {me.is_admin ? "Requests" : "Your requests"}
          </Link>
        </div>
        <SearchBox q={q} />
      </header>

      {q.trim() ? (
        <div className="px-4 sm:px-8">
          <SearchResults q={q.trim()} />
        </div>
      ) : list ? (
        <div className="px-4 sm:px-8">
          <ListGrid list={list.list} title={list.title} />
        </div>
      ) : (
        <DiscoverRows />
      )}

      <p className="px-4 text-xs text-subtle sm:px-8">
        Titles, details and pictures from TMDB. This product uses the TMDB API but is not endorsed or certified by TMDB.
      </p>
    </div>
  );
}
