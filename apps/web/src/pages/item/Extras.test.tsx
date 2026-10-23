import { screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { makeItem } from "@/test/fixtures";
import { renderWithProviders } from "@/test/render";
import { extraSubtitle, ExtrasRow } from "./Extras";
import { encodeId } from "@/lib/ids";

describe("extraSubtitle", () => {
  it("names the kind of extra and its length", () => {
    expect(extraSubtitle({ extra_type: "trailer", duration_ms: 150_000 })).toBe("Trailer · 3m");
    expect(extraSubtitle({ extra_type: "behind_the_scenes", duration_ms: null })).toBe("Behind the Scenes");
    expect(extraSubtitle({ extra_type: "blooper" })).toBe("Extra");
    expect(extraSubtitle({ extra_type: null, duration_ms: 20_000 })).toBe("Extra · 20s");
  });
});

describe("ExtrasRow", () => {
  it("links each extra to the player", () => {
    const trailer = makeItem({ id: 41, kind: "extra", title: "Official Trailer", extra_type: "trailer", duration_ms: 120_000 });
    const featurette = makeItem({ id: 42, kind: "extra", title: "Making Of", extra_type: "featurette" });
    renderWithProviders(<ExtrasRow extras={[trailer, featurette]} />);

    expect(screen.getByRole("heading", { name: "Extras" })).toBeInTheDocument();
    expect(screen.getByText("Trailer · 2m")).toBeInTheDocument();
    const links = screen.getAllByRole("link", { name: /Official Trailer/ });
    expect(links.length).toBeGreaterThan(0);
    for (const link of links) expect(link).toHaveAttribute("href", `/play/${encodeId(41)}`);
    expect(screen.getAllByRole("link", { name: /Making Of/ })[0]).toHaveAttribute("href", `/play/${encodeId(42)}`);
  });

  it("renders nothing without extras", () => {
    const { container } = renderWithProviders(<ExtrasRow extras={[]} />);
    expect(container).toBeEmptyDOMElement();
  });
});
