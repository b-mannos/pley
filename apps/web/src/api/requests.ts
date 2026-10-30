// Requests (docs/API.md): movies and shows found on TMDB, asked for,
// decided on by an administrator, and sent to Radarr or Sonarr.

import {
  queryOptions,
  useInfiniteQuery,
  useMutation,
  useQuery,
  useQueryClient,
  type InfiniteData,
  type QueryClient,
} from "@tanstack/react-query";
import { api, isApiError, qs } from "./client";
import type { Id, RequestAccess, Timestamp } from "./types";

export type { RequestAccess } from "./types";

export type TitleKind = "movie" | "show";
export type RequestStatus = "pending" | "approved" | "declined" | "failed" | "available";
export type DiscoverList = "trending" | "movies" | "shows" | "upcoming" | "anime";
export type ArrService = "radarr" | "sonarr";

export interface RequestSummary {
  /** Requests are on (and TMDB has a key). */
  enabled: boolean;
  /** This account's; always "auto" for administrators. */
  access: RequestAccess;
  /** Requests it may still make in the 7 days; `null`: no limit. */
  movies_left: number | null;
  seasons_left: number | null;
  /** With no movies left: when one more may be asked for. */
  movies_next_at: Timestamp | null;
  /** With no seasons left: likewise. */
  seasons_next_at: Timestamp | null;
  /** Administrators: requests waiting for a decision (others: 0). */
  pending: number;
}

export interface DiscoverTitle {
  kind: TitleKind;
  tmdb_id: number;
  title: string;
  year: number | null;
  overview: string;
  /** TMDB's, 342 pixels wide (500 in a `DiscoverDetail`). */
  poster_url: string | null;
  /** TMDB's, 1280 pixels wide. */
  backdrop_url: string | null;
  /** TMDB's users', 0–10; `null` with too few votes. */
  rating: number | null;
  /** For this account: a library has it, an active request asks for it, or neither. */
  status: "library" | "requested" | "none";
  /** In a library: the title, where this account may open it. */
  item_id: Id | null;
  /** Requested: the active request, where it's this account's (any for administrators). */
  request_id: Id | null;
}

export interface DiscoverPage {
  page: number;
  total_pages: number;
  results: DiscoverTitle[];
}

export interface DiscoverSeason {
  /** 0: specials. */
  number: number;
  name: string;
  /** Episodes TMDB lists. */
  episodes: number;
  /** Of those, the ones aired by now. */
  aired: number;
  /** Its first episode's ("2024-05-01"). */
  air_date: string | null;
  poster_url: string | null;
  /** Episodes of it a library has. */
  in_library: number;
  /** An active request asks for it. */
  requested: boolean;
}

export interface DiscoverDetail extends DiscoverTitle {
  /** Where it differs from `title`. */
  original_title: string | null;
  tagline: string | null;
  genres: string[];
  /** A movie's release, a show's first air date ("2024-05-01"). */
  release_date: string | null;
  /** Movies. */
  runtime_minutes: number | null;
  /** "PG-13", "TV-MA": the server's region's. */
  certification: string | null;
  /** A show's first network, a movie's first studio. */
  network: string | null;
  /** Shows: TMDB's ("Returning Series", "Ended"). */
  show_status: string | null;
  /** Requests for it go to anime's folder and profile. */
  anime: boolean;
  /** 12 at most. */
  cast: { name: string; character: string | null; photo_url: string | null }[];
  /** Shows, in order (specials, 0, first). */
  seasons: DiscoverSeason[] | null;
  /** For it: this account's, everyone's for administrators; newest first. */
  requests: MediaRequest[];
}

export interface RequestUser {
  id: Id;
  name: string;
  avatar_url: string | null;
  avatar_color: string | null;
}

export interface RequestDownload {
  /** 0–1, of everything of it queued. */
  progress: number;
  state: "queued" | "downloading" | "paused" | "importing" | "warning";
  /** The queue's estimate, where it has one. */
  eta: Timestamp | null;
  /** Shows: episodes queued (movies: 1). */
  episodes: number;
}

export interface MediaRequest {
  id: Id;
  kind: TitleKind;
  tmdb_id: number;
  /** As TMDB had it when asked. */
  title: string;
  year: number | null;
  poster_url: string | null;
  backdrop_url: string | null;
  /** Shows: the seasons asked for; `null` for movies. */
  seasons: number[] | null;
  /** Shows: those a library has whole. */
  available_seasons: number[];
  anime: boolean;
  status: RequestStatus;
  /** Declined: the administrator's (or `null`); failed: what went wrong. */
  reason: string | null;
  requested_by: RequestUser;
  /** Who approved or declined it (the maker where approved as made); `null` for an account deleted since. */
  decided_by: RequestUser | null;
  decided_at: Timestamp | null;
  created_at: Timestamp;
  updated_at: Timestamp;
  available_at: Timestamp | null;
  /** A library's title once there, where this account may open it. */
  item_id: Id | null;
  download: RequestDownload | null;
}

