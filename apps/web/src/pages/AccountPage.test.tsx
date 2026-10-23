import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import type { User } from "@/api/types";
import { mockApi, renderWithProviders, testUser } from "@/test/render";
import { PlaybackCard } from "./AccountPage";

function serve() {
  return mockApi({ "PATCH /api/me": (req) => ({ ...testUser, ...(req.body as Partial<User>) }) });
}

describe("PlaybackCard", () => {
  it("saves the audio language and when subtitles come on", async () => {
    const user = userEvent.setup();
    const requests = serve();
    renderWithProviders(<PlaybackCard />);

    // Only forced subtitles: there is no language to read them in.
    expect(screen.queryByLabelText("Subtitle language")).not.toBeInTheDocument();
    expect(screen.getByText(/they translate the lines you're meant to read/)).toBeInTheDocument();

    await user.selectOptions(screen.getByLabelText("Audio language"), "French");
    await waitFor(() => expect(requests.at(-1)?.body).toEqual({ audio_language: "fre" }));
    expect(await screen.findByText("Audio language saved")).toBeInTheDocument();

    await user.selectOptions(screen.getByLabelText("Subtitles"), "Always");
    await waitFor(() => expect(requests.at(-1)?.body).toEqual({ subtitle_mode: "always" }));
  });

  it("asks for the language subtitles are read in", async () => {
    const user = userEvent.setup();
    const requests = serve();
    const viewer: User = { ...testUser, audio_language: "fre", subtitle_mode: "foreign" };
    renderWithProviders(<PlaybackCard />, { user: viewer });

    const language = screen.getByLabelText("Subtitle language");
    expect(language).toHaveValue("");
    expect(screen.queryByText(/without one, only forced subtitles/)).not.toBeInTheDocument();
    await user.selectOptions(language, "English");
    await waitFor(() => expect(requests.at(-1)?.body).toEqual({ subtitle_language: "eng" }));

    // The audio language back to the file's: nothing says which language is read.
    await user.selectOptions(screen.getByLabelText("Audio language"), "The file's default");
    await waitFor(() => expect(requests.at(-1)?.body).toEqual({ audio_language: null }));
  });

  it("explains that a language is needed", () => {
    serve();
    renderWithProviders(<PlaybackCard />, { user: { ...testUser, subtitle_mode: "always" } });
    expect(screen.getByText(/without one, only forced subtitles come on/)).toBeInTheDocument();
  });
});
