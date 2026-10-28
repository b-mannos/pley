/**
 * Getting a new session's stream ready while the last one plays on (a
 * quality, audio or Automatic change): its playlist and the segment it
 * starts with are asked for, so the transcoder makes them, and the player
 * moves to it once that segment is there rather than waiting on a spinner.
 */

/** A size of a stream in several sizes, as its multivariant playlist lists it. */
export interface Variant {
  uri: string;
  width: number;
  height: number;
}

/** The sizes a multivariant playlist lists, in its order; none for a media playlist. */
export function variantsOf(master: string, base: string): Variant[] {
  const lines = master.split(/\r?\n/).map((l) => l.trim());
  const found: Variant[] = [];
  lines.forEach((line, i) => {
    if (!line.startsWith("#EXT-X-STREAM-INF")) return;
    const uri = lines.slice(i + 1).find((l) => l && !l.startsWith("#"));
    if (!uri) return;
    const [width, height] = (/RESOLUTION=(\d+)x(\d+)/.exec(line) ?? []).slice(1).map(Number);
    found.push({ uri: new URL(uri, base).href, width: width || 0, height: height || 0 });
  });
  return found;
}

/** The segment of a media playlist that holds `seconds`: the last one for a time past its end. */
export function segmentAt(playlist: string, base: string, seconds: number): string | null {
  let at = 0;
  let duration: number | null = null;
  let last: string | null = null;
  for (const raw of playlist.split(/\r?\n/)) {
    const line = raw.trim();
    if (line.startsWith("#EXTINF:")) {
      duration = Number.parseFloat(line.slice("#EXTINF:".length));
    } else if (line && !line.startsWith("#") && duration !== null) {
      last = new URL(line, base).href;
      if (seconds < at + duration) return last;
      at += duration;
      duration = null;
    }
  }
  return last;
}

async function text(url: string, signal: AbortSignal): Promise<string> {
  const res = await fetch(url, { signal, credentials: "same-origin" });
  if (!res.ok) throw new Error(`${res.status}`);
  return res.text();
}

/**
 * Asks for the stream at `url` as a player starting at `startSec` would:
 * its playlist, the size `pick` chooses of a stream in several sizes
 * (given smallest first), and the segment that size starts with. Resolves
 * once that segment has come, which is when the transcoder has made it.
 */
export async function warmStream(
  url: string,
  startSec: number,
  pick: (smallestFirst: Variant[], listedFirst: number) => number,
  signal: AbortSignal,
): Promise<void> {
  let playlistUrl = new URL(url, window.location.href).href;
  let playlist = await text(playlistUrl, signal);
  const sizes = variantsOf(playlist, playlistUrl);
  if (sizes.length) {
    const sorted = [...sizes].sort((a, b) => a.width * a.height - b.width * b.height);
    const chosen = sorted[pick(sorted, sorted.indexOf(sizes[0]!))] ?? sizes[0]!;
    playlistUrl = chosen.uri;
    playlist = await text(playlistUrl, signal);
  }
  const segment = segmentAt(playlist, playlistUrl, startSec);
  if (!segment) return;
  const res = await fetch(segment, { signal, credentials: "same-origin" });
  if (!res.ok) throw new Error(`${res.status}`);
  await res.arrayBuffer();
}
