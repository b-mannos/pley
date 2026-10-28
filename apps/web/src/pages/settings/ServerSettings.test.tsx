import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import type { ServiceTestResult, Settings } from "@/api/types";
import { mockApi, renderWithProviders, testQueryClient } from "@/test/render";
import { ServerSettings, TestButton } from "./ServerSettings";

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
    expect(screen.getByText(/Turning this off signs them out\. Leave it off unless you need one\./)).toBeInTheDocument();

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
});

describe("ServerSettings' transcoder", () => {
  const transcoder = { available: true, managed: true, encoders: [], hwaccels: [], tonemapping: ["zscale"], dolby_vision: false };
  const folders = { in_use: "C:\\pley\\transcode", default: "C:\\pley\\transcode", problem: null, fixed: false };

  it("chooses the transcode folder for pley's next start, and goes back to the default", async () => {
    const user = userEvent.setup();
    const requests = mockApi({
      "GET /api/settings": () => SETTINGS,
      "GET /api/system": () => ({ transcoder, transcode_dir: { ...folders, next: "D:\\fast" } }),
      "PUT /api/system/transcode-dir": () => ({ ...folders, next: null }),
    });
    renderWithProviders(<ServerSettings />);
    expect(await screen.findByText("D:\\fast")).toBeInTheDocument();
    expect(screen.getByText(/From pley's next start/)).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Use the default" }));
    expect(await screen.findByText("Saved")).toBeInTheDocument();
    expect(requests.find((r) => r.method === "PUT")?.body).toEqual({ path: null });
    expect(screen.queryByText(/From pley's next start/)).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Use the default" })).not.toBeInTheDocument();
  });

  it("says why the folder chosen isn't used, and where to get Dolby Vision", async () => {
    mockApi({
      "GET /api/settings": () => SETTINGS,
      "GET /api/system": () => ({
        transcoder,
        transcode_dir: { ...folders, next: null, problem: { folder: "E:\\gone", reason: "The device is not ready." } },
      }),
    });
    renderWithProviders(<ServerSettings />);
    expect(await screen.findByText("E:\\gone")).toBeInTheDocument();
    expect(screen.getByText(/The device is not ready\./)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Use the default" })).toBeInTheDocument();
    expect(screen.getByText(/Dolby Vision needs an FFmpeg with libplacebo/)).toBeInTheDocument();
  });

  it("leaves a folder the command line chooses alone", async () => {
    mockApi({
      "GET /api/settings": () => SETTINGS,
      "GET /api/system": () => ({ transcoder, transcode_dir: { ...folders, next: null, fixed: true } }),
    });
    renderWithProviders(<ServerSettings />);
    expect(await screen.findByText(/PLEY_TRANSCODE_DIR/)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Change…" })).not.toBeInTheDocument();
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
