import { useMemo } from "react";
import { useInfiniteQuery, useMutation, useQuery, useQueryClient, type QueryClient } from "@tanstack/react-query";
import { api, qs, request } from "./client";
import type {
  Activity,
  Backup,
  Backups,
  CleanReport,
  Device,
  FsListing,
  HistoryPage,
  Hub,
  Id,
  Invite,
  InviteCheck,
  ItemDetail,
  ItemSummary,
  Library,
  LibraryFilters,
  LibraryItemsQuery,
  LibraryItemsResponse,
  LibraryKind,
  LoginResponse,
  PersonDetail,
  SearchResults,
  ServerStatus,
  Settings,
  StorageUsage,
  SystemInfo,
  TranscoderStatus,
  Trickplay,
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
  person: (id: Id) => ["person", id] as const,
  related: (id: Id) => ["item", id, "related"] as const,
  history: (scope: HistoryScope, pageSize: number) => ["history", scope, pageSize] as const,
  activity: ["activity"] as const,
  users: ["users"] as const,
  userDevices: (id: Id) => ["users", id, "devices"] as const,
  profiles: ["profiles"] as const,
  devices: ["devices"] as const,
  invites: ["invites"] as const,
  invite: (code: string) => ["invite", code] as const,
  settings: ["settings"] as const,
  system: ["system"] as const,
  storage: ["system", "storage"] as const,
  backups: ["system", "backups"] as const,
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
// Profiles and your account
// ---------------------------------------------------------------------------

/** Home profiles this user can switch to (empty for people outside the home). */
export function useProfiles(enabled = true) {
  return useQuery({
    queryKey: keys.profiles,
    queryFn: () => api.get<User[]>("/api/auth/profiles"),
    enabled,
    staleTime: 60_000,
  });
}

export function fetchProfiles(qc: QueryClient) {
  return qc.fetchQuery({ queryKey: keys.profiles, queryFn: () => api.get<User[]>("/api/auth/profiles"), staleTime: 0 });
}

/**
 * Switch this device to another home profile. Everything cached belongs to
 * the previous profile, so the cache starts over with the new user.
 */
export function useSwitchProfile() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (body: { user_id: Id; pin?: string; password?: string }) =>
      api.post<LoginResponse>("/api/auth/switch", body, { noAuthRedirect: true }),
    onSuccess: (res) => {
      const server = qc.getQueryData<ServerStatus>(keys.server);
      qc.clear();
      if (server) qc.setQueryData(keys.server, server);
      qc.setQueryData(keys.me, res.user);
    },
  });
}

function rememberMe(qc: QueryClient, user: User) {
  qc.setQueryData(keys.me, user);
  qc.invalidateQueries({ queryKey: keys.users });
  qc.invalidateQueries({ queryKey: keys.profiles });
}

export function useUpdateProfile() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (body: { display_name?: string | null; avatar_color?: string | null }) => api.patch<User>("/api/me", body),
    onSuccess: (user) => rememberMe(qc, user),
  });
}

export function useUploadAvatar() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (file: Blob) => request<User>("/api/me/avatar", { method: "PUT", rawBody: file }),
    onSuccess: (user) => rememberMe(qc, user),
  });
}

export function useDeleteAvatar() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: () => api.del<void>("/api/me/avatar"),
    onSuccess: () => {
      qc.setQueryData<User | undefined>(keys.me, (u) => (u ? { ...u, avatar_url: null } : u));
      qc.invalidateQueries({ queryKey: keys.users });
      qc.invalidateQueries({ queryKey: keys.profiles });
    },
  });
}

export function useSetPin() {
  const qc = useQueryClient();
  return useMutation({
    // A wrong password must not look like an expired session.
    mutationFn: (body: { pin: string | null; password: string }) =>
      api.put<void>("/api/me/pin", body, { noAuthRedirect: true }),
    onSuccess: (_data, body) => {
      qc.setQueryData<User | undefined>(keys.me, (u) => (u ? { ...u, has_pin: !!body.pin } : u));
      qc.invalidateQueries({ queryKey: keys.profiles });
      qc.invalidateQueries({ queryKey: keys.users });
    },
  });
}

export function useDevices() {
  return useQuery({ queryKey: keys.devices, queryFn: () => api.get<Device[]>("/api/me/devices") });
}

