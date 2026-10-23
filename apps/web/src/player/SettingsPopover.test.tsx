import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { qualitiesFor } from "./quality";
import { SettingsPopover, type SettingsPopoverProps } from "./SettingsPopover";

function renderPopover(overrides: Partial<SettingsPopoverProps> = {}) {
  const props: SettingsPopoverProps = {
    open: true,
    setOpen: vi.fn(),
    info: null,
    reason: null,
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
});
