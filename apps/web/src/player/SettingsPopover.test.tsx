import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { MediaStream } from "@/api/types";
import { qualitiesFor } from "./quality";
import { SettingsPopover, type SettingsPopoverProps } from "./SettingsPopover";
import { DEFAULT_SUBTITLE_STYLE } from "./subtitles";

function renderPopover(overrides: Partial<SettingsPopoverProps> = {}) {
  const props: SettingsPopoverProps = {
    open: true,
    setOpen: vi.fn(),
    qualities: qualitiesFor({ bitrate: 9_000_000, height: 1080 }),
    quality: "original",
    originalHint: "1080p · 9.0 Mbps",
    onQuality: vi.fn(),
    audioStreams: [],
    audioIndex: null,
    onAudio: vi.fn(),
    subtitleStreams: [],
    subtitleIndex: null,
    onSubtitle: vi.fn(),
    subtitleStyle: DEFAULT_SUBTITLE_STYLE,
    onSubtitleStyle: vi.fn(),
    autoSkip: false,
    onAutoSkip: vi.fn(),
    ...overrides,
  };
  render(<SettingsPopover {...props} />);
  return props;
}

describe("SettingsPopover", () => {
  it("has one menu for quality, no separate playback method", () => {
    renderPopover();
    expect(screen.getByRole("button", { name: /Quality\s*Original/ })).toBeInTheDocument();
    expect(screen.queryByText("Playback")).not.toBeInTheDocument();
  });

  it("offers the original and the qualities that convert it", async () => {
    const user = userEvent.setup();
    const props = renderPopover();
    await user.click(screen.getByRole("button", { name: /Quality/ }));
    const options = screen.getAllByRole("menuitemradio");
    expect(options.map((o) => o.textContent)).toEqual([
      "Original1080p · 9.0 Mbps",
      "8 Mbps 1080pConverted",
      "4 Mbps 720pConverted",
      "2 Mbps 480pConverted",
    ]);
    expect(options[0]).toHaveAttribute("aria-checked", "true");

    await user.click(screen.getByRole("menuitemradio", { name: /4 Mbps/ }));
    expect(props.onQuality).toHaveBeenCalledWith("4m");
    expect(props.setOpen).toHaveBeenCalledWith(false);
  });

  const english: MediaStream = {
    index: 3,
    kind: "subtitle",
    codec: "subrip",
    language: "eng",
    title: null,
    default: false,
    forced: false,
    text_based: true,
  };

  it("styles text subtitles: their size and background", async () => {
    const user = userEvent.setup();
    const props = renderPopover({ subtitleStreams: [english], subtitleIndex: 3 });
    await user.click(screen.getByRole("button", { name: /Subtitle style\s*Normal/ }));
    await user.click(screen.getByRole("menuitemradio", { name: "Large" }));
    expect(props.onSubtitleStyle).toHaveBeenCalledWith({ size: "large", background: "none" });
    await user.click(screen.getByRole("menuitemradio", { name: "Dark box" }));
    expect(props.onSubtitleStyle).toHaveBeenCalledWith({ size: "normal", background: "box" });
  });

  it("has no style to choose without text subtitles", () => {
    renderPopover({ subtitleStreams: [{ ...english, codec: "hdmv_pgs_subtitle", text_based: false }] });
    expect(screen.queryByRole("button", { name: /Subtitle style/ })).not.toBeInTheDocument();
  });
});
