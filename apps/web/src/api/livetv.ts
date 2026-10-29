// Live TV (docs/API.md, "Live TV"): types and React Query hooks.

import { queryOptions, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api, qs } from "./client";
import type { Id, Timestamp } from "./types";

export interface LiveSummary {
  /** Live TV is open to this account and has channels. */
  available: boolean;
  /** Open to this account (it may have no channels yet). */
  allowed: boolean;
  channels: number;
  /** Sources set up (administrators only; 0 for others). */
  sources: number;
  /** The channels watched last, newest first. */
  recent: Id[];
}

export interface Program {
  channel_id: Id;
  start: Timestamp;
  stop: Timestamp;
  title: string;
  subtitle: string | null;
  description: string | null;
  categories: string[];
  /** Same-origin picture URL, when the guide has one. */
  image: string | null;
  season: number | null;
  episode: number | null;
  is_new: boolean;
  is_live: boolean;
  rating: string | null;
}

export interface Channel {
  id: Id;
  source_id: Id;
  name: string;
  number: string | null;
  group: string;
  /** Same-origin logo URL, when the channel has one. */
  logo: string | null;
  favorite: boolean;
  /** The guide has programmes for it. */
  has_guide: boolean;
  now: Program | null;
  next: Program | null;
}

export interface Guide {
  from: Timestamp;
  to: Timestamp;
  /** Programmes by channel id. */
  programs: Record<string, Program[]>;
}

export interface Watching {
  session_id: string;
  /** The live HLS playlist. */
  url: string;
  kind: "hls" | "stream";
  channel: Channel;
}

// ---------------------------------------------------------------- admin

export type LiveAccess = "everyone" | "admins";

export interface LiveSettings {
  access: LiveAccess;
  /** Profiles with an age limit see Live TV too. */
  age_limited: boolean;
  deinterlace: boolean;
  /** The most lines a transcoded channel has; null: as many as it has. */
  max_height: number | null;
  timeshift_minutes: number;
  refresh_hours: number;
  guide_days: number;
}

export type SourceKind = "m3u" | "xtream";

export interface XtreamAccount {
  status: string;
  expires_at: Timestamp | null;
  max_connections: number | null;
  active_connections: number | null;
  trial: boolean;
}

export interface LiveSource {
  id: Id;
  name: string;
  kind: SourceKind;
  url: string;
  username: string;
  password_set: boolean;
  guide_url: string;
  user_agent: string;
  /** 0: no limit. */
  max_connections: number;
  enabled: boolean;
  created_at: Timestamp;
  channels_at: Timestamp | null;
  channels_error: string | null;
  guide_at: Timestamp | null;
  guide_error: string | null;
  account: XtreamAccount | null;
  channel_count: number;
  hidden_count: number;
  guided_count: number;
  connections_in_use: number;
  refreshing: boolean;
}

export interface SourceInput {
  name: string;
  kind: SourceKind;
  url: string;
  username: string;
  /** Omitted: the saved one is kept. */
  password?: string;
  guide_url: string;
  user_agent: string;
  max_connections: number;
  enabled: boolean;
}

export interface SourceTest {
  channels: number;
  groups: number;
  guide_url: string | null;
  account: XtreamAccount | null;
}

export interface LineupChannel {
  id: Id;
  source_id: Id;
  name: string;
  number: string | null;
  group: string;
  logo: string | null;
  guide_id: string;
  hidden: boolean;
  source_name: string;
  source_number: string | null;
  source_guide_id: string;
  matched_guide_id: string | null;
  custom_name: string | null;
  custom_number: string | null;
  custom_guide_id: string | null;
  has_guide: boolean;
}

export interface LineupPatch {
  ids: Id[];
  hidden?: boolean;
  /** "" goes back to the source's. */
  name?: string;
  number?: string;
  guide_id?: string;
}

