import { useMemo } from "react";
import { useParams, useSearchParams } from "react-router";
import { ArrowDownWideNarrow, ArrowUpNarrowWide, FolderSearch, RefreshCw } from "lucide-react";
import { errorMessage } from "@/api/client";
import { useLibrary, useLibraryFilters, useLibraryHubs, useLibraryItems, useScanLibrary } from "@/api/hooks";
import type { LibraryKind, LibrarySort, SortOrder } from "@/api/types";
import { useAuth } from "@/auth";
import { HubRow, HubRowSkeleton } from "@/components/HubRow";
import { TmdbKeyNotice } from "@/components/MetadataNotice";
import { ItemGrid, ItemGridSkeleton } from "@/components/ItemGrid";
import { CollectionsRow } from "@/components/lists/CollectionCard";
import { LibraryIcon } from "@/components/Sidebar";
import { useToast } from "@/components/Toast";
import { Button, EmptyState, IconButton, Select } from "@/components/ui";
import { NotFoundPage } from "./NotFoundPage";
import { CollectionsTab } from "./collections/CollectionsTab";
import type { CardAspect } from "@/components/cards";
import { cn } from "@/lib/cn";
import { pluralize } from "@/lib/format";
import { useDocumentTitle, useLocalStorage } from "@/lib/hooks";

const SORT_LABELS: Record<LibrarySort, string> = {
  title: "Title",
  year: "Year",
  added: "Date added",
  rating: "Rating",
  release: "Release date",
  duration: "Duration",
  last_played: "Last played",
  user_rating: "Your rating",
};

const SORTS_BY_KIND: Record<LibraryKind, LibrarySort[]> = {
  movies: ["title", "year", "added", "rating", "user_rating", "release", "duration", "last_played"],
  shows: ["title", "year", "added", "rating", "user_rating", "release", "last_played"],
  music: ["title", "added", "user_rating", "last_played"],
};

const ITEM_NOUN: Record<LibraryKind, [string, string]> = {
  movies: ["movie", "movies"],
  shows: ["show", "shows"],
  music: ["artist", "artists"],
};

function defaultOrder(sort: LibrarySort): SortOrder {
  return sort === "title" ? "asc" : "desc";
}

export function LibraryPage() {
  const { id } = useParams();
  const libId = Number(id);
  const lib = useLibrary(libId);
  const [params, setParams] = useSearchParams();
  const rawTab = params.get("tab");
  const tab = rawTab === "recommended" || rawTab === "collections" ? rawTab : "library";
  useDocumentTitle(lib.data?.name);

  const setParam = (updates: Record<string, string | null>) => {
    const next = new URLSearchParams(params);
    for (const [k, v] of Object.entries(updates)) {
      if (v == null || v === "") next.delete(k);
      else next.set(k, v);
    }
    setParams(next, { replace: true });
  };

  if (!Number.isFinite(libId)) return <NotFoundPage />;
  if (lib.isSuccess && !lib.data) return <NotFoundPage />;

  const kind = lib.data?.kind ?? "movies";

  return (
    <div className="pb-12">
      <div className="flex flex-wrap items-end justify-between gap-4 px-4 pb-1 pt-6 sm:px-8">
        <div className="flex min-w-0 items-center gap-3">
          <div className="flex size-10 shrink-0 items-center justify-center rounded-lg bg-surface-2 text-accent">
            <LibraryIcon kind={kind} className="size-5" />
          </div>
          <div className="min-w-0">
            <h1 className="truncate text-2xl font-bold tracking-tight">{lib.data?.name ?? " "}</h1>
          </div>
        </div>
        <div className="flex rounded-lg bg-surface-2 p-1" role="tablist">
          {(kind === "music" ? (["recommended", "library"] as const) : (["recommended", "library", "collections"] as const)).map((t) => (
            <button
              key={t}
              type="button"
              role="tab"
              aria-selected={tab === t}
              onClick={() => setParam({ tab: t === "library" ? null : t })}
              className={cn(
                "rounded-md px-4 py-1.5 text-sm font-medium capitalize transition-colors",
                tab === t ? "bg-surface-3 text-fg shadow" : "text-muted hover:text-fg",
              )}
            >
              {t}
            </button>
          ))}
        </div>
      </div>

      {kind === "movies" ? <TmdbKeyNotice className="mx-4 mt-4 sm:mx-8" /> : null}
      {tab === "recommended" ? (
        <RecommendedTab libraryId={libId} kind={kind} />
      ) : tab === "collections" && kind !== "music" ? (
        <CollectionsTab libraryId={libId} kind={kind} />
      ) : (
        <LibraryTab libraryId={libId} kind={kind} params={params} setParam={setParam} ready={lib.isSuccess} />
      )}
    </div>
  );
}

