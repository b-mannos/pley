import { useMutation, useQuery, useQueryClient, type QueryClient } from "@tanstack/react-query";
import { api, qs } from "./client";
import type {
  Activity,
  FsListing,
  Hub,
  Id,
  ItemDetail,
  ItemSummary,
  Library,
  LibraryFilters,
  LibraryItemsQuery,
  LibraryItemsResponse,
  LibraryKind,
  LoginResponse,
  ServerStatus,
  Settings,
  SystemInfo,
  TranscoderStatus,
  User,
} from "./types";

export const keys = {
  server: ["server"] as const,
  me: ["me"] as const,
  libraries: ["libraries"] as const,
  library: (id: Id) => ["library", id] as const,
  libraryItems: (id: Id, q: LibraryItemsQuery) => ["library", id, "items", q] as const,
  libraryFilters: (id: Id) => ["library", id, "filters"] as const,
  libraryHubs: (id: Id) => ["library", id, "hubs"] as const,
  homeHubs: ["hubs", "home"] as const,
  item: (id: Id) => ["item", id] as const,
  children: (id: Id) => ["item", id, "children"] as const,
  onDeck: (id: Id) => ["item", id, "on-deck"] as const,
  next: (id: Id) => ["item", id, "next"] as const,
  search: (q: string, limit: number) => ["search", q, limit] as const,
  activity: ["activity"] as const,
  users: ["users"] as const,
  settings: ["settings"] as const,
  system: ["system"] as const,
  transcoder: ["transcoder"] as const,
  fs: (path: string | null) => ["fs", path ?? ""] as const,
};

/**
 * Children responses are `ItemSummary`s. Episodes may additionally carry
 * `summary`/`release_date` if the server includes them (not in the v1 contract).
 */
export type ChildItem = ItemSummary & { summary?: string | null; release_date?: string | null };

// ---------------------------------------------------------------------------
// Server / auth
// ---------------------------------------------------------------------------

export function useServer() {
  return useQuery({
    queryKey: keys.server,
    queryFn: () => api.get<ServerStatus>("/api/server", { noAuthRedirect: true }),
    staleTime: 60_000,
    retry: 1,
  });
}

export function useMe(enabled = true) {
  return useQuery({
    queryKey: keys.me,
    queryFn: () => api.get<User>("/api/auth/me", { noAuthRedirect: true }),
    enabled,
    retry: false,
    staleTime: 5 * 60_000,
  });
}

export function useLogin() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (body: { username: string; password: string }) =>
      api.post<LoginResponse>("/api/auth/login", body, { noAuthRedirect: true }),
    onSuccess: (res) => {
      qc.setQueryData(keys.me, res.user);
    },
  });
}

export function useSetup() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (body: { username: string; password: string }) =>
      api.post<LoginResponse>("/api/setup", body, { noAuthRedirect: true }),
    onSuccess: (res) => {
      qc.setQueryData(keys.me, res.user);
      qc.setQueryData<ServerStatus | undefined>(keys.server, (old) => (old ? { ...old, setup_required: false } : old));
    },
  });
}

export function useLogout() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: () => api.post<void>("/api/auth/logout", undefined, { noAuthRedirect: true }),
    onSettled: () => {
      qc.clear();
    },
  });
}

export function useChangePassword() {
  return useMutation({
    // A wrong current password must not trigger the global "session expired" redirect.
    mutationFn: (body: { current_password: string; new_password: string }) =>
      api.put<void>("/api/auth/password", body, { noAuthRedirect: true }),
  });
}

// ---------------------------------------------------------------------------
// Libraries
// ---------------------------------------------------------------------------

export function useLibraries() {
  return useQuery({
    queryKey: keys.libraries,
    queryFn: () => api.get<Library[]>("/api/libraries"),
    staleTime: 30_000,
  });
}

export function useLibrary(id: Id) {
  const libs = useLibraries();
  return { ...libs, data: libs.data?.find((l) => l.id === id) };
}

export const DEFAULT_ITEMS_LIMIT = 100_000;

