import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import type { DiscoverPage as Page } from "@/api/requests";
import { LocationDisplay, mockApi, renderWithProviders, testUser } from "@/test/render";
import { makeSummary, makeTitle } from "@/test/requests";
import { DiscoverPage } from "./DiscoverPage";

const ann = { ...testUser, id: 2, username: "ann", name: "Ann", is_admin: false };

function page(results = [makeTitle()], n = 1, total = 1): Page {
  return { page: n, total_pages: total, results };
}

/** Every list answers with one title of its own. */
function lists() {
  return {
    "GET /api/discover/trending": () => page([makeTitle({ tmdb_id: 1, title: "Trending One" })]),
    "GET /api/discover/movies": () => page([makeTitle({ tmdb_id: 2, title: "Popular Movie", status: "library", item_id: 7 })]),
    "GET /api/discover/shows": () => page([makeTitle({ tmdb_id: 3, kind: "show", title: "Popular Show" })]),
    "GET /api/discover/upcoming": () => page([]),
    "GET /api/discover/anime": () => page([makeTitle({ tmdb_id: 5, kind: "show", title: "Some Anime", status: "requested" })]),
  };
}

describe("DiscoverPage", () => {
  it("shows TMDB's lists as rows, each with a way to all of it", async () => {
    mockApi({ "GET /api/requests/summary": () => makeSummary({ movies_left: 3 }), ...lists() });
    renderWithProviders(<DiscoverPage />, { route: "/discover", user: ann });

    expect((await screen.findAllByRole("link", { name: "Trending One" }))[0]).toHaveAttribute("href", "/discover/movie/1");
    expect((await screen.findAllByRole("link", { name: "Popular Show" }))[0]).toHaveAttribute("href", "/discover/show/3");
    expect(screen.getByText(/An administrator decides on what you ask for\. You can ask for 3 more movies this week\./)).toBeInTheDocument();
    // A list without titles leaves no row.
    expect(screen.queryByRole("heading", { name: "Coming to cinemas" })).not.toBeInTheDocument();
    const anime = (await screen.findByRole("heading", { name: "Popular anime" })).closest("section") as HTMLElement;
    expect(within(anime).getByText("Requested")).toBeInTheDocument();
    expect(within(anime).getByRole("link", { name: "See all" })).toHaveAttribute("href", "/discover?list=anime");
  });

  it("shows one list whole, a page at a time", async () => {
    const user = userEvent.setup();
    const requests = mockApi({
      "GET /api/requests/summary": () => makeSummary(),
      "GET /api/discover/movies": (req) =>
        req.path.includes("page=2")
          ? page([makeTitle({ tmdb_id: 12, title: "Second Page" })], 2, 2)
          : page([makeTitle({ tmdb_id: 11, title: "First Page" })], 1, 2),
    });
    renderWithProviders(<DiscoverPage />, { route: "/discover?list=movies" });

    expect(await screen.findByRole("heading", { name: "Popular movies" })).toBeInTheDocument();
    expect((await screen.findAllByRole("link", { name: "First Page" }))[0]).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Show more" }));
    expect((await screen.findAllByRole("link", { name: "Second Page" }))[0]).toBeInTheDocument();
    expect(requests.map((r) => r.path)).toContain("/api/discover/movies?page=2");
    expect(screen.queryByRole("button", { name: "Show more" })).not.toBeInTheDocument();
  });

  it("searches TMDB as one types, keeping the query in the address", async () => {
    const user = userEvent.setup();
    const requests = mockApi({
      "GET /api/requests/summary": () => makeSummary(),
      ...lists(),
      "GET /api/discover/search": (req) => (req.path.includes("q=dune") ? page([makeTitle({ tmdb_id: 438631, title: "Dune", year: 2021 })]) : page([])),
    });
    renderWithProviders(
      <>
        <DiscoverPage />
        <LocationDisplay />
      </>,
      { route: "/discover" },
    );

    await user.type(await screen.findByRole("searchbox", { name: "Search for movies and shows to ask for" }), "dune");
    expect(await screen.findByRole("heading", { name: "Results for “dune”" })).toBeInTheDocument();
    expect(screen.getAllByRole("link", { name: "Dune" })[0]).toHaveAttribute("href", "/discover/movie/438631");
    expect(screen.getByTestId("location")).toHaveTextContent("/discover?q=dune");
    expect(requests.filter((r) => r.path.startsWith("/api/discover/search"))).toHaveLength(1);

    await user.clear(screen.getByRole("searchbox"));
    await waitFor(() => expect(screen.getByTestId("location")).toHaveTextContent(/^\/discover$/));
    expect(await screen.findByRole("heading", { name: "Trending this week" })).toBeInTheDocument();
  });

  it("says when TMDB finds nothing", async () => {
    mockApi({ "GET /api/requests/summary": () => makeSummary(), "GET /api/discover/search": () => page([]) });
    renderWithProviders(<DiscoverPage />, { route: "/discover?q=zzz" });
    expect(await screen.findByText("Nothing on TMDB for “zzz”")).toBeInTheDocument();
    expect(screen.getByRole("searchbox")).toHaveValue("zzz");
  });

  it("sends administrators to set requests up while they're off", async () => {
    const requests = mockApi({ "GET /api/requests/summary": () => makeSummary({ enabled: false }) });
    renderWithProviders(<DiscoverPage />, { route: "/discover" });
    expect(await screen.findByRole("heading", { name: "Requests are off" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Set up requests" })).toHaveAttribute("href", "/settings/requests");
    expect(requests.some((r) => r.path.startsWith("/api/discover"))).toBe(false);
  });

  it("says requests are off to others, and on servers without them", async () => {
    mockApi({});
    renderWithProviders(<DiscoverPage />, { route: "/discover", user: ann });
    expect(await screen.findByRole("heading", { name: "Requests are off" })).toBeInTheDocument();
    expect(screen.getByText("This server doesn't take requests for movies and shows.")).toBeInTheDocument();
    expect(screen.queryByRole("link", { name: "Set up requests" })).not.toBeInTheDocument();
  });

  it("says when this account may not ask", async () => {
    mockApi({ "GET /api/requests/summary": () => makeSummary({ access: "off" }) });
    renderWithProviders(<DiscoverPage />, { route: "/discover", user: ann });
    expect(await screen.findByRole("heading", { name: "Requests aren't open to this account" })).toBeInTheDocument();
  });
});
