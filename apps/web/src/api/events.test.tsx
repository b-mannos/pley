import type { ReactNode } from "react";
import { QueryClientProvider, type QueryClient } from "@tanstack/react-query";
import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { connectionLost } from "@/lib/connection";
import { testQueryClient } from "@/test/render";
import { LOST_AFTER_MS, onServerEvent, useServerEvents } from "./events";
import { keys } from "./hooks";
import { requestKeys } from "./requests";
import type { Activity, Library, ServerEvent } from "./types";

/** Stands in for the browser's EventSource; tests drive it. */
class FakeEventSource {
  static readonly CONNECTING = 0;
  static readonly OPEN = 1;
  static readonly CLOSED = 2;
  static all: FakeEventSource[] = [];

  readyState = FakeEventSource.CONNECTING;
  onopen: (() => void) | null = null;
  onmessage: ((msg: { data: string }) => void) | null = null;
  onerror: (() => void) | null = null;

  readonly url: string;

  constructor(url: string) {
    this.url = url;
    FakeEventSource.all.push(this);
  }

  close() {
    this.readyState = FakeEventSource.CLOSED;
  }

  open() {
    this.readyState = FakeEventSource.OPEN;
    this.onopen?.();
  }

  send(event: ServerEvent | string) {
    this.onmessage?.({ data: typeof event === "string" ? event : JSON.stringify(event) });
  }

  /** The server went away: the browser gives up on this connection. */
  fail() {
    this.readyState = FakeEventSource.CLOSED;
    this.onerror?.();
  }
}

const library = (id: number, scanning = false): Library => ({
  id,
  name: `Library ${id}`,
  kind: "movies",
  paths: [],
  item_count: 0,
  created_at: 0,
  scanned_at: null,
  scanning,
});

function setup(enabled = true) {
  const client = testQueryClient();
  const wrapper = ({ children }: { children: ReactNode }) => <QueryClientProvider client={client}>{children}</QueryClientProvider>;
  const hook = renderHook(({ on }) => useServerEvents(on), { wrapper, initialProps: { on: enabled } });
  return { client, ...hook, source: () => FakeEventSource.all.at(-1) as FakeEventSource };
}

function invalidated(client: QueryClient, key: readonly unknown[]): boolean {
  return client.getQueryState(key)?.isInvalidated ?? false;
}

beforeEach(() => {
  FakeEventSource.all = [];
  vi.stubGlobal("EventSource", FakeEventSource);
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
});

