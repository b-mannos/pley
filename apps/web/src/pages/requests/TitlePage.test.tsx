import { Route, Routes } from "react-router";
import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import type { MediaRequest } from "@/api/requests";
import type { User } from "@/api/types";
import { itemUrl } from "@/lib/urls";
import { mockApi, renderWithProviders, testUser } from "@/test/render";
import { makeDetail, makeRequest, makeSeason, makeSummary } from "@/test/requests";
import { TitlePage } from "./TitlePage";

const ann: User = { ...testUser, id: 2, username: "ann", name: "Ann", is_admin: false };

function open(route: string, user: User = ann) {
  return renderWithProviders(
    <Routes>
      <Route path="/discover/:kind/:tmdbId" element={<TitlePage />} />
    </Routes>,
    { route, user },
  );
}

const show = makeDetail({
  kind: "show",
  tmdb_id: 1399,
  title: "Seasons Show",
  status: "library",
  item_id: 40,
  runtime_minutes: null,
  show_status: "Returning Series",
  seasons: [
    makeSeason(0, { episodes: 3, aired: 3 }),
    makeSeason(1, { in_library: 10 }),
    makeSeason(2, { in_library: 3 }),
    makeSeason(3, { requested: true }),
    makeSeason(4),
    makeSeason(5, { aired: 0, air_date: null }),
  ],
});

