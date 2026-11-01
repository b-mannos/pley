import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ApiError } from "../src/api/client";
import type { MediaRequest, RequestList, RequestSummary, ServerEvent, User } from "../src/api/types";
import type { KeyValue } from "../src/auth/tokens";
import { NetworkError } from "../src/net/transport";
import type { Platform } from "../src/platform";
import { AppState, SUMMARY_POLL_MS } from "../src/state";

function memoryStore(): KeyValue {
  const kept = new Map<string, string>();
  return {
    get: (k) => kept.get(k) ?? null,
    set: (k, v) => void kept.set(k, v),
    remove: (k) => void kept.delete(k),
  };
}

function summary(over: Partial<RequestSummary> = {}): RequestSummary {
  return {
    enabled: true,
    access: "ask",
    movies_left: null,
    seasons_left: null,
    movies_limit: null,
    seasons_limit: null,
    movies_next_at: null,
    seasons_next_at: null,
    manage: false,
    pending: 0,
    failed: 0,
    unseen: 0,
    own: 1,
    changed_at: 100,
    sign_in_again: null,
    ...over,
  };
}

function request(id: number, over: Partial<MediaRequest> = {}): MediaRequest {
  return {
    id,
    kind: "movie",
    tmdb_id: 100 + id,
    title: `Title ${id}`,
    year: 2020,
    poster_url: null,
    backdrop_url: null,
    seasons: null,
    available_seasons: [],
    anime: false,
    status: "available",
    stage: "available",
    stage_at: null,
    reason: null,
    note: null,
    mine: true,
    watching: false,
    watchers: 0,
    unseen: true,
    actions: [],
    requested_by: null,
    decided_by: null,
    decided_at: null,
    created_at: 1,
    updated_at: 1_000 + id,
    available_at: 1_000 + id,
    retry_at: null,
    sent_to: null,
    item_id: 50 + id,
    download: null,
    ...over,
  };
}

/**
 * An AppState signed in to a server whose summary, unseen list and
 * requests the test decides, with a way to hand it the server's events and
 * to say its event stream went down or came back.
 */
function setup(store = memoryStore()) {
  const platform = { name: "webos", clientId: "test", storage: store } as unknown as Platform;
  const app = new AppState(platform, () => Promise.reject(new Error("no network in tests")));
  const asked: string[] = [];
  const server = {
    summary: (): Promise<RequestSummary> => Promise.resolve(summary()),
    unseen: (): Promise<MediaRequest[]> => Promise.resolve([]),
    request: (id: number): Promise<MediaRequest> => Promise.resolve(request(id)),
  };
  (app.api as { value: unknown }).value = {
    requestSummary: () => {
      asked.push("summary");
      return server.summary();
    },
    requests: (q: { unseen?: boolean; limit?: number }) => {
      asked.push(`requests unseen=${q.unseen ? 1 : 0} limit=${q.limit}`);
      return server.unseen().then((requests): RequestList => ({ total: requests.length, counts: {} as RequestList["counts"], requests, people: [] }));
    },
    request: (id: number) => {
      asked.push(`request ${id}`);
      return server.request(id);
    },
    home: () => Promise.resolve({ hubs: [] }),
    me: () => Promise.resolve({ id: 7, name: "Ann" }),
    libraries: () => Promise.resolve([]),
    profiles: () => Promise.resolve([]),
    reachable: () => Promise.reject(new Error("nothing answers there")),
  };
  (app as unknown as { tokens: unknown }).tokens = { signedIn: true };
  app.server.value = "https://pley.example.com";
  app.me.value = { id: 7, name: "Ann" } as User;
  const hidden = app as unknown as { handleEvent(e: ServerEvent): void; reconnected(): void; eventsLost(): void; events: unknown };
  const hear = (e: ServerEvent) => hidden.handleEvent(e);
  const streamDown = () => {
    hidden.events = {};
    hidden.eventsLost();
  };
  const streamBack = () => hidden.reconnected();
  return { app, server, asked, hear, streamDown, streamBack, store };
}

const summaries = (asked: string[]) => asked.filter((a) => a === "summary").length;

