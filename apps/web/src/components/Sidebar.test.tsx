import { useState } from "react";
import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { liveKeys } from "@/api/livetv";
import { mockApi, renderWithProviders } from "@/test/render";
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

describe("Sidebar Live TV", () => {
  it("shows Live TV once it has channels, not to an administrator before", async () => {
    let channels = 0;
    mockApi({
      "GET /api/libraries": () => [],
      "GET /api/playlists": () => ({ playlists: [] }),
      "GET /api/live": () => ({ available: channels > 0, allowed: true, channels, sources: channels ? 1 : 0, recent: [] }),
    });
    const { client } = renderWithProviders(<Sidebar open={false} onClose={() => {}} />);

    await waitFor(() => expect(client.getQueryState(liveKeys.summary)?.status).toBe("success"));
    expect(screen.queryByRole("link", { name: "Live TV" })).not.toBeInTheDocument();

    // A source added: the server says live_channels_changed, and the summary is asked for again.
    channels = 5;
    await client.invalidateQueries({ queryKey: liveKeys.all });
    expect(await screen.findByRole("link", { name: "Live TV" })).toHaveAttribute("href", "/live");
  });
});
