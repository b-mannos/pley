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

describe("ServerSettings on this server's hardware", () => {
  /** pley on Windows with an Intel graphics chip, and an FFmpeg without libplacebo, Chromaprint or -readrate. */
  const intel = {
    available: true,
    managed: true,
    max_jobs: 8,
    encoders: ["h264_qsv", "libx264"],
    active_encoder: "h264_qsv",
    hwaccels: ["qsv", "d3d11va"],
    accels: ["qsv"],
    hardware_decoding: true,
    tonemapping: ["opencl", "zscale"],
    dolby_vision: false,
    chromaprint: false,
    throttling: false,
  };
  const system = (transcoder: object, os = "windows") => ({ os, transcoder });
  const option = (name: RegExp) => screen.getByRole("option", { name });

  it("greys out what this computer can't do, and says why", async () => {
    mockApi({ "GET /api/settings": () => SETTINGS, "GET /api/system": () => system(intel) });
    renderWithProviders(<ServerSettings />);
    expect(await screen.findByRole("option", { name: "Automatic · Intel Quick Sync" })).toBeEnabled();
    expect(option(/^Intel Quick Sync$/)).toBeEnabled();
    expect(option(/^NVIDIA NVENC \(needs an NVIDIA graphics card\)$/)).toBeDisabled();
    expect(option(/^AMD AMF \(needs an AMD graphics card\)$/)).toBeDisabled();
    // Never on Windows: not offered at all.
    expect(screen.queryByRole("option", { name: /VAAPI|VideoToolbox/ })).not.toBeInTheDocument();

    expect(option(/^Automatic · OpenCL$/)).toBeEnabled();
    expect(option(/^libplacebo \(needs libplacebo and a Vulkan GPU\)$/)).toBeDisabled();
    expect(option(/^zscale/)).toBeEnabled();

    // The current values stay, greyed out with the reason.
    expect(screen.getByRole("switch", { name: "Detect intros and credits" })).toBeDisabled();
    expect(screen.getByRole("switch", { name: "Detect intros and credits" })).toBeChecked();
    expect(screen.getByText(/no Chromaprint to compare audio with/)).toBeInTheDocument();
    expect(screen.getByRole("switch", { name: "Throttle transcodes" })).toBeDisabled();
    expect(screen.getByText(/it needs FFmpeg 6\.1 or newer/)).toBeInTheDocument();
    expect(screen.getByRole("switch", { name: "Hardware decoding" })).toBeEnabled();
    // The preset is for software encoding only.
    expect(screen.getByText("Intel Quick Sync encodes here: this applies only when software encodes instead.")).toBeInTheDocument();
  });

  it("says what a choice saved before does here instead", async () => {
    mockApi({
      "GET /api/settings": () => ({ ...SETTINGS, hwaccel: "nvenc", tonemapping: "libplacebo" }),
      "GET /api/system": () => system({ ...intel, active_encoder: "libx264", hardware_decoding: false }),
    });
    renderWithProviders(<ServerSettings />);
    expect(await screen.findByLabelText("Hardware acceleration")).toHaveValue("nvenc");
    expect(screen.getByText("Needs an NVIDIA graphics card: transcodes are encoded in software instead (libx264).")).toBeInTheDocument();
    expect(screen.getByLabelText("HDR tone mapping")).toHaveValue("libplacebo");
    expect(screen.getByText("Needs libplacebo and a Vulkan GPU: HDR is tone mapped with OpenCL instead.")).toBeInTheDocument();
    expect(screen.getByRole("switch", { name: "Hardware decoding" })).toBeDisabled();
    expect(screen.getByText(/videos are decoded on the CPU/)).toBeInTheDocument();
  });

  it("keeps a choice this system never has when it was saved", async () => {
    mockApi({
      "GET /api/settings": () => ({ ...SETTINGS, hwaccel: "vaapi" }),
      "GET /api/system": () => system({ ...intel, available: false, accels: [], encoders: [] }),
    });
    renderWithProviders(<ServerSettings />);
    expect(await screen.findByLabelText("Hardware acceleration")).toHaveValue("vaapi");
    expect(option(/^VAAPI \(on Linux only\)$/)).toBeDisabled();
    expect(screen.getByText("On Linux only: transcodes are encoded in software instead.")).toBeInTheDocument();
    // The transcoder isn't running: what its FFmpeg can do isn't known, so nothing else is greyed out.
    expect(option(/^NVIDIA NVENC$/)).toBeEnabled();
    expect(option(/^Automatic \(best available\)$/)).toBeEnabled();
    expect(screen.getByRole("switch", { name: "Detect intros and credits" })).toBeEnabled();
  });

  it("judges a transcoder elsewhere by its encoders alone", async () => {
    mockApi({
      "GET /api/settings": () => SETTINGS,
      "GET /api/system": () => system({ ...intel, managed: false, url: "http://gpu-box:7540", accels: ["vaapi"] }),
    });
    renderWithProviders(<ServerSettings />);
    expect(await screen.findByRole("option", { name: "Automatic · VAAPI" })).toBeEnabled();
    expect(option(/^VAAPI$/)).toBeEnabled();
    expect(option(/^VideoToolbox \(needs an FFmpeg with VideoToolbox\)$/)).toBeDisabled();
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

  it("says when the transcoder runs fewer at once than allowed", async () => {
    mockApi({ "GET /api/settings": () => ({ ...SETTINGS, max_transcodes: 12 }), "GET /api/system": () => system(intel) });
    renderWithProviders(<ServerSettings />);
    expect(await screen.findByText(/The transcoder itself runs 8 at most/)).toBeInTheDocument();
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
