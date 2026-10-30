import type { ReactElement } from "react";
import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import type { User } from "@/api/types";
import { LocationDisplay, mockApi, renderWithProviders, testUser } from "@/test/render";
import { makeRequest, makeRequestList, makeSummary } from "@/test/requests";
import { RequestsPage } from "./RequestsPage";

const ann: User = { ...testUser, id: 2, username: "ann", name: "Ann", is_admin: false };

const pending = makeRequest({ id: 1, title: "The Matrix" });
const approved = makeRequest({
  id: 2,
  kind: "show",
  tmdb_id: 1399,
  title: "Seasons Show",
  seasons: [1, 2, 3],
  available_seasons: [1],
  status: "approved",
  decided_by: { id: 1, name: "Admin", avatar_url: null, avatar_color: null },
  decided_at: 1_700_000_100,
  download: { progress: 0.25, state: "downloading", eta: null, episodes: 8 },
});
const declined = makeRequest({ id: 3, title: "Old Film", status: "declined", reason: "We have it on disc" });

function show(ui: ReactElement, route = "/requests", user: User = testUser) {
  return renderWithProviders(
    <>
      {ui}
      <LocationDisplay />
    </>,
    { route, user },
  );
}

describe("RequestsPage", () => {
  it("lists everyone's requests for administrators, with counts by status", async () => {
    const requests = mockApi({
      "GET /api/requests/summary": () => makeSummary({ access: "auto", pending: 1 }),
      "GET /api/requests": (req) =>
        req.path.includes("status=pending") ? { ...makeRequestList([pending]), counts: makeRequestList([pending, approved, declined]).counts } : makeRequestList([pending, approved, declined]),
    });
    const user = userEvent.setup();
    show(<RequestsPage />);

    const list = await screen.findByRole("list");
    const rows = within(list).getAllByRole("listitem");
    expect(rows).toHaveLength(3);
    expect(rows[1]).toHaveTextContent("Seasons 1–3");
    expect(rows[1]).toHaveTextContent("Approved");
    expect(rows[1]).toHaveTextContent("Season 1 ready to watch");
    expect(rows[1]).toHaveTextContent("25%");
    expect(rows[2]).toHaveTextContent("Declined: “We have it on disc”");
    expect(within(rows[0] as HTMLElement).getByRole("button", { name: "Approve" })).toBeInTheDocument();

    const tabs = screen.getByRole("tablist", { name: "Status" });
    expect(within(tabs).getByRole("tab", { name: "All 3" })).toHaveAttribute("aria-selected", "true");
    await user.click(within(tabs).getByRole("tab", { name: "Pending 1" }));
    expect(screen.getByTestId("location")).toHaveTextContent("/requests?status=pending");
    await waitFor(() => expect(within(screen.getByRole("list")).getAllByRole("listitem")).toHaveLength(1));
    expect(requests.map((r) => r.path)).toContain("/api/requests?status=pending&limit=50");
  });

  it("keeps an administrator to their own requests when asked", async () => {
    const user = userEvent.setup();
    const requests = mockApi({
      "GET /api/requests/summary": () => makeSummary({ access: "auto" }),
      "GET /api/requests": (req) => (req.path.includes("mine=1") ? makeRequestList([]) : makeRequestList([pending])),
    });
    show(<RequestsPage />);
    await screen.findByRole("list");
    await user.click(screen.getByRole("switch", { name: "Only mine" }));
    expect(await screen.findByText("You haven't asked for anything yet.")).toBeInTheDocument();
    expect(requests.map((r) => r.path)).toContain("/api/requests?mine=1&limit=50");
  });

  it("shows 50 at a time", async () => {
    const user = userEvent.setup();
    const many = Array.from({ length: 50 }, (_, i) => makeRequest({ id: 100 + i, title: `Title ${i}` }));
    const requests = mockApi({
      "GET /api/requests/summary": () => makeSummary(),
      "GET /api/requests": (req) =>
        req.path.includes("offset=50") ? makeRequestList([makeRequest({ id: 999, title: "The Last One" })], 51) : makeRequestList(many, 51),
    });
    show(<RequestsPage />);
    await user.click(await screen.findByRole("button", { name: "Show more" }));
    expect(await screen.findByText("The Last One")).toBeInTheDocument();
    expect(requests.map((r) => r.path)).toContain("/api/requests?offset=50&limit=50");
    expect(screen.queryByRole("button", { name: "Show more" })).not.toBeInTheDocument();
  });

  it("deletes a request after asking, saying what Radarr keeps", async () => {
    const user = userEvent.setup();
    let list = [makeRequest({ id: 7, status: "available", item_id: 70, available_at: 1_700_000_500 })];
    const requests = mockApi({
      "GET /api/requests/summary": () => makeSummary(),
      "GET /api/requests": () => makeRequestList(list),
      "DELETE /api/requests/7": () => {
        list = [];
        return undefined;
      },
    });
    show(<RequestsPage />);
    const row = (await screen.findAllByRole("listitem"))[0] as HTMLElement;
    expect(within(row).getByRole("link", { name: "Watch" })).toBeInTheDocument();
    await user.click(within(row).getByRole("button", { name: "Delete request" }));
    const dialog = screen.getByRole("dialog", { name: "Delete this request?" });
    expect(dialog).toHaveTextContent("What Radarr has of it stays there.");
    await user.click(within(dialog).getByRole("button", { name: "Delete" }));
    expect(await screen.findByText("Deleted the request for “The Matrix”")).toBeInTheDocument();
    expect(requests.find((r) => r.method === "DELETE")?.path).toBe("/api/requests/7");
    expect(await screen.findByText("No one has asked for anything yet.")).toBeInTheDocument();
  });

  it("lists someone's own requests, which they may withdraw while they wait", async () => {
    mockApi({
      "GET /api/requests/summary": () => makeSummary(),
      "GET /api/requests": () => makeRequestList([pending, declined]),
    });
    show(<RequestsPage />, "/requests", ann);
    expect(await screen.findByRole("heading", { name: "Your requests" })).toBeInTheDocument();
    const rows = await screen.findAllByRole("listitem");
    expect(within(rows[0] as HTMLElement).getByRole("button", { name: "Withdraw" })).toBeInTheDocument();
    expect(within(rows[0] as HTMLElement).queryByRole("button", { name: "Approve" })).not.toBeInTheDocument();
    expect(within(rows[1] as HTMLElement).queryByRole("button", { name: "Withdraw" })).not.toBeInTheDocument();
    // Declined, it may be cleared away.
    expect(within(rows[1] as HTMLElement).getByRole("button", { name: "Delete request" })).toBeInTheDocument();
    expect(screen.queryByRole("switch", { name: "Only mine" })).not.toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Discover" })).toHaveAttribute("href", "/discover");
  });

  it("says requests are off when the server says so", async () => {
    mockApi({
      "GET /api/requests/summary": () => makeSummary({ enabled: false }),
      "GET /api/requests": () =>
        new Response(JSON.stringify({ code: "unavailable", message: "requests are off", setup: false }), {
          status: 503,
          headers: { "content-type": "application/json" },
        }),
    });
    show(<RequestsPage />, "/requests", ann);
    expect(await screen.findByRole("heading", { name: "Requests are off" })).toBeInTheDocument();
  });
});
