import { Route, Routes } from "react-router";
import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { type FakeRequest, LocationDisplay, mockApi, renderWithProviders } from "@/test/render";
import { SaveFiltersModal } from "./SaveFiltersModal";

const FILTER = "genre=Comedy&unwatched=1&sort=year%3Adesc";

function lastPost(requests: FakeRequest[], path: string) {
  return requests.filter((r) => r.method === "POST" && r.path === path).at(-1);
}

function renderModal(allowCollection: boolean) {
  const requests = mockApi({
    "POST /api/playlists": () => ({ id: 9, entries: [] }),
    "POST /api/libraries/3/collections": () => ({ id: 4, library_id: 3, items: [] }),
  });
  renderWithProviders(
    <Routes>
      <Route path="/library/3" element={<SaveFiltersModal onClose={vi.fn()} libraryId={3} filter={FILTER} allowCollection={allowCollection} />} />
      <Route path="*" element={<LocationDisplay />} />
    </Routes>,
    { route: "/library/3" },
  );
  return requests;
}

describe("SaveFiltersModal", () => {
  it("says what it saves and makes a smart playlist", async () => {
    const user = userEvent.setup();
    const requests = renderModal(false);
    expect(screen.getByText(/Genre: Comedy · Unwatched · Newest first/)).toBeInTheDocument();
    expect(screen.queryByRole("radiogroup")).not.toBeInTheDocument();

    await user.type(screen.getByLabelText("Title"), "Comedies to watch");
    await user.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() =>
      expect(lastPost(requests, "/api/playlists")?.body).toEqual({ title: "Comedies to watch", library_id: 3, smart_filter: FILTER }),
    );
    expect(await screen.findByTestId("location")).toHaveTextContent("/playlists/9");
  });

  it("lets administrators make a smart collection instead", async () => {
    const user = userEvent.setup();
    const requests = renderModal(true);
    await user.type(screen.getByLabelText("Title"), "Comedies");
    await user.click(screen.getByRole("radio", { name: /Smart collection/ }));
    await user.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() =>
      expect(lastPost(requests, "/api/libraries/3/collections")?.body).toEqual({ title: "Comedies", smart_filter: FILTER }),
    );
    expect(await screen.findByTestId("location")).toHaveTextContent("/collection/4");
  });
});
