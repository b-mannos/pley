import { fireEvent, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { layoutBox, makeItem } from "@/test/fixtures";
import { apiError, mockApi, renderWithProviders } from "@/test/render";
import { StarRating } from "./StarRating";

const rated = (rating: number | null) =>
  makeItem({ id: 7, user_state: { position_ms: 0, watched: false, play_count: 0, rating } });

function stars(container: HTMLElement): HTMLButtonElement[] {
  return [...container.querySelectorAll("button")];
}

describe("StarRating", () => {
  it("shows the rating in stars", () => {
    renderWithProviders(<StarRating item={rated(7)} />);
    const slider = screen.getByRole("slider", { name: "Your rating" });
    expect(slider).toHaveAttribute("aria-valuenow", "7");
    expect(slider).toHaveAttribute("aria-valuetext", "3.5 stars");
  });

  it("says when there is no rating", () => {
    renderWithProviders(<StarRating item={rated(null)} />);
    expect(screen.getByRole("slider")).toHaveAttribute("aria-valuetext", "Not rated");
  });

  it("rates with the keyboard", async () => {
    const user = userEvent.setup();
    const requests = mockApi({ "PUT /api/items/7/rating": () => undefined });
    renderWithProviders(<StarRating item={rated(6)} />);

    screen.getByRole("slider").focus();
    await user.keyboard("{ArrowRight}");
    await user.keyboard("{Home}");
    await user.keyboard("{End}");
    await user.keyboard("{Delete}");

    await waitFor(() => expect(requests).toHaveLength(4));
    expect(requests.map((r) => r.body)).toEqual([{ rating: 7 }, { rating: null }, { rating: 10 }, { rating: null }]);
  });

  it("rates whole and half stars with the pointer, and clears the same rating", async () => {
    const requests = mockApi({ "PUT /api/items/7/rating": () => undefined });
    const { container } = renderWithProviders(<StarRating item={rated(8)} />);
    // Each star is 20px wide: its left half is a half star.
    vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue(layoutBox(0, 20));
    const fourth = stars(container)[3] as HTMLButtonElement;

    fireEvent.click(fourth, { clientX: 15 });
    await waitFor(() => expect(requests).toHaveLength(1));
    fireEvent.click(fourth, { clientX: 5 });
    await waitFor(() => expect(requests).toHaveLength(2));

    expect(requests.map((r) => r.body)).toEqual([{ rating: null }, { rating: 7 }]);
  });

  it("shows the rating being saved right away", async () => {
    let finish: (() => void) | undefined;
    mockApi({ "PUT /api/items/7/rating": () => new Promise<void>((resolve) => (finish = resolve)) });
    renderWithProviders(<StarRating item={rated(null)} />);

    screen.getByRole("slider").focus();
    await userEvent.keyboard("{End}");
    await waitFor(() => expect(screen.getByRole("slider")).toHaveAttribute("aria-valuenow", "10"));
    finish?.();
  });

  it("explains failures", async () => {
    mockApi({ "PUT /api/items/7/rating": () => apiError(500, "internal", "database is locked") });
    renderWithProviders(<StarRating item={rated(null)} />);

    screen.getByRole("slider").focus();
    await userEvent.keyboard("{ArrowRight}");
    expect(await screen.findByRole("alert")).toHaveTextContent("Database is locked");
  });
});
