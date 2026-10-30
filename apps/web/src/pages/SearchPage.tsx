import { Link, useSearchParams } from "react-router";
import { Compass, Search } from "lucide-react";
import { errorMessage } from "@/api/client";
import { useSearch } from "@/api/hooks";
import { programProgress, type Channel } from "@/api/livetv";
import { mayRequest, useRequestSummary } from "@/api/requests";
import type { ItemKind, ItemSummary } from "@/api/types";
import { aspectFor, CardSkeleton, MediaCard } from "@/components/cards";
import { PersonCard } from "@/components/PersonCard";
import { Button, buttonClass, EmptyState, SectionTitle, Spinner } from "@/components/ui";
import { useDocumentTitle, useElementWidth } from "@/lib/hooks";
import { ChannelLogo, liveUrl, useNow } from "@/pages/livetv/common";
import { discoverUrl } from "@/lib/urls";

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

/** Live TV channels found: logo, number and name, what's on with how far it is. */
function ChannelResults({ channels }: { channels: Channel[] }) {
  const now = useNow();
  return (
    <section>
      <SectionTitle>
        Live TV <span className="ml-1 text-sm font-normal text-subtle">{channels.length}</span>
      </SectionTitle>
      <ul className="grid grid-cols-[repeat(auto-fill,minmax(16rem,1fr))] gap-3">
        {channels.map((c) => {
          const on = c.now && c.now.start <= now && c.now.stop > now ? c.now : null;
          return (
            <li key={c.id}>
              <Link
                to={liveUrl(c)}
                className="flex items-center gap-3 rounded-lg border border-border bg-surface p-2.5 transition-colors hover:border-border-strong hover:bg-surface-2"
              >
                <ChannelLogo channel={c} width={96} showName={false} className="h-11 w-[4.5rem] shrink-0 rounded-md bg-surface-3" />
                <div className="min-w-0 flex-1">
                  <div className="flex items-baseline gap-1.5">
                    {c.number ? <span className="shrink-0 text-xs tabular-nums text-subtle">{c.number}</span> : null}
                    <span className="truncate text-sm font-medium">{c.name}</span>
                  </div>
                  {on ? (
                    <>
                      <div className="truncate text-xs text-muted">{on.title}</div>
                      <div className="mt-1 h-0.5 overflow-hidden rounded-full bg-white/10">
                        <div className="h-full bg-accent" style={{ width: `${programProgress(on, now) * 100}%` }} />
                      </div>
                    </>
                  ) : (
                    <div className="truncate text-xs text-subtle">{c.group}</div>
                  )}
                </div>
              </Link>
            </li>
          );
        })}
      </ul>
    </section>
  );
}

export function SearchPage() {
  const [params] = useSearchParams();
  const q = params.get("q") ?? "";
  useDocumentTitle(q ? `Search: ${q}` : "Search");
  const search = useSearch(q);
  // Where this account may ask for titles: what the libraries lack, on TMDB.
  const discover = mayRequest(useRequestSummary(q.trim() !== "").data);

  if (!q.trim()) {
    return (
      <EmptyState
        className="min-h-[60vh]"
        icon={<Search />}
        title="Search your libraries"
        description="Find movies, shows, episodes, people, artists, albums, tracks and Live TV channels. Press / anywhere to jump to the search box."
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
  const channels = search.data?.channels ?? [];
  if (!results.length && !people.length && !channels.length) {
    return (
      <EmptyState
        className="min-h-[60vh]"
        icon={<Search />}
        title={`No results for “${q}”`}
        description={discover ? "Try a different spelling or a shorter query, or look for it on TMDB and ask for it." : "Try a different spelling or a shorter query."}
        action={
          discover ? (
            <Link to={discoverUrl(q)} className={buttonClass("secondary")}>
              <Compass className="size-4" />
              Look for “{q.trim()}” in Discover
            </Link>
          ) : undefined
        }
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
      {channels.length ? <ChannelResults channels={channels} /> : null}
      {GROUPS.map(({ kind, title }) => {
        const items = results.filter((r) => r.kind === kind);
        return items.length ? <ResultGroup key={kind} title={title} items={items} /> : null;
      })}
      {discover ? (
        <p className="text-sm text-muted">
          Not here?{" "}
          <Link to={discoverUrl(q)} className="font-medium text-accent hover:underline">
            Look for “{q.trim()}” in Discover
          </Link>
        </p>
      ) : null}
    </div>
  );
}