beforeEach(() => {
  vi.useFakeTimers();
});
afterEach(() => {
  vi.useRealTimers();
});

describe("the summary's gate", () => {
  it("keeps what it heard, so the rail has its entry at the next start before the server answers", async () => {
    const { app, store } = setup();
    await app.loadRequests();
    expect(app.requests.value?.enabled).toBe(true);
    const again = setup(store);
    expect(again.app.requests.value?.own).toBe(1);
  });

  it("takes a server that answers 404 or the older 403 for one without requests, and asks again only once the events reconnect", async () => {
    for (const refusal of [new ApiError(404, "not_found", "no such route"), new ApiError(403, "insufficient_scope", "scope")]) {
      const { app, server, asked, streamBack } = setup();
      server.summary = () => Promise.reject(refusal);
      await app.loadRequests();
      expect(app.requests.value).toBeNull();
      await app.loadRequests();
      expect(summaries(asked)).toBe(1);
      server.summary = () => Promise.resolve(summary());
      streamBack();
      await vi.advanceTimersByTimeAsync(0);
      expect(summaries(asked)).toBe(2);
      expect(app.requests.value?.enabled).toBe(true);
    }
  });

  it("keeps what it had while the server is out of reach", async () => {
    const { app, server } = setup();
    await app.loadRequests();
    server.summary = () => Promise.reject(new NetworkError("Can't reach the server"));
    await app.loadRequests();
    expect(app.requests.value?.enabled).toBe(true);
  });

  it("asks once more after one in flight when something changed meanwhile, not once per ask", async () => {
    const { app, asked } = setup();
    const a = app.loadRequests();
    void app.loadRequests();
    await a;
    expect(summaries(asked)).toBe(1);
    const b = app.loadRequests();
    void app.loadRequests(true);
    void app.loadRequests(true);
    await b;
    await vi.advanceTimersByTimeAsync(0);
    expect(summaries(asked)).toBe(3);
  });
});

