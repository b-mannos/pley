import { atHome } from "../auth/addresses";
import { SignedOutError, type Tokens } from "../auth/tokens";
import { Cancel, NetworkError, TIMED_OUT, withQuery, type HttpRequest, type HttpResponse, type Transport } from "../net/transport";
import type {
  Collection,
  Hub,
  HistoryEntry,
  HouseholdProfile,
  Id,
  ItemDetail,
  ItemSummary,
  Library,
  LibrarySort,
  PersonDetail,
  PersonSummary,
  PlaybackInfo,
  PlaybackRequest,
  PlaybackState,
  Playlist,
  PlaylistEntry,
  SessionStatus,
  SubtitleMode,
  Trickplay,
  User,
} from "./types";

/** An answer that isn't 2xx, with pley's `{ code, message }`. */
export class ApiError extends Error {
  readonly status: number;
  readonly code: string;
  constructor(status: number, code: string, message: string) {
    super(message);
    this.name = "ApiError";
    this.status = status;
    this.code = code;
  }
}

/** A playback session stopped from elsewhere (`410 session_stopped`), with what the one who stopped it wrote. */
export class SessionStoppedError extends ApiError {
  /** What the one who stopped it wrote, if anything. */
  readonly note: string | null;
  constructor(note: string | null) {
    super(410, "session_stopped", note ?? "Playback was stopped by the server's administrator");
    this.name = "SessionStoppedError";
    this.note = note;
  }
}

export function isApiError(e: unknown, status?: number): e is ApiError {
  return e instanceof ApiError && (status === undefined || e.status === status);
}

/** What to show for an error, as a sentence. */
export function errorMessage(e: unknown): string {
  const text = e instanceof Error ? e.message : String(e);
  return text ? text.charAt(0).toUpperCase() + text.slice(1) : text;
}

/** The server answered nothing: no connection, no answer in time, or a gateway in front of it saying it has none (502–504). */
export function unreachable(e: unknown): boolean {
  if (e instanceof NetworkError) return !e.cancelled;
  return e instanceof ApiError && (e.status === 502 || e.status === 503 || e.status === 504);
}

/** Why the server answered nothing, as the Android app says it: "Nothing answers there." or "No answer in time." */
export function unreachableReason(e: unknown): string {
  return e instanceof NetworkError && e.message === TIMED_OUT ? "No answer in time." : "Nothing answers there.";
}

/** A server at home accepts at once or never: after this long without an answer, a quick question tells which. */
export const HOME_WAIT_MS = 3_000;

/**
 * `req` to a server on the home network, which a TV can't give a connect
 * timeout of its own (the Android app gives it 3 s): when it hasn't
 * answered in 3 s, the server is asked a quick question, and if that gets
 * no answer in 3 s either, `req` gives up with "no answer in time". A
 * server that is only slow (a conversion starting) answers the question,
 * and `req` waits as long as it was going to.
 */
function watched(http: Transport, req: HttpRequest, ping: string): Promise<HttpResponse> {
  const own = new Cancel();
  const forget = req.cancel?.onCancel(() => own.cancel());
  return new Promise<HttpResponse>((resolve, reject) => {
    let done = false;
    let timer: ReturnType<typeof setTimeout> | null = null;
    const finish = (then: () => void) => {
      if (done) return;
      done = true;
      if (timer !== null) clearTimeout(timer);
      forget?.();
      then();
    };
    http({ ...req, cancel: own }).then(
      (res) => finish(() => resolve(res)),
      (e: unknown) => finish(() => reject(e)),
    );
    timer = setTimeout(() => {
      timer = null;
      http({ method: "GET", url: ping, headers: { Accept: "application/json" }, responseType: "json", timeoutMs: HOME_WAIT_MS, cancel: own }).then(
        () => undefined,
        (e: unknown) => {
          if (!(e instanceof NetworkError) || e.cancelled) return;
          finish(() => {
            own.cancel();
            reject(new NetworkError(TIMED_OUT));
          });
        },
      );
    }, HOME_WAIT_MS);
  });
}

