// Collections and playlists (docs/API.md): types and React Query hooks.

import { useMutation, useQuery, useQueryClient, type QueryClient } from "@tanstack/react-query";
import { api } from "./client";
import type { Id, ItemKind, ItemSummary, Timestamp } from "./types";

export type CollectionSort = "release" | "title" | "custom";

export interface Collection {
  id: Id;
  library_id: Id;
  title: string;
  summary: string | null;
  /** Filled from TMDB. */
  automatic: boolean;
  sort: CollectionSort;
  /** Items the user can see. */
  item_count: number;
  /** The collection's own artwork, else its first item's. */
  poster_url: string;
  backdrop_url: string;
  created_at: Timestamp;
  updated_at: Timestamp;
}

export interface CollectionDetail extends Collection {
  items: ItemSummary[];
}

export interface CollectionRef {
  id: Id;
  title: string;
}

export type PlaylistKind = "video" | "audio";

export interface Playlist {
  id: Id;
  title: string;
  kind: PlaylistKind;
  item_count: number;
  duration_ms: number;
  /** Artwork of the first entries (up to 4), for a mosaic. */
  artwork: string[];
  created_at: Timestamp;
  updated_at: Timestamp;
}

export interface PlaylistEntry {
  entry_id: Id;
  item: ItemSummary;
}

export interface PlaylistDetail extends Playlist {
  entries: PlaylistEntry[];
}

/** The playlist kind an item (or its episodes / tracks) goes in. */
export function playlistKindOf(kind: ItemKind): PlaylistKind {
  return kind === "artist" || kind === "album" || kind === "track" ? "audio" : "video";
}

/** Movies and shows can be put in collections. */
export function collectable(item: Pick<ItemSummary, "kind">): boolean {
  return item.kind === "movie" || item.kind === "show";
}

export const listKeys = {
  // Under the library key, so library events refresh them.
  collections: (libraryId: Id) => ["library", libraryId, "collections"] as const,
  collection: (id: Id) => ["collection", id] as const,
  itemCollections: (itemId: Id) => ["item", itemId, "collections"] as const,
  playlists: ["playlists"] as const,
  playlist: (id: Id) => ["playlists", id] as const,
};

// ---------------------------------------------------------------------------
// Collections
// ---------------------------------------------------------------------------

export function useCollections(libraryId: Id, enabled = true) {
  return useQuery({
    queryKey: listKeys.collections(libraryId),
    queryFn: () => api.get<{ collections: Collection[] }>(`/api/libraries/${libraryId}/collections`),
    enabled,
    select: (d) => d.collections,
  });
}

export function useCollection(id: Id) {
  return useQuery({
    queryKey: listKeys.collection(id),
    queryFn: ({ signal }) => api.get<CollectionDetail>(`/api/collections/${id}`, { signal }),
    enabled: Number.isFinite(id) && id > 0,
  });
}

export function useItemCollections(itemId: Id, enabled = true) {
  return useQuery({
    queryKey: listKeys.itemCollections(itemId),
    queryFn: () => api.get<{ collections: CollectionRef[] }>(`/api/items/${itemId}/collections`),
    enabled,
    select: (d) => d.collections,
  });
}

function collectionChanged(qc: QueryClient, c: Pick<Collection, "id" | "library_id">) {
  void qc.invalidateQueries({ queryKey: listKeys.collection(c.id) });
  void qc.invalidateQueries({ queryKey: listKeys.collections(c.library_id) });
  void qc.invalidateQueries({ queryKey: ["item"], predicate: (q) => q.queryKey[2] === "collections" });
}

export function useCreateCollection() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ libraryId, ...body }: { libraryId: Id; title: string; summary?: string | null; item_ids?: Id[] }) =>
      api.post<CollectionDetail>(`/api/libraries/${libraryId}/collections`, body),
    onSuccess: (data) => {
      qc.setQueryData(listKeys.collection(data.id), data);
      collectionChanged(qc, data);
    },
  });
}

export function useUpdateCollection() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, ...body }: { id: Id; title?: string; summary?: string | null; sort?: CollectionSort }) =>
      api.patch<CollectionDetail>(`/api/collections/${id}`, body),
    onSuccess: (data) => {
      qc.setQueryData(listKeys.collection(data.id), data);
      collectionChanged(qc, data);
    },
  });
}

export function useDeleteCollection() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (c: Pick<Collection, "id" | "library_id">) => api.del<void>(`/api/collections/${c.id}`),
    onSuccess: (_data, c) => {
      qc.removeQueries({ queryKey: listKeys.collection(c.id) });
      collectionChanged(qc, c);
    },
  });
}

export function useAddToCollection() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ collection, itemIds }: { collection: Pick<Collection, "id" | "library_id">; itemIds: Id[] }) =>
      api.post<{ added: number }>(`/api/collections/${collection.id}/items`, { item_ids: itemIds }),
    onSuccess: (_data, { collection }) => collectionChanged(qc, collection),
  });
}