export function useLibraryItems(id: Id, q: LibraryItemsQuery, enabled = true) {
  return useQuery({
    queryKey: keys.libraryItems(id, q),
    queryFn: ({ signal }) =>
      api.get<LibraryItemsResponse>(
        `/api/libraries/${id}/items${qs({
          sort: q.sort,
          order: q.order,
          watched: q.watched,
          genre: q.genre,
          year: q.year,
          offset: q.offset,
          limit: q.limit ?? DEFAULT_ITEMS_LIMIT,
        })}`,
        { signal },
      ),
    enabled,
    // Keep showing the list while sorting/filtering, but never another library's items.
    placeholderData: (prev, prevQuery) => (prevQuery?.queryKey[1] === id ? prev : undefined),
  });
}

export function useLibraryFilters(id: Id, enabled = true) {
  return useQuery({
    queryKey: keys.libraryFilters(id),
    queryFn: () => api.get<LibraryFilters>(`/api/libraries/${id}/filters`),
    enabled,
    staleTime: 60_000,
  });
}

export function useLibraryHubs(id: Id, enabled = true) {
  return useQuery({
    queryKey: keys.libraryHubs(id),
    queryFn: () => api.get<{ hubs: Hub[] }>(`/api/libraries/${id}/hubs`),
    enabled,
  });
}

export interface LibraryInput {
  name: string;
  kind: LibraryKind;
  paths: string[];
}

export function useCreateLibrary() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (body: LibraryInput) => api.post<Library>("/api/libraries", body),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: keys.libraries });
      qc.invalidateQueries({ queryKey: keys.activity });
      qc.invalidateQueries({ queryKey: ["hubs"] });
    },
  });
}

export function useUpdateLibrary() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, ...body }: { id: Id; name?: string; paths?: string[] }) =>
      api.patch<Library>(`/api/libraries/${id}`, body),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: keys.libraries });
    },
  });
}

export function useDeleteLibrary() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: Id) => api.del<void>(`/api/libraries/${id}`),
    onSuccess: (_data, id) => {
      qc.removeQueries({ queryKey: keys.library(id) });
      qc.invalidateQueries({ queryKey: keys.libraries });
      qc.invalidateQueries({ queryKey: ["hubs"] });
      qc.invalidateQueries({ queryKey: ["search"] });
    },
  });
}

export function useScanLibrary() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, force }: { id: Id; force?: boolean }) =>
      api.post<void>(`/api/libraries/${id}/scan`, { force: !!force }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: keys.activity });
      qc.invalidateQueries({ queryKey: keys.libraries });
    },
  });
}

export function useScanAll() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: () => api.post<void>("/api/libraries/scan"),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: keys.activity });
      qc.invalidateQueries({ queryKey: keys.libraries });
    },
  });
}

// ---------------------------------------------------------------------------
// Items
// ---------------------------------------------------------------------------

export function useItem(id: Id | null | undefined) {
  return useQuery({
    queryKey: keys.item(id ?? 0),
    queryFn: ({ signal }) => api.get<ItemDetail>(`/api/items/${id}`, { signal }),
    enabled: id != null && Number.isFinite(id) && id > 0,
  });
}

export function useChildren(id: Id | null | undefined, enabled = true) {
  return useQuery({
    queryKey: keys.children(id ?? 0),
    queryFn: ({ signal }) => api.get<{ items: ChildItem[] }>(`/api/items/${id}/children`, { signal }),
    enabled: enabled && id != null && id > 0,
    select: (d) => d.items,
  });
}

export function fetchChildren(qc: QueryClient, id: Id): Promise<ChildItem[]> {
  return qc
    .fetchQuery({
      queryKey: keys.children(id),
      queryFn: () => api.get<{ items: ChildItem[] }>(`/api/items/${id}/children`),
      staleTime: 30_000,
    })
    .then((d) => d.items);
}

export function useOnDeck(id: Id | null | undefined, enabled = true) {
  return useQuery({
    queryKey: keys.onDeck(id ?? 0),
    queryFn: () => api.get<{ item: ItemSummary | null }>(`/api/items/${id}/on-deck`),
    enabled: enabled && id != null && id > 0,
    select: (d) => d.item,
  });
}

export function fetchOnDeck(qc: QueryClient, id: Id) {
  return qc
    .fetchQuery({
      queryKey: keys.onDeck(id),
      queryFn: () => api.get<{ item: ItemSummary | null }>(`/api/items/${id}/on-deck`),
      staleTime: 0,
    })
    .then((d) => d.item);
}

