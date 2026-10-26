import { act, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { APP_DOWNLOAD_EVENT, type AppDownload } from "@/lib/appDownloads";
import { renderWithProviders } from "@/test/render";
import { AppDownloads } from "./AppDownloads";

const ALIEN: AppDownload = {
  id: 3,
  name: "Alien (1979).mkv",
  size: 8_804_682_752,
  received: 0,
  state: "queued",
  error: null,
};

/** What the desktop app sends the page. */
function tell(download: AppDownload) {
  act(() => void window.dispatchEvent(new CustomEvent(APP_DOWNLOAD_EVENT, { detail: download })));
}

describe("AppDownloads", () => {
  beforeAll(() => {
    (window as unknown as { __PLEY_APP__?: string }).__PLEY_APP__ = "desktop";
  });
  afterAll(() => {
    delete (window as unknown as { __PLEY_APP__?: string }).__PLEY_APP__;
  });

  it("ignores words that aren't a download", () => {
    renderWithProviders(<AppDownloads />);
    act(() => void window.dispatchEvent(new CustomEvent(APP_DOWNLOAD_EVENT, { detail: { id: 9, state: "nope" } })));
    expect(screen.queryByRole("button")).toBeNull();
  });

  it("follows the desktop app's downloads from this server", async () => {
    const user = userEvent.setup();
    renderWithProviders(<AppDownloads />);
    expect(screen.queryByRole("button")).toBeNull();

    tell(ALIEN);
    expect(screen.getByRole("status")).toHaveTextContent("Downloading Alien (1979).mkv");
    tell({ ...ALIEN, state: "downloading", received: 3_650_722_201 });
    const button = screen.getByRole("button", { name: "Downloading Alien (1979).mkv, 41%" });

    await user.click(button);
    expect(screen.getByText("3.4 GB of 8.2 GB")).toBeInTheDocument();

    tell({ ...ALIEN, state: "done", received: ALIEN.size! });
    expect(screen.getByText("Downloaded Alien (1979).mkv")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /^Downloading/ })).toBeNull();

    // Asked for again: it is there already.
    tell({ ...ALIEN, state: "done", received: ALIEN.size! });
    expect(screen.getByText("Alien (1979).mkv is downloaded already")).toBeInTheDocument();
  });

  it("says the server is making a file for the app's player", async () => {
    const user = userEvent.setup();
    renderWithProviders(<AppDownloads />);
    tell({ ...ALIEN, id: 5, name: "Alien (1979).mp4", size: null, state: "preparing", preparing: 0.42 });

    await user.click(screen.getByRole("button", { name: "Downloading Alien (1979).mp4" }));
    expect(screen.getByText("Preparing on the server · 42%")).toBeInTheDocument();
  });

  it("says why a download failed, and where a file the page made went", () => {
    renderWithProviders(<AppDownloads />);
    tell({ ...ALIEN, id: 4, name: "Heat (1995).mkv", state: "failed", error: "This file is no longer on the server." });
    expect(screen.getByText("Couldn't download Heat (1995).mkv. This file is no longer on the server.")).toBeInTheDocument();
    tell({ ...ALIEN, id: 0, name: "pley-recovery-codes.txt", size: null, state: "done" });
    expect(screen.getByText("Saved pley-recovery-codes.txt in your Downloads folder")).toBeInTheDocument();
  });
});
