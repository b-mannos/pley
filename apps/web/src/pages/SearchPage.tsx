import { useSearchParams } from "react-router";
import { Search } from "lucide-react";
import { errorMessage } from "@/api/client";
import { useSearch } from "@/api/hooks";
import type { ItemKind, ItemSummary } from "@/api/types";
import { aspectFor, CardSkeleton, MediaCard } from "@/components/cards";
import { PersonCard } from "@/components/PersonCard";
import { Button, EmptyState, SectionTitle, Spinner } from "@/components/ui";
import { useDocumentTitle, useElementWidth } from "@/lib/hooks";

const GROUPS: { kind: ItemKind; title: string }[] = [
  { kind: "movie", title: "Movies" },
  { kind: "show", title: "Shows" },
  { kind: "season", title: "Seasons" },
  { kind: "episode", title: "Episodes" },
  { kind: "artist", title: "Artists" },
  { kind: "album", title: "Albums" },
  { kind: "track", title: "Tracks" },
];

function ResultGroup({ title, items }: { title: string; items: ItemSummary[] }) {
  const [ref, width] = useElementWidth<HTMLDivElement>();
  const aspect = items[0] ? aspectFor(items[0]) : "poster";
  const target = aspect === "thumb" ? 260 : aspect === "square" ? 160 : 150;
  const gap = 18;
  const cols = width > 0 ? Math.max(2, Math.floor((width + gap) / (target + gap))) : 4;
  const colWidth = width > 0 ? (width - gap * (cols - 1)) / cols : target;
  return (
    <section>
      <SectionTitle>
        {title} <span className="ml-1 text-sm font-normal text-subtle">{items.length}</span>
      </SectionTitle>
      <div ref={ref} className="grid gap-x-[18px] gap-y-6" style={{ gridTemplateColumns: `repeat(${cols}, minmax(0, 1fr))` }}>
        {items.map((item) => (
          <MediaCard key={item.id} item={item} aspect={aspect} width={colWidth} playable={item.kind !== "track"} />
        ))}
      </div>
    </section>
  );
}

export function SearchPage() {
  const [params] = useSearchParams();
  const q = params.get("q") ?? "";
  useDocumentTitle(q ? `Search: ${q}` : "Search");
  const search = useSearch(q);

  if (!q.trim()) {
    return (
      <EmptyState
        className="min-h-[60vh]"
        icon={<Search />}
        title="Search your libraries"
        description="Find movies, shows, episodes, people, artists, albums and tracks. Press / anywhere to jump to the search box."
      />
    );
  }

  if (search.isPending) {
    return (
      <div className="px-4 py-8 sm:px-8">
        <div className="grid gap-x-[18px] gap-y-6" style={{ gridTemplateColumns: "repeat(auto-fill, minmax(150px, 1fr))" }}>
          {Array.from({ length: 12 }, (_, i) => (
            <CardSkeleton key={i} aspect="poster" />
          ))}
        </div>
      </div>
    );
  }

  if (search.isError) {
    return (
      <EmptyState
        className="min-h-[60vh]"
        title="Couldn't search your libraries"
        description={errorMessage(search.error)}
        action={<Button onClick={() => void search.refetch()}>Retry</Button>}
      />
    );
  }

  const results = search.data?.items ?? [];
  const people = search.data?.people ?? [];
  if (!results.length && !people.length) {
    return (
      <EmptyState
        className="min-h-[60vh]"
        icon={<Search />}
        title={`No results for “${q}”`}
        description="Try a different spelling or a shorter query."
      />
    );
  }

  return (
    <div className="space-y-10 px-4 py-8 sm:px-8">
      <div className="flex items-center gap-3">
        <h1 className="text-xl font-semibold tracking-tight">
          Results for “{q}”
        </h1>
        {search.isFetching ? <Spinner className="size-4" /> : null}
      </div>
      {people.length ? (
        <section>
          <SectionTitle>
            People <span className="ml-1 text-sm font-normal text-subtle">{people.length}</span>
          </SectionTitle>
          <div className="flex flex-wrap gap-x-4 gap-y-5">
            {people.map((p) => (
              <PersonCard key={p.id} person={p} width={112} />
            ))}
          </div>
        </section>
      ) : null}
      {GROUPS.map(({ kind, title }) => {
        const items = results.filter((r) => r.kind === kind);
        return items.length ? <ResultGroup key={kind} title={title} items={items} /> : null;
      })}
    </div>
  );
}