describe("request events", () => {
  it("ask for the summary once, a second after a burst, and tell the pages once which title changed", async () => {
    const { app, asked, hear } = setup();
    const before = app.requestChanges.value.n;
    hear({ type: "requests_changed", request_id: 1, kind: "movie", tmdb_id: 101 });
    hear({ type: "requests_changed", request_id: 1, kind: "movie", tmdb_id: 101 });
    await vi.advanceTimersByTimeAsync(300);
    expect(app.requestChanges.value).toEqual({ n: before + 1, title: "movie:101" });
    expect(summaries(asked)).toBe(0);
    await vi.advanceTimersByTimeAsync(700);
    expect(summaries(asked)).toBe(1);
    // Many titles at once: the pages hear "any".
    hear({ type: "requests_changed", request_id: null, kind: null, tmdb_id: null });
    hear({ type: "requests_changed", request_id: 2, kind: "show", tmdb_id: 9 });
    await vi.advanceTimersByTimeAsync(300);
    expect(app.requestChanges.value.title).toBeNull();
  });

  it("move a title's badge in rows already drawn from its request, only while a page draws badges", async () => {
    const { app, server, asked, hear } = setup();
    hear({ type: "requests_changed", request_id: 3, kind: "movie", tmdb_id: 103 });
    await vi.advanceTimersByTimeAsync(0);
    expect(asked.filter((a) => a.startsWith("request "))).toEqual([]);
    const stop = app.watchMarks();
    server.request = (id) => Promise.resolve(request(id, { status: "declined", stage: "declined", item_id: null }));
    hear({ type: "requests_changed", request_id: 3, kind: "movie", tmdb_id: 103 });
    await vi.advanceTimersByTimeAsync(0);
    expect(app.marks.value["movie:103"]).toMatchObject({ status: "declined", item_id: null });
    server.request = () => Promise.reject(new ApiError(404, "not_found", "gone"));
    hear({ type: "requests_changed", request_id: 3, kind: "movie", tmdb_id: 103 });
    await vi.advanceTimersByTimeAsync(0);
    expect(app.marks.value["movie:103"]?.status).toBe("none");
    stop();
  });

  it("pop up what is ready to watch, once", async () => {
    const { app, hear } = setup();
    const ready: ServerEvent = { type: "request_available", request_id: 4, title: "Severance", kind: "show", tmdb_id: 104, item_id: 54, seasons: [2], complete: false };
    hear(ready);
    hear(ready);
    expect(app.notices.value.map((n) => n.text)).toEqual(["Season 2 of “Severance” is ready to watch."]);
    expect(app.notices.value[0]?.itemId).toBe(54);
    expect(app.marks.value["show:104"]).toMatchObject({ status: "partial", item_id: 54 });
    // The next batch of its seasons is news of its own, told once too.
    const more: ServerEvent = { ...ready, seasons: [3, 4] };
    hear(more);
    hear(more);
    hear({ ...ready, complete: true, seasons: null });
    expect(app.notices.value.map((n) => n.text)).toEqual([
      "Season 2 of “Severance” is ready to watch.",
      "Seasons 3–4 of “Severance” are ready to watch.",
      "“Severance” is ready to watch.",
    ]);
  });

  it("pop up nothing for decisions, new requests or failures: TVs show those as the number and in the lists", async () => {
    const { app, hear } = setup();
    hear({ type: "request_decided", request_id: 1, title: "A", kind: "movie", tmdb_id: 1, status: "declined", reason: null });
    hear({ type: "request_created", request_id: 2, title: "B", kind: "movie", tmdb_id: 2, seasons: null, requested_by: { id: 1, name: "Bo", avatar_url: null, avatar_color: null }, status: "pending" });
    hear({ type: "request_failed", request_id: 3, title: "C", kind: "movie", reason: "Radarr isn't answering", retry_at: null });
    await vi.advanceTimersByTimeAsync(2_000);
    expect(app.notices.value).toEqual([]);
  });

  it("keep the last three notices while they can't show", () => {
    const { app, hear } = setup();
    for (let id = 1; id <= 5; id++) hear({ type: "request_available", request_id: id, title: `T${id}`, item_id: id, complete: true });
    expect(app.notices.value.map((n) => n.text)).toEqual(["“T3” is ready to watch.", "“T4” is ready to watch.", "“T5” is ready to watch."]);
    app.dropNotice(app.notices.value[0]!.id);
    expect(app.notices.value.length).toBe(2);
  });
});

describe("while the events are down", () => {
  it("asks for the summary every 60 s while the app shows, and stops once they reconnect", async () => {
    const { app, asked, streamDown, streamBack } = setup();
    await app.loadRequests();
    streamDown();
    expect(app.eventsDown.value).toBe(true);
    await vi.advanceTimersByTimeAsync(SUMMARY_POLL_MS);
    expect(summaries(asked)).toBe(2);
    app.visible.value = false;
    await vi.advanceTimersByTimeAsync(SUMMARY_POLL_MS);
    expect(summaries(asked)).toBe(2);
    app.visible.value = true;
    await vi.advanceTimersByTimeAsync(SUMMARY_POLL_MS);
    expect(summaries(asked)).toBe(3);
    streamBack();
    await vi.advanceTimersByTimeAsync(0);
    expect(app.eventsDown.value).toBe(false);
    const after = summaries(asked);
    await vi.advanceTimersByTimeAsync(3 * SUMMARY_POLL_MS);
    expect(summaries(asked)).toBe(after);
  });

  it("tells the pages to ask again when the summary's changed_at moved", async () => {
    const { app, server, streamDown } = setup();
    await app.loadRequests();
    streamDown();
    const before = app.requestChanges.value.n;
    await vi.advanceTimersByTimeAsync(SUMMARY_POLL_MS);
    await vi.advanceTimersByTimeAsync(300);
    expect(app.requestChanges.value.n).toBe(before);
    server.summary = () => Promise.resolve(summary({ changed_at: 200 }));
    await vi.advanceTimersByTimeAsync(SUMMARY_POLL_MS);
    await vi.advanceTimersByTimeAsync(300);
    expect(app.requestChanges.value.n).toBe(before + 1);
  });
});

