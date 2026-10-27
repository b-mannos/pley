import { screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import type { MediaFile } from "@/api/types";
import { makeStream } from "@/test/fixtures";
import { renderWithProviders, testUser } from "@/test/render";
import { MediaInfo } from "./MediaInfo";

function makeFile(id: number, overrides: Partial<MediaFile> = {}): MediaFile {
  return {
    id,
    path: `/media/movie-${id}.mkv`,
    size: 4_000_000_000,
    container: "mkv",
    duration_ms: 7_200_000,
    bitrate: 8_000_000,
    width: 1920,
    height: 1080,
    video_codec: "hevc",
    audio_codec: "eac3",
    streams: [makeStream({ index: 0, kind: "video", codec: "hevc" }), makeStream({ index: 1, kind: "audio", codec: "eac3", language: "eng" })],
    edition: null,
    selected_audio: null,
    selected_subtitle: null,
    chapters: [],
    ...overrides,
  };
}

describe("MediaInfo", () => {
  it("is closed until asked for, then opens and closes from the keyboard", async () => {
    const user = userEvent.setup();
    renderWithProviders(<MediaInfo files={[makeFile(1)]} />);

    const toggle = screen.getByRole("button", { name: "Media info" });
    expect(toggle).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByText("/media/movie-1.mkv")).not.toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();

    await user.tab();
    expect(toggle).toHaveFocus();
    await user.keyboard("{Enter}");
    expect(toggle).toHaveAttribute("aria-expanded", "true");
    const panel = document.getElementById(toggle.getAttribute("aria-controls") ?? "");
    expect(panel).toBeVisible();
    expect(panel).toHaveTextContent("/media/movie-1.mkv");
    expect(panel).toHaveTextContent("8.0 Mbps");
    expect(screen.getByRole("table")).toHaveTextContent("English");

    await user.keyboard(" ");
    expect(toggle).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByText("/media/movie-1.mkv")).not.toBeInTheDocument();
  });

  it("opens each version on its own", async () => {
    const user = userEvent.setup();
    renderWithProviders(<MediaInfo files={[makeFile(1, { edition: "Director's Cut" }), makeFile(2)]} />);

    await user.click(screen.getByRole("button", { name: "Media info (2 versions)" }));
    const cut = screen.getByRole("button", { name: "Director's Cut" });
    const second = screen.getByRole("button", { name: "Version 2" });
    expect(cut).toHaveAttribute("aria-expanded", "false");
    expect(second).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByText("/media/movie-1.mkv")).not.toBeInTheDocument();

    await user.click(second);
    expect(second).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByText("/media/movie-2.mkv")).toBeInTheDocument();
    expect(screen.queryByText("/media/movie-1.mkv")).not.toBeInTheDocument();
  });

  it("shows administrators where the file is, and everyone else its name", async () => {
    const user = userEvent.setup();
    const { unmount } = renderWithProviders(<MediaInfo files={[makeFile(1)]} />);
    await user.click(screen.getByRole("button", { name: "Media info" }));
    expect(screen.getByText("/media/movie-1.mkv")).toBeInTheDocument();
    unmount();

    renderWithProviders(<MediaInfo files={[makeFile(1)]} />, { user: { ...testUser, is_admin: false } });
    await user.click(screen.getByRole("button", { name: "Media info" }));
    expect(screen.getByText("movie-1.mkv")).toBeInTheDocument();
    expect(screen.queryByText("/media/movie-1.mkv")).not.toBeInTheDocument();
  });
});
