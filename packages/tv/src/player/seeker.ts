/**
 * Seeking with the remote (the Android app's TvSeeker): each press steps
 * 10 s (media keys 30 s) from where the last press left the target, and
 * the seek happens once, 400 ms after the last press; a badge sums the
 * presses. A held key scrubs faster the longer it is held (30×, then 90×,
 * then a whole video in about 15 s) and seeks once, on release.
 */

export const STEP_MS = 10_000;
export const MEDIA_STEP_MS = 30_000;
/** The seek waits this long after the last press. */
export const SETTLE_MS = 400;
/** The badge goes this long after the last press. */
export const BADGE_MS = 600;
/** A held key's repeats count at most this much time each. */
const MAX_REPEAT_GAP_MS = 200;
/** The target stays this far before the end. */
const END_MARGIN_MS = 1_000;

export interface SeekerClock {
  now(): number;
  setTimeout(fn: () => void, ms: number): unknown;
  clearTimeout(handle: unknown): void;
}

const realClock: SeekerClock = {
  now: () => (typeof performance !== "undefined" ? performance.now() : Date.now()),
  setTimeout: (fn, ms) => setTimeout(fn, ms),
  clearTimeout: (h) => clearTimeout(h as ReturnType<typeof setTimeout>),
};

/** Video ms scrubbed per ms held: 30× for 1.5 s, 90× until 4 s, then a whole video in ~15 s (at least 180×). */
export function holdRate(heldMs: number, durationMs: number): number {
  if (heldMs < 1_500) return 30;
  if (heldMs < 4_000) return 90;
  return Math.max(180, durationMs / 15_000);
}

export class Seeker {
  /** Where the next seek goes; `null` when none is pending. */
  target: number | null = null;
  /** The presses' sum in a row, for the badge; 0 hides it. */
  badgeMs = 0;
  holding = false;
  private holdStart = 0;
  private lastRepeat = 0;
  private settle: unknown = null;
  private badgeTimer: unknown = null;
  private readonly clock: SeekerClock;
  private readonly onSeek: (ms: number) => void;
  private readonly onChange: () => void;

  constructor(opts: { onSeek: (ms: number) => void; onChange: () => void; clock?: SeekerClock }) {
    this.onSeek = opts.onSeek;
    this.onChange = opts.onChange;
    this.clock = opts.clock ?? realClock;
  }

  private clamp(ms: number, durationMs: number): number {
    const top = durationMs > 0 ? Math.max(0, durationMs - END_MARGIN_MS) : Number.MAX_SAFE_INTEGER;
    return Math.max(0, Math.min(top, ms));
  }

  /** A press: one step from the pending target, else from the playhead. */
  press(direction: 1 | -1, stepMs: number, positionMs: number, durationMs: number): void {
    const from = this.target ?? positionMs;
    this.target = this.clamp(from + direction * stepMs, durationMs);
    this.badgeMs += direction * stepMs;
    if (this.settle !== null) this.clock.clearTimeout(this.settle);
    this.settle = this.clock.setTimeout(() => this.commit(), SETTLE_MS);
    if (this.badgeTimer !== null) this.clock.clearTimeout(this.badgeTimer);
    this.badgeTimer = this.clock.setTimeout(() => {
      this.badgeTimer = null;
      this.badgeMs = 0;
      this.onChange();
    }, BADGE_MS);
    this.onChange();
  }

  /** A repeat of a held key: no seek until it is let go. */
  repeat(direction: 1 | -1, positionMs: number, durationMs: number): void {
    const now = this.clock.now();
    if (!this.holding) {
      this.holding = true;
      this.holdStart = now;
      this.lastRepeat = now;
      this.badgeMs = 0;
      if (this.settle !== null) this.clock.clearTimeout(this.settle);
      this.settle = null;
      if (this.target === null) this.target = positionMs;
    }
    const dt = Math.min(MAX_REPEAT_GAP_MS, now - this.lastRepeat);
    this.lastRepeat = now;
    const advance = holdRate(now - this.holdStart, durationMs) * dt;
    this.target = this.clamp((this.target ?? positionMs) + direction * advance, durationMs);
    this.onChange();
  }

  /** The key let go: a held key seeks now. */
  release(): void {
    if (!this.holding) return;
    this.holding = false;
    this.commit();
  }

  /** Forgets the pending seek (Up Next or an error came). */
  cancel(): void {
    if (this.settle !== null) this.clock.clearTimeout(this.settle);
    if (this.badgeTimer !== null) this.clock.clearTimeout(this.badgeTimer);
    this.settle = null;
    this.badgeTimer = null;
    this.target = null;
    this.badgeMs = 0;
    this.holding = false;
    this.onChange();
  }

  private commit(): void {
    this.settle = null;
    const t = this.target;
    this.target = null;
    if (t !== null) this.onSeek(t);
    this.onChange();
  }
}

/** The badge's text: "−10 s", "+30 s". */
export function badgeText(ms: number): string {
  const s = Math.floor((Math.abs(ms) + 500) / 1000);
  return `${ms < 0 ? "−" : "+"}${s} s`;
}