interface CallOptions {
  method?: "GET" | "POST" | "PUT" | "PATCH" | "DELETE";
  query?: Record<string, string | number | boolean | null | undefined>;
  body?: unknown;
  cancel?: Cancel;
  timeoutMs?: number;
  responseType?: "json" | "blob" | "text" | "arraybuffer";
  headers?: Record<string, string>;
}

/**
 * pley's API for one server, as one signed-in app: every request carries
 * the access token in `Authorization: Bearer` (the only place pley takes
 * an app's token), refreshed once on `invalid_token`.
 */
export class Api {
  readonly server: string;
  private readonly tokens: Tokens;
  private readonly http: Transport;
  /** The server is on the home network: a request it doesn't answer within 3 s gets a quick question. */
  private readonly homeNetwork: boolean;

  constructor(server: string, tokens: Tokens, http: Transport) {
    this.server = server;
    this.tokens = tokens;
    this.http = http;
    this.homeNetwork = atHome(server);
  }

  /** An absolute address on this server for `path` ("/api/…"). */
  url(path: string): string {
    return /^https?:\/\//.test(path) ? path : this.server + path;
  }

  /** The raw answer of an authenticated request; errors as [ApiError]. */
  async send(path: string, opts: CallOptions = {}): Promise<HttpResponse> {
    const token = await this.tokens.accessToken();
    if (!token) throw new SignedOutError();
    let res = await this.once(path, opts, token);
    if (res.status === 401 && invalidToken(res)) {
      // Refused just after a refresh: the server no longer takes this sign-in.
      if (await this.tokens.refusedAfterRefresh(token)) throw new SignedOutError();
      const fresh = await this.tokens.refresh(token);
      res = await this.once(path, opts, fresh);
    }
    if (res.status >= 200 && res.status < 300) return res;
    throw toError(res);
  }

  private once(path: string, opts: CallOptions, token: string): Promise<HttpResponse> {
    const headers: Record<string, string> = { Accept: "application/json", Authorization: `Bearer ${token}` };
    let body: string | null = null;
    if (opts.body !== undefined) {
      headers["Content-Type"] = "application/json";
      body = JSON.stringify(opts.body);
    }
    if (opts.headers) for (const k in opts.headers) headers[k] = opts.headers[k]!;
    const req: HttpRequest = {
      method: opts.method ?? "GET",
      url: withQuery(this.url(path), opts.query),
      headers,
      body,
      responseType: opts.responseType ?? "json",
      timeoutMs: opts.timeoutMs,
      cancel: opts.cancel,
    };
    return this.homeNetwork ? watched(this.http, req, this.url("/api/server")) : this.http(req);
  }

  async get<T>(path: string, opts: CallOptions = {}): Promise<T> {
    return (await this.send(path, opts)).body as T;
  }

  async call(method: "POST" | "PUT" | "PATCH" | "DELETE", path: string, body?: unknown, cancel?: Cancel): Promise<unknown> {
    return (await this.send(path, { method, body, cancel })).body;
  }

  // ---- account

  me(cancel?: Cancel): Promise<User> {
    return this.get("/api/auth/me", { cancel });
  }

  profiles(cancel?: Cancel): Promise<HouseholdProfile[]> {
    return this.get("/api/oauth/profiles", { cancel });
  }

  setTrackDefaults(audio: string | null, subtitle: string | null, mode: SubtitleMode): Promise<unknown> {
    return this.call("PUT", "/api/me/track-defaults", { audio_language: audio, subtitle_language: subtitle, subtitle_mode: mode });
  }

  // ---- browsing

  home(cancel?: Cancel): Promise<{ hubs: Hub[] }> {
    return this.get("/api/hubs/home", { cancel });
  }

  libraries(cancel?: Cancel): Promise<Library[]> {
    return this.get("/api/libraries", { cancel });
  }