export interface RequestList {
  total: number;
  /** Each status's requests in the same list, whatever `status` keeps. */
  counts: Record<RequestStatus, number>;
  requests: MediaRequest[];
}

export interface RequestFilter {
  /** Only these; `null`: all. */
  status: RequestStatus | null;
  /** Administrators: only their own (others only ever see theirs). */
  mine: boolean;
}

/** What Radarr or Sonarr offers requests (`POST /api/settings/arr`). */
export interface ArrOptions {
  version: string;
  quality_profiles: { id: number; name: string }[];
  root_folders: { path: string; free_bytes: number | null }[];
  /** Sonarr 3 only; empty otherwise. */
  language_profiles: { id: number; name: string }[];
}

/** An address and key typed in (not saved yet); the saved ones where left out. */
export interface ArrConnection {
  service: ArrService;
  url?: string;
  api_key?: string;
}

export const requestKeys = {
  /** All of them: request events refresh everything here. */
  all: ["requests"] as const,
  summary: ["requests", "summary"] as const,
  discover: (list: DiscoverList) => ["requests", "discover", list] as const,
  search: (q: string) => ["requests", "search", q] as const,
  titles: ["requests", "title"] as const,
  title: (kind: TitleKind, tmdbId: number) => ["requests", "title", kind, tmdbId] as const,
  lists: ["requests", "list"] as const,
  list: (filter: RequestFilter) => ["requests", "list", filter] as const,
  /** With the settings: what the saved Radarr or Sonarr offers. */
  arr: (service: ArrService) => ["settings", "arr", service] as const,
};

/** An approved request with a download queued or running: its progress comes again every 10 s. */
export function downloading(request: MediaRequest): boolean {
  return request.status === "approved" && request.download !== null;
}

const POLL_MS = 10_000;
const LIST_PAGE = 50;

export const requestQueries = {
  summary: () =>
    queryOptions({
      queryKey: requestKeys.summary,
      queryFn: () => api.get<RequestSummary>("/api/requests/summary"),
      staleTime: 60_000,
    }),
  title: (kind: TitleKind, tmdbId: number) =>
    queryOptions({
      queryKey: requestKeys.title(kind, tmdbId),
      queryFn: ({ signal }) => api.get<DiscoverDetail>(`/api/discover/${kind}/${tmdbId}`, { signal }),
      refetchInterval: (query) => (query.state.data?.requests.some(downloading) ? POLL_MS : false),
    }),
};

/**
 * Why requests can't be made here, from an answer's error: they aren't set
 * up on this server (503 with `setup: false`), or not for this account
 * (403); `null` for any other failure.
 */
export function requestsUnavailable(err: unknown): "setup" | "forbidden" | null {
  if (isApiError(err, 503) && (err.body as { setup?: unknown } | undefined)?.setup === false) return "setup";
  if (isApiError(err, 403)) return "forbidden";
  return null;
}

/** TMDB refused the server's key or didn't answer (503 with `setup: true`): `message` says which. */
export function tmdbDown(err: unknown): boolean {
  return isApiError(err, 503) && (err.body as { setup?: unknown } | undefined)?.setup === true;
}

/** This account may look for titles and ask for them. */
export function mayRequest(summary: RequestSummary | undefined): summary is RequestSummary {
  return !!summary?.enabled && summary.access !== "off";
}

/**
 * Whether this account may ask, and how much more. Servers without
 * requests answer 404: no `data`, as when they're off.
 */
export function useRequestSummary(enabled = true) {
  return useQuery({ ...requestQueries.summary(), enabled });
}

function nextPage(last: DiscoverPage): number | undefined {
  return last.page < Math.min(last.total_pages, 500) ? last.page + 1 : undefined;
}

/** One of TMDB's lists, 20 titles a page. */
export function useDiscoverList(list: DiscoverList, enabled = true) {
  return useInfiniteQuery({
    queryKey: requestKeys.discover(list),
    queryFn: ({ pageParam, signal }) =>
      api.get<DiscoverPage>(`/api/discover/${list}${qs({ page: pageParam > 1 ? pageParam : null })}`, { signal }),
    initialPageParam: 1,
    getNextPageParam: nextPage,
    enabled,
  });
}