describe("news that came while the app wasn't listening", () => {
  it("is caught up with at start: one title by its words", async () => {
    const { app, server, asked } = setup();
    server.summary = () => Promise.resolve(summary({ unseen: 1 }));
    server.unseen = () => Promise.resolve([request(1)]);
    await app.loadRequests();
    await vi.advanceTimersByTimeAsync(0);
    expect(asked).toContain("requests unseen=1 limit=20");
    expect(app.notices.value.map((n) => [n.text, n.itemId])).toEqual([["“Title 1” is ready to watch.", 51]]);
  });

  it("says how many when several are ready, and never announces one twice", async () => {
    const { app, server, store } = setup();
    server.summary = () => Promise.resolve(summary({ unseen: 3 }));
    // A declined one is news too, but TVs pop up only what is ready.
    server.unseen = () => Promise.resolve([request(1), request(2, { kind: "show", stage: "partial", status: "approved", available_seasons: [1] }), request(3, { status: "declined", stage: "declined" })]);
    await app.loadRequests();
    await vi.advanceTimersByTimeAsync(0);
    expect(app.notices.value.map((n) => [n.text, n.itemId])).toEqual([["2 of your requests have news", null]]);
    // The next start, on the same server and profile: nothing new.
    const again = setup(store);
    again.server.summary = server.summary;
    again.server.unseen = server.unseen;
    await again.app.loadRequests();
    await vi.advanceTimersByTimeAsync(0);
    expect(again.app.notices.value).toEqual([]);
  });

  it("tells a show's seasons that came meanwhile, and none told as they came", async () => {
    const { app, hear, store } = setup();
    hear({ type: "request_available", request_id: 4, title: "Severance", kind: "show", tmdb_id: 104, item_id: 54, seasons: [1], complete: false });
    expect(app.notices.value.length).toBe(1);
    const severance = (seasons: number[]) => request(4, { kind: "show", title: "Severance", status: "approved", stage: "partial", available_seasons: seasons });
    // The next start, with only what was told: nothing.
    const again = setup(store);
    again.server.summary = () => Promise.resolve(summary({ unseen: 1 }));
    again.server.unseen = () => Promise.resolve([severance([1])]);
    await again.app.loadRequests();
    await vi.advanceTimersByTimeAsync(0);
    expect(again.app.notices.value).toEqual([]);
    // Another batch came while the app wasn't listening: told.
    const later = setup(store);
    later.server.summary = again.server.summary;
    later.server.unseen = () => Promise.resolve([severance([1, 2])]);
    await later.app.loadRequests();
    await vi.advanceTimersByTimeAsync(0);
    expect(later.app.notices.value.map((n) => n.text)).toEqual(["Seasons 1–2 of “Severance” are ready to watch."]);
  });

  it("is caught up with when unseen rises with no event to explain it, and not when one did", async () => {
    const { app, server, asked, hear } = setup();
    await app.loadRequests();
    const lists = () => asked.filter((a) => a.startsWith("requests ")).length;
    expect(lists()).toBe(0);
    // An event explained it.
    hear({ type: "request_available", request_id: 1, title: "Title 1", item_id: 51, complete: true });
    server.summary = () => Promise.resolve(summary({ unseen: 1 }));
    await vi.advanceTimersByTimeAsync(1_000);
    expect(lists()).toBe(1); // the first news of this launch is caught up with once
    expect(app.notices.value.length).toBe(1);
    server.summary = () => Promise.resolve(summary({ unseen: 2 }));
    server.unseen = () => Promise.resolve([request(1), request(2)]);
    await app.loadRequests();
    await vi.advanceTimersByTimeAsync(0);
    expect(lists()).toBe(2);
    expect(app.notices.value.map((n) => n.text)).toEqual(["“Title 1” is ready to watch.", "“Title 2” is ready to watch."]);
  });

  it("goes with the profile: another one starts afresh", async () => {
    const { app, hear } = setup();
    await app.loadRequests();
    hear({ type: "request_available", request_id: 1, title: "Title 1", item_id: 51, complete: true });
    expect(app.notices.value.length).toBe(1);
    await app.switched();
    expect(app.notices.value).toEqual([]);
    expect(app.marks.value).toEqual({});
  });
});
