import { useState } from "react";
import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { mockApi, renderWithProviders } from "@/test/render";
import { makeSummary } from "@/test/requests";
import { Sidebar } from "./Sidebar";

function Harness() {
  const [open, setOpen] = useState(false);
  return (
    <>
      <button type="button" onClick={() => setOpen(true)}>
        Open menu
      </button>
      <Sidebar open={open} onClose={() => setOpen(false)} />
    </>
  );
}

describe("Sidebar drawer", () => {
  it("takes the keyboard while open and gives it back to its button when closed", async () => {
    mockApi({
      "GET /api/libraries": () => [],
      "GET /api/playlists": () => ({ playlists: [] }),
    });
    const user = userEvent.setup();
    renderWithProviders(<Harness />);

    const opener = screen.getByRole("button", { name: "Open menu" });
    await user.click(opener);
    const drawer = screen.getByRole("dialog", { name: "Menu" });
    expect(drawer).toContainElement(document.activeElement as HTMLElement);
    expect(document.body.style.overflow).toBe("hidden");

    // Shift+Tab from the first control wraps to the drawer's last one, not the page behind.
    await user.tab({ shift: true });
    expect(drawer).toContainElement(document.activeElement as HTMLElement);
    expect(within(drawer).getAllByRole("link").at(-1)).toHaveFocus();

    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog", { name: "Menu" })).not.toBeInTheDocument();
    expect(opener).toHaveFocus();
    expect(document.body.style.overflow).toBe("");
  });
});

describe("Sidebar requests", () => {
  it("shows Discover and Requests to accounts that may ask, with how many wait for an administrator", async () => {
    mockApi({
      "GET /api/libraries": () => [],
      "GET /api/playlists": () => ({ playlists: [] }),
      "GET /api/requests/summary": () => makeSummary({ access: "auto", pending: 3 }),
    });
    renderWithProviders(<Sidebar open={false} onClose={() => {}} />);
    expect(await screen.findByRole("link", { name: "Discover" })).toHaveAttribute("href", "/discover");
    expect(screen.getByRole("link", { name: "Requests 3 waiting" })).toHaveAttribute("href", "/requests");
  });

  it("leaves them out for an account that may not ask", async () => {
    mockApi({
      "GET /api/libraries": () => [],
      "GET /api/playlists": () => ({ playlists: [] }),
      "GET /api/requests/summary": () => makeSummary({ access: "off" }),
    });
    const { client } = renderWithProviders(<Sidebar open={false} onClose={() => {}} />);
    await waitFor(() => expect(client.getQueryState(["requests", "summary"])?.status).toBe("success"));
    expect(screen.queryByRole("link", { name: "Discover" })).not.toBeInTheDocument();
    expect(screen.queryByRole("link", { name: /Requests/ })).not.toBeInTheDocument();
  });
});