/** Movies and shows on TMDB matching `q`; the last answer stays while the next one comes. */
export function useDiscoverSearch(q: string, enabled = true) {
  const trimmed = q.trim();
  return useInfiniteQuery({
    queryKey: requestKeys.search(trimmed),
    queryFn: ({ pageParam, signal }) =>
      api.get<DiscoverPage>(`/api/discover/search${qs({ q: trimmed, page: pageParam > 1 ? pageParam : null })}`, { signal }),
    initialPageParam: 1,
    getNextPageParam: nextPage,
    enabled: enabled && trimmed.length > 0,
    placeholderData: (prev) => prev,
  });
}

/** A movie or show as TMDB has it, with what this account (or anyone, for administrators) asked of it. */
export function useDiscoverTitle(kind: TitleKind, tmdbId: number) {
  return useQuery({ ...requestQueries.title(kind, tmdbId), enabled: Number.isInteger(tmdbId) && tmdbId > 0 });
}

/** Requests, newest first, 50 at a time; again every 10 s while one shown downloads. */
export function useRequests(filter: RequestFilter) {
  return useInfiniteQuery({
    queryKey: requestKeys.list(filter),
    queryFn: ({ pageParam, signal }) =>
      api.get<RequestList>(
        `/api/requests${qs({ status: filter.status, mine: filter.mine ? 1 : null, offset: pageParam || null, limit: LIST_PAGE })}`,
        { signal },
      ),
    initialPageParam: 0,
    getNextPageParam: (last, pages) => {
      const loaded = pages.reduce((n, p) => n + p.requests.length, 0);
      return loaded < last.total ? loaded : undefined;
    },
    refetchInterval: (query) => (query.state.data?.pages.some((p) => p.requests.some(downloading)) ? POLL_MS : false),
  });
}

/**
 * A request's news (`next`; `null`: gone) in the lists and the titles'
 * pages at once, then everything here fetched again: statuses, counts, the
 * number waiting. Not waited for: what made the change says how it went
 * while it's still there (a request withdrawn leaves its list).
 */
function requestChanged(qc: QueryClient, id: Id | null, next: MediaRequest | null) {
  if (id !== null) {
    const swap = (list: MediaRequest[]) => (next ? list.map((r) => (r.id === id ? next : r)) : list.filter((r) => r.id !== id));
    qc.setQueriesData<InfiniteData<RequestList>>({ queryKey: requestKeys.lists }, (old) =>
      old ? { ...old, pages: old.pages.map((p) => ({ ...p, requests: swap(p.requests) })) } : old,
    );
    qc.setQueriesData<DiscoverDetail>({ queryKey: requestKeys.titles }, (old) => (old ? { ...old, requests: swap(old.requests) } : old));
  }
  void qc.invalidateQueries({ queryKey: requestKeys.all });
}

export function useCreateRequest() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (body: { kind: TitleKind; tmdb_id: number; seasons?: number[] }) => api.post<MediaRequest>("/api/requests", body),
    onSuccess: () => requestChanged(qc, null, null),
  });
}

/** Approves a pending request, or tries a failed one again. */
export function useApproveRequest() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, ...body }: { id: Id; quality_profile?: number; root_folder?: string }) =>
      api.post<MediaRequest>(`/api/requests/${id}/approve`, body),
    onSuccess: (request) => requestChanged(qc, request.id, request),
  });
}

export function useDeclineRequest() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, reason }: { id: Id; reason?: string }) => api.post<MediaRequest>(`/api/requests/${id}/decline`, reason ? { reason } : {}),
    onSuccess: (request) => requestChanged(qc, request.id, request),
  });
}

/** Withdraws your own request while it isn't approved (pending, declined or failed); administrators delete any. */
export function useDeleteRequest() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: Id) => api.del<void>(`/api/requests/${id}`),
    onSuccess: (_data, id) => requestChanged(qc, id, null),
  });
}

/** What the saved Radarr or Sonarr offers requests (`enabled` once an address and key are saved). */
export function useArrOptions(service: ArrService, enabled: boolean) {
  return useQuery({
    queryKey: requestKeys.arr(service),
    queryFn: () => api.post<ArrOptions>("/api/settings/arr", { service }),
    enabled,
    staleTime: 5 * 60_000,
  });
}

/** Asks Radarr or Sonarr at an address typed in what it offers (the settings aren't saved). */
export function useConnectArr() {
  return useMutation({
    mutationFn: (body: ArrConnection) => api.post<ArrOptions>("/api/settings/arr", body),
  });
}