export function fetchNext(qc: QueryClient, id: Id) {
  return qc
    .fetchQuery({
      queryKey: keys.next(id),
      queryFn: () => api.get<{ item: ItemSummary | null }>(`/api/items/${id}/next`),
      staleTime: 0,
    })
    .then((d) => d.item);
}

/** Invalidate everything that shows watch state. */
export function invalidateWatchState(qc: QueryClient) {
  qc.invalidateQueries({ queryKey: ["item"] });
  qc.invalidateQueries({ queryKey: ["library"] });
  qc.invalidateQueries({ queryKey: ["hubs"] });
  qc.invalidateQueries({ queryKey: ["search"] });
}

export function useSetWatched() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, watched }: { id: Id; watched: boolean }) =>
      api.post<void>(`/api/items/${id}/${watched ? "watched" : "unwatched"}`),
    onSuccess: () => invalidateWatchState(qc),
  });
}

export function useRefreshItem() {
  return useMutation({
    mutationFn: (id: Id) => api.post<void>(`/api/items/${id}/refresh`),
  });
}

// ---------------------------------------------------------------------------
// Home / search
// ---------------------------------------------------------------------------

export function useHomeHubs() {
  return useQuery({
    queryKey: keys.homeHubs,
    queryFn: () => api.get<{ hubs: Hub[] }>("/api/hubs/home"),
    select: (d) => d.hubs,
  });
}

export function useSearch(q: string, limit = 60) {
  const trimmed = q.trim();
  return useQuery({
    queryKey: keys.search(trimmed, limit),
    queryFn: ({ signal }) => api.get<{ items: ItemSummary[] }>(`/api/search${qs({ q: trimmed, limit })}`, { signal }),
    enabled: trimmed.length > 0,
    select: (d) => d.items,
    placeholderData: (prev) => prev,
  });
}

// ---------------------------------------------------------------------------
// Activity
// ---------------------------------------------------------------------------

export function useActivity(opts: { poll?: number | false } = {}) {
  return useQuery({
    queryKey: keys.activity,
    queryFn: () => api.get<Activity>("/api/activity"),
    refetchInterval: (query) => {
      if (opts.poll) return opts.poll;
      const scans = query.state.data?.scans ?? [];
      return scans.some((s) => s.phase !== "done") ? 4_000 : false;
    },
    refetchIntervalInBackground: false,
    staleTime: 2_000,
  });
}

// ---------------------------------------------------------------------------
// Users (admin)
// ---------------------------------------------------------------------------

export function useUsers() {
  return useQuery({ queryKey: keys.users, queryFn: () => api.get<User[]>("/api/users") });
}

export function useCreateUser() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (body: { username: string; password: string; is_admin: boolean }) => api.post<User>("/api/users", body),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.users }),
  });
}

export function useUpdateUser() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, ...body }: { id: Id; password?: string; is_admin?: boolean }) =>
      api.patch<User>(`/api/users/${id}`, body),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.users }),
  });
}

export function useDeleteUser() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: Id) => api.del<void>(`/api/users/${id}`),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.users }),
  });
}

// ---------------------------------------------------------------------------
// System (admin)
// ---------------------------------------------------------------------------

export function useSettings() {
  return useQuery({ queryKey: keys.settings, queryFn: () => api.get<Settings>("/api/settings") });
}

export function useSaveSettings() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (body: Partial<Settings>) => api.put<Settings>("/api/settings", body),
    onSuccess: (data) => {
      qc.setQueryData(keys.settings, data);
      qc.invalidateQueries({ queryKey: keys.system });
      qc.invalidateQueries({ queryKey: keys.server });
    },
  });
}

export function useSystemInfo() {
  return useQuery({ queryKey: keys.system, queryFn: () => api.get<SystemInfo>("/api/system"), staleTime: 10_000 });
}

export function useTranscoderStatus(pollMs: number | false = 2_500) {
  return useQuery({
    queryKey: keys.transcoder,
    queryFn: () => api.get<TranscoderStatus>("/api/transcoder"),
    refetchInterval: pollMs,
    refetchIntervalInBackground: false,
    staleTime: 1_000,
    retry: 1,
  });
}

export function useFs(path: string | null, enabled = true) {
  return useQuery({
    queryKey: keys.fs(path),
    queryFn: () => api.get<FsListing>(`/api/fs${qs({ path })}`),
    enabled,
    staleTime: 5_000,
    retry: false,
  });
}
