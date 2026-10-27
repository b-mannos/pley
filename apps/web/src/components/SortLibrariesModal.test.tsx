import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import type { Library } from "@/api/types";
import { apiError, mockApi, renderWithProviders } from "@/test/render";
import { Sidebar } from "./Sidebar";

function library(id: number, name: string, kind: Library["kind"]): Library {
  return { id, name, kind, paths: [], item_count: 0, created_at: 0, scanned_at: null, scanning: false };
}

const libraries = [library(1, "Movies", "movies"), library(2, "TV", "shows"), library(3, "Music", "music")];

/** The libraries in the user's order, which they may sort, or go back to the shared one. */
function serve(list = libraries, { refuse = false } = {}) {
  let current = list;
  return mockApi({
    "GET /api/libraries": () => current,
    "GET /api/playlists": () => ({ playlists: [] }),
    "PUT /api/me/libraries/order": (req) => {
      if (refuse) return apiError(409, "conflict", "your libraries have changed: the order must list each of them once");
      const { ids } = req.body as { ids: number[] };
      current = ids.flatMap((id) => list.filter((l) => l.id === id));
      return current;
    },
    "DELETE /api/me/libraries/order": () => {
      current = list;
      return current;
    },
  });
}

/** The libraries the sidebar lists, in order. */
function sidebarNames(): string[] {
  const names = libraries.map((l) => l.name);
  return within(screen.getByRole("navigation", { name: "Main" }))
    .getAllByRole("link")
    .map((link) => link.textContent ?? "")
    .filter((name) => names.includes(name));
}

/** The libraries the dialog lists, in order. */
function dialogNames(): string[] {
  return within(screen.getByRole("list", { name: "Your libraries" }))
    .getAllByRole("listitem")
    .map((row) => row.textContent ?? "");
}

describe("SortLibrariesModal", () => {
  it("sorts the user's libraries one move at a time, the sidebar too", async () => {
    const user = userEvent.setup();
    const requests = serve();
    renderWithProviders(<Sidebar open={false} onClose={() => {}} />);
    await user.click(await screen.findByRole("button", { name: "Sort libraries" }));
    const dialog = await screen.findByRole("dialog", { name: "Sort libraries" });
    expect(dialogNames()).toEqual(["Movies", "TV", "Music"]);
    expect(within(dialog).getByRole("button", { name: "Move Movies up" })).toBeDisabled();

    await user.click(within(dialog).getByRole("button", { name: "Move Music up" }));
    await waitFor(() => expect(dialogNames()).toEqual(["Movies", "Music", "TV"]));
    expect(sidebarNames()).toEqual(["Movies", "Music", "TV"]);
    expect(within(dialog).getByRole("status")).toHaveTextContent("Music moved to position 2 of 3");
    await waitFor(() => expect(requests.filter((r) => r.method === "PUT")).toHaveLength(1));
    expect(requests.find((r) => r.method === "PUT")).toMatchObject({ path: "/api/me/libraries/order", body: { ids: [1, 3, 2] } });
    expect(within(dialog).getByRole("button", { name: "Move Music up" })).toHaveFocus();

    await user.click(within(dialog).getByRole("button", { name: "Use the server's order" }));
    await waitFor(() => expect(requests.filter((r) => r.method === "DELETE")).toHaveLength(1));
    await waitFor(() => expect(dialogNames()).toEqual(["Movies", "TV", "Music"]));
    expect(sidebarNames()).toEqual(["Movies", "TV", "Music"]);

    await user.click(within(dialog).getByRole("button", { name: "Done" }));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("puts a library back when the move is refused, and says so", async () => {
    const user = userEvent.setup();
    serve(libraries, { refuse: true });
    renderWithProviders(<Sidebar open={false} onClose={() => {}} />);
    await user.click(await screen.findByRole("button", { name: "Sort libraries" }));
    const dialog = await screen.findByRole("dialog", { name: "Sort libraries" });

    await user.click(within(dialog).getByRole("button", { name: "Move TV down" }));
    expect(await screen.findByText("Your libraries have changed: the order must list each of them once")).toBeInTheDocument();
    await waitFor(() => expect(dialogNames()).toEqual(["Movies", "TV", "Music"]));
    expect(within(dialog).getByRole("status")).toHaveTextContent("TV moved back to position 2 of 3");
  });

  it("offers no sorting with a single library", async () => {
    serve([libraries[0] as Library]);
    renderWithProviders(<Sidebar open={false} onClose={() => {}} />);
    await waitFor(() => expect(sidebarNames()).toEqual(["Movies"]));
    expect(screen.queryByRole("button", { name: "Sort libraries" })).not.toBeInTheDocument();
  });
});