  libraryItems(
    id: Id,
    query: { sort?: LibrarySort; order?: "asc" | "desc"; watched?: boolean; genre?: string; year?: number; offset?: number; limit?: number },
    cancel?: Cancel,
  ): Promise<{ total: number; items: ItemSummary[] }> {
    return this.get(`/api/libraries/${id}/items`, { query, cancel });
  }

  libraryHubs(id: Id, cancel?: Cancel): Promise<{ hubs: Hub[] }> {
    return this.get(`/api/libraries/${id}/hubs`, { cancel });
  }

  libraryFilters(id: Id, cancel?: Cancel): Promise<{ genres: string[]; years: number[] }> {
    return this.get(`/api/libraries/${id}/filters`, { cancel });
  }

  collections(libraryId: Id, cancel?: Cancel): Promise<{ collections: Collection[] }> {
    return this.get(`/api/libraries/${libraryId}/collections`, { cancel });
  }

  collection(id: Id, cancel?: Cancel): Promise<Collection & { items: ItemSummary[] }> {
    return this.get(`/api/collections/${id}`, { cancel });
  }

  item(id: Id, cancel?: Cancel): Promise<ItemDetail> {
    return this.get(`/api/items/${id}`, { cancel });
  }

  children(id: Id, cancel?: Cancel): Promise<{ items: ItemSummary[] }> {
    return this.get(`/api/items/${id}/children`, { cancel });
  }

  onDeck(id: Id, cancel?: Cancel): Promise<{ item: ItemSummary | null }> {
    return this.get(`/api/items/${id}/on-deck`, { cancel });
  }

  next(id: Id, cancel?: Cancel): Promise<{ item: ItemSummary | null }> {
    return this.get(`/api/items/${id}/next`, { cancel });
  }

  related(id: Id, cancel?: Cancel): Promise<{ items: ItemSummary[] }> {
    return this.get(`/api/items/${id}/related`, { query: { limit: 20 }, cancel });
  }

  person(id: Id, cancel?: Cancel): Promise<PersonDetail> {
    return this.get(`/api/people/${id}`, { cancel });
  }

  search(q: string, cancel?: Cancel): Promise<{ items: ItemSummary[]; people?: PersonSummary[] }> {
    return this.get("/api/search", { query: { q, limit: 60 }, cancel });
  }

  playlists(cancel?: Cancel): Promise<{ playlists: Playlist[] }> {
    return this.get("/api/playlists", { cancel });
  }

  playlist(id: Id, cancel?: Cancel): Promise<Playlist & { entries: PlaylistEntry[] }> {
    return this.get(`/api/playlists/${id}`, { cancel });
  }

  /** The collections an item is part of. */
  itemCollections(id: Id, cancel?: Cancel): Promise<{ collections: { id: Id; title: string }[] }> {
    return this.get(`/api/items/${id}/collections`, { cancel });
  }

  addToPlaylist(playlistId: Id, itemIds: Id[]): Promise<{ added: number }> {
    return this.call("POST", `/api/playlists/${playlistId}/items`, { item_ids: itemIds }) as Promise<{ added: number }>;
  }

  createPlaylist(title: string, kind: "video" | "audio" | null, itemIds: Id[]): Promise<Playlist> {
    return this.call("POST", "/api/playlists", { title, ...(kind ? { kind } : {}), item_ids: itemIds }) as Promise<Playlist>;
  }

  /** A smart playlist: a library's listing by `filter` (Plex's syntax), which keeps up with the library by itself. */
  createSmartPlaylist(title: string, libraryId: Id, filter: string): Promise<Playlist> {
    return this.call("POST", "/api/playlists", { title, smart_filter: filter, library_id: libraryId }) as Promise<Playlist>;
  }

  // ---- watching

  history(offset: number, limit: number, cancel?: Cancel): Promise<{ total: number; entries: HistoryEntry[] }> {
    return this.get("/api/history", { query: { offset, limit }, cancel });
  }

  removeHistory(id: Id): Promise<unknown> {
    return this.call("DELETE", `/api/history/${id}`);
  }

  addToMyList(id: Id): Promise<unknown> {
    return this.call("PUT", `/api/watchlist/${id}`);
  }

