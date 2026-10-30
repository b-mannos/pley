import type { DiscoverDetail, DiscoverSeason, DiscoverTitle, MediaRequest, RequestList, RequestSummary } from "@/api/requests";

/** Requests on, for an account that asks an administrator, without limits. */
export function makeSummary(overrides: Partial<RequestSummary> = {}): RequestSummary {
  return { enabled: true, access: "ask", movies_left: null, seasons_left: null, movies_next_at: null, seasons_next_at: null, pending: 0, ...overrides };
}

/** A movie on TMDB no library has. */
export function makeTitle(overrides: Partial<DiscoverTitle> = {}): DiscoverTitle {
  return {
    kind: "movie",
    tmdb_id: 603,
    title: "The Matrix",
    year: 1999,
    overview: "A hacker learns what the world is.",
    poster_url: "https://image.tmdb.org/t/p/w342/matrix.jpg",
    backdrop_url: "https://image.tmdb.org/t/p/w1280/matrix.jpg",
    rating: 8.2,
    status: "none",
    item_id: null,
    request_id: null,
    ...overrides,
  };
}

export function makeSeason(number: number, overrides: Partial<DiscoverSeason> = {}): DiscoverSeason {
  return {
    number,
    name: number === 0 ? "Specials" : `Season ${number}`,
    episodes: 10,
    aired: 10,
    air_date: "2020-01-01",
    poster_url: null,
    in_library: 0,
    requested: false,
    ...overrides,
  };
}

export function makeDetail(overrides: Partial<DiscoverDetail> = {}): DiscoverDetail {
  return {
    ...makeTitle(),
    original_title: null,
    tagline: null,
    genres: ["Action"],
    release_date: "1999-03-31",
    runtime_minutes: 136,
    certification: "R",
    network: "Warner Bros.",
    show_status: null,
    anime: false,
    cast: [],
    seasons: null,
    requests: [],
    ...overrides,
  };
}

let nextId = 1;

/** A pending request for the movie, by account 2 (Ann). */
export function makeRequest(overrides: Partial<MediaRequest> = {}): MediaRequest {
  return {
    id: overrides.id ?? nextId++,
    kind: "movie",
    tmdb_id: 603,
    title: "The Matrix",
    year: 1999,
    poster_url: null,
    backdrop_url: null,
    seasons: null,
    available_seasons: [],
    anime: false,
    status: "pending",
    reason: null,
    requested_by: { id: 2, name: "Ann", avatar_url: null, avatar_color: null },
    decided_by: null,
    decided_at: null,
    created_at: 1_700_000_000,
    updated_at: 1_700_000_000,
    available_at: null,
    item_id: null,
    download: null,
    ...overrides,
  };
}

/** A page of `requests`, counted as they are. */
export function makeRequestList(requests: MediaRequest[], total = requests.length): RequestList {
  const counts = { pending: 0, approved: 0, declined: 0, failed: 0, available: 0 };
  for (const r of requests) counts[r.status] += 1;
  return { total, counts, requests };
}