function RecommendedTab({ libraryId, kind }: { libraryId: number; kind: LibraryKind }) {
  const hubs = useLibraryHubs(libraryId);
  if (hubs.isPending) {
    return (
      <div className="space-y-10 pt-6">
        <HubRowSkeleton />
        <HubRowSkeleton />
      </div>
    );
  }
  if (hubs.isError) {
    return <EmptyState title="Couldn't load recommendations" description={errorMessage(hubs.error)} />;
  }
  const visible = hubs.data.hubs.filter((h) => h.items.length > 0);
  if (!visible.length) {
    return <EmptyState title="Nothing to recommend yet" description="Recommendations appear once the library has some items." />;
  }
  return (
    <div className="space-y-10 pt-6">
      {visible.map((hub) => (
        <HubRow key={hub.key} hub={hub} />
      ))}
      <CollectionsRow libraryId={libraryId} kind={kind} />
    </div>
  );
}

function LibraryTab({
  libraryId,
  kind,
  params,
  setParam,
  ready,
}: {
  libraryId: number;
  kind: LibraryKind;
  params: URLSearchParams;
  setParam: (u: Record<string, string | null>) => void;
  ready: boolean;
}) {
  const user = useAuth();
  const toast = useToast();
  const scan = useScanLibrary();
  const sorts = SORTS_BY_KIND[kind];
  const rawSort = params.get("sort") as LibrarySort | null;
  const sort: LibrarySort = rawSort && sorts.includes(rawSort) ? rawSort : "title";
  const order: SortOrder = params.get("order") === "asc" || params.get("order") === "desc" ? (params.get("order") as SortOrder) : defaultOrder(sort);
  const watchedParam = params.get("watched");
  const watched = watchedParam === "true" ? true : watchedParam === "false" ? false : undefined;
  const genre = params.get("genre") ?? undefined;
  const yearParam = params.get("year");
  const year = yearParam ? Number(yearParam) : undefined;

  const query = useMemo(() => ({ sort, order, watched, genre, year }), [sort, order, watched, genre, year]);
  const items = useLibraryItems(libraryId, query, ready);
  const filters = useLibraryFilters(libraryId, ready && kind !== "music");

  const aspect: CardAspect = kind === "music" ? "square" : "poster";
  const [cardWidth, setCardWidth] = useLocalStorage<number>(`pley.cardWidth.${aspect}`, aspect === "square" ? 170 : 160);
  const noun = ITEM_NOUN[kind];
  const filtered = watched !== undefined || !!genre || year !== undefined;

  return (
    <>
      <div className="sticky top-16 z-10 mt-3 border-b border-white/[0.04] bg-bg/85 px-4 py-2.5 backdrop-blur-md sm:px-8">
        <div className="scrollbar-none -mx-4 flex items-center gap-2 overflow-x-auto px-4 sm:mx-0 sm:px-0 [&>*]:shrink-0">
          <Select
            aria-label="Sort by"
            value={sort}
            onChange={(e) => {
              const s = e.target.value as LibrarySort;
              setParam({ sort: s === "title" ? null : s, order: null });
            }}
          >
            {sorts.map((s) => (
              <option key={s} value={s}>
                {SORT_LABELS[s]}
              </option>
            ))}
          </Select>
          <IconButton
            size="sm"
            label={order === "asc" ? "Ascending" : "Descending"}
            onClick={() => setParam({ order: order === "asc" ? "desc" : "asc" })}
            className="rounded-md bg-surface-2 hover:bg-surface-3"
          >
            {order === "asc" ? <ArrowUpNarrowWide className="size-4" /> : <ArrowDownWideNarrow className="size-4" />}
          </IconButton>
          {kind !== "music" ? (
            <Select
              aria-label="Watched filter"
              value={watched === undefined ? "all" : watched ? "watched" : "unwatched"}
              onChange={(e) => {
                const v = e.target.value;
                setParam({ watched: v === "all" ? null : v === "watched" ? "true" : "false" });
              }}
            >
              <option value="all">All {noun[1]}</option>
              <option value="unwatched">Unwatched</option>
              <option value="watched">Watched</option>
            </Select>
          ) : null}
          {kind !== "music" && filters.data && filters.data.genres.length > 0 ? (
            <Select aria-label="Genre" value={genre ?? ""} onChange={(e) => setParam({ genre: e.target.value || null })}>
              <option value="">All genres</option>
              {filters.data.genres.map((g) => (
                <option key={g} value={g}>
                  {g}
                </option>
              ))}
            </Select>
          ) : null}
          {kind !== "music" && filters.data && filters.data.years.length > 0 ? (
            <Select aria-label="Year" value={year ?? ""} onChange={(e) => setParam({ year: e.target.value || null })}>
              <option value="">All years</option>
              {filters.data.years.map((y) => (
                <option key={y} value={y}>
                  {y}
                </option>
              ))}
            </Select>
          ) : null}
          {filtered ? (
            <Button size="sm" variant="ghost" onClick={() => setParam({ watched: null, genre: null, year: null })}>
              Clear filters
            </Button>
          ) : null}

          <div className="ml-auto flex items-center gap-3">
            <span className="hidden text-sm tabular-nums text-muted sm:inline">
              {items.data ? pluralize(items.data.total, noun[0], noun[1]) : ""}
            </span>
            <input
              type="range"
              min={110}
              max={260}
              step={10}
              value={cardWidth}
              onChange={(e) => setCardWidth(Number(e.target.value))}
              aria-label="Card size"
              title="Card size"
              className="hidden w-24 md:block"
            />
          </div>
        </div>
      </div>

      <div className="px-4 pt-5 sm:px-8">
        {items.isPending || !ready ? (
          <ItemGridSkeleton aspect={aspect} cardWidth={cardWidth} />
        ) : items.isError || !items.data ? (
          <EmptyState title="Couldn't load this library" description={errorMessage(items.error)} action={<Button onClick={() => void items.refetch()}>Retry</Button>} />
        ) : items.data.items.length === 0 ? (
          filtered ? (
            <EmptyState
              title="No matches"
              description="Nothing in this library matches the current filters."
              action={<Button onClick={() => setParam({ watched: null, genre: null, year: null })}>Clear filters</Button>}
            />
          ) : (
            <EmptyState
              icon={<FolderSearch />}
              title="This library is empty"
              description="pley didn't find any media yet. Check the library folders, then scan again."
              action={
                user.is_admin ? (
                  <Button
                    variant="primary"
                    icon={<RefreshCw className="size-4" />}
                    loading={scan.isPending}
                    onClick={() =>
                      scan.mutate(
                        { id: libraryId },
                        {
                          onSuccess: () => toast.success("Scan started"),
                          onError: (e) => toast.error(errorMessage(e)),
                        },
                      )
                    }
                  >
                    Scan library
                  </Button>
                ) : undefined
              }
            />
          )
        ) : (
          <ItemGrid items={items.data.items} total={items.data.total} aspect={aspect} cardWidth={cardWidth} jumpBar={sort === "title"} />
        )}
      </div>
    </>
  );
}
