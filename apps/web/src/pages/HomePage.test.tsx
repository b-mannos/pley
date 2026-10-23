import { screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import type { Hub, Library } from "@/api/types";
import { makeItem } from "@/test/fixtures";
import { apiError, mockApi, renderWithProviders, testUser } from "@/test/render";
import { HomePage } from "./HomePage";
import { libraryUrl } from "@/lib/urls";

const movies: Library = {
  id: 1,
  name: "Movies",
  kind: "movies",
  paths: ["/media/movies"],
  item_count: 2,
  created_at: 0,
  scanned_at: 0,
  scanning: false,
};

const hubs: Hub[] = [
  {
    key: "continue_watching",
    title: "Continue Watching",
    library_id: null,
    items: [
      makeItem({ id: 6, title: "Alien", duration_ms: 2_640_000, user_state: { position_ms: 600_000, watched: false, play_count: 0 } }),
      makeItem({ id: 7, kind: "episode", title: "Pilot", grandparent_title: "Lost", parent_index: 1, index: 1 }),
    ],
  },
  { key: "recently_added_1", title: "Recently Added in Movies", library_id: 1, items: [makeItem({ id: 8, title: "Heat", year: 1995 })] },
  { key: "top_rated_1", title: "Top Rated", library_id: 1, items: [] },
];

function serve({ libraries = [movies], home = hubs }: { libraries?: Library[]; home?: Hub[] | Response } = {}) {
  return mockApi({
    "GET /api/libraries": () => libraries,
    "GET /api/hubs/home": () => (home instanceof Response ? home : { hubs: home }),
  });
}

describe("HomePage", () => {
  it("shows the hubs that have something in them", async () => {
    serve();
    renderWithProviders(<HomePage />);

    const continueRow = (await screen.findByRole("heading", { name: "Continue Watching" })).closest("section") as HTMLElement;
    // Movies say how much is left; episodes which one is next.
    expect(within(continueRow).getByText("34m left")).toBeInTheDocument();
    expect(within(continueRow).getByText("S1 · E1 · Pilot")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Recently Added in Movies" })).toHaveAttribute(
      "href",
      libraryUrl(1, "tab=library&sort=added&order=desc"),
    );
    expect(screen.queryByRole("heading", { name: "Top Rated" })).not.toBeInTheDocument();
  });

  it("asks an administrator to add a first library", async () => {
    serve({ libraries: [] });
    renderWithProviders(<HomePage />);

    expect(await screen.findByText("No libraries yet")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Add a library" })).toHaveAttribute("href", "/settings/libraries?add=1");
  });

  it("tells everyone else to ask the owner", async () => {
    serve({ libraries: [] });
    renderWithProviders(<HomePage />, { user: { ...testUser, is_admin: false } });

    expect(await screen.findByText("Ask the server owner to add a library.")).toBeInTheDocument();
    expect(screen.queryByRole("link", { name: "Add a library" })).not.toBeInTheDocument();
  });

  it("says when the libraries are still being scanned", async () => {
    serve({ libraries: [{ ...movies, scanning: true }], home: [] });
    renderWithProviders(<HomePage />);
    expect(await screen.findByText("Scanning your libraries…")).toBeInTheDocument();
  });

  it("retries after an error", async () => {
    const user = userEvent.setup();
    let fail = true;
    mockApi({
      "GET /api/libraries": () => [movies],
      "GET /api/hubs/home": () => (fail ? apiError(500, "internal", "database is locked") : { hubs }),
    });
    renderWithProviders(<HomePage />);

    expect(await screen.findByText("Database is locked")).toBeInTheDocument();
    fail = false;
    await user.click(screen.getByRole("button", { name: "Retry" }));
    expect(await screen.findByRole("heading", { name: "Continue Watching" })).toBeInTheDocument();
  });
});
