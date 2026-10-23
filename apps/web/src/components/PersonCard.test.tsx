import { fireEvent, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { renderWithProviders } from "@/test/render";
import { PersonCard } from "./PersonCard";

describe("PersonCard", () => {
  it("links to the person, with the part they play", () => {
    renderWithProviders(<PersonCard person={{ id: 5, name: "Keanu Reeves" }} subtitle="Neo" width={120} />);
    const link = screen.getByRole("link");
    expect(link).toHaveAttribute("href", "/person/5");
    expect(link).toHaveTextContent("Keanu Reeves");
    expect(screen.getByText("Neo")).toBeInTheDocument();
  });

  it("shows initials without a photo", () => {
    renderWithProviders(<PersonCard person={{ id: 5, name: "  Keanu Charles Reeves " }} width={120} />);
    expect(screen.getByText("KR")).toBeInTheDocument();
  });

  it("shows one initial for a single name", () => {
    renderWithProviders(<PersonCard person={{ id: 6, name: "Cher" }} width={120} />);
    expect(screen.getByText("C")).toBeInTheDocument();
  });

  it("loads a sized photo and keeps the initials if it fails", () => {
    const { container } = renderWithProviders(
      <PersonCard person={{ id: 5, name: "Keanu Reeves", photo_url: "/api/people/5/photo?v=1" }} width={120} />,
    );
    const img = container.querySelector("img") as HTMLImageElement;
    expect(img).toHaveAttribute("src", "/api/people/5/photo?v=1&w=160");
    // The initials show until the photo is in.
    expect(screen.getByText("KR")).toBeInTheDocument();

    fireEvent.error(img);
    expect(container.querySelector("img")).toBeNull();
    expect(screen.getByText("KR")).toBeInTheDocument();
  });

  it("hides the initials once the photo loads", () => {
    const { container } = renderWithProviders(
      <PersonCard person={{ id: 5, name: "Keanu Reeves", photo_url: "/api/people/5/photo?v=1" }} width={120} />,
    );
    fireEvent.load(container.querySelector("img") as HTMLImageElement);
    expect(screen.queryByText("KR")).not.toBeInTheDocument();
  });
});
