import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { Library } from "@/api/types";
import { Sidebar } from "@/components/Sidebar";
import { apiError, mockApi, renderWithProviders, type FakeRequest } from "@/test/render";
import { LibrariesSettings } from "./LibrariesSettings";

function library(id: number, name: string, kind: Library["kind"]): Library {
  return { id, name, kind, paths: [`/media/${name.toLowerCase()}`], item_count: 0, created_at: 0, scanned_at: null, scanning: false };
}

const libraries = [library(1, "Movies", "movies"), library(2, "TV", "shows"), library(3, "Music", "music")];
const names = libraries.map((l) => l.name);

/**
 * The libraries; a new order is saved as asked unless `order` answers
 * otherwise, and `refetch` answers the libraries after the first time (both
 * may hold the answer back).
 */
function serve({
  list = libraries,
  order,
  refetch,
}: { list?: Library[]; order?: (req: FakeRequest) => unknown; refetch?: () => unknown } = {}) {
  let current = list;
  let fetched = 0;
  return mockApi({
    "GET /api/libraries": () => (fetched++ > 0 && refetch ? refetch() : current),
    "GET /api/activity": () => ({ scans: [], sessions: [] }),
    "GET /api/playlists": () => ({ playlists: [] }),
    "PUT /api/libraries/order": (req) => {
      if (order) return order(req);
      const { ids } = req.body as { ids: number[] };
      current = ids.flatMap((id) => list.filter((l) => l.id === id));
      return current;
    },
  });
}

/** An answer to give later: `answer` sends it. */
function heldBack() {
  let answer: (value: unknown) => void = () => {};
  const reply = new Promise((resolve) => {
    answer = resolve;
  });
  return { reply: () => reply, answer: (value: unknown) => answer(value) };
}

/** The libraries a part of the page lists, in order. */
function namesIn(element: HTMLElement): string[] {
  return within(element)
    .getAllByRole("link")
    .map((link) => link.textContent ?? "")
    .filter((name) => names.includes(name));
}

/** The row of a library in the settings list. */
function rowOf(name: string): HTMLElement {
  const row = within(screen.getByRole("list", { name: "Libraries" }))
    .getByRole("link", { name })
    .closest<HTMLElement>("[data-library]");
  if (!row) throw new Error(`no row for ${name}`);
  return row;
}

describe("LibrariesSettings", () => {
  it("moves a library with its buttons and keeps the focus on it", async () => {
    const user = userEvent.setup();
    const requests = serve();
    renderWithProviders(<LibrariesSettings />);
    const list = await screen.findByRole("list", { name: "Libraries" });
    expect(namesIn(list)).toEqual(["Movies", "TV", "Music"]);
    expect(screen.getByRole("button", { name: "Move Movies up" })).toBeDisabled();

    screen.getByRole("button", { name: "Move Movies down" }).focus();
    await user.keyboard("{Enter}");
    await waitFor(() => expect(namesIn(list)).toEqual(["TV", "Movies", "Music"]));
    expect(screen.getByRole("button", { name: "Move Movies down" })).toHaveFocus();
    expect(screen.getByRole("status")).toHaveTextContent("Movies moved to position 2 of 3");
    await waitFor(() => expect(requests.filter((r) => r.method === "PUT")).toHaveLength(1));
    expect(requests.find((r) => r.method === "PUT")?.body).toEqual({ ids: [2, 1, 3] });

    // Last in the list, its "down" button stops working: the focus goes up.
    await user.keyboard("{Enter}");
    await waitFor(() => expect(namesIn(list)).toEqual(["TV", "Music", "Movies"]));
    expect(screen.getByRole("button", { name: "Move Movies down" })).toBeDisabled();
    await waitFor(() => expect(screen.getByRole("button", { name: "Move Movies up" })).toHaveFocus());
    expect(screen.getByRole("status")).toHaveTextContent("Movies moved to position 3 of 3");
    await waitFor(() => expect(requests.filter((r) => r.method === "PUT")).toHaveLength(2));
    expect(requests.filter((r) => r.method === "PUT")[1]?.body).toEqual({ ids: [2, 3, 1] });
  });

  it("moves a library where it is dropped", async () => {
    const requests = serve();
    renderWithProviders(<LibrariesSettings />);
    const list = await screen.findByRole("list", { name: "Libraries" });

    const dataTransfer = { effectAllowed: "", dropEffect: "", setData: vi.fn() };
    fireEvent.dragStart(rowOf("Music"), { dataTransfer });
    fireEvent.dragOver(rowOf("Movies"), { dataTransfer });
    fireEvent.drop(rowOf("Movies"), { dataTransfer });

    await waitFor(() => expect(namesIn(list)).toEqual(["Music", "Movies", "TV"]));
    await waitFor(() => expect(requests.find((r) => r.method === "PUT")?.body).toEqual({ ids: [3, 1, 2] }));
  });

  it("shows the new order in the sidebar before the server answers", async () => {
    const user = userEvent.setup();
    const saved = heldBack();
    serve({ order: saved.reply });
    renderWithProviders(
      <>
        <Sidebar open={false} onClose={() => {}} />
        <LibrariesSettings />
      </>,
    );
    const sidebar = await screen.findByRole("navigation", { name: "Main" });
    await waitFor(() => expect(namesIn(sidebar)).toEqual(["Movies", "TV", "Music"]));

    await user.click(await screen.findByRole("button", { name: "Move Music up" }));
    await waitFor(() => expect(namesIn(sidebar)).toEqual(["Movies", "Music", "TV"]));
    saved.answer([libraries[0], libraries[2], libraries[1]]);
    await waitFor(() => expect(screen.getByRole("button", { name: "Move TV down" })).toBeDisabled());
    expect(namesIn(sidebar)).toEqual(["Movies", "Music", "TV"]);
  });

  it("puts the old order back when the server refuses the new one", async () => {
    const user = userEvent.setup();
    const saved = heldBack();
    const refetched = heldBack();
    const requests = serve({ order: saved.reply, refetch: refetched.reply });
    renderWithProviders(<LibrariesSettings />);
    const list = await screen.findByRole("list", { name: "Libraries" });

    await user.click(screen.getByRole("button", { name: "Move Music up" }));
    await waitFor(() => expect(namesIn(list)).toEqual(["Movies", "Music", "TV"]));
    saved.answer(apiError(409, "conflict", "the libraries have changed: the order must list each of them once"));

    // Back at once, before the libraries are fetched again (in case some
    // came or went meanwhile).
    expect(await screen.findByText("The libraries have changed: the order must list each of them once")).toBeInTheDocument();
    await waitFor(() => expect(namesIn(list)).toEqual(["Movies", "TV", "Music"]));
    expect(screen.getByRole("button", { name: "Move Music up" })).toHaveFocus();
    await waitFor(() => expect(requests.filter((r) => r.path === "/api/libraries")).toHaveLength(2));
    refetched.answer(libraries);
  });

  it("has nothing to move with a single library", async () => {
    serve({ list: [libraries[0] as Library] });
    renderWithProviders(<LibrariesSettings />);
    await screen.findByRole("list", { name: "Libraries" });

    expect(screen.queryByRole("button", { name: /^Move / })).not.toBeInTheDocument();
    expect(rowOf("Movies")).not.toHaveAttribute("draggable");
    expect(screen.queryByText("Everyone sees them in this order.", { exact: false })).not.toBeInTheDocument();
  });
});
