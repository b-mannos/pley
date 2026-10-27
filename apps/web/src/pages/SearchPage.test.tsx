import { screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { makeItem } from "@/test/fixtures";
import { apiError, mockApi, renderWithProviders } from "@/test/render";
import { SearchPage } from "./SearchPage";
import { personUrl } from "@/lib/urls";

function search(q: string) {
  return renderWithProviders(<SearchPage />, { route: `/search?q=${encodeURIComponent(q)}` });
}

describe("SearchPage", () => {
  it("groups the results by kind, people first", async () => {
    const requests = mockApi({
      "GET /api/search": () => ({
        items: [
          makeItem({ id: 1, kind: "episode", title: "The Heat Is On", parent_index: 2, index: 4 }),
          makeItem({ id: 2, title: "Heat", year: 1995 }),
          makeItem({ id: 3, title: "The Heat", year: 2013 }),
        ],
        people: [{ id: 9, name: "Heath Ledger" }],
      }),
    });
    search("heat ");

    expect(await screen.findByRole("heading", { name: "Results for “heat ”" })).toBeInTheDocument();
    const headings = screen.getAllByRole("heading", { level: 2 }).map((h) => h.textContent);
    expect(headings).toEqual(["People 1", "Movies 2", "Episodes 1"]);
    expect(screen.getByRole("link", { name: /Heath Ledger/ })).toHaveAttribute("href", personUrl(9));

    const movies = screen.getByRole("heading", { name: "Movies 2" }).closest("section") as HTMLElement;
    expect(within(movies).getByText("1995")).toBeInTheDocument();
    // The query is trimmed.
    expect(requests[0]?.path).toBe("/api/search?q=heat&limit=60");
  });

  it("invites a search without a query", () => {
    const requests = mockApi({});
    search("  ");
    expect(screen.getByText("Search your libraries")).toBeInTheDocument();
    expect(requests).toHaveLength(0);
  });

  it("says when nothing matched", async () => {
    mockApi({ "GET /api/search": () => ({ items: [] }) });
    search("zzz");
    expect(await screen.findByText("No results for “zzz”")).toBeInTheDocument();
  });

  it("explains failures", async () => {
    mockApi({ "GET /api/search": () => apiError(500, "internal", "index is rebuilding") });
    search("heat");
    expect(await screen.findByText("Index is rebuilding")).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Couldn't search your libraries" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Retry" })).toBeInTheDocument();
  });
});
