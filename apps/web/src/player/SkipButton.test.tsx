import { render, renderHook, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { ItemDetail, Marker, MediaFile } from "@/api/types";
import { makeItem, makeMarker } from "@/test/fixtures";
import { activeMarker, fileMarkers, SkipButton, useSkipMarkers } from "./SkipButton";

const intro = makeMarker({ kind: "intro", start_ms: 10_000, end_ms: 70_000 });
const credits = makeMarker({ kind: "credits", start_ms: 1_200_000, end_ms: 1_300_000 });

describe("fileMarkers", () => {
  const item = {
    ...makeItem(),
    files: [{ id: 1 }, { id: 2 }] as MediaFile[],
    markers: [intro, makeMarker({ file_id: 2, start_ms: 0, end_ms: 5_000 })],
  } as ItemDetail;

  it("takes the markers of the file being played, else the first file", () => {
    expect(fileMarkers(item, 2)).toEqual([item.markers?.[1]]);
    expect(fileMarkers(item, null)).toEqual([intro]);
    expect(fileMarkers({ ...item, markers: undefined }, 1)).toEqual([]);
  });
});

describe("activeMarker", () => {
  it("finds the segment the playhead is in, until a second before it ends", () => {
    expect(activeMarker([intro, credits], 9.9)).toBeNull();
    expect(activeMarker([intro, credits], 10)).toBe(intro);
    expect(activeMarker([intro, credits], 68.9)).toBe(intro);
    expect(activeMarker([intro, credits], 69)).toBeNull();
    expect(activeMarker([intro, credits], 1_250)).toBe(credits);
  });
});

describe("useSkipMarkers", () => {
  type Props = { seconds: number; autoSkip?: boolean; enabled?: boolean; markers?: Marker[] };

  function setup(initial: Props) {
    const onAutoSkip = vi.fn();
    const onCredits = vi.fn();
    const hook = renderHook(
      ({ seconds, autoSkip = true, enabled = true, markers = [intro, credits] }: Props) =>
        useSkipMarkers({ markers, seconds, enabled, autoSkip, onAutoSkip, onCredits }),
      { initialProps: initial },
    );
    return { ...hook, onAutoSkip, onCredits };
  }

  it("skips an intro once, even when seeking back into it", () => {
    const { result, rerender, onAutoSkip } = setup({ seconds: 5 });
    expect(result.current).toBeNull();

    rerender({ seconds: 12 });
    expect(result.current).toBe(intro);
    expect(onAutoSkip).toHaveBeenCalledTimes(1);
    expect(onAutoSkip).toHaveBeenCalledWith(intro);

    rerender({ seconds: 20 });
    rerender({ seconds: 100 });
    rerender({ seconds: 15 });
    expect(onAutoSkip).toHaveBeenCalledTimes(1);
  });

  it("only offers the button when auto-skip is off, until it is turned on", () => {
    const { result, rerender, onAutoSkip } = setup({ seconds: 12, autoSkip: false });
    expect(result.current).toBe(intro);
    expect(onAutoSkip).not.toHaveBeenCalled();

    rerender({ seconds: 13, autoSkip: true });
    expect(onAutoSkip).toHaveBeenCalledTimes(1);
  });

  it("announces the credits", () => {
    const { rerender, onCredits, onAutoSkip } = setup({ seconds: 1_000, autoSkip: false });
    rerender({ seconds: 1_201, autoSkip: false });
    expect(onCredits).toHaveBeenCalledWith(credits);
    expect(onAutoSkip).not.toHaveBeenCalled();
  });

  it("does nothing while disabled", () => {
    const { result, onAutoSkip } = setup({ seconds: 12, enabled: false });
    expect(result.current).toBeNull();
    expect(onAutoSkip).not.toHaveBeenCalled();
  });
});

describe("SkipButton", () => {
  it("names the segment and skips it", async () => {
    const onSkip = vi.fn();
    render(<SkipButton marker={credits} onSkip={onSkip} lifted={false} />);
    await userEvent.click(screen.getByRole("button", { name: /Skip Credits/ }));
    expect(onSkip).toHaveBeenCalledTimes(1);
  });
});
