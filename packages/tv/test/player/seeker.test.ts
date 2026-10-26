import { describe, expect, it } from "vitest";
import { badgeText, holdRate, Seeker, SETTLE_MS, STEP_MS, type SeekerClock } from "../../src/player/seeker";

/** A clock the test moves by hand. */
function fakeClock() {
  let now = 0;
  let timers: { at: number; fn: () => void; id: number }[] = [];
  let next = 1;
  const clock: SeekerClock = {
    now: () => now,
    setTimeout: (fn, ms) => {
      const id = next++;
      timers.push({ at: now + ms, fn, id });
      return id;
    },
    clearTimeout: (h) => {
      timers = timers.filter((t) => t.id !== h);
    },
  };
  const advance = (ms: number) => {
    const until = now + ms;
    for (;;) {
      const due = timers.filter((t) => t.at <= until).sort((a, b) => a.at - b.at)[0];
      if (!due) break;
      timers = timers.filter((t) => t !== due);
      now = due.at;
      due.fn();
    }
    now = until;
  };
  return { clock, advance };
}

describe("seeking with the remote", () => {
  it("steps 10 s a press and seeks once, 400 ms after the last", () => {
    const { clock, advance } = fakeClock();
    const seeks: number[] = [];
    const s = new Seeker({ onSeek: (ms) => seeks.push(ms), onChange: () => undefined, clock });
    s.press(-1, STEP_MS, 600_000, 7_200_000);
    advance(100);
    s.press(-1, STEP_MS, 600_000, 7_200_000);
    advance(100);
    s.press(-1, STEP_MS, 600_000, 7_200_000);
    expect(s.badgeMs).toBe(-30_000);
    expect(badgeText(s.badgeMs)).toBe("−30 s");
    expect(seeks).toEqual([]);
    advance(SETTLE_MS + 10);
    expect(seeks).toEqual([570_000]);
    advance(600);
    expect(s.badgeMs).toBe(0);
  });

  it("stays a second short of the end, and never before the start", () => {
    const { clock, advance } = fakeClock();
    const seeks: number[] = [];
    const s = new Seeker({ onSeek: (ms) => seeks.push(ms), onChange: () => undefined, clock });
    s.press(1, 30_000, 7_190_000, 7_200_000);
    advance(500);
    s.press(-1, 30_000, 5_000, 7_200_000);
    advance(500);
    expect(seeks).toEqual([7_199_000, 0]);
  });

  it("scrubs a held key faster and faster, and seeks once on release", () => {
    const { clock, advance } = fakeClock();
    const seeks: number[] = [];
    const s = new Seeker({ onSeek: (ms) => seeks.push(ms), onChange: () => undefined, clock });
    s.press(1, STEP_MS, 600_000, 7_200_000);
    for (let i = 0; i < 100; i++) {
      advance(50);
      s.repeat(1, 600_000, 7_200_000);
    }
    expect(seeks).toEqual([]);
    expect(s.target!).toBeGreaterThan(600_000 + 150_000);
    s.release();
    expect(seeks.length).toBe(1);
    expect(s.target).toBeNull();
  });

  it("gets to a whole video's end in about 15 s of holding", () => {
    expect(holdRate(1_000, 7_200_000)).toBe(30);
    expect(holdRate(3_000, 7_200_000)).toBe(90);
    expect(holdRate(5_000, 7_200_000)).toBe(480);
    expect(holdRate(5_000, 600_000)).toBe(180);
  });
});