export function useSignOutDevice() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => api.del<void>(`/api/me/devices/${encodeURIComponent(id)}`),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.devices }),
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

/** Items fetched on their own first, so large libraries paint right away. */
export const FIRST_PAGE = 300;

function libraryItemsQuery(id: Id, q: LibraryItemsQuery, enabled: boolean) {
  return {
    queryKey: keys.libraryItems(id, q),
    queryFn: ({ signal }: { signal: AbortSignal }) =>
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
    placeholderData: (prev: LibraryItemsResponse | undefined, prevQuery?: { queryKey: readonly unknown[] }) =>
      prevQuery?.queryKey[1] === id ? prev : undefined,
  };
}

export interface LibraryItemsState {
  /** `items` may be shorter than `total` while the rest is loading. */
  data: LibraryItemsResponse | undefined;
  isPending: boolean;
  isError: boolean;
  error: unknown;
  refetch: () => Promise<unknown>;
}

/** A whole library listing: the first {@link FIRST_PAGE} items, then the rest. */
export function useLibraryItems(id: Id, q: LibraryItemsQuery, enabled = true): LibraryItemsState {
  const first = useQuery(libraryItemsQuery(id, { ...q, offset: undefined, limit: FIRST_PAGE }, enabled));
  const fresh = first.data !== undefined && !first.isPlaceholderData;
  const more = fresh && first.data.total > FIRST_PAGE;
  const rest = useQuery(libraryItemsQuery(id, { ...q, offset: FIRST_PAGE, limit: undefined }, enabled && more));

  const data = useMemo(() => {
    if (!first.data) return undefined;
    // While a new sort/filter loads both halves are the old list's; once the
    // new first page is in, never append the old rest to it.
    const tail = !fresh ? rest.data : more && !rest.isPlaceholderData ? rest.data : undefined;
    return tail ? { total: first.data.total, items: first.data.items.concat(tail.items) } : first.data;
  }, [first.data, fresh, more, rest.data, rest.isPlaceholderData]);

  return {
    data,
    isPending: first.isPending,
    isError: first.isError || rest.isError,
    error: first.error ?? rest.error,
    refetch: () => Promise.all([first.refetch(), more ? rest.refetch() : null]),
  };
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

/** Seek previews of a file. They are made in the background on first use, so keep asking until they exist. */
export function useTrickplay(fileId: Id | null | undefined) {
  return useQuery({
    queryKey: ["trickplay", fileId ?? 0],
    queryFn: ({ signal }) => api.get<Trickplay>(`/api/files/${fileId}/trickplay`, { signal }),
    enabled: fileId != null && fileId > 0,
    retry: false,
    staleTime: Infinity,
    refetchInterval: (query) => (query.state.data ? false : 20_000),
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

export function useSetRating() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, rating }: { id: Id; rating: number | null }) => api.put<void>(`/api/items/${id}/rating`, { rating }),
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
    queryFn: ({ signal }) => api.get<SearchResults>(`/api/search${qs({ q: trimmed, limit })}`, { signal }),
    enabled: trimmed.length > 0,
    placeholderData: (prev) => prev,
  });
}

/** Whose plays: your own (default), one user's or everyone's (administrators). */
export interface HistoryScope {
  all?: boolean;
  userId?: Id;
}

export function useHistory(scope: HistoryScope, pageSize = 20) {
  return useInfiniteQuery({
    queryKey: keys.history(scope, pageSize),
    queryFn: ({ pageParam, signal }) =>
      api.get<HistoryPage>(
        `/api/history${qs({ all: scope.all ? 1 : undefined, user_id: scope.userId, offset: pageParam, limit: pageSize })}`,
        { signal },
      ),
    initialPageParam: 0,
    getNextPageParam: (last, pages) => {
      const loaded = pages.reduce((n, p) => n + p.entries.length, 0);
      return loaded < last.total ? loaded : undefined;
    },
  });
}

export function useDeleteHistory() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: Id) => api.del<void>(`/api/history/${id}`),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["history"] }),
  });
}

/** "More Like This" for a movie or show. */
export function useRelated(id: Id, enabled = true) {
  return useQuery({
    queryKey: keys.related(id),
    queryFn: ({ signal }) => api.get<{ items: ItemSummary[] }>(`/api/items/${id}/related`, { signal }),
    select: (d) => d.items,
    enabled,
    staleTime: 5 * 60_000,
  });
}