describe("TitlePage", () => {
  it("asks for a movie, saying an administrator decides", async () => {
    const user = userEvent.setup();
    const requests = mockApi({
      "GET /api/discover/movie/603": () => makeDetail({ cast: [{ name: "Keanu Reeves", character: "Neo", photo_url: null }] }),
      "GET /api/requests/summary": () => makeSummary(),
      "POST /api/requests": () => makeRequest(),
    });
    open("/discover/movie/603");

    expect(await screen.findByRole("heading", { name: "The Matrix", level: 1 })).toBeInTheDocument();
    expect(screen.getByText("2h 16m")).toBeInTheDocument();
    expect(screen.getByText("Neo")).toBeInTheDocument();
    await user.click(await screen.findByRole("button", { name: "Request" }));
    const dialog = screen.getByRole("dialog", { name: "Request movie" });
    expect(within(dialog).getByText("An administrator decides on it. You'll hear when it's ready to watch.")).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Request" }));

    expect(await screen.findByText("Requested “The Matrix”")).toBeInTheDocument();
    expect(requests.find((r) => r.method === "POST")?.body).toEqual({ kind: "movie", tmdb_id: 603 });
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
  });

  it("says why a request couldn't be made, in the dialog", async () => {
    const user = userEvent.setup();
    mockApi({
      "GET /api/discover/movie/603": () => makeDetail(),
      "GET /api/requests/summary": () => makeSummary(),
      "POST /api/requests": () =>
        new Response(JSON.stringify({ code: "too_many_requests", message: "you can ask again on Nov 2" }), {
          status: 429,
          headers: { "content-type": "application/json" },
        }),
    });
    open("/discover/movie/603");
    await user.click(await screen.findByRole("button", { name: "Request" }));
    const dialog = screen.getByRole("dialog", { name: "Request movie" });
    await user.click(within(dialog).getByRole("button", { name: "Request" }));
    expect(await within(dialog).findByText("You can ask again on Nov 2")).toBeInTheDocument();
  });

  it("asks for the seasons a library lacks, all of them picked at first", async () => {
    const user = userEvent.setup();
    const requests = mockApi({
      "GET /api/discover/show/1399": () => show,
      "GET /api/requests/summary": () => makeSummary({ access: "auto" }),
      "POST /api/requests": (req) => makeRequest({ kind: "show", title: "Seasons Show", seasons: (req.body as { seasons: number[] }).seasons, status: "approved" }),
    });
    open("/discover/show/1399");

    expect(await screen.findByRole("link", { name: "Open in library" })).toHaveAttribute("href", itemUrl(40));
    await user.click(await screen.findByRole("button", { name: "Request more" }));
    const dialog = screen.getByRole("dialog", { name: "Request seasons" });
    expect(within(dialog).getByText("Approved as you ask, and sent on to download.")).toBeInTheDocument();

    // Whole in a library, or asked for already: in, and not to be picked.
    expect(within(dialog).getByRole("checkbox", { name: /Season 1/ })).toBeDisabled();
    expect(within(dialog).getByRole("checkbox", { name: /Season 3/ })).toBeDisabled();
    expect(within(dialog).getByRole("checkbox", { name: /Season 2/ })).toBeChecked();
    expect(within(dialog).getByText("3 of 10 episodes in library")).toBeInTheDocument();
    expect(within(dialog).getByRole("checkbox", { name: /Season 4/ })).toBeChecked();
    // Not begun, and specials: offered, not picked.
    expect(within(dialog).getByRole("checkbox", { name: /Season 5/ })).not.toBeChecked();
    expect(within(dialog).getByRole("checkbox", { name: /Specials/ })).not.toBeChecked();
    expect(within(dialog).getByRole("checkbox", { name: /All missing seasons/ })).toBeChecked();

    await user.click(within(dialog).getByRole("checkbox", { name: /Specials/ }));
    expect(within(dialog).getByRole("checkbox", { name: /All missing seasons/ })).not.toBeChecked();
    await user.click(within(dialog).getByRole("button", { name: "Request 3 seasons" }));

    expect(await screen.findByText("“Seasons Show” is approved and on its way")).toBeInTheDocument();
    expect(requests.find((r) => r.method === "POST")?.body).toEqual({ kind: "show", tmdb_id: 1399, seasons: [0, 2, 4] });
  });

  it("won't ask for more seasons than the account has left", async () => {
    const user = userEvent.setup();
    mockApi({ "GET /api/discover/show/1399": () => show, "GET /api/requests/summary": () => makeSummary({ seasons_left: 1 }) });
    open("/discover/show/1399");
    await user.click(await screen.findByRole("button", { name: "Request more" }));
    const dialog = screen.getByRole("dialog", { name: "Request seasons" });
    expect(within(dialog).getByText("You can ask for 1 more season this week.")).toHaveClass("text-danger");
    expect(within(dialog).getByRole("button", { name: "Request 2 seasons" })).toBeDisabled();
    await user.click(within(dialog).getByRole("checkbox", { name: /Season 4/ }));
    expect(within(dialog).getByRole("button", { name: "Request 1 season" })).toBeEnabled();
  });

  it("shows a movie the account asked for, and lets it withdraw the request", async () => {
    const user = userEvent.setup();
    const pending = makeRequest({ id: 5 });
    let mine: MediaRequest[] = [pending];
    const requests = mockApi({
      "GET /api/discover/movie/603": () => makeDetail({ status: mine.length ? "requested" : "none", request_id: mine.length ? 5 : null, requests: mine }),
      "GET /api/requests/summary": () => makeSummary(),
      "DELETE /api/requests/5": () => {
        mine = [];
        return undefined;
      },
    });
    open("/discover/movie/603");

    expect(await screen.findByText("Requested")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Request" })).not.toBeInTheDocument();
    const list = screen.getByRole("list");
    expect(within(list).getByText("Pending")).toBeInTheDocument();
    expect(within(list).getByText("You")).toBeInTheDocument();
    expect(within(list).queryByRole("button", { name: "Approve" })).not.toBeInTheDocument();

    await user.click(within(list).getByRole("button", { name: "Withdraw" }));
    await user.click(within(screen.getByRole("dialog", { name: "Withdraw your request?" })).getByRole("button", { name: "Withdraw" }));
    expect(await screen.findByText("Withdrew “The Matrix”")).toBeInTheDocument();
    expect(requests.find((r) => r.method === "DELETE")?.path).toBe("/api/requests/5");
    expect(await screen.findByRole("button", { name: "Request" })).toBeInTheDocument();
  });

  it("lets an administrator approve a request, or decline it with a reason", async () => {
    const user = userEvent.setup();
    const pending = makeRequest({ id: 5 });
    const requests = mockApi({
      "GET /api/discover/movie/603": () => makeDetail({ status: "requested", request_id: 5, requests: [pending] }),
      "GET /api/requests/summary": () => makeSummary({ access: "auto", pending: 1 }),
      "POST /api/requests/5/approve": () => ({ ...pending, status: "failed", reason: "no quality profile is chosen" }),
      "POST /api/requests/5/decline": () => ({ ...pending, status: "declined" }),
    });
    open("/discover/movie/603", testUser);

    const list = await screen.findByRole("list");
    expect(within(list).getByText("Ann")).toBeInTheDocument();
    await user.click(within(list).getByRole("button", { name: "Approve" }));
    // Approved, but Radarr turned it down.
    expect(await screen.findByText("“The Matrix” couldn't go to Radarr: no quality profile is chosen")).toBeInTheDocument();
    expect(requests.find((r) => r.path === "/api/requests/5/approve")?.body).toEqual({});

    await user.click(within(list).getByRole("button", { name: "Decline" }));
    const dialog = screen.getByRole("dialog", { name: "Decline “The Matrix”?" });
    await user.type(within(dialog).getByLabelText("Reason"), "We have it on disc");
    await user.click(within(dialog).getByRole("button", { name: "Decline" }));
    expect(await screen.findByText("Declined “The Matrix”")).toBeInTheDocument();
    expect(requests.find((r) => r.path === "/api/requests/5/decline")?.body).toEqual({ reason: "We have it on disc" });
  });

  it("shows an approved request's download", async () => {
    mockApi({
      "GET /api/discover/movie/603": () =>
        makeDetail({
          status: "requested",
          request_id: 5,
          requests: [makeRequest({ id: 5, status: "approved", download: { progress: 0.5, state: "downloading", eta: null, episodes: 1 } })],
        }),
      "GET /api/requests/summary": () => makeSummary(),
    });
    open("/discover/movie/603");
    expect(await screen.findByText("Downloading")).toBeInTheDocument();
    expect(screen.getByText("50%")).toBeInTheDocument();
  });

  it("says requests are off when the server says so", async () => {
    mockApi({
      "GET /api/discover/movie/603": () =>
        new Response(JSON.stringify({ code: "unavailable", message: "requests are off", setup: false }), {
          status: 503,
          headers: { "content-type": "application/json" },
        }),
      "GET /api/requests/summary": () => makeSummary({ enabled: false }),
    });
    open("/discover/movie/603");
    expect(await screen.findByRole("heading", { name: "Requests are off" })).toBeInTheDocument();
  });

  it("says TMDB isn't answering, and tries again", async () => {
    const user = userEvent.setup();
    let down = true;
    mockApi({
      "GET /api/discover/movie/603": () =>
        down
          ? new Response(JSON.stringify({ code: "unavailable", message: "TMDB couldn't be reached: try again in a moment", setup: true }), {
              status: 503,
              headers: { "content-type": "application/json" },
            })
          : makeDetail(),
      "GET /api/requests/summary": () => makeSummary(),
    });
    open("/discover/movie/603");
    expect(await screen.findByRole("heading", { name: "TMDB isn't answering" })).toBeInTheDocument();
    expect(screen.getByText("TMDB couldn't be reached: try again in a moment")).toBeInTheDocument();
    down = false;
    await user.click(screen.getByRole("button", { name: "Retry" }));
    expect(await screen.findByRole("heading", { name: "The Matrix", level: 1 })).toBeInTheDocument();
  });

  it("knows nothing of other kinds of titles", async () => {
    mockApi({});
    open("/discover/person/5");
    expect(await screen.findByRole("heading", { name: "Page not found" })).toBeInTheDocument();
  });
});
