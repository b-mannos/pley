import type { ReactNode } from "react";
import { QueryClientProvider } from "@tanstack/react-query";
import { act, renderHook, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { makeItem } from "@/test/fixtures";
import { mockApi, testQueryClient } from "@/test/render";
import { FIRST_PAGE, useLibraryItems } from "./hooks";
import type { SortOrder } from "./types";

const items = Array.from({ length: FIRST_PAGE + 2 }, (_, i) => makeItem({ id: i + 1 }));
const ascending = items.map((i) => i.id);
const descending = [...ascending].reverse();

/** Serves the library in pages, in either order; `delay` holds back the second half. */
function serveLibrary(list = items) {
  return mockApi({
    "GET /api/libraries/1/items": (req) => {
      const params = new URL(req.path, "http://pley.test").searchParams;
      const ordered = params.get("order") === "desc" ? [...list].reverse() : list;
      const offset = Number(params.get("offset") ?? 0);
      const limit = Number(params.get("limit"));
      return { total: ordered.length, items: ordered.slice(offset, offset + limit) };
    },
  });
}

function wrapper({ children }: { children: ReactNode }) {
  return <QueryClientProvider client={testQueryClient()}>{children}</QueryClientProvider>;
}

describe("useLibraryItems", () => {
  it("loads the first page on its own, then the rest", async () => {
    const requests = serveLibrary();
    const { result } = renderHook(() => useLibraryItems(1, { sort: "title", order: "asc" }), { wrapper });

    await waitFor(() => expect(result.current.data?.items).toHaveLength(items.length));
    expect(result.current.data?.items.map((i) => i.id)).toEqual(ascending);
    expect(requests.map((r) => r.path)).toEqual([
      `/api/libraries/1/items?sort=title&order=asc&limit=${FIRST_PAGE}`,
      `/api/libraries/1/items?sort=title&order=asc&offset=${FIRST_PAGE}&limit=100000`,
    ]);
  });

  it("makes one request for small libraries", async () => {
    const requests = serveLibrary(items.slice(0, 10));
    const { result } = renderHook(() => useLibraryItems(1, {}), { wrapper });

    await waitFor(() => expect(result.current.data?.items).toHaveLength(10));
    expect(requests).toHaveLength(1);
    expect(result.current.isPending).toBe(false);
  });

  it("never mixes the new order with the old one while re-sorting", async () => {
    serveLibrary();
    const seen: number[][] = [];
    const { result, rerender } = renderHook(
      ({ order }: { order: SortOrder }) => {
        const state = useLibraryItems(1, { sort: "title", order });
        if (state.data) seen.push(state.data.items.map((i) => i.id));
        return state;
      },
      { wrapper, initialProps: { order: "asc" } },
    );
    await waitFor(() => expect(result.current.data?.items).toHaveLength(items.length));

    rerender({ order: "desc" });
    await waitFor(() => expect(result.current.data?.items.map((i) => i.id)).toEqual(descending));

    // Every list shown is the start of one order or the other.
    for (const ids of seen) {
      const start = (order: number[]) => order.slice(0, ids.length);
      expect([start(ascending), start(descending)]).toContainEqual(ids);
    }
  });

  it("refetches both halves", async () => {
    const requests = serveLibrary();
    const { result } = renderHook(() => useLibraryItems(1, {}), { wrapper });
    await waitFor(() => expect(result.current.data?.items).toHaveLength(items.length));

    await act(() => result.current.refetch());
    expect(requests).toHaveLength(4);
  });
});
