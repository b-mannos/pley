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
    subtitleOffset: 0,
    onSubtitleOffset: vi.fn(),
    sleep: null,
    sleepLeft: null,
    sleepEndLabel: "End of this episode",
    onSleep: vi.fn(),
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

  it("names the quality the original is converted at", async () => {
    const user = userEvent.setup();
    renderPopover({ qualities: qualitiesFor({ bitrate: 60_000_000, height: 2160 }), originalHint: "4K · 60 Mbps", autoQuality: "30m" });
    expect(screen.getByRole("button", { name: /Quality\s*30 Mbps 4K \(auto\)/ })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: /Quality/ }));
    const original = screen.getAllByRole("menuitemradio")[0];
    expect(original).toHaveTextContent("Plays as 30 Mbps 4K here (the file: 4K · 60 Mbps)");
    expect(original).toHaveAttribute("aria-checked", "true");
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

  it("moves text subtitles earlier or later, 0.1 s at a time", async () => {
    const user = userEvent.setup();
    const props = renderPopover({ subtitleStreams: [english], subtitleIndex: 3, subtitleOffset: 0.5 });
    // Said next to the subtitles chosen.
    await user.click(screen.getByRole("button", { name: /Subtitles.*\+0\.5 s/ }));
    expect(screen.getByTestId("subtitle-offset")).toHaveTextContent("+0.5 s");

    await user.click(screen.getByRole("button", { name: "Show subtitles later" }));
    expect(props.onSubtitleOffset).toHaveBeenLastCalledWith(0.6);
    await user.click(screen.getByRole("button", { name: "Show subtitles earlier" }));
    expect(props.onSubtitleOffset).toHaveBeenLastCalledWith(0.4);
    await user.click(screen.getByRole("button", { name: "Reset" }));
    expect(props.onSubtitleOffset).toHaveBeenLastCalledWith(0);
    // The menu stays open to try again.
    expect(props.setOpen).not.toHaveBeenCalled();
  });

  it("stops at ten seconds, and has nothing to reset at 0", async () => {
    const user = userEvent.setup();
    renderPopover({ subtitleStreams: [english], subtitleIndex: 3, subtitleOffset: 10 });
    await user.click(screen.getByRole("button", { name: /Subtitles/ }));
    expect(screen.getByRole("button", { name: "Show subtitles later" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Reset" })).toBeEnabled();
  });

  it("doesn't offer to move subtitles that are off or burned in", async () => {
    const user = userEvent.setup();
    const pgs: MediaStream = { ...english, index: 4, codec: "hdmv_pgs_subtitle", text_based: false };
    renderPopover({ subtitleStreams: [english, pgs], subtitleIndex: 4, subtitleOffset: 0.5 });
    expect(screen.getByRole("button", { name: /Subtitles/ })).not.toHaveTextContent("+0.5 s");
    await user.click(screen.getByRole("button", { name: /Subtitles/ }));
    expect(screen.queryByText("Timing")).not.toBeInTheDocument();
  });

  it("sets the sleep timer", async () => {
    const user = userEvent.setup();
    const props = renderPopover();
    await user.click(screen.getByRole("button", { name: /Sleep timer\s*Off/ }));
    expect(screen.getAllByRole("menuitemradio").map((o) => o.textContent)).toEqual([
      "Off",
      "15 minutes",
      "30 minutes",
      "45 minutes",
      "60 minutes",
      "90 minutes",
      "End of this episode",
    ]);
    await user.click(screen.getByRole("menuitemradio", { name: "45 minutes" }));
    expect(props.onSleep).toHaveBeenCalledWith(45);
    expect(props.setOpen).toHaveBeenCalledWith(false);
  });

  it("says how long the sleep timer has left", async () => {
    const user = userEvent.setup();
    const props = renderPopover({ sleep: 30, sleepLeft: 754 });
    await user.click(screen.getByRole("button", { name: /Sleep timer\s*12:34 left/ }));
    expect(screen.getByRole("menuitemradio", { name: /30 minutes/ })).toHaveAttribute("aria-checked", "true");
    await user.click(screen.getByRole("menuitemradio", { name: "Off" }));
    expect(props.onSleep).toHaveBeenCalledWith(null);
  });
});
