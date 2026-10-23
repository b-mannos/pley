import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Trickplay } from "@/api/types";
import { layoutBox, makeChapter } from "@/test/fixtures";
import { SeekBar, type SeekBarProps } from "./SeekBar";

beforeEach(() => {
  // A 1000px bar: every 10px is a second of a 100s video.
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue(layoutBox(0, 1000));
});

function renderBar(props: Partial<SeekBarProps> = {}) {
  const onSeek = vi.fn();
  const onScrub = vi.fn();
  render(<SeekBar current={30} duration={100} buffered={[]} onSeek={onSeek} onScrub={onScrub} {...props} />);
  return { slider: screen.getByRole("slider", { name: "Seek" }), onSeek, onScrub };
}

describe("SeekBar", () => {
  it("tells where playback is", () => {
    const { slider } = renderBar();
    expect(slider).toHaveAttribute("aria-valuenow", "30");
    expect(slider).toHaveAttribute("aria-valuetext", "0:30 of 1:40");
  });

  it("scrubs while dragging and seeks on release", () => {
    const { slider, onSeek, onScrub } = renderBar();

    fireEvent.pointerDown(slider, { clientX: 250, button: 0, pointerId: 1 });
    expect(onScrub).toHaveBeenLastCalledWith(25);
    expect(slider).toHaveAttribute("aria-valuenow", "25");

    fireEvent.pointerMove(slider, { clientX: 500, pointerId: 1 });
    expect(onScrub).toHaveBeenLastCalledWith(50);

    fireEvent.pointerUp(slider, { clientX: 500, pointerId: 1 });
    expect(onScrub).toHaveBeenLastCalledWith(null);
    expect(onSeek).toHaveBeenCalledWith(50);
    expect(slider).toHaveAttribute("aria-valuenow", "30");
  });

  it("ignores other buttons and keeps the drag inside the bar", () => {
    const { slider, onSeek, onScrub } = renderBar();
    fireEvent.pointerDown(slider, { clientX: 250, button: 2 });
    expect(onScrub).not.toHaveBeenCalled();

    fireEvent.pointerDown(slider, { clientX: 100, button: 0 });
    fireEvent.pointerUp(slider, { clientX: 5000 });
    expect(onSeek).toHaveBeenCalledWith(100);
  });

  it("cancels a drag without seeking", () => {
    const { slider, onSeek, onScrub } = renderBar();
    fireEvent.pointerDown(slider, { clientX: 250, button: 0 });
    fireEvent.pointerCancel(slider);
    expect(onScrub).toHaveBeenLastCalledWith(null);
    expect(onSeek).not.toHaveBeenCalled();
  });

  it("names the time and chapter under the pointer", () => {
    const { slider } = renderBar({ chapters: [makeChapter(0, "Opening"), makeChapter(60_000, "Heist")] });
    fireEvent.pointerMove(slider, { clientX: 700 });
    expect(screen.getByText("Heist · 1:10")).toBeInTheDocument();

    fireEvent.pointerLeave(slider);
    expect(screen.queryByText(/1:10/)).not.toBeInTheDocument();
  });

  it("shows the preview tile for the time under the pointer", () => {
    const trickplay: Trickplay = { id: "abc", interval_ms: 10_000, width: 320, height: 180, columns: 5, rows: 2, count: 10, sheets: 1 };
    const { slider } = renderBar({ previews: { fileId: 3, trickplay } });
    fireEvent.pointerMove(slider, { clientX: 720 });

    // 72s is tile 7: the third in the second row (224px wide, 126px high on screen).
    const tile = document.querySelector<HTMLElement>("[style*='trickplay']");
    expect(tile?.style.backgroundImage).toContain("/api/files/3/trickplay/0.jpg?v=abc");
    expect(tile?.style.backgroundPosition).toBe("-448px -126px");
  });

  it("jumps to the start and the end", () => {
    const { slider, onSeek } = renderBar();
    fireEvent.keyDown(slider, { key: "Home" });
    fireEvent.keyDown(slider, { key: "End" });
    expect(onSeek.mock.calls).toEqual([[0], [99]]);
  });
});