export function usePerson(id: Id) {
  return useQuery({
    queryKey: keys.person(id),
    queryFn: ({ signal }) => api.get<PersonDetail>(`/api/people/${id}`, { signal }),
    enabled: Number.isFinite(id) && id > 0,
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

export interface NewUserInput {
  username: string;
  password?: string;
  display_name?: string;
  is_admin?: boolean;
  managed?: boolean;
  home?: boolean;
  all_libraries?: boolean;
  library_ids?: Id[];
  max_age?: number | null;
  pin?: string;
}

export function useCreateUser() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (body: NewUserInput) => api.post<User>("/api/users", body),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: keys.users });
      qc.invalidateQueries({ queryKey: keys.profiles });
    },
  });
}

/** Only the fields present are changed; `null` clears nullable ones. */
export interface UserPatch {
  password?: string;
  display_name?: string | null;
  is_admin?: boolean;
  home?: boolean;
  disabled?: boolean;
  all_libraries?: boolean;
  library_ids?: Id[];
  max_age?: number | null;
  pin?: string | null;
}

export function useUpdateUser() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, ...body }: UserPatch & { id: Id }) => api.patch<User>(`/api/users/${id}`, body),
    onSuccess: (user) => {
      qc.invalidateQueries({ queryKey: keys.users });
      qc.invalidateQueries({ queryKey: keys.profiles });
      qc.setQueryData<User | undefined>(keys.me, (me) => (me && me.id === user.id ? user : me));
    },
  });
}

export function useDeleteUser() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: Id) => api.del<void>(`/api/users/${id}`),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: keys.users });
      qc.invalidateQueries({ queryKey: keys.profiles });
    },
  });
}

export function useUserDevices(id: Id | null) {
  return useQuery({
    queryKey: keys.userDevices(id ?? 0),
    queryFn: () => api.get<Device[]>(`/api/users/${id}/devices`),
    enabled: id != null,
  });
}

export function useSignOutEverywhere() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: Id) => api.del<void>(`/api/users/${id}/devices`),
    onSuccess: (_data, id) => qc.invalidateQueries({ queryKey: keys.userDevices(id) }),
  });
}

// ---------------------------------------------------------------------------
// Invitations
// ---------------------------------------------------------------------------

export function useInvites() {
  return useQuery({ queryKey: keys.invites, queryFn: () => api.get<Invite[]>("/api/invites") });
}

export interface NewInviteInput {
  expires_in_days?: number;
  uses?: number;
  all_libraries?: boolean;
  library_ids?: Id[];
  max_age?: number | null;
  note?: string;
}

export function useCreateInvite() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (body: NewInviteInput) => api.post<Invite>("/api/invites", body),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.invites }),
  });
}

export function useDeleteInvite() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (code: string) => api.del<void>(`/api/invites/${encodeURIComponent(code)}`),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.invites }),
  });
}

/** Public: is this invitation still valid? */
export function useInviteCheck(code: string) {
  return useQuery({
    queryKey: keys.invite(code),
    queryFn: () => api.get<InviteCheck>(`/api/invites/${encodeURIComponent(code)}`, { noAuthRedirect: true }),
    retry: false,
  });
}

export function useAcceptInvite(code: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (body: { username: string; password: string; display_name?: string }) =>
      api.post<LoginResponse>(`/api/invites/${encodeURIComponent(code)}/accept`, body, { noAuthRedirect: true }),
    onSuccess: (res) => {
      const server = qc.getQueryData<ServerStatus>(keys.server);
      qc.clear();
      if (server) qc.setQueryData(keys.server, server);
      qc.setQueryData(keys.me, res.user);
    },
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

export function useStorageUsage() {
  return useQuery({ queryKey: keys.storage, queryFn: () => api.get<StorageUsage>("/api/system/storage"), staleTime: 30_000 });
}

export function useCleanStorage() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: () => api.post<CleanReport>("/api/system/storage/clean"),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.storage }),
  });
}

export function useBackups() {
  return useQuery({ queryKey: keys.backups, queryFn: () => api.get<Backups>("/api/system/backups") });
}

export function useCreateBackup() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: () => api.post<Backup>("/api/system/backups"),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.backups }),
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