describe("useServerEvents", () => {
  it("connects only while signed in, and closes when done", () => {
    const { rerender, unmount, source } = setup(false);
    expect(FakeEventSource.all).toHaveLength(0);

    rerender({ on: true });
    expect(source().url).toBe("/api/events");
    unmount();
    expect(source().readyState).toBe(FakeEventSource.CLOSED);
  });

  it("follows scans", () => {
    const { client, source } = setup();
    client.setQueryData(keys.libraries, [library(1), library(2)]);
    client.setQueryData<Activity>(keys.activity, { scans: [], sessions: [] });

    act(() => source().send({ type: "scan_started", library_id: 1 }));
    expect(client.getQueryData<Library[]>(keys.libraries)?.map((l) => l.scanning)).toEqual([true, false]);
    expect(invalidated(client, keys.activity)).toBe(true);

    act(() => source().send({ type: "scan_progress", library_id: 1, phase: "probing", processed: 3, total: 10 }));
    act(() => source().send({ type: "scan_progress", library_id: 1, phase: "metadata", processed: 7, total: 10 }));
    expect(client.getQueryData<Activity>(keys.activity)?.scans).toEqual([
      expect.objectContaining({ library_id: 1, library_name: "Library 1", phase: "metadata", processed: 7, total: 10 }),
    ]);

    act(() => source().send({ type: "scan_finished", library_id: 1, added: 2, updated: 0, removed: 0 }));
    expect(client.getQueryData<Library[]>(keys.libraries)?.[0]?.scanning).toBe(false);
  });

  it("refreshes a changed library once per burst", () => {
    const { client, source } = setup();
    client.setQueryData(keys.library(1), library(1));
    client.setQueryData(keys.homeHubs, { hubs: [] });

    act(() => {
      for (let i = 0; i < 20; i++) source().send({ type: "library_changed", library_id: 1 });
    });
    expect(invalidated(client, keys.library(1))).toBe(false);

    act(() => {
      vi.advanceTimersByTime(1500);
    });
    expect(invalidated(client, keys.library(1))).toBe(true);
    expect(invalidated(client, keys.homeHubs)).toBe(true);
  });

  it("refreshes everything after a reconnect, not on the first connection", () => {
    const { client, source } = setup();
    client.setQueryData(keys.item(5), { id: 5 });

    act(() => source().open());
    expect(invalidated(client, keys.item(5))).toBe(false);

    act(() => source().open());
    expect(invalidated(client, keys.item(5))).toBe(true);
  });

  it("says the server is out of reach when it stays away, and stops once it is back", async () => {
    const health = vi.fn(() => Promise.reject(new TypeError("Failed to fetch")));
    vi.stubGlobal("fetch", health);
    const { source } = setup();
    act(() => source().open());

    act(() => source().fail());
    act(() => {
      vi.advanceTimersByTime(LOST_AFTER_MS - 1);
    });
    expect(connectionLost()).toBe(false);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1);
    });
    await vi.waitFor(() => expect(connectionLost()).toBe(true));
    expect(health).toHaveBeenCalledWith("/api/health", expect.anything());

    act(() => {
      vi.advanceTimersByTime(30_000);
    });
    act(() => source().open());
    expect(connectionLost()).toBe(false);
  });

  it("doesn't say so for a stream the server refuses while it answers", async () => {
    vi.stubGlobal("fetch", vi.fn(() => Promise.resolve(new Response("ok"))));
    const { source } = setup();

    act(() => source().fail());
    await act(async () => {
      await vi.advanceTimersByTimeAsync(LOST_AFTER_MS);
    });
    expect(connectionLost()).toBe(false);
  });

  it("reconnects after the server goes away, waiting longer each time", () => {
    const { source } = setup();

    act(() => source().fail());
    act(() => {
      vi.advanceTimersByTime(1_999);
    });
    expect(FakeEventSource.all).toHaveLength(1);
    act(() => {
      vi.advanceTimersByTime(1);
    });
    expect(FakeEventSource.all).toHaveLength(2);

    act(() => source().fail());
    act(() => {
      vi.advanceTimersByTime(3_999);
    });
    expect(FakeEventSource.all).toHaveLength(2);
    act(() => {
      vi.advanceTimersByTime(1);
    });
    expect(FakeEventSource.all).toHaveLength(3);
  });

  it("refreshes My List when it changed on another device", () => {
    const { client, source } = setup();
    client.setQueryData(keys.watchlist, { items: [] });
    client.setQueryData(keys.homeHubs, { hubs: [] });
    client.setQueryData(keys.item(5), { id: 5 });

    act(() => source().send({ type: "watchlist_changed" }));
    expect(invalidated(client, keys.watchlist)).toBe(true);
    expect(invalidated(client, keys.homeHubs)).toBe(true);
    expect(invalidated(client, keys.item(5))).toBe(true);
  });

  it("refreshes Home and the items when watch state changed, after a moment", () => {
    const { client, source } = setup();
    client.setQueryData(keys.homeHubs, { hubs: [] });
    client.setQueryData(keys.libraryHubs(1), { hubs: [] });
    client.setQueryData(keys.watchlist, { items: [] });
    client.setQueryData(keys.item(5), { id: 5 });
    client.setQueryData(keys.children(4), { items: [] });
    client.setQueryData(keys.item(9), { id: 9 });

    act(() => source().send({ type: "user_state_changed", item_ids: [5, 4, 3], children: false }));
    act(() => {
      vi.advanceTimersByTime(299);
    });
    expect(invalidated(client, keys.homeHubs)).toBe(false);

    act(() => {
      vi.advanceTimersByTime(1);
    });
    expect(invalidated(client, keys.homeHubs)).toBe(true);
    expect(invalidated(client, keys.libraryHubs(1))).toBe(true);
    expect(invalidated(client, keys.watchlist)).toBe(true);
    expect(invalidated(client, keys.item(5))).toBe(true);
    // The season's episodes.
    expect(invalidated(client, keys.children(4))).toBe(true);
    expect(invalidated(client, keys.item(9))).toBe(false);
  });

  it("refreshes every item when what is under one changed", () => {
    const { client, source } = setup();
    client.setQueryData(keys.item(9), { id: 9 });

    act(() => source().send({ type: "user_state_changed", item_ids: [3], children: true }));
    act(() => {
      vi.advanceTimersByTime(300);
    });
    expect(invalidated(client, keys.item(9))).toBe(true);
  });

  it("leaves alone what was fetched since the change was heard", () => {
    const { client, source } = setup();
    client.setQueryData(keys.homeHubs, { hubs: [] });
    client.setQueryData(keys.item(5), { id: 5 });

    act(() => source().send({ type: "user_state_changed", item_ids: [5], children: false }));
    act(() => {
      vi.advanceTimersByTime(100);
    });
    // This page made the change and fetched Home again already.
    client.setQueryData(keys.homeHubs, { hubs: [] });
    act(() => {
      vi.advanceTimersByTime(200);
    });
    expect(invalidated(client, keys.homeHubs)).toBe(false);
    expect(invalidated(client, keys.item(5))).toBe(true);
  });

  it("refreshes a library change heard with a watch state change once, at once", () => {
    const { client, source } = setup();
    client.setQueryData(keys.library(1), library(1));
    client.setQueryData(keys.homeHubs, { hubs: [] });

    // A play that ends: its watch state, and its library's change.
    act(() => {
      source().send({ type: "user_state_changed", item_ids: [5], children: false });
      source().send({ type: "library_changed", library_id: 1 });
    });
    act(() => {
      vi.advanceTimersByTime(300);
    });
    expect(invalidated(client, keys.library(1))).toBe(true);
    expect(invalidated(client, keys.homeHubs)).toBe(true);

    // Fetched again: the library's flush doesn't come a second time.
    client.setQueryData(keys.library(1), library(1));
    client.setQueryData(keys.homeHubs, { hubs: [] });
    act(() => {
      vi.advanceTimersByTime(1500);
    });
    expect(invalidated(client, keys.library(1))).toBe(false);
    expect(invalidated(client, keys.homeHubs)).toBe(false);
  });

  it("refreshes requests, Discover and the count waiting when a request changes", () => {
    const { client, source } = setup();
    client.setQueryData(requestKeys.summary, { pending: 0 });
    client.setQueryData(requestKeys.title("movie", 603), { tmdb_id: 603 });
    client.setQueryData(requestKeys.list({ status: null, mine: false }), { pages: [], pageParams: [] });
    client.setQueryData(keys.item(5), { id: 5 });

    act(() => source().send({ type: "requests_changed" }));
    expect(invalidated(client, requestKeys.summary)).toBe(true);
    expect(invalidated(client, requestKeys.title("movie", 603))).toBe(true);
    expect(invalidated(client, requestKeys.list({ status: null, mine: false }))).toBe(true);
    expect(invalidated(client, keys.item(5))).toBe(false);
  });

  it("refreshes requests after a reconnect", () => {
    const { client, source } = setup();
    act(() => source().open());
    client.setQueryData(requestKeys.summary, { pending: 0 });
    act(() => source().open());
    expect(invalidated(client, requestKeys.summary)).toBe(true);
  });

  it("passes events on to whoever listens", () => {
    const { source } = setup();
    const heard: ServerEvent[] = [];
    const stop = onServerEvent((ev) => heard.push(ev));
    const stopped: ServerEvent = { type: "session_stopped", session_id: "s1", message: "Bye", by_admin: true };

    act(() => source().send(stopped));
    stop();
    act(() => source().send({ type: "sessions_changed" }));
    expect(heard).toEqual([stopped]);
  });

  it("ignores events it can't read", () => {
    const { client, source } = setup();
    client.setQueryData(keys.libraries, [library(1)]);
    act(() => source().send("{not json"));
    act(() => source().send(JSON.stringify({ type: "something_new" })));
    expect(client.getQueryData<Library[]>(keys.libraries)).toEqual([library(1)]);
  });
});