export function useRemoveFromCollection() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ collection, itemId }: { collection: Pick<Collection, "id" | "library_id">; itemId: Id }) =>
      api.del<void>(`/api/collections/${collection.id}/items/${itemId}`),
    onSuccess: (_data, { collection }) => collectionChanged(qc, collection),
  });
}

/** Custom order; shows the new order right away. */
export function useReorderCollection() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ collection, itemIds }: { collection: Pick<Collection, "id" | "library_id">; itemIds: Id[] }) =>
      api.put<void>(`/api/collections/${collection.id}/order`, { item_ids: itemIds }),
    onMutate: ({ collection, itemIds }) => {
      qc.setQueryData<CollectionDetail>(listKeys.collection(collection.id), (old) =>
        old ? { ...old, sort: "custom", items: reordered(old.items, itemIds, (i) => i.id) } : old,
      );
    },
    onSettled: (_data, _err, { collection }) => collectionChanged(qc, collection),
  });
}

// ---------------------------------------------------------------------------
// Playlists
// ---------------------------------------------------------------------------

export function usePlaylists(enabled = true) {
  return useQuery({
    queryKey: listKeys.playlists,
    queryFn: () => api.get<{ playlists: Playlist[] }>("/api/playlists"),
    enabled,
    select: (d) => d.playlists,
  });
}

export function usePlaylist(id: Id) {
  return useQuery({
    queryKey: listKeys.playlist(id),
    queryFn: ({ signal }) => api.get<PlaylistDetail>(`/api/playlists/${id}`, { signal }),
    enabled: Number.isFinite(id) && id > 0,
  });
}

export function useCreatePlaylist() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (body: { title: string; kind?: PlaylistKind; item_ids?: Id[] }) => api.post<PlaylistDetail>("/api/playlists", body),
    onSuccess: (data) => {
      qc.setQueryData(listKeys.playlist(data.id), data);
      void qc.invalidateQueries({ queryKey: listKeys.playlists, exact: true });
    },
  });
}

export function useRenamePlaylist() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, title }: { id: Id; title: string }) => api.patch<PlaylistDetail>(`/api/playlists/${id}`, { title }),
    onSuccess: (data) => {
      qc.setQueryData(listKeys.playlist(data.id), data);
      void qc.invalidateQueries({ queryKey: listKeys.playlists, exact: true });
    },
  });
}

export function useDeletePlaylist() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: Id) => api.del<void>(`/api/playlists/${id}`),
    onSuccess: (_data, id) => {
      qc.removeQueries({ queryKey: listKeys.playlist(id) });
      void qc.invalidateQueries({ queryKey: listKeys.playlists, exact: true });
    },
  });
}

export function useAddToPlaylist() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, itemIds }: { id: Id; itemIds: Id[] }) =>
      api.post<{ added: number }>(`/api/playlists/${id}/items`, { item_ids: itemIds }),
    // The list and every playlist's details.
    onSuccess: () => qc.invalidateQueries({ queryKey: listKeys.playlists }),
  });
}

export function useRemoveFromPlaylist() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, entryId }: { id: Id; entryId: Id }) => api.del<void>(`/api/playlists/${id}/items/${entryId}`),
    onMutate: ({ id, entryId }) => {
      qc.setQueryData<PlaylistDetail>(listKeys.playlist(id), (old) =>
        old ? { ...old, entries: old.entries.filter((e) => e.entry_id !== entryId), item_count: old.item_count - 1 } : old,
      );
    },
    onSettled: () => qc.invalidateQueries({ queryKey: listKeys.playlists }),
  });
}

/** New order of the entries; shows it right away. */
export function useReorderPlaylist() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, entryIds }: { id: Id; entryIds: Id[] }) =>
      api.put<PlaylistDetail>(`/api/playlists/${id}/order`, { entry_ids: entryIds }),
    onMutate: ({ id, entryIds }) => {
      qc.setQueryData<PlaylistDetail>(listKeys.playlist(id), (old) =>
        old ? { ...old, entries: reordered(old.entries, entryIds, (e) => e.entry_id) } : old,
      );
    },
    onSuccess: (data) => qc.setQueryData(listKeys.playlist(data.id), data),
    onError: (_err, { id }) => qc.invalidateQueries({ queryKey: listKeys.playlist(id) }),
    onSettled: () => qc.invalidateQueries({ queryKey: listKeys.playlists, exact: true }),
  });
}

/** `list` in the order of `ids` (unlisted ones keep their order after them). */
function reordered<T>(list: T[], ids: Id[], idOf: (t: T) => Id): T[] {
  const byId = new Map(list.map((t) => [idOf(t), t]));
  const first = ids.map((id) => byId.get(id)).filter((t): t is T => t !== undefined);
  const listed = new Set(ids);
  return [...first, ...list.filter((t) => !listed.has(idOf(t)))];
}
