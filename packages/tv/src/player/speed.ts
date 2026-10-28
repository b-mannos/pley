/**
 * Playback speed (the web's player/speed.ts): the speeds offered, for one
 * title at a time (the next plays at normal speed). Only engines that play
 * them with their sound offer them: LG's <video>, not Samsung's AVPlay,
 * whose speeds are a silent fast-forward (docs/TV_PLATFORMS.md).
 */
export const SPEEDS: readonly number[] = [0.5, 0.75, 1, 1.25, 1.5, 2];

/** "Normal", "1.5×". */
export function speedLabel(speed: number): string {
  return speed === 1 ? "Normal" : `${speed}×`;
}
