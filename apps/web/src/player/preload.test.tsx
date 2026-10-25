import { renderHook, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ItemKind } from "@/api/types";
import { MediaCard } from "@/components/cards";
import { makeItem } from "@/test/fixtures";
import { mockApi, renderWithProviders } from "@/test/render";
import { playIntent, usePreloadPlayerWhenIdle } from "./preload";

const { loadHls, page } = vi.hoisted(() => ({
  loadHls: vi.fn(() => Promise.resolve({})),
  page: { loaded: false },
}));
vi.mock("@/lib/hls", () => ({ loadHls }));
vi.mock("./PlayerPage", () => {
  page.loaded = true;
  return { PlayerPage: () => null };
});

const ANDROID = "Mozilla/5.0 (Linux; Android 15; wv) Chrome/140.0 Mobile Safari/537.36 pleyAndroid/0.1.0";

beforeEach(() => {
  loadHls.mockClear();
  mockApi({});
});

afterEach(() => {
  delete (window as unknown as { PleyAndroid?: unknown }).PleyAndroid;
});

describe("preloading the player", () => {
  // First in the file: the player's module stays loaded once it is.
  it("fetches the player and hls.js when the pointer reaches a card's Play", async () => {
    const user = userEvent.setup();
    renderWithProviders(<MediaCard item={makeItem({ id: 7, title: "Heat" })} width={200} />);
    expect(page.loaded).toBe(false);
    expect(loadHls).not.toHaveBeenCalled();

    await user.hover(screen.getByRole("button", { name: "Play Heat" }));

    expect(loadHls).toHaveBeenCalled();
    await vi.waitFor(() => expect(page.loaded).toBe(true));
  });

  it("fetches them when a Play button gets the focus", async () => {
    const user = userEvent.setup();
    renderWithProviders(<MediaCard item={makeItem({ id: 7, title: "Heat" })} width={200} />);
    await user.tab(); // the card's link
    expect(loadHls).not.toHaveBeenCalled();
    await user.tab();
    expect(screen.getByRole("button", { name: "Play Heat" })).toHaveFocus();
    expect(loadHls).toHaveBeenCalled();
  });

  it("leaves music alone", () => {
    expect(playIntent("album")).toEqual({});
    expect(playIntent("track")).toEqual({});
    for (const kind of ["movie", "episode", "extra", "show", "season"] satisfies ItemKind[]) {
      expect(playIntent(kind).onPointerEnter).toBeTypeOf("function");
      expect(playIntent(kind).onFocus).toBeTypeOf("function");
    }
  });

  it("doesn't fetch hls.js in the Android app, whose own player plays", () => {
    vi.stubGlobal("navigator", { ...navigator, userAgent: ANDROID });
    (window as unknown as { PleyAndroid: unknown }).PleyAndroid = { play: () => true };
    playIntent("movie").onFocus?.();
    expect(loadHls).not.toHaveBeenCalled();
  });

  it("fetches them once an item page of a video is idle", () => {
    const idle = vi.fn<(callback: () => void, options?: IdleRequestOptions) => number>(() => 1);
    vi.stubGlobal("requestIdleCallback", idle);
    vi.stubGlobal("cancelIdleCallback", vi.fn());

    const { rerender } = renderHook(({ kind }: { kind: ItemKind | undefined }) => usePreloadPlayerWhenIdle(kind), {
      initialProps: { kind: undefined as ItemKind | undefined },
    });
    expect(idle).not.toHaveBeenCalled(); // still loading: not known to be a video

    rerender({ kind: "movie" });
    expect(idle).toHaveBeenCalledTimes(1);
    expect(loadHls).not.toHaveBeenCalled();
    idle.mock.calls[0]?.[0]();
    expect(loadHls).toHaveBeenCalled();
  });

  it("doesn't on an album's page", () => {
    const idle = vi.fn(() => 1);
    vi.stubGlobal("requestIdleCallback", idle);
    renderHook(() => usePreloadPlayerWhenIdle("album"));
    expect(idle).not.toHaveBeenCalled();
  });

  it("waits a moment instead in browsers without idle callbacks", () => {
    vi.useFakeTimers();
    try {
      vi.stubGlobal("requestIdleCallback", undefined);
      renderHook(() => usePreloadPlayerWhenIdle("episode"));
      expect(loadHls).not.toHaveBeenCalled();
      vi.advanceTimersByTime(1_000);
      expect(loadHls).toHaveBeenCalled();
    } finally {
      vi.useRealTimers();
    }
  });

  it("doesn't once the page is gone", () => {
    const cancel = vi.fn();
    vi.stubGlobal("requestIdleCallback", vi.fn(() => 42));
    vi.stubGlobal("cancelIdleCallback", cancel);
    const { unmount } = renderHook(() => usePreloadPlayerWhenIdle("movie"));
    unmount();
    expect(cancel).toHaveBeenCalledWith(42);
  });
});
