import { act, render, renderHook, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { carriedSleepTimer, SleepNotice, useSleepTimer, type SleepTimer } from "./sleepTimer";

function setup({ initial = null, mediaLeft = 3_600 }: { initial?: SleepTimer | null; mediaLeft?: number } = {}) {
  const onSleep = vi.fn<() => void>();
  const hook = renderHook(({ left }) => useSleepTimer({ initial, mediaLeft: left, onSleep }), { initialProps: { left: mediaLeft } });
  return { onSleep, ...hook };
}

const advance = (ms: number) =>
  act(() => {
    vi.advanceTimersByTime(ms);
  });

describe("useSleepTimer", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("is off at first", () => {
    const { result } = setup();
    expect(result.current.timer).toBeNull();
    expect(result.current.secondsLeft).toBeNull();
    expect(result.current.notice).toBeNull();
  });

  it("says so 30 s before it pauses, then pauses", () => {
    const { result, onSleep } = setup();
    act(() => result.current.choose(15));
    expect(result.current.secondsLeft).toBe(900);

    advance(869_000);
    expect(result.current.notice).toBeNull();
    advance(1_000);
    expect(result.current.notice).toBe(30);
    expect(onSleep).not.toHaveBeenCalled();

    advance(30_000);
    expect(onSleep).toHaveBeenCalledTimes(1);
    expect(result.current.timer).toBeNull();
    expect(result.current.notice).toBeNull();
  });

  it("gives the same time again to keep watching", () => {
    const { result, onSleep } = setup();
    act(() => result.current.choose(30));
    advance(29 * 60_000 + 40_000);
    expect(result.current.notice).toBe(20);

    act(() => result.current.keepWatching());
    expect(result.current.notice).toBeNull();
    expect(result.current.secondsLeft).toBe(30 * 60 + 20);
    advance(30 * 60_000);
    expect(onSleep).not.toHaveBeenCalled();
    advance(20_000);
    expect(onSleep).toHaveBeenCalledTimes(1);
  });

  it("can be turned off", () => {
    const { result, onSleep } = setup();
    act(() => result.current.choose(45));
    act(() => result.current.choose(null));
    advance(46 * 60_000);
    expect(onSleep).not.toHaveBeenCalled();
  });

  it("counts down the end of the episode from what's left of it", () => {
    const { result, rerender } = setup({ mediaLeft: 600 });
    act(() => result.current.choose("end"));
    expect(result.current.atEnd).toBe(true);
    expect(result.current.notice).toBeNull();
    rerender({ left: 29.4 });
    expect(result.current.notice).toBe(30);
    // Keep watching: it lets the end come.
    act(() => result.current.keepWatching());
    expect(result.current.timer).toBeNull();
  });

  it("goes on with the next episode while it runs", () => {
    const now = Date.now();
    const timer = { minutes: 30, endsAt: now + 60_000 };
    expect(carriedSleepTimer({ sleep: timer }, now)).toEqual(timer);
    expect(carriedSleepTimer({ sleep: { ...timer, endsAt: now - 1 } }, now)).toBeNull();
    expect(carriedSleepTimer(null, now)).toBeNull();
    expect(carriedSleepTimer({ sleep: { end: true } }, now)).toBeNull();

    const { result } = setup({ initial: timer });
    expect(result.current.secondsLeft).toBe(60);
  });
});

describe("SleepNotice", () => {
  it("offers to keep watching", async () => {
    const user = userEvent.setup();
    const onKeepWatching = vi.fn<() => void>();
    render(<SleepNotice seconds={30} onKeepWatching={onKeepWatching} lifted={false} />);
    expect(screen.getByRole("status")).toHaveTextContent("Pausing in 30 s");
    await user.click(screen.getByRole("button", { name: "Keep watching" }));
    expect(onKeepWatching).toHaveBeenCalledTimes(1);
  });
});