  removeFromMyList(id: Id): Promise<unknown> {
    return this.call("DELETE", `/api/watchlist/${id}`);
  }

  hideFromContinueWatching(id: Id): Promise<unknown> {
    return this.call("DELETE", `/api/continue-watching/${id}`);
  }

  /** The person's rating, 0–10 (halves), or none. */
  rate(id: Id, rating: number | null): Promise<unknown> {
    return this.call("PUT", `/api/items/${id}/rating`, rating === null ? {} : { rating });
  }

  markWatched(id: Id): Promise<unknown> {
    return this.call("POST", `/api/items/${id}/watched`);
  }

  markUnwatched(id: Id): Promise<unknown> {
    return this.call("POST", `/api/items/${id}/unwatched`);
  }

  prefetch(id: Id): Promise<unknown> {
    return this.call("POST", `/api/items/${id}/prefetch`);
  }

  setStreams(itemId: Id, fileId: Id | null, audio: number | null, subtitle: number | null): Promise<unknown> {
    return this.call("PUT", `/api/items/${itemId}/streams`, { file_id: fileId ?? undefined, audio_index: audio, subtitle_index: subtitle });
  }

  // ---- playback

  play(req: PlaybackRequest, cancel?: Cancel): Promise<PlaybackInfo> {
    return this.send("/api/playback", { method: "POST", body: req, cancel, timeoutMs: 60_000 }).then((r) => r.body as PlaybackInfo);
  }

  sessionStatus(session: string, cancel?: Cancel, timeoutMs?: number): Promise<SessionStatus> {
    return this.get(`/api/playback/${encodeURIComponent(session)}`, { cancel, timeoutMs });
  }

  /** Whether the server answers at all (a player reconnecting without a session to ask about). */
  reachable(timeoutMs: number): Promise<unknown> {
    return this.get("/api/server", { timeoutMs });
  }

  stopSession(session: string): Promise<unknown> {
    return this.call("DELETE", `/api/playback/${encodeURIComponent(session)}`);
  }

  progress(itemId: Id, positionMs: number, durationMs: number | null, state: PlaybackState, session: string | null): Promise<unknown> {
    return this.call("POST", `/api/items/${itemId}/progress`, {
      position_ms: Math.max(0, Math.round(positionMs)),
      duration_ms: durationMs && durationMs > 0 ? Math.round(durationMs) : undefined,
      state,
      session_id: session ?? undefined,
    });
  }

  trickplay(fileId: Id, cancel?: Cancel): Promise<Trickplay> {
    return this.get(`/api/files/${fileId}/trickplay`, { cancel });
  }
}

function invalidToken(res: HttpResponse): boolean {
  const challenge = res.header("WWW-Authenticate") ?? "";
  // A bare 401 from an older server counts too: the token is what failed.
  return !challenge || /invalid_token/.test(challenge);
}

function toError(res: HttpResponse): ApiError {
  let code = statusCode(res.status);
  let message = `the server refused (HTTP ${res.status})`;
  const body = res.body;
  if (body && typeof body === "object" && !(body instanceof Blob)) {
    const o = body as { code?: unknown; message?: unknown };
    if (typeof o.code === "string") code = o.code;
    if (typeof o.message === "string") message = o.message;
  }
  if (res.status === 410 && code === "session_stopped") {
    const note = body && typeof body === "object" ? (body as { message?: unknown }).message : null;
    return new SessionStoppedError(typeof note === "string" ? note : null);
  }
  return new ApiError(res.status, code, message);
}

function statusCode(status: number): string {
  switch (status) {
    case 400:
      return "bad_request";
    case 401:
      return "unauthorized";
    case 403:
      return "forbidden";
    case 404:
      return "not_found";
    case 409:
      return "conflict";
    case 410:
      return "gone";
    case 429:
      return "too_many_requests";
    case 503:
      return "unavailable";
    default:
      return status >= 500 ? "internal" : "bad_request";
  }
}

export { Cancel, NetworkError };
