import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import type { ServiceTestResult, Settings } from "@/api/types";
import { mockApi, renderWithProviders, testQueryClient } from "@/test/render";
import { ServerSettings } from "./ServerSettings";
import { TestButton } from "./SettingsForm";

const SETTINGS = {
  server_name: "Den",
  tmdb_api_key: "",
  metadata_language: "en-US",
  tv_metadata: "auto",
  tvdb_api_key: "",
  tvdb_pin: "",
  music_metadata: true,
  hwaccel: "auto",
  hardware_decoding: true,
  tonemapping: "auto",
  max_transcodes: 4,
  transcode_preset: "veryfast",
  throttle_transcodes: true,
  transcode_in_memory: true,
  scan_interval_minutes: 360,
  watch_libraries: true,
  detect_markers: true,
  seek_previews: true,
  previews_in_background: true,
  third_party_apps: false,
} satisfies Settings;

describe("ServerSettings", () => {
  it("opens again with the settings it has, without a spinner that stays", async () => {
    mockApi({ "GET /api/settings": () => SETTINGS, "GET /api/system": () => ({}) });
    const first = renderWithProviders(<ServerSettings />);
    expect(await screen.findByLabelText("Server name")).toHaveValue("Den");
    first.unmount();
    // The second time the settings come from the cache straight away.
    renderWithProviders(<ServerSettings />, { client: first.client });
    expect(await screen.findByLabelText("Server name")).toHaveValue("Den");
  });

  it("lets apps by other publishers sign in", async () => {
    const user = userEvent.setup();
    const requests = mockApi({
      "GET /api/settings": () => SETTINGS,
      "GET /api/system": () => ({}),
      "PUT /api/settings": (req) => ({ ...SETTINGS, ...(req.body as Partial<Settings>) }),
    });
    renderWithProviders(<ServerSettings />);
    const toggle = await screen.findByRole("switch", { name: "Apps by other publishers" });
    expect(toggle).not.toBeChecked();
    expect(screen.getByText(/turning it off signs them out\./)).toBeInTheDocument();

    await user.click(toggle);
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(await screen.findByText("Settings saved")).toBeInTheDocument();
    await waitFor(() => expect(requests.find((r) => r.method === "PUT")?.body).toEqual({ third_party_apps: true }));
    expect(screen.getByRole("switch", { name: "Apps by other publishers" })).toBeChecked();
  });

  it("lists apps again once turning other publishers off signed them out", async () => {
    const user = userEvent.setup();
    mockApi({
      "GET /api/settings": () => ({ ...SETTINGS, third_party_apps: true }),
      "GET /api/system": () => ({}),
      "PUT /api/settings": (req) => ({ ...SETTINGS, ...(req.body as Partial<Settings>) }),
    });
    const client = testQueryClient();
    // What the account page and the users dialog showed before.
    client.setQueryData(["me", "apps"], []);
    client.setQueryData(["users", 2, "apps"], []);
    renderWithProviders(<ServerSettings />, { client });

    await user.click(await screen.findByRole("switch", { name: "Apps by other publishers" }));
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(await screen.findByText("Settings saved")).toBeInTheDocument();
    await waitFor(() => expect(client.getQueryState(["me", "apps"])?.isInvalidated).toBe(true));
    expect(client.getQueryState(["users", 2, "apps"])?.isInvalidated).toBe(true);
  });

  it("keeps apps by other publishers off when the server doesn't say", async () => {
    const older: Partial<Settings> = { ...SETTINGS };
    delete older.third_party_apps;
    mockApi({ "GET /api/settings": () => older, "GET /api/system": () => ({}) });
    renderWithProviders(<ServerSettings />);
    expect(await screen.findByRole("switch", { name: "Apps by other publishers" })).not.toBeChecked();
    expect(screen.getByRole("button", { name: "Save" })).toBeDisabled();
  });

  it("greys out previews for whole libraries without seek previews", async () => {
    const user = userEvent.setup();
    mockApi({ "GET /api/settings": () => SETTINGS, "GET /api/system": () => ({}) });
    renderWithProviders(<ServerSettings />);
    const whole = await screen.findByRole("switch", { name: "Previews for whole libraries" });
    expect(whole).toBeEnabled();
    await user.click(screen.getByRole("switch", { name: "Seek previews" }));
    expect(whole).toBeDisabled();
    expect(whole).toBeChecked();
    expect(screen.getByText("Needs Seek previews.")).toBeInTheDocument();
  });

  it("greys out intro detection when the transcoder's FFmpeg has no Chromaprint, and says why", async () => {
    mockApi({
      "GET /api/settings": () => SETTINGS,
      "GET /api/system": () => ({ os: "windows", transcoder: { available: true, managed: true, encoders: ["libx264"], accels: [], chromaprint: false } }),
    });
    renderWithProviders(<ServerSettings />);
    const detect = await screen.findByRole("switch", { name: "Detect intros and credits" });
    // The current value stays, greyed out with the reason.
    expect(detect).toBeDisabled();
    expect(detect).toBeChecked();
    expect(screen.getByText(/lacks Chromaprint/)).toBeInTheDocument();
  });

  it("leaves the transcoder to its own page", async () => {
    mockApi({ "GET /api/settings": () => SETTINGS, "GET /api/system": () => ({}) });
    renderWithProviders(<ServerSettings />);
    expect(await screen.findByLabelText("Server name")).toBeInTheDocument();
    expect(screen.queryByLabelText("Hardware acceleration")).not.toBeInTheDocument();
    expect(screen.queryByRole("switch", { name: "Throttle transcodes" })).not.toBeInTheDocument();
  });
});

describe("TestButton", () => {
  it("tests the key as typed and says how it went", async () => {
    const user = userEvent.setup();
    const requests = mockApi({
      "POST /api/settings/test": ({ body }): ServiceTestResult =>
        (body as { api_key: string }).api_key === "good"
          ? { ok: true, message: "TMDB accepted the key.", ms: 182 }
          : { ok: false, message: "TMDB rejected this key.", ms: 95 },
    });
    const { rerender } = renderWithProviders(<TestButton label="Test the TMDB key" test={{ service: "tmdb", api_key: "good" }} />);

    await user.click(screen.getByRole("button", { name: "Test the TMDB key" }));
    expect(await screen.findByText("TMDB accepted the key.")).toBeInTheDocument();
    expect(screen.getByText("182 ms")).toBeInTheDocument();
    expect(requests.at(-1)?.body).toEqual({ service: "tmdb", api_key: "good" });

    // A new key, a new answer (the settings page remounts it by key).
    rerender(<TestButton key="bad" label="Test the TMDB key" test={{ service: "tmdb", api_key: "bad" }} />);
    expect(screen.queryByText("TMDB accepted the key.")).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Test the TMDB key" }));
    expect(await screen.findByText("TMDB rejected this key.")).toBeInTheDocument();
    expect(screen.queryByText("95 ms")).not.toBeInTheDocument();
  });
});
