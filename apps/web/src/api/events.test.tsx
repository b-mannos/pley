import type { ReactNode } from "react";
import { QueryClientProvider, type QueryClient } from "@tanstack/react-query";
import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { testQueryClient } from "@/test/render";
import { onServerEvent, useServerEvents } from "./events";
import { keys } from "./hooks";
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
