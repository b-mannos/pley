import { Route, Routes } from "react-router";
import { screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import type { PersonDetail } from "@/api/types";
import { makeItem } from "@/test/fixtures";
import { apiError, mockApi, renderWithProviders } from "@/test/render";
import { PersonPage, roleText } from "./PersonPage";

describe("roleText", () => {
  it("describes parts and jobs", () => {
    expect(roleText([{ role: "actor", character: "Neo" }])).toBe("as Neo");
    expect(roleText([{ role: "actor" }])).toBe("Actor");
    expect(roleText([{ role: "actor", episodes: 3 }])).toBe("Guest star (3 episodes)");
    expect(roleText([{ role: "actor", character: "Bob", episodes: 1 }])).toBe("as Bob (1 episode)");
    expect(roleText([{ role: "director" }, { role: "writer" }])).toBe("Director, Writer");
    expect(roleText([])).toBe("");
  });
});

function renderPerson(id: number) {
  return renderWithProviders(
    <Routes>
      <Route path="/person/:id" element={<PersonPage />} />
    </Routes>,
    { route: `/person/${id}` },
  );
}

describe("PersonPage", () => {
  it("lists the movies and shows a person is in", async () => {
    const person: PersonDetail = {
      id: 5,
      name: "Keanu Reeves",
      credits: [
        { item: makeItem({ id: 10, title: "The Matrix", year: 1999 }), roles: [{ role: "actor", character: "Neo" }] },
        { item: makeItem({ id: 11, title: "John Wick", year: 2014 }), roles: [{ role: "actor", character: "John Wick" }] },
        { item: makeItem({ id: 12, kind: "show", title: "Swedish Dicks", year: 2016 }), roles: [{ role: "actor", episodes: 2 }] },
      ],
    };
    mockApi({ "GET /api/people/5": () => person });
    renderPerson(5);

    expect(await screen.findByRole("heading", { name: "Keanu Reeves" })).toBeInTheDocument();
    expect(screen.getByText("2 movies · 1 show in your libraries")).toBeInTheDocument();
    expect(screen.getByText("1999 · as Neo")).toBeInTheDocument();
    expect(screen.getByText("2016 · Guest star (2 episodes)")).toBeInTheDocument();
    expect(document.title).toBe("Keanu Reeves · pley");
  });

  it("explains a missing person", async () => {
    mockApi({ "GET /api/people/9": () => apiError(404, "not_found", "no such person") });
    renderPerson(9);
    expect(await screen.findByRole("heading", { name: "Person not found" })).toBeInTheDocument();
    expect(screen.getByText("No such person")).toBeInTheDocument();
  });

  it("offers Retry when the server couldn't answer, rather than saying there is no such person", async () => {
    let fail = true;
    mockApi({
      "GET /api/people/5": () =>
        fail ? apiError(500, "internal", "the server is busy") : { id: 5, name: "Keanu Reeves", credits: [] },
    });
    renderPerson(5);
    expect(await screen.findByRole("heading", { name: "Couldn't load this person" })).toBeInTheDocument();
    expect(screen.queryByText("Person not found")).not.toBeInTheDocument();
    fail = false;
    await userEvent.setup().click(screen.getByRole("button", { name: "Retry" }));
    expect(await screen.findByRole("heading", { name: "Keanu Reeves" })).toBeInTheDocument();
  });
});
