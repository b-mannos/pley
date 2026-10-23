import type { ReactNode } from "react";
import { act, cleanup, renderHook, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { ItemSummary } from "@/api/types";
import { makeItem } from "@/test/fixtures";
import { apiError, LocationDisplay, mockApi, TestProviders, testQueryClient } from "@/test/render";
import { playUrl, usePlayItem, type PlayOptions } from "./usePlayItem";

describe("playUrl", () => {
  it("adds only the options given", () => {
    expect(playUrl(5)).toBe("/play/5");
    expect(playUrl(5, { startMs: 0, sub: null })).toBe("/play/5");
    expect(playUrl(5, { startMs: 1234.6, fileId: 2, audio: 0, sub: 3 })).toBe("/play/5?t=1234&file=2&audio=0&sub=3");
  });
});

function wrapper({ children }: { children: ReactNode }) {
  return (
    <TestProviders client={testQueryClient()}>
      {children}
      <LocationDisplay />
    </TestProviders>
  );
}

/** Plays `item` from a fresh page; returns where that went. */
async function play(item: ItemSummary, opts?: PlayOptions) {
  cleanup();
  const { result } = renderHook(() => usePlayItem(), { wrapper });
  await act(() => result.current(item, opts));
  return screen.getByTestId("location").textContent;
}

const resumable = { position_ms: 90_000, watched: false, play_count: 0 };

describe("usePlayItem", () => {
  it("resumes movies and episodes", async () => {
    expect(await play(makeItem({ id: 3, user_state: resumable }))).toBe("/play/3?t=90000");
  });

  it("starts over when asked or once watched", async () => {
    expect(await play(makeItem({ id: 3, user_state: resumable }), { fromStart: true })).toBe("/play/3");
    expect(await play(makeItem({ id: 3, kind: "episode", user_state: { ...resumable, watched: true } }))).toBe("/play/3");
  });

  it("passes the chosen file and tracks", async () => {
    expect(await play(makeItem({ id: 3 }), { fileId: 8, audio: 1, sub: 2 })).toBe("/play/3?file=8&audio=1&sub=2");
  });

  it("plays extras", async () => {
    expect(await play(makeItem({ id: 41, kind: "extra", extra_type: "trailer" }))).toBe("/play/41");
  });

  it("plays the next episode of a show", async () => {
    mockApi({ "GET /api/items/1/on-deck": () => ({ item: makeItem({ id: 12, kind: "episode", user_state: resumable }) }) });
    expect(await play(makeItem({ id: 1, kind: "show" }))).toBe("/play/12?t=90000");
  });

  it("says when a show has nothing to play", async () => {
    mockApi({ "GET /api/items/1/on-deck": () => ({ item: null }) });
    expect(await play(makeItem({ id: 1, kind: "season" }))).toBe("/");
    expect(screen.getByRole("status")).toHaveTextContent("Nothing to play yet");
  });

  it("says when an album is empty", async () => {
    mockApi({ "GET /api/items/9/children": () => ({ items: [] }) });
    await play(makeItem({ id: 9, kind: "album" }));
    expect(screen.getByRole("status")).toHaveTextContent("This album has no tracks");
  });

  it("explains failures", async () => {
    mockApi({ "GET /api/items/1/on-deck": () => apiError(500, "internal", "database is locked") });
    await play(makeItem({ id: 1, kind: "show" }));
    expect(screen.getByRole("alert")).toHaveTextContent("Database is locked");
  });
});