export interface LivePlan {
  video_codec: string | null;
  width: number | null;
  height: number | null;
  frame_rate: number | null;
  interlaced: boolean;
  audio_codec: string | null;
  audio_channels: number | null;
  /** "copy", or the encoder transcoding it. */
  video: string;
  audio: string;
}

export interface TunerStatus {
  channel_id: Id;
  channel_name: string;
  source_id: Id;
  source_name: string;
  started_at: Timestamp;
  connected: boolean;
  error: string | null;
  bytes_in: number;
  kbps: number;
  viewers: { session_id: string; user_id: Id; username: string; kind: "hls" | "stream"; started_at: Timestamp }[];
  idle: boolean;
  plan: LivePlan | null;
}

// ---------------------------------------------------------------- queries

export const liveKeys = {
  all: ["live"] as const,
  summary: ["live", "summary"] as const,
  channels: ["live", "channels"] as const,
  channel: (id: Id) => ["live", "channels", id] as const,
  guide: (from: Timestamp, to: Timestamp) => ["live", "guide", from, to] as const,
  // Administrators'.
  settings: ["live", "admin", "settings"] as const,
  sources: ["live", "admin", "sources"] as const,
  lineup: ["live", "admin", "lineup"] as const,
  tuners: ["live", "admin", "tuners"] as const,
};

export const liveQueries = {
  summary: () => queryOptions({ queryKey: liveKeys.summary, queryFn: () => api.get<LiveSummary>("/api/live") }),
  channels: () =>
    queryOptions({
      queryKey: liveKeys.channels,
      queryFn: ({ signal }) => api.get<Channel[]>("/api/live/channels", { signal }),
      // What's on now changes as time passes.
      refetchInterval: 60_000,
      refetchIntervalInBackground: false,
    }),
  channel: (id: Id) =>
    queryOptions({
      queryKey: liveKeys.channel(id),
      queryFn: ({ signal }) => api.get<Channel>(`/api/live/channels/${id}`, { signal }),
    }),
  guide: (from: Timestamp, to: Timestamp) =>
    queryOptions({
      queryKey: liveKeys.guide(from, to),
      queryFn: ({ signal }) => api.get<Guide>(`/api/live/guide${qs({ from, to })}`, { signal }),
      staleTime: 5 * 60_000,
    }),
  settings: () => queryOptions({ queryKey: liveKeys.settings, queryFn: () => api.get<LiveSettings>("/api/live/settings") }),
  sources: () => queryOptions({ queryKey: liveKeys.sources, queryFn: () => api.get<LiveSource[]>("/api/live/sources") }),
  lineup: () => queryOptions({ queryKey: liveKeys.lineup, queryFn: () => api.get<LineupChannel[]>("/api/live/lineup") }),
  tuners: () => queryOptions({ queryKey: liveKeys.tuners, queryFn: () => api.get<TunerStatus[]>("/api/live/tuners") }),
};

export function useLiveSummary(enabled = true) {
  return useQuery({ ...liveQueries.summary(), enabled });
}

export function useChannels() {
  return useQuery(liveQueries.channels());
}

export function useGuide(from: Timestamp, to: Timestamp) {
  return useQuery(liveQueries.guide(from, to));
}

export function useSetFavorite() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, favorite }: { id: Id; favorite: boolean }) =>
      favorite ? api.put<void>(`/api/live/channels/${id}/favorite`) : api.del<void>(`/api/live/channels/${id}/favorite`),
    onMutate: async ({ id, favorite }) => {
      await qc.cancelQueries({ queryKey: liveKeys.channels });
      qc.setQueryData<Channel[]>(liveKeys.channels, (list) => list?.map((c) => (c.id === id ? { ...c, favorite } : c)));
    },
    onSettled: () => qc.invalidateQueries({ queryKey: liveKeys.channels }),
  });
}

// ---------------------------------------------------------------- admin hooks

export function useLiveSettings() {
  return useQuery(liveQueries.settings());
}

