/**
 * How pley draws text subtitles (player/SubtitleOverlay.tsx): white, a little
 * heavy, with a dark outline and no box, as Plex does; in a size and with a
 * background chosen on each device.
 */

export type SubtitleSize = "small" | "normal" | "large" | "huge";
export type SubtitleBackground = "none" | "box";

export interface SubtitleStyle {
  size: SubtitleSize;
  background: SubtitleBackground;
}

export const DEFAULT_SUBTITLE_STYLE: SubtitleStyle = { size: "normal", background: "none" };

/** The sizes offered, with the text's height as a share of the player's. */
export const SUBTITLE_SIZES: { id: SubtitleSize; label: string; share: number }[] = [
  { id: "small", label: "Small", share: 3.8 },
  { id: "normal", label: "Normal", share: 4.8 },
  { id: "large", label: "Large", share: 6 },
  { id: "huge", label: "Huge", share: 7.6 },
];

export const SUBTITLE_BACKGROUNDS: { id: SubtitleBackground; label: string }[] = [
  { id: "none", label: "None" },
  { id: "box", label: "Dark box" },
];

/** The CSS font size for `size`: a share of the player's height, never unreadably small. */
export function subtitleFontSize(size: SubtitleSize): string {
  const share = SUBTITLE_SIZES.find((s) => s.id === size)?.share ?? 4.8;
  return `clamp(15px, ${share}cqh, 96px)`;
}

/** A style read back from storage: anything unknown falls back to the default. */
export function subtitleStyle(stored: unknown): SubtitleStyle {
  const value = (stored ?? {}) as Partial<SubtitleStyle>;
  return {
    size: SUBTITLE_SIZES.some((s) => s.id === value.size) ? (value.size as SubtitleSize) : DEFAULT_SUBTITLE_STYLE.size,
    background: SUBTITLE_BACKGROUNDS.some((b) => b.id === value.background)
      ? (value.background as SubtitleBackground)
      : DEFAULT_SUBTITLE_STYLE.background,
  };
}

/** What the overlay needs of a cue (a `VTTCue` in browsers). */
export interface SubtitleCue {
  startTime: number;
  endTime: number;
  text: string;
  /** `"auto"` (the default) or a line number / percentage from the top. */
  line?: number | "auto" | null;
  snapToLines?: boolean;
  getCueAsHTML(): DocumentFragment;
}

/**
 * Whether a cue goes at the top: cues placed in the upper half (signs and
 * notes that a file moves out of the way of the dialogue). Everything else
 * goes at the bottom.
 */
export function cueAtTop(cue: SubtitleCue): boolean {
  const line = cue.line;
  if (line == null || line === "auto") return false;
  // Line numbers count from the top when positive; percentages are of the height.
  return cue.snapToLines === false ? line < 50 : line >= 0 && line < 4;
}

/** `"01:02:03.456"` (or `"02:03,456"`) → seconds. */
function cueSeconds(time: string): number {
  return time
    .replace(",", ".")
    .split(":")
    .reduce((total, part) => total * 60 + Number(part), 0);
}

const TIMING = /^\s*((?:\d+:)?\d{1,2}:\d{2}[.,]\d{1,3})\s+-->\s+((?:\d+:)?\d{1,2}:\d{2}[.,]\d{1,3})(.*)$/;

/**
 * The cues of a WebVTT file (`VTTCue`s in browsers; `Cue` stands in for
 * them in tests). Only what pley uses is read: the times, the text and the
 * line a cue is placed on.
 */
export function parseVtt<C extends Pick<VTTCue, "line" | "snapToLines">>(
  text: string,
  Cue: new (start: number, end: number, text: string) => C,
): C[] {
  const cues: C[] = [];
  for (const block of text.replace(/\r\n?/g, "\n").split(/\n{2,}/)) {
    const lines = block.split("\n");
    const at = lines.findIndex((l) => l.includes("-->"));
    const timing = at >= 0 ? TIMING.exec(lines[at] ?? "") : null;
    if (!timing) continue;
    const body = lines.slice(at + 1).join("\n").trim();
    if (!body) continue;
    const [, from = "", to = "", settings = ""] = timing;
    const start = cueSeconds(from);
    const end = cueSeconds(to);
    if (!(end > start)) continue;
    const cue = new Cue(start, end, body);
    const line = /(?:^|\s)line:(-?\d+(?:\.\d+)?)(%)?/.exec(settings);
    if (line) {
      cue.snapToLines = !line[2];
      cue.line = Number(line[1] ?? "");
    }
    cues.push(cue);
  }
  return cues;
}

/**
 * The cues showing at `seconds`, in order. (Not the text track's
 * `activeCues`: after a seek browsers keep those as they were until the
 * video plays again, which can be seconds while it loads.)
 */
export function cuesAt<C extends Pick<SubtitleCue, "startTime" | "endTime">>(cues: readonly C[], seconds: number): C[] {
  return cues.filter((c) => c.startTime <= seconds && seconds < c.endTime).sort((a, b) => a.startTime - b.startTime);
}
