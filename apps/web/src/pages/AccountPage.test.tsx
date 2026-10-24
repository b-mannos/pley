import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import type { User } from "@/api/types";
import { AudioPlayerProvider } from "@/audio/AudioPlayer";
import { type FakeRequest, mockApi, renderWithProviders, testUser } from "@/test/render";
import { AccountPage, PlaybackCard } from "./AccountPage";

/** The last account change: queries refetched after it may come later. */
function lastSave(requests: FakeRequest[]) {
  return requests.filter((r) => r.method === "PATCH" && r.path === "/api/me").at(-1);
}

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
    await waitFor(() => expect(lastSave(requests)?.body).toEqual({ audio_language: "fre" }));
    expect(await screen.findByText("Audio language saved")).toBeInTheDocument();

    await user.selectOptions(screen.getByLabelText("Subtitles"), "Always");
    await waitFor(() => expect(lastSave(requests)?.body).toEqual({ subtitle_mode: "always" }));
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
    await waitFor(() => expect(lastSave(requests)?.body).toEqual({ subtitle_language: "eng" }));

    // The audio language back to the file's: nothing says which language is read.
    await user.selectOptions(screen.getByLabelText("Audio language"), "The file's default");
    await waitFor(() => expect(lastSave(requests)?.body).toEqual({ audio_language: null }));
  });

  it("explains that a language is needed", () => {
    serve();
    renderWithProviders(<PlaybackCard />, { user: { ...testUser, subtitle_mode: "always" } });
    expect(screen.getByText(/without one, only forced subtitles come on/)).toBeInTheDocument();
  });
});

/** The whole page as `user`, with nothing in its history or devices. */
function renderAccount(user: User) {
  mockApi({
    "GET /api/me/two-step": () => ({ enabled: false, since: null, recovery_codes_left: 0 }),
    "GET /api/history": () => ({ total: 0, entries: [] }),
    "GET /api/me/devices": () => [],
  });
  renderWithProviders(
    <AudioPlayerProvider>
      <AccountPage />
    </AudioPlayerProvider>,
    { user },
  );
}

const NO_PASSWORD = "This account has no password: an administrator can give it one.";

describe("AccountPage", () => {
  it("tells an account without a password that an administrator can give it one", () => {
    renderAccount({ ...testUser, is_admin: false, has_password: false });
    expect(screen.getByText(NO_PASSWORD)).toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: "Change password" })).not.toBeInTheDocument();
  });

  it("lets an account with a password change it", () => {
    renderAccount(testUser);
    expect(screen.getByRole("heading", { name: "Change password" })).toBeInTheDocument();
    expect(screen.queryByText(NO_PASSWORD)).not.toBeInTheDocument();
  });

  it("says nothing about passwords to a managed profile", () => {
    renderAccount({ ...testUser, is_admin: false, managed: true, has_password: false });
    expect(screen.queryByRole("heading", { name: "Change password" })).not.toBeInTheDocument();
    expect(screen.queryByText(NO_PASSWORD)).not.toBeInTheDocument();
  });
});
