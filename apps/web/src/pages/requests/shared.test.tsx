import { screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { makeRequest, makeSummary, makeTitle } from "@/test/requests";
import { renderWithProviders, testUser } from "@/test/render";
import { allowanceText, DownloadProgress, requestActions, seasonsLabel, TitleCard, uniqueTitles } from "./shared";

describe("seasonsLabel", () => {
  it("names seasons in runs", () => {
    expect(seasonsLabel(null)).toBe("");
    expect(seasonsLabel([2])).toBe("Season 2");
    expect(seasonsLabel([3, 1, 2, 5])).toBe("Seasons 1–3, 5");
    expect(seasonsLabel([0, 1, 2])).toBe("Seasons 1–2 and specials");
    expect(seasonsLabel([0])).toBe("Specials");
  });
});

describe("allowanceText", () => {
  it("says nothing without a limit", () => {
    expect(allowanceText(makeSummary())).toBeNull();
  });

  it("says what's left of each limit", () => {
    expect(allowanceText(makeSummary({ movies_left: 2, seasons_left: 1 }))).toBe("You can ask for 2 more movies and 1 more season this week.");
    expect(allowanceText(makeSummary({ movies_left: 0, seasons_left: 5 }))).toBe("You can ask for 5 more seasons this week, and no more movies.");
    expect(allowanceText(makeSummary({ movies_left: 0 }))).toBe("You've asked for all the movies you can this week.");
  });

  it("says when more of what's used up may be asked for", () => {
    const now = Math.floor(Date.now() / 1000);
    const [soon, later] = [now + 2 * 86400, now + 5 * 86400];
    expect(allowanceText(makeSummary({ movies_left: 0, seasons_left: 3, movies_next_at: soon }))).toBe(
      "You can ask for 3 more seasons this week, and for more movies in 2 days.",
    );
    expect(allowanceText(makeSummary({ seasons_left: 0, seasons_next_at: later }))).toBe(
      "You've asked for all the seasons you can this week: you can ask for more in 5 days.",
    );
    expect(allowanceText(makeSummary({ movies_left: 0, seasons_left: 0, movies_next_at: soon, seasons_next_at: later }))).toBe(
      "You've asked for all you can this week: you can ask for more movies in 2 days and more seasons in 5 days.",
    );
  });
});

describe("requestActions", () => {
  const ann = { ...testUser, id: 2, is_admin: false };

  it("lets administrators decide and delete, and the maker withdraw while it waits", () => {
    expect(requestActions(makeRequest(), testUser)).toEqual({ approve: true, retry: false, decline: true, withdraw: false, remove: true });
    expect(requestActions(makeRequest({ status: "failed" }), testUser)).toMatchObject({ approve: false, retry: true, decline: true });
    expect(requestActions(makeRequest(), ann)).toEqual({ approve: false, retry: false, decline: false, withdraw: true, remove: false });
    expect(requestActions(makeRequest({ status: "approved" }), ann).withdraw).toBe(false);
    expect(requestActions(makeRequest(), { ...ann, id: 3 }).withdraw).toBe(false);
  });
});

describe("titles", () => {
  it("keeps each title once across pages", () => {
    const a = makeTitle({ tmdb_id: 1 });
    const b = makeTitle({ tmdb_id: 2 });
    const show = makeTitle({ tmdb_id: 1, kind: "show" });
    expect(uniqueTitles([{ results: [a, b] }, { results: [b, show] }])).toEqual([a, b, show]);
  });

  it("links a card to the title's page and says where it stands", () => {
    renderWithProviders(<TitleCard title={makeTitle({ status: "requested" })} width={150} />);
    // The poster and the name, both to its page.
    for (const link of screen.getAllByRole("link", { name: "The Matrix" })) expect(link).toHaveAttribute("href", "/discover/movie/603");
    expect(screen.getByText("Requested")).toBeInTheDocument();
    expect(screen.getByText("1999 · Movie")).toBeInTheDocument();
    // TMDB's pictures come as they are, not resized by Pley.
    expect(document.querySelector("img")).toHaveAttribute("src", "https://image.tmdb.org/t/p/w342/matrix.jpg");
  });
});

describe("DownloadProgress", () => {
  it("shows how far a download is, and the queue's estimate", () => {
    const eta = Math.floor(Date.now() / 1000) + 20 * 60;
    renderWithProviders(<DownloadProgress download={{ progress: 0.456, state: "downloading", eta, episodes: 3 }} />);
    expect(screen.getByText("Downloading")).toBeInTheDocument();
    expect(screen.getByText("· 3 episodes")).toBeInTheDocument();
    expect(screen.getByText("46% · done in 20 minutes")).toBeInTheDocument();
  });
});
