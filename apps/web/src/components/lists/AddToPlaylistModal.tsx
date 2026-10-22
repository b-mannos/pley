import { useState, type FormEvent } from "react";
import { Plus } from "lucide-react";
import { errorMessage } from "@/api/client";
import { playlistKindOf, useAddToPlaylist, useCreatePlaylist, usePlaylists, type Playlist } from "@/api/lists";
import type { ItemSummary } from "@/api/types";
import { Modal } from "@/components/Modal";
import { useToast } from "@/components/Toast";
import { Button, Skeleton, TextInput } from "@/components/ui";
import { formatDuration, fullTitle, pluralize } from "@/lib/format";
import { PlaylistArt } from "./PlaylistArt";

/** Pick (or start) a playlist for an item; shows, seasons, albums and artists add all their items. */
export function AddToPlaylistModal({ item, onClose }: { item: ItemSummary | null; onClose: () => void }) {
  const open = item != null;
  const kind = item ? playlistKindOf(item.kind) : "video";
  const playlists = usePlaylists(open);
  const create = useCreatePlaylist();
  const add = useAddToPlaylist();
  const toast = useToast();
  const [title, setTitle] = useState("");
  const matching = playlists.data?.filter((p) => p.kind === kind) ?? [];
  const busy = create.isPending || add.isPending;

  const onCreate = (e: FormEvent) => {
    e.preventDefault();
    const name = title.trim();
    if (!item || !name) return;
    create.mutate(
      { title: name, kind, item_ids: [item.id] },
      {
        onSuccess: (p) => {
          toast.success(`Added to ${p.title}`);
          onClose();
        },
        onError: (err) => toast.error(errorMessage(err)),
      },
    );
  };

  const onAdd = (p: Playlist) => {
    if (!item) return;
    add.mutate(
      { id: p.id, itemIds: [item.id] },
      {
        onSuccess: ({ added }) => {
          if (added === 0) toast.info("Nothing to add");
          else toast.success(added === 1 ? `Added to ${p.title}` : `Added ${added} items to ${p.title}`);
          onClose();
        },
        onError: (err) => toast.error(errorMessage(err)),
      },
    );
  };

  return (
    <Modal open={open} onClose={onClose} title="Add to playlist" size="sm" locked={busy}>
      {item ? <p className="-mt-1 mb-3 truncate text-sm text-muted">{fullTitle(item)}</p> : null}
      <form onSubmit={onCreate} className="flex gap-2">
        <TextInput
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          placeholder={kind === "audio" ? "New music playlist" : "New video playlist"}
          aria-label="New playlist name"
          maxLength={200}
        />
        <Button type="submit" variant="primary" icon={<Plus className="size-4" />} disabled={!title.trim() || add.isPending} loading={create.isPending}>
          Create
        </Button>
      </form>
      <div className="mt-4">
        {playlists.isPending ? (
          <div className="space-y-2">
            <Skeleton className="h-12" />
            <Skeleton className="h-12" />
          </div>
        ) : playlists.isError ? (
          <p className="text-sm text-danger">{errorMessage(playlists.error)}</p>
        ) : matching.length === 0 ? (
          <p className="py-2 text-sm text-muted">No {kind === "audio" ? "music" : "video"} playlists yet: name one above.</p>
        ) : (
          <ul className="-mx-2 max-h-72 overflow-y-auto">
            {matching.map((p) => (
              <li key={p.id}>
                <button
                  type="button"
                  disabled={busy}
                  onClick={() => onAdd(p)}
                  className="flex w-full items-center gap-3 rounded-md px-2 py-2 text-left transition-colors hover:bg-white/5 disabled:opacity-50"
                >
                  <PlaylistArt playlist={p} width={44} className="size-11 shrink-0" />
                  <span className="min-w-0 flex-1">
                    <span className="block truncate text-sm font-medium text-fg">{p.title}</span>
                    <span className="block text-xs text-muted">
                      {[pluralize(p.item_count, "item"), formatDuration(p.duration_ms)].filter(Boolean).join(" · ")}
                    </span>
                  </span>
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>
    </Modal>
  );
}