export function useSaveLiveSettings() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (settings: LiveSettings) => api.put<LiveSettings>("/api/live/settings", settings),
    onSuccess: (data) => {
      qc.setQueryData(liveKeys.settings, data);
      void qc.invalidateQueries({ queryKey: liveKeys.summary });
    },
  });
}

export function useLiveSources() {
  return useQuery({
    ...liveQueries.sources(),
    // A source being read says so until it is done.
    refetchInterval: (q) => (q.state.data?.some((s) => s.refreshing) ? 2000 : false),
  });
}

function useInvalidateLive() {
  const qc = useQueryClient();
  return () => qc.invalidateQueries({ queryKey: liveKeys.all });
}

export function useAddSource() {
  const invalidate = useInvalidateLive();
  return useMutation({
    mutationFn: (input: SourceInput) => api.post<LiveSource>("/api/live/sources", input),
    onSuccess: invalidate,
  });
}

export function useUpdateSource() {
  const invalidate = useInvalidateLive();
  return useMutation({
    mutationFn: ({ id, input }: { id: Id; input: SourceInput }) => api.put<LiveSource>(`/api/live/sources/${id}`, input),
    onSuccess: invalidate,
  });
}

export function useDeleteSource() {
  const invalidate = useInvalidateLive();
  return useMutation({ mutationFn: (id: Id) => api.del<void>(`/api/live/sources/${id}`), onSuccess: invalidate });
}

export function useRefreshSource() {
  const invalidate = useInvalidateLive();
  return useMutation({ mutationFn: (id: Id) => api.post<void>(`/api/live/sources/${id}/refresh`), onSuccess: invalidate });
}

export function useTestSource() {
  return useMutation({
    mutationFn: (body: SourceInput & { source_id?: Id }) => api.post<SourceTest>("/api/live/sources/test", body),
  });
}

export function useLineup(enabled = true) {
  return useQuery({ ...liveQueries.lineup(), enabled });
}

export function usePatchLineup() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (patch: LineupPatch) => api.patch<void>("/api/live/lineup", patch),
    onMutate: async (patch) => {
      await qc.cancelQueries({ queryKey: liveKeys.lineup });
      const ids = new Set(patch.ids);
      qc.setQueryData<LineupChannel[]>(liveKeys.lineup, (list) =>
        list?.map((c) => (ids.has(c.id) && patch.hidden !== undefined ? { ...c, hidden: patch.hidden } : c)),
      );
    },
    onSettled: () => qc.invalidateQueries({ queryKey: liveKeys.all }),
  });
}

export function useTuners(enabled = true) {
  return useQuery({ ...liveQueries.tuners(), enabled, refetchInterval: 5000, refetchIntervalInBackground: false });
}

export function useStopTuner() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (channelId: Id) => api.del<void>(`/api/live/tuners/${channelId}`),
    onSuccess: () => qc.invalidateQueries({ queryKey: liveKeys.tuners }),
  });
}

/** Starts watching a channel on this device. */
export function watchChannel(id: Id, clientId: string): Promise<Watching> {
  return api.post<Watching>(`/api/live/channels/${id}/watch`, { client_id: clientId });
}

/** Stops watching (keepalive: also as the page goes away). */
export function leaveChannel(sessionId: string, keepalive = false): Promise<void> {
  return api.del<void>(`/api/live/sessions/${encodeURIComponent(sessionId)}`, { keepalive }).catch(() => undefined);
}

// ---------------------------------------------------------------- helpers

/** The programme's place between its start and stop at `now` (0–1). */
export function programProgress(p: Pick<Program, "start" | "stop">, now: number): number {
  return Math.max(0, Math.min(1, (now - p.start) / Math.max(1, p.stop - p.start)));
}

/** "S2 E14", when the guide numbers the episode. */
export function episodeLabel(p: Pick<Program, "season" | "episode">): string | null {
  if (p.season != null && p.episode != null) return `S${p.season} E${p.episode}`;
  if (p.episode != null) return `E${p.episode}`;
  return null;
}
