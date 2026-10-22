import { useState } from "react";
import { Link, useNavigate } from "react-router";
import { ListMusic, ListPlus, Plus } from "lucide-react";
import { errorMessage } from "@/api/client";
import { usePlaylists, type Playlist } from "@/api/lists";
import { PlaylistArt } from "@/components/lists/PlaylistArt";
import { Button, EmptyState, Skeleton } from "@/components/ui";
import { formatDuration, pluralize } from "@/lib/format";
import { useDocumentTitle, useElementWidth } from "@/lib/hooks";
import { PlaylistFormModal } from "./PlaylistFormModal";

const CARD_WIDTH = 180;

export function playlistMeta(p: Pick<Playlist, "item_count" | "duration_ms">): string {
  return [pluralize(p.item_count, "item"), formatDuration(p.duration_ms)].filter(Boolean).join(" · ");
}

export function PlaylistsPage() {
  useDocumentTitle("Playlists");
  const navigate = useNavigate();
  const playlists = usePlaylists();
  const [creating, setCreating] = useState(false);
  const [ref, width] = useElementWidth<HTMLDivElement>();
  const gap = width > 0 && width < 640 ? 12 : 18;
  const cols = width > 0 ? Math.max(2, Math.floor((width + gap) / (CARD_WIDTH + gap))) : 6;
  const colWidth = width > 0 ? (width - gap * (cols - 1)) / cols : CARD_WIDTH;
  const list = playlists.data ?? [];

  return (
    <div className="px-4 pb-12 pt-6 sm:px-8">
      <div className="mb-6 flex flex-wrap items-center justify-between gap-4">
        <div className="flex min-w-0 items-center gap-3">
          <div className="flex size-10 shrink-0 items-center justify-center rounded-lg bg-surface-2 text-accent">
            <ListMusic className="size-5" />
          </div>
          <h1 className="truncate text-2xl font-bold tracking-tight">Playlists</h1>
        </div>
        {list.length > 0 ? (
          <Button icon={<Plus className="size-4" />} onClick={() => setCreating(true)}>
            New playlist
          </Button>
        ) : null}
      </div>

      <div ref={ref}>
        {playlists.isPending ? (
          <div className="grid grid-cols-[repeat(auto-fill,minmax(180px,1fr))] gap-x-[18px] gap-y-[26px]">
            {Array.from({ length: 6 }, (_, i) => (
              <div key={i}>
                <Skeleton className="aspect-square w-full" />
                <Skeleton className="mt-2 h-3.5 w-3/4" />
                <Skeleton className="mt-1.5 h-3 w-1/3" />
              </div>
            ))}
          </div>
        ) : playlists.isError ? (
          <EmptyState
            title="Couldn't load your playlists"
            description={errorMessage(playlists.error)}
            action={<Button onClick={() => void playlists.refetch()}>Retry</Button>}
          />
        ) : list.length === 0 ? (
          <EmptyState
            icon={<ListPlus />}
            title="No playlists yet"
            description="Make one here, or use “Add to playlist” on a movie, show, album or episode (also when right-clicking a poster)."
            action={
              <Button variant="primary" icon={<Plus className="size-4" />} onClick={() => setCreating(true)}>
                New playlist
              </Button>
            }
          />
        ) : (
          <div
            className="grid"
            style={{ gridTemplateColumns: `repeat(${cols}, minmax(0, 1fr))`, columnGap: gap, rowGap: gap < 18 ? 18 : 26 }}
          >
            {list.map((p) => (
              <Link key={p.id} to={`/playlists/${p.id}`} className="group/card min-w-0 outline-none">
                <PlaylistArt
                  playlist={p}
                  width={colWidth}
                  className="w-full transition duration-200 group-hover/card:ring-2 group-hover/card:ring-accent group-focus-visible/card:ring-2 group-focus-visible/card:ring-accent"
                />
                <div className="mt-2 px-0.5">
                  <div className="truncate text-sm font-medium text-fg transition-colors group-hover/card:text-accent" title={p.title}>
                    {p.title}
                  </div>
                  <div className="truncate text-xs text-muted">{playlistMeta(p)}</div>
                </div>
              </Link>
            ))}
          </div>
        )}
      </div>

      <PlaylistFormModal
        key={creating ? "open" : "closed"}
        open={creating}
        onClose={() => setCreating(false)}
        onCreated={(p) => navigate(`/playlists/${p.id}`)}
      />
    </div>
  );
}
